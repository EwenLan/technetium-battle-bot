use crate::domain::{MissionId, MissionSpec, OwnerPath};
use crate::fsm::MissionState;

use super::GoalEvidence;
use super::progress::EconomyProgress;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissionCancellation {
    pub(super) assignee: i64,
    pub(super) owner: OwnerPath,
}

impl MissionCancellation {
    pub const fn assignee(self) -> i64 {
        self.assignee
    }

    pub const fn owner(self) -> OwnerPath {
        self.owner
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissionCompletion {
    pub(super) owner: OwnerPath,
    pub(super) ready: Vec<MissionId>,
}

impl MissionCompletion {
    pub const fn owner(&self) -> OwnerPath {
        self.owner
    }

    pub fn ready(&self) -> &[MissionId] {
        &self.ready
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissionView {
    spec: MissionSpec,
    owner: OwnerPath,
    assignee: i64,
    state: MissionState,
    completion_evidence: Vec<GoalEvidence>,
    goal_submission_round: Option<i32>,
    progress_evidence: Vec<GoalEvidence>,
}

impl MissionView {
    pub const fn spec(&self) -> &MissionSpec {
        &self.spec
    }

    pub const fn owner(&self) -> OwnerPath {
        self.owner
    }

    pub const fn assignee(&self) -> i64 {
        self.assignee
    }

    pub const fn state(&self) -> MissionState {
        self.state
    }

    pub fn completion_evidence(&self) -> &[GoalEvidence] {
        &self.completion_evidence
    }

    pub const fn goal_submission_round(&self) -> Option<i32> {
        self.goal_submission_round
    }

    pub fn progress_evidence(&self) -> &[GoalEvidence] {
        &self.progress_evidence
    }

    pub fn dependencies(&self) -> &std::collections::BTreeSet<MissionId> {
        self.spec.dependencies()
    }
}

#[derive(Clone, Debug)]
pub(super) struct MissionRecord {
    pub(super) spec: MissionSpec,
    pub(super) owner: OwnerPath,
    pub(super) assignee: i64,
    pub(super) state: MissionState,
    pub(super) completion_evidence: Vec<GoalEvidence>,
    pub(super) goal_submission_round: Option<i32>,
    pub(super) economy_progress: EconomyProgress,
}

impl MissionRecord {
    pub(super) fn view(&self) -> MissionView {
        MissionView {
            spec: self.spec.clone(),
            owner: self.owner,
            assignee: self.assignee,
            state: self.state,
            completion_evidence: self.completion_evidence.clone(),
            goal_submission_round: self.goal_submission_round,
            progress_evidence: self.economy_progress.evidence().to_vec(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissionResolution {
    assignee: i64,
    owner: OwnerPath,
    state: MissionState,
}

impl MissionResolution {
    pub(super) const fn from_record(record: &MissionRecord) -> Self {
        Self {
            assignee: record.assignee,
            owner: record.owner,
            state: record.state,
        }
    }

    pub const fn assignee(self) -> i64 {
        self.assignee
    }

    pub const fn owner(self) -> OwnerPath {
        self.owner
    }

    pub const fn state(self) -> MissionState {
        self.state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissionReconciliation {
    resolved: Vec<MissionResolution>,
    ready: Vec<MissionId>,
}

impl MissionReconciliation {
    pub(super) const fn new(resolved: Vec<MissionResolution>, ready: Vec<MissionId>) -> Self {
        Self { resolved, ready }
    }

    pub fn resolved(&self) -> &[MissionResolution] {
        &self.resolved
    }

    pub fn ready(&self) -> &[MissionId] {
        &self.ready
    }
}
