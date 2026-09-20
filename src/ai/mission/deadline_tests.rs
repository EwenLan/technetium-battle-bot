use std::time::{Duration, Instant};

use super::assign;
use super::deadline::{challenge_spec, construction_spec, defense_spec, economy_spec};
use crate::ai::{DecisionState, propose_owned};
use crate::command::Arbiter;
use crate::domain::{
    Action, DeadlineKind, MissionDeadline, MissionSpec, Observation, OwnerPath, Pos,
};
use crate::fsm::MissionState;
use crate::protocol::decode;
use crate::rules::constants::{
    ANSWER_MARGIN_ROUNDS, DAY_ROUNDS, FIRST_ROUND, HTTP_DECISION_TIMEOUT_MS, NIGHT_ROUNDS,
};

const DAY_REQUEST: &str = include_str!("../../../tests/fixtures/day.json");
const NIGHT_REQUEST: &str = include_str!("../../../tests/fixtures/night.json");
const WORKER_ID: i64 = 10;
const PIONEER_ID: i64 = 11;
const WEAPON_ID: i64 = 20;
const SITE_X: i32 = 4;
const SITE_Y: i32 = 4;
const BUILDING_KIND: &str = "gatling";
const TASK_TIMEOUT_ROUNDS: i32 = 30;
const SECOND_ROUND: i32 = FIRST_ROUND + FIRST_ROUND;
const LAST_FIRST_DAY_ROUND: i32 = DAY_ROUNDS;
const EARLIER_DAY_DEADLINE_ROUND: i32 = LAST_FIRST_DAY_ROUND - FIRST_ROUND;
const SECOND_DAY_FIRST_ROUND: i32 = DAY_ROUNDS + NIGHT_ROUNDS + FIRST_ROUND;
const CHALLENGE_DEADLINE_ROUND: i32 = FIRST_ROUND + TASK_TIMEOUT_ROUNDS - ANSWER_MARGIN_ROUNDS;

#[test]
fn generated_deadlines_follow_day_task_and_defense_windows() {
    let day = day_observation();
    assert_deadline(
        &economy_spec(&day),
        LAST_FIRST_DAY_ROUND,
        DeadlineKind::ActionSubmission,
    );
    assert_deadline(
        &construction_spec(&day, site(), BUILDING_KIND),
        LAST_FIRST_DAY_ROUND,
        DeadlineKind::ActionSubmission,
    );
    assert_deadline(
        &challenge_spec(&day, Some(TASK_TIMEOUT_ROUNDS)),
        CHALLENGE_DEADLINE_ROUND,
        DeadlineKind::ActionSubmission,
    );
    assert_deadline(
        &defense_spec(&day, WEAPON_ID),
        SECOND_DAY_FIRST_ROUND,
        DeadlineKind::EffectObservation,
    );
    assert_deadline(
        &defense_spec(&night_observation(), WEAPON_ID),
        SECOND_DAY_FIRST_ROUND,
        DeadlineKind::EffectObservation,
    );
}

#[test]
fn explicit_deadline_change_replaces_the_assignment() {
    let observation = day_observation();
    let mut state = DecisionState::default();
    let first_spec = MissionSpec::economy().with_deadline(MissionDeadline::new(
        LAST_FIRST_DAY_ROUND,
        true,
        DeadlineKind::ActionSubmission,
    ));
    let second_spec = MissionSpec::economy().with_deadline(MissionDeadline::new(
        EARLIER_DAY_DEADLINE_ROUND,
        true,
        DeadlineKind::ActionSubmission,
    ));

    let first = accept_move(&observation, &mut state, first_spec);
    let second = accept_move(&observation, &mut state, second_spec);

    assert_ne!(first.mission(), second.mission());
    assert_eq!(mission_state(&state, first), MissionState::Cancelled);
    assert_eq!(mission_state(&state, second), MissionState::Executing);
}

#[test]
fn live_strategy_freezes_a_relative_challenge_deadline() {
    let mut observation = day_observation();
    let mut state = DecisionState::default();
    assign_once(&observation, &mut state);
    let first = *state.role_owners.get(&PIONEER_ID).expect("pioneer owner");
    assert_deadline(
        state
            .mission_view(first.mission().id)
            .expect("mission")
            .spec(),
        CHALLENGE_DEADLINE_ROUND,
        DeadlineKind::ActionSubmission,
    );

    observation.round_no = SECOND_ROUND;
    assign_once(&observation, &mut state);
    let second = *state.role_owners.get(&PIONEER_ID).expect("pioneer owner");

    assert_eq!(first.mission(), second.mission());
    assert_deadline(
        state
            .mission_view(second.mission().id)
            .expect("mission")
            .spec(),
        CHALLENGE_DEADLINE_ROUND,
        DeadlineKind::ActionSubmission,
    );
}

#[test]
fn expired_contract_is_rejected_before_owner_allocation() {
    let mut observation = day_observation();
    observation.round_no = SECOND_ROUND;
    let deadline = MissionDeadline::new(FIRST_ROUND, true, DeadlineKind::ActionSubmission);
    let spec = MissionSpec::economy().with_deadline(deadline);
    let mut state = DecisionState::default();
    let mut arbiter = Arbiter::new(&observation);

    let accepted = propose_owned(
        &observation,
        &mut state,
        &mut arbiter,
        WORKER_ID,
        Action::Move(site()),
        spec,
    )
    .expect("proposal");

    assert!(!accepted);
    assert!(state.role_owners.is_empty());
}

fn assert_deadline(spec: &MissionSpec, round: i32, kind: DeadlineKind) {
    let deadline = spec.deadline().expect("generated deadline");
    assert_eq!(deadline.at_round(), round);
    assert!(deadline.inclusive());
    assert_eq!(deadline.kind(), kind);
}

fn assign_once(observation: &Observation, state: &mut DecisionState) {
    let mut arbiter = Arbiter::new(observation);
    let deadline = Instant::now() + Duration::from_millis(HTTP_DECISION_TIMEOUT_MS);
    assign(observation, state, &mut arbiter, deadline).expect("mission assignment");
}

fn accept_move(
    observation: &Observation,
    state: &mut DecisionState,
    spec: MissionSpec,
) -> OwnerPath {
    let mut arbiter = Arbiter::new(observation);
    assert!(
        propose_owned(
            observation,
            state,
            &mut arbiter,
            WORKER_ID,
            Action::Move(site()),
            spec,
        )
        .expect("proposal")
    );
    *state.role_owners.get(&WORKER_ID).expect("worker owner")
}

fn mission_state(state: &DecisionState, owner: OwnerPath) -> MissionState {
    state
        .mission_view(owner.mission().id)
        .expect("mission")
        .state()
}

fn day_observation() -> Observation {
    decode(DAY_REQUEST.as_bytes())
        .expect("day fixture")
        .observation
}

fn night_observation() -> Observation {
    decode(NIGHT_REQUEST.as_bytes())
        .expect("night fixture")
        .observation
}

const fn site() -> Pos {
    Pos {
        x: SITE_X,
        y: SITE_Y,
    }
}
