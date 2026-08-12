use mclone_core::Vec3d;

use crate::{EntityId, EntityPersistentId};

pub const MALLARD_FIELD_GUIDE_OBSERVATION_COUNT: u32 = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MallardLifeStage {
    Duckling,
    Adult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MallardSnapshotData {
    pub life_stage: MallardLifeStage,
    pub in_water: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MallardUpdateData {
    pub life_stage: MallardLifeStage,
    pub in_water: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MallardNestSnapshotData {
    pub incubation_progress: u32,
    pub incubation_required: u32,
    pub attended: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MallardNestUpdateData {
    pub incubation_progress: u32,
    pub incubation_required: u32,
    pub attended: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MallardObservationKind {
    Seen,
    HeardCall,
    FoundFeather,
    FoundTrack,
    FoundNest,
    WitnessedHatch,
}

impl MallardObservationKind {
    pub const ALL: [Self; MALLARD_FIELD_GUIDE_OBSERVATION_COUNT as usize] = [
        Self::Seen,
        Self::HeardCall,
        Self::FoundFeather,
        Self::FoundTrack,
        Self::FoundNest,
        Self::WitnessedHatch,
    ];

    pub const fn bit(self) -> u32 {
        1 << match self {
            Self::Seen => 0,
            Self::HeardCall => 1,
            Self::FoundFeather => 2,
            Self::FoundTrack => 3,
            Self::FoundNest => 4,
            Self::WitnessedHatch => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MallardFieldGuideProgress {
    observations: u32,
}

impl MallardFieldGuideProgress {
    pub const KNOWN_MASK: u32 = (1 << MALLARD_FIELD_GUIDE_OBSERVATION_COUNT) - 1;

    pub const fn from_bits_retain(bits: u32) -> Self {
        Self {
            observations: bits & Self::KNOWN_MASK,
        }
    }

    pub const fn bits(self) -> u32 {
        self.observations
    }

    pub const fn contains(self, observation: MallardObservationKind) -> bool {
        self.observations & observation.bit() != 0
    }

    pub fn observe(&mut self, observation: MallardObservationKind) -> bool {
        let before = self.observations;
        self.observations |= observation.bit();
        self.observations != before
    }

    pub const fn discovered_count(self) -> u32 {
        self.observations.count_ones()
    }

    pub const fn is_complete(self) -> bool {
        self.observations == Self::KNOWN_MASK
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MallardCallCue {
    pub source: EntityId,
    pub position: Vec3d,
    pub sequence: u64,
    pub audible_radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MallardTrackCue {
    pub source: EntityPersistentId,
    pub position: Vec3d,
    pub y_rot_degrees: f32,
    pub sequence: u64,
}
