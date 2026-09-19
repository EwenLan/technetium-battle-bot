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
    Action, ActionProposal, ActiveOwners, MissionSpec, Observation, OwnerAllocator, OwnerError,
    OwnerPath, Response,
};
use crate::event::{EventInbox, EventRecord};
use crate::fsm::{IndividualState, MissionState, StrategyState, TacticalState};

#[derive(Clone, Copy, Debug)]
struct RoleAssignment {
    spec: MissionSpec,
    owner: OwnerPath,
}

struct PreparedAction {
    proposal: ActionProposal,
    assignment: RoleAssignment,
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
    role_assignments: BTreeMap<i64, RoleAssignment>,
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
            role_assignments: BTreeMap::new(),
        }
    }
}

impl DecisionState {
    pub fn start_work(&mut self, reporter: i64) -> Result<OwnerPath, OwnerError> {
        let owner = self.active_owners.create_path(&mut self.owner_allocator)?;
        self.commit_owner(reporter, owner);
        Ok(owner)
    }

    fn prepare_action(
        &mut self,
        actor: i64,
        action: Action,
        spec: MissionSpec,
    ) -> Result<PreparedAction, OwnerError> {
        let reporter = action_reporter(actor, &action);
        let current = self.current_assignment(reporter, spec);
        match current {
            Some(assignment) => self.prepare_for_assignment(actor, action, assignment),
            None => self.prepare_new_assignment(actor, action, spec),
        }
    }

    fn prepare_for_assignment(
        &mut self,
        actor: i64,
        action: Action,
        assignment: RoleAssignment,
    ) -> Result<PreparedAction, OwnerError> {
        let owner = self
            .active_owners
            .create_intent(&mut self.owner_allocator, &assignment.owner)?;
        Ok(PreparedAction {
            proposal: ActionProposal::new(actor, owner, action),
            assignment,
            replaces_assignment: false,
        })
    }

    fn prepare_new_assignment(
        &mut self,
        actor: i64,
        action: Action,
        spec: MissionSpec,
    ) -> Result<PreparedAction, OwnerError> {
        let owner = self.active_owners.create_path(&mut self.owner_allocator)?;
        let assignment = RoleAssignment {
            spec,
            owner: owner.assignment(),
        };
        Ok(PreparedAction {
            proposal: ActionProposal::new(actor, owner, action),
            assignment,
            replaces_assignment: true,
        })
    }

    fn commit_proposal(&mut self, prepared: &PreparedAction) {
        let reporter = prepared.proposal.reporter();
        if prepared.replaces_assignment {
            self.replace_assignment(reporter, prepared.assignment);
        } else if let Some(previous) = self.role_owners.get(&reporter).copied() {
            self.deactivate_intent(&previous);
        }
        self.role_owners
            .insert(reporter, *prepared.proposal.owner());
    }

    fn discard_proposal(&mut self, prepared: &PreparedAction) {
        if prepared.replaces_assignment {
            self.active_owners
                .deactivate_assignment(&prepared.assignment.owner);
        } else {
            self.deactivate_intent(prepared.proposal.owner());
        }
    }

    fn commit_owner(&mut self, reporter: i64, owner: OwnerPath) {
        if let Some(previous) = self.role_owners.insert(reporter, owner) {
            self.active_owners.deactivate_assignment(&previous);
        }
    }

    fn current_assignment(&self, reporter: i64, spec: MissionSpec) -> Option<RoleAssignment> {
        self.role_assignments
            .get(&reporter)
            .copied()
            .filter(|assignment| {
                assignment.spec == spec && self.active_owners.is_current(&assignment.owner)
            })
    }

    fn replace_assignment(&mut self, reporter: i64, assignment: RoleAssignment) {
        let previous = self
            .role_assignments
            .insert(reporter, assignment)
            .map(|assignment| assignment.owner)
            .or_else(|| self.role_owners.get(&reporter).copied());
        if let Some(previous) = previous {
            self.active_owners.deactivate_assignment(&previous);
        }
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
) -> Result<bool, OwnerError> {
    let prepared = state.prepare_action(actor, action, spec)?;
    if arbiter.propose(&prepared.proposal, &state.active_owners) {
        state.commit_proposal(&prepared);
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
) -> Result<Response, OwnerError> {
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
