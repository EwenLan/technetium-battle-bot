use crate::domain::{DeadlineKind, MissionDeadline, MissionId, Observation};
use crate::event::EventRecord;
use crate::fsm::MissionState;

use super::goal::{GoalEvaluation, evaluate_goal};
use super::record::{MissionReconciliation, MissionResolution};
use super::registry::{MissionRegistry, is_terminal};

impl MissionRegistry {
    pub fn reconcile(
        &mut self,
        observation: &Observation,
        events: &[EventRecord],
    ) -> MissionReconciliation {
        let mut resolved = self.complete_satisfied(observation, events);
        resolved.extend(self.expire_due(observation.round_no));
        let ready = self.wake_ready();
        MissionReconciliation::new(resolved, ready)
    }

    fn complete_satisfied(
        &mut self,
        observation: &Observation,
        events: &[EventRecord],
    ) -> Vec<MissionResolution> {
        let satisfied: Vec<_> = self
            .records
            .iter()
            .filter(|(_, record)| goal_can_complete(record.state))
            .filter_map(|(mission, record)| {
                match evaluate_goal(record.spec.goal(), observation, events) {
                    GoalEvaluation::Satisfied(evidence)
                        if completion_is_timely(record, &evidence) =>
                    {
                        Some((*mission, evidence))
                    }
                    _ => None,
                }
            })
            .collect();
        satisfied
            .into_iter()
            .filter_map(|(mission, evidence)| self.complete(mission, evidence))
            .collect()
    }

    fn complete(
        &mut self,
        mission: MissionId,
        evidence: Vec<super::GoalEvidence>,
    ) -> Option<MissionResolution> {
        let record = self.records.get_mut(&mission)?;
        record.state = MissionState::Succeeded;
        record.completion_evidence = evidence;
        let resolution = MissionResolution::from_record(record);
        self.remove_active(resolution.assignee(), mission);
        Some(resolution)
    }

    fn expire_due(&mut self, round: i32) -> Vec<MissionResolution> {
        let due: Vec<_> = self
            .records
            .iter()
            .filter(|(_, record)| !is_terminal(record.state))
            .filter(|(_, record)| {
                record
                    .spec
                    .deadline()
                    .is_some_and(|deadline| deadline_missed(record, deadline, round))
            })
            .map(|(mission, _)| *mission)
            .collect();
        due.into_iter()
            .flat_map(|mission| self.expire_tree(mission))
            .collect()
    }

    fn expire_tree(&mut self, root: MissionId) -> Vec<MissionResolution> {
        if self
            .records
            .get(&root)
            .is_none_or(|record| is_terminal(record.state))
        {
            return Vec::new();
        }
        self.cancellation_ids(root)
            .into_iter()
            .filter_map(|mission| self.expire_record(root, mission))
            .collect()
    }

    fn expire_record(&mut self, root: MissionId, mission: MissionId) -> Option<MissionResolution> {
        let record = self.records.get_mut(&mission)?;
        if is_terminal(record.state) {
            return None;
        }
        record.state = if mission == root {
            MissionState::Expired
        } else {
            MissionState::Cancelled
        };
        let resolution = MissionResolution::from_record(record);
        self.remove_active(resolution.assignee(), mission);
        Some(resolution)
    }
}

fn completion_is_timely(
    record: &super::record::MissionRecord,
    evidence: &[super::GoalEvidence],
) -> bool {
    let Some(deadline) = record.spec.deadline() else {
        return true;
    };
    let evidence_timely = evidence
        .iter()
        .all(|item| !deadline.is_expired(item.observed_round()));
    evidence_timely || submitted_on_time(record, deadline)
}

fn deadline_missed(
    record: &super::record::MissionRecord,
    deadline: MissionDeadline,
    round: i32,
) -> bool {
    deadline.is_expired(round) && !submitted_on_time(record, deadline)
}

fn submitted_on_time(record: &super::record::MissionRecord, deadline: MissionDeadline) -> bool {
    deadline.kind() == DeadlineKind::ActionSubmission
        && record
            .goal_submission_round
            .is_some_and(|round| !deadline.is_expired(round))
}

fn goal_can_complete(state: MissionState) -> bool {
    matches!(
        state,
        MissionState::Ready
            | MissionState::Assigned
            | MissionState::Executing
            | MissionState::Blocked
            | MissionState::Suspended
    )
}
