pub mod cognition;
pub mod individual;
pub mod mission;
pub mod strategy;
pub mod tactics;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use crate::command::Arbiter;
use crate::domain::{
    Action, ActionProposal, ActiveOwners, MissionId, MissionSpec, Observation, OwnerAllocator,
    OwnerError, OwnerPath, Response,
};
use crate::event::{EventInbox, EventRecord};
use crate::fsm::{IndividualState, MissionState, StrategyState, TacticalState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionError {
    Owner(OwnerError),
    Mission(mission::MissionRegistryError),
}

impl From<OwnerError> for DecisionError {
    fn from(error: OwnerError) -> Self {
        Self::Owner(error)
    }
}

impl From<mission::MissionRegistryError> for DecisionError {
    fn from(error: mission::MissionRegistryError) -> Self {
        Self::Mission(error)
    }
}

struct PreparedAction {
    proposal: ActionProposal,
    spec: MissionSpec,
    assignment: OwnerPath,
    replaces_assignment: bool,
}

#[derive(Clone, Debug)]
pub struct DecisionState {
    pub strategy: StrategyState,
    pub missions: BTreeMap<i64, MissionState>,
    pub tactics: BTreeMap<i64, TacticalState>,
    pub individuals: BTreeMap<i64, IndividualState>,
    pub pending_actions: BTreeMap<i64, individual::PendingAction>,
    pub reports: Vec<crate::event::ExecutionReport>,
    pub challenge: cognition::ChallengeMemory,
    pub emergency_clear: u8,
    pub build_plans: BTreeMap<i64, mission::BuildPlan>,
    pub bad_build_sites: BTreeSet<crate::domain::Pos>,
    pub event_inbox: EventInbox,
    pub active_owners: ActiveOwners,
    pub owner_allocator: OwnerAllocator,
    pub role_owners: BTreeMap<i64, OwnerPath>,
    mission_registry: mission::MissionRegistry,
}

impl Default for DecisionState {
    fn default() -> Self {
        Self {
            strategy: StrategyState::Bootstrap,
            missions: BTreeMap::new(),
            tactics: BTreeMap::new(),
            individuals: BTreeMap::new(),
            pending_actions: BTreeMap::new(),
            reports: Vec::new(),
            challenge: cognition::ChallengeMemory::default(),
            emergency_clear: crate::rules::constants::ZERO_COUNTER,
            build_plans: BTreeMap::new(),
            bad_build_sites: BTreeSet::new(),
            event_inbox: EventInbox::default(),
            active_owners: ActiveOwners::default(),
            owner_allocator: OwnerAllocator::default(),
            role_owners: BTreeMap::new(),
            mission_registry: mission::MissionRegistry::default(),
        }
    }
}

impl DecisionState {
    pub fn start_work(&mut self, reporter: i64) -> Result<OwnerPath, OwnerError> {
        let owner = self.active_owners.create_path(&mut self.owner_allocator)?;
        let cancelled = self.mission_registry.cancel_current(reporter);
        self.apply_cancellations(cancelled);
        self.commit_owner(reporter, owner);
        Ok(owner)
    }

    pub fn mission_view(&self, mission: MissionId) -> Option<mission::MissionView> {
        self.mission_registry.view(mission)
    }

    pub(crate) fn reconcile_missions(&mut self, observation: &Observation, events: &[EventRecord]) {
        let reconciliation = self.mission_registry.reconcile(observation, events);
        for resolution in reconciliation.resolved() {
            self.apply_resolution(*resolution);
        }
    }

    fn prepare_action(
        &mut self,
        actor: i64,
        action: Action,
        spec: MissionSpec,
    ) -> Result<PreparedAction, DecisionError> {
        let reporter = action_reporter(actor, &action);
        let current = self.current_assignment(reporter, &spec);
        match current {
            Some(assignment) => self.prepare_for_assignment(actor, action, spec, assignment),
            None => self.prepare_new_assignment(actor, action, spec),
        }
    }

    fn prepare_for_assignment(
        &mut self,
        actor: i64,
        action: Action,
        spec: MissionSpec,
        assignment: OwnerPath,
    ) -> Result<PreparedAction, DecisionError> {
        let owner = self
            .active_owners
            .create_intent(&mut self.owner_allocator, &assignment)?;
        Ok(PreparedAction {
            proposal: ActionProposal::new(actor, owner, action),
            spec,
            assignment,
            replaces_assignment: false,
        })
    }

    fn prepare_new_assignment(
        &mut self,
        actor: i64,
        action: Action,
        spec: MissionSpec,
    ) -> Result<PreparedAction, DecisionError> {
        let owner = self.active_owners.create_path(&mut self.owner_allocator)?;
        Ok(PreparedAction {
            proposal: ActionProposal::new(actor, owner, action),
            spec,
            assignment: owner.assignment(),
            replaces_assignment: true,
        })
    }

    fn commit_proposal(&mut self, prepared: &PreparedAction) -> Result<(), DecisionError> {
        let reporter = prepared.proposal.reporter();
        if prepared.replaces_assignment {
            let cancelled = self.mission_registry.activate_for_action(
                reporter,
                prepared.spec.clone(),
                prepared.assignment,
            )?;
            self.apply_cancellations(cancelled);
        } else {
            self.mission_registry
                .resume_for_action(&prepared.assignment)?;
            if let Some(previous) = self.role_owners.get(&reporter).copied() {
                self.deactivate_intent(&previous);
            }
        }
        self.role_owners
            .insert(reporter, *prepared.proposal.owner());
        self.missions.insert(reporter, MissionState::Executing);
        Ok(())
    }

    fn discard_proposal(&mut self, prepared: &PreparedAction) {
        if prepared.replaces_assignment {
            self.active_owners
                .deactivate_assignment(&prepared.assignment);
        } else {
            self.deactivate_intent(prepared.proposal.owner());
        }
    }

    fn commit_owner(&mut self, reporter: i64, owner: OwnerPath) {
        if let Some(previous) = self.role_owners.insert(reporter, owner) {
            self.active_owners.deactivate_assignment(&previous);
        }
    }

    fn current_assignment(&self, reporter: i64, spec: &MissionSpec) -> Option<OwnerPath> {
        self.mission_registry
            .current_assignment(reporter, spec)
            .filter(|owner| self.active_owners.is_current(owner))
    }

    fn apply_cancellations(&mut self, cancelled: Vec<mission::MissionCancellation>) {
        for cancellation in cancelled {
            let assignee = cancellation.assignee();
            let owner = cancellation.owner();
            self.active_owners.deactivate_assignment(&owner);
            if self
                .role_owners
                .get(&assignee)
                .is_some_and(|current| current.mission() == owner.mission())
            {
                self.role_owners.remove(&assignee);
            }
            self.missions.insert(assignee, MissionState::Cancelled);
        }
    }

    fn apply_resolution(&mut self, resolution: mission::MissionResolution) {
        let assignee = resolution.assignee();
        let owner = resolution.owner();
        self.active_owners.deactivate_assignment(&owner);
        if self
            .role_owners
            .get(&assignee)
            .is_some_and(|current| current.mission() == owner.mission())
        {
            self.role_owners.remove(&assignee);
        }
        self.missions.insert(assignee, resolution.state());
    }

    pub(crate) fn block_mission(&mut self, owner: &OwnerPath) {
        self.mission_registry.mark_blocked(owner);
    }

    pub(crate) fn record_mission_action(
        &mut self,
        owner: &OwnerPath,
        actor: i64,
        action: &Action,
        round: i32,
    ) {
        self.mission_registry
            .record_action_commit(owner, actor, action, round);
    }

    pub(crate) fn cancel_mission(&mut self, owner: &OwnerPath) {
        let cancelled = self.mission_registry.cancel_owner(owner);
        self.apply_cancellations(cancelled);
    }

    fn deactivate_intent(&mut self, owner: &OwnerPath) {
        if let Some(intent) = owner.intent() {
            self.active_owners.deactivate_intent(intent.id);
        }
    }
}

pub(crate) fn propose_owned(
    state: &mut DecisionState,
    arbiter: &mut Arbiter<'_>,
    actor: i64,
    action: Action,
    spec: MissionSpec,
) -> Result<bool, DecisionError> {
    let prepared = state.prepare_action(actor, action, spec)?;
    if arbiter.propose(&prepared.proposal, &state.active_owners) {
        state.commit_proposal(&prepared)?;
        return Ok(true);
    }
    state.discard_proposal(&prepared);
    Ok(false)
}

fn action_reporter(actor: i64, action: &Action) -> i64 {
    match action {
        Action::Attack { controller, .. } => *controller,
        _ => actor,
    }
}

pub fn decide(
    observation: &Observation,
    events: &[EventRecord],
    state: &mut DecisionState,
    deadline: Instant,
) -> Result<Response, DecisionError> {
    state.reconcile_missions(observation, events);
    state.strategy = strategy::advance(
        observation,
        events,
        state.strategy,
        &mut state.emergency_clear,
    );
    let mut arbiter = Arbiter::new(observation);
    mission::assign(observation, state, &mut arbiter, deadline)?;
    let arbitration = arbiter.finish(&state.active_owners);
    individual::record_committed(observation.round_no, &arbitration, state);
    let mut response = arbitration.response;
    response.prompt = cognition::prepare_prompt(observation, &mut state.challenge);
    Ok(response)
}
