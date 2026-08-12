use mclone_core::Vec3d;
use mclone_protocol::{EntityKind, EntityPersistentId, MallardLifeStage};
use mclone_worldgen::prng::SimpleRandomSource;

const CHICKEN_EGG_TIME_MIN: i32 = 6_000;
const CHICKEN_EGG_TIME_RANGE: i32 = 6_000;
const MALLARD_EGG_TIME_MIN: i32 = 8_000;
const MALLARD_EGG_TIME_RANGE: i32 = 8_000;
const MALLARD_FEATHER_TIME_MIN: i32 = 2_400;
const MALLARD_FEATHER_TIME_RANGE: i32 = 2_400;
const MALLARD_CALL_TIME_MIN: i32 = 160;
const MALLARD_CALL_TIME_RANGE: i32 = 320;
pub(crate) const MALLARD_GROWTH_REQUIRED_TICKS: u32 = 2_400;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MallardRuntimeSaveData {
    pub(crate) egg_time: i32,
    pub(crate) age_ticks: u32,
    pub(crate) parents: [Option<EntityPersistentId>; 2],
    pub(crate) feather_time: i32,
    pub(crate) call_time: i32,
}

#[derive(Debug, PartialEq)]
pub(super) enum MobSpeciesState {
    Cow,
    Chicken(ChickenRuntimeState),
    Mallard(MallardRuntimeState),
}

impl MobSpeciesState {
    pub(super) fn from_spawn(kind: EntityKind, random: &mut SimpleRandomSource) -> Self {
        match kind {
            EntityKind::Cow | EntityKind::Mannequin => Self::Cow,
            EntityKind::Chicken => Self::Chicken(ChickenRuntimeState::new(random)),
            EntityKind::Mallard => Self::Mallard(MallardRuntimeState::new(random)),
            EntityKind::DebugCube | EntityKind::Item | EntityKind::MallardNest => {
                debug_assert!(false, "non-mob entities do not use mob species state");
                Self::Cow
            }
        }
    }

    pub(super) fn from_saved(
        kind: EntityKind,
        random: &mut SimpleRandomSource,
        egg_time: Option<i32>,
        mallard: Option<MallardRuntimeSaveData>,
    ) -> Self {
        match kind {
            EntityKind::Cow | EntityKind::Mannequin => Self::Cow,
            EntityKind::Chicken => Self::Chicken(ChickenRuntimeState::from_saved(
                egg_time.unwrap_or_else(|| next_egg_time(random)),
            )),
            EntityKind::Mallard => Self::Mallard(MallardRuntimeState::from_saved(
                mallard.unwrap_or(MallardRuntimeSaveData {
                    egg_time: egg_time.unwrap_or_else(|| next_mallard_egg_time(random)),
                    age_ticks: MALLARD_GROWTH_REQUIRED_TICKS,
                    parents: [None; 2],
                    feather_time: next_mallard_feather_time(random),
                    call_time: next_mallard_call_time(random),
                }),
            )),
            EntityKind::DebugCube | EntityKind::Item | EntityKind::MallardNest => {
                debug_assert!(false, "non-mob entities do not use mob species state");
                Self::Cow
            }
        }
    }

    pub(super) fn ai_step(
        &mut self,
        on_ground: bool,
        delta_movement: &mut Vec3d,
        random: &mut SimpleRandomSource,
    ) {
        match self {
            Self::Cow => {}
            Self::Chicken(chicken) => chicken.ai_step(on_ground, delta_movement, random),
            Self::Mallard(mallard) => mallard.ai_step(),
        }
    }

    pub(super) fn chicken(&self) -> Option<&ChickenRuntimeState> {
        match self {
            Self::Cow => None,
            Self::Chicken(chicken) => Some(chicken),
            Self::Mallard(_) => None,
        }
    }

    pub(super) fn chicken_mut(&mut self) -> Option<&mut ChickenRuntimeState> {
        match self {
            Self::Cow => None,
            Self::Chicken(chicken) => Some(chicken),
            Self::Mallard(_) => None,
        }
    }

    pub(super) fn mallard(&self) -> Option<&MallardRuntimeState> {
        match self {
            Self::Mallard(mallard) => Some(mallard),
            Self::Cow | Self::Chicken(_) => None,
        }
    }

