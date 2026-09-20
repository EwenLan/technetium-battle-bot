use std::collections::{BTreeMap, BTreeSet};

use crate::domain::{Action, GoalPredicate, MissionId, MissionSpec, OwnerPath};
use crate::fsm::MissionState;

use super::record::{MissionCancellation, MissionCompletion, MissionRecord, MissionView};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissionRegistryError {
    DuplicateMission,
    UnknownMission,
    MissingPlan,
    IntentNotAllowed,
    SelfDependency,
    UnknownDependency,
    DependencyUnavailable,
    InvalidTransition,
}

#[derive(Clone, Debug, Default)]
pub struct MissionRegistry {
    pub(super) records: BTreeMap<MissionId, MissionRecord>,
    pub(super) active_by_assignee: BTreeMap<i64, MissionId>,
}

impl MissionRegistry {
    pub fn register(
        &mut self,
        assignee: i64,
        spec: MissionSpec,
        owner: OwnerPath,
    ) -> Result<MissionId, MissionRegistryError> {
        validate_assignment(owner)?;
        let mission = owner.mission().id;
        if self.records.contains_key(&mission) {
            return Err(MissionRegistryError::DuplicateMission);
        }
        self.validate_dependencies(mission, spec.dependencies())?;
        let state = self.initial_state(spec.dependencies());
        self.records.insert(
            mission,
            MissionRecord {
                spec,
                owner,
                assignee,
                state,
                completion_evidence: Vec::new(),
                goal_submission_round: None,
                economy_progress: super::progress::EconomyProgress::default(),
            },
        );
        Ok(mission)
    }

    pub fn assign(
        &mut self,
        mission: MissionId,
    ) -> Result<Vec<MissionCancellation>, MissionRegistryError> {
        let record = self
            .records
            .get(&mission)
            .ok_or(MissionRegistryError::UnknownMission)?;
        if record.state != MissionState::Ready {
            return Err(MissionRegistryError::InvalidTransition);
        }
        let assignee = record.assignee;
        let cancelled = self.cancel_current_except(assignee, mission);
        let record = self
            .records
            .get_mut(&mission)
            .ok_or(MissionRegistryError::UnknownMission)?;
        record.state = MissionState::Assigned;
        self.active_by_assignee.insert(assignee, mission);
        Ok(cancelled)
    }

    pub fn activate(&mut self, mission: MissionId) -> Result<(), MissionRegistryError> {
        self.transition(mission, MissionState::Assigned, MissionState::Executing)
    }

    pub fn succeed(
        &mut self,
        mission: MissionId,
    ) -> Result<MissionCompletion, MissionRegistryError> {
        let record = self
            .records
            .get(&mission)
            .ok_or(MissionRegistryError::UnknownMission)?;
        if !matches!(
            record.state,
            MissionState::Executing | MissionState::Blocked | MissionState::Suspended
        ) {
            return Err(MissionRegistryError::InvalidTransition);
        }
        let assignee = record.assignee;
        let owner = record.owner;
        let record = self
            .records
            .get_mut(&mission)
            .ok_or(MissionRegistryError::UnknownMission)?;
        record.state = MissionState::Succeeded;
        self.remove_active(assignee, mission);
        Ok(MissionCompletion {
            owner,
            ready: self.wake_ready(),
        })
    }

    pub fn cancel(
        &mut self,
        mission: MissionId,
    ) -> Result<Vec<MissionCancellation>, MissionRegistryError> {
        let state = self
            .records
            .get(&mission)
            .map(|record| record.state)
            .ok_or(MissionRegistryError::UnknownMission)?;
        if is_terminal(state) {
            return Err(MissionRegistryError::InvalidTransition);
        }
        Ok(self.cancel_tree(mission))
    }

    pub fn view(&self, mission: MissionId) -> Option<MissionView> {
        self.records.get(&mission).map(MissionRecord::view)
    }

