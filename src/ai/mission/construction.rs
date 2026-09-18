use crate::ai::DecisionState;
use crate::ai::tactics::next_step;
use crate::domain::{Action, Observation, Pos, Role};
use crate::rules::build::BuildMask;
use crate::rules::constants::{
    ACTION_RESULT_GRACE_ROUNDS, MAX_WEAPONS, NEIGHBOR_RANGE, NO_HEALTH, WEAPON_BUILD_GOLD,
};
use crate::rules::geometry::{distance, station_distance};
use crate::world::WorldView;

const OPENING_LOADOUT: [&str; MAX_WEAPONS] = ["gatling", "railgun", "rocket"];
const NEW_BUILD_PLAN_COUNT: i32 = 1;

#[derive(Clone, Debug)]
pub struct BuildPlan {
    pub kind: &'static str,
    pub site: Pos,
    pub sent_round: Option<i32>,
}

pub fn refresh(observation: &Observation, state: &mut DecisionState) {
    let finished: Vec<(i64, Pos, bool)> = state
        .build_plans
        .iter()
        .filter_map(|(worker, plan)| plan_finished(observation, *worker, plan))
        .collect();
    for (worker, site, rejected) in finished {
        state.build_plans.remove(&worker);
        if rejected {
            state.bad_build_sites.insert(site);
        }
    }
}

fn plan_finished(
    observation: &Observation,
    worker: i64,
    plan: &BuildPlan,
) -> Option<(i64, Pos, bool)> {
    let actor_alive = observation
        .team_our
        .roles
        .iter()
        .any(|role| role.id == worker && role.health.is_some_and(|hp| hp > NO_HEALTH));
    if !actor_alive {
        return Some((worker, plan.site, false));
    }
    let built = observation.team_our.roles.iter().any(|role| {
        role.pos == plan.site
            && role.role_type == plan.kind
            && role.health.is_some_and(|hp| hp > NO_HEALTH)
    });
    if built {
        return Some((worker, plan.site, false));
    }
    let site_taken = observation.team_our.roles.iter().any(|role| {
        role.pos == plan.site && role.id != worker && role.health.is_some_and(|hp| hp > NO_HEALTH)
    });
    if site_taken {
        return Some((worker, plan.site, true));
    }
    let sent = plan.sent_round?;
    let feedback = observation
        .last_round_role_action_results
        .get(&worker.to_string());
    let expired = observation.round_no - sent > ACTION_RESULT_GRACE_ROUNDS;
    if feedback == Some(&false) || expired {
        return Some((worker, plan.site, true));
    }
    None
}

pub fn worker_action(
    observation: &Observation,
    worker: &Role,
    state: &mut DecisionState,
    mask: Option<&BuildMask>,
) -> Option<Action> {
    if worker.health.unwrap_or(NO_HEALTH) <= NO_HEALTH {
        return None;
    }
    let mask = mask?;
    if !state.build_plans.contains_key(&worker.id) {
        let plan = choose_plan(observation, worker, state, mask)?;
        state.build_plans.insert(worker.id, plan);
    }
    let plan = state.build_plans.get(&worker.id)?;
    if plan.sent_round.is_some() {
        return None;
    }
    if observation.team_our.gold_num.unwrap_or(NO_HEALTH) < WEAPON_BUILD_GOLD {
        return None;
    }
    plan_action(observation, worker, plan)
}

fn choose_plan(
    observation: &Observation,
    worker: &Role,
    state: &DecisionState,
    mask: &BuildMask,
) -> Option<BuildPlan> {
    let pending = i32::try_from(state.build_plans.len()).ok()?;
    let required = pending
        .saturating_add(NEW_BUILD_PLAN_COUNT)
        .saturating_mul(WEAPON_BUILD_GOLD);
    if observation.team_our.gold_num? < required {
        return None;
    }
    let existing = observation
        .team_our
        .roles
        .iter()
        .filter(|role| {
            crate::ai::strategy::is_weapon(&role.role_type)
                && role.health.is_some_and(|hp| hp > NO_HEALTH)
        })
        .count();
    if existing + state.build_plans.len() >= MAX_WEAPONS {
        return None;
    }
    let kind = OPENING_LOADOUT.into_iter().find(|kind| {
        !observation
            .team_our
            .roles
            .iter()
            .any(|role| role.role_type == *kind)
            && !state.build_plans.values().any(|plan| plan.kind == *kind)
    })?;
    let site = choose_site(observation, worker, state, mask)?;
    Some(BuildPlan {
        kind,
        site,
        sent_round: None,
    })
}

fn choose_site(
    observation: &Observation,
    worker: &Role,
    state: &DecisionState,
    mask: &BuildMask,
) -> Option<Pos> {
    let view = WorldView::new(observation);
    let base = observation
        .team_our
        .roles
        .iter()
        .find(|role| role.role_type == "station")?;
    mask.weapon_sites(&observation.team_our.faction)
        .filter(|site| {
            *site != worker.pos
                && view.can_enter(*site, worker.id)
                && !state.bad_build_sites.contains(site)
        })
        .filter(|site| !state.build_plans.values().any(|plan| plan.site == *site))
        .filter_map(|site| {
            let stands = view.interact_positions(site, worker.id);
            let path = next_step(&view, worker.id, worker.pos, &stands)?;
            Some((site, path.steps))
        })
        .min_by_key(|(site, steps)| (station_distance(base.pos, *site), *steps, site.y, site.x))
        .map(|(site, _)| site)
}

fn plan_action(observation: &Observation, worker: &Role, plan: &BuildPlan) -> Option<Action> {
    let view = WorldView::new(observation);
    if distance(worker.pos, plan.site) == NEIGHBOR_RANGE {
        return Some(Action::Build {
            name: plan.kind.to_owned(),
            pos: plan.site,
        });
    }
    let stands = view.interact_positions(plan.site, worker.id);
    Some(Action::Move(
        next_step(&view, worker.id, worker.pos, &stands)?.next,
    ))
}

pub fn mark_sent(state: &mut DecisionState, worker: i64, round: i32) {
    if let Some(plan) = state.build_plans.get_mut(&worker) {
        plan.sent_round = Some(round);
    }
}
