use crate::ai::cognition;
use crate::ai::mission::economy::{pioneer_max_hp, should_heal};
use crate::ai::tactics::next_step;
use crate::ai::{DecisionState, propose_owned};
use crate::command::Arbiter;
use crate::domain::{Action, Observation, OwnerError, PlayerTask, Role};
use crate::rules::constants::{
    ANSWER_MARGIN_ROUNDS, CHALLENGE_SETUP_ROUNDS, NO_COOLDOWN, NO_HEALTH,
};
use crate::world::WorldView;

pub fn assign(
    observation: &Observation,
    state: &mut DecisionState,
    arbiter: &mut Arbiter<'_>,
) -> Result<(), OwnerError> {
    let Some(pioneer) = observation
        .team_our
        .roles
        .iter()
        .find(|role| role.role_type == "pioneer")
    else {
        return Ok(());
    };
    if pioneer.health.unwrap_or(NO_HEALTH) <= NO_HEALTH {
        return Ok(());
    }
    let action = choose_action(observation, pioneer, state);
    if let Some(action) = action {
        let submitted = matches!(action, Action::SubmitAnswer(_));
        if propose_owned(state, arbiter, pioneer.id, action)? && submitted {
            cognition::record_submission(observation, &mut state.challenge);
        }
    }
    Ok(())
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
    if view.adjacent_task(pioneer.pos, task.task_position) {
        return Some(Action::AcceptTask);
    }
    let stands = view.task_stands(task.task_position, pioneer.id);
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
            let stands = view.task_stands(task.task_position, pioneer.id);
            let path = next_step(view, pioneer.id, pioneer.pos, &stands)?;
            let needed = CHALLENGE_SETUP_ROUNDS + ANSWER_MARGIN_ROUNDS + path.steps;
            task.timeout_rounds
                .filter(|limit| *limit >= needed)
                .map(|_| (task, path.steps))
        })
        .min_by_key(|(task, steps)| (*steps, task.task_position.y, task.task_position.x))
        .map(|(task, _)| task)
}
