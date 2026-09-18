use std::collections::{BTreeMap, BTreeSet};

use crate::domain::{Action, Command, Observation, Response};
use crate::rules::build::BuildMask;
use crate::rules::constants::{NO_HEALTH, ZERO_GOLD};
use crate::rules::time::{Phase, phase};
use crate::world::WorldView;

pub struct Arbiter<'a> {
    view: WorldView<'a>,
    commands: BTreeMap<String, Command>,
    used_roles: BTreeSet<i64>,
    reserved_destinations: BTreeSet<crate::domain::Pos>,
    reserved_build_sites: BTreeSet<crate::domain::Pos>,
    accepted: Vec<(i64, Action)>,
    remaining_gold: Option<i32>,
    build_mask: Option<&'a BuildMask>,
}

impl<'a> Arbiter<'a> {
    pub fn new(observation: &'a Observation, build_mask: Option<&'a BuildMask>) -> Self {
        Self {
            view: WorldView::new(observation),
            commands: BTreeMap::new(),
            used_roles: BTreeSet::new(),
            reserved_destinations: BTreeSet::new(),
            reserved_build_sites: BTreeSet::new(),
            accepted: Vec::new(),
            remaining_gold: observation.team_our.gold_num,
            build_mask,
        }
    }

    pub fn propose(&mut self, actor: i64, action: Action) -> bool {
        if self.used_roles.contains(&actor) || !self.validate(actor, &action) {
            return false;
        }
        let key = actor.to_string();
        self.reserve(actor, &action);
        self.accepted.push((actor, action.clone()));
        self.commands.insert(key, Command::from(action));
        true
    }

    pub fn accepted(&self) -> &[(i64, Action)] {
        &self.accepted
    }

    pub fn finish(self) -> Response {
        Response {
            role_command_map: self.commands,
            ..Response::default()
        }
    }

    fn validate(&self, actor: i64, action: &Action) -> bool {
        let role = self
            .view
            .observation
            .team_our
            .roles
            .iter()
            .find(|role| role.id == actor);
        let Some(role) = role.filter(|role| role.health.is_some_and(|hp| hp > NO_HEALTH)) else {
            return false;
        };
        match action {
            Action::Move(_) | Action::Collect(_) | Action::Build { .. } => {
                self.valid_spatial(role, actor, action)
            }
            Action::Sell { .. } | Action::Use { .. } => self.valid_inventory(role, action),
            Action::Attack {
                controller,
                targets,
            } => self.valid_attack(actor, *controller, targets),
            Action::AcceptTask => self.valid_accept(role),
            _ => false,
        }
    }

    fn valid_spatial(&self, role: &crate::domain::Role, actor: i64, action: &Action) -> bool {
        match action {
            Action::Move(pos) => self.valid_move(role.pos, *pos, actor),
            Action::Collect(pos) => role.role_type == "worker" && self.valid_mine(role.pos, *pos),
            Action::Build { name, pos } => self.valid_build(role, name, *pos),
            _ => false,
        }
    }

    fn valid_inventory(&self, role: &crate::domain::Role, action: &Action) -> bool {
        match action {
            Action::Sell { name, quantity } => self.valid_sell(role, name, *quantity),
            Action::Use { name, pos } => self.valid_use(role, name, *pos),
            _ => false,
        }
    }

    fn valid_move(&self, from: crate::domain::Pos, to: crate::domain::Pos, actor: i64) -> bool {
        self.view.adjacent(from, to)
            && from != to
            && self.view.can_enter(to, actor)
            && !self.reserved_destinations.contains(&to)
            && !self.reserved_build_sites.contains(&to)
    }

    fn valid_interact(&self, from: crate::domain::Pos, to: crate::domain::Pos) -> bool {
        self.view.adjacent(from, to) && self.view.zone_at(to).is_some()
    }

    fn valid_mine(&self, from: crate::domain::Pos, to: crate::domain::Pos) -> bool {
        self.valid_interact(from, to)
            && self.view.zone_at(to).is_some_and(|zone| {
                matches!(zone.neutral_type.as_str(), "stone" | "iron" | "copper")
            })
    }

    fn valid_sell(&self, role: &crate::domain::Role, name: &str, quantity: i32) -> bool {
        let count = role
            .backpack
            .iter()
            .filter(|item| item.as_str() == name)
            .count();
        let shop_adjacent =
            self.view.observation.map_info.zones.iter().any(|zone| {
                zone.neutral_type == "vendor" && self.view.adjacent(role.pos, zone.pos)
            });
        let listed = self
            .view
            .observation
            .vendor_shop_list
            .iter()
            .any(|item| item.name == name);
        quantity > ZERO_GOLD
            && count >= usize::try_from(quantity).unwrap_or(usize::MAX)
            && shop_adjacent
            && listed
    }

    fn valid_accept(&self, role: &crate::domain::Role) -> bool {
        let task_available = self
            .view
            .observation
            .team_our
            .player_tasks
            .iter()
            .any(|task| {
                task.is_valid == Some(true)
                    && task.cold_down_rounds == Some(crate::rules::constants::NO_COOLDOWN)
                    && self.view.adjacent(role.pos, task.task_position)
            });
        role.role_type == "pioneer" && self.view.observation.phase_task.is_empty() && task_available
    }

