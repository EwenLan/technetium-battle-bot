use technetium_battle_bot::command::Arbiter;
use technetium_battle_bot::domain::{Action, ActiveOwners, OwnerAllocator, Pos};
use technetium_battle_bot::protocol::decode;
use technetium_battle_bot::rules::build::{BuildArea, area, wall_sites, weapon_sites};
use technetium_battle_bot::rules::constants::{MIN_COORDINATE, SINGLE_ITEM, WEAPON_BUILD_GOLD};
use technetium_battle_bot::runtime::Session;

const DAY_REQUEST: &str = include_str!("fixtures/day.json");
const STATION_X: i32 = 10;
const STATION_Y: i32 = 10;
const MAP_WIDTH: i32 = 41;
const MAP_HEIGHT: i32 = 32;
const WEAPON_RING_CELLS: usize = 12;
const WALL_RING_CELLS: usize = 20;
const WORKER_ONE: &str = "10";
const WORKER_TWO: &str = "12";
const WORKER_INDEX: usize = 0;
const WALL_X: i32 = 0;
const WALL_Y: i32 = 6;
const WALL_STAND_X: i32 = 0;
const WALL_STAND_Y: i32 = 5;
const EDGE_STATION_X: i32 = 0;
const EDGE_STATION_Y: i32 = 1;
const EDGE_MAP_SIZE: i32 = 4;
const CLIPPED_WEAPON_CELLS: usize = 5;
const CLIPPED_WALL_CELLS: usize = 7;

mod support;

#[test]
fn build_rings_follow_two_four_six_geometry() {
    let station = Pos {
        x: STATION_X,
        y: STATION_Y,
    };
    let weapons = weapon_sites(station, MAP_WIDTH, MAP_HEIGHT);
    let walls = wall_sites(station, MAP_WIDTH, MAP_HEIGHT);
    assert_eq!(weapons.len(), WEAPON_RING_CELLS);
    assert_eq!(walls.len(), WALL_RING_CELLS);
    assert!(
        weapons
            .iter()
            .all(|pos| area(station, *pos) == BuildArea::Weapon)
    );
    assert!(
        walls
            .iter()
            .all(|pos| area(station, *pos) == BuildArea::Wall)
    );
}

#[test]
fn build_rings_are_clipped_at_map_boundary() {
    let station = Pos {
        x: EDGE_STATION_X,
        y: EDGE_STATION_Y,
    };
    let weapons = weapon_sites(station, EDGE_MAP_SIZE, EDGE_MAP_SIZE);
    let walls = wall_sites(station, EDGE_MAP_SIZE, EDGE_MAP_SIZE);
    assert_eq!(weapons.len(), CLIPPED_WEAPON_CELLS);
    assert_eq!(walls.len(), CLIPPED_WALL_CELLS);
    assert!(weapons.iter().chain(&walls).all(|pos| {
        pos.x >= MIN_COORDINATE
            && pos.y >= MIN_COORDINATE
            && pos.x < EDGE_MAP_SIZE
            && pos.y < EDGE_MAP_SIZE
    }));
}

#[test]
fn observed_base_enables_two_affordable_distinct_weapon_builds() {
    let mut session = Session::new();
    let response = session.handle(DAY_REQUEST.as_bytes());
    let value: serde_json::Value = serde_json::from_slice(&response).expect("reply");
    let commands = &value["roleCommandMap"];
    assert_eq!(commands[WORKER_ONE]["action"], "build");
    assert_eq!(commands[WORKER_TWO]["action"], "build");
    assert_ne!(commands[WORKER_ONE]["name"], commands[WORKER_TWO]["name"]);
    assert_ne!(
        commands[WORKER_ONE]["targetPos"],
        commands[WORKER_TWO]["targetPos"]
    );
}

#[test]
fn concurrent_builds_do_not_exceed_observed_gold() {
    let mut request: serde_json::Value = serde_json::from_str(DAY_REQUEST).expect("fixture");
    request["teamOur"]["goldNum"] = serde_json::Value::from(WEAPON_BUILD_GOLD);
    let mut session = Session::new();
    let response = session.handle(&serde_json::to_vec(&request).expect("request"));
    let value: serde_json::Value = serde_json::from_slice(&response).expect("reply");
    let builds = value["roleCommandMap"]
        .as_object()
        .expect("commands")
        .values()
        .filter(|command| command["action"] == "build")
        .count();
    assert_eq!(builds, SINGLE_ITEM);
}

#[test]
fn wall_build_requires_wall_ring_and_worker_stone() {
    let mut request: serde_json::Value = serde_json::from_str(DAY_REQUEST).expect("fixture");
    request["teamOur"]["roles"][WORKER_INDEX]["pos"] =
        serde_json::json!({"x": WALL_STAND_X, "y": WALL_STAND_Y});
    request["teamOur"]["roles"][WORKER_INDEX]["backpack"] = serde_json::json!(["stone"]);
    let observation = decode(&serde_json::to_vec(&request).expect("JSON"))
        .expect("observation")
        .observation;
    let mut arbiter = Arbiter::new(&observation);
    let mut owners = ActiveOwners::default();
    let mut allocator = OwnerAllocator::default();
    let actor = observation.team_our.roles[WORKER_INDEX].id;
    let weapon = support::owned_action(
        &mut owners,
        &mut allocator,
        actor,
        Action::Build {
            name: "gatling".into(),
            pos: Pos {
                x: WALL_X,
                y: WALL_Y,
            },
        },
    );
    assert!(!arbiter.propose(&weapon, &owners));
    let wall = support::owned_action(
        &mut owners,
        &mut allocator,
        actor,
        Action::Build {
            name: "wall".into(),
            pos: Pos {
                x: WALL_X,
                y: WALL_Y,
            },
        },
    );
    assert!(arbiter.propose(&wall, &owners));
}
