use technetium_battle_bot::ai::mission::{GoalEvaluation, GoalEvidence, evaluate_goal};
use technetium_battle_bot::domain::{GoalPredicate, Pos};
use technetium_battle_bot::event::{EventRecord, WorldEvent};
use technetium_battle_bot::protocol::decode;
use technetium_battle_bot::rules::time::Phase;

const DAY_REQUEST: &str = include_str!("fixtures/day.json");
const CHALLENGE_EVENT_ID: u64 = 7;
const DEFENSE_EVENT_ID: u64 = 8;
const OBSERVED_ROUND: i32 = 131;
const WEAPON_ID: i64 = 20;
const UNUSED_X: i32 = 1;
const UNUSED_Y: i32 = 1;
const BUILDING_KIND: &str = "rocket";

#[test]
fn challenge_and_defense_goals_require_matching_world_events() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let events = vec![
        event(CHALLENGE_EVENT_ID, WorldEvent::ChallengeEnded),
        event(DEFENSE_EVENT_ID, WorldEvent::PhaseChanged(Phase::Day)),
    ];

    assert_eq!(
        evaluate_goal(&GoalPredicate::ChallengeResolved, &observation, &events),
        GoalEvaluation::Satisfied(vec![GoalEvidence::EventObserved {
            event: CHALLENGE_EVENT_ID,
            observed_round: OBSERVED_ROUND,
        }])
    );
    assert_eq!(
        evaluate_goal(
            &GoalPredicate::DefenseWindowCompleted { weapon: WEAPON_ID },
            &observation,
            &events,
        ),
        GoalEvaluation::Satisfied(vec![GoalEvidence::EventObserved {
            event: DEFENSE_EVENT_ID,
            observed_round: OBSERVED_ROUND,
        }])
    );
}

#[test]
fn all_of_remains_pending_until_every_goal_has_evidence() {
    let observation = decode(DAY_REQUEST.as_bytes()).expect("fixture").observation;
    let events = vec![event(CHALLENGE_EVENT_ID, WorldEvent::ChallengeEnded)];
    let goal = GoalPredicate::AllOf(vec![
        GoalPredicate::ChallengeResolved,
        GoalPredicate::BuildingPresent {
            kind: BUILDING_KIND.to_owned(),
            site: Pos {
                x: UNUSED_X,
                y: UNUSED_Y,
            },
        },
    ]);

    assert_eq!(
        evaluate_goal(&goal, &observation, &events),
        GoalEvaluation::Pending
    );
}

fn event(id: u64, event: WorldEvent) -> EventRecord {
    EventRecord {
        id,
        observed_round: OBSERVED_ROUND,
        event,
    }
}
