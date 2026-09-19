use technetium_battle_bot::domain::{
    ActiveOwners, Generation, IntentId, MissionId, OwnerAllocator, OwnerError, OwnerPath, PlanId,
    Versioned,
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

#[test]
fn allocator_creates_distinct_current_owner_paths() {
    let mut owners = ActiveOwners::default();
    let mut allocator = OwnerAllocator::default();

    let first = owners.create_path(&mut allocator).expect("first path");
    let second = owners.create_path(&mut allocator).expect("second path");

    assert_ne!(first.mission().id, second.mission().id);
    assert!(owners.is_current(&first));
    assert!(owners.is_current(&second));
}

#[test]
fn assignment_reuses_parent_for_successive_intents() {
    let mut owners = ActiveOwners::default();
    let mut allocator = OwnerAllocator::default();
    let assignment = owners
        .create_assignment(&mut allocator)
        .expect("assignment");
    let first = owners
        .create_intent(&mut allocator, &assignment)
        .expect("first intent");
    assert!(owners.deactivate_intent(first.intent().expect("intent").id));
    let second = owners
        .create_intent(&mut allocator, &assignment)
        .expect("second intent");

    assert_eq!(first.mission(), second.mission());
    assert_eq!(first.plan(), second.plan());
    assert_ne!(first.intent(), second.intent());
    assert!(owners.is_current(&assignment));
    assert!(!owners.is_current(&first));
    assert!(owners.is_current(&second));
}

#[test]
fn deactivated_nodes_keep_generation_tombstones() {
    let initial = Generation::initial();
    let next = initial.next().expect("generation space");
    let mission = versioned_mission(initial);
    let mut owners = ActiveOwners::default();

    owners.activate_mission(mission).expect("new mission");
    assert!(owners.deactivate_mission(mission.id));
    assert_eq!(
        owners.activate_mission(mission),
        Err(OwnerError::GenerationNotAdvanced)
    );
    assert!(owners.activate_mission(versioned_mission(next)).is_ok());
}

#[test]
fn deactivating_plan_invalidates_its_intents_only() {
    let mut owners = ActiveOwners::default();
    let mut allocator = OwnerAllocator::default();
    let assignment = owners
        .create_assignment(&mut allocator)
        .expect("assignment");
    let action = owners
        .create_intent(&mut allocator, &assignment)
        .expect("intent");
    let mission = OwnerPath::new(assignment.mission(), None, None).expect("mission owner");

    assert!(owners.deactivate_plan(assignment.plan().expect("plan").id));

    assert!(owners.is_current(&mission));
    assert!(!owners.is_current(&assignment));
    assert!(!owners.is_current(&action));
}

#[test]
fn deactivated_intent_requires_an_advanced_generation() {
    let mut owners = ActiveOwners::default();
    let mut allocator = OwnerAllocator::default();
    let assignment = owners
        .create_assignment(&mut allocator)
        .expect("assignment");
    let action = owners
        .create_intent(&mut allocator, &assignment)
        .expect("intent");
    let mission = assignment.mission();
    let plan = assignment.plan().expect("plan");
    let intent = action.intent().expect("intent");
    assert!(owners.deactivate_intent(intent.id));

    assert_eq!(
        owners.activate_intent(mission, plan, intent),
        Err(OwnerError::GenerationNotAdvanced)
    );
    let advanced = Versioned::new(
        intent.id,
        intent.generation.next().expect("generation space"),
    );
    owners
        .activate_intent(mission, plan, advanced)
        .expect("advanced intent");
    let replacement = OwnerPath::new(mission, Some(plan), Some(advanced)).expect("owner");
    assert!(owners.is_current(&replacement));
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
