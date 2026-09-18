use technetium_battle_bot::ai::cognition::{ChallengeMemory, answer_action, prepare_prompt};
use technetium_battle_bot::domain::Observation;
use technetium_battle_bot::event::WorldEvent;
use technetium_battle_bot::protocol::decode;
use technetium_battle_bot::rules::constants::{EMPTY_COUNT, NO_HEALTH, SINGLE_ITEM};
use technetium_battle_bot::runtime::Session;
use technetium_battle_bot::world::{EnemyStatus, World};

const DAY_REQUEST: &str = include_str!("fixtures/day.json");
const NIGHT_REQUEST: &str = include_str!("fixtures/night.json");
const DUPLICATE_REQUEST: &str = include_str!("fixtures/duplicate.json");
const WORKER_INDEX: usize = 0;
const CONFLICT_GOLD: i32 = 74;
const WEAPON_KEY: &str = "20";
const CONTROLLER_KEY: &str = "10";
const ENEMY_ID: i64 = 31;
const TASK_TEXT: &str = "Return the code";
const ANSWER_TEXT: &str = "code";

#[test]
fn decodes_optional_fields_and_rejects_duplicate_keys() {
    let request = decode(DAY_REQUEST.as_bytes()).expect("valid fixture");
    assert_eq!(
        request.observation.team_our.roles.len(),
        technetium_battle_bot::rules::constants::MAX_WEAPONS + SINGLE_ITEM
    );
    assert!(decode(DUPLICATE_REQUEST.as_bytes()).is_err());
}

#[test]
fn identical_round_reuses_bytes_without_advancing_decision() {
    let mut session = Session::default();
    let first = session.handle(DAY_REQUEST.as_bytes());
    assert_eq!(first, session.handle(DAY_REQUEST.as_bytes()));
    let value: serde_json::Value = serde_json::from_slice(&first).expect("response JSON");
    assert!(value["roleCommandMap"].as_object().is_some());
    assert!(value["prompt"].is_string());
    assert!(value["executeCmd"].is_string());
}

#[test]
fn conflicting_same_round_returns_empty_response() {
    let mut session = Session::default();
    session.handle(DAY_REQUEST.as_bytes());
    let mut request: serde_json::Value = serde_json::from_str(DAY_REQUEST).expect("fixture JSON");
    request["teamOur"]["goldNum"] = serde_json::Value::from(CONFLICT_GOLD);
    let body = serde_json::to_vec(&request).expect("changed fixture");
    let reply: serde_json::Value = serde_json::from_slice(&session.handle(&body)).expect("reply");
    assert_eq!(
        reply["roleCommandMap"].as_object().map(|map| map.len()),
        Some(EMPTY_COUNT)
    );
}

#[test]
fn actual_death_is_recorded_before_next_decision() {
    let initial: Observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let mut current = initial.clone();
    current.round_no += technetium_battle_bot::rules::constants::MIN_ACTION_ROUNDS;
    current.team_our.roles[WORKER_INDEX].health = Some(NO_HEALTH);
    let mut world = World::default();
    world.apply(initial);
    world.apply(current);
    assert!(world.events.contains(&WorldEvent::UnitDied(
        world.observation.as_ref().expect("world").team_our.roles[WORKER_INDEX].id
    )));
}

#[test]
fn nighttime_attack_uses_weapon_key_and_controller_string() {
    let mut session = Session::default();
    let reply = session.handle(NIGHT_REQUEST.as_bytes());
    let value: serde_json::Value = serde_json::from_slice(&reply).expect("reply");
    let attack = &value["roleCommandMap"][WEAPON_KEY];
    assert_eq!(attack["action"], "attack");
    assert_eq!(attack["controllerId"], CONTROLLER_KEY);
    assert!(value["roleCommandMap"].get(CONTROLLER_KEY).is_none());
}

#[test]
fn hidden_enemy_becomes_unobserved_without_death_claim() {
    let mut first: serde_json::Value = serde_json::from_str(NIGHT_REQUEST).expect("fixture");
    let mut enemy = first["teamOur"]["roles"][WORKER_INDEX].clone();
    enemy["id"] = serde_json::Value::from(ENEMY_ID);
    first["teamEnemy"]["roles"] = serde_json::Value::Array(vec![enemy]);
    let mut second = first.clone();
    second["roundNo"] = serde_json::Value::from(
        first["roundNo"].as_i64().expect("round")
            + i64::from(technetium_battle_bot::rules::constants::MIN_ACTION_ROUNDS),
    );
    second["teamEnemy"]["roles"] = serde_json::Value::Array(Vec::new());
    let initial = decode(&serde_json::to_vec(&first).expect("JSON"))
        .expect("observation")
        .observation;
    let current = decode(&serde_json::to_vec(&second).expect("JSON"))
        .expect("observation")
        .observation;
    let mut world = World::default();
    world.apply(initial);
    world.apply(current);
    assert_eq!(
        world
            .memory
            .enemies
            .get(&ENEMY_ID)
            .expect("remembered")
            .status,
        EnemyStatus::Unobserved
    );
    assert!(
        world
            .events
            .contains(&WorldEvent::EnemyUnobserved(ENEMY_ID))
    );
    assert!(!world.events.contains(&WorldEvent::UnitDied(ENEMY_ID)));
}

#[test]
fn delayed_model_result_cannot_answer_expired_prompt() {
    let mut observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    observation.phase_task = TASK_TEXT.into();
    let mut memory = ChallengeMemory::default();
    assert!(!prepare_prompt(&observation, &mut memory).is_empty());
    observation.round_no += technetium_battle_bot::rules::constants::MIN_ACTION_ROUNDS;
    observation.llm_resp = ANSWER_TEXT.into();
    assert!(answer_action(&observation, &memory).is_some());
    observation.round_no += technetium_battle_bot::rules::constants::MIN_ACTION_ROUNDS;
    assert!(answer_action(&observation, &memory).is_none());
}
