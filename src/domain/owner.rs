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
    IntentAlreadyPresent,
    MissionNotCurrent,
    PlanNotCurrent,
    GenerationNotAdvanced,
    ParentChanged,
    IdSpaceExhausted,
    IdAlreadyRegistered,
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

    pub const fn assignment(&self) -> Self {
        Self {
            mission: self.mission,
            plan: self.plan,
            intent: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct MissionRegistration {
    generation: Generation,
    active: bool,
}

#[derive(Clone, Copy, Debug)]
struct PlanRegistration {
    mission: Versioned<MissionId>,
    generation: Generation,
    active: bool,
}

#[derive(Clone, Copy, Debug)]
struct IntentRegistration {
    plan: Versioned<PlanId>,
    generation: Generation,
    active: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ActiveOwners {
    missions: BTreeMap<MissionId, MissionRegistration>,
    plans: BTreeMap<PlanId, PlanRegistration>,
    intents: BTreeMap<IntentId, IntentRegistration>,
}

#[derive(Clone, Debug)]
pub struct OwnerAllocator {
    next_id: u64,
}

impl Default for OwnerAllocator {
    fn default() -> Self {
        Self {
            next_id: ZERO_VERSION,
        }
    }
}

impl OwnerAllocator {
    fn allocate(&mut self) -> Result<u64, OwnerError> {
        let allocated = self.next_id;
        let next = allocated
            .checked_add(VERSION_INCREMENT)
            .ok_or(OwnerError::IdSpaceExhausted)?;
        self.next_id = next;
        Ok(allocated)
    }
}

impl ActiveOwners {
    pub fn create_assignment(
        &mut self,
        allocator: &mut OwnerAllocator,
    ) -> Result<OwnerPath, OwnerError> {
        let id = allocator.allocate()?;
        let generation = Generation::initial();
        let mission = Versioned::new(MissionId::new(id), generation);
        let plan = Versioned::new(PlanId::new(id), generation);
        if self.missions.contains_key(&mission.id) || self.plans.contains_key(&plan.id) {
            return Err(OwnerError::IdAlreadyRegistered);
        }
        self.missions.insert(
            mission.id,
            MissionRegistration {
                generation,
                active: true,
            },
        );
        self.plans.insert(
            plan.id,
            PlanRegistration {
                mission,
                generation,
                active: true,
            },
        );
        Ok(OwnerPath {
            mission,
            plan: Some(plan),
            intent: None,
        })
    }

    pub fn create_intent(
        &mut self,
        allocator: &mut OwnerAllocator,
        assignment: &OwnerPath,
    ) -> Result<OwnerPath, OwnerError> {
        if assignment.intent.is_some() {
            return Err(OwnerError::IntentAlreadyPresent);
        }
        let Some(plan) = assignment.plan else {
            return Err(OwnerError::PlanNotCurrent);
        };
        if !self.plan_is_current(assignment.mission, plan) {
            return Err(OwnerError::PlanNotCurrent);
        }
        let id = allocator.allocate()?;
        let intent = Versioned::new(IntentId::new(id), Generation::initial());
        if self.intents.contains_key(&intent.id) {
            return Err(OwnerError::IdAlreadyRegistered);
        }
        self.intents.insert(
            intent.id,
            IntentRegistration {
                plan,
                generation: intent.generation,
                active: true,
            },
        );
        OwnerPath::new(assignment.mission, Some(plan), Some(intent))
    }

    pub fn create_path(&mut self, allocator: &mut OwnerAllocator) -> Result<OwnerPath, OwnerError> {
        let assignment = self.create_assignment(allocator)?;
        let result = self.create_intent(allocator, &assignment);
        if result.is_err() {
            self.deactivate_assignment(&assignment);
        }
        result
    }

    pub fn deactivate_assignment(&mut self, assignment: &OwnerPath) -> bool {
        let mission_changed = self.deactivate_mission(assignment.mission.id);
        let plan_changed = assignment
            .plan
            .is_some_and(|plan| self.deactivate_plan(plan.id));
        mission_changed || plan_changed
    }

    pub fn deactivate_mission(&mut self, mission: MissionId) -> bool {
        let Some(registered) = self.missions.get_mut(&mission) else {
            return false;
        };
        let was_active = registered.active;
        registered.active = false;
        was_active
    }

    pub fn deactivate_plan(&mut self, plan: PlanId) -> bool {
        let Some(registered) = self.plans.get_mut(&plan) else {
            return false;
        };
        let was_active = registered.active;
        registered.active = false;
        was_active
    }

    pub fn deactivate_intent(&mut self, intent: IntentId) -> bool {
        let Some(registered) = self.intents.get_mut(&intent) else {
            return false;
        };
        let was_active = registered.active;
        registered.active = false;
        was_active
    }

    pub fn activate_mission(&mut self, mission: Versioned<MissionId>) -> Result<(), OwnerError> {
        let current = self
            .missions
            .get(&mission.id)
            .map(|registered| registered.generation);
        validate_generation(current, mission.generation)?;
        self.missions.insert(
            mission.id,
            MissionRegistration {
                generation: mission.generation,
                active: true,
            },
        );
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
                active: true,
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
                active: true,
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
        self.missions.get(&mission.id).is_some_and(|registered| {
            registered.active && registered.generation == mission.generation
        })
    }

    fn plan_is_current(&self, mission: Versioned<MissionId>, plan: Versioned<PlanId>) -> bool {
        self.plans.get(&plan.id).is_some_and(|registered| {
            registered.active
                && registered.mission == mission
                && registered.generation == plan.generation
        })
    }

    fn intent_is_current(&self, plan: Versioned<PlanId>, intent: Versioned<IntentId>) -> bool {
        self.intents.get(&intent.id).is_some_and(|registered| {
            registered.active
                && registered.plan == plan
                && registered.generation == intent.generation
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
