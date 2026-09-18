use crate::domain::Pos;
use crate::rules::constants::NEIGHBOR_RANGE;

pub const DIRECTIONS: [(i32, i32); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];

pub fn distance(from: Pos, to: Pos) -> i32 {
    (from.x - to.x).abs().max((from.y - to.y).abs())
}

pub fn station_distance(station: Pos, target: Pos) -> i32 {
    let nearest_x = target.x.clamp(station.x, station.x + NEIGHBOR_RANGE);
    let nearest_y = target.y.clamp(station.y - NEIGHBOR_RANGE, station.y);
    distance(
        target,
        Pos {
            x: nearest_x,
            y: nearest_y,
        },
    )
}

pub fn neighbors(pos: Pos) -> impl Iterator<Item = Pos> {
    DIRECTIONS.into_iter().filter_map(move |(dx, dy)| {
        Some(Pos {
            x: pos.x.checked_add(dx)?,
            y: pos.y.checked_add(dy)?,
        })
    })
}