    pub(super) fn mallard_mut(&mut self) -> Option<&mut MallardRuntimeState> {
        match self {
            Self::Mallard(mallard) => Some(mallard),
            Self::Cow | Self::Chicken(_) => None,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct ChickenRuntimeState {
    flap: f32,
    flap_speed: f32,
    old_flap_speed: f32,
    old_flap: f32,
    flapping: f32,
    next_flap: f32,
    egg_time: i32,
    is_chicken_jockey: bool,
    pending_egg_lays: u32,
}

impl ChickenRuntimeState {
    fn new(random: &mut SimpleRandomSource) -> Self {
        Self {
            flap: 0.0,
            flap_speed: 0.0,
            old_flap_speed: 0.0,
            old_flap: 0.0,
            flapping: 1.0,
            next_flap: 1.0,
            egg_time: next_egg_time(random),
            is_chicken_jockey: false,
            pending_egg_lays: 0,
        }
    }

    fn from_saved(egg_time: i32) -> Self {
        Self {
            flap: 0.0,
            flap_speed: 0.0,
            old_flap_speed: 0.0,
            old_flap: 0.0,
            flapping: 1.0,
            next_flap: 1.0,
            egg_time,
            is_chicken_jockey: false,
            pending_egg_lays: 0,
        }
    }

    fn ai_step(
        &mut self,
        on_ground: bool,
        delta_movement: &mut Vec3d,
        random: &mut SimpleRandomSource,
    ) {
        self.old_flap = self.flap;
        self.old_flap_speed = self.flap_speed;
        self.flap_speed += if on_ground { -0.3 } else { 1.2 };
        self.flap_speed = self.flap_speed.clamp(0.0, 1.0);
        if !on_ground && self.flapping < 1.0 {
            self.flapping = 1.0;
        }

        self.flapping *= 0.9;
        if !on_ground && delta_movement.y < 0.0 {
            *delta_movement =
                Vec3d::new(delta_movement.x, delta_movement.y * 0.6, delta_movement.z);
        }

        self.flap += self.flapping * 2.0;
        if !self.is_chicken_jockey {
            self.egg_time -= 1;
            if self.egg_time <= 0 {
                self.pending_egg_lays = self.pending_egg_lays.saturating_add(1);
                self.egg_time = next_egg_time(random);
            }
        }
    }

    #[cfg(test)]
    pub(super) fn flap_speed(&self) -> f32 {
        self.flap_speed
    }

    pub(super) fn egg_time(&self) -> i32 {
        self.egg_time
    }

    #[cfg(test)]
    pub(super) fn pending_egg_lays(&self) -> u32 {
        self.pending_egg_lays
    }

    #[cfg(test)]
    pub(super) fn set_egg_time_for_test(&mut self, egg_time: i32) {
        self.egg_time = egg_time;
    }

    pub(super) fn take_pending_egg_lays(&mut self) -> u32 {
        let pending = self.pending_egg_lays;
        self.pending_egg_lays = 0;
        pending
    }
}

fn next_egg_time(random: &mut SimpleRandomSource) -> i32 {
    random.next_int_bound(CHICKEN_EGG_TIME_RANGE) + CHICKEN_EGG_TIME_MIN
}

#[derive(Debug, PartialEq)]
pub(super) struct MallardRuntimeState {
    egg_time: i32,
    age_ticks: u32,
    parents: [Option<EntityPersistentId>; 2],
    feather_time: i32,
    call_time: i32,
}

impl MallardRuntimeState {
    fn new(random: &mut SimpleRandomSource) -> Self {
        Self {
            egg_time: next_mallard_egg_time(random),
            age_ticks: MALLARD_GROWTH_REQUIRED_TICKS,
            parents: [None; 2],
            feather_time: next_mallard_feather_time(random),
            call_time: next_mallard_call_time(random),
        }
    }

    fn from_saved(saved: MallardRuntimeSaveData) -> Self {
        Self {
            egg_time: saved.egg_time,
            age_ticks: saved.age_ticks,
            parents: saved.parents,
            feather_time: saved.feather_time,
            call_time: saved.call_time,
        }
    }

    fn ai_step(&mut self) {
        self.age_ticks = self
            .age_ticks
            .saturating_add(1)
            .min(MALLARD_GROWTH_REQUIRED_TICKS);
        if self.egg_time > 0 {
            self.egg_time -= 1;
        }
        if self.feather_time > 0 {
            self.feather_time -= 1;
        }
        if self.call_time > 0 {
            self.call_time -= 1;
        }
    }

    pub(super) const fn egg_time(&self) -> i32 {
        self.egg_time
    }

    pub(super) const fn save_data(&self) -> MallardRuntimeSaveData {
        MallardRuntimeSaveData {
            egg_time: self.egg_time,
            age_ticks: self.age_ticks,
            parents: self.parents,
            feather_time: self.feather_time,
            call_time: self.call_time,
        }
    }

    pub(super) const fn life_stage(&self) -> MallardLifeStage {
        if self.age_ticks < MALLARD_GROWTH_REQUIRED_TICKS {
            MallardLifeStage::Duckling
        } else {
            MallardLifeStage::Adult
        }
    }

    pub(super) fn take_due_feather(
        &mut self,
        habitat_suitable: bool,
        random: &mut SimpleRandomSource,
    ) -> bool {
        if self.life_stage() != MallardLifeStage::Adult
            || self.feather_time > 0
            || !habitat_suitable
        {
            return false;
        }
        self.feather_time = next_mallard_feather_time(random);
        true
    }

    pub(super) fn take_due_call(&mut self, random: &mut SimpleRandomSource) -> bool {
        if self.life_stage() != MallardLifeStage::Adult || self.call_time > 0 {
            return false;
        }
        self.call_time = next_mallard_call_time(random);
        true
    }

    pub(super) fn take_due_egg(
        &mut self,
        habitat_suitable: bool,
        random: &mut SimpleRandomSource,
    ) -> u32 {
        if self.egg_time > 0 || !habitat_suitable {
            return 0;
        }
        self.egg_time = next_mallard_egg_time(random);
        1
    }

    #[cfg(test)]
    pub(super) fn set_egg_time_for_test(&mut self, egg_time: i32) {
        self.egg_time = egg_time;
    }

    #[cfg(test)]
    pub(super) fn set_trace_times_for_test(&mut self, feather_time: i32, call_time: i32) {
        self.feather_time = feather_time;
        self.call_time = call_time;
    }
}

fn next_mallard_egg_time(random: &mut SimpleRandomSource) -> i32 {
    random.next_int_bound(MALLARD_EGG_TIME_RANGE) + MALLARD_EGG_TIME_MIN
}

fn next_mallard_feather_time(random: &mut SimpleRandomSource) -> i32 {
    random.next_int_bound(MALLARD_FEATHER_TIME_RANGE) + MALLARD_FEATHER_TIME_MIN
}

fn next_mallard_call_time(random: &mut SimpleRandomSource) -> i32 {
    random.next_int_bound(MALLARD_CALL_TIME_RANGE) + MALLARD_CALL_TIME_MIN
}
