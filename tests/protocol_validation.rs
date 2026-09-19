use technetium_battle_bot::protocol::decode;
use technetium_battle_bot::rules::constants::{MIN_COORDINATE, SINGLE_ITEM};

const DAY_REQUEST: &str = include_str!("fixtures/day.json");
const FIRST_TASK_INDEX: usize = 0;
const ROBOT_ID: i64 = 900;
const ROBOT_HEALTH: i32 = 1;

#[test]
fn rejects_robot_outside_map() {
    let mut request: serde_json::Value = serde_json::from_str(DAY_REQUEST).expect("fixture");
    let robot = serde_json::json!({
        "id": ROBOT_ID,
        "pos": {"x": request["mapInfo"]["width"], "y": MIN_COORDINATE},
        "roleType": "smallRobot",
        "health": ROBOT_HEALTH
    });
    request["robot"]["roles"] = serde_json::Value::Array(vec![robot]);
    assert!(decode(&serde_json::to_vec(&request).expect("JSON")).is_err());
}

#[test]
fn rejects_task_position_outside_map() {
    let mut request: serde_json::Value = serde_json::from_str(DAY_REQUEST).expect("fixture");
    request["teamOur"]["playerTasks"][FIRST_TASK_INDEX]["taskPosition"]["y"] =
        request["mapInfo"]["height"].clone();
    assert!(decode(&serde_json::to_vec(&request).expect("JSON")).is_err());
}

#[test]
fn rejects_station_whose_footprint_leaves_map() {
    let mut request: serde_json::Value = serde_json::from_str(DAY_REQUEST).expect("fixture");
    let station_index = request["teamOur"]["roles"].as_array().expect("roles").len() - SINGLE_ITEM;
    request["teamOur"]["roles"][station_index]["pos"]["y"] =
        serde_json::Value::from(MIN_COORDINATE);
    assert!(decode(&serde_json::to_vec(&request).expect("JSON")).is_err());
}

#[test]
fn accepts_optional_enemy_and_robot_collections_when_absent() {
    let mut request: serde_json::Value = serde_json::from_str(DAY_REQUEST).expect("fixture");
    let object = request.as_object_mut().expect("request");
    object.remove("teamEnemy");
    object.remove("robot");
    let decoded = decode(&serde_json::to_vec(&request).expect("JSON")).expect("observation");
    assert!(decoded.observation.team_enemy.is_none());
    assert!(decoded.observation.robot.is_none());
}
