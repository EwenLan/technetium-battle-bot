use crate::ai::DecisionState;
use crate::ai::cognition;
use crate::ai::mission::economy::{pioneer_max_hp, should_heal};
use crate::ai::tactics::next_step;
use crate::command::Arbiter;
use crate::domain::{Action, Observation, PlayerTask, Role};
use crate::rules::constants::{
    ANSWER_MARGIN_ROUNDS, CHALLENGE_SETUP_ROUNDS, NO_COOLDOWN, NO_HEALTH,
};
use crate::world::WorldView;

pub fn assign(observation: &Observation, state: &mut DecisionState, arbiter: &mut Arbiter<'_>) {
    let Some(pioneer) = observation
        .team_our
        .roles
        .iter()
        .find(|role| role.role_type == "pioneer")
    else {
        return;
    };
    if pioneer.health.unwrap_or(NO_HEALTH) <= NO_HEALTH {
        return;
    }
    let action = choose_action(observation, pioneer, state);
    if let Some(action) = action {
        let submitted = matches!(action, Action::SubmitAnswer(_));
        if arbiter.propose(pioneer.id, action) && submitted {
            cognition::record_submission(observation, &mut state.challenge);
        }
    }
}

fn choose_action(
    observation: &Observation,
    pioneer: &Role,
    state: &DecisionState,
) -> Option<Action> {
    if should_heal(pioneer, pioneer_max_hp()) {
        return Some(Action::Use {
            name: "Medicine".into(),
            pos: None,
        });
    }
    if !observation.phase_task.is_empty() {
        return cognition::answer_action(observation, &state.challenge);
    }
    let view = WorldView::new(observation);
    let task = choose_task(observation, pioneer, &view)?;
    if view.adjacent(pioneer.pos, task.task_position) {
        return Some(Action::AcceptTask);
    }
    let stands = view.interact_positions(task.task_position, pioneer.id);
    Some(Action::Move(
        next_step(&view, pioneer.id, pioneer.pos, &stands)?.next,
    ))
}

fn choose_task<'a>(
    observation: &'a Observation,
    pioneer: &Role,
    view: &WorldView<'_>,
) -> Option<&'a PlayerTask> {
    observation
        .team_our
        .player_tasks
        .iter()
        .filter(|task| task.is_valid == Some(true) && task.cold_down_rounds == Some(NO_COOLDOWN))
        .filter_map(|task| {
            let stands = view.interact_positions(task.task_position, pioneer.id);
            let path = next_step(view, pioneer.id, pioneer.pos, &stands)?;
            let needed = CHALLENGE_SETUP_ROUNDS + ANSWER_MARGIN_ROUNDS + path.steps;
            task.timeout_rounds
                .filter(|limit| *limit >= needed)
                .map(|_| (task, path.steps))
        })
        .min_by_key(|(task, steps)| (*steps, task.task_position.y, task.task_position.x))
        .map(|(task, _)| task)
}
