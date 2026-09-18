use technetium_battle_bot::domain::Pos;
use technetium_battle_bot::protocol::decode;
use technetium_battle_bot::rules::constants::{
    DAY_ROUNDS, FIRST_ROUND, MATCH_ROUNDS, NEIGHBOR_RANGE, NIGHT_ROUNDS,
};
use technetium_battle_bot::rules::time::{Phase, next_night, phase};
use technetium_battle_bot::world::WorldView;

const DAY_REQUEST: &str = include_str!("fixtures/day.json");
const START_X: i32 = 3;
const START_Y: i32 = 4;
const WORKER_ID: i64 = 10;
const MINE_X: i32 = 5;
const MINE_Y: i32 = 5;
const STATION_X: i32 = 2;
const STATION_Y: i32 = 6;
const STATION_EDGE_X: i32 = 3;
const STATION_EDGE_Y: i32 = 5;

#[test]
fn phase_boundaries_follow_one_based_rounds() {
    assert_eq!(phase(FIRST_ROUND), Some(Phase::Day));
    assert_eq!(phase(DAY_ROUNDS), Some(Phase::Day));
    assert_eq!(phase(DAY_ROUNDS + FIRST_ROUND), Some(Phase::Night));
    assert_eq!(phase(DAY_ROUNDS + NIGHT_ROUNDS), Some(Phase::Night));
    assert_eq!(
        phase(DAY_ROUNDS + NIGHT_ROUNDS + FIRST_ROUND),
        Some(Phase::Day)
    );
    assert_eq!(phase(MATCH_ROUNDS), Some(Phase::Night));
    assert_eq!(next_night(FIRST_ROUND), Some(DAY_ROUNDS + FIRST_ROUND));
}

#[test]
fn path_finds_legal_adjacent_mine_stand() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let view = WorldView::new(&observation);
    let start = Pos {
        x: START_X,
        y: START_Y,
    };
    let target = Pos {
        x: MINE_X,
        y: MINE_Y,
    };
    let stands = view.interact_positions(target, WORKER_ID);
    let path = technetium_battle_bot::ai::tactics::next_step(&view, WORKER_ID, start, &stands)
        .expect("reachable mine stand");
    assert!(view.can_enter(path.next, WORKER_ID));
    assert_eq!(
        technetium_battle_bot::rules::geometry::distance(start, path.next),
        NEIGHBOR_RANGE
    );
}

#[test]
fn station_distance_uses_all_occupied_cells() {
    let station = Pos {
        x: STATION_X,
        y: STATION_Y,
    };
    let lower_right = Pos {
        x: STATION_EDGE_X,
        y: STATION_EDGE_Y,
    };
    assert_eq!(
        technetium_battle_bot::rules::geometry::station_distance(station, lower_right),
        technetium_battle_bot::rules::constants::ZERO_ROUNDS
    );
}
