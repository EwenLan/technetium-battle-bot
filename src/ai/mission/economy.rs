use crate::ai::tactics::next_step;
use crate::domain::{Action, Observation, Pos, Role, Zone};
use crate::rules::constants::{
    ECONOMY_SETUP_ROUNDS, GATHER_BATCH_MAX, MEDICINE_HEAL_THRESHOLD, MIN_ACTION_ROUNDS,
    NEIGHBOR_RANGE, NO_HEALTH, PIONEER_MAX_HP, POSITION_SETUP_ROUNDS, RETURN_MARGIN_ROUNDS,
    SCORE_SCALE, WORKER_MAX_HP,
};
use crate::rules::geometry::distance;
use crate::rules::time::next_night;
use crate::world::WorldView;

struct MineChoice<'a> {
    zone: &'a Zone,
    gain: i32,
    rounds: i32,
}

pub fn worker_action(observation: &Observation, worker: &Role) -> Option<Action> {
    if worker.health.unwrap_or(NO_HEALTH) <= NO_HEALTH {
        return None;
    }
    if should_heal(worker, WORKER_MAX_HP) {
        return Some(Action::Use {
            name: "Medicine".into(),
            pos: None,
        });
    }
    let view = WorldView::new(observation);
    let ore_count = worker.backpack.iter().filter(|item| is_ore(item)).count();
    let capacity = worker.back_pack_capability?;
    if ore_count >= GATHER_BATCH_MAX || worker.backpack.len() >= capacity {
        return sell_action(observation, worker, &view);
    }
    mine_action(observation, worker, &view)
}

pub fn should_heal(role: &Role, max_hp: i32) -> bool {
    let Some(hp) = role.health else { return false };
    let has_medicine = role.backpack.iter().any(|item| item == "Medicine");
    has_medicine && hp * SCORE_SCALE < max_hp * MEDICINE_HEAL_THRESHOLD
}

pub fn pioneer_max_hp() -> i32 {
    PIONEER_MAX_HP
}

fn sell_action(observation: &Observation, worker: &Role, view: &WorldView<'_>) -> Option<Action> {
    let vendor = nearest_zone(observation, worker, view, |zone| {
        zone.neutral_type == "vendor"
    })?;
    if view.adjacent(worker.pos, vendor.pos) {
        return best_sale(observation, worker);
    }
    move_to(worker, vendor.pos, view)
}

fn best_sale(observation: &Observation, worker: &Role) -> Option<Action> {
    observation
        .vendor_shop_list
        .iter()
        .filter(|entry| is_ore(&entry.name))
        .filter_map(|entry| {
            let count = worker
                .backpack
                .iter()
                .filter(|item| *item == &entry.name)
                .count();
            i32::try_from(count)
                .ok()
                .filter(|quantity| *quantity > NO_HEALTH)
                .map(|quantity| (entry, quantity))
        })
        .max_by_key(|(entry, quantity)| (entry.price.saturating_mul(*quantity), &entry.name))
        .map(|(entry, quantity)| Action::Sell {
            name: entry.name.clone(),
            quantity,
        })
}

fn mine_action(observation: &Observation, worker: &Role, view: &WorldView<'_>) -> Option<Action> {
    let free = worker
        .back_pack_capability?
        .saturating_sub(worker.backpack.len());
    let quantity = i32::try_from(free.min(GATHER_BATCH_MAX)).ok()?;
    let mine = observation
        .map_info
        .zones
        .iter()
        .filter(|zone| is_ore(&zone.neutral_type))
        .filter_map(|zone| mine_value(observation, worker, view, zone, quantity))
        .max_by(compare_mines)?
        .zone;
    if view.adjacent(worker.pos, mine.pos) {
        return Some(Action::Collect(mine.pos));
    }
    move_to(worker, mine.pos, view)
}

fn mine_value<'a>(
    observation: &Observation,
    worker: &Role,
    view: &WorldView<'_>,
    mine: &'a Zone,
    quantity: i32,
) -> Option<MineChoice<'a>> {
    let price = observation
        .vendor_shop_list
        .iter()
        .find(|item| item.name == mine.neutral_type && item.price > NO_HEALTH)?
        .price;
    let mine_stands = view.interact_positions(mine.pos, worker.id);
    let mine_path = next_step(view, worker.id, worker.pos, &mine_stands)?;
    let (vendor_stand, market_steps) = market_route(observation, view, worker.id, mine_path.goal)?;
    let rounds =
        mine_path.steps + quantity + market_steps + MIN_ACTION_ROUNDS + ECONOMY_SETUP_ROUNDS;
    if !fits_return(observation, vendor_stand, rounds) {
        return None;
    }
    Some(MineChoice {
        zone: mine,
        gain: price.saturating_mul(quantity),
        rounds,
    })
}

fn market_route(
    observation: &Observation,
    view: &WorldView<'_>,
    actor: i64,
    from: Pos,
) -> Option<(Pos, i32)> {
    observation
        .map_info
        .zones
        .iter()
        .filter(|zone| zone.neutral_type == "vendor")
        .filter_map(|zone| {
            let stands = view.interact_positions(zone.pos, actor);
            let path = next_step(view, actor, from, &stands)?;
            Some((path.goal, path.steps))
        })
        .min_by_key(|(stand, steps)| (*steps, stand.y, stand.x))
}

fn fits_return(observation: &Observation, from: Pos, work_rounds: i32) -> bool {
    let nearest = observation
        .team_our
        .roles
        .iter()
        .filter(|role| {
            crate::ai::strategy::is_weapon(&role.role_type)
                && role.health.is_some_and(|hp| hp > NO_HEALTH)
        })
        .map(|weapon| distance(from, weapon.pos).saturating_sub(NEIGHBOR_RANGE))
        .min();
    let Some((night, travel)) = next_night(observation.round_no).zip(nearest) else {
        return true;
    };
    observation.round_no + work_rounds + travel + POSITION_SETUP_ROUNDS + RETURN_MARGIN_ROUNDS
        <= night
}

fn compare_mines(left: &MineChoice<'_>, right: &MineChoice<'_>) -> std::cmp::Ordering {
    let value = i64::from(left.gain) * i64::from(right.rounds);
    let other = i64::from(right.gain) * i64::from(left.rounds);
    value
        .cmp(&other)
        .then_with(|| right.rounds.cmp(&left.rounds))
        .then_with(|| right.zone.pos.cmp(&left.zone.pos))
}

fn nearest_zone<'a>(
    observation: &'a Observation,
    worker: &Role,
    view: &WorldView<'_>,
    predicate: impl Fn(&Zone) -> bool,
) -> Option<&'a Zone> {
    observation
        .map_info
        .zones
        .iter()
        .filter(|zone| predicate(zone))
        .filter_map(|zone| {
            let stands = view.interact_positions(zone.pos, worker.id);
            let path = next_step(view, worker.id, worker.pos, &stands)?;
            Some((zone, path.steps))
        })
        .min_by_key(|(zone, steps)| (*steps, zone.pos.y, zone.pos.x))
        .map(|(zone, _)| zone)
}

fn move_to(worker: &Role, target: Pos, view: &WorldView<'_>) -> Option<Action> {
    let stands = view.interact_positions(target, worker.id);
    let path = next_step(view, worker.id, worker.pos, &stands)?;
    if path.steps < MIN_ACTION_ROUNDS {
        return None;
    }
    Some(Action::Move(path.next))
}

fn is_ore(item: &str) -> bool {
    matches!(item, "stone" | "iron" | "copper")
}
