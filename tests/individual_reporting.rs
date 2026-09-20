use technetium_battle_bot::ai::DecisionState;
use technetium_battle_bot::ai::individual::{reconcile, record_committed};
use technetium_battle_bot::command::Arbiter;
use technetium_battle_bot::domain::{Action, ActionProposal, ActiveOwners, OwnerAllocator, Pos};
use technetium_battle_bot::event::{ReportReason, ReportScope, ReportStatus};
use technetium_battle_bot::fsm::{IndividualState, MissionState, TacticalState};
use technetium_battle_bot::protocol::decode;
use technetium_battle_bot::rules::constants::MIN_ACTION_ROUNDS;

const DAY_REQUEST: &str = include_str!("fixtures/day.json");
const NIGHT_REQUEST: &str = include_str!("fixtures/night.json");
const WORKER_ID: i64 = 10;
const WEAPON_ID: i64 = 20;
const MOVE_X: i32 = 4;
const MOVE_Y: i32 = 4;

mod support;

#[test]
fn rejected_action_does_not_enter_waiting_state() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let worker = observation
        .team_our
        .roles
        .iter()
        .find(|role| role.id == WORKER_ID)
        .expect("worker");
    let mut arbiter = Arbiter::new(&observation);
    let mut owners = ActiveOwners::default();
    let mut allocator = OwnerAllocator::default();
    let proposal = support::owned_action(
        &mut owners,
        &mut allocator,
        WORKER_ID,
        Action::Move(worker.pos),
    );
    assert!(!arbiter.propose(&proposal, &owners));
    let mut state = DecisionState::default();
    let arbitration = arbiter.finish(&owners);
    record_committed(&observation, &arbitration, &mut state);
    assert!(state.pending_actions.is_empty());
    assert!(!state.individuals.contains_key(&WORKER_ID));
    assert!(!state.role_owners.contains_key(&WORKER_ID));
}

#[test]
fn rejected_by_judger_action_reports_failure_to_parent_states() {
    let mut observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut arbiter = Arbiter::new(&observation);
    let mut state = DecisionState::default();
    let proposal = state_action(
        &mut state,
        WORKER_ID,
        Action::Move(Pos {
            x: MOVE_X,
            y: MOVE_Y,
        }),
    );
    assert!(arbiter.propose(&proposal, &state.active_owners));
    let arbitration = arbiter.finish(&state.active_owners);
    record_committed(&observation, &arbitration, &mut state);
    assert_eq!(
        state.individuals.get(&WORKER_ID),
        Some(&IndividualState::WaitingResult)
    );
    observation.round_no += MIN_ACTION_ROUNDS;
    observation
        .last_round_role_action_results
        .insert(WORKER_ID.to_string(), false);
    let reports = reconcile(&observation, &mut state);
    assert_eq!(
        reports.len(),
        technetium_battle_bot::rules::constants::SINGLE_ITEM
    );
    let report = reports.first().expect("report");
    assert_eq!(report.status, ReportStatus::Failed);
    assert_eq!(report.reason, ReportReason::ActionRejected);
    assert_eq!(
        report.scope,
        ReportScope::Step(report.owner.intent().expect("intent"))
    );
    assert!(state.active_owners.is_current(&report.owner));
    assert_eq!(
        state.individuals.get(&WORKER_ID),
        Some(&IndividualState::Recovering)
    );
    assert_eq!(state.missions.get(&WORKER_ID), Some(&MissionState::Blocked));
    assert_eq!(state.tactics.get(&WORKER_ID), Some(&TacticalState::Replan));
}

#[test]
fn weapon_feedback_is_attributed_to_its_controller() {
    let mut observation = decode(NIGHT_REQUEST.as_bytes())
        .expect("fixture")
        .observation;
    let target = observation
        .robot
        .as_ref()
        .expect("robots")
        .roles
        .first()
        .expect("robot")
        .pos;
    let mut arbiter = Arbiter::new(&observation);
    let mut state = DecisionState::default();
    let proposal = state_action(
        &mut state,
        WEAPON_ID,
        Action::Attack {
            controller: WORKER_ID,
            targets: vec![target],
        },
    );
    assert!(arbiter.propose(&proposal, &state.active_owners));
    let arbitration = arbiter.finish(&state.active_owners);
    record_committed(&observation, &arbitration, &mut state);
    observation.round_no += MIN_ACTION_ROUNDS;
    observation
        .last_round_role_action_results
        .insert(WEAPON_ID.to_string(), true);
    let reports = reconcile(&observation, &mut state);
    assert_eq!(reports.first().expect("report").reporter, WORKER_ID);
    assert_eq!(
        reports.first().expect("report").status,
        ReportStatus::Progress
    );
    assert_eq!(
        reports.first().expect("report").reason,
        ReportReason::ActionLegal
    );
    assert_eq!(
        state.individuals.get(&WORKER_ID),
        Some(&IndividualState::Idle)
    );
}

#[test]
fn skipped_round_feedback_is_not_attributed_to_old_action() {
    let mut observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut state = DecisionState::default();
    let action = Action::Move(Pos {
        x: MOVE_X,
        y: MOVE_Y,
    });
    commit_action(&observation, &mut state, WORKER_ID, action);
    observation.round_no += MIN_ACTION_ROUNDS + MIN_ACTION_ROUNDS;
    observation
        .last_round_role_action_results
        .insert(WORKER_ID.to_string(), true);
    let reports = reconcile(&observation, &mut state);
    assert_eq!(
        reports.first().expect("report").status,
        ReportStatus::AtRisk
    );
    assert_eq!(
        reports.first().expect("report").reason,
        ReportReason::ActionResultUnknown
    );
}

#[test]
fn stale_owner_report_is_kept_without_advancing_replacement_work() {
    let mut observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut state = DecisionState::default();
    let action = Action::Move(Pos {
        x: MOVE_X,
        y: MOVE_Y,
    });
    commit_action(&observation, &mut state, WORKER_ID, action);
    let old_owner = state
        .pending_actions
        .get(&WORKER_ID)
        .expect("pending action")
        .owner;
    state.start_work(WORKER_ID).expect("replacement work");
    state.individuals.insert(WORKER_ID, IndividualState::Moving);

    observation.round_no += MIN_ACTION_ROUNDS;
    observation
        .last_round_role_action_results
        .insert(WORKER_ID.to_string(), false);
    let reports = reconcile(&observation, &mut state);

    assert_eq!(reports.first().expect("report").owner, old_owner);
    assert!(!state.active_owners.is_current(&old_owner));
    assert_eq!(
        state.individuals.get(&WORKER_ID),
        Some(&IndividualState::Moving)
    );
    assert_eq!(
        reports.first().expect("report").status,
        ReportStatus::Failed
    );
}

fn state_action(state: &mut DecisionState, actor: i64, action: Action) -> ActionProposal {
    let reporter = match &action {
        Action::Attack { controller, .. } => *controller,
        _ => actor,
    };
    let owner = state.start_work(reporter).expect("owner path");
    ActionProposal::new(actor, owner, action)
}

fn commit_action(
    observation: &technetium_battle_bot::domain::Observation,
    state: &mut DecisionState,
    actor: i64,
    action: Action,
) {
    let proposal = state_action(state, actor, action);
    let mut arbiter = Arbiter::new(observation);
    assert!(arbiter.propose(&proposal, &state.active_owners));
    let arbitration = arbiter.finish(&state.active_owners);
    record_committed(observation, &arbitration, state);
}
