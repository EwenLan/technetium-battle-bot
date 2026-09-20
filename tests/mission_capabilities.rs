use technetium_battle_bot::ai::mission::{
    CapabilityRejection, check_assignment, role_capabilities,
};
use technetium_battle_bot::domain::{MissionCapability, MissionSpec, Pos};
use technetium_battle_bot::protocol::decode;

const DAY_REQUEST: &str = include_str!("fixtures/day.json");
const WORKER_ID: i64 = 10;
const PIONEER_ID: i64 = 11;
const UNKNOWN_ID: i64 = 999;
const SITE_X: i32 = 4;
const SITE_Y: i32 = 4;
const WEAPON_ID: i64 = 20;
const NO_HEALTH: i32 = 0;
const BUILDING_KIND: &str = "gatling";

#[test]
fn worker_profile_supports_economy_construction_and_defense() {
    let observation = observation();
    let worker = role(&observation, WORKER_ID);
    let available = role_capabilities(worker);

    assert!(available.contains(&MissionCapability::Gather));
    assert!(available.contains(&MissionCapability::Sell));
    assert!(available.contains(&MissionCapability::Build));
    assert!(available.contains(&MissionCapability::OperateWeapon));
    assert!(check_assignment(&observation, WORKER_ID, &MissionSpec::economy()).is_ok());
    assert!(check_assignment(&observation, WORKER_ID, &construction_spec()).is_ok());
    assert!(check_assignment(&observation, WORKER_ID, &MissionSpec::defense(WEAPON_ID)).is_ok());
}

#[test]
fn pioneer_profile_is_limited_to_challenges_and_weapon_control() {
    let observation = observation();
    let pioneer = role(&observation, PIONEER_ID);
    let available = role_capabilities(pioneer);

    assert!(available.contains(&MissionCapability::SolveChallenge));
    assert!(available.contains(&MissionCapability::OperateWeapon));
    assert_eq!(
        check_assignment(&observation, PIONEER_ID, &MissionSpec::economy()),
        Err(CapabilityRejection::MissingCapability(
            MissionCapability::Gather
        ))
    );
    assert!(check_assignment(&observation, PIONEER_ID, &MissionSpec::challenge()).is_ok());
}

#[test]
fn unavailable_assignees_are_rejected_before_capability_matching() {
    let mut dead = observation();
    role_mut(&mut dead, PIONEER_ID).health = Some(NO_HEALTH);
    let mut unknown_health = observation();
    role_mut(&mut unknown_health, PIONEER_ID).health = None;

    assert_eq!(
        check_assignment(&dead, PIONEER_ID, &MissionSpec::challenge()),
        Err(CapabilityRejection::AssigneeDead)
    );
    assert_eq!(
        check_assignment(&unknown_health, PIONEER_ID, &MissionSpec::challenge()),
        Err(CapabilityRejection::HealthUnknown)
    );
    assert_eq!(
        check_assignment(&dead, UNKNOWN_ID, &MissionSpec::challenge()),
        Err(CapabilityRejection::AssigneeMissing)
    );
}

fn observation() -> technetium_battle_bot::domain::Observation {
    decode(DAY_REQUEST.as_bytes()).expect("fixture").observation
}

fn role(
    observation: &technetium_battle_bot::domain::Observation,
    id: i64,
) -> &technetium_battle_bot::domain::Role {
    observation
        .team_our
        .roles
        .iter()
        .find(|role| role.id == id)
        .expect("role")
}

fn role_mut(
    observation: &mut technetium_battle_bot::domain::Observation,
    id: i64,
) -> &mut technetium_battle_bot::domain::Role {
    observation
        .team_our
        .roles
        .iter_mut()
        .find(|role| role.id == id)
        .expect("role")
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
