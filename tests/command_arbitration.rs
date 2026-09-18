use technetium_battle_bot::command::Arbiter;
use technetium_battle_bot::domain::{Action, Pos};
use technetium_battle_bot::protocol::decode;
use technetium_battle_bot::rules::constants::{DEFAULT_LEVEL, NEIGHBOR_RANGE, NEXT_LEVEL};

const NIGHT_REQUEST: &str = include_str!("fixtures/night.json");
const DAY_REQUEST: &str = include_str!("fixtures/day.json");
const WEAPON_ID: i64 = 20;
const WORKER_ONE: i64 = 10;
const WORKER_TWO: i64 = 12;
const MOVE_TARGET_X: i32 = 3;
const MOVE_TARGET_Y: i32 = 3;
const ROBOT_X: i32 = 6;
const ROBOT_Y: i32 = 4;
const WEAPON_X: i32 = 4;

#[test]
fn attack_reserves_its_controller_action_slot() {
    let observation = decode(NIGHT_REQUEST.as_bytes())
        .expect("fixture")
        .observation;
    let mut arbiter = Arbiter::new(&observation, None);
    let target = Pos {
        x: ROBOT_X,
        y: ROBOT_Y,
    };
    let move_target = Pos {
        x: MOVE_TARGET_X,
        y: MOVE_TARGET_Y,
    };
    assert!(arbiter.propose(
        WEAPON_ID,
        Action::Attack {
            controller: WORKER_ONE,
            targets: vec![target]
        }
    ));
    assert!(!arbiter.propose(WORKER_ONE, Action::Move(move_target)));
}

#[test]
fn same_destination_is_reserved_once() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut arbiter = Arbiter::new(&observation, None);
    let target = Pos {
        x: MOVE_TARGET_X,
        y: MOVE_TARGET_Y,
    };
    assert!(arbiter.propose(WORKER_ONE, Action::Move(target)));
    assert!(!arbiter.propose(WORKER_TWO, Action::Move(target)));
}

#[test]
fn upgraded_gatling_rejects_opposite_target_directions() {
    let mut observation = decode(NIGHT_REQUEST.as_bytes())
        .expect("fixture")
        .observation;
    let weapon = observation
        .team_our
        .roles
        .iter_mut()
        .find(|role| role.id == WEAPON_ID)
        .expect("weapon");
    weapon.level = Some(DEFAULT_LEVEL + NEXT_LEVEL);
    let right = Pos {
        x: ROBOT_X,
        y: ROBOT_Y,
    };
    let left = Pos {
        x: WEAPON_X - NEIGHBOR_RANGE,
        y: ROBOT_Y,
    };
    let mut arbiter = Arbiter::new(&observation, None);
    assert!(!arbiter.propose(
        WEAPON_ID,
        Action::Attack {
            controller: WORKER_ONE,
            targets: vec![right, left]
        }
    ));
}
