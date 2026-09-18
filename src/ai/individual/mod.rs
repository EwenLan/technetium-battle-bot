use crate::domain::{Action, Role};
use crate::fsm::IndividualState;
use crate::rules::constants::NO_HEALTH;

pub fn state_after_commit(role: &Role, action: &Action) -> IndividualState {
    if role.health.unwrap_or(NO_HEALTH) <= NO_HEALTH {
        return IndividualState::Dead;
    }
    match action {
        Action::Move(_) => IndividualState::WaitingResult,
        Action::Attack { .. } => IndividualState::WaitingResult,
        _ => IndividualState::WaitingResult,
    }
}
