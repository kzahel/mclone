use mclone_core::Vec3d;
use mclone_protocol::EntityKind;
use mclone_worldgen::prng::SimpleRandomSource;

const CHICKEN_EGG_TIME_MIN: i32 = 6_000;
const CHICKEN_EGG_TIME_RANGE: i32 = 6_000;

#[derive(Debug, PartialEq)]
pub(super) enum MobSpeciesState {
    Cow,
    Chicken(ChickenRuntimeState),
}

impl MobSpeciesState {
    pub(super) fn from_spawn(kind: EntityKind, random: &mut SimpleRandomSource) -> Self {
        match kind {
            EntityKind::Cow => Self::Cow,
            EntityKind::Chicken => Self::Chicken(ChickenRuntimeState::new(random)),
            EntityKind::DebugCube => {
                debug_assert!(false, "debug cubes do not use mob species state");
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
        }
    }

    #[cfg(test)]
    pub(super) fn chicken(&self) -> Option<&ChickenRuntimeState> {
        match self {
            Self::Cow => None,
            Self::Chicken(chicken) => Some(chicken),
        }
    }

    #[cfg(test)]
    pub(super) fn chicken_mut(&mut self) -> Option<&mut ChickenRuntimeState> {
        match self {
            Self::Cow => None,
            Self::Chicken(chicken) => Some(chicken),
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

    #[cfg(test)]
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
}

fn next_egg_time(random: &mut SimpleRandomSource) -> i32 {
    random.next_int_bound(CHICKEN_EGG_TIME_RANGE) + CHICKEN_EGG_TIME_MIN
}
