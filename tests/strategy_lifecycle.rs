use technetium_battle_bot::ai::strategy;
use technetium_battle_bot::event::{EventRecord, WorldEvent};
use technetium_battle_bot::fsm::StrategyState;
use technetium_battle_bot::protocol::decode;
use technetium_battle_bot::rules::constants::{
    COUNTER_INCREMENT, DAY_ROUNDS, EMERGENCY_CLEAR_ROUNDS, FIRST_ROUND, NIGHT_ROUNDS, ZERO_COUNTER,
    ZERO_VERSION,
};

const NIGHT_REQUEST: &str = include_str!("fixtures/night.json");
const PREPARE_ROUND: i32 = DAY_ROUNDS - FIRST_ROUND;
const FIRST_NIGHT_ROUND: i32 = DAY_ROUNDS + FIRST_ROUND;
const SECOND_DAY_ROUND: i32 = DAY_ROUNDS + NIGHT_ROUNDS + FIRST_ROUND;

#[test]
fn continuous_phase_frames_drive_strategy_cycle() {
    let mut observation = decode(NIGHT_REQUEST.as_bytes())
        .expect("fixture")
        .observation;
    let mut clear = ZERO_COUNTER;

    observation.round_no = PREPARE_ROUND;
    let prepare = strategy::advance(&observation, &[], StrategyState::DayDevelop, &mut clear);
    assert_eq!(prepare, StrategyState::PrepareNight);

    observation.round_no = FIRST_NIGHT_ROUND;
    let night = strategy::advance(&observation, &[], prepare, &mut clear);
    assert_eq!(night, StrategyState::NightDefend);

    observation.round_no = SECOND_DAY_ROUND;
    let day = strategy::advance(&observation, &[], night, &mut clear);
    assert_eq!(day, StrategyState::DayDevelop);
}

#[test]
fn emergency_exit_requires_configured_clear_observations() {
    let mut observation = decode(NIGHT_REQUEST.as_bytes())
        .expect("fixture")
        .observation;
    observation.robot = None;
    let mut clear = ZERO_COUNTER;
    let mut state = StrategyState::Emergency;
    for _ in ZERO_COUNTER..EMERGENCY_CLEAR_ROUNDS - COUNTER_INCREMENT {
        state = strategy::advance(&observation, &[], state, &mut clear);
        assert_eq!(state, StrategyState::Emergency);
        observation.round_no += FIRST_ROUND;
    }
    state = strategy::advance(&observation, &[], state, &mut clear);
    assert_ne!(state, StrategyState::Emergency);
}

#[test]
fn world_recovery_restarts_emergency_clear_evidence() {
    let mut observation = decode(NIGHT_REQUEST.as_bytes())
        .expect("fixture")
        .observation;
    observation.robot = None;
    let mut clear = EMERGENCY_CLEAR_ROUNDS - COUNTER_INCREMENT;
    let recovery = EventRecord {
        id: ZERO_VERSION,
        observed_round: observation.round_no,
        event: WorldEvent::WorldRecovered,
    };

    let state = strategy::advance(
        &observation,
        &[recovery],
        StrategyState::Emergency,
        &mut clear,
    );

    assert_eq!(state, StrategyState::Emergency);
    assert_eq!(clear, COUNTER_INCREMENT);
}
