use std::collections::BTreeSet;

use super::{MissionId, Pos};
use crate::rules::constants::MAX_LOCAL_REPLANS;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MissionKind {
    GatherAndSell,
    Construct,
    Challenge,
    DefendSector,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ObjectiveKey {
    EconomyCycle,
    ConstructionSite(Pos),
    ChallengeSession,
    DefenseWeapon(i64),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GoalPredicate {
    EconomyCycleCompleted,
    BuildingPresent { kind: String, site: Pos },
    ChallengeResolved,
    DefenseWindowCompleted { weapon: i64 },
    AllOf(Vec<GoalPredicate>),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MissionCapability {
    Gather,
    Sell,
    Build,
    SolveChallenge,
    OperateWeapon,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PriorityClass {
    Q0Preserve,
    Q1Defense,
    Q2Deadline,
    Q3Economy,
    Q4Opportunity,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Interruptibility {
    SuspendAndResume,
    CancelOnly,
    FinishCurrentStep,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DeadlineKind {
    ActionSubmission,
    EffectObservation,
    InternalPlanning,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MissionDeadline {
    at_round: i32,
    inclusive: bool,
    kind: DeadlineKind,
}

impl MissionDeadline {
    pub const fn new(at_round: i32, inclusive: bool, kind: DeadlineKind) -> Self {
        Self {
            at_round,
            inclusive,
            kind,
        }
    }

    pub const fn at_round(self) -> i32 {
        self.at_round
    }

    pub const fn inclusive(self) -> bool {
        self.inclusive
    }

    pub const fn kind(self) -> DeadlineKind {
        self.kind
    }

    pub const fn is_expired(self, round: i32) -> bool {
        if self.inclusive {
            round > self.at_round
        } else {
            round >= self.at_round
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RetryPolicy {
    max_replans: u8,
}

impl RetryPolicy {
    pub const fn bounded(max_replans: u8) -> Self {
        Self { max_replans }
    }

    pub const fn max_replans(self) -> u8 {
        self.max_replans
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MissionSpec {
    kind: MissionKind,
    objective: ObjectiveKey,
    goal: GoalPredicate,
    dependencies: BTreeSet<MissionId>,
    required_capabilities: BTreeSet<MissionCapability>,
    deadline: Option<MissionDeadline>,
    priority: PriorityClass,
    interruptibility: Interruptibility,
    retry_policy: RetryPolicy,
}

impl MissionSpec {
    pub fn economy() -> Self {
        Self::new(
            MissionKind::GatherAndSell,
            ObjectiveKey::EconomyCycle,
            GoalPredicate::EconomyCycleCompleted,
            [MissionCapability::Gather, MissionCapability::Sell],
            PriorityClass::Q3Economy,
            Interruptibility::SuspendAndResume,
        )
    }

    pub fn construction(site: Pos, building: impl Into<String>) -> Self {
        Self::new(
            MissionKind::Construct,
            ObjectiveKey::ConstructionSite(site),
            GoalPredicate::BuildingPresent {
                kind: building.into(),
                site,
            },
            [MissionCapability::Build],
            PriorityClass::Q1Defense,
            Interruptibility::FinishCurrentStep,
        )
    }

    pub fn challenge() -> Self {
        Self::new(
            MissionKind::Challenge,
            ObjectiveKey::ChallengeSession,
            GoalPredicate::ChallengeResolved,
            [MissionCapability::SolveChallenge],
            PriorityClass::Q2Deadline,
            Interruptibility::CancelOnly,
        )
    }

    pub fn defense(weapon: i64) -> Self {
        Self::new(
            MissionKind::DefendSector,
            ObjectiveKey::DefenseWeapon(weapon),
            GoalPredicate::DefenseWindowCompleted { weapon },
            [MissionCapability::OperateWeapon],
            PriorityClass::Q1Defense,
            Interruptibility::FinishCurrentStep,
        )
    }

    pub fn with_dependencies(mut self, dependencies: impl IntoIterator<Item = MissionId>) -> Self {
        self.dependencies = dependencies.into_iter().collect();
        self
    }

    pub fn with_deadline(mut self, deadline: MissionDeadline) -> Self {
        self.deadline = Some(deadline);
        self
    }

    pub fn with_priority(mut self, priority: PriorityClass) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_interruptibility(mut self, interruptibility: Interruptibility) -> Self {
        self.interruptibility = interruptibility;
        self
    }

    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    pub const fn kind(&self) -> MissionKind {
        self.kind
    }

    pub const fn objective(&self) -> ObjectiveKey {
        self.objective
    }

    pub const fn goal(&self) -> &GoalPredicate {
        &self.goal
    }

    pub const fn dependencies(&self) -> &BTreeSet<MissionId> {
        &self.dependencies
    }

    pub const fn required_capabilities(&self) -> &BTreeSet<MissionCapability> {
        &self.required_capabilities
    }

    pub const fn deadline(&self) -> Option<MissionDeadline> {
        self.deadline
    }

    pub const fn priority(&self) -> PriorityClass {
        self.priority
    }

    pub const fn interruptibility(&self) -> Interruptibility {
        self.interruptibility
    }

    pub const fn retry_policy(&self) -> RetryPolicy {
        self.retry_policy
    }

    pub(crate) fn matches_except_deadline(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.objective == other.objective
            && self.goal == other.goal
            && self.dependencies == other.dependencies
            && self.required_capabilities == other.required_capabilities
            && self.priority == other.priority
            && self.interruptibility == other.interruptibility
            && self.retry_policy == other.retry_policy
    }

    fn new(
        kind: MissionKind,
        objective: ObjectiveKey,
        goal: GoalPredicate,
        required_capabilities: impl IntoIterator<Item = MissionCapability>,
        priority: PriorityClass,
        interruptibility: Interruptibility,
    ) -> Self {
        Self {
            kind,
            objective,
            goal,
            dependencies: BTreeSet::new(),
            required_capabilities: required_capabilities.into_iter().collect(),
            deadline: None,
            priority,
            interruptibility,
            retry_policy: RetryPolicy::bounded(MAX_LOCAL_REPLANS),
        }
    }
}
