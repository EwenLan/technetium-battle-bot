use technetium_battle_bot::ai::mission::{
    MissionCancellation, MissionRegistry, MissionRegistryError,
};
use technetium_battle_bot::domain::{
    ActiveOwners, MissionId, MissionSpec, OwnerAllocator, OwnerPath, Pos, PriorityClass,
};
use technetium_battle_bot::fsm::MissionState;

const ROOT_ASSIGNEE: i64 = 10;
const CHILD_ASSIGNEE: i64 = 11;
const GRANDCHILD_ASSIGNEE: i64 = 12;
const SITE_X: i32 = 3;
const SITE_Y: i32 = 7;
const WEAPON_ID: i64 = 20;
const UNKNOWN_MISSION_KEY: u64 = 999;
const CANCELLED_MISSION_COUNT: usize = 3;
const BUILDING_KIND: &str = "gatling";

#[test]
fn succeeding_a_prerequisite_releases_its_dependents() {
    let mut owners = ActiveOwners::default();
    let mut allocator = OwnerAllocator::default();
    let mut registry = MissionRegistry::default();
    let root_owner = assignment(&mut owners, &mut allocator);
    let root = registry
        .register(ROOT_ASSIGNEE, MissionSpec::economy(), root_owner)
        .expect("root mission");
    registry.assign(root).expect("ready root");
    registry.activate(root).expect("assigned root");
    let child_owner = assignment(&mut owners, &mut allocator);
    let child = registry
        .register(
            CHILD_ASSIGNEE,
            construction_spec().with_dependencies([root]),
            child_owner,
        )
        .expect("dependent mission");

    let child_view = registry.view(child).expect("child view");
    assert_eq!(
        child_view.spec().objective(),
        construction_spec().objective()
    );
    assert_eq!(child_view.owner(), child_owner);
    assert_eq!(child_view.assignee(), CHILD_ASSIGNEE);
    assert!(child_view.dependencies().contains(&root));
    assert_eq!(child_view.state(), MissionState::Proposed);
    let completion = registry.succeed(root).expect("executing root");
    owners.deactivate_assignment(&completion.owner());
    assert_eq!(completion.ready(), &[child]);
    assert_eq!(state(&registry, root), MissionState::Succeeded);
    assert_eq!(state(&registry, child), MissionState::Ready);
    assert!(
        registry
            .current_assignment(ROOT_ASSIGNEE, &MissionSpec::economy())
            .is_none()
    );
}

#[test]
fn cancelling_a_prerequisite_cancels_the_dependency_tree() {
    let mut owners = ActiveOwners::default();
    let mut allocator = OwnerAllocator::default();
    let mut registry = MissionRegistry::default();
    let root_owner = assignment(&mut owners, &mut allocator);
    let root = registry
        .register(ROOT_ASSIGNEE, MissionSpec::economy(), root_owner)
        .expect("root mission");
    registry.assign(root).expect("ready root");
    registry.activate(root).expect("assigned root");
    let child = register_child(&mut registry, &mut owners, &mut allocator, root);
    let grandchild_owner = assignment(&mut owners, &mut allocator);
    let grandchild = registry
        .register(
            GRANDCHILD_ASSIGNEE,
            MissionSpec::defense(WEAPON_ID).with_dependencies([child]),
            grandchild_owner,
        )
        .expect("grandchild mission");

    let cancelled = registry.cancel(root).expect("active root");
    deactivate_cancelled(&mut owners, &cancelled);

    assert_eq!(cancelled.len(), CANCELLED_MISSION_COUNT);
    assert_eq!(state(&registry, root), MissionState::Cancelled);
    assert_eq!(state(&registry, child), MissionState::Cancelled);
    assert_eq!(state(&registry, grandchild), MissionState::Cancelled);
}

