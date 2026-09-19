use technetium_battle_bot::domain::{
    ActiveOwners, Generation, IntentId, MissionId, OwnerError, OwnerPath, PlanId, Versioned,
};

const MISSION_KEY: u64 = 11;
const PLAN_KEY: u64 = 21;
const NEXT_PLAN_KEY: u64 = 22;
const INTENT_KEY: u64 = 31;

#[test]
fn owner_path_requires_a_complete_parent_chain() {
    let mission = versioned_mission(Generation::initial());
    let intent = versioned_intent(Generation::initial());

    assert_eq!(
        OwnerPath::new(mission, None, Some(intent)),
        Err(OwnerError::IntentWithoutPlan)
    );
}

#[test]
fn changing_a_mission_generation_invalidates_the_whole_old_path() {
    let initial = Generation::initial();
    let next = initial.next().expect("generation space");
    let mission = versioned_mission(initial);
    let plan = versioned_plan(PLAN_KEY, initial);
    let intent = versioned_intent(initial);
    let owner = OwnerPath::new(mission, Some(plan), Some(intent)).expect("complete path");
    let mut owners = ActiveOwners::default();

    owners.activate_mission(mission).expect("new mission");
    owners.activate_plan(mission, plan).expect("current parent");
    owners
        .activate_intent(mission, plan, intent)
        .expect("current parents");
    assert!(owners.is_current(&owner));

    owners
        .activate_mission(versioned_mission(next))
        .expect("advanced mission");
    assert!(!owners.is_current(&owner));
}

#[test]
fn registration_rejects_stale_parents_and_reused_children() {
    let initial = Generation::initial();
    let next = initial.next().expect("generation space");
    let mission = versioned_mission(initial);
    let plan = versioned_plan(PLAN_KEY, initial);
    let mut owners = ActiveOwners::default();

    assert_eq!(
        owners.activate_plan(mission, plan),
        Err(OwnerError::MissionNotCurrent)
    );
    owners.activate_mission(mission).expect("new mission");
    owners.activate_plan(mission, plan).expect("new plan");
    assert_eq!(
        owners.activate_plan(mission, plan),
        Err(OwnerError::GenerationNotAdvanced)
    );

    let advanced_mission = versioned_mission(next);
    owners
        .activate_mission(advanced_mission)
        .expect("advanced mission");
    assert_eq!(
        owners.activate_plan(advanced_mission, versioned_plan(PLAN_KEY, next)),
        Err(OwnerError::ParentChanged)
    );
    assert!(
        owners
            .activate_plan(advanced_mission, versioned_plan(NEXT_PLAN_KEY, initial))
            .is_ok()
    );
}

fn versioned_mission(generation: Generation) -> Versioned<MissionId> {
    Versioned::new(MissionId::new(MISSION_KEY), generation)
}

fn versioned_plan(id: u64, generation: Generation) -> Versioned<PlanId> {
    Versioned::new(PlanId::new(id), generation)
}

fn versioned_intent(generation: Generation) -> Versioned<IntentId> {
    Versioned::new(IntentId::new(INTENT_KEY), generation)
}
