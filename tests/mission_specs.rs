use technetium_battle_bot::domain::{
    DeadlineKind, GoalPredicate, Interruptibility, MissionCapability, MissionDeadline, MissionId,
    MissionKind, MissionSpec, ObjectiveKey, Pos, PriorityClass, RetryPolicy,
};

const SITE_X: i32 = 3;
const SITE_Y: i32 = 7;
const OTHER_SITE_X: i32 = 4;
const WEAPON_ID: i64 = 20;
const OTHER_WEAPON_ID: i64 = 21;
const BUILDING_KIND: &str = "gatling";
const DEPENDENCY_KEY: u64 = 31;
const DEADLINE_ROUND: i32 = 40;
const DEADLINE_NEXT_ROUND: i32 = 41;
const CUSTOM_REPLAN_LIMIT: u8 = 2;

#[test]
fn constructors_bind_mission_kinds_to_their_objectives() {
    let site = Pos {
        x: SITE_X,
        y: SITE_Y,
    };

    assert_eq!(MissionSpec::economy().kind(), MissionKind::GatherAndSell);
    assert_eq!(
        MissionSpec::economy().objective(),
        ObjectiveKey::EconomyCycle
    );
    assert_eq!(
        MissionSpec::construction(site, BUILDING_KIND).kind(),
        MissionKind::Construct
    );
    assert_eq!(
        MissionSpec::construction(site, BUILDING_KIND).objective(),
        ObjectiveKey::ConstructionSite(site)
    );
    assert_eq!(MissionSpec::challenge().kind(), MissionKind::Challenge);
    assert_eq!(
        MissionSpec::challenge().objective(),
        ObjectiveKey::ChallengeSession
    );
    assert_eq!(
        MissionSpec::defense(WEAPON_ID).kind(),
        MissionKind::DefendSector
    );
    assert_eq!(
        MissionSpec::defense(WEAPON_ID).objective(),
        ObjectiveKey::DefenseWeapon(WEAPON_ID)
    );
}

#[test]
fn objective_keys_distinguish_targets_within_one_mission_kind() {
    let first_site = Pos {
        x: SITE_X,
        y: SITE_Y,
    };
    let second_site = Pos {
        x: OTHER_SITE_X,
        y: SITE_Y,
    };

    assert_ne!(
        MissionSpec::construction(first_site, BUILDING_KIND),
        MissionSpec::construction(second_site, BUILDING_KIND)
    );
    assert_ne!(
        MissionSpec::defense(WEAPON_ID),
        MissionSpec::defense(OTHER_WEAPON_ID)
    );
}

#[test]
fn mission_contract_carries_goal_dependencies_and_policy() {
    let site = Pos {
        x: SITE_X,
        y: SITE_Y,
    };
    let dependency = MissionId::new(DEPENDENCY_KEY);
    let deadline = MissionDeadline::new(DEADLINE_ROUND, true, DeadlineKind::EffectObservation);
    let spec = MissionSpec::construction(site, BUILDING_KIND)
        .with_dependencies([dependency])
        .with_deadline(deadline)
        .with_priority(PriorityClass::Q2Deadline)
        .with_interruptibility(Interruptibility::CancelOnly)
        .with_retry_policy(RetryPolicy::bounded(CUSTOM_REPLAN_LIMIT));

    assert_eq!(
        spec.goal(),
        &GoalPredicate::BuildingPresent {
            kind: BUILDING_KIND.to_owned(),
            site,
        }
    );
    assert!(spec.dependencies().contains(&dependency));
    assert!(
        spec.required_capabilities()
            .contains(&MissionCapability::Build)
    );
    assert_eq!(spec.deadline(), Some(deadline));
    assert_eq!(spec.priority(), PriorityClass::Q2Deadline);
    assert_eq!(spec.interruptibility(), Interruptibility::CancelOnly);
    assert_eq!(spec.retry_policy().max_replans(), CUSTOM_REPLAN_LIMIT);
}

#[test]
fn deadline_inclusivity_controls_the_expired_boundary() {
    let inclusive = MissionDeadline::new(DEADLINE_ROUND, true, DeadlineKind::EffectObservation);
    let exclusive = MissionDeadline::new(DEADLINE_ROUND, false, DeadlineKind::ActionSubmission);

    assert!(!inclusive.is_expired(DEADLINE_ROUND));
    assert!(inclusive.is_expired(DEADLINE_NEXT_ROUND));
    assert!(exclusive.is_expired(DEADLINE_ROUND));
}
