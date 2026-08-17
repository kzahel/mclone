use mclone_core::{BlockPos, Vec3d};

use crate::{EntityId, EntityPersistentId};

pub const RABBIT_FIELD_GUIDE_OBSERVATION_COUNT: u32 = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RabbitLifeStage {
    Kit,
    Adult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RabbitBehavior {
    Idle,
    Hop,
    Dig,
    Emerge,
    Forage,
    Raid,
    Flee,
    EnterBurrow,
    Underground,
    Courtship,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RabbitObservationKind {
    Seen,
    FoundBurrow,
    WitnessedDig,
    WitnessedThresholdUse,
    WitnessedRaid,
    WitnessedFamily,
}

impl RabbitObservationKind {
    pub const ALL: [Self; RABBIT_FIELD_GUIDE_OBSERVATION_COUNT as usize] = [
        Self::Seen,
        Self::FoundBurrow,
        Self::WitnessedDig,
        Self::WitnessedThresholdUse,
        Self::WitnessedRaid,
        Self::WitnessedFamily,
    ];

    pub const fn bit(self) -> u32 {
        1 << match self {
            Self::Seen => 0,
            Self::FoundBurrow => 1,
            Self::WitnessedDig => 2,
            Self::WitnessedThresholdUse => 3,
            Self::WitnessedRaid => 4,
            Self::WitnessedFamily => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RabbitFieldGuideProgress {
    observations: u32,
}

impl RabbitFieldGuideProgress {
    pub const KNOWN_MASK: u32 = (1 << RABBIT_FIELD_GUIDE_OBSERVATION_COUNT) - 1;

    pub const fn from_bits_retain(bits: u32) -> Self {
        Self {
            observations: bits & Self::KNOWN_MASK,
        }
    }

    pub const fn bits(self) -> u32 {
        self.observations
    }

    pub const fn contains(self, observation: RabbitObservationKind) -> bool {
        self.observations & observation.bit() != 0
    }

    pub fn observe(&mut self, observation: RabbitObservationKind) -> bool {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RabbitSoundKind {
    Thump,
    Dig,
    Rustle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RabbitSoundCue {
    pub source: EntityId,
    pub position: Vec3d,
    pub sequence: u64,
    pub audible_radius: f32,
    pub kind: RabbitSoundKind,
}

pub const BEE_FIELD_GUIDE_OBSERVATION_COUNT: u32 = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BeeBehavior {
    Hover,
    FlyToFlower,
    Forage,
    ReturnHome,
    AtNest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BeeObservationKind {
    Seen,
    FoundNest,
    WitnessedForage,
    WitnessedReturn,
    WitnessedPollination,
    CollectedBeeswax,
}

impl BeeObservationKind {
    pub const ALL: [Self; BEE_FIELD_GUIDE_OBSERVATION_COUNT as usize] = [
        Self::Seen,
        Self::FoundNest,
        Self::WitnessedForage,
        Self::WitnessedReturn,
        Self::WitnessedPollination,
        Self::CollectedBeeswax,
    ];

    pub const fn bit(self) -> u32 {
        1 << match self {
            Self::Seen => 0,
            Self::FoundNest => 1,
            Self::WitnessedForage => 2,
            Self::WitnessedReturn => 3,
            Self::WitnessedPollination => 4,
            Self::CollectedBeeswax => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BeeFieldGuideProgress {
    observations: u32,
}

impl BeeFieldGuideProgress {
    pub const KNOWN_MASK: u32 = (1 << BEE_FIELD_GUIDE_OBSERVATION_COUNT) - 1;

    pub const fn from_bits_retain(bits: u32) -> Self {
        Self {
            observations: bits & Self::KNOWN_MASK,
        }
    }

    pub const fn bits(self) -> u32 {
        self.observations
    }

    pub const fn contains(self, observation: BeeObservationKind) -> bool {
        self.observations & observation.bit() != 0
    }

    pub fn observe(&mut self, observation: BeeObservationKind) -> bool {
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
pub struct BeeSoundCue {
    pub source: EntityId,
    pub position: Vec3d,
    pub sequence: u64,
    pub audible_radius: f32,
}

pub const DEER_FIELD_GUIDE_OBSERVATION_COUNT: u32 = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeerSex {
    Female,
    Male,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeerLifeStage {
    Fawn,
    Adult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeerBehavior {
    Idle,
    Walk,
    Graze,
    Drink,
    Alert,
    Flee,
    LieDown,
    Bedded,
    StandUp,
    Hit,
    Fall,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeerSnapshotData {
    pub sex: DeerSex,
    pub life_stage: DeerLifeStage,
    pub antlered: bool,
    pub behavior: DeerBehavior,
    pub health: u8,
    pub max_health: u8,
}

pub type DeerUpdateData = DeerSnapshotData;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeerObservationKind {
    Seen,
    FoundSign,
    WitnessedAlert,
    WitnessedFlee,
    FoundAntler,
    Harvested,
}

impl DeerObservationKind {
    pub const ALL: [Self; DEER_FIELD_GUIDE_OBSERVATION_COUNT as usize] = [
        Self::Seen,
        Self::FoundSign,
        Self::WitnessedAlert,
        Self::WitnessedFlee,
        Self::FoundAntler,
        Self::Harvested,
    ];

    pub const fn bit(self) -> u32 {
        1 << match self {
            Self::Seen => 0,
            Self::FoundSign => 1,
            Self::WitnessedAlert => 2,
            Self::WitnessedFlee => 3,
            Self::FoundAntler => 4,
            Self::Harvested => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeerFieldGuideProgress {
    observations: u32,
}

impl DeerFieldGuideProgress {
    pub const KNOWN_MASK: u32 = (1 << DEER_FIELD_GUIDE_OBSERVATION_COUNT) - 1;

    pub const fn from_bits_retain(bits: u32) -> Self {
        Self {
            observations: bits & Self::KNOWN_MASK,
        }
    }

    pub const fn bits(self) -> u32 {
        self.observations
    }

    pub const fn contains(self, observation: DeerObservationKind) -> bool {
        self.observations & observation.bit() != 0
    }

    pub fn observe(&mut self, observation: DeerObservationKind) -> bool {
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

pub const MALLARD_FIELD_GUIDE_OBSERVATION_COUNT: u32 = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MallardSex {
    Female,
    Male,
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeerSoundKind {
    Contact,
    Alarm,
    Impact,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeerSoundCue {
    pub source: EntityId,
    pub position: Vec3d,
    pub sequence: u64,
    pub audible_radius: f32,
    pub kind: DeerSoundKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SquirrelSex {
    Female,
    Male,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SquirrelLifeStage {
    Kit,
    Adult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SquirrelBehavior {
    Idle,
    Bound,
    Forage,
    Alarm,
    Flee,
    TrunkApproach,
    Climb,
    RefugeEnter,
    RefugeIdle,
    RefugeExit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SquirrelRetainedIntent {
    GroundForage,
    CoverEscape,
    TreeRefuge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SquirrelSnapshotData {
    pub sex: SquirrelSex,
    pub age_ticks: u32,
    pub life_stage: SquirrelLifeStage,
    pub condition: u16,
    pub behavior: SquirrelBehavior,
    pub behavior_epoch: u32,
    pub retained_intent: Option<SquirrelRetainedIntent>,
    pub refuge: Option<BlockPos>,
}

pub type SquirrelUpdateData = SquirrelSnapshotData;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SquirrelSoundKind {
    Alarm,
    Rustle,
    Dig,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SquirrelSoundCue {
    pub source: EntityId,
    pub position: Vec3d,
    pub sequence: u64,
    pub audible_radius: f32,
    pub kind: SquirrelSoundKind,
}
