use crate::ai::DecisionState;
use crate::domain::{Action, Observation};
use crate::event::{ExecutionReport, ReportReason, ReportScope, ReportStatus};
use crate::fsm::{IndividualState, MissionState, TacticalState};
use crate::rules::constants::{MIN_ACTION_ROUNDS, NO_HEALTH};

#[derive(Clone, Debug)]
pub struct PendingAction {
    pub actor: i64,
    pub owner: i64,
    pub action: Action,
    pub sent_round: i32,
}

pub fn record_committed(round: i32, accepted: &[(i64, Action)], state: &mut DecisionState) {
    for (actor, action) in accepted {
        let owner = action_owner(*actor, action);
        state.pending_actions.insert(
            owner,
            PendingAction {
                actor: *actor,
                owner,
                action: action.clone(),
                sent_round: round,
            },
        );
        state
            .individuals
            .insert(owner, IndividualState::WaitingResult);
    }
}

pub fn reconcile(observation: &Observation, state: &mut DecisionState) -> Vec<ExecutionReport> {
    let mut reports = Vec::new();
    let pending = std::mem::take(&mut state.pending_actions);
    for (owner, action) in pending {
        if observation.round_no <= action.sent_round {
            state.pending_actions.insert(owner, action);
            continue;
        }
        let report = action_report(observation, &action);
        update_states(state, &report);
        reports.push(report);
    }
    reports
}

fn action_owner(actor: i64, action: &Action) -> i64 {
    match action {
        Action::Attack { controller, .. } => *controller,
        _ => actor,
    }
}

fn action_report(observation: &Observation, pending: &PendingAction) -> ExecutionReport {
    let dead = observation
        .team_our
        .roles
        .iter()
        .any(|role| role.id == pending.owner && role.health.is_some_and(|hp| hp <= NO_HEALTH));
    let result = if pending.sent_round.checked_add(MIN_ACTION_ROUNDS) == Some(observation.round_no)
    {
        observation
            .last_round_role_action_results
            .get(&pending.actor.to_string())
    } else {
        None
    };
    let (status, reason) = match (dead, result) {
        (true, _) => (ReportStatus::Failed, ReportReason::OwnerDied),
        (_, Some(true)) => (ReportStatus::Progress, ReportReason::ActionLegal),
        (_, Some(false)) => (ReportStatus::Failed, ReportReason::ActionRejected),
        _ => (ReportStatus::AtRisk, ReportReason::ActionResultUnknown),
    };
    ExecutionReport {
        scope: ReportScope::Step,
        status,
        owner: pending.owner,
        reason,
        observed_round: observation.round_no,
    }
}

fn update_states(state: &mut DecisionState, report: &ExecutionReport) {
    if report.reason == ReportReason::OwnerDied {
        state
            .individuals
            .insert(report.owner, IndividualState::Dead);
        state.missions.insert(report.owner, MissionState::Cancelled);
        state.tactics.insert(report.owner, TacticalState::Cancelled);
        return;
    }
    if report.status == ReportStatus::Progress {
        state
            .individuals
            .insert(report.owner, IndividualState::Idle);
        return;
    }
    state
        .individuals
        .insert(report.owner, IndividualState::Recovering);
    state.missions.insert(report.owner, MissionState::Blocked);
    state.tactics.insert(report.owner, TacticalState::Replan);
}
