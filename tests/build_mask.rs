use std::path::Path;

use technetium_battle_bot::rules::build::BuildMask;
use technetium_battle_bot::rules::constants::WEAPON_BUILD_GOLD;
use technetium_battle_bot::runtime::Session;

const DAY_REQUEST: &str = include_str!("fixtures/day.json");
const MASK_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/build_mask.json"
);
const WORKER_ONE: &str = "10";
const WORKER_TWO: &str = "12";

#[test]
fn configured_mask_allows_two_affordable_distinct_builds() {
    let mask = BuildMask::from_path(Path::new(MASK_PATH)).expect("mask fixture");
    let mut session = Session::new(Some(mask));
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
    let mask = BuildMask::from_path(Path::new(MASK_PATH)).expect("mask fixture");
    let mut request: serde_json::Value = serde_json::from_str(DAY_REQUEST).expect("fixture");
    request["teamOur"]["goldNum"] = serde_json::Value::from(WEAPON_BUILD_GOLD);
    let mut session = Session::new(Some(mask));
    let response = session.handle(&serde_json::to_vec(&request).expect("request"));
    let value: serde_json::Value = serde_json::from_slice(&response).expect("reply");
    let actions: Vec<&str> = value["roleCommandMap"]
        .as_object()
        .expect("commands")
        .values()
        .filter_map(|command| command["action"].as_str())
        .collect();
    let builds = actions.iter().filter(|action| **action == "build").count();
    assert_eq!(builds, technetium_battle_bot::rules::constants::SINGLE_ITEM);
}