#[test]
fn assigning_replacement_cancels_the_previous_active_mission() {
    let mut owners = ActiveOwners::default();
    let mut allocator = OwnerAllocator::default();
    let mut registry = MissionRegistry::default();
    let first_owner = assignment(&mut owners, &mut allocator);
    let first = registry
        .register(ROOT_ASSIGNEE, MissionSpec::economy(), first_owner)
        .expect("first mission");
    registry.assign(first).expect("ready first");
    registry.activate(first).expect("assigned first");
    let second_owner = assignment(&mut owners, &mut allocator);
    let second = registry
        .register(ROOT_ASSIGNEE, construction_spec(), second_owner)
        .expect("replacement mission");

    let cancelled = registry.assign(second).expect("ready replacement");
    deactivate_cancelled(&mut owners, &cancelled);
    registry.activate(second).expect("assigned replacement");

    assert_eq!(
        cancelled.first().map(|item| item.owner()),
        Some(first_owner)
    );
    assert_eq!(state(&registry, first), MissionState::Cancelled);
    assert_eq!(state(&registry, second), MissionState::Executing);
    assert!(!owners.is_current(&first_owner));
    assert!(owners.is_current(&second_owner));
    assert_eq!(
        registry.current_assignment(ROOT_ASSIGNEE, &construction_spec()),
        Some(second_owner)
    );
}

#[test]
fn assignment_identity_includes_the_full_contract() {
    let mut owners = ActiveOwners::default();
    let mut allocator = OwnerAllocator::default();
    let mut registry = MissionRegistry::default();
    let owner = assignment(&mut owners, &mut allocator);
    let spec = construction_spec();
    let mission = registry
        .register(ROOT_ASSIGNEE, spec.clone(), owner)
        .expect("mission");
    registry.assign(mission).expect("ready mission");
    registry.activate(mission).expect("assigned mission");

    assert_eq!(
        registry.current_assignment(ROOT_ASSIGNEE, &spec),
        Some(owner)
    );
    let changed = construction_spec().with_priority(PriorityClass::Q2Deadline);
    assert!(
        registry
            .current_assignment(ROOT_ASSIGNEE, &changed)
            .is_none()
    );
}

#[test]
fn registration_rejects_unknown_dependencies() {
    let mut owners = ActiveOwners::default();
    let mut allocator = OwnerAllocator::default();
    let mut registry = MissionRegistry::default();
    let owner = assignment(&mut owners, &mut allocator);

    assert_eq!(
        registry.register(
            ROOT_ASSIGNEE,
            MissionSpec::economy().with_dependencies([MissionId::new(UNKNOWN_MISSION_KEY)]),
            owner,
        ),
        Err(MissionRegistryError::UnknownDependency)
    );

    let self_owner = assignment(&mut owners, &mut allocator);
    let self_dependency = self_owner.mission().id;
    assert_eq!(
        registry.register(
            CHILD_ASSIGNEE,
            MissionSpec::economy().with_dependencies([self_dependency]),
            self_owner,
        ),
        Err(MissionRegistryError::SelfDependency)
    );

    let root_owner = assignment(&mut owners, &mut allocator);
    let root = registry
        .register(ROOT_ASSIGNEE, MissionSpec::economy(), root_owner)
        .expect("root mission");
    registry.cancel(root).expect("ready root");
    let dependent_owner = assignment(&mut owners, &mut allocator);
    assert_eq!(
        registry.register(
            CHILD_ASSIGNEE,
            construction_spec().with_dependencies([root]),
            dependent_owner,
        ),
        Err(MissionRegistryError::DependencyUnavailable)
    );
}

fn assignment(owners: &mut ActiveOwners, allocator: &mut OwnerAllocator) -> OwnerPath {
    owners.create_assignment(allocator).expect("owner space")
}

fn construction_spec() -> MissionSpec {
    MissionSpec::construction(
        Pos {
            x: SITE_X,
            y: SITE_Y,
        },
        BUILDING_KIND,
    )
}

fn register_child(
    registry: &mut MissionRegistry,
    owners: &mut ActiveOwners,
    allocator: &mut OwnerAllocator,
    root: MissionId,
) -> MissionId {
    let owner = assignment(owners, allocator);
    registry
        .register(
            CHILD_ASSIGNEE,
            construction_spec().with_dependencies([root]),
            owner,
        )
        .expect("child mission")
}

fn state(registry: &MissionRegistry, mission: MissionId) -> MissionState {
    registry.view(mission).expect("mission view").state()
}

fn deactivate_cancelled(owners: &mut ActiveOwners, cancelled: &[MissionCancellation]) {
    for cancellation in cancelled {
        owners.deactivate_assignment(&cancellation.owner());
    }
}
