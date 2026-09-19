mod log;

pub use log::{
    AcknowledgeError, DeliveryBatch, DeliveryReceipt, EventCursor, EventInbox, EventLog,
    EventRecord, ReaderId,
};

use crate::domain::{IntentId, MissionId, OwnerPath, PlanId, Pos, Versioned};
use crate::rules::time::Phase;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorldEvent {
    WorldReady,
    WorldDegraded(WorldIssue),
    WorldRecovered,
    RoundStarted(i32),
    PhaseChanged(Phase),
    UnitDied(i64),
    UnitRevived(i64),
    UnitMoved { id: i64, from: Pos, to: Pos },
    EnemyUnobserved(i64),
    EnemyRemoved(i64),
    MineAppeared(Pos),
    MineDisappeared(Pos),
    GoldChanged { previous: i32, current: i32 },
    InventoryChanged(i64),
    BuildingChanged(i64),
    WeaponReady(i64),
    ChallengeStarted,
    ChallengeEnded,
    NewsChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldIssue {
    MapDimensionsChanged,
    FactionChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportScope {
    Step(Versioned<IntentId>),
    Plan(Versioned<PlanId>),
    Mission(Versioned<MissionId>),
    Strategy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReportStatus {
    Progress,
    Completed,
    Blocked,
    Failed,
    AtRisk,
    ResourceNeeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportReason {
    OwnerDied,
    ActionLegal,
    ActionRejected,
    ActionResultUnknown,
}

#[derive(Clone, Debug)]
pub struct ExecutionReport {
    pub scope: ReportScope,
    pub status: ReportStatus,
    pub owner: OwnerPath,
    pub reporter: i64,
    pub reason: ReportReason,
    pub observed_round: i32,
}
