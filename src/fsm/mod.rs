use crate::rules::time::Phase;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldState {
    ColdStart,
    Ready,
    Degraded,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrategyState {
    Bootstrap,
    DayDevelop,
    PrepareNight,
    NightDefend,
    Emergency,
    Endgame,
    Finished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissionState {
    Proposed,
    Ready,
    Assigned,
    Executing,
    Blocked,
    Suspended,
    Succeeded,
    Failed,
    Cancelled,
    Expired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TacticalState {
    Plan,
    Approach,
    Position,
    Execute,
    Evaluate,
    Replan,
    Hold,
    Retreat,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndividualState {
    Idle,
    Moving,
    Interacting,
    OperatingWeapon,
    WaitingResult,
    Recovering,
    Dead,
}

pub fn normal_strategy(phase: Phase, return_due: bool, endgame: bool) -> StrategyState {
    if endgame {
        return StrategyState::Endgame;
    }
    match phase {
        Phase::Night => StrategyState::NightDefend,
        Phase::Day if return_due => StrategyState::PrepareNight,
        Phase::Day => StrategyState::DayDevelop,
    }
}
