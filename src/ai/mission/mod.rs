mod capability;
mod construction;
mod economy;
mod goal;
mod lifecycle;
mod pioneer;
mod progress;
mod record;
mod registry;

#[cfg(test)]
mod progress_tests;

pub use capability::{CapabilityRejection, check_assignment, role_capabilities};
pub use construction::BuildPlan;
pub use goal::{GoalEvaluation, GoalEvidence, evaluate_goal};
pub use record::{
    MissionCancellation, MissionCompletion, MissionReconciliation, MissionResolution, MissionView,
};
pub use registry::{MissionRegistry, MissionRegistryError};

use std::time::Instant;

use crate::ai::{DecisionError, DecisionState, propose_owned, tactics};
use crate::command::Arbiter;
use crate::domain::{Action, MissionSpec, Observation, Role};
use crate::fsm::{IndividualState, MissionState, StrategyState, TacticalState};
use crate::rules::time::{Phase, phase};

pub fn assign(
    observation: &Observation,
    state: &mut DecisionState,
    arbiter: &mut Arbiter<'_>,
    deadline: Instant,
) -> Result<(), DecisionError> {
    construction::refresh(observation, state);
    let is_night = phase(observation.round_no) == Some(Phase::Night);
    let returning = matches!(
        state.strategy,
        StrategyState::PrepareNight | StrategyState::Emergency
    );
    if is_night || returning {
        return tactics::defend(observation, state, arbiter);
    }
    assign_day(observation, state, arbiter, deadline)
}

fn assign_day(
    observation: &Observation,
    state: &mut DecisionState,
    arbiter: &mut Arbiter<'_>,
    deadline: Instant,
) -> Result<(), DecisionError> {
    let mut workers: Vec<_> = observation
        .team_our
        .roles
        .iter()
        .filter(|role| role.role_type == "worker")
        .collect();
    workers.sort_by_key(|role| role.id);
    for worker in workers {
        if Instant::now() >= deadline {
            break;
        }
        if state
            .build_plans
            .get(&worker.id)
            .is_some_and(|plan| plan.sent_round.is_some())
        {
            continue;
        }
        assign_worker(observation, worker, state, arbiter)?;
    }
    pioneer::assign(observation, state, arbiter)
}

fn assign_worker(
    observation: &Observation,
    worker: &Role,
    state: &mut DecisionState,
    arbiter: &mut Arbiter<'_>,
) -> Result<(), DecisionError> {
    let Some((spec, action)) = worker_selection(observation, worker, state) else {
        return Ok(());
    };
    let (tactical_state, individual_state) = state_for_action(&action);
    let building = matches!(action, crate::domain::Action::Build { .. });
    if !propose_owned(observation, state, arbiter, worker.id, action, spec)? {
        return Ok(());
    }
    if building {
        construction::mark_sent(state, worker.id, observation.round_no);
    }
    state.missions.insert(worker.id, MissionState::Executing);
    state.tactics.insert(worker.id, tactical_state);
    state.individuals.insert(worker.id, individual_state);
    Ok(())
}

fn worker_selection(
    observation: &Observation,
    worker: &Role,
    state: &mut DecisionState,
) -> Option<(MissionSpec, Action)> {
    if let Some(action) = construction::worker_action(observation, worker, state) {
        let plan = state.build_plans.get(&worker.id)?;
        return Some((MissionSpec::construction(plan.site, plan.kind), action));
    }
    economy::worker_action(observation, worker).map(|action| (MissionSpec::economy(), action))
}

fn state_for_action(action: &Action) -> (TacticalState, IndividualState) {
    match action {
        Action::Move(_) => (TacticalState::Approach, IndividualState::WaitingResult),
        _ => (TacticalState::Evaluate, IndividualState::WaitingResult),
    }
}
