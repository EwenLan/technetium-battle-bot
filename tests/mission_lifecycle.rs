use technetium_battle_bot::ai::mission::{GoalEvidence, MissionRegistry};
use technetium_battle_bot::domain::{
    ActiveOwners, DeadlineKind, MissionDeadline, MissionSpec, OwnerAllocator, OwnerPath, Pos, Role,
};
use technetium_battle_bot::fsm::MissionState;
use technetium_battle_bot::protocol::decode;
use technetium_battle_bot::rules::constants::SINGLE_ITEM;

const DAY_REQUEST: &str = include_str!("fixtures/day.json");
const ROOT_ASSIGNEE: i64 = 10;
const CHILD_ASSIGNEE: i64 = 11;
const BUILDING_ID: i64 = 90;
const SITE_X: i32 = 4;
const SITE_Y: i32 = 4;
const BUILDING_HP: i32 = 1000;
const DEADLINE_ROUND: i32 = 40;
const AFTER_DEADLINE_ROUND: i32 = 41;
const BUILDING_KIND: &str = "gatling";
const WEAPON_ID: i64 = 20;
const EXPIRED_TREE_SIZE: usize = 2;

#[test]
fn observed_building_completes_mission_with_evidence() {
    let mut observation = observation();
    observation.team_our.roles.push(building());
    let (mut registry, mut owners, mut allocator) = registry_context();
    let owner = assignment(&mut owners, &mut allocator);
    let mission = activate(
        &mut registry,
        ROOT_ASSIGNEE,
        MissionSpec::construction(site(), BUILDING_KIND),
        owner,
    );

    let result = registry.reconcile(&observation, &[]);

    assert_eq!(result.resolved().len(), SINGLE_ITEM);
    assert_eq!(
        result.resolved().first().map(|item| item.state()),
        Some(MissionState::Succeeded)
    );
    let view = registry.view(mission).expect("mission view");
    assert_eq!(view.state(), MissionState::Succeeded);
    assert_eq!(
        view.completion_evidence(),
        &[GoalEvidence::EntityObserved {
            entity: BUILDING_ID,
            observed_round: observation.round_no,
        }]
    );
}

#[test]
fn completed_goal_releases_a_dependent_mission() {
    let mut observation = observation();
    observation.team_our.roles.push(building());
    let (mut registry, mut owners, mut allocator) = registry_context();
    let root_owner = assignment(&mut owners, &mut allocator);
    let root = activate(
        &mut registry,
        ROOT_ASSIGNEE,
        MissionSpec::construction(site(), BUILDING_KIND),
        root_owner,
    );
    let child_owner = assignment(&mut owners, &mut allocator);
    let child = registry
        .register(
            CHILD_ASSIGNEE,
            MissionSpec::defense(WEAPON_ID).with_dependencies([root]),
            child_owner,
        )
        .expect("dependent mission");

    let result = registry.reconcile(&observation, &[]);

    assert_eq!(result.ready(), &[child]);
    assert_eq!(state(&registry, root), MissionState::Succeeded);
    assert_eq!(state(&registry, child), MissionState::Ready);
}

#[test]
fn expired_prerequisite_cancels_its_dependency_tree() {
    let mut observation = observation();
    observation.round_no = DEADLINE_ROUND;
    let (mut registry, mut owners, mut allocator) = registry_context();
    let root_owner = assignment(&mut owners, &mut allocator);
    let root = activate(
        &mut registry,
        ROOT_ASSIGNEE,
        MissionSpec::construction(site(), BUILDING_KIND).with_deadline(MissionDeadline::new(
            DEADLINE_ROUND,
            false,
            DeadlineKind::ActionSubmission,
        )),
        root_owner,
    );
    let child_owner = assignment(&mut owners, &mut allocator);
    let child = registry
        .register(
            CHILD_ASSIGNEE,
            MissionSpec::defense(WEAPON_ID).with_dependencies([root]),
            child_owner,
        )
        .expect("dependent mission");

    let result = registry.reconcile(&observation, &[]);

    assert_eq!(result.resolved().len(), EXPIRED_TREE_SIZE);
    assert_eq!(state(&registry, root), MissionState::Expired);
    assert_eq!(state(&registry, child), MissionState::Cancelled);
    assert!(result.ready().is_empty());
}

#[test]
fn late_effect_evidence_does_not_revive_an_expired_mission() {
    let mut observation = observation();
    observation.round_no = AFTER_DEADLINE_ROUND;
    observation.team_our.roles.push(building());
    let (mut registry, mut owners, mut allocator) = registry_context();
    let owner = assignment(&mut owners, &mut allocator);
    let mission = activate(
        &mut registry,
        ROOT_ASSIGNEE,
        MissionSpec::construction(site(), BUILDING_KIND).with_deadline(MissionDeadline::new(
            DEADLINE_ROUND,
            true,
            DeadlineKind::EffectObservation,
        )),
        owner,
    );

    registry.reconcile(&observation, &[]);

    let view = registry.view(mission).expect("mission view");
    assert_eq!(view.state(), MissionState::Expired);
    assert!(view.completion_evidence().is_empty());
}

fn observation() -> technetium_battle_bot::domain::Observation {
    decode(DAY_REQUEST.as_bytes()).expect("fixture").observation
}

fn building() -> Role {
    Role {
        id: BUILDING_ID,
        pos: site(),
        role_type: BUILDING_KIND.to_owned(),
        health: Some(BUILDING_HP),
        attack_power: None,
        attack_range: None,
        back_pack_capability: None,
        backpack: Vec::new(),
        level: None,
        cooldown: None,
    }
}

fn site() -> Pos {
    Pos {
        x: SITE_X,
        y: SITE_Y,
    }
}

fn registry_context() -> (MissionRegistry, ActiveOwners, OwnerAllocator) {
    (
        MissionRegistry::default(),
        ActiveOwners::default(),
        OwnerAllocator::default(),
    )
}

fn assignment(owners: &mut ActiveOwners, allocator: &mut OwnerAllocator) -> OwnerPath {
    owners.create_assignment(allocator).expect("owner path")
}

fn activate(
    registry: &mut MissionRegistry,
    assignee: i64,
    spec: MissionSpec,
    owner: OwnerPath,
) -> technetium_battle_bot::domain::MissionId {
    let mission = registry.register(assignee, spec, owner).expect("mission");
    registry.assign(mission).expect("ready mission");
    registry.activate(mission).expect("assigned mission");
    mission
}

fn state(
    registry: &MissionRegistry,
    mission: technetium_battle_bot::domain::MissionId,
) -> MissionState {
    registry.view(mission).expect("mission view").state()
}
