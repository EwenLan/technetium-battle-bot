use technetium_battle_bot::event::{EventLog, WorldEvent, WorldIssue};
use technetium_battle_bot::fsm::WorldState;
use technetium_battle_bot::protocol::decode;
use technetium_battle_bot::rules::constants::{
    EMPTY_COUNT, FIRST_ROUND, MAX_EVENT_LOG_RECORDS, MIN_ACTION_ROUNDS, SINGLE_ITEM,
    VERSION_INCREMENT,
};
use technetium_battle_bot::runtime::Session;
use technetium_battle_bot::world::World;

const DAY_REQUEST: &str = include_str!("fixtures/day.json");
const MAP_GROWTH: i32 = 1;

#[test]
fn world_degrades_without_replacing_last_valid_snapshot_and_recovers() {
    let first = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut world = World::default();
    assert!(world.apply(first.clone()));
    assert_eq!(world.state, WorldState::Ready);
    assert!(world.events.contains(&WorldEvent::WorldReady));
    let version = world.version;

    let mut invalid = first.clone();
    invalid.round_no += MIN_ACTION_ROUNDS;
    invalid.map_info.width += MAP_GROWTH;
    assert!(!world.apply(invalid));
    assert_eq!(world.state, WorldState::Degraded);
    assert_eq!(world.version, version);
    assert_eq!(
        world.observation.as_ref().expect("snapshot").round_no,
        first.round_no
    );
    assert_eq!(
        world.events,
        vec![WorldEvent::WorldDegraded(WorldIssue::MapDimensionsChanged)]
    );

    let mut recovered = first;
    recovered.round_no += MIN_ACTION_ROUNDS;
    assert!(world.apply(recovered));
    assert_eq!(world.state, WorldState::Ready);
    assert!(world.events.contains(&WorldEvent::WorldRecovered));
    assert_eq!(
        world.version,
        version + technetium_battle_bot::rules::constants::VERSION_INCREMENT
    );
}

#[test]
fn lifecycle_events_are_kept_in_stable_cursor_order() {
    let mut observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut world = World::default();
    assert!(world.apply(observation.clone()));
    observation.round_no += MIN_ACTION_ROUNDS;
    observation.map_info.width += MAP_GROWTH;
    assert!(!world.apply(observation));

    let records: Vec<_> = world.event_log.records().collect();
    assert!(records.len() > SINGLE_ITEM);
    assert!(records.windows(2).all(|pair| pair[0].id < pair[1].id));
    let first_id = records.first().expect("first event").id;
    assert_eq!(
        world.event_log.after(Some(first_id)).count(),
        records.len() - SINGLE_ITEM
    );
}

#[test]
fn degraded_session_returns_empty_until_corrected_new_frame() {
    let mut session = Session::new();
    let first = session.handle(DAY_REQUEST.as_bytes());
    let first_value: serde_json::Value = serde_json::from_slice(&first).expect("reply");
    assert!(
        first_value["roleCommandMap"]
            .as_object()
            .is_some_and(|map| !map.is_empty())
    );

    let mut invalid: serde_json::Value = serde_json::from_str(DAY_REQUEST).expect("fixture");
    invalid["roundNo"] = serde_json::Value::from(MIN_ACTION_ROUNDS + MIN_ACTION_ROUNDS);
    invalid["mapInfo"]["width"] = serde_json::Value::from(
        invalid["mapInfo"]["width"].as_i64().expect("width") + i64::from(MAP_GROWTH),
    );
    let fallback = session.handle(&serde_json::to_vec(&invalid).expect("JSON"));
    assert_empty(&fallback);
    assert_eq!(session.world_state(), WorldState::Degraded);
    let event_count = session.event_count();
    assert_empty(&session.handle(&serde_json::to_vec(&invalid).expect("JSON")));
    assert_eq!(session.event_count(), event_count);
    assert_empty(&session.handle(DAY_REQUEST.as_bytes()));

    let mut corrected: serde_json::Value = serde_json::from_str(DAY_REQUEST).expect("fixture");
    corrected["roundNo"] = serde_json::Value::from(MIN_ACTION_ROUNDS + MIN_ACTION_ROUNDS);
    let reply = session.handle(&serde_json::to_vec(&corrected).expect("JSON"));
    assert_eq!(session.world_state(), WorldState::Ready);
    let value: serde_json::Value = serde_json::from_slice(&reply).expect("reply");
    assert!(value["roleCommandMap"].as_object().is_some());
}

#[test]
fn event_log_discards_oldest_records_at_capacity() {
    let mut log = EventLog::default();
    for sequence in EMPTY_COUNT..MAX_EVENT_LOG_RECORDS + SINGLE_ITEM {
        let round = i32::try_from(sequence).expect("bounded sequence") + FIRST_ROUND;
        log.append(round, &[WorldEvent::RoundStarted(round)]);
    }
    let records: Vec<_> = log.records().collect();
    assert_eq!(records.len(), MAX_EVENT_LOG_RECORDS);
    assert_eq!(
        records.first().expect("oldest retained").id,
        VERSION_INCREMENT
    );
}

fn assert_empty(reply: &[u8]) {
    let value: serde_json::Value = serde_json::from_slice(reply).expect("reply");
    assert_eq!(
        value["roleCommandMap"].as_object().map(|map| map.len()),
        Some(EMPTY_COUNT)
    );
}
