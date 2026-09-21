use crate::domain::{MissionId, MissionSpec, OwnerPath};
use crate::fsm::MissionState;
use crate::rules::constants::COUNTER_INCREMENT;

use super::record::{MissionFailure, MissionResolution};
use super::registry::{MissionRegistry, is_blockable, is_terminal};

impl MissionRegistry {
    pub(crate) fn proposal_allowed(&self, assignee: i64, spec: &MissionSpec) -> bool {
        !self.records.values().any(|record| {
            record.assignee == assignee
                && record.state == MissionState::Failed
                && &record.spec == spec
        })
    }

    pub(crate) fn record_action_rejection(
        &mut self,
        owner: &OwnerPath,
        observed_round: i32,
    ) -> Vec<MissionResolution> {
        let mission = owner.mission().id;
        let Some(record) = self.records.get_mut(&mission) else {
            return Vec::new();
        };
        if record.owner != owner.assignment() || !is_blockable(record.state) {
            return Vec::new();
        }
        record.retry_count = record.retry_count.saturating_add(COUNTER_INCREMENT);
        if record.retry_count < record.spec.retry_policy().max_replans() {
            record.state = MissionState::Blocked;
            return Vec::new();
        }
        self.fail_tree(mission, observed_round)
    }

    fn fail_tree(&mut self, root: MissionId, observed_round: i32) -> Vec<MissionResolution> {
        self.cancellation_ids(root)
            .into_iter()
            .filter_map(|mission| self.fail_record(root, mission, observed_round))
            .collect()
    }

    fn fail_record(
        &mut self,
        root: MissionId,
        mission: MissionId,
        observed_round: i32,
    ) -> Option<MissionResolution> {
        let record = self.records.get_mut(&mission)?;
        if is_terminal(record.state) {
            return None;
        }
        record.state = if mission == root {
            record.failure = Some(MissionFailure::retry_exhausted(
                observed_round,
                record.retry_count,
            ));
            MissionState::Failed
        } else {
            MissionState::Cancelled
        };
        let resolution = MissionResolution::from_record(record);
        self.remove_active(resolution.assignee(), mission);
        Some(resolution)
    }
}
