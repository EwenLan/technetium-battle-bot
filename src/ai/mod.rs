pub mod cognition;
pub mod individual;
pub mod mission;
pub mod strategy;
pub mod tactics;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use crate::command::Arbiter;
use crate::domain::{ActiveOwners, Observation, OwnerAllocator, OwnerError, OwnerPath, Response};
use crate::event::{EventInbox, EventRecord};
use crate::fsm::{IndividualState, MissionState, StrategyState, TacticalState};

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
        }
    }
}

impl DecisionState {
    pub fn start_work(&mut self, reporter: i64) -> Result<OwnerPath, OwnerError> {
        let owner = self.active_owners.create_path(&mut self.owner_allocator)?;
        if let Some(previous) = self.role_owners.insert(reporter, owner) {
            self.active_owners.deactivate_mission(previous.mission().id);
        }
        Ok(owner)
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
    mission::assign(observation, state, &mut arbiter, deadline);
    individual::record_committed(observation.round_no, arbiter.accepted(), state)?;
    let mut response = arbiter.finish();
    response.prompt = cognition::prepare_prompt(observation, &mut state.challenge);
    Ok(response)
}
