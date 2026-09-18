use technetium_battle_bot::domain::Pos;
use technetium_battle_bot::protocol::decode;
use technetium_battle_bot::runtime::Session;
use technetium_battle_bot::world::WorldView;

const DAY_REQUEST: &str = include_str!("fixtures/day.json");
const PIONEER_INDEX: usize = 1;
const TASK_ZONE_INDEX: usize = 2;
const PIONEER_ID: i64 = 11;
const SECOND_CELL_X: i32 = 6;
const SECOND_CELL_Y: i32 = 3;
const PIONEER_X: i32 = 5;
const PIONEER_Y: i32 = 4;

#[test]
fn pioneer_can_accept_two_cell_task_from_second_cell() {
    let mut request: serde_json::Value = serde_json::from_str(DAY_REQUEST).expect("fixture");
    let task_kind = "challengerTaskPoint2";
    request["mapInfo"]["zones"][TASK_ZONE_INDEX]["neutralType"] = task_kind.into();
    request["mapInfo"]["zones"]
        .as_array_mut()
        .expect("zones")
        .push(serde_json::json!({
            "pos": {"x": SECOND_CELL_X, "y": SECOND_CELL_Y},
            "neutralType": task_kind
        }));
    request["teamOur"]["roles"][PIONEER_INDEX]["pos"] =
        serde_json::json!({"x": PIONEER_X, "y": PIONEER_Y});
    let body = serde_json::to_vec(&request).expect("JSON");
    let observation = decode(&body).expect("observation").observation;
    let view = WorldView::new(&observation);
    let task = observation.team_our.player_tasks.first().expect("task");
    assert!(view.adjacent_task(
        Pos {
            x: PIONEER_X,
            y: PIONEER_Y
        },
        task.task_position
    ));
    assert!(view.task_cells(task.task_position).contains(&Pos {
        x: SECOND_CELL_X,
        y: SECOND_CELL_Y
    }));
    let mut session = Session::default();
    let response: serde_json::Value =
        serde_json::from_slice(&session.handle(&body)).expect("reply");
    assert_eq!(
        response["roleCommandMap"][PIONEER_ID.to_string()]["action"],
        "acceptTask"
    );
}
