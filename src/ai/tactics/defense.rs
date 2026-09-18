use std::collections::BTreeSet;

use crate::command::Arbiter;
use crate::domain::{Action, Observation, Pos, Role};
use crate::rules::constants::{
    DEFAULT_LEVEL, MAX_FIRE_TARGETS, NEIGHBOR_RANGE, NO_COOLDOWN, NO_HEALTH,
};
use crate::rules::geometry::{distance, station_distance};
use crate::world::WorldView;

use super::next_step;

pub fn defend(observation: &Observation, arbiter: &mut Arbiter<'_>) {
    let view = WorldView::new(observation);
    let mut available: BTreeSet<i64> = observation
        .team_our
        .roles
        .iter()
        .filter(|role| {
            is_unit(role) && (role.role_type == "worker" || observation.phase_task.is_empty())
        })
        .map(|role| role.id)
        .collect();
    let mut weapons: Vec<&Role> = observation
        .team_our
        .roles
        .iter()
        .filter(|role| crate::ai::strategy::is_weapon(&role.role_type) && is_alive(role))
        .collect();
    weapons.sort_by_key(|role| role.id);
    for weapon in weapons {
        let Some(controller) = choose_controller(observation, weapon, &available) else {
            continue;
        };
        available.remove(&controller.id);
        let action = defense_action(&view, controller, weapon);
        if let Some(action) = action {
            let actor = if matches!(action, Action::Attack { .. }) {
                weapon.id
            } else {
                controller.id
            };
            arbiter.propose(actor, action);
        }
    }
}

fn is_alive(role: &Role) -> bool {
    role.health.is_some_and(|hp| hp > NO_HEALTH)
}

fn is_unit(role: &Role) -> bool {
    is_alive(role) && matches!(role.role_type.as_str(), "worker" | "pioneer")
}

fn choose_controller<'a>(
    observation: &'a Observation,
    weapon: &Role,
    available: &BTreeSet<i64>,
) -> Option<&'a Role> {
    observation
        .team_our
        .roles
        .iter()
        .filter(|role| available.contains(&role.id))
        .min_by_key(|role| (distance(role.pos, weapon.pos), role.id))
}

fn defense_action(view: &WorldView<'_>, controller: &Role, weapon: &Role) -> Option<Action> {
    if distance(controller.pos, weapon.pos) <= NEIGHBOR_RANGE {
        return fire(view.observation, controller, weapon);
    }
    let stands = view.interact_positions(weapon.pos, controller.id);
    let step = next_step(view, controller.id, controller.pos, &stands)?;
    Some(Action::Move(step.next))
}

fn fire(observation: &Observation, controller: &Role, weapon: &Role) -> Option<Action> {
    if weapon.cooldown != Some(NO_COOLDOWN) {
        return None;
    }
    let targets = fire_targets(observation, weapon);
    if targets.is_empty() {
        return None;
    }
    Some(Action::Attack {
        controller: controller.id,
        targets,
    })
}

fn fire_targets(observation: &Observation, weapon: &Role) -> Vec<Pos> {
    let range = weapon.attack_range.unwrap_or(NO_HEALTH);
    let level = weapon.level.unwrap_or(DEFAULT_LEVEL);
    let Some(robots) = &observation.robot else {
        return Vec::new();
    };
    let mut candidates: Vec<&Role> = robots
        .roles
        .iter()
        .filter(|role| {
            role.health.is_some_and(|hp| hp > NO_HEALTH) && distance(role.pos, weapon.pos) <= range
        })
        .collect();
    candidates.sort_by_key(|role| target_priority(observation, role));
    candidates.truncate(MAX_FIRE_TARGETS);
    select_targets(weapon, level, &candidates)
}

fn target_priority(observation: &Observation, robot: &Role) -> (i32, i32, i64) {
    let base = observation
        .team_our
        .roles
        .iter()
        .find(|role| role.role_type == "station");
    let base_distance = base
        .map(|role| station_distance(role.pos, robot.pos))
        .unwrap_or(i32::MAX);
    (base_distance, robot.health.unwrap_or(i32::MAX), robot.id)
}

fn select_targets(weapon: &Role, level: i32, candidates: &[&Role]) -> Vec<Pos> {
    let Ok(required) = usize::try_from(level) else {
        return Vec::new();
    };
    match weapon.role_type.as_str() {
        "railgun" => candidates
            .first()
            .map(|role| vec![role.pos])
            .unwrap_or_default(),
        "rocket" => candidates
            .first()
            .map(|role| vec![role.pos; required])
            .unwrap_or_default(),
        "gatling" => select_gatling(weapon.pos, candidates, required),
        _ => Vec::new(),
    }
}

fn select_gatling(origin: Pos, candidates: &[&Role], required: usize) -> Vec<Pos> {
    let mut selected = Vec::new();
    for candidate in candidates {
        if selected.len() == required {
            break;
        }
        if selected
            .iter()
            .all(|pos| angle_legal(origin, *pos, candidate.pos))
        {
            selected.push(candidate.pos);
        }
    }
    if selected.len() == required {
        selected
    } else {
        Vec::new()
    }
}

fn angle_legal(origin: Pos, first: Pos, second: Pos) -> bool {
    let a_x = i64::from(first.x - origin.x);
    let a_y = i64::from(first.y - origin.y);
    let b_x = i64::from(second.x - origin.x);
    let b_y = i64::from(second.y - origin.y);
    a_x * b_x + a_y * b_y >= i64::from(NO_HEALTH)
}
