use super::{MissionFailureReason, MissionRegistry};
use crate::ai::individual::{reconcile, record_committed};
use crate::ai::{DecisionState, propose_owned};
use crate::command::Arbiter;
use crate::domain::{
    Action, ActiveOwners, MissionId, MissionSpec, Observation, OwnerAllocator, OwnerPath, Pos,
    PriorityClass, RetryPolicy,
};
use crate::event::{EventRecord, ReportReason, WorldEvent};
use crate::fsm::{IndividualState, MissionState, TacticalState};
use crate::protocol::decode;
use crate::rules::constants::{COUNTER_INCREMENT, MIN_ACTION_ROUNDS, ZERO_COUNTER};

const DAY_REQUEST: &str = include_str!("../../../tests/fixtures/day.json");
const WORKER_ID: i64 = 10;
const CHILD_ID: i64 = 11;
const MOVE_X: i32 = 4;
const MOVE_Y: i32 = 4;
const RETRY_LIMIT: u8 = 2;
const SINGLE_RETRY_LIMIT: u8 = 1;
const PROGRESS_EVENT_ID: u64 = 90;
const START_X: i32 = 3;
const DEPENDENT_RESOLUTION_COUNT: usize = 2;

#[test]
fn repeated_rejections_exhaust_the_mission_retry_policy() {
    let mut observation = observation();
    let mut state = DecisionState::default();
    let spec = retry_spec(RETRY_LIMIT);
    let first = submit_move(&observation, &mut state, spec.clone());

    reject_pending(&mut observation, &mut state);
    let blocked = state.mission_view(first.mission().id).expect("mission");
    assert_eq!(blocked.state(), MissionState::Blocked);
    assert_eq!(blocked.retry_count(), COUNTER_INCREMENT);

    let second = submit_move(&observation, &mut state, spec);
    reject_pending(&mut observation, &mut state);
    let failed = state.mission_view(second.mission().id).expect("mission");

    assert_eq!(failed.state(), MissionState::Failed);
    assert_eq!(failed.retry_count(), RETRY_LIMIT);
    let failure = failed.failure().expect("failure metadata");
    assert_eq!(failure.attempts(), RETRY_LIMIT);
    assert_eq!(failure.observed_round(), observation.round_no);
    assert_eq!(failure.reason(), MissionFailureReason::RetryExhausted);
    assert!(!state.active_owners.is_current(&second));
    assert!(!state.role_owners.contains_key(&WORKER_ID));
    assert_eq!(
        state.individuals.get(&WORKER_ID),
        Some(&IndividualState::Idle)
    );
    assert_eq!(state.tactics.get(&WORKER_ID), Some(&TacticalState::Failed));

    observation.round_no += MIN_ACTION_ROUNDS;
    state.reconcile_missions(&observation, &[movement_event(observation.round_no)]);
    assert_eq!(
        state
            .mission_view(second.mission().id)
            .expect("terminal mission")
            .retry_count(),
        RETRY_LIMIT
    );
}

#[test]
fn observed_progress_resets_the_rejection_counter() {
    let (mut registry, owner, mission, _) = active_registry(retry_spec(RETRY_LIMIT));
    assert!(
        registry
            .record_action_rejection(&owner, observation().round_no)
            .is_empty()
    );
    assert_eq!(
        registry.view(mission).expect("mission").retry_count(),
        COUNTER_INCREMENT
    );
    let mut input = observation();
    input.round_no += MIN_ACTION_ROUNDS;
    let event = movement_event(input.round_no);

    registry.reconcile(&input, &[event]);

    let view = registry.view(mission).expect("mission");
    assert_eq!(view.retry_count(), ZERO_COUNTER);
    assert_eq!(view.state(), MissionState::Blocked);
}

#[test]
fn failed_contract_is_suppressed_but_changed_policy_can_run() {
    let mut observation = observation();
    let mut state = DecisionState::default();
    let failed_spec = retry_spec(SINGLE_RETRY_LIMIT);
    let failed_owner = submit_move(&observation, &mut state, failed_spec.clone());
    reject_pending(&mut observation, &mut state);
    assert_eq!(
        state
            .mission_view(failed_owner.mission().id)
            .expect("mission")
            .state(),
        MissionState::Failed
    );

    assert!(!try_move(&observation, &mut state, failed_spec));
    let changed = retry_spec(SINGLE_RETRY_LIMIT).with_priority(PriorityClass::Q2Deadline);
    assert!(try_move(&observation, &mut state, changed));
}

#[test]
fn retry_exhaustion_cancels_dependent_missions() {
    let (mut registry, owner, root, child_owner) = active_registry(retry_spec(SINGLE_RETRY_LIMIT));
    let child = registry
        .register(
            CHILD_ID,
            MissionSpec::economy().with_dependencies([root]),
            child_owner,
        )
        .expect("child mission");

    let resolved = registry.record_action_rejection(&owner, observation().round_no);

    assert_eq!(resolved.len(), DEPENDENT_RESOLUTION_COUNT);
    assert_eq!(
        registry.view(root).expect("root").state(),
        MissionState::Failed
    );
    assert_eq!(
        registry.view(child).expect("child").state(),
        MissionState::Cancelled
    );
}

fn submit_move(
    observation: &Observation,
    state: &mut DecisionState,
    spec: MissionSpec,
) -> OwnerPath {
    assert!(try_move(observation, state, spec));
    *state.role_owners.get(&WORKER_ID).expect("worker owner")
}

fn try_move(observation: &Observation, state: &mut DecisionState, spec: MissionSpec) -> bool {
    let mut arbiter = Arbiter::new(observation);
    let accepted = propose_owned(
        observation,
        state,
        &mut arbiter,
        WORKER_ID,
        Action::Move(destination()),
        spec,
    )
    .expect("proposal");
    let arbitration = arbiter.finish(&state.active_owners);
    record_committed(observation, &arbitration, state);
    accepted
}

fn reject_pending(observation: &mut Observation, state: &mut DecisionState) {
    observation.round_no += MIN_ACTION_ROUNDS;
    observation
        .last_round_role_action_results
        .insert(WORKER_ID.to_string(), false);
    let reports = reconcile(observation, state);
    assert_eq!(
        reports.first().map(|report| report.reason),
        Some(ReportReason::ActionRejected)
    );
}

fn active_registry(spec: MissionSpec) -> (MissionRegistry, OwnerPath, MissionId, OwnerPath) {
    let mut registry = MissionRegistry::default();
    let mut owners = ActiveOwners::default();
    let mut allocator = OwnerAllocator::default();
    let owner = owners.create_assignment(&mut allocator).expect("owner");
    let spare = owners
        .create_assignment(&mut allocator)
        .expect("spare owner");
    let mission = registry.register(WORKER_ID, spec, owner).expect("mission");
    registry.assign(mission).expect("ready");
    registry.activate(mission).expect("assigned");
    (registry, owner, mission, spare)
}

fn movement_event(round: i32) -> EventRecord {
    EventRecord {
        id: PROGRESS_EVENT_ID,
        observed_round: round,
        event: WorldEvent::UnitMoved {
            id: WORKER_ID,
            from: Pos {
                x: START_X,
                y: MOVE_Y,
            },
            to: destination(),
        },
    }
}

fn retry_spec(limit: u8) -> MissionSpec {
    MissionSpec::economy().with_retry_policy(RetryPolicy::bounded(limit))
}

fn observation() -> Observation {
    decode(DAY_REQUEST.as_bytes()).expect("fixture").observation
}

const fn destination() -> Pos {
    Pos {
        x: MOVE_X,
        y: MOVE_Y,
    }
}