    pub fn current_assignment(&self, assignee: i64, spec: &MissionSpec) -> Option<OwnerPath> {
        let mission = self.active_by_assignee.get(&assignee)?;
        self.records.get(mission).and_then(|record| {
            (&record.spec == spec && is_active(record.state)).then_some(record.owner)
        })
    }

    pub(crate) fn activate_for_action(
        &mut self,
        assignee: i64,
        spec: MissionSpec,
        owner: OwnerPath,
    ) -> Result<Vec<MissionCancellation>, MissionRegistryError> {
        let mission = self.register(assignee, spec, owner)?;
        let cancelled = self.assign(mission)?;
        self.activate(mission)?;
        Ok(cancelled)
    }

    pub(crate) fn mark_blocked(&mut self, owner: &OwnerPath) -> bool {
        let mission = owner.mission().id;
        let Some(record) = self.records.get_mut(&mission) else {
            return false;
        };
        if record.owner != owner.assignment() || !is_blockable(record.state) {
            return false;
        }
        record.state = MissionState::Blocked;
        true
    }

    pub(crate) fn record_action_commit(
        &mut self,
        owner: &OwnerPath,
        actor: i64,
        action: &Action,
        input: &crate::domain::Observation,
    ) -> bool {
        let Some(record) = self.records.get_mut(&owner.mission().id) else {
            return false;
        };
        if record.owner != owner.assignment() || !is_active(record.state) {
            return false;
        }
        super::progress::record_action(record, action, input);
        if action_submits_goal(record.spec.goal(), actor, action) {
            record.goal_submission_round.get_or_insert(input.round_no);
        }
        true
    }

    pub(crate) fn resume_for_action(
        &mut self,
        owner: &OwnerPath,
    ) -> Result<(), MissionRegistryError> {
        let record = self
            .records
            .get_mut(&owner.mission().id)
            .ok_or(MissionRegistryError::UnknownMission)?;
        if record.owner != owner.assignment() || !is_active(record.state) {
            return Err(MissionRegistryError::InvalidTransition);
        }
        record.state = MissionState::Executing;
        Ok(())
    }

    pub(crate) fn cancel_owner(&mut self, owner: &OwnerPath) -> Vec<MissionCancellation> {
        let mission = owner.mission().id;
        let matches = self
            .records
            .get(&mission)
            .is_some_and(|record| record.owner == owner.assignment());
        if !matches {
            return Vec::new();
        }
        self.cancel_tree(mission)
    }

    pub(crate) fn cancel_current(&mut self, assignee: i64) -> Vec<MissionCancellation> {
        self.active_by_assignee
            .get(&assignee)
            .copied()
            .map(|mission| self.cancel_tree(mission))
            .unwrap_or_default()
    }

    fn initial_state(&self, dependencies: &BTreeSet<MissionId>) -> MissionState {
        if self.dependencies_succeeded(dependencies) {
            MissionState::Ready
        } else {
            MissionState::Proposed
        }
    }

    fn validate_dependencies(
        &self,
        mission: MissionId,
        dependencies: &BTreeSet<MissionId>,
    ) -> Result<(), MissionRegistryError> {
        for dependency in dependencies {
            if *dependency == mission {
                return Err(MissionRegistryError::SelfDependency);
            }
            match self.records.get(dependency) {
                None => return Err(MissionRegistryError::UnknownDependency),
                Some(record) if dependency_unavailable(record.state) => {
                    return Err(MissionRegistryError::DependencyUnavailable);
                }
                Some(_) => {}
            }
        }
        Ok(())
    }

    fn dependencies_succeeded(&self, dependencies: &BTreeSet<MissionId>) -> bool {
        dependencies.iter().all(|dependency| {
            self.records
                .get(dependency)
                .is_some_and(|record| record.state == MissionState::Succeeded)
        })
    }

    fn transition(
        &mut self,
        mission: MissionId,
        expected: MissionState,
        next: MissionState,
    ) -> Result<(), MissionRegistryError> {
        let record = self
            .records
            .get_mut(&mission)
            .ok_or(MissionRegistryError::UnknownMission)?;
        if record.state != expected {
            return Err(MissionRegistryError::InvalidTransition);
        }
        record.state = next;
        Ok(())
    }

