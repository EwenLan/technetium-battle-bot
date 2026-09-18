use crate::domain::{Action, Observation};

#[derive(Clone, Debug, Default)]
pub struct ChallengeMemory {
    task_text: String,
    prompt_round: Option<i32>,
    submitted_answer: String,
}

pub fn prepare_prompt(observation: &Observation, memory: &mut ChallengeMemory) -> String {
    if observation.phase_task.is_empty() {
        *memory = ChallengeMemory::default();
        return String::new();
    }
    if memory.task_text != observation.phase_task {
        memory.task_text = observation.phase_task.clone();
        memory.prompt_round = None;
        memory.submitted_answer.clear();
    }
    if memory.prompt_round.is_some() {
        return String::new();
    }
    memory.prompt_round = Some(observation.round_no);
    format!(
        "Solve the competition task. Return only the answer text, preserving its required format. Task:\n{}",
        observation.phase_task
    )
}

pub fn answer_action(observation: &Observation, memory: &ChallengeMemory) -> Option<Action> {
    let expected_round = memory
        .prompt_round?
        .checked_add(crate::rules::constants::MIN_ACTION_ROUNDS)?;
    if observation.round_no != expected_round {
        return None;
    }
    let answer = observation.llm_resp.trim();
    if answer.is_empty() || answer == memory.submitted_answer {
        return None;
    }
    if memory.task_text != observation.phase_task {
        return None;
    }
    Some(Action::SubmitAnswer(answer.to_owned()))
}

pub fn record_submission(observation: &Observation, memory: &mut ChallengeMemory) {
    memory.submitted_answer = observation.llm_resp.trim().to_owned();
}
