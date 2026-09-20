use std::collections::BTreeSet;

use crate::domain::{MissionCapability, MissionSpec, Observation, Role};
use crate::rules::constants::NO_HEALTH;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityRejection {
    AssigneeMissing,
    HealthUnknown,
    AssigneeDead,
    MissingCapability(MissionCapability),
}

pub fn role_capabilities(role: &Role) -> BTreeSet<MissionCapability> {
    match role.role_type.as_str() {
        "worker" => BTreeSet::from([
            MissionCapability::Gather,
            MissionCapability::Sell,
            MissionCapability::Build,
            MissionCapability::OperateWeapon,
        ]),
        "pioneer" => BTreeSet::from([
            MissionCapability::SolveChallenge,
            MissionCapability::OperateWeapon,
        ]),
        _ => BTreeSet::new(),
    }
}

pub fn check_assignment(
    observation: &Observation,
    assignee: i64,
    spec: &MissionSpec,
) -> Result<(), CapabilityRejection> {
    let role = observation
        .team_our
        .roles
        .iter()
        .find(|role| role.id == assignee)
        .ok_or(CapabilityRejection::AssigneeMissing)?;
    check_health(role)?;
    let available = role_capabilities(role);
    spec.required_capabilities()
        .iter()
        .find(|required| !available.contains(required))
        .copied()
        .map(CapabilityRejection::MissingCapability)
        .map_or(Ok(()), Err)
}

fn check_health(role: &Role) -> Result<(), CapabilityRejection> {
    match role.health {
        Some(health) if health > NO_HEALTH => Ok(()),
        Some(_) => Err(CapabilityRejection::AssigneeDead),
        None => Err(CapabilityRejection::HealthUnknown),
    }
}
