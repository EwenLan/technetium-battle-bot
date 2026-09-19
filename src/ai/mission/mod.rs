mod construction;
mod economy;
mod pioneer;

pub use construction::BuildPlan;

use std::time::Instant;

use crate::ai::{DecisionState, tactics};
use crate::command::Arbiter;
use crate::domain::Observation;
use crate::fsm::{IndividualState, MissionState, StrategyState, TacticalState};
use crate::rules::time::{Phase, phase};

pub fn assign(
    observation: &Observation,
    state: &mut DecisionState,
    arbiter: &mut Arbiter<'_>,
    deadline: Instant,
) {
    construction::refresh(observation, state);
    let is_night = phase(observation.round_no) == Some(Phase::Night);
    let returning = matches!(
        state.strategy,
        StrategyState::PrepareNight | StrategyState::Emergency
    );
    if is_night || returning {
        tactics::defend(observation, arbiter);
        return;
    }
    assign_day(observation, state, arbiter, deadline);
}

fn assign_day(
    observation: &Observation,
    state: &mut DecisionState,
    arbiter: &mut Arbiter<'_>,
    deadline: Instant,
) {
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
        assign_worker(observation, worker, state, arbiter);
    }
    pioneer::assign(observation, state, arbiter);
}

fn assign_worker(
    observation: &Observation,
    worker: &crate::domain::Role,
    state: &mut DecisionState,
    arbiter: &mut Arbiter<'_>,
) {
    let action = construction::worker_action(observation, worker, state)
        .or_else(|| economy::worker_action(observation, worker));
    let Some(action) = action else { return };
    let state_after = state_for_action(&action);
    let building = matches!(action, crate::domain::Action::Build { .. });
    if !arbiter.propose(worker.id, action) {
        return;
    }
    if building {
        construction::mark_sent(state, worker.id, observation.round_no);
    }
    state.missions.insert(worker.id, MissionState::Executing);
    state.tactics.insert(worker.id, state_after.0);
    state.individuals.insert(worker.id, state_after.1);
}

fn state_for_action(action: &crate::domain::Action) -> (TacticalState, IndividualState) {
    match action {
        crate::domain::Action::Move(_) => (TacticalState::Approach, IndividualState::WaitingResult),
        _ => (TacticalState::Evaluate, IndividualState::WaitingResult),
    }
}
