use crate::domain::{Observation, Pos};
use crate::event::{EventRecord, WorldEvent};
use crate::fsm::{StrategyState, normal_strategy};
use crate::rules::constants::{
    BASE_RISK_ENTER, BASE_RISK_EXIT, COUNTER_INCREMENT, EMERGENCY_CLEAR_ROUNDS,
    ENDGAME_WINDOW_ROUNDS, MATCH_ROUNDS, MIN_POSITIVE_HP, POSITION_SETUP_ROUNDS,
    RETURN_MARGIN_ROUNDS, RISK_HORIZON_ROUNDS, ROBOT_ATTACK_RANGE, ROBOT_STEP_ESTIMATE,
    SCORE_SCALE, ZERO_COUNTER,
};
use crate::rules::geometry::{distance, station_distance};
use crate::rules::time::{next_night, phase};

pub fn advance(
    observation: &Observation,
    events: &[EventRecord],
    previous: StrategyState,
    clear_count: &mut u8,
) -> StrategyState {
    reset_clear_after_recovery(events, clear_count);
    let Some(current_phase) = phase(observation.round_no) else {
        return previous;
    };
    let risk = base_risk(observation);
    if risk >= BASE_RISK_ENTER {
        *clear_count = ZERO_COUNTER;
        return StrategyState::Emergency;
    }
    if previous == StrategyState::Emergency && risk >= BASE_RISK_EXIT {
        *clear_count = ZERO_COUNTER;
        return previous;
    }
    if previous == StrategyState::Emergency {
        *clear_count = clear_count.saturating_add(COUNTER_INCREMENT);
        if *clear_count < EMERGENCY_CLEAR_ROUNDS {
            return previous;
        }
    }
    normal_strategy(
        current_phase,
        return_due(observation),
        endgame(observation.round_no),
    )
}

fn reset_clear_after_recovery(events: &[EventRecord], clear_count: &mut u8) {
    if events
        .iter()
        .any(|record| matches!(&record.event, WorldEvent::WorldRecovered))
    {
        *clear_count = ZERO_COUNTER;
    }
}

pub fn return_due(observation: &Observation) -> bool {
    let Some(night) = next_night(observation.round_no) else {
        return false;
    };
    let max_return = observation
        .team_our
        .roles
        .iter()
        .filter(|role| is_active_unit(role))
        .filter_map(|unit| nearest_weapon_distance(observation, unit.pos))
        .max();
    max_return.is_some_and(|travel| {
        night - observation.round_no <= travel + POSITION_SETUP_ROUNDS + RETURN_MARGIN_ROUNDS
    })
}

fn nearest_weapon_distance(observation: &Observation, from: Pos) -> Option<i32> {
    observation
        .team_our
        .roles
        .iter()
        .filter(|role| {
            is_weapon(&role.role_type)
                && role
                    .health
                    .is_some_and(|hp| hp > crate::rules::constants::NO_HEALTH)
        })
        .map(|weapon| {
            distance(from, weapon.pos).saturating_sub(crate::rules::constants::NEIGHBOR_RANGE)
        })
        .min()
}

pub fn is_weapon(kind: &str) -> bool {
    matches!(kind, "gatling" | "railgun" | "rocket")
}

fn is_active_unit(role: &crate::domain::Role) -> bool {
    matches!(role.role_type.as_str(), "worker" | "pioneer")
        && role
            .health
            .is_some_and(|hp| hp > crate::rules::constants::NO_HEALTH)
}

fn endgame(round: i32) -> bool {
    MATCH_ROUNDS - round + crate::rules::constants::FIRST_ROUND <= ENDGAME_WINDOW_ROUNDS
}

pub fn base_risk(observation: &Observation) -> i32 {
    let Some(base) = observation.team_our.roles.iter().find(|role| {
        role.role_type == "station"
            && role
                .health
                .is_some_and(|hp| hp > crate::rules::constants::NO_HEALTH)
    }) else {
        return crate::rules::constants::ZERO_SCORE;
    };
    let damage: i32 = observation
        .robot
        .as_ref()
        .into_iter()
        .flat_map(|robots| &robots.roles)
        .filter(|robot| in_horizon(station_distance(base.pos, robot.pos)))
        .map(|robot| robot_attack(&robot.role_type))
        .sum();
    let numerator = i64::from(SCORE_SCALE) * i64::from(RISK_HORIZON_ROUNDS) * i64::from(damage);
    let hp = i64::from(base.health.unwrap_or(MIN_POSITIVE_HP).max(MIN_POSITIVE_HP));
    i32::try_from(((numerator + hp - i64::from(MIN_POSITIVE_HP)) / hp).min(i64::from(SCORE_SCALE)))
        .unwrap_or(SCORE_SCALE)
}

fn in_horizon(distance_to_base: i32) -> bool {
    distance_to_base <= ROBOT_ATTACK_RANGE + RISK_HORIZON_ROUNDS * ROBOT_STEP_ESTIMATE
}

fn robot_attack(kind: &str) -> i32 {
    use crate::rules::constants::{
        BOSS_ROBOT_ATTACK, LARGE_ROBOT_ATTACK, MIDDLE_ROBOT_ATTACK, SMALL_ROBOT_ATTACK,
    };
    match kind {
        "smallRobot" => SMALL_ROBOT_ATTACK,
        "middleRobot" => MIDDLE_ROBOT_ATTACK,
        "largeRobot" => LARGE_ROBOT_ATTACK,
        "bossRobot" => BOSS_ROBOT_ATTACK,
        _ => crate::rules::constants::ZERO_SCORE,
    }
}
