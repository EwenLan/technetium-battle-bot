use crate::domain::{DeadlineKind, MissionDeadline, MissionSpec, Observation, Pos};
use crate::rules::constants::{ANSWER_MARGIN_ROUNDS, MIN_ACTION_ROUNDS};
use crate::rules::time::{next_day, next_night};

pub(super) fn economy_spec(observation: &Observation) -> MissionSpec {
    attach(MissionSpec::economy(), day_submission_deadline(observation))
}

pub(super) fn construction_spec(
    observation: &Observation,
    site: Pos,
    building: impl Into<String>,
) -> MissionSpec {
    attach(
        MissionSpec::construction(site, building),
        day_submission_deadline(observation),
    )
}

pub(super) fn challenge_spec(
    observation: &Observation,
    timeout_rounds: Option<i32>,
) -> MissionSpec {
    let day = day_submission_deadline(observation);
    let relative = timeout_rounds.and_then(|timeout| {
        observation
            .round_no
            .checked_add(timeout)?
            .checked_sub(ANSWER_MARGIN_ROUNDS)
    });
    attach(
        MissionSpec::challenge(),
        earlier_submission_deadline(day, relative),
    )
}

pub(crate) fn defense_spec(observation: &Observation, weapon: i64) -> MissionSpec {
    let deadline = next_day(observation.round_no)
        .map(|round| MissionDeadline::new(round, true, DeadlineKind::EffectObservation));
    attach(MissionSpec::defense(weapon), deadline)
}

fn day_submission_deadline(observation: &Observation) -> Option<MissionDeadline> {
    let round = next_night(observation.round_no)?.checked_sub(MIN_ACTION_ROUNDS)?;
    Some(MissionDeadline::new(
        round,
        true,
        DeadlineKind::ActionSubmission,
    ))
}

fn earlier_submission_deadline(
    day: Option<MissionDeadline>,
    relative_round: Option<i32>,
) -> Option<MissionDeadline> {
    let day = day?;
    let round = relative_round
        .map(|relative| relative.min(day.at_round()))
        .unwrap_or(day.at_round());
    Some(MissionDeadline::new(
        round,
        true,
        DeadlineKind::ActionSubmission,
    ))
}

fn attach(spec: MissionSpec, deadline: Option<MissionDeadline>) -> MissionSpec {
    match deadline {
        Some(deadline) => spec.with_deadline(deadline),
        None => spec,
    }
}
