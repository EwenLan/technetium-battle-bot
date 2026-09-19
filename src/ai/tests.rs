use super::{DecisionState, propose_owned};
use crate::command::Arbiter;
use crate::domain::Action;
use crate::protocol::decode;

const DAY_REQUEST: &str = include_str!("../../tests/fixtures/day.json");
const NIGHT_REQUEST: &str = include_str!("../../tests/fixtures/night.json");
const WEAPON_ID: i64 = 20;
const WORKER_ID: i64 = 10;

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
        )
        .expect("proposal")
    );

    let owner = state.role_owners.get(&WORKER_ID).expect("controller owner");
    assert!(state.active_owners.is_current(owner));
    assert!(!state.role_owners.contains_key(&WEAPON_ID));
}
