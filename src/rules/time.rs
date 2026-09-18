use super::constants::{DAY_ROUNDS, FIRST_ROUND, MATCH_ROUNDS, NIGHT_ROUNDS, ROUND_OFFSET};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Day,
    Night,
}

pub fn phase(round: i32) -> Option<Phase> {
    if !(FIRST_ROUND..=MATCH_ROUNDS).contains(&round) {
        return None;
    }
    let day_length = DAY_ROUNDS.checked_add(NIGHT_ROUNDS)?;
    let slot = (round - ROUND_OFFSET) % day_length;
    Some(if slot < DAY_ROUNDS {
        Phase::Day
    } else {
        Phase::Night
    })
}

pub fn next_night(round: i32) -> Option<i32> {
    let day_length = DAY_ROUNDS.checked_add(NIGHT_ROUNDS)?;
    let slot = (round - ROUND_OFFSET).rem_euclid(day_length);
    let delta = if slot < DAY_ROUNDS {
        DAY_ROUNDS - slot
    } else {
        day_length - slot + DAY_ROUNDS
    };
    round.checked_add(delta)
}

pub fn day_number(round: i32) -> Option<i32> {
    if !(FIRST_ROUND..=MATCH_ROUNDS).contains(&round) {
        return None;
    }
    let day_length = DAY_ROUNDS.checked_add(NIGHT_ROUNDS)?;
    Some((round - ROUND_OFFSET) / day_length + FIRST_ROUND)
}
