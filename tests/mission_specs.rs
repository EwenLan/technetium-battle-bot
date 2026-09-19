use technetium_battle_bot::domain::{MissionKind, MissionSpec, ObjectiveKey, Pos};

const SITE_X: i32 = 3;
const SITE_Y: i32 = 7;
const OTHER_SITE_X: i32 = 4;
const WEAPON_ID: i64 = 20;
const OTHER_WEAPON_ID: i64 = 21;

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
        MissionSpec::construction(site).kind(),
        MissionKind::Construct
    );
    assert_eq!(
        MissionSpec::construction(site).objective(),
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
        MissionSpec::construction(first_site),
        MissionSpec::construction(second_site)
    );
    assert_ne!(
        MissionSpec::defense(WEAPON_ID),
        MissionSpec::defense(OTHER_WEAPON_ID)
    );
}
