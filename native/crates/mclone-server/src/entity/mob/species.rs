use mclone_core::{BlockPos, Vec3d};
use mclone_protocol::{
    BeeBehavior, DeerBehavior, DeerLifeStage, DeerSex, DeerSnapshotData, EntityKind,
    EntityPersistentId, MallardLifeStage, RabbitBehavior, RabbitLifeStage,
};
use mclone_worldgen::prng::SimpleRandomSource;

const CHICKEN_EGG_TIME_MIN: i32 = 6_000;
const CHICKEN_EGG_TIME_RANGE: i32 = 6_000;
const MALLARD_EGG_TIME_MIN: i32 = 8_000;
const MALLARD_EGG_TIME_RANGE: i32 = 8_000;
const MALLARD_FEATHER_TIME_MIN: i32 = 2_400;
const MALLARD_FEATHER_TIME_RANGE: i32 = 2_400;
const MALLARD_CALL_TIME_MIN: i32 = 160;
const MALLARD_CALL_TIME_RANGE: i32 = 320;
const DEER_ANTLER_SHED_TIME_MIN: i32 = 36_000;
const DEER_ANTLER_SHED_TIME_RANGE: i32 = 36_000;
pub(crate) const MALLARD_GROWTH_REQUIRED_TICKS: u32 = 2_400;
pub(crate) const RABBIT_GROWTH_REQUIRED_TICKS: u32 = 2_400;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DeerRuntimeSaveData {
    pub(crate) sex: DeerSex,
    pub(crate) life_stage: DeerLifeStage,
    pub(crate) antlered: bool,
    pub(crate) behavior: DeerBehavior,
    pub(crate) behavior_ticks: u32,
    pub(crate) health: u8,
    pub(crate) max_health: u8,
    pub(crate) antler_shed_time: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MallardRuntimeSaveData {
    pub(crate) egg_time: i32,
    pub(crate) age_ticks: u32,
    pub(crate) parents: [Option<EntityPersistentId>; 2],
    pub(crate) feather_time: i32,
    pub(crate) call_time: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BeeRuntimeSaveData {
    pub(crate) home: EntityPersistentId,
    pub(crate) flower: Option<mclone_core::BlockPos>,
    pub(crate) behavior: BeeBehavior,
    pub(crate) behavior_ticks: u32,
    pub(crate) carrying_pollen: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RabbitRuntimeSaveData {
    pub(crate) home: Option<EntityPersistentId>,
    pub(crate) dig_target: Option<BlockPos>,
    pub(crate) life_stage: RabbitLifeStage,
    pub(crate) age_ticks: u32,
    pub(crate) parents: [Option<EntityPersistentId>; 2],
    pub(crate) behavior: RabbitBehavior,
    pub(crate) behavior_ticks: u32,
    pub(crate) health: u8,
    pub(crate) max_health: u8,
    pub(crate) love_ticks: u32,
    pub(crate) breed_cooldown: u32,
    pub(crate) raid_cooldown: u32,
}

#[derive(Debug, PartialEq)]
pub(super) enum MobSpeciesState {
    Cow,
    Chicken(ChickenRuntimeState),
    Mallard(MallardRuntimeState),
    Deer(DeerRuntimeState),
    Bee(BeeRuntimeState),
    Rabbit(RabbitRuntimeState),
}

impl MobSpeciesState {
    pub(super) fn from_spawn(kind: EntityKind, random: &mut SimpleRandomSource) -> Self {
        match kind {
            EntityKind::Cow | EntityKind::Mannequin => Self::Cow,
            EntityKind::Chicken => Self::Chicken(ChickenRuntimeState::new(random)),
            EntityKind::Mallard => Self::Mallard(MallardRuntimeState::new(random)),
            EntityKind::Deer => Self::Deer(DeerRuntimeState::new(random)),
            EntityKind::Bee => {
                debug_assert!(false, "bee spawn requires a durable colony home");
                Self::Bee(BeeRuntimeState::from_saved(BeeRuntimeSaveData {
                    home: EntityPersistentId::new(0, 0),
                    flower: None,
                    behavior: BeeBehavior::Hover,
                    behavior_ticks: 0,
                    carrying_pollen: false,
                }))
            }
            EntityKind::Rabbit => Self::Rabbit(RabbitRuntimeState::new_founder()),
            EntityKind::DebugCube
            | EntityKind::Item
            | EntityKind::MallardNest
            | EntityKind::DeerBed
            | EntityKind::BeeNest
            | EntityKind::BeeHotel => {
                debug_assert!(false, "non-mob entities do not use mob species state");
                Self::Cow
            }
            EntityKind::RabbitBurrow => {
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
        deer: Option<DeerRuntimeSaveData>,
        bee: Option<BeeRuntimeSaveData>,
        rabbit: Option<RabbitRuntimeSaveData>,
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
            EntityKind::Deer => Self::Deer(DeerRuntimeState::from_saved(
                deer.unwrap_or_else(|| DeerRuntimeState::new(random).save_data()),
            )),
            EntityKind::Bee => Self::Bee(BeeRuntimeState::from_saved(bee.unwrap_or(
                BeeRuntimeSaveData {
                    home: EntityPersistentId::new(0, 0),
                    flower: None,
                    behavior: BeeBehavior::Hover,
                    behavior_ticks: 0,
                    carrying_pollen: false,
                },
            ))),
            EntityKind::Rabbit => Self::Rabbit(RabbitRuntimeState::from_saved(
                rabbit.unwrap_or_else(RabbitRuntimeState::founder_save_data),
            )),
            EntityKind::DebugCube
            | EntityKind::Item
            | EntityKind::MallardNest
            | EntityKind::DeerBed
            | EntityKind::BeeNest
            | EntityKind::BeeHotel => {
                debug_assert!(false, "non-mob entities do not use mob species state");
                Self::Cow
            }
            EntityKind::RabbitBurrow => {
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
            Self::Deer(_) => {}
            Self::Bee(_) => {}
            Self::Rabbit(_) => {}
        }
    }

    pub(super) fn chicken(&self) -> Option<&ChickenRuntimeState> {
        match self {
            Self::Cow => None,
            Self::Chicken(chicken) => Some(chicken),
            Self::Mallard(_) => None,
            Self::Deer(_) => None,
            Self::Bee(_) => None,
            Self::Rabbit(_) => None,
        }
    }

    pub(super) fn chicken_mut(&mut self) -> Option<&mut ChickenRuntimeState> {
        match self {
            Self::Cow => None,
            Self::Chicken(chicken) => Some(chicken),
            Self::Mallard(_) => None,
            Self::Deer(_) => None,
            Self::Bee(_) => None,
            Self::Rabbit(_) => None,
        }
    }

    pub(super) fn mallard(&self) -> Option<&MallardRuntimeState> {
        match self {
            Self::Mallard(mallard) => Some(mallard),
            Self::Cow | Self::Chicken(_) | Self::Deer(_) | Self::Bee(_) | Self::Rabbit(_) => None,
        }
    }

    pub(super) fn mallard_mut(&mut self) -> Option<&mut MallardRuntimeState> {
        match self {
            Self::Mallard(mallard) => Some(mallard),
            Self::Cow | Self::Chicken(_) | Self::Deer(_) | Self::Bee(_) | Self::Rabbit(_) => None,
        }
    }

    pub(super) fn deer(&self) -> Option<&DeerRuntimeState> {
        match self {
            Self::Deer(deer) => Some(deer),
            Self::Cow | Self::Chicken(_) | Self::Mallard(_) | Self::Bee(_) | Self::Rabbit(_) => {
                None
            }
        }
    }

    pub(super) fn deer_mut(&mut self) -> Option<&mut DeerRuntimeState> {
        match self {
            Self::Deer(deer) => Some(deer),
            Self::Cow | Self::Chicken(_) | Self::Mallard(_) | Self::Bee(_) | Self::Rabbit(_) => {
                None
            }
        }
    }

    pub(super) fn bee(&self) -> Option<&BeeRuntimeState> {
        match self {
            Self::Bee(bee) => Some(bee),
            Self::Cow | Self::Chicken(_) | Self::Mallard(_) | Self::Deer(_) | Self::Rabbit(_) => {
                None
            }
        }
    }

    pub(super) fn bee_mut(&mut self) -> Option<&mut BeeRuntimeState> {
        match self {
            Self::Bee(bee) => Some(bee),
            Self::Cow | Self::Chicken(_) | Self::Mallard(_) | Self::Deer(_) | Self::Rabbit(_) => {
                None
            }
        }
    }

    pub(super) fn rabbit(&self) -> Option<&RabbitRuntimeState> {
        match self {
            Self::Rabbit(rabbit) => Some(rabbit),
            Self::Cow | Self::Chicken(_) | Self::Mallard(_) | Self::Deer(_) | Self::Bee(_) => None,
        }
    }

    pub(super) fn rabbit_mut(&mut self) -> Option<&mut RabbitRuntimeState> {
        match self {
            Self::Rabbit(rabbit) => Some(rabbit),
            Self::Cow | Self::Chicken(_) | Self::Mallard(_) | Self::Deer(_) | Self::Bee(_) => None,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct RabbitRuntimeState {
    saved: RabbitRuntimeSaveData,
}

impl RabbitRuntimeState {
    const fn founder_save_data() -> RabbitRuntimeSaveData {
        RabbitRuntimeSaveData {
            home: None,
            dig_target: None,
            life_stage: RabbitLifeStage::Adult,
            age_ticks: RABBIT_GROWTH_REQUIRED_TICKS,
            parents: [None; 2],
            behavior: RabbitBehavior::Idle,
            behavior_ticks: 0,
            health: 3,
            max_health: 3,
            love_ticks: 0,
            breed_cooldown: 0,
            raid_cooldown: 0,
        }
    }

    fn new_founder() -> Self {
        Self {
            saved: Self::founder_save_data(),
        }
    }

    pub(super) const fn from_saved(saved: RabbitRuntimeSaveData) -> Self {
        Self { saved }
    }

    pub(super) const fn save_data(&self) -> RabbitRuntimeSaveData {
        self.saved
    }

    pub(super) const fn behavior(&self) -> RabbitBehavior {
        self.saved.behavior
    }

    pub(super) const fn behavior_ticks(&self) -> u32 {
        self.saved.behavior_ticks
    }

    pub(super) const fn home(&self) -> Option<EntityPersistentId> {
        self.saved.home
    }

    pub(super) const fn dig_target(&self) -> Option<BlockPos> {
        self.saved.dig_target
    }

    pub(super) const fn life_stage(&self) -> RabbitLifeStage {
        self.saved.life_stage
    }

    pub(super) const fn is_underground(&self) -> bool {
        matches!(self.saved.behavior, RabbitBehavior::Underground)
    }

    pub(super) const fn is_in_love(&self) -> bool {
        self.saved.love_ticks > 0
    }

    pub(super) fn can_breed(&self) -> bool {
        self.saved.life_stage == RabbitLifeStage::Adult
            && self.saved.love_ticks > 0
            && self.saved.breed_cooldown == 0
            && self.saved.health > 0
    }

    pub(super) const fn can_raid(&self) -> bool {
        self.saved.raid_cooldown == 0 && self.saved.health > 0
    }

    pub(super) fn set_home(&mut self, home: Option<EntityPersistentId>) {
        self.saved.home = home;
    }

    pub(super) fn set_dig_target(&mut self, target: Option<BlockPos>) {
        self.saved.dig_target = target;
    }

    pub(super) fn set_behavior(&mut self, behavior: RabbitBehavior) -> bool {
        if self.saved.behavior == behavior {
            return false;
        }
        self.saved.behavior = behavior;
        self.saved.behavior_ticks = 0;
        true
    }

    pub(super) fn advance_tick(&mut self) {
        self.saved.behavior_ticks = self.saved.behavior_ticks.saturating_add(1);
        self.saved.love_ticks = self.saved.love_ticks.saturating_sub(1);
        self.saved.breed_cooldown = self.saved.breed_cooldown.saturating_sub(1);
        self.saved.raid_cooldown = self.saved.raid_cooldown.saturating_sub(1);
        if self.saved.life_stage == RabbitLifeStage::Kit {
            self.saved.age_ticks = self.saved.age_ticks.saturating_add(1);
            if self.saved.age_ticks >= RABBIT_GROWTH_REQUIRED_TICKS {
                self.saved.life_stage = RabbitLifeStage::Adult;
            }
        }
    }

    pub(super) fn feed(&mut self, love_ticks: u32) -> bool {
        if self.saved.life_stage != RabbitLifeStage::Adult
            || self.saved.health == 0
            || self.saved.breed_cooldown > 0
        {
            return false;
        }
        self.saved.love_ticks = self.saved.love_ticks.max(love_ticks);
        true
    }

    pub(super) fn complete_breeding(&mut self, cooldown: u32) {
        self.saved.love_ticks = 0;
        self.saved.breed_cooldown = cooldown;
        self.set_behavior(RabbitBehavior::Idle);
    }

    pub(super) fn complete_raid(&mut self, cooldown: u32) {
        self.saved.raid_cooldown = cooldown;
        self.set_behavior(RabbitBehavior::Forage);
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct BeeRuntimeState {
    saved: BeeRuntimeSaveData,
}

impl BeeRuntimeState {
    pub(super) const fn from_saved(saved: BeeRuntimeSaveData) -> Self {
        Self { saved }
    }

    pub(super) const fn save_data(&self) -> BeeRuntimeSaveData {
        self.saved
    }

    pub(super) const fn behavior(&self) -> BeeBehavior {
        self.saved.behavior
    }

    pub(super) const fn behavior_ticks(&self) -> u32 {
        self.saved.behavior_ticks
    }

    pub(super) const fn home(&self) -> EntityPersistentId {
        self.saved.home
    }

    pub(super) const fn carrying_pollen(&self) -> bool {
        self.saved.carrying_pollen
    }

    pub(super) const fn flower(&self) -> Option<mclone_core::BlockPos> {
        self.saved.flower
    }

    pub(super) fn set_flower(&mut self, flower: Option<mclone_core::BlockPos>) {
        self.saved.flower = flower;
    }

    pub(super) fn advance_behavior_tick(&mut self) {
        self.saved.behavior_ticks = self.saved.behavior_ticks.saturating_add(1);
    }

    pub(super) fn set_behavior(&mut self, behavior: BeeBehavior) {
        if self.saved.behavior != behavior {
            self.saved.behavior = behavior;
            self.saved.behavior_ticks = 0;
        }
    }

    pub(super) fn set_carrying_pollen(&mut self, carrying: bool) {
        self.saved.carrying_pollen = carrying;
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct DeerRuntimeState {
    saved: DeerRuntimeSaveData,
}

impl DeerRuntimeState {
    fn new(random: &mut SimpleRandomSource) -> Self {
        let life_stage = if random.next_int_bound(5) == 0 {
            DeerLifeStage::Fawn
        } else {
            DeerLifeStage::Adult
        };
        let sex = if random.next_boolean() {
            DeerSex::Female
        } else {
            DeerSex::Male
        };
        let antlered = sex == DeerSex::Male
            && life_stage == DeerLifeStage::Adult
            && random.next_int_bound(3) != 0;
        let max_health = if life_stage == DeerLifeStage::Fawn {
            12
        } else {
            20
        };
        Self {
            saved: DeerRuntimeSaveData {
                sex,
                life_stage,
                antlered,
                behavior: DeerBehavior::Idle,
                behavior_ticks: 0,
                health: max_health,
                max_health,
                antler_shed_time: if antlered {
                    next_deer_antler_shed_time(random)
                } else {
                    -1
                },
            },
        }
    }

    fn from_saved(saved: DeerRuntimeSaveData) -> Self {
        Self { saved }
    }

    pub(super) const fn save_data(&self) -> DeerRuntimeSaveData {
        self.saved
    }

    pub(super) const fn snapshot_data(&self) -> DeerSnapshotData {
        DeerSnapshotData {
            sex: self.saved.sex,
            life_stage: self.saved.life_stage,
            antlered: self.saved.antlered,
            behavior: self.saved.behavior,
            health: self.saved.health,
            max_health: self.saved.max_health,
        }
    }

    pub(super) const fn behavior(&self) -> DeerBehavior {
        self.saved.behavior
    }

    pub(super) const fn health(&self) -> u8 {
        self.saved.health
    }

    pub(super) fn is_ready_for_harvest(&self, fall_ticks: u32) -> bool {
        self.saved.health == 0
            && self.saved.behavior == DeerBehavior::Fall
            && self.saved.behavior_ticks >= fall_ticks
    }

    pub(super) const fn antlered(&self) -> bool {
        self.saved.antlered
    }

    pub(super) fn take_due_antler_shed(&mut self) -> bool {
        if !self.saved.antlered || self.saved.health == 0 {
            return false;
        }
        if self.saved.antler_shed_time > 0 {
            self.saved.antler_shed_time -= 1;
        }
        if self.saved.antler_shed_time != 0 {
            return false;
        }
        self.saved.antlered = false;
        self.saved.antler_shed_time = -1;
        true
    }

    #[cfg(test)]
    pub(super) fn make_antler_shed_due_for_test(&mut self) {
        self.saved.sex = DeerSex::Male;
        self.saved.life_stage = DeerLifeStage::Adult;
        self.saved.antlered = true;
        self.saved.antler_shed_time = 1;
    }

    pub(super) fn apply_damage(&mut self, damage: u8) -> bool {
        if self.saved.health == 0 || damage == 0 {
            return false;
        }
        self.saved.health = self.saved.health.saturating_sub(damage);
        self.set_behavior(if self.saved.health == 0 {
            DeerBehavior::Fall
        } else {
            DeerBehavior::Hit
        });
        true
    }

    pub(super) const fn behavior_ticks(&self) -> u32 {
        self.saved.behavior_ticks
    }

    pub(super) fn advance_behavior_tick(&mut self) {
        self.saved.behavior_ticks = self.saved.behavior_ticks.saturating_add(1);
    }

    pub(super) fn set_behavior(&mut self, behavior: DeerBehavior) -> bool {
        if self.saved.behavior == behavior {
            return false;
        }
        self.saved.behavior = behavior;
        self.saved.behavior_ticks = 0;
        true
    }
}

fn next_deer_antler_shed_time(random: &mut SimpleRandomSource) -> i32 {
    random.next_int_bound(DEER_ANTLER_SHED_TIME_RANGE) + DEER_ANTLER_SHED_TIME_MIN
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
