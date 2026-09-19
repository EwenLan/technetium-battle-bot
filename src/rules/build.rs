use crate::domain::Pos;
use crate::rules::constants::{MIN_COORDINATE, NEIGHBOR_RANGE};

const WEAPON_MIN_X_OFFSET: i32 = -1;
const WEAPON_MAX_X_OFFSET: i32 = 2;
const WEAPON_MIN_Y_OFFSET: i32 = -2;
const WEAPON_MAX_Y_OFFSET: i32 = 1;
const WALL_MIN_X_OFFSET: i32 = -2;
const WALL_MAX_X_OFFSET: i32 = 3;
const WALL_MIN_Y_OFFSET: i32 = -3;
const WALL_MAX_Y_OFFSET: i32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildArea {
    Station,
    Weapon,
    Wall,
    Outside,
}

pub fn area(station: Pos, pos: Pos) -> BuildArea {
    let dx = pos.x - station.x;
    let dy = pos.y - station.y;
    if in_rect(
        dx,
        dy,
        MIN_COORDINATE,
        NEIGHBOR_RANGE,
        -NEIGHBOR_RANGE,
        MIN_COORDINATE,
    ) {
        return BuildArea::Station;
    }
    if in_rect(
        dx,
        dy,
        WEAPON_MIN_X_OFFSET,
        WEAPON_MAX_X_OFFSET,
        WEAPON_MIN_Y_OFFSET,
        WEAPON_MAX_Y_OFFSET,
    ) {
        return BuildArea::Weapon;
    }
    if in_rect(
        dx,
        dy,
        WALL_MIN_X_OFFSET,
        WALL_MAX_X_OFFSET,
        WALL_MIN_Y_OFFSET,
        WALL_MAX_Y_OFFSET,
    ) {
        return BuildArea::Wall;
    }
    BuildArea::Outside
}

pub fn weapon_sites(station: Pos, width: i32, height: i32) -> Vec<Pos> {
    ring_sites(station, width, height, BuildArea::Weapon)
}

pub fn wall_sites(station: Pos, width: i32, height: i32) -> Vec<Pos> {
    ring_sites(station, width, height, BuildArea::Wall)
}

fn ring_sites(station: Pos, width: i32, height: i32, expected: BuildArea) -> Vec<Pos> {
    let mut sites = Vec::new();
    for x in MIN_COORDINATE..width {
        for y in MIN_COORDINATE..height {
            let pos = Pos { x, y };
            if area(station, pos) == expected {
                sites.push(pos);
            }
        }
    }
    sites
}

fn in_rect(dx: i32, dy: i32, min_x: i32, max_x: i32, min_y: i32, max_y: i32) -> bool {
    (min_x..=max_x).contains(&dx) && (min_y..=max_y).contains(&dy)
}
