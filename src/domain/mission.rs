use super::Pos;

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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MissionSpec {
    kind: MissionKind,
    objective: ObjectiveKey,
}

impl MissionSpec {
    pub const fn economy() -> Self {
        Self {
            kind: MissionKind::GatherAndSell,
            objective: ObjectiveKey::EconomyCycle,
        }
    }

    pub const fn construction(site: Pos) -> Self {
        Self {
            kind: MissionKind::Construct,
            objective: ObjectiveKey::ConstructionSite(site),
        }
    }

    pub const fn challenge() -> Self {
        Self {
            kind: MissionKind::Challenge,
            objective: ObjectiveKey::ChallengeSession,
        }
    }

    pub const fn defense(weapon: i64) -> Self {
        Self {
            kind: MissionKind::DefendSector,
            objective: ObjectiveKey::DefenseWeapon(weapon),
        }
    }

    pub const fn kind(self) -> MissionKind {
        self.kind
    }

    pub const fn objective(self) -> ObjectiveKey {
        self.objective
    }
}