    fn valid_build(&self, role: &crate::domain::Role, name: &str, pos: crate::domain::Pos) -> bool {
        let mask_allows = self
            .build_mask
            .is_some_and(|mask| mask.allows_weapon(&self.view.observation.team_our.faction, pos));
        let existing_weapons = self
            .view
            .observation
            .team_our
            .roles
            .iter()
            .filter(|building| {
                crate::ai::strategy::is_weapon(&building.role_type)
                    && building.health.is_some_and(|hp| hp > NO_HEALTH)
            })
            .count();
        role.role_type == "worker"
            && phase(self.view.observation.round_no) == Some(Phase::Day)
            && crate::ai::strategy::is_weapon(name)
            && existing_weapons + self.reserved_build_sites.len()
                < crate::rules::constants::MAX_WEAPONS
            && self
                .remaining_gold
                .is_some_and(|gold| gold >= crate::rules::constants::WEAPON_BUILD_GOLD)
            && self.view.adjacent(role.pos, pos)
            && role.pos != pos
            && self.view.can_enter(pos, role.id)
            && !self.reserved_build_sites.contains(&pos)
            && !self.reserved_destinations.contains(&pos)
            && mask_allows
    }

    fn valid_attack(&self, weapon: i64, controller: i64, targets: &[crate::domain::Pos]) -> bool {
        let observation = self.view.observation;
        let weapon_role = observation
            .team_our
            .roles
            .iter()
            .find(|role| role.id == weapon);
        let controller_role = observation
            .team_our
            .roles
            .iter()
            .find(|role| role.id == controller);
        let Some((weapon_role, controller_role)) = weapon_role.zip(controller_role) else {
            return false;
        };
        let weapon_kind = matches!(
            weapon_role.role_type.as_str(),
            "gatling" | "railgun" | "rocket"
        );
        let targets_valid = targets.iter().all(|pos| {
            self.view.in_bounds(*pos)
                && crate::rules::geometry::distance(weapon_role.pos, *pos)
                    <= weapon_role.attack_range.unwrap_or(NO_HEALTH)
        });
        phase(observation.round_no) == Some(Phase::Night)
            && weapon_kind
            && !self.used_roles.contains(&controller)
            && self.view.adjacent(controller_role.pos, weapon_role.pos)
            && weapon_role.cooldown == Some(crate::rules::constants::NO_COOLDOWN)
            && controller_role.health.is_some_and(|hp| hp > NO_HEALTH)
            && targets_valid
            && valid_target_count(weapon_role, targets)
            && valid_fire_geometry(weapon_role, targets)
    }

    fn valid_use(
        &self,
        role: &crate::domain::Role,
        name: &str,
        pos: Option<crate::domain::Pos>,
    ) -> bool {
        name == "Medicine" && pos.is_none() && role.backpack.iter().any(|item| item == name)
    }

    fn reserve(&mut self, actor: i64, action: &Action) {
        self.used_roles.insert(actor);
        if let Action::Move(pos) = action {
            self.reserved_destinations.insert(*pos);
        }
        if let Action::Build { pos, .. } = action {
            self.reserved_build_sites.insert(*pos);
            self.remaining_gold = self
                .remaining_gold
                .map(|gold| gold.saturating_sub(crate::rules::constants::WEAPON_BUILD_GOLD));
        }
        if let Action::Attack { controller, .. } = action {
            self.used_roles.insert(*controller);
        }
    }
}

fn valid_target_count(weapon: &crate::domain::Role, targets: &[crate::domain::Pos]) -> bool {
    let Some(level) = weapon.level else {
        return false;
    };
    let required = if weapon.role_type == "railgun" {
        crate::rules::constants::SINGLE_ITEM
    } else {
        usize::try_from(level).unwrap_or(usize::MAX)
    };
    targets.len() == required
}

fn valid_fire_geometry(weapon: &crate::domain::Role, targets: &[crate::domain::Pos]) -> bool {
    if targets.contains(&weapon.pos) {
        return false;
    }
    if weapon.role_type != "gatling" {
        return true;
    }
    let distinct = targets.iter().copied().collect::<BTreeSet<_>>().len() == targets.len();
    distinct
        && targets.iter().enumerate().all(|(index, first)| {
            targets
                .iter()
                .skip(index + crate::rules::constants::SINGLE_ITEM)
                .all(|second| cone_pair(weapon.pos, *first, *second))
        })
}

fn cone_pair(
    origin: crate::domain::Pos,
    first: crate::domain::Pos,
    second: crate::domain::Pos,
) -> bool {
    let ax = i64::from(first.x) - i64::from(origin.x);
    let ay = i64::from(first.y) - i64::from(origin.y);
    let bx = i64::from(second.x) - i64::from(origin.x);
    let by = i64::from(second.y) - i64::from(origin.y);
    ax * bx + ay * by >= i64::from(NO_HEALTH)
}
