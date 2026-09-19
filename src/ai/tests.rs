use super::{AssignmentKind, DecisionState, propose_owned};
use crate::command::Arbiter;
use crate::domain::{Action, OwnerPath, Pos};
use crate::protocol::decode;

const DAY_REQUEST: &str = include_str!("../../tests/fixtures/day.json");
const NIGHT_REQUEST: &str = include_str!("../../tests/fixtures/night.json");
const WEAPON_ID: i64 = 20;
const WORKER_ID: i64 = 10;
const MOVE_X: i32 = 4;
const MOVE_Y: i32 = 4;

#[test]
fn rejected_owned_proposal_preserves_existing_work() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let worker = observation
        .team_our
        .roles
        .iter()
        .find(|role| role.id == WORKER_ID)
        .expect("worker");
    let mut arbiter = Arbiter::new(&observation);
    let mut state = DecisionState::default();
    let previous = state.start_work(WORKER_ID).expect("existing work");

    let accepted = propose_owned(
        &mut state,
        &mut arbiter,
        WORKER_ID,
        Action::Move(worker.pos),
        AssignmentKind::Economy,
    )
    .expect("proposal");

    assert!(!accepted);
    assert!(state.active_owners.is_current(&previous));
    assert_eq!(state.role_owners.get(&WORKER_ID), Some(&previous));
    assert!(
        arbiter
            .finish(&state.active_owners)
            .response
            .role_command_map
            .is_empty()
    );
}

#[test]
fn attack_owner_is_committed_to_the_controller() {
    let observation = decode(NIGHT_REQUEST.as_bytes())
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

    assert!(
        propose_owned(
            &mut state,
            &mut arbiter,
            WEAPON_ID,
            Action::Attack {
                controller: WORKER_ID,
                targets: vec![target],
            },
            AssignmentKind::Defense,
        )
        .expect("proposal")
    );

    let owner = state.role_owners.get(&WORKER_ID).expect("controller owner");
    assert!(state.active_owners.is_current(owner));
    assert!(!state.role_owners.contains_key(&WEAPON_ID));
}

#[test]
fn successive_steps_reuse_assignment_and_replace_intent() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut state = DecisionState::default();

    let first = accept_move(&observation, &mut state, AssignmentKind::Economy);
    let second = accept_move(&observation, &mut state, AssignmentKind::Economy);

    assert_eq!(first.mission(), second.mission());
    assert_eq!(first.plan(), second.plan());
    assert_ne!(first.intent(), second.intent());
    assert!(!state.active_owners.is_current(&first));
    assert!(state.active_owners.is_current(&second));
}

#[test]
fn changing_work_kind_replaces_the_assignment() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut state = DecisionState::default();

    let economy = accept_move(&observation, &mut state, AssignmentKind::Economy);
    let construction = accept_move(&observation, &mut state, AssignmentKind::Construction);

    assert_ne!(economy.mission(), construction.mission());
    assert_ne!(economy.plan(), construction.plan());
    assert!(!state.active_owners.is_current(&economy));
    assert!(state.active_owners.is_current(&construction));
}

#[test]
fn discarded_draft_does_not_advance_persistent_owner_ids() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let state = DecisionState::default();
    let mut discarded = state.clone();
    let discarded_owner = accept_move(&observation, &mut discarded, AssignmentKind::Economy);
    let mut committed = state;

    let committed_owner = accept_move(&observation, &mut committed, AssignmentKind::Economy);

    assert_eq!(discarded_owner, committed_owner);
}

fn accept_move(
    observation: &crate::domain::Observation,
    state: &mut DecisionState,
    kind: AssignmentKind,
) -> OwnerPath {
    let mut arbiter = Arbiter::new(observation);
    let action = Action::Move(Pos {
        x: MOVE_X,
        y: MOVE_Y,
    });
    assert!(propose_owned(state, &mut arbiter, WORKER_ID, action, kind).expect("proposal"));
    let arbitration = arbiter.finish(&state.active_owners);
    assert!(
        arbitration
            .response
            .role_command_map
            .contains_key(&WORKER_ID.to_string())
    );
    *state.role_owners.get(&WORKER_ID).expect("role owner")
}
