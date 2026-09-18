use std::collections::BTreeMap;

use crate::domain::{Observation, Role};
use crate::event::WorldEvent;
use crate::rules::constants::{NO_COOLDOWN, NO_HEALTH, VERSION_INCREMENT};
use crate::rules::time::phase;
use crate::world::memory::WorldMemory;

#[derive(Clone, Debug, Default)]
pub struct World {
    pub observation: Option<Observation>,
    pub version: u64,
    pub events: Vec<WorldEvent>,
    pub memory: WorldMemory,
}

impl World {
    pub fn apply(&mut self, observation: Observation) {
        self.events = build_events(self.observation.as_ref(), &observation);
        self.memory.observe(&observation);
        self.observation = Some(observation);
        self.version = self.version.saturating_add(VERSION_INCREMENT);
    }
}

fn build_events(previous: Option<&Observation>, current: &Observation) -> Vec<WorldEvent> {
    let mut events = vec![WorldEvent::RoundStarted(current.round_no)];
    let Some(previous) = previous else {
        return events;
    };
    append_phase(previous, current, &mut events);
    append_roles(previous, current, &mut events);
    append_new_buildings(previous, current, &mut events);
    append_mines(previous, current, &mut events);
    append_enemy_visibility(previous, current, &mut events);
    append_misc(previous, current, &mut events);
    events
}

fn append_new_buildings(
    previous: &Observation,
    current: &Observation,
    events: &mut Vec<WorldEvent>,
) {
    let old = role_index(&previous.team_our.roles);
    for role in &current.team_our.roles {
        if !old.contains_key(&role.id) && is_building(role) {
            events.push(WorldEvent::BuildingChanged(role.id));
        }
    }
    let new = role_index(&current.team_our.roles);
    for role in &previous.team_our.roles {
        if !new.contains_key(&role.id) && is_building(role) {
            events.push(WorldEvent::BuildingChanged(role.id));
        }
    }
}

fn append_mines(previous: &Observation, current: &Observation, events: &mut Vec<WorldEvent>) {
    let old = mine_positions(previous);
    let new = mine_positions(current);
    for pos in new.difference(&old) {
        events.push(WorldEvent::MineAppeared(*pos));
    }
    for pos in old.difference(&new) {
        events.push(WorldEvent::MineDisappeared(*pos));
    }
}

fn mine_positions(observation: &Observation) -> std::collections::BTreeSet<crate::domain::Pos> {
    observation
        .map_info
        .zones
        .iter()
        .filter(|zone| matches!(zone.neutral_type.as_str(), "stone" | "iron" | "copper"))
        .map(|zone| zone.pos)
        .collect()
}

fn append_enemy_visibility(
    previous: &Observation,
    current: &Observation,
    events: &mut Vec<WorldEvent>,
) {
    let old = previous
        .team_enemy
        .as_ref()
        .into_iter()
        .flat_map(|team| &team.roles);
    let visible: std::collections::BTreeSet<_> = current
        .team_enemy
        .as_ref()
        .into_iter()
        .flat_map(|team| &team.roles)
        .map(|role| role.id)
        .collect();
    for role in old {
        if !visible.contains(&role.id) {
            events.push(WorldEvent::EnemyUnobserved(role.id));
        }
    }
}

fn append_phase(previous: &Observation, current: &Observation, events: &mut Vec<WorldEvent>) {
    let new_phase = phase(current.round_no);
    if new_phase != phase(previous.round_no)
        && let Some(value) = new_phase
    {
        events.push(WorldEvent::PhaseChanged(value));
    }
}

fn append_roles(previous: &Observation, current: &Observation, events: &mut Vec<WorldEvent>) {
    let old = role_index(&previous.team_our.roles);
    for role in &current.team_our.roles {
        if let Some(before) = old.get(&role.id) {
            append_role_changes(before, role, events);
        }
    }
}

fn role_index(roles: &[Role]) -> BTreeMap<i64, &Role> {
    roles.iter().map(|role| (role.id, role)).collect()
}

fn append_role_changes(before: &Role, after: &Role, events: &mut Vec<WorldEvent>) {
    append_health_change(before, after, events);
    append_position_change(before, after, events);
    append_building_change(before, after, events);
}

fn append_health_change(before: &Role, after: &Role, events: &mut Vec<WorldEvent>) {
    match (before.health, after.health) {
        (Some(old), Some(new)) if old > NO_HEALTH && new <= NO_HEALTH => {
            events.push(WorldEvent::UnitDied(after.id));
        }
        (Some(old), Some(new)) if old <= NO_HEALTH && new > NO_HEALTH => {
            events.push(WorldEvent::UnitRevived(after.id));
        }
        _ => {}
    }
}

fn append_position_change(before: &Role, after: &Role, events: &mut Vec<WorldEvent>) {
    if before.pos != after.pos {
        events.push(WorldEvent::UnitMoved {
            id: after.id,
            from: before.pos,
            to: after.pos,
        });
    }
    if before.backpack != after.backpack {
        events.push(WorldEvent::InventoryChanged(after.id));
    }
}

fn append_building_change(before: &Role, after: &Role, events: &mut Vec<WorldEvent>) {
    if is_building(after) && (before.health != after.health || before.level != after.level) {
        events.push(WorldEvent::BuildingChanged(after.id));
    }
    if before.cooldown.is_some_and(|value| value > NO_COOLDOWN)
        && after.cooldown == Some(NO_COOLDOWN)
    {
        events.push(WorldEvent::WeaponReady(after.id));
    }
}

fn is_building(role: &Role) -> bool {
    matches!(
        role.role_type.as_str(),
        "station" | "wall" | "gatling" | "railgun" | "rocket"
    )
}

fn append_misc(previous: &Observation, current: &Observation, events: &mut Vec<WorldEvent>) {
    if let (Some(old), Some(new)) = (previous.team_our.gold_num, current.team_our.gold_num)
        && old != new
    {
        events.push(WorldEvent::GoldChanged {
            previous: old,
            current: new,
        });
    }
    if previous.phase_task.is_empty() && !current.phase_task.is_empty() {
        events.push(WorldEvent::ChallengeStarted);
    }
    if !previous.phase_task.is_empty() && current.phase_task.is_empty() {
        events.push(WorldEvent::ChallengeEnded);
    }
    if previous.world_news.official_news != current.world_news.official_news
        || previous.world_news.folk_legends != current.world_news.folk_legends
    {
        events.push(WorldEvent::NewsChanged);
    }
}
