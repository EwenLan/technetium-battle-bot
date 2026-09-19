use crate::domain::{GoalPredicate, Observation};
use crate::event::{EventRecord, WorldEvent};
use crate::rules::constants::NO_HEALTH;
use crate::rules::time::Phase;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GoalEvidence {
    EntityObserved { entity: i64, observed_round: i32 },
    EventObserved { event: u64, observed_round: i32 },
}

impl GoalEvidence {
    pub const fn observed_round(self) -> i32 {
        match self {
            Self::EntityObserved { observed_round, .. }
            | Self::EventObserved { observed_round, .. } => observed_round,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GoalEvaluation {
    Satisfied(Vec<GoalEvidence>),
    Pending,
    Impossible,
    Unknown,
}

pub fn evaluate_goal(
    goal: &GoalPredicate,
    observation: &Observation,
    events: &[EventRecord],
) -> GoalEvaluation {
    match goal {
        GoalPredicate::EconomyCycleCompleted => GoalEvaluation::Pending,
        GoalPredicate::BuildingPresent { kind, site } => {
            evaluate_building(kind, *site, observation)
        }
        GoalPredicate::ChallengeResolved => evaluate_event(events, challenge_ended),
        GoalPredicate::DefenseWindowCompleted { .. } => evaluate_event(events, day_started),
        GoalPredicate::AllOf(goals) => evaluate_all(goals, observation, events),
    }
}

fn evaluate_building(
    kind: &str,
    site: crate::domain::Pos,
    observation: &Observation,
) -> GoalEvaluation {
    let entity = observation.team_our.roles.iter().find(|role| {
        role.role_type == kind
            && role.pos == site
            && role.health.is_some_and(|health| health > NO_HEALTH)
    });
    match entity {
        Some(role) => GoalEvaluation::Satisfied(vec![GoalEvidence::EntityObserved {
            entity: role.id,
            observed_round: observation.round_no,
        }]),
        None => GoalEvaluation::Pending,
    }
}

fn evaluate_event(events: &[EventRecord], predicate: fn(&WorldEvent) -> bool) -> GoalEvaluation {
    events
        .iter()
        .find(|record| predicate(&record.event))
        .map(|record| {
            GoalEvaluation::Satisfied(vec![GoalEvidence::EventObserved {
                event: record.id,
                observed_round: record.observed_round,
            }])
        })
        .unwrap_or(GoalEvaluation::Pending)
}

fn evaluate_all(
    goals: &[GoalPredicate],
    observation: &Observation,
    events: &[EventRecord],
) -> GoalEvaluation {
    let mut evidence = Vec::new();
    let mut pending = false;
    for goal in goals {
        match evaluate_goal(goal, observation, events) {
            GoalEvaluation::Satisfied(found) => evidence.extend(found),
            GoalEvaluation::Pending => pending = true,
            GoalEvaluation::Impossible => return GoalEvaluation::Impossible,
            GoalEvaluation::Unknown => return GoalEvaluation::Unknown,
        }
    }
    if pending {
        GoalEvaluation::Pending
    } else {
        evidence.sort_unstable();
        evidence.dedup();
        GoalEvaluation::Satisfied(evidence)
    }
}

fn challenge_ended(event: &WorldEvent) -> bool {
    matches!(event, WorldEvent::ChallengeEnded)
}

fn day_started(event: &WorldEvent) -> bool {
    matches!(event, WorldEvent::PhaseChanged(Phase::Day))
}
