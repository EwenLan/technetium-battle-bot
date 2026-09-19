use std::collections::BTreeSet;

use crate::domain::{MissionId, MissionSpec, OwnerPath};
use crate::fsm::MissionState;

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
    dependencies: BTreeSet<MissionId>,
}

impl MissionView {
    pub const fn spec(&self) -> MissionSpec {
        self.spec
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

    pub const fn dependencies(&self) -> &BTreeSet<MissionId> {
        &self.dependencies
    }
}

#[derive(Clone, Debug)]
pub(super) struct MissionRecord {
    pub(super) spec: MissionSpec,
    pub(super) owner: OwnerPath,
    pub(super) assignee: i64,
    pub(super) state: MissionState,
    pub(super) dependencies: BTreeSet<MissionId>,
}

impl MissionRecord {
    pub(super) fn view(&self) -> MissionView {
        MissionView {
            spec: self.spec,
            owner: self.owner,
            assignee: self.assignee,
            state: self.state,
            dependencies: self.dependencies.clone(),
        }
    }
}
