pub mod cognition;
pub mod individual;
pub mod mission;
pub mod strategy;
pub mod tactics;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use crate::command::Arbiter;
use crate::domain::{Observation, Response};
use crate::event::WorldEvent;
use crate::fsm::{IndividualState, MissionState, StrategyState, TacticalState};
use crate::rules::build::BuildMask;

#[derive(Clone, Debug)]
pub struct DecisionState {
    pub strategy: StrategyState,
    pub missions: BTreeMap<i64, MissionState>,
    pub tactics: BTreeMap<i64, TacticalState>,
    pub individuals: BTreeMap<i64, IndividualState>,
    pub challenge: cognition::ChallengeMemory,
    pub emergency_clear: u8,
    pub build_plans: BTreeMap<i64, mission::BuildPlan>,
    pub bad_build_sites: BTreeSet<crate::domain::Pos>,
}

impl Default for DecisionState {
    fn default() -> Self {
        Self {
            strategy: StrategyState::Bootstrap,
            missions: BTreeMap::new(),
            tactics: BTreeMap::new(),
            individuals: BTreeMap::new(),
            challenge: cognition::ChallengeMemory::default(),
            emergency_clear: crate::rules::constants::ZERO_COUNTER,
            build_plans: BTreeMap::new(),
            bad_build_sites: BTreeSet::new(),
        }
    }
}

pub fn decide(
    observation: &Observation,
    events: &[WorldEvent],
    state: &mut DecisionState,
    build_mask: Option<&BuildMask>,
    deadline: Instant,
) -> Response {
    state.strategy = strategy::advance(
        observation,
        events,
        state.strategy,
        &mut state.emergency_clear,
    );
    let mut arbiter = Arbiter::new(observation, build_mask);
    mission::assign(observation, state, &mut arbiter, build_mask, deadline);
    let mut response = arbiter.finish();
    response.prompt = cognition::prepare_prompt(observation, &mut state.challenge);
    response
}
