use crate::domain::{Observation, Pos, Role, Zone};
use crate::rules::constants::{MIN_COORDINATE, NEIGHBOR_RANGE, NO_ACTOR_ID, NO_HEALTH};
use crate::rules::geometry::distance;

pub struct WorldView<'a> {
    pub observation: &'a Observation,
}

impl<'a> WorldView<'a> {
    pub fn new(observation: &'a Observation) -> Self {
        Self { observation }
    }

    pub fn in_bounds(&self, pos: Pos) -> bool {
        pos.x >= MIN_COORDINATE
            && pos.y >= MIN_COORDINATE
            && pos.x < self.observation.map_info.width
            && pos.y < self.observation.map_info.height
    }

    pub fn zone_at(&self, pos: Pos) -> Option<&Zone> {
        self.observation
            .map_info
            .zones
            .iter()
            .find(|zone| zone.pos == pos)
    }

    pub fn can_enter(&self, pos: Pos, actor: i64) -> bool {
        self.in_bounds(pos)
            && self.zone_at(pos).is_none()
            && !self
                .observation
                .team_our
                .roles
                .iter()
                .any(|role| occupies(role, pos, actor))
            && !self.enemy_at(pos)
            && !self.robot_at(pos)
    }

    pub fn interact_positions(&self, target: Pos, actor: i64) -> Vec<Pos> {
        crate::rules::geometry::neighbors(target)
            .filter(|pos| self.can_enter(*pos, actor) || self.actor_at(*pos, actor))
            .collect()
    }

    pub fn actor_at(&self, pos: Pos, actor: i64) -> bool {
        self.observation
            .team_our
            .roles
            .iter()
            .any(|role| role.id == actor && role.pos == pos)
    }

    pub fn adjacent(&self, from: Pos, target: Pos) -> bool {
        distance(from, target) <= NEIGHBOR_RANGE
    }

    fn enemy_at(&self, pos: Pos) -> bool {
        self.observation.team_enemy.as_ref().is_some_and(|enemy| {
            enemy
                .roles
                .iter()
                .any(|role| occupies(role, pos, NO_ACTOR_ID))
        })
    }

    fn robot_at(&self, pos: Pos) -> bool {
        self.observation.robot.as_ref().is_some_and(|robots| {
            robots
                .roles
                .iter()
                .any(|role| role.pos == pos && role.health.unwrap_or(NO_HEALTH) > NO_HEALTH)
        })
    }
}

fn occupies(role: &Role, pos: Pos, actor: i64) -> bool {
    if role.id == actor || role.health == Some(NO_HEALTH) {
        return false;
    }
    let footprint = role.role_type == "station"
        && (pos.x == role.pos.x || pos.x == role.pos.x + NEIGHBOR_RANGE)
        && (pos.y == role.pos.y || pos.y == role.pos.y - NEIGHBOR_RANGE);
    footprint || role.pos == pos
}
