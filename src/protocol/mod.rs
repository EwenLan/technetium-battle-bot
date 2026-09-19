mod unique;

use unique::NoDuplicates;

use crate::domain::{Observation, Response};
use crate::rules::constants::{MAP_MIN_SIZE, MATCH_ROUNDS, MAX_HTTP_BODY_BYTES, MAX_MAP_CELLS};

#[derive(Debug)]
pub enum DecodeError {
    InvalidJson,
    InvalidWorld,
    TooLarge,
}

pub struct DecodedRequest {
    pub observation: Observation,
    pub canonical_key: String,
}

pub fn decode(body: &[u8]) -> Result<DecodedRequest, DecodeError> {
    if body.len() > MAX_HTTP_BODY_BYTES {
        return Err(DecodeError::TooLarge);
    }
    let NoDuplicates(value): NoDuplicates =
        serde_json::from_slice(body).map_err(|_| DecodeError::InvalidJson)?;
    let canonical_key = serde_json::to_string(&value).map_err(|_| DecodeError::InvalidJson)?;
    let observation: Observation =
        serde_json::from_value(value).map_err(|_| DecodeError::InvalidJson)?;
    validate(&observation)?;
    Ok(DecodedRequest {
        observation,
        canonical_key,
    })
}

fn validate(observation: &Observation) -> Result<(), DecodeError> {
    let map = &observation.map_info;
    let cells = map.width.checked_mul(map.height);
    let map_valid = map.width >= MAP_MIN_SIZE && map.height >= MAP_MIN_SIZE;
    let round_valid = (MAP_MIN_SIZE..=MATCH_ROUNDS).contains(&observation.round_no);
    let roles_valid = !observation.team_our.roles.is_empty();
    let team_valid = !observation.team_our.team_id.is_empty();
    let positions_valid = observation_positions_valid(observation);
    let mut ids = std::collections::BTreeSet::new();
    let ids_unique = observation
        .team_our
        .roles
        .iter()
        .all(|role| ids.insert(role.id));
    match cells {
        Some(total)
            if map_valid
                && total <= MAX_MAP_CELLS
                && round_valid
                && roles_valid
                && team_valid
                && positions_valid
                && ids_unique =>
        {
            Ok(())
        }
        _ => Err(DecodeError::InvalidWorld),
    }
}

fn observation_positions_valid(observation: &Observation) -> bool {
    let map = &observation.map_info;
    roles_in_map(&observation.team_our.roles, map.width, map.height)
        && observation
            .team_enemy
            .as_ref()
            .is_none_or(|team| roles_in_map(&team.roles, map.width, map.height))
        && observation
            .robot
            .as_ref()
            .is_none_or(|team| roles_in_map(&team.roles, map.width, map.height))
        && observation
            .team_our
            .player_tasks
            .iter()
            .all(|task| in_map(task.task_position, map.width, map.height))
        && map
            .zones
            .iter()
            .all(|zone| in_map(zone.pos, map.width, map.height))
}

fn roles_in_map(roles: &[crate::domain::Role], width: i32, height: i32) -> bool {
    roles.iter().all(|role| {
        in_map(role.pos, width, height)
            && (role.role_type != "station" || station_in_map(role.pos, width, height))
    })
}

fn station_in_map(pos: crate::domain::Pos, width: i32, height: i32) -> bool {
    let Some(right) = pos.x.checked_add(crate::rules::constants::NEIGHBOR_RANGE) else {
        return false;
    };
    let Some(bottom) = pos.y.checked_sub(crate::rules::constants::NEIGHBOR_RANGE) else {
        return false;
    };
    in_map(
        crate::domain::Pos {
            x: right,
            y: bottom,
        },
        width,
        height,
    )
}

fn in_map(pos: crate::domain::Pos, width: i32, height: i32) -> bool {
    pos.x >= crate::rules::constants::MIN_COORDINATE
        && pos.y >= crate::rules::constants::MIN_COORDINATE
        && pos.x < width
        && pos.y < height
}

pub fn encode(response: &Response) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(response)
}

pub fn empty_response() -> Vec<u8> {
    encode(&Response::default()).unwrap_or_default()
}
