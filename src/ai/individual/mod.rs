use crate::ai::DecisionState;
use crate::domain::{Action, Observation, OwnerError, OwnerPath};
use crate::event::{ExecutionReport, ReportReason, ReportScope, ReportStatus};
use crate::fsm::{IndividualState, MissionState, TacticalState};
use crate::rules::constants::{MIN_ACTION_ROUNDS, NO_HEALTH};

#[derive(Clone, Debug)]
pub struct PendingAction {
    pub actor: i64,
    pub reporter: i64,
    pub owner: OwnerPath,
    pub action: Action,
    pub sent_round: i32,
}

pub fn record_committed(
    round: i32,
    accepted: &[(i64, Action)],
    state: &mut DecisionState,
) -> Result<(), OwnerError> {
    for (actor, action) in accepted {
        let reporter = action_reporter(*actor, action);
        let owner = state.start_work(reporter)?;
        state.pending_actions.insert(
            reporter,
            PendingAction {
                actor: *actor,
                reporter,
                owner,
                action: action.clone(),
                sent_round: round,
            },
        );
        state
            .individuals
            .insert(reporter, IndividualState::WaitingResult);
    }
    Ok(())
}

pub fn reconcile(observation: &Observation, state: &mut DecisionState) -> Vec<ExecutionReport> {
    let mut reports = Vec::new();
    let pending = std::mem::take(&mut state.pending_actions);
    for (reporter, action) in pending {
        if observation.round_no <= action.sent_round {
            state.pending_actions.insert(reporter, action);
            continue;
        }
        let report = action_report(observation, &action);
        update_states(state, &report);
        reports.push(report);
    }
    reports
}

fn action_reporter(actor: i64, action: &Action) -> i64 {
    match action {
        Action::Attack { controller, .. } => *controller,
        _ => actor,
    }
}

fn action_report(observation: &Observation, pending: &PendingAction) -> ExecutionReport {
    let dead =
        observation.team_our.roles.iter().any(|role| {
            role.id == pending.reporter && role.health.is_some_and(|hp| hp <= NO_HEALTH)
        });
    let result = action_result(observation, pending);
    let (status, reason) = report_outcome(dead, result);
    ExecutionReport {
        scope: report_scope(&pending.owner),
        status,
        owner: pending.owner,
        reporter: pending.reporter,
        reason,
        observed_round: observation.round_no,
    }
}

fn action_result<'a>(observation: &'a Observation, pending: &PendingAction) -> Option<&'a bool> {
    if pending.sent_round.checked_add(MIN_ACTION_ROUNDS) != Some(observation.round_no) {
        return None;
    }
    observation
        .last_round_role_action_results
        .get(&pending.actor.to_string())
}

fn report_outcome(dead: bool, result: Option<&bool>) -> (ReportStatus, ReportReason) {
    match (dead, result) {
        (true, _) => (ReportStatus::Failed, ReportReason::OwnerDied),
        (_, Some(true)) => (ReportStatus::Progress, ReportReason::ActionLegal),
        (_, Some(false)) => (ReportStatus::Failed, ReportReason::ActionRejected),
        _ => (ReportStatus::AtRisk, ReportReason::ActionResultUnknown),
    }
}

fn report_scope(owner: &OwnerPath) -> ReportScope {
    if let Some(intent) = owner.intent() {
        return ReportScope::Step(intent);
    }
    if let Some(plan) = owner.plan() {
        return ReportScope::Plan(plan);
    }
    ReportScope::Mission(owner.mission())
}

fn update_states(state: &mut DecisionState, report: &ExecutionReport) {
    if !state.active_owners.is_current(&report.owner) {
        return;
    }
    if report.reason == ReportReason::OwnerDied {
        update_dead_states(state, report.reporter);
        return;
    }
    if report.status == ReportStatus::Progress {
        state
            .individuals
            .insert(report.reporter, IndividualState::Idle);
        return;
    }
    update_recovery_states(state, report.reporter);
}

fn update_dead_states(state: &mut DecisionState, reporter: i64) {
    state.individuals.insert(reporter, IndividualState::Dead);
    state.missions.insert(reporter, MissionState::Cancelled);
    state.tactics.insert(reporter, TacticalState::Cancelled);
}

fn update_recovery_states(state: &mut DecisionState, reporter: i64) {
    state
        .individuals
        .insert(reporter, IndividualState::Recovering);
    state.missions.insert(reporter, MissionState::Blocked);
    state.tactics.insert(reporter, TacticalState::Replan);
}