    pub(super) fn wake_ready(&mut self) -> Vec<MissionId> {
        let ready: Vec<_> = self
            .records
            .iter()
            .filter(|(_, record)| {
                record.state == MissionState::Proposed
                    && self.dependencies_succeeded(record.spec.dependencies())
            })
            .map(|(mission, _)| *mission)
            .collect();
        for mission in &ready {
            if let Some(record) = self.records.get_mut(mission) {
                record.state = MissionState::Ready;
            }
        }
        ready
    }

    fn cancel_current_except(
        &mut self,
        assignee: i64,
        replacement: MissionId,
    ) -> Vec<MissionCancellation> {
        self.active_by_assignee
            .get(&assignee)
            .copied()
            .filter(|current| *current != replacement)
            .map(|current| self.cancel_tree(current))
            .unwrap_or_default()
    }

    fn cancel_tree(&mut self, root: MissionId) -> Vec<MissionCancellation> {
        let missions = self.cancellation_ids(root);
        let mut cancelled = Vec::new();
        for mission in missions {
            let Some(record) = self.records.get(&mission) else {
                continue;
            };
            if is_terminal(record.state) {
                continue;
            }
            let cancellation = MissionCancellation {
                assignee: record.assignee,
                owner: record.owner,
            };
            if let Some(record) = self.records.get_mut(&mission) {
                record.state = MissionState::Cancelled;
            }
            self.remove_active(cancellation.assignee, mission);
            cancelled.push(cancellation);
        }
        cancelled
    }

    pub(super) fn cancellation_ids(&self, root: MissionId) -> Vec<MissionId> {
        let mut pending = BTreeSet::from([root]);
        let mut found = BTreeSet::new();
        while let Some(mission) = pending.pop_first() {
            found.insert(mission);
            for (candidate, record) in &self.records {
                if record.spec.dependencies().contains(&mission) && !found.contains(candidate) {
                    pending.insert(*candidate);
                }
            }
        }
        found.into_iter().collect()
    }

    pub(super) fn remove_active(&mut self, assignee: i64, mission: MissionId) {
        if self.active_by_assignee.get(&assignee) == Some(&mission) {
            self.active_by_assignee.remove(&assignee);
        }
    }
}

fn action_submits_goal(goal: &GoalPredicate, actor: i64, action: &Action) -> bool {
    match (goal, action) {
        (GoalPredicate::EconomyCycleCompleted, Action::Sell { .. }) => true,
        (GoalPredicate::BuildingPresent { kind, site }, Action::Build { name, pos }) => {
            kind == name && site == pos
        }
        (GoalPredicate::ChallengeResolved, Action::SubmitAnswer(_)) => true,
        (GoalPredicate::DefenseWindowCompleted { weapon }, Action::Attack { .. }) => {
            *weapon == actor
        }
        _ => false,
    }
}

fn validate_assignment(owner: OwnerPath) -> Result<(), MissionRegistryError> {
    if owner.plan().is_none() {
        return Err(MissionRegistryError::MissingPlan);
    }
    if owner.intent().is_some() {
        return Err(MissionRegistryError::IntentNotAllowed);
    }
    Ok(())
}

fn is_active(state: MissionState) -> bool {
    matches!(
        state,
        MissionState::Assigned
            | MissionState::Executing
            | MissionState::Blocked
            | MissionState::Suspended
    )
}

fn is_blockable(state: MissionState) -> bool {
    matches!(
        state,
        MissionState::Assigned | MissionState::Executing | MissionState::Blocked
    )
}

fn dependency_unavailable(state: MissionState) -> bool {
    matches!(
        state,
        MissionState::Failed | MissionState::Cancelled | MissionState::Expired
    )
}

pub(super) fn is_terminal(state: MissionState) -> bool {
    matches!(
        state,
        MissionState::Succeeded
            | MissionState::Failed
            | MissionState::Cancelled
            | MissionState::Expired
    )
}
