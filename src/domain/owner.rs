use std::collections::BTreeMap;

use crate::rules::constants::{VERSION_INCREMENT, ZERO_VERSION};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MissionId {
    value: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PlanId {
    value: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct IntentId {
    value: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Generation {
    value: u64,
}

impl MissionId {
    pub const fn new(value: u64) -> Self {
        Self { value }
    }

    pub const fn value(self) -> u64 {
        self.value
    }
}

impl PlanId {
    pub const fn new(value: u64) -> Self {
        Self { value }
    }

    pub const fn value(self) -> u64 {
        self.value
    }
}

impl IntentId {
    pub const fn new(value: u64) -> Self {
        Self { value }
    }

    pub const fn value(self) -> u64 {
        self.value
    }
}

impl Generation {
    pub const fn initial() -> Self {
        Self {
            value: ZERO_VERSION,
        }
    }

    pub fn next(self) -> Option<Self> {
        self.value
            .checked_add(VERSION_INCREMENT)
            .map(|value| Self { value })
    }

    pub const fn value(self) -> u64 {
        self.value
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Versioned<T> {
    pub id: T,
    pub generation: Generation,
}

impl<T> Versioned<T> {
    pub const fn new(id: T, generation: Generation) -> Self {
        Self { id, generation }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerPath {
    mission: Versioned<MissionId>,
    plan: Option<Versioned<PlanId>>,
    intent: Option<Versioned<IntentId>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerError {
    IntentWithoutPlan,
    MissionNotCurrent,
    PlanNotCurrent,
    GenerationNotAdvanced,
    ParentChanged,
}

impl OwnerPath {
    pub fn new(
        mission: Versioned<MissionId>,
        plan: Option<Versioned<PlanId>>,
        intent: Option<Versioned<IntentId>>,
    ) -> Result<Self, OwnerError> {
        if intent.is_some() && plan.is_none() {
            return Err(OwnerError::IntentWithoutPlan);
        }
        Ok(Self {
            mission,
            plan,
            intent,
        })
    }

    pub const fn mission(&self) -> Versioned<MissionId> {
        self.mission
    }

    pub const fn plan(&self) -> Option<Versioned<PlanId>> {
        self.plan
    }

    pub const fn intent(&self) -> Option<Versioned<IntentId>> {
        self.intent
    }
}

#[derive(Clone, Copy, Debug)]
struct PlanRegistration {
    mission: Versioned<MissionId>,
    generation: Generation,
}

#[derive(Clone, Copy, Debug)]
struct IntentRegistration {
    plan: Versioned<PlanId>,
    generation: Generation,
}

#[derive(Clone, Debug, Default)]
pub struct ActiveOwners {
    missions: BTreeMap<MissionId, Generation>,
    plans: BTreeMap<PlanId, PlanRegistration>,
    intents: BTreeMap<IntentId, IntentRegistration>,
}

impl ActiveOwners {
    pub fn activate_mission(&mut self, mission: Versioned<MissionId>) -> Result<(), OwnerError> {
        validate_generation(self.missions.get(&mission.id).copied(), mission.generation)?;
        self.missions.insert(mission.id, mission.generation);
        Ok(())
    }

    pub fn activate_plan(
        &mut self,
        mission: Versioned<MissionId>,
        plan: Versioned<PlanId>,
    ) -> Result<(), OwnerError> {
        if !self.mission_is_current(mission) {
            return Err(OwnerError::MissionNotCurrent);
        }
        validate_plan_registration(self.plans.get(&plan.id), mission, plan.generation)?;
        self.plans.insert(
            plan.id,
            PlanRegistration {
                mission,
                generation: plan.generation,
            },
        );
        Ok(())
    }

    pub fn activate_intent(
        &mut self,
        mission: Versioned<MissionId>,
        plan: Versioned<PlanId>,
        intent: Versioned<IntentId>,
    ) -> Result<(), OwnerError> {
        if !self.plan_is_current(mission, plan) {
            return Err(OwnerError::PlanNotCurrent);
        }
        validate_intent_registration(self.intents.get(&intent.id), plan, intent.generation)?;
        self.intents.insert(
            intent.id,
            IntentRegistration {
                plan,
                generation: intent.generation,
            },
        );
        Ok(())
    }

    pub fn is_current(&self, owner: &OwnerPath) -> bool {
        if !self.mission_is_current(owner.mission) {
            return false;
        }
        if owner
            .plan
            .is_some_and(|plan| !self.plan_is_current(owner.mission, plan))
        {
            return false;
        }
        owner.intent.is_none_or(|intent| {
            owner
                .plan
                .is_some_and(|plan| self.intent_is_current(plan, intent))
        })
    }

    fn mission_is_current(&self, mission: Versioned<MissionId>) -> bool {
        self.missions.get(&mission.id) == Some(&mission.generation)
    }

    fn plan_is_current(&self, mission: Versioned<MissionId>, plan: Versioned<PlanId>) -> bool {
        self.plans.get(&plan.id).is_some_and(|registered| {
            registered.mission == mission && registered.generation == plan.generation
        })
    }

    fn intent_is_current(&self, plan: Versioned<PlanId>, intent: Versioned<IntentId>) -> bool {
        self.intents.get(&intent.id).is_some_and(|registered| {
            registered.plan == plan && registered.generation == intent.generation
        })
    }
}

fn validate_generation(
    current: Option<Generation>,
    candidate: Generation,
) -> Result<(), OwnerError> {
    if current.is_some_and(|generation| candidate <= generation) {
        return Err(OwnerError::GenerationNotAdvanced);
    }
    Ok(())
}

fn validate_plan_registration(
    current: Option<&PlanRegistration>,
    mission: Versioned<MissionId>,
    generation: Generation,
) -> Result<(), OwnerError> {
    if current.is_some_and(|registered| registered.mission != mission) {
        return Err(OwnerError::ParentChanged);
    }
    validate_generation(current.map(|registered| registered.generation), generation)
}

fn validate_intent_registration(
    current: Option<&IntentRegistration>,
    plan: Versioned<PlanId>,
    generation: Generation,
) -> Result<(), OwnerError> {
    if current.is_some_and(|registered| registered.plan != plan) {
        return Err(OwnerError::ParentChanged);
    }
    validate_generation(current.map(|registered| registered.generation), generation)
}
