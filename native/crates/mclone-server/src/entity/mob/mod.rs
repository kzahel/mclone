#![allow(dead_code)]

use std::collections::BTreeMap;

use mclone_blocks::{
    BlockFluidKind, block_collision_aabb, block_fluid_kind, block_friction, block_jump_factor,
    block_speed_factor, collide_movement, collide_movement_result,
    collision_aabb_for_feet_position,
};
use mclone_core::{Aabb, AnimationClipId, AnimationState, BlockPos, BlockStateId, Vec3d};
use mclone_protocol::{EntityId, EntityKind, EntityPersistentId};
use mclone_worldgen::block::{
    ACACIA_LEAVES, ACACIA_LOG, BIRCH_LEAVES, BIRCH_LOG, CARROTS_AGE_7, DANDELION, DARK_OAK_LEAVES,
    DARK_OAK_LOG, DIRT, FERN, GRASS, GRASS_BLOCK, JUNGLE_LEAVES, LARGE_FERN_LOWER,
    LARGE_FERN_UPPER, OAK_LEAVES, OAK_LOG, POPPY, SPRUCE_LEAVES, SPRUCE_LOG, TALL_GRASS_LOWER,
    TALL_GRASS_UPPER, generated_block_state_id,
};
use mclone_worldgen::prng::SimpleRandomSource;

#[cfg(test)]
use crate::ecology::WildlifeLifeState;
use crate::ecology::{Availability, KnownPlace, WildlifeLifecycleTuning};

use super::metadata::EntityMetadata;
use super::spawning::habitat::{
    SquirrelRefugeCandidate, find_squirrel_refuge_candidate, sample_wetland_habitat,
    squirrel_refuge_support_is_valid,
};
use super::state::ServerEntityState;

mod attributes;
mod control;
pub(crate) mod goals;
mod navigation;
mod species;

use attributes::MobAttributes;
use control::{JumpControl, LookControl, MoveControl};
use goals::{GoalSelector, passive};
pub(crate) use navigation::BlockPathType;
use navigation::GroundPathNavigation;
use species::MobSpeciesState;
pub(crate) use species::{
    BeeRuntimeSaveData, DeerRuntimeSaveData, MALLARD_GROWTH_REQUIRED_TICKS, MallardRuntimeSaveData,
    RABBIT_GROWTH_REQUIRED_TICKS, RabbitRuntimeSaveData, SquirrelRuntimeSaveData,
    identity_mallard_sex,
};

const PLAYER_EYE_HEIGHT: f64 = 1.62;
const MOB_GRAVITY: f64 = 0.08;
const MOB_VERTICAL_DRAG: f64 = 0.98;
const MOB_COLLISION_EPSILON: f64 = 1.0e-7;
const MOB_INPUT_DAMPING: f64 = 0.98;
const MOB_FRICTION_INFLUENCE_NUMERATOR: f64 = 0.21600002;
const MOB_GROUND_DRAG_MULTIPLIER: f64 = 0.91;
const MOB_AIR_DRAG: f64 = 0.91;
const MOB_FLYING_SPEED: f64 = 0.02;
const MOB_INPUT_EPSILON_SQR: f64 = 1.0e-7;
const MALLARD_HABITAT_SEARCH_RADIUS: i32 = 9;
const MALLARD_MIN_DESTINATION_DISTANCE_SQR: f64 = 2.5 * 2.5;
const MALLARD_INTENT_MIN_TICKS: u16 = 160;
const MALLARD_INTENT_RANDOM_TICKS: i32 = 161;
const MALLARD_TARGET_REACHED_DISTANCE_SQR: f64 = 0.35 * 0.35;
const MALLARD_MAX_TURN_DEGREES: f32 = 12.0;
const DEER_ALERT_RADIUS_SQR: f64 = 16.0 * 16.0;
const DEER_FLEE_RADIUS_SQR: f64 = 10.0 * 10.0;
const DEER_FLEE_RELEASE_RADIUS_SQR: f64 = 26.0 * 26.0;
const DEER_ALERT_MIN_TICKS: u32 = 24;
const DEER_FLEE_MIN_TICKS: u32 = 100;
const DEER_TRANSITION_TICKS: u32 = 20;
const DEER_TARGET_REACHED_DISTANCE_SQR: f64 = 0.65 * 0.65;
const DEER_WALK_SPEED: f64 = 0.055;
const DEER_FLEE_SPEED: f64 = 0.14;
const DEER_WALK_MAX_TURN_DEGREES: f32 = 7.0;
const DEER_FLEE_MAX_TURN_DEGREES: f32 = 18.0;
const DEER_HABITAT_SEARCH_RADIUS: i32 = 9;
const DEER_HERD_COHESION_DISTANCE_SQR: f64 = 12.0 * 12.0;
const DEER_HERD_SEPARATION_DISTANCE_SQR: f64 = 1.75 * 1.75;
const BEE_TARGET_REACHED_DISTANCE_SQR: f64 = 0.28 * 0.28;
const BEE_FLIGHT_SPEED: f64 = 0.075;
const BEE_MAX_TURN_DEGREES: f32 = 20.0;
const BEE_FORAGE_TICKS: u32 = 36;
const BEE_NEST_TICKS: u32 = 24;
const BEE_HOVER_TICKS: u32 = 36;
const BEE_FLOWER_SEARCH_RADIUS: i32 = 22;
const BEE_MIN_FORAGE_DISTANCE_SQR: f64 = 3.5 * 3.5;
const BEE_MAX_TRAVEL_TICKS: u32 = 520;
const BEE_STALL_RECOVERY_TICKS: u16 = 6;
const BEE_PROGRESS_DISTANCE_SQR: f64 = 0.008 * 0.008;
const RABBIT_FLEE_ENTER_RADIUS_SQR: f64 = 8.0 * 8.0;
const RABBIT_FLEE_EXIT_RADIUS_SQR: f64 = 10.0 * 10.0;
const RABBIT_HOME_REACHED_DISTANCE_SQR: f64 = 0.42 * 0.42;
const RABBIT_TARGET_REACHED_DISTANCE_SQR: f64 = 0.45 * 0.45;
const RABBIT_HOP_SPEED: f64 = 0.07;
const RABBIT_FLEE_SPEED: f64 = 0.15;
const RABBIT_DIG_TICKS: u32 = 56;
const RABBIT_ENTRY_TICKS: u32 = 18;
const RABBIT_EMERGE_TICKS: u32 = 18;
const RABBIT_FORAGE_TICKS: u32 = 36;
const RABBIT_RAID_TICKS: u32 = 26;
const RABBIT_UNDERGROUND_MIN_TICKS: u32 = 80;
const RABBIT_ACTIVE_REST_INTERVAL_TICKS: u64 = 480;
const RABBIT_ACTIVE_REST_WARMUP_TICKS: u64 = 60;
const RABBIT_INTENT_TICKS: u16 = 280;
const RABBIT_STALL_TICKS: u16 = 24;
const RABBIT_SEARCH_RADIUS: i32 = 12;
const RABBIT_REFUGE_DISCOVERY_RADIUS_SQR: f64 = 18.0 * 18.0;
const RABBIT_MAX_LOCAL_REFUGES: usize = 3;
const RABBIT_DIG_COOLDOWN_TICKS: u32 = 2_400;
const RABBIT_DECISION_INTERVAL_TICKS: u64 = 10;
const RABBIT_BLOCK_RECONSIDER_RADIUS_SQR: f64 = 14.0 * 14.0;
const SQUIRREL_THREAT_RADIUS_SQR: f64 = 10.0 * 10.0;
const SQUIRREL_ALARM_TICKS: u32 = 12;
const SQUIRREL_COVER_FLEE_TICKS: u32 = 18;
const SQUIRREL_REFUGE_REST_TICKS: u32 = 80;
const SQUIRREL_GROUND_SPEED: f64 = 0.10;
const SQUIRREL_FLEE_SPEED: f64 = 0.18;
const SQUIRREL_CLIMB_SPEED: f64 = 0.09;
pub(crate) const DEER_FALL_PRESENTATION_TICKS: u32 = 30;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PathfindingMalusTable {
    overrides: BTreeMap<BlockPathType, f32>,
}

impl PathfindingMalusTable {
    pub(crate) fn get(&self, path_type: BlockPathType) -> f32 {
        self.overrides
            .get(&path_type)
            .copied()
            .unwrap_or_else(|| path_type.default_malus())
    }

    pub(crate) fn set(&mut self, path_type: BlockPathType, malus: f32) {
        self.overrides.insert(path_type, malus);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MobPlayerTarget {
    pub(crate) position: Vec3d,
    pub(crate) eye_y: f64,
    pub(crate) tempting_carrot: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MallardFlockmateTarget {
    pub(crate) position: Vec3d,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DeerHerdmateTarget {
    pub(crate) position: Vec3d,
    pub(crate) behavior: mclone_protocol::DeerBehavior,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RabbitRefugeCandidate {
    pub(crate) persistent_id: EntityPersistentId,
    pub(crate) position: Vec3d,
    pub(crate) capacity: u8,
    pub(crate) occupancy: u8,
    pub(crate) disturbed: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RabbitEcologyAdmission {
    pub(crate) decision: bool,
    pub(crate) habitat_query: bool,
    pub(crate) path_request: bool,
}

impl RabbitEcologyAdmission {
    const UNBOUNDED: Self = Self {
        decision: true,
        habitat_query: true,
        path_request: true,
    };
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SquirrelEcologyAdmission {
    pub(crate) refuge_query: bool,
}

impl SquirrelEcologyAdmission {
    const UNBOUNDED: Self = Self { refuge_query: true };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MallardHabitatKind {
    Water,
    Shore,
    Nest,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MallardHabitatIntent {
    kind: MallardHabitatKind,
    target: Vec3d,
    ticks_remaining: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DeerHabitatIntent {
    kind: DeerHabitatKind,
    target: Vec3d,
    ticks_remaining: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RabbitIntentKind {
    Dig,
    Escape,
    Forage,
    Raid,
    Home,
    Tempt,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RabbitHabitatIntent {
    kind: RabbitIntentKind,
    target: Vec3d,
    block: Option<BlockPos>,
    ticks_remaining: u16,
    stall_ticks: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeerHabitatKind {
    Forage,
    Water,
    Herd,
    Escape,
}

impl MobPlayerTarget {
    pub(crate) fn from_position(position: Vec3d) -> Self {
        Self {
            position,
            eye_y: position.y + PLAYER_EYE_HEIGHT,
            tempting_carrot: false,
        }
    }

    pub(crate) fn from_position_with_carrot(position: Vec3d, tempting_carrot: bool) -> Self {
        Self {
            position,
            eye_y: position.y + PLAYER_EYE_HEIGHT,
            tempting_carrot,
        }
    }
}

#[derive(Debug)]
pub(crate) struct MobRuntimeState {
    no_action_time: u32,
    on_ground: bool,
    y_body_rot_degrees: f32,
    y_head_rot_degrees: f32,
    attributes: MobAttributes,
    eye_height: f64,
    delta_movement: Vec3d,
    pathfinding_malus: PathfindingMalusTable,
    random: SimpleRandomSource,
    species: MobSpeciesState,
    goal_selector: GoalSelector,
    navigation: GroundPathNavigation,
    move_control: MoveControl,
    jump_control: JumpControl,
    look_control: LookControl,
    mallard_in_water: bool,
    mallard_habitat_intent: Option<MallardHabitatIntent>,
    mallard_seek_shore_next: bool,
    deer_habitat_intent: Option<DeerHabitatIntent>,
    bee_target: Option<Vec3d>,
    bee_route_destination: Option<Vec3d>,
    bee_route_recovery: bool,
    bee_stall_ticks: u16,
    bee_trip_sequence: u32,
    bee_foraging_band: usize,
    bee_last_flower: Option<BlockPos>,
    bee_flower: Option<BlockPos>,
    bee_home_position: Option<Vec3d>,
    bee_completed_deposit: Option<BlockPos>,
    rabbit_refuge: Option<(EntityPersistentId, Vec3d)>,
    rabbit_reserved_refuge: Option<EntityPersistentId>,
    rabbit_habitat_intent: Option<RabbitHabitatIntent>,
    rabbit_escape_attempt: u8,
    rabbit_underground_threat_hold: bool,
    rabbit_completed_dig: Option<BlockPos>,
    rabbit_completed_raid: Option<BlockPos>,
    rabbit_no_dig_site_origin: Option<BlockPos>,
    squirrel_refuge_route: Option<SquirrelRefugeCandidate>,
    squirrel_ground_target: Option<Vec3d>,
}

impl MobRuntimeState {
    #[cfg(test)]
    pub(crate) fn from_spawn(
        id: EntityId,
        metadata: EntityMetadata,
        on_ground: bool,
        y_rot_degrees: f32,
    ) -> Self {
        Self::from_spawn_with_persistent(
            id,
            EntityPersistentId::new(0, id.0),
            metadata,
            on_ground,
            y_rot_degrees,
            WildlifeLifecycleTuning::default(),
        )
    }

    pub(crate) fn from_spawn_with_persistent(
        id: EntityId,
        persistent_id: EntityPersistentId,
        metadata: EntityMetadata,
        on_ground: bool,
        y_rot_degrees: f32,
        wildlife_tuning: WildlifeLifecycleTuning,
    ) -> Self {
        debug_assert!(
            metadata.is_passive_mob(),
            "mob runtime state requires mob metadata"
        );
        let mut pathfinding_malus = PathfindingMalusTable::default();
        if metadata.kind == EntityKind::Chicken {
            pathfinding_malus.set(BlockPathType::Water, 0.0);
        }

        let mut random = SimpleRandomSource::new(mob_random_seed(id, metadata.kind));
        let species =
            MobSpeciesState::from_spawn(metadata.kind, persistent_id, &mut random, wildlife_tuning);

        let mut goal_selector = GoalSelector::default();
        match metadata.kind {
            EntityKind::Cow => passive::register_cow_goals(&mut goal_selector),
            EntityKind::Chicken => passive::register_chicken_goals(&mut goal_selector),
            EntityKind::Mallard => passive::register_mallard_goals(&mut goal_selector),
            EntityKind::Deer => passive::register_cow_goals(&mut goal_selector),
            EntityKind::Bee => {}
            EntityKind::Rabbit => {}
            EntityKind::Squirrel => {}
            EntityKind::Mannequin => passive::register_mannequin_goals(&mut goal_selector),
            EntityKind::DebugCube
            | EntityKind::Item
            | EntityKind::MallardNest
            | EntityKind::DeerBed
            | EntityKind::BeeNest
            | EntityKind::BeeHotel
            | EntityKind::RabbitBurrow
            | EntityKind::SleepingMat => {}
            EntityKind::WildlifeRemains => {}
        }
        let attributes = MobAttributes::from_metadata(metadata);

        Self {
            no_action_time: 0,
            on_ground,
            y_body_rot_degrees: y_rot_degrees,
            y_head_rot_degrees: y_rot_degrees,
            attributes,
            eye_height: metadata.standing_eye_height() as f64,
            delta_movement: Vec3d::new(0.0, -MOB_GRAVITY * MOB_VERTICAL_DRAG, 0.0),
            pathfinding_malus,
            random,
            species,
            goal_selector,
            navigation: GroundPathNavigation::default(),
            move_control: MoveControl::default(),
            jump_control: JumpControl::default(),
            look_control: LookControl::default(),
            mallard_in_water: false,
            mallard_habitat_intent: None,
            mallard_seek_shore_next: false,
            deer_habitat_intent: None,
            bee_target: None,
            bee_route_destination: None,
            bee_route_recovery: false,
            bee_stall_ticks: 0,
            bee_trip_sequence: 0,
            bee_foraging_band: 0,
            bee_last_flower: None,
            bee_flower: None,
            bee_home_position: None,
            bee_completed_deposit: None,
            rabbit_refuge: None,
            rabbit_reserved_refuge: None,
            rabbit_habitat_intent: None,
            rabbit_escape_attempt: 0,
            rabbit_underground_threat_hold: false,
            rabbit_completed_dig: None,
            rabbit_completed_raid: None,
            rabbit_no_dig_site_origin: None,
            squirrel_refuge_route: None,
            squirrel_ground_target: None,
        }
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_saved(
        id: EntityId,
        metadata: EntityMetadata,
        on_ground: bool,
        y_rot_degrees: f32,
        delta_movement: Vec3d,
        egg_time: Option<i32>,
        mallard: Option<MallardRuntimeSaveData>,
        deer: Option<DeerRuntimeSaveData>,
        bee: Option<BeeRuntimeSaveData>,
        rabbit: Option<RabbitRuntimeSaveData>,
    ) -> Self {
        Self::from_saved_with_persistent(
            id,
            EntityPersistentId::new(0, id.0),
            metadata,
            on_ground,
            y_rot_degrees,
            delta_movement,
            egg_time,
            mallard,
            deer,
            bee,
            rabbit,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_saved_with_persistent(
        id: EntityId,
        persistent_id: EntityPersistentId,
        metadata: EntityMetadata,
        on_ground: bool,
        y_rot_degrees: f32,
        delta_movement: Vec3d,
        egg_time: Option<i32>,
        mallard: Option<MallardRuntimeSaveData>,
        deer: Option<DeerRuntimeSaveData>,
        bee: Option<BeeRuntimeSaveData>,
        rabbit: Option<RabbitRuntimeSaveData>,
        squirrel: Option<SquirrelRuntimeSaveData>,
    ) -> Self {
        debug_assert!(
            metadata.is_passive_mob(),
            "mob runtime state requires mob metadata"
        );
        let mut pathfinding_malus = PathfindingMalusTable::default();
        if metadata.kind == EntityKind::Chicken {
            pathfinding_malus.set(BlockPathType::Water, 0.0);
        }

        let mut random = SimpleRandomSource::new(mob_random_seed(id, metadata.kind));
        let bee_flower = bee.and_then(|saved| saved.flower);
        let species = MobSpeciesState::from_saved(
            metadata.kind,
            persistent_id,
            &mut random,
            egg_time,
            mallard,
            deer,
            bee,
            rabbit,
            squirrel,
        );

        let mut goal_selector = GoalSelector::default();
        match metadata.kind {
            EntityKind::Cow => passive::register_cow_goals(&mut goal_selector),
            EntityKind::Chicken => passive::register_chicken_goals(&mut goal_selector),
            EntityKind::Mallard => passive::register_mallard_goals(&mut goal_selector),
            EntityKind::Deer => passive::register_cow_goals(&mut goal_selector),
            EntityKind::Bee => {}
            EntityKind::Rabbit => {}
            EntityKind::Squirrel => {}
            EntityKind::Mannequin => passive::register_mannequin_goals(&mut goal_selector),
            EntityKind::DebugCube
            | EntityKind::Item
            | EntityKind::MallardNest
            | EntityKind::DeerBed
            | EntityKind::BeeNest
            | EntityKind::BeeHotel
            | EntityKind::RabbitBurrow
            | EntityKind::SleepingMat => {}
            EntityKind::WildlifeRemains => {}
        }
        let attributes = MobAttributes::from_metadata(metadata);

        Self {
            no_action_time: 0,
            on_ground,
            y_body_rot_degrees: y_rot_degrees,
            y_head_rot_degrees: y_rot_degrees,
            attributes,
            eye_height: metadata.standing_eye_height() as f64,
            delta_movement,
            pathfinding_malus,
            random,
            species,
            goal_selector,
            navigation: GroundPathNavigation::default(),
            move_control: MoveControl::default(),
            jump_control: JumpControl::default(),
            look_control: LookControl::default(),
            mallard_in_water: false,
            mallard_habitat_intent: None,
            mallard_seek_shore_next: false,
            deer_habitat_intent: None,
            bee_target: None,
            bee_route_destination: None,
            bee_route_recovery: false,
            bee_stall_ticks: 0,
            bee_trip_sequence: 0,
            bee_foraging_band: 0,
            bee_last_flower: None,
            bee_flower,
            bee_home_position: None,
            bee_completed_deposit: None,
            rabbit_refuge: None,
            rabbit_reserved_refuge: None,
            rabbit_habitat_intent: None,
            rabbit_escape_attempt: 0,
            rabbit_underground_threat_hold: false,
            rabbit_completed_dig: None,
            rabbit_completed_raid: None,
            rabbit_no_dig_site_origin: None,
            squirrel_refuge_route: None,
            squirrel_ground_target: None,
        }
    }

    pub(crate) fn sync_from_entity(&mut self, entity: ServerEntityState) {
        self.on_ground = entity.on_ground;
        self.y_body_rot_degrees = entity.y_rot_degrees;
        self.y_head_rot_degrees = entity.y_rot_degrees;
        self.mallard_habitat_intent = None;
        self.deer_habitat_intent = None;
        self.bee_target = None;
        self.bee_route_destination = None;
        self.bee_route_recovery = false;
        self.bee_stall_ticks = 0;
        self.bee_flower = None;
        self.bee_completed_deposit = None;
    }

    pub(crate) fn reject_unavailable_movement(&mut self, entity: ServerEntityState) {
        self.sync_from_entity(entity);
        self.navigation.stop();
        self.rabbit_completed_dig = None;
        self.rabbit_completed_raid = None;
        self.clear_rabbit_shelter_reservation();
    }

    pub(crate) const fn no_action_time(&self) -> u32 {
        self.no_action_time
    }

    pub(crate) const fn on_ground(&self) -> bool {
        self.on_ground
    }

    pub(crate) const fn y_body_rot_degrees(&self) -> f32 {
        self.y_body_rot_degrees
    }

    pub(crate) const fn y_head_rot_degrees(&self) -> f32 {
        self.y_head_rot_degrees
    }

    pub(crate) const fn movement_speed(&self) -> f64 {
        self.attributes.movement_speed
    }

    pub(crate) const fn follow_range(&self) -> f32 {
        self.attributes.follow_range
    }

    pub(crate) const fn max_up_step(&self) -> f64 {
        self.attributes.max_up_step
    }

    pub(crate) const fn jump_power(&self) -> f64 {
        self.attributes.jump_power
    }

    pub(crate) const fn eye_height(&self) -> f64 {
        self.eye_height
    }

    pub(crate) const fn delta_movement(&self) -> Vec3d {
        self.delta_movement
    }

    pub(crate) fn chicken_egg_time(&self) -> Option<i32> {
        self.species.chicken().map(|chicken| chicken.egg_time())
    }

    pub(crate) fn mallard_egg_time(&self) -> Option<i32> {
        self.species.mallard().map(|mallard| mallard.egg_time())
    }

    pub(crate) fn mallard_save_data(&self) -> Option<MallardRuntimeSaveData> {
        self.species.mallard().map(|mallard| mallard.save_data())
    }

    pub(crate) fn mallard_life_stage(&self) -> Option<mclone_protocol::MallardLifeStage> {
        self.species.mallard().map(|mallard| mallard.life_stage())
    }

    pub(crate) fn mallard_sex(&self) -> Option<mclone_protocol::MallardSex> {
        self.species.mallard().map(|mallard| mallard.sex())
    }

    pub(crate) fn mallard_can_nest(&self, threshold: u16) -> bool {
        self.species
            .mallard()
            .is_some_and(|mallard| mallard.can_nest(threshold))
    }

    pub(crate) fn mallard_can_fertilize(&self, threshold: u16) -> bool {
        self.species
            .mallard()
            .is_some_and(|mallard| mallard.can_fertilize(threshold))
    }

    pub(crate) fn spend_mallard_reproduction(&mut self, cost: u16, cooldown: u32) {
        self.species
            .mallard_mut()
            .expect("mallard species")
            .spend_reproduction(cost, cooldown);
    }

    pub(crate) fn mallard_remembered_nest_site(&self) -> Option<BlockPos> {
        self.species
            .mallard()
            .and_then(|mallard| mallard.remembered_nest_site())
    }

    pub(crate) fn set_mallard_remembered_nest_site(&mut self, site: Option<BlockPos>) {
        self.species
            .mallard_mut()
            .expect("mallard species")
            .set_remembered_nest_site(site);
    }

    pub(crate) fn set_mallard_active_nest_target(&mut self, target: BlockPos) {
        self.set_mallard_remembered_nest_site(Some(target));
        self.mallard_habitat_intent = Some(MallardHabitatIntent {
            kind: MallardHabitatKind::Nest,
            target: Vec3d::new(
                f64::from(target.x) + 0.5,
                f64::from(target.y),
                f64::from(target.z) + 0.5,
            ),
            ticks_remaining: u16::MAX,
        });
    }

    pub(crate) fn clear_mallard_active_nest_target(&mut self) {
        if self
            .mallard_habitat_intent
            .is_some_and(|intent| intent.kind == MallardHabitatKind::Nest)
        {
            self.mallard_habitat_intent = None;
        }
    }

    pub(crate) fn mallard_shore_intent(&self) -> Option<BlockPos> {
        self.mallard_habitat_intent.and_then(|intent| {
            (intent.kind == MallardHabitatKind::Shore)
                .then_some(BlockPos::containing(intent.target))
        })
    }

    pub(crate) fn deer_save_data(&self) -> Option<DeerRuntimeSaveData> {
        self.species.deer().map(|deer| deer.save_data())
    }

    pub(crate) fn deer_snapshot_data(&self) -> Option<mclone_protocol::DeerSnapshotData> {
        self.species.deer().map(|deer| deer.snapshot_data())
    }

    pub(crate) fn deer_behavior(&self) -> Option<mclone_protocol::DeerBehavior> {
        self.species.deer().map(|deer| deer.behavior())
    }

    pub(crate) fn bee_save_data(&self) -> Option<BeeRuntimeSaveData> {
        self.species.bee().map(|bee| bee.save_data())
    }

    pub(crate) fn rabbit_save_data(&self) -> Option<RabbitRuntimeSaveData> {
        self.species.rabbit().map(|rabbit| rabbit.save_data())
    }

    pub(crate) fn rabbit_behavior(&self) -> Option<mclone_protocol::RabbitBehavior> {
        self.species.rabbit().map(|rabbit| rabbit.behavior())
    }

    pub(crate) fn rabbit_life_stage(&self) -> Option<mclone_protocol::RabbitLifeStage> {
        self.species.rabbit().map(|rabbit| rabbit.life_stage())
    }

    pub(crate) fn squirrel_save_data(&self) -> Option<SquirrelRuntimeSaveData> {
        self.species.squirrel().map(|squirrel| squirrel.save_data())
    }

    pub(crate) fn squirrel_snapshot_data(&self) -> Option<mclone_protocol::SquirrelSnapshotData> {
        self.species
            .squirrel()
            .map(|squirrel| squirrel.snapshot_data())
    }

    pub(crate) fn squirrel_behavior(&self) -> Option<mclone_protocol::SquirrelBehavior> {
        self.species.squirrel().map(|squirrel| squirrel.behavior())
    }

    pub(crate) fn apply_wildlife_energy_step(
        &mut self,
        intake: u16,
        cost: u16,
        cadence_ticks: u32,
        maximum_energy: u16,
    ) {
        if let Some(rabbit) = self.species.rabbit_mut() {
            rabbit
                .lifecycle_mut()
                .apply_energy_step(intake, cost, cadence_ticks, maximum_energy);
        } else if let Some(deer) = self.species.deer_mut() {
            deer.lifecycle_mut()
                .apply_energy_step(intake, cost, cadence_ticks, maximum_energy);
        } else if let Some(mallard) = self.species.mallard_mut() {
            mallard
                .lifecycle_mut()
                .apply_energy_step(intake, cost, cadence_ticks, maximum_energy);
        } else if let Some(squirrel) = self.species.squirrel_mut() {
            squirrel
                .lifecycle_mut()
                .apply_energy_step(intake, cost, cadence_ticks, maximum_energy);
        }
    }

    pub(crate) fn reconcile_wildlife_maturation(
        &mut self,
        entity: &mut ServerEntityState,
        tuning: WildlifeLifecycleTuning,
    ) {
        if let Some(rabbit) = self.species.rabbit_mut() {
            if rabbit.reconcile_maturation(tuning.rabbit_maturation_ticks)
                && !entity.hidden_from_clients
            {
                entity.width = EntityMetadata::RABBIT.dimensions.width;
                entity.height = EntityMetadata::RABBIT.dimensions.height;
            }
        } else if let Some(deer) = self.species.deer_mut()
            && deer.reconcile_maturation(tuning.deer_maturation_ticks)
        {
            entity.width = EntityMetadata::DEER.dimensions.width;
            entity.height = EntityMetadata::DEER.dimensions.height;
            entity.deer = Some(deer.snapshot_data());
        } else if let Some(mallard) = self.species.mallard_mut()
            && mallard.reconcile_maturation(tuning.mallard_maturation_ticks)
        {
            entity.width = EntityMetadata::MALLARD.dimensions.width;
            entity.height = EntityMetadata::MALLARD.dimensions.height;
            if let Some(snapshot) = entity.mallard.as_mut() {
                snapshot.life_stage = mclone_protocol::MallardLifeStage::Adult;
            }
        } else if let Some(squirrel) = self.species.squirrel_mut()
            && squirrel.reconcile_maturation(tuning.squirrel_maturation_ticks)
        {
            entity.width = EntityMetadata::SQUIRREL.dimensions.width;
            entity.height = EntityMetadata::SQUIRREL.dimensions.height;
            entity.squirrel = Some(squirrel.snapshot_data());
        }
    }

    pub(crate) fn rabbit_enter_natural_love(&mut self, threshold: u16, love_ticks: u32) -> bool {
        self.species
            .rabbit_mut()
            .is_some_and(|rabbit| rabbit.enter_natural_love(threshold, love_ticks))
    }

    pub(crate) fn spend_rabbit_reproduction(&mut self, cost: u16, cooldown: u32) {
        self.species
            .rabbit_mut()
            .expect("rabbit species")
            .spend_reproduction(cost, cooldown);
    }

    pub(crate) fn deer_can_breed(&self, threshold: u16) -> bool {
        self.species
            .deer()
            .is_some_and(|deer| deer.can_breed(threshold))
    }

    pub(crate) fn deer_sex(&self) -> Option<mclone_protocol::DeerSex> {
        self.species.deer().map(|deer| deer.sex())
    }

    pub(crate) fn spend_deer_reproduction(&mut self, cost: u16, cooldown: u32) {
        self.species
            .deer_mut()
            .expect("deer species")
            .spend_reproduction(cost, cooldown);
    }

    pub(crate) fn rabbit_familiar_refuge(&self) -> Option<KnownPlace> {
        self.species
            .rabbit()
            .and_then(|rabbit| rabbit.familiar_refuge())
    }

    pub(crate) fn rabbit_sheltered_in(&self) -> Option<EntityPersistentId> {
        self.species
            .rabbit()
            .and_then(|rabbit| rabbit.sheltered_in())
    }

    pub(crate) fn rabbit_refuge_claim(&self) -> Option<EntityPersistentId> {
        self.rabbit_sheltered_in().or(self.rabbit_reserved_refuge)
    }

    pub(crate) fn rabbit_decision_schedule(&self) -> Option<crate::ecology::DecisionSchedule> {
        self.species
            .rabbit()
            .map(|rabbit| rabbit.decision_schedule())
    }

    pub(crate) fn rabbit_escape_path_needed(
        &self,
        entity: ServerEntityState,
        nearby_players: &[MobPlayerTarget],
    ) -> bool {
        let Some(behavior) = self.rabbit_behavior() else {
            return false;
        };
        if matches!(
            behavior,
            mclone_protocol::RabbitBehavior::EnterBurrow
                | mclone_protocol::RabbitBehavior::Underground
        ) {
            return false;
        }
        let Some(threat) = rabbit_player_threat(
            entity.position,
            behavior,
            self.rabbit_habitat_intent,
            self.rabbit_underground_threat_hold,
            nearby_players,
        ) else {
            return false;
        };
        !self.rabbit_escape_intent_is_usable(entity.position, threat.position)
    }

    pub(crate) fn rabbit_has_player_threat(
        &self,
        entity: ServerEntityState,
        nearby_players: &[MobPlayerTarget],
    ) -> bool {
        self.rabbit_behavior().is_some_and(|behavior| {
            rabbit_player_threat(
                entity.position,
                behavior,
                self.rabbit_habitat_intent,
                self.rabbit_underground_threat_hold,
                nearby_players,
            )
            .is_some()
        })
    }

    pub(crate) fn rabbit_can_breed(&self) -> bool {
        self.species
            .rabbit()
            .is_some_and(|rabbit| rabbit.can_breed())
    }

    pub(crate) fn complete_rabbit_breeding(&mut self, cooldown: u32) {
        let rabbit = self.species.rabbit_mut().expect("rabbit species");
        rabbit.complete_breeding(cooldown);
        rabbit.set_behavior(mclone_protocol::RabbitBehavior::Courtship);
    }

    pub(crate) fn bee_home(&self) -> Option<EntityPersistentId> {
        self.species.bee().map(|bee| bee.home())
    }

    pub(crate) fn set_bee_home_position(&mut self, position: Option<Vec3d>) {
        self.bee_home_position = position;
    }

    pub(crate) fn set_bee_foraging_band(&mut self, band: usize) {
        self.bee_foraging_band = band % 3;
    }

    pub(crate) fn take_bee_completed_deposit(&mut self) -> Option<BlockPos> {
        self.bee_completed_deposit.take()
    }

    pub(crate) fn set_rabbit_refuge(
        &mut self,
        refuge: Option<(EntityPersistentId, Vec3d)>,
        observed_tick: u64,
    ) {
        self.rabbit_refuge = refuge;
        if let Some((persistent_id, position)) = refuge {
            self.species
                .rabbit_mut()
                .expect("rabbit species")
                .confirm_refuge(KnownPlace::observed(
                    persistent_id,
                    BlockPos::containing(position),
                    observed_tick,
                ));
        }
    }

    fn select_rabbit_refuge(
        &mut self,
        persistent_id: EntityPersistentId,
        position: Vec3d,
        observed_tick: u64,
    ) {
        let rabbit = self.species.rabbit_mut().expect("rabbit species");
        rabbit.remember_refuge(KnownPlace::observed(
            persistent_id,
            BlockPos::containing(position),
            observed_tick,
        ));
        self.rabbit_reserved_refuge = Some(persistent_id);
        self.rabbit_refuge = Some((persistent_id, position));
    }

    fn clear_rabbit_shelter_reservation(&mut self) {
        self.rabbit_reserved_refuge = None;
    }

    pub(crate) fn take_rabbit_completed_dig(&mut self) -> Option<BlockPos> {
        self.rabbit_completed_dig.take()
    }

    pub(crate) fn take_rabbit_completed_raid(&mut self) -> Option<BlockPos> {
        self.rabbit_completed_raid.take()
    }

    pub(crate) fn assign_rabbit_refuge(
        &mut self,
        refuge: EntityPersistentId,
        position: Vec3d,
        observed_tick: u64,
    ) {
        let rabbit = self.species.rabbit_mut().expect("rabbit species");
        rabbit.remember_refuge(KnownPlace::observed(
            refuge,
            BlockPos::containing(position),
            observed_tick,
        ));
        rabbit.set_sheltered_in(None);
        rabbit.set_dig_target(None);
        rabbit.set_dig_cooldown(RABBIT_DIG_COOLDOWN_TICKS);
        rabbit.set_behavior(mclone_protocol::RabbitBehavior::Emerge);
        self.rabbit_refuge = Some((refuge, position));
        self.rabbit_reserved_refuge = None;
        self.rabbit_habitat_intent = None;
    }

    pub(crate) fn complete_rabbit_raid(&mut self, cooldown: u32) {
        self.species
            .rabbit_mut()
            .expect("rabbit species")
            .complete_raid(cooldown);
        self.rabbit_habitat_intent = None;
    }

    pub(crate) fn cancel_rabbit_dig(&mut self) -> bool {
        let Some(rabbit) = self.species.rabbit_mut() else {
            return false;
        };
        rabbit.set_dig_target(None);
        rabbit.set_behavior(mclone_protocol::RabbitBehavior::Forage);
        self.rabbit_completed_dig = None;
        self.rabbit_habitat_intent = None;
        self.navigation.stop();
        true
    }

    pub(crate) fn release_rabbit_refuge(
        &mut self,
        entity: &mut ServerEntityState,
        home: mclone_protocol::EntityPersistentId,
        collapsed: bool,
    ) -> bool {
        let Some(rabbit) = self.species.rabbit_mut() else {
            return false;
        };
        if rabbit.sheltered_in() != Some(home) && !collapsed {
            return false;
        }
        if collapsed {
            rabbit.invalidate_refuge(home);
        }
        rabbit.set_sheltered_in(None);
        rabbit.set_dig_target(None);
        rabbit.set_behavior(mclone_protocol::RabbitBehavior::Emerge);
        self.rabbit_refuge = None;
        self.rabbit_reserved_refuge = None;
        self.rabbit_habitat_intent = None;
        self.rabbit_completed_dig = None;
        self.rabbit_completed_raid = None;
        self.navigation.stop();

        let scale = if rabbit.life_stage() == mclone_protocol::RabbitLifeStage::Kit {
            0.62
        } else {
            1.0
        };
        entity.hidden_from_clients = false;
        entity.width = EntityMetadata::RABBIT.dimensions.width * scale;
        entity.height = EntityMetadata::RABBIT.dimensions.height * scale;
        set_rabbit_animation(entity, mclone_protocol::RabbitBehavior::Emerge);
        true
    }

    pub(crate) fn invalidate_rabbit_refuge(&mut self, refuge: EntityPersistentId) -> bool {
        let Some(rabbit) = self.species.rabbit_mut() else {
            return false;
        };
        let changed = rabbit.invalidate_refuge(refuge);
        if self
            .rabbit_refuge
            .is_some_and(|(persistent_id, _)| persistent_id == refuge)
        {
            self.rabbit_refuge = None;
            self.rabbit_reserved_refuge = None;
            self.rabbit_habitat_intent = None;
            self.navigation.stop();
        }
        changed
    }

    pub(crate) fn feed_rabbit(&mut self, love_ticks: u32) -> bool {
        self.species
            .rabbit_mut()
            .is_some_and(|rabbit| rabbit.feed(love_ticks))
    }

    pub(crate) fn damage_deer(&mut self, entity: &mut ServerEntityState, damage: u8) -> bool {
        let Some(deer) = self.species.deer_mut() else {
            return false;
        };
        if !deer.apply_damage(damage) {
            return false;
        }
        set_deer_animation(entity, deer.behavior());
        entity.deer = Some(deer.snapshot_data());
        true
    }

    pub(crate) fn deer_harvest_ready(&self) -> Option<bool> {
        self.species
            .deer()
            .map(|deer| deer.is_ready_for_harvest(DEER_FALL_PRESENTATION_TICKS))
    }

    pub(crate) fn deer_antlered(&self) -> Option<bool> {
        self.species.deer().map(|deer| deer.antlered())
    }

    pub(crate) fn take_deer_due_antler_shed(&mut self) -> bool {
        self.species
            .deer_mut()
            .is_some_and(|deer| deer.take_due_antler_shed())
    }

    #[cfg(test)]
    pub(crate) fn make_deer_antler_shed_due_for_test(&mut self) {
        self.species
            .deer_mut()
            .expect("test expected deer species state")
            .make_antler_shed_due_for_test();
    }

    pub(crate) const fn mallard_in_water(&self) -> bool {
        self.mallard_in_water
    }

    pub(crate) fn available_goal_count(&self) -> usize {
        self.goal_selector.available_goal_count()
    }

    pub(crate) fn running_goal_count(&self) -> usize {
        self.goal_selector.running_goal_count()
    }

    pub(crate) fn navigation_in_progress(&self) -> bool {
        self.navigation.is_in_progress()
    }

    pub(crate) fn navigation_is_stuck(&self) -> bool {
        self.navigation.is_stuck()
    }

    pub(crate) fn pathfinding_malus(&self, path_type: BlockPathType) -> f32 {
        self.pathfinding_malus.get(path_type)
    }

    pub(crate) fn next_random_int_bound(&mut self, bound: i32) -> i32 {
        self.random.next_int_bound(bound)
    }

    pub(crate) fn on_block_changed(&mut self, entity: ServerEntityState, pos: BlockPos) -> bool {
        if entity.kind == EntityKind::Rabbit
            && squared_horizontal_distance(
                entity.position,
                Vec3d::new(
                    f64::from(pos.x) + 0.5,
                    entity.position.y,
                    f64::from(pos.z) + 0.5,
                ),
            ) <= RABBIT_BLOCK_RECONSIDER_RADIUS_SQR
        {
            self.rabbit_no_dig_site_origin = None;
            self.rabbit_habitat_intent = None;
            self.navigation.stop();
            self.species
                .rabbit_mut()
                .expect("rabbit species")
                .wake_decision(entity.tick_count);
            if self.rabbit_behavior() == Some(mclone_protocol::RabbitBehavior::Raid) {
                self.species
                    .rabbit_mut()
                    .expect("rabbit species")
                    .set_behavior(mclone_protocol::RabbitBehavior::Forage);
            }
            return true;
        }
        self.navigation.recompute_path_around(pos, entity.position)
    }

    pub(crate) fn tick_entity<F>(
        &mut self,
        entity: &mut ServerEntityState,
        nearby_players: &[MobPlayerTarget],
        flockmates: &[MallardFlockmateTarget],
        herdmates: &[DeerHerdmateTarget],
        block_state_at: &F,
    ) where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        self.tick_entity_at_time(
            entity,
            nearby_players,
            flockmates,
            herdmates,
            6_000,
            block_state_at,
        );
    }

    pub(crate) fn tick_entity_at_time<F>(
        &mut self,
        entity: &mut ServerEntityState,
        nearby_players: &[MobPlayerTarget],
        flockmates: &[MallardFlockmateTarget],
        herdmates: &[DeerHerdmateTarget],
        day_time: u64,
        block_state_at: &F,
    ) where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        self.tick_entity_at_time_with_ecology(
            entity,
            nearby_players,
            flockmates,
            herdmates,
            day_time,
            &[],
            RabbitEcologyAdmission::UNBOUNDED,
            SquirrelEcologyAdmission::UNBOUNDED,
            block_state_at,
        );
    }

    pub(crate) fn tick_entity_at_time_with_ecology<F>(
        &mut self,
        entity: &mut ServerEntityState,
        nearby_players: &[MobPlayerTarget],
        flockmates: &[MallardFlockmateTarget],
        herdmates: &[DeerHerdmateTarget],
        day_time: u64,
        rabbit_refuges: &[RabbitRefugeCandidate],
        rabbit_admission: RabbitEcologyAdmission,
        squirrel_admission: SquirrelEcologyAdmission,
        block_state_at: &F,
    ) where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        self.on_ground = entity.on_ground;
        self.y_body_rot_degrees = entity.y_rot_degrees;

        if entity.kind == EntityKind::Squirrel {
            self.tick_squirrel(entity, nearby_players, squirrel_admission, block_state_at);
            return;
        }

        if entity.kind == EntityKind::Rabbit {
            self.tick_rabbit(
                entity,
                nearby_players,
                day_time,
                rabbit_refuges,
                rabbit_admission,
                block_state_at,
            );
            return;
        }

        if entity.kind == EntityKind::Deer {
            self.tick_deer(entity, nearby_players, herdmates, block_state_at);
            return;
        }

        if entity.kind == EntityKind::Bee {
            self.tick_bee(entity, block_state_at);
            return;
        }

        if entity.kind == EntityKind::Mallard
            && self.tick_mallard_water_or_shore(entity, flockmates, block_state_at)
        {
            self.species
                .ai_step(self.on_ground, &mut self.delta_movement, &mut self.random);
            return;
        }
        self.mallard_in_water = false;

        let random = std::mem::replace(&mut self.random, SimpleRandomSource::new(0));
        let navigation = std::mem::take(&mut self.navigation);
        let move_control = std::mem::take(&mut self.move_control);
        let jump_control = std::mem::take(&mut self.jump_control);
        let look_control = std::mem::take(&mut self.look_control);
        let mut context = MobGoalContext {
            position: entity.position,
            mob_width: entity.width,
            mob_height: entity.height,
            eye_height: self.eye_height,
            y_body_rot_degrees: self.y_body_rot_degrees,
            y_head_rot_degrees: self.y_head_rot_degrees,
            attributes: self.attributes,
            delta_movement: self.delta_movement,
            no_action_time: self.no_action_time,
            on_ground: self.on_ground,
            nearby_players: nearby_players.to_vec(),
            pathfinding_malus: self.pathfinding_malus.clone(),
            random,
            navigation,
            move_control,
            jump_control,
            look_control,
            block_state_at,
        };

        let mut goal_selector = std::mem::take(&mut self.goal_selector);
        goal_selector.tick(&mut context);
        context.apply_controls(entity);
        self.goal_selector = goal_selector;

        self.no_action_time = context.no_action_time;
        self.on_ground = entity.on_ground;
        self.y_body_rot_degrees = entity.y_rot_degrees;
        self.y_head_rot_degrees = context.y_head_rot_degrees;
        self.delta_movement = context.delta_movement;
        self.random = context.random;
        self.species
            .ai_step(self.on_ground, &mut self.delta_movement, &mut self.random);
        self.navigation = context.navigation;
        self.move_control = context.move_control;
        self.jump_control = context.jump_control;
        self.look_control = context.look_control;
    }

    fn tick_squirrel<F>(
        &mut self,
        entity: &mut ServerEntityState,
        nearby_players: &[MobPlayerTarget],
        admission: SquirrelEcologyAdmission,
        blocks: &F,
    ) where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let Some(squirrel) = self.species.squirrel_mut() else {
            return;
        };
        squirrel.advance_tick();
        let nearest_threat = nearby_players
            .iter()
            .copied()
            .filter(|target| {
                squared_horizontal_distance(entity.position, target.position)
                    <= SQUIRREL_THREAT_RADIUS_SQR
            })
            .min_by(|left, right| {
                squared_horizontal_distance(entity.position, left.position).total_cmp(
                    &squared_horizontal_distance(entity.position, right.position),
                )
            });

        if self
            .squirrel_refuge_route
            .is_some_and(|candidate| !squirrel_refuge_support_is_valid(candidate, blocks))
        {
            self.squirrel_refuge_route = None;
            self.squirrel_ground_target = None;
            entity.on_ground = false;
            let squirrel = self.species.squirrel_mut().expect("squirrel species");
            squirrel.set_refuge(None);
            squirrel
                .set_retained_intent(Some(mclone_protocol::SquirrelRetainedIntent::CoverEscape));
            squirrel.set_behavior(mclone_protocol::SquirrelBehavior::Flee);
        }

        let behavior = self
            .species
            .squirrel()
            .expect("squirrel species")
            .behavior();
        let behavior_ticks = self
            .species
            .squirrel()
            .expect("squirrel species")
            .behavior_ticks();

        match behavior {
            mclone_protocol::SquirrelBehavior::RefugeIdle => {
                if nearest_threat.is_none() && behavior_ticks >= SQUIRREL_REFUGE_REST_TICKS {
                    self.species
                        .squirrel_mut()
                        .expect("squirrel species")
                        .set_behavior(mclone_protocol::SquirrelBehavior::RefugeExit);
                }
            }
            mclone_protocol::SquirrelBehavior::RefugeEnter => {
                if behavior_ticks >= 8 {
                    self.species
                        .squirrel_mut()
                        .expect("squirrel species")
                        .set_behavior(mclone_protocol::SquirrelBehavior::RefugeIdle);
                }
            }
            mclone_protocol::SquirrelBehavior::RefugeExit => {
                if let Some(route) = self.squirrel_refuge_route {
                    if move_squirrel_from_canopy_perch(entity, route, blocks) {
                        entity.on_ground = true;
                        self.species
                            .squirrel_mut()
                            .expect("squirrel species")
                            .set_behavior(mclone_protocol::SquirrelBehavior::Idle);
                    }
                } else {
                    self.species
                        .squirrel_mut()
                        .expect("squirrel species")
                        .set_behavior(mclone_protocol::SquirrelBehavior::Flee);
                }
            }
            mclone_protocol::SquirrelBehavior::Climb => {
                if let Some(route) = self.squirrel_refuge_route {
                    if move_squirrel_to_canopy_perch(entity, route, blocks) {
                        entity.on_ground = true;
                        self.species
                            .squirrel_mut()
                            .expect("squirrel species")
                            .set_behavior(mclone_protocol::SquirrelBehavior::RefugeEnter);
                    }
                }
            }
            mclone_protocol::SquirrelBehavior::TrunkApproach => {
                if let Some(route) = self.squirrel_refuge_route {
                    let target = Vec3d::new(
                        f64::from(route.approach.x) + 0.5,
                        f64::from(route.approach.y),
                        f64::from(route.approach.z) + 0.5,
                    );
                    if squared_horizontal_distance(entity.position, target) <= 0.35 * 0.35 {
                        entity.position.x = target.x;
                        entity.position.z = target.z;
                        self.species
                            .squirrel_mut()
                            .expect("squirrel species")
                            .set_behavior(mclone_protocol::SquirrelBehavior::Climb);
                    } else {
                        move_deer_toward(entity, target, SQUIRREL_FLEE_SPEED, blocks);
                    }
                }
            }
            mclone_protocol::SquirrelBehavior::Alarm => {
                if nearest_threat.is_some() && behavior_ticks >= SQUIRREL_ALARM_TICKS {
                    let route = self.squirrel_refuge_route.or_else(|| {
                        if admission.refuge_query {
                            find_squirrel_refuge_candidate(
                                BlockPos::containing(entity.position),
                                blocks,
                            )
                        } else {
                            None
                        }
                    });
                    if let Some(route) = route {
                        self.squirrel_refuge_route = Some(route);
                        let squirrel = self.species.squirrel_mut().expect("squirrel species");
                        squirrel.set_retained_intent(Some(
                            mclone_protocol::SquirrelRetainedIntent::CoverEscape,
                        ));
                        squirrel.set_refuge(Some(route.refuge));
                        squirrel.set_behavior(mclone_protocol::SquirrelBehavior::Flee);
                    }
                } else if nearest_threat.is_none() {
                    self.species
                        .squirrel_mut()
                        .expect("squirrel species")
                        .set_behavior(mclone_protocol::SquirrelBehavior::Idle);
                }
            }
            mclone_protocol::SquirrelBehavior::Flee => {
                if let Some(route) = self.squirrel_refuge_route {
                    let target = Vec3d::new(
                        f64::from(route.approach.x) + 0.5,
                        f64::from(route.approach.y),
                        f64::from(route.approach.z) + 0.5,
                    );
                    move_deer_toward(entity, target, SQUIRREL_FLEE_SPEED, blocks);
                    if behavior_ticks >= SQUIRREL_COVER_FLEE_TICKS {
                        let squirrel = self.species.squirrel_mut().expect("squirrel species");
                        squirrel.set_retained_intent(Some(
                            mclone_protocol::SquirrelRetainedIntent::TreeRefuge,
                        ));
                        squirrel.set_behavior(mclone_protocol::SquirrelBehavior::TrunkApproach);
                    }
                } else if let Some(threat) = nearest_threat {
                    let dx = entity.position.x - threat.position.x;
                    let dz = entity.position.z - threat.position.z;
                    let length = (dx * dx + dz * dz).sqrt().max(1.0e-6);
                    let target = Vec3d::new(
                        entity.position.x + dx / length * 6.0,
                        entity.position.y,
                        entity.position.z + dz / length * 6.0,
                    );
                    move_deer_toward(entity, target, SQUIRREL_FLEE_SPEED, blocks);
                } else {
                    self.species
                        .squirrel_mut()
                        .expect("squirrel species")
                        .set_behavior(mclone_protocol::SquirrelBehavior::Idle);
                }
            }
            mclone_protocol::SquirrelBehavior::Forage => {
                if nearest_threat.is_some() {
                    self.species
                        .squirrel_mut()
                        .expect("squirrel species")
                        .set_behavior(mclone_protocol::SquirrelBehavior::Alarm);
                } else if behavior_ticks >= 36 {
                    let dx = self.random.next_int_bound(13) - 6;
                    let dz = self.random.next_int_bound(13) - 6;
                    self.squirrel_ground_target = Some(entity.position.add(Vec3d::new(
                        f64::from(dx),
                        0.0,
                        f64::from(dz),
                    )));
                    self.species
                        .squirrel_mut()
                        .expect("squirrel species")
                        .set_behavior(mclone_protocol::SquirrelBehavior::Bound);
                }
            }
            mclone_protocol::SquirrelBehavior::Bound => {
                if nearest_threat.is_some() {
                    self.species
                        .squirrel_mut()
                        .expect("squirrel species")
                        .set_behavior(mclone_protocol::SquirrelBehavior::Alarm);
                } else if let Some(target) = self.squirrel_ground_target {
                    move_deer_toward(entity, target, SQUIRREL_GROUND_SPEED, blocks);
                    if squared_horizontal_distance(entity.position, target) <= 0.35 * 0.35
                        || behavior_ticks >= 100
                    {
                        self.squirrel_ground_target = None;
                        self.species
                            .squirrel_mut()
                            .expect("squirrel species")
                            .set_behavior(mclone_protocol::SquirrelBehavior::Idle);
                    }
                }
            }
            mclone_protocol::SquirrelBehavior::Idle => {
                if nearest_threat.is_some() {
                    self.species
                        .squirrel_mut()
                        .expect("squirrel species")
                        .set_behavior(mclone_protocol::SquirrelBehavior::Alarm);
                } else if behavior_ticks >= 60 {
                    let squirrel = self.species.squirrel_mut().expect("squirrel species");
                    squirrel.set_retained_intent(Some(
                        mclone_protocol::SquirrelRetainedIntent::GroundForage,
                    ));
                    squirrel.set_behavior(mclone_protocol::SquirrelBehavior::Forage);
                }
            }
        }

        if self.squirrel_refuge_route.is_none() && !entity.on_ground {
            let feet = BlockPos::containing(entity.position);
            if blocks(feet).is_some() && blocks(feet.below()).is_some() {
                let requested = Vec3d::new(0.0, -MOB_GRAVITY, 0.0);
                let bounding_box = collision_aabb_for_feet_position(
                    entity.position,
                    f64::from(entity.width),
                    f64::from(entity.height),
                );
                let traveled = collide_movement(blocks, bounding_box, requested);
                entity.position = entity.position.add(traveled);
                entity.on_ground = collide_movement_result(requested, traveled).on_ground;
            }
        }

        let snapshot = self
            .species
            .squirrel()
            .expect("squirrel species")
            .snapshot_data();
        entity.squirrel = Some(snapshot);
        set_squirrel_animation(entity, snapshot.behavior);
        self.on_ground = entity.on_ground;
        self.y_body_rot_degrees = entity.y_rot_degrees;
        self.y_head_rot_degrees = entity.y_rot_degrees;
        self.delta_movement = Vec3d::ZERO;
    }

    pub(crate) fn squirrel_refuge_query_needed(&self) -> bool {
        self.squirrel_refuge_route.is_none()
            && self.species.squirrel().is_some_and(|squirrel| {
                squirrel.behavior() == mclone_protocol::SquirrelBehavior::Alarm
                    && squirrel.behavior_ticks().saturating_add(1) >= SQUIRREL_ALARM_TICKS
            })
    }

    fn tick_rabbit<F>(
        &mut self,
        entity: &mut ServerEntityState,
        nearby_players: &[MobPlayerTarget],
        day_time: u64,
        refuges: &[RabbitRefugeCandidate],
        admission: RabbitEcologyAdmission,
        blocks: &F,
    ) where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let Some(saved) = self.species.rabbit().map(|rabbit| rabbit.save_data()) else {
            return;
        };
        self.species
            .rabbit_mut()
            .expect("rabbit species")
            .advance_tick();
        let nearest_player = nearby_players.iter().copied().min_by(|left, right| {
            entity
                .position
                .distance_to_sqr(left.position)
                .total_cmp(&entity.position.distance_to_sqr(right.position))
        });
        let threat = rabbit_player_threat(
            entity.position,
            saved.behavior,
            self.rabbit_habitat_intent,
            self.rabbit_underground_threat_hold,
            nearby_players,
        );
        let active = rabbit_active_time(day_time);
        let mut next = saved.behavior;
        let ticks = saved.behavior_ticks.saturating_add(1);
        let mut decision_consumed = false;

        if threat.is_none()
            && self
                .rabbit_habitat_intent
                .is_some_and(|intent| intent.kind == RabbitIntentKind::Escape)
        {
            self.rabbit_habitat_intent = None;
            self.rabbit_escape_attempt = 0;
            self.navigation.stop();
        }

        if saved.behavior == mclone_protocol::RabbitBehavior::EnterBurrow {
            self.rabbit_underground_threat_hold = threat.is_some();
            next = if ticks >= RABBIT_ENTRY_TICKS {
                self.species
                    .rabbit_mut()
                    .expect("rabbit species")
                    .set_sheltered_in(self.rabbit_refuge.map(|(id, _)| id));
                self.rabbit_reserved_refuge = None;
                mclone_protocol::RabbitBehavior::Underground
            } else {
                saved.behavior
            };
        } else if saved.behavior == mclone_protocol::RabbitBehavior::Underground {
            self.rabbit_underground_threat_hold = threat.is_some();
            if threat.is_none() && active && ticks >= RABBIT_UNDERGROUND_MIN_TICKS {
                next = mclone_protocol::RabbitBehavior::Emerge;
                if let Some((_, position)) = self.rabbit_refuge {
                    entity.position = position;
                }
                self.rabbit_underground_threat_hold = false;
            }
        } else if let Some(threat) = threat {
            self.rabbit_underground_threat_hold = false;
            next = mclone_protocol::RabbitBehavior::Flee;
            self.clear_rabbit_shelter_reservation();
            if saved.behavior == mclone_protocol::RabbitBehavior::Emerge {
                self.species
                    .rabbit_mut()
                    .expect("rabbit species")
                    .set_sheltered_in(None);
            }
            if saved.behavior == mclone_protocol::RabbitBehavior::Dig
                || self
                    .rabbit_habitat_intent
                    .is_some_and(|intent| intent.kind == RabbitIntentKind::Dig)
            {
                self.species
                    .rabbit_mut()
                    .expect("rabbit species")
                    .set_dig_target(None);
            }
            if !self.rabbit_escape_intent_is_usable(entity.position, threat.position) {
                self.rabbit_habitat_intent = None;
                self.navigation.stop();
            }
            if self.rabbit_habitat_intent.is_none() && admission.path_request {
                self.rabbit_habitat_intent =
                    self.start_rabbit_escape_navigation(*entity, threat.position, blocks);
            }
        } else if saved.behavior == mclone_protocol::RabbitBehavior::Emerge {
            self.rabbit_underground_threat_hold = false;
            if ticks < RABBIT_EMERGE_TICKS {
                next = saved.behavior;
            } else {
                self.species
                    .rabbit_mut()
                    .expect("rabbit species")
                    .set_sheltered_in(None);
                next = mclone_protocol::RabbitBehavior::Idle;
            }
        } else if saved.behavior == mclone_protocol::RabbitBehavior::Dig {
            if ticks >= RABBIT_DIG_TICKS {
                self.rabbit_completed_dig = saved.dig_target;
                next = mclone_protocol::RabbitBehavior::Idle;
            }
        } else if saved.behavior == mclone_protocol::RabbitBehavior::Raid {
            let raid_target = self.rabbit_habitat_intent.filter(|intent| {
                intent.kind == RabbitIntentKind::Raid
                    && intent.block.is_some_and(|block| {
                        blocks(block) == Some(generated_block_state_id(CARROTS_AGE_7))
                    })
            });
            if raid_target.is_none() {
                next = mclone_protocol::RabbitBehavior::Forage;
                self.rabbit_habitat_intent = None;
                self.navigation.stop();
            } else if ticks >= RABBIT_RAID_TICKS {
                self.rabbit_completed_raid = raid_target.and_then(|intent| intent.block);
                next = mclone_protocol::RabbitBehavior::Forage;
                self.rabbit_habitat_intent = None;
            }
        } else if nearest_player.is_some_and(|player| player.tempting_carrot) {
            let player = nearest_player.expect("checked player");
            next = mclone_protocol::RabbitBehavior::Hop;
            self.rabbit_habitat_intent = Some(RabbitHabitatIntent {
                kind: RabbitIntentKind::Tempt,
                target: player.position,
                block: None,
                ticks_remaining: 40,
                stall_ticks: 0,
            });
        } else {
            let shelter_needed =
                !active || rabbit_active_rest_due(*entity, self.rabbit_habitat_intent);
            let continuing_shelter = self.rabbit_habitat_intent.is_some_and(|intent| {
                matches!(intent.kind, RabbitIntentKind::Home | RabbitIntentKind::Dig)
                    && intent.ticks_remaining > 0
            });
            if shelter_needed || continuing_shelter {
                let mut selected = self.rabbit_habitat_intent.filter(|intent| {
                    matches!(intent.kind, RabbitIntentKind::Home | RabbitIntentKind::Dig)
                        && intent.ticks_remaining > 0
                });
                if selected.is_none() && admission.decision {
                    decision_consumed = true;
                    self.navigation.stop();
                    let remembered = resolve_rabbit_refuge(saved, refuges);
                    let candidate = match remembered {
                        Availability::Available(candidate) => Some(candidate),
                        Availability::CurrentlyUnavailable
                        | Availability::ConfirmedUnsuitable
                        | Availability::ConfirmedGone => admission
                            .habitat_query
                            .then(|| nearby_rabbit_refuge(entity.position, saved, refuges))
                            .flatten(),
                    }
                    .or_else(|| {
                        self.rabbit_refuge
                            .filter(|(_, position)| {
                                entity.position.distance_to_sqr(*position)
                                    <= RABBIT_REFUGE_DISCOVERY_RADIUS_SQR
                            })
                            .map(|(persistent_id, position)| RabbitRefugeCandidate {
                                persistent_id,
                                position,
                                capacity: 1,
                                occupancy: u8::from(saved.sheltered_in == Some(persistent_id)),
                                disturbed: false,
                            })
                    });

                    if let Some(candidate) = candidate {
                        self.select_rabbit_refuge(
                            candidate.persistent_id,
                            candidate.position,
                            entity.tick_count,
                        );
                        if entity.position.distance_to_sqr(candidate.position)
                            <= RABBIT_HOME_REACHED_DISTANCE_SQR
                        {
                            next = mclone_protocol::RabbitBehavior::EnterBurrow;
                        } else if admission.path_request
                            && self.start_rabbit_navigation(
                                *entity,
                                candidate.position,
                                RABBIT_HOP_SPEED,
                                blocks,
                            )
                        {
                            selected = Some(RabbitHabitatIntent {
                                kind: RabbitIntentKind::Home,
                                target: candidate.position,
                                block: None,
                                ticks_remaining: RABBIT_INTENT_TICKS,
                                stall_ticks: 0,
                            });
                        } else {
                            self.clear_rabbit_shelter_reservation();
                        }
                    } else if admission.habitat_query
                        && admission.path_request
                        && saved.dig_cooldown == 0
                        && nearby_rabbit_refuge_count(entity.position, refuges)
                            < RABBIT_MAX_LOCAL_REFUGES
                    {
                        if let Some(target) = self.select_rabbit_dig_site(entity.position, blocks) {
                            self.species
                                .rabbit_mut()
                                .expect("rabbit species")
                                .set_dig_target(Some(target));
                            let entrance = rabbit_entrance_position(target, entity.position);
                            if entity.position.distance_to_sqr(entrance)
                                <= RABBIT_TARGET_REACHED_DISTANCE_SQR
                            {
                                next = mclone_protocol::RabbitBehavior::Dig;
                            } else if self.start_rabbit_navigation(
                                *entity,
                                entrance,
                                RABBIT_HOP_SPEED,
                                blocks,
                            ) {
                                selected = Some(RabbitHabitatIntent {
                                    kind: RabbitIntentKind::Dig,
                                    target: entrance,
                                    block: Some(target),
                                    ticks_remaining: RABBIT_INTENT_TICKS,
                                    stall_ticks: 0,
                                });
                            }
                        }
                    }
                    self.rabbit_habitat_intent = selected;
                }
                if let Some(intent) = selected {
                    if entity.position.distance_to_sqr(intent.target)
                        <= RABBIT_TARGET_REACHED_DISTANCE_SQR
                    {
                        self.rabbit_habitat_intent = None;
                        next = if intent.kind == RabbitIntentKind::Home {
                            mclone_protocol::RabbitBehavior::EnterBurrow
                        } else {
                            mclone_protocol::RabbitBehavior::Dig
                        };
                    } else {
                        next = mclone_protocol::RabbitBehavior::Hop;
                    }
                } else if next == saved.behavior {
                    next = if active {
                        mclone_protocol::RabbitBehavior::Forage
                    } else {
                        mclone_protocol::RabbitBehavior::Idle
                    };
                }
            } else {
                if saved.behavior == mclone_protocol::RabbitBehavior::Forage
                    && ticks >= RABBIT_FORAGE_TICKS
                {
                    self.rabbit_habitat_intent = None;
                }
                let target_is_valid = self.rabbit_habitat_intent.is_some_and(|intent| {
                    intent.ticks_remaining > 0
                        && intent.block.is_none_or(|block| {
                            intent.kind != RabbitIntentKind::Raid
                                || blocks(block) == Some(generated_block_state_id(CARROTS_AGE_7))
                        })
                });
                if !target_is_valid && admission.decision {
                    decision_consumed = true;
                    self.navigation.stop();
                    let mut selected = None;
                    if admission.habitat_query
                        && admission.path_request
                        && saved.raid_cooldown == 0
                        && saved.health > 0
                    {
                        for block in rabbit_carrot_targets(entity.position, blocks) {
                            let intent = RabbitHabitatIntent {
                                kind: RabbitIntentKind::Raid,
                                target: Vec3d::new(
                                    f64::from(block.x) + 0.5,
                                    f64::from(block.y),
                                    f64::from(block.z) + 0.5,
                                ),
                                block: Some(block),
                                ticks_remaining: RABBIT_INTENT_TICKS,
                                stall_ticks: 0,
                            };
                            if self.start_rabbit_navigation(
                                *entity,
                                intent.target,
                                RABBIT_HOP_SPEED,
                                blocks,
                            ) {
                                selected = Some(intent);
                                break;
                            }
                        }
                    }
                    if selected.is_none() && admission.path_request {
                        for _ in 0..2 {
                            let Some(intent) = random_rabbit_forage_target(
                                entity.position,
                                &mut self.random,
                                blocks,
                            ) else {
                                break;
                            };
                            if self.start_rabbit_navigation(
                                *entity,
                                intent.target,
                                RABBIT_HOP_SPEED,
                                blocks,
                            ) {
                                selected = Some(intent);
                                break;
                            }
                        }
                    }
                    self.rabbit_habitat_intent = selected;
                }
                if let Some(intent) = self.rabbit_habitat_intent {
                    if entity.position.distance_to_sqr(intent.target)
                        <= RABBIT_TARGET_REACHED_DISTANCE_SQR
                    {
                        next = match intent.kind {
                            RabbitIntentKind::Raid => mclone_protocol::RabbitBehavior::Raid,
                            RabbitIntentKind::Home => {
                                self.rabbit_habitat_intent = None;
                                mclone_protocol::RabbitBehavior::EnterBurrow
                            }
                            RabbitIntentKind::Dig => mclone_protocol::RabbitBehavior::Dig,
                            RabbitIntentKind::Escape => mclone_protocol::RabbitBehavior::Flee,
                            RabbitIntentKind::Forage | RabbitIntentKind::Tempt => {
                                mclone_protocol::RabbitBehavior::Forage
                            }
                        };
                    } else {
                        next = mclone_protocol::RabbitBehavior::Hop;
                    }
                } else {
                    next = mclone_protocol::RabbitBehavior::Idle;
                }
            }
        }

        if decision_consumed {
            let interval = RABBIT_DECISION_INTERVAL_TICKS
                + entity.persistent_id.least % RABBIT_DECISION_INTERVAL_TICKS;
            self.species
                .rabbit_mut()
                .expect("rabbit species")
                .complete_decision(entity.tick_count, interval);
        }

        if next == mclone_protocol::RabbitBehavior::EnterBurrow
            && saved.behavior != mclone_protocol::RabbitBehavior::EnterBurrow
        {
            self.species
                .rabbit_mut()
                .expect("rabbit species")
                .set_sheltered_in(self.rabbit_refuge.map(|(id, _)| id));
            self.rabbit_reserved_refuge = None;
        }

        let behavior_changed = self
            .species
            .rabbit_mut()
            .expect("rabbit species")
            .set_behavior(next);
        if behavior_changed {
            // Intent remains useful across the hop -> forage/raid transition.
        }

        let behavior = self.species.rabbit().expect("rabbit species").behavior();
        if matches!(
            behavior,
            mclone_protocol::RabbitBehavior::Hop | mclone_protocol::RabbitBehavior::Flee
        ) && let Some(mut intent) = self.rabbit_habitat_intent
        {
            let previous = entity.position;
            let navigable = self.move_rabbit_along_navigation(
                entity,
                intent.target,
                if behavior == mclone_protocol::RabbitBehavior::Flee {
                    RABBIT_FLEE_SPEED
                } else {
                    RABBIT_HOP_SPEED
                },
                blocks,
            );
            intent.ticks_remaining = intent.ticks_remaining.saturating_sub(1);
            if !navigable {
                if intent.kind == RabbitIntentKind::Dig {
                    self.species
                        .rabbit_mut()
                        .expect("rabbit species")
                        .set_dig_target(None);
                }
                if intent.kind == RabbitIntentKind::Home {
                    self.clear_rabbit_shelter_reservation();
                }
                self.rabbit_habitat_intent = None;
            } else if squared_horizontal_distance(previous, entity.position) < 1.0e-8 {
                intent.stall_ticks = intent.stall_ticks.saturating_add(1);
                self.rabbit_habitat_intent = (intent.ticks_remaining > 0
                    && intent.stall_ticks < RABBIT_STALL_TICKS)
                    .then_some(intent);
            } else {
                intent.stall_ticks = 0;
                self.rabbit_habitat_intent = (intent.ticks_remaining > 0).then_some(intent);
            }
        } else {
            self.navigation.stop();
        }

        let life_stage = self.species.rabbit().expect("rabbit species").life_stage();
        let scale = if life_stage == mclone_protocol::RabbitLifeStage::Kit {
            0.62
        } else {
            1.0
        };
        entity.hidden_from_clients = behavior == mclone_protocol::RabbitBehavior::Underground;
        if behavior == mclone_protocol::RabbitBehavior::Underground {
            entity.width = 0.001;
            entity.height = 0.001;
        } else {
            entity.width = EntityMetadata::RABBIT.dimensions.width * scale;
            entity.height = EntityMetadata::RABBIT.dimensions.height * scale;
        }
        set_rabbit_animation(entity, behavior);
    }

    fn select_rabbit_dig_site<F>(&mut self, position: Vec3d, blocks: &F) -> Option<BlockPos>
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let origin = BlockPos::containing(position);
        if self.rabbit_no_dig_site_origin == Some(origin) {
            return None;
        }
        let target = select_rabbit_dig_site(position, blocks);
        self.rabbit_no_dig_site_origin = target.is_none().then_some(origin);
        target
    }

    fn rabbit_escape_intent_is_usable(&self, position: Vec3d, threat: Vec3d) -> bool {
        let Some(intent) = self
            .rabbit_habitat_intent
            .filter(|intent| intent.kind == RabbitIntentKind::Escape)
        else {
            return false;
        };
        intent.ticks_remaining > 0
            && position.distance_to_sqr(intent.target) > RABBIT_TARGET_REACHED_DISTANCE_SQR
            && intent.target.distance_to_sqr(threat) > position.distance_to_sqr(threat) + 0.25
            && self.navigation.target_pos() == Some(BlockPos::containing(intent.target))
            && self.navigation.is_in_progress()
            && self.navigation.path_reaches_target()
    }

    fn start_rabbit_escape_navigation<F>(
        &mut self,
        entity: ServerEntityState,
        threat: Vec3d,
        blocks: &F,
    ) -> Option<RabbitHabitatIntent>
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let candidates = rabbit_escape_candidates(entity.position, threat, blocks);
        let candidate = candidates
            .get(usize::from(self.rabbit_escape_attempt) % candidates.len().max(1))
            .copied();
        self.rabbit_escape_attempt = self.rabbit_escape_attempt.wrapping_add(1);
        let target = candidate?;
        self.start_rabbit_navigation(entity, target, RABBIT_FLEE_SPEED, blocks)
            .then_some(RabbitHabitatIntent {
                kind: RabbitIntentKind::Escape,
                target,
                block: None,
                ticks_remaining: RABBIT_INTENT_TICKS,
                stall_ticks: 0,
            })
    }

    fn start_rabbit_navigation<F>(
        &mut self,
        entity: ServerEntityState,
        target: Vec3d,
        speed: f64,
        blocks: &F,
    ) -> bool
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let target_block = BlockPos::containing(target);
        if self.navigation.target_pos() == Some(target_block)
            && self.navigation.is_in_progress()
            && self.navigation.path_reaches_target()
        {
            return true;
        }

        let pathfinding_malus = self.pathfinding_malus.clone();
        let speed_modifier = speed / self.attributes.movement_speed.max(f64::EPSILON);
        let started = self.navigation.move_to_with_reach_range(
            entity.position,
            target,
            speed_modifier,
            entity.width,
            entity.height,
            self.attributes.follow_range,
            self.attributes.max_up_step,
            0,
            blocks,
            |path_type| pathfinding_malus.get(path_type),
        );
        if !started || !self.navigation.path_reaches_target() {
            self.navigation.stop();
            return false;
        }
        true
    }

    fn move_rabbit_along_navigation<F>(
        &mut self,
        entity: &mut ServerEntityState,
        target: Vec3d,
        speed: f64,
        blocks: &F,
    ) -> bool
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        if !self.start_rabbit_navigation(*entity, target, speed, blocks) {
            return false;
        }

        let pathfinding_malus = self.pathfinding_malus.clone();
        let waypoint = self.navigation.tick(
            entity.position,
            entity.width,
            entity.height,
            entity.on_ground,
            self.attributes.follow_range,
            self.attributes.max_up_step,
            self.attributes.movement_speed,
            blocks,
            |path_type| pathfinding_malus.get(path_type),
        );
        if !self.navigation.path_reaches_target() {
            self.navigation.stop();
            return false;
        }

        let steering_target = waypoint.map_or(target, |waypoint| waypoint.position);
        move_rabbit_toward(entity, steering_target, speed, blocks);
        true
    }

    fn tick_deer<F>(
        &mut self,
        entity: &mut ServerEntityState,
        nearby_players: &[MobPlayerTarget],
        herdmates: &[DeerHerdmateTarget],
        block_state_at: &F,
    ) where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let nearest_player = nearby_players.iter().copied().min_by(|left, right| {
            entity
                .position
                .distance_to_sqr(left.position)
                .total_cmp(&entity.position.distance_to_sqr(right.position))
        });
        let nearest_distance_sqr = nearest_player
            .map(|player| entity.position.distance_to_sqr(player.position))
            .unwrap_or(f64::INFINITY);
        let herd_alarm = herdmates.iter().any(|herdmate| {
            herdmate.behavior == mclone_protocol::DeerBehavior::Flee
                && entity.position.distance_to_sqr(herdmate.position) <= 18.0 * 18.0
        });
        let nearest_herdmate = herdmates.iter().copied().min_by(|left, right| {
            entity
                .position
                .distance_to_sqr(left.position)
                .total_cmp(&entity.position.distance_to_sqr(right.position))
        });

        if let Some(intent) = self.deer_habitat_intent.as_mut() {
            intent.ticks_remaining = intent.ticks_remaining.saturating_sub(1);
            if intent.ticks_remaining == 0 {
                self.deer_habitat_intent = None;
            }
        }

        let Some(deer) = self.species.deer_mut() else {
            return;
        };
        deer.advance_behavior_tick();
        let behavior = deer.behavior();
        let ticks = deer.behavior_ticks();
        if deer.health() == 0 {
            deer.set_behavior(mclone_protocol::DeerBehavior::Fall);
            set_deer_animation(entity, deer.behavior());
            self.delta_movement = Vec3d::ZERO;
            return;
        }
        let mut next = behavior;
        if behavior == mclone_protocol::DeerBehavior::Hit && ticks < 8 {
            set_deer_animation(entity, behavior);
            self.delta_movement = Vec3d::ZERO;
            return;
        }
        if behavior == mclone_protocol::DeerBehavior::Hit {
            next = mclone_protocol::DeerBehavior::Flee;
        }
        if behavior != mclone_protocol::DeerBehavior::Hit
            && nearest_distance_sqr <= DEER_FLEE_RADIUS_SQR
        {
            next = mclone_protocol::DeerBehavior::Flee;
        } else if behavior == mclone_protocol::DeerBehavior::Flee {
            if ticks >= DEER_FLEE_MIN_TICKS && nearest_distance_sqr > DEER_FLEE_RELEASE_RADIUS_SQR {
                next = mclone_protocol::DeerBehavior::Alert;
            }
        } else if nearest_distance_sqr <= DEER_ALERT_RADIUS_SQR || herd_alarm {
            if !matches!(
                behavior,
                mclone_protocol::DeerBehavior::Alert | mclone_protocol::DeerBehavior::Flee
            ) {
                next = if matches!(
                    behavior,
                    mclone_protocol::DeerBehavior::Bedded | mclone_protocol::DeerBehavior::LieDown
                ) {
                    mclone_protocol::DeerBehavior::StandUp
                } else {
                    mclone_protocol::DeerBehavior::Alert
                };
            }
        } else {
            next = match behavior {
                mclone_protocol::DeerBehavior::Alert if ticks >= DEER_ALERT_MIN_TICKS => {
                    mclone_protocol::DeerBehavior::Idle
                }
                mclone_protocol::DeerBehavior::LieDown if ticks >= DEER_TRANSITION_TICKS => {
                    mclone_protocol::DeerBehavior::Bedded
                }
                mclone_protocol::DeerBehavior::Bedded if ticks >= 160 => {
                    mclone_protocol::DeerBehavior::StandUp
                }
                mclone_protocol::DeerBehavior::StandUp if ticks >= DEER_TRANSITION_TICKS => {
                    mclone_protocol::DeerBehavior::Idle
                }
                mclone_protocol::DeerBehavior::Graze if ticks >= 100 => {
                    mclone_protocol::DeerBehavior::Idle
                }
                mclone_protocol::DeerBehavior::Drink if ticks >= 80 => {
                    mclone_protocol::DeerBehavior::Idle
                }
                mclone_protocol::DeerBehavior::Idle if ticks >= 80 => {
                    let herd_too_far = nearest_herdmate.is_some_and(|herdmate| {
                        entity.position.distance_to_sqr(herdmate.position)
                            > DEER_HERD_COHESION_DISTANCE_SQR
                    });
                    if herd_too_far {
                        mclone_protocol::DeerBehavior::Walk
                    } else {
                        match self.random.next_int_bound(8) {
                            0 if deer_bedding_site(
                                BlockPos::containing(entity.position),
                                block_state_at,
                            ) =>
                            {
                                mclone_protocol::DeerBehavior::LieDown
                            }
                            1..=4 => mclone_protocol::DeerBehavior::Graze,
                            _ => mclone_protocol::DeerBehavior::Walk,
                        }
                    }
                }
                _ => behavior,
            };
        }
        if deer.set_behavior(next) {
            self.deer_habitat_intent = None;
        }

        let behavior = deer.behavior();
        let movement = match behavior {
            mclone_protocol::DeerBehavior::Walk => {
                if self.deer_habitat_intent.is_none() {
                    self.deer_habitat_intent = nearest_herdmate
                        .and_then(|herdmate| deer_herd_target(entity.position, herdmate.position))
                        .or_else(|| {
                            (self.random.next_int_bound(5) == 0)
                                .then(|| {
                                    random_deer_safe_bank_target(
                                        entity.position,
                                        &mut self.random,
                                        block_state_at,
                                    )
                                })
                                .flatten()
                        })
                        .or_else(|| {
                            random_deer_forage_target(
                                entity.position,
                                &mut self.random,
                                block_state_at,
                            )
                        });
                }
                self.deer_habitat_intent
                    .map(|intent| (intent.target, DEER_WALK_SPEED))
            }
            mclone_protocol::DeerBehavior::Flee => {
                if self.deer_habitat_intent.is_none() {
                    self.deer_habitat_intent = Some(deer_escape_target(
                        entity.position,
                        nearest_player.map(|player| player.position),
                        block_state_at,
                    ));
                }
                self.deer_habitat_intent
                    .map(|intent| (intent.target, DEER_FLEE_SPEED))
            }
            _ => None,
        };

        if let Some((target, speed)) = movement {
            if entity.position.distance_to_sqr(target) <= DEER_TARGET_REACHED_DISTANCE_SQR {
                let reached_kind = self.deer_habitat_intent.map(|intent| intent.kind);
                self.deer_habitat_intent = None;
                if behavior == mclone_protocol::DeerBehavior::Walk {
                    deer.set_behavior(if reached_kind == Some(DeerHabitatKind::Water) {
                        mclone_protocol::DeerBehavior::Drink
                    } else {
                        mclone_protocol::DeerBehavior::Graze
                    });
                }
            } else {
                move_deer_toward(entity, target, speed, block_state_at);
            }
        }
        set_deer_animation(entity, deer.behavior());
        self.on_ground = entity.on_ground;
        self.y_body_rot_degrees = entity.y_rot_degrees;
        self.y_head_rot_degrees = entity.y_rot_degrees;
        self.delta_movement = Vec3d::ZERO;
    }

    #[cfg(test)]
    pub(crate) fn set_deer_behavior_for_test(&mut self, behavior: mclone_protocol::DeerBehavior) {
        self.species
            .deer_mut()
            .expect("test expected deer species state")
            .set_behavior(behavior);
    }

    #[cfg(test)]
    pub(crate) fn set_wildlife_lifecycle_for_test(
        &mut self,
        lifecycle: WildlifeLifeState,
        deer_sex: Option<mclone_protocol::DeerSex>,
    ) {
        if let Some(rabbit) = self.species.rabbit_mut() {
            rabbit.set_lifecycle_for_test(lifecycle);
        } else if let Some(deer) = self.species.deer_mut() {
            deer.set_lifecycle_for_test(
                lifecycle,
                deer_sex.expect("deer lifecycle test requires a sex"),
            );
        } else {
            panic!("wildlife lifecycle test requires rabbit or deer state");
        }
    }

    #[cfg(test)]
    pub(crate) fn set_mallard_lifecycle_for_test(
        &mut self,
        lifecycle: WildlifeLifeState,
        sex: mclone_protocol::MallardSex,
    ) {
        self.species
            .mallard_mut()
            .expect("mallard lifecycle test requires mallard state")
            .set_lifecycle_for_test(lifecycle, sex);
    }

    fn tick_bee<F>(&mut self, entity: &mut ServerEntityState, block_state_at: &F)
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let Some(bee) = self.species.bee_mut() else {
            return;
        };
        bee.advance_behavior_tick();
        let mut behavior = bee.behavior();
        if behavior == mclone_protocol::BeeBehavior::Hover
            && bee.behavior_ticks() >= BEE_HOVER_TICKS + entity.id.0 as u32 % 24
        {
            self.bee_trip_sequence = self.bee_trip_sequence.wrapping_add(1);
            self.bee_flower = select_bee_flower(
                self.bee_home_position.unwrap_or(entity.position),
                self.bee_foraging_band,
                self.bee_last_flower,
                &mut self.random,
                block_state_at,
            );
            bee.set_flower(self.bee_flower);
            if let Some(flower) = self.bee_flower {
                let destination = bee_flower_target(flower);
                let target = bee_cruise_target(
                    entity,
                    destination,
                    self.bee_trip_sequence,
                    false,
                    block_state_at,
                );
                self.bee_route_destination = Some(destination);
                self.bee_route_recovery = target != destination;
                self.bee_stall_ticks = 0;
                self.bee_target = Some(target);
                behavior = mclone_protocol::BeeBehavior::FlyToFlower;
                bee.set_behavior(behavior);
            }
        }
        if behavior == mclone_protocol::BeeBehavior::Forage
            && bee.behavior_ticks() >= BEE_FORAGE_TICKS
        {
            bee.set_carrying_pollen(true);
            behavior = mclone_protocol::BeeBehavior::ReturnHome;
            bee.set_behavior(behavior);
            if let Some(home) = self.bee_home_position.map(bee_home_target) {
                let target =
                    bee_cruise_target(entity, home, self.bee_trip_sequence, true, block_state_at);
                self.bee_route_destination = Some(home);
                self.bee_route_recovery = target != home;
                self.bee_stall_ticks = 0;
                self.bee_target = Some(target);
            }
        }
        if behavior == mclone_protocol::BeeBehavior::AtNest
            && bee.behavior_ticks() >= BEE_NEST_TICKS
        {
            if bee.carrying_pollen() {
                let completed_flower = self
                    .bee_flower
                    .unwrap_or_else(|| BlockPos::containing(entity.position));
                self.bee_completed_deposit = Some(completed_flower);
                self.bee_last_flower = Some(completed_flower);
            }
            bee.set_carrying_pollen(false);
            bee.set_behavior(mclone_protocol::BeeBehavior::Hover);
            behavior = mclone_protocol::BeeBehavior::Hover;
            self.bee_target = self
                .bee_home_position
                .map(|home| bee_hover_target(home, entity.persistent_id, self.bee_trip_sequence));
            self.bee_route_destination = None;
            self.bee_route_recovery = false;
            self.bee_flower = None;
            bee.set_flower(None);
        }

        if matches!(
            behavior,
            mclone_protocol::BeeBehavior::FlyToFlower | mclone_protocol::BeeBehavior::ReturnHome
        ) && bee.behavior_ticks() >= BEE_MAX_TRAVEL_TICKS
        {
            if behavior == mclone_protocol::BeeBehavior::FlyToFlower {
                self.bee_last_flower = self.bee_flower.take();
                bee.set_flower(None);
                bee.set_carrying_pollen(false);
            }
            behavior = mclone_protocol::BeeBehavior::ReturnHome;
            bee.set_behavior(behavior);
            if let Some(home) = self.bee_home_position.map(bee_home_target) {
                let target =
                    bee_cruise_target(entity, home, self.bee_trip_sequence, true, block_state_at);
                self.bee_route_destination = Some(home);
                self.bee_route_recovery = target != home;
                self.bee_stall_ticks = 0;
                self.bee_target = Some(target);
            }
        }

        if matches!(
            behavior,
            mclone_protocol::BeeBehavior::FlyToFlower
                | mclone_protocol::BeeBehavior::ReturnHome
                | mclone_protocol::BeeBehavior::Hover
        ) {
            if self.bee_target.is_none() {
                self.bee_target = match behavior {
                    mclone_protocol::BeeBehavior::FlyToFlower => {
                        self.bee_flower.map(bee_flower_target)
                    }
                    mclone_protocol::BeeBehavior::ReturnHome => {
                        self.bee_home_position.map(bee_home_target)
                    }
                    mclone_protocol::BeeBehavior::Hover => self.bee_home_position.map(|home| {
                        bee_hover_target(home, entity.persistent_id, self.bee_trip_sequence)
                    }),
                    _ => None,
                };
                self.bee_route_destination = matches!(
                    behavior,
                    mclone_protocol::BeeBehavior::FlyToFlower
                        | mclone_protocol::BeeBehavior::ReturnHome
                )
                .then_some(self.bee_target)
                .flatten();
                self.bee_route_recovery = false;
            }
            if let Some(target) = self.bee_target {
                let outcome = bee_move_toward(entity, target, block_state_at);
                self.bee_stall_ticks = if outcome.progressed {
                    0
                } else {
                    self.bee_stall_ticks.saturating_add(1)
                };
                if outcome.reached {
                    if self.bee_route_recovery {
                        self.bee_target = self.bee_route_destination;
                        self.bee_route_recovery = false;
                    } else {
                        match behavior {
                            mclone_protocol::BeeBehavior::FlyToFlower => {
                                bee.set_behavior(mclone_protocol::BeeBehavior::Forage);
                                behavior = mclone_protocol::BeeBehavior::Forage;
                                self.bee_target = None;
                            }
                            mclone_protocol::BeeBehavior::ReturnHome => {
                                bee.set_behavior(mclone_protocol::BeeBehavior::AtNest);
                                behavior = mclone_protocol::BeeBehavior::AtNest;
                                self.bee_target = None;
                            }
                            mclone_protocol::BeeBehavior::Hover => {
                                self.bee_trip_sequence = self.bee_trip_sequence.wrapping_add(1);
                                self.bee_target = self.bee_home_position.map(|home| {
                                    bee_hover_target(
                                        home,
                                        entity.persistent_id,
                                        self.bee_trip_sequence,
                                    )
                                });
                            }
                            _ => {}
                        }
                    }
                    self.bee_stall_ticks = 0;
                } else if self.bee_stall_ticks >= BEE_STALL_RECOVERY_TICKS {
                    self.bee_target = Some(bee_recovery_target(
                        entity,
                        self.bee_route_destination.unwrap_or(target),
                        self.bee_trip_sequence,
                        &mut self.random,
                        block_state_at,
                    ));
                    self.bee_route_recovery = true;
                    self.bee_stall_ticks = 0;
                }
            }
        }
        set_bee_animation(entity, behavior);
        entity.on_ground = false;
        self.on_ground = false;
        self.delta_movement = Vec3d::ZERO;
    }

    fn tick_mallard_water_or_shore<F>(
        &mut self,
        entity: &mut ServerEntityState,
        flockmates: &[MallardFlockmateTarget],
        block_state_at: &F,
    ) -> bool
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let water = mallard_water_occupancy(entity.position, block_state_at);
        if self.mallard_habitat_intent.is_some_and(|intent| {
            intent.ticks_remaining == 0 || !mallard_intent_is_valid(intent, block_state_at)
        }) {
            if self
                .mallard_habitat_intent
                .is_some_and(|intent| intent.kind == MallardHabitatKind::Nest)
            {
                self.set_mallard_remembered_nest_site(None);
            }
            self.mallard_habitat_intent = None;
        }
        if let Some(intent) = self.mallard_habitat_intent.as_mut() {
            intent.ticks_remaining = intent.ticks_remaining.saturating_sub(1);
        }

        if self.mallard_habitat_intent.is_none() {
            self.mallard_habitat_intent =
                self.choose_mallard_habitat_intent(entity.position, water, block_state_at);
        }
        let Some(intent) = self.mallard_habitat_intent else {
            return false;
        };

        let mut steer_x = intent.target.x - entity.position.x;
        let mut steer_z = intent.target.z - entity.position.z;
        let target_distance_sqr = steer_x * steer_x + steer_z * steer_z;
        let target_reached = target_distance_sqr <= MALLARD_TARGET_REACHED_DISTANCE_SQR;
        if target_reached {
            steer_x = 0.0;
            steer_z = 0.0;
            if let Some(intent) = self.mallard_habitat_intent.as_mut() {
                intent.ticks_remaining = intent.ticks_remaining.min(30);
            }
        } else {
            let target_distance = target_distance_sqr.sqrt();
            steer_x /= target_distance;
            steer_z /= target_distance;
        }
        if !target_reached && let Some(flockmate) = nearest_flockmate(entity.position, flockmates) {
            let dx = flockmate.position.x - entity.position.x;
            let dz = flockmate.position.z - entity.position.z;
            let distance_sqr = dx * dx + dz * dz;
            if distance_sqr < 1.5 * 1.5 && distance_sqr > 1.0e-8 {
                let distance = distance_sqr.sqrt();
                steer_x -= dx / distance * 1.5;
                steer_z -= dz / distance * 1.5;
            } else if distance_sqr > 4.0 * 4.0 && distance_sqr < 24.0 * 24.0 {
                let distance = distance_sqr.sqrt();
                steer_x += dx / distance * 0.25;
                steer_z += dz / distance * 0.25;
            }
        }

        let horizontal_length = (steer_x * steer_x + steer_z * steer_z).sqrt();
        let speed = if water.is_some() { 0.045 } else { 0.035 };
        let (move_x, move_z) = if horizontal_length > 1.0e-6 {
            (
                steer_x / horizontal_length * speed,
                steer_z / horizontal_length * speed,
            )
        } else {
            (0.0, 0.0)
        };
        let move_y = water.map_or(-MOB_GRAVITY, |surface_y| {
            (surface_y - entity.position.y).clamp(-0.045, 0.045)
        });
        let requested = Vec3d::new(move_x, move_y, move_z);
        let bounding_box = collision_aabb_for_feet_position(
            entity.position,
            f64::from(entity.width),
            f64::from(entity.height),
        );
        let traveled = collide_mob_movement(
            block_state_at,
            bounding_box,
            requested,
            self.attributes.max_up_step,
            entity.on_ground,
        );
        entity.position = entity.position.add(traveled);
        if traveled.x * traveled.x + traveled.z * traveled.z > 1.0e-8 {
            let wanted_y_rot = (-traveled.x).atan2(traveled.z).to_degrees() as f32;
            entity.y_rot_degrees = rotate_degrees_towards(
                entity.y_rot_degrees,
                wanted_y_rot,
                MALLARD_MAX_TURN_DEGREES,
            );
            self.y_body_rot_degrees = entity.y_rot_degrees;
            self.y_head_rot_degrees = entity.y_rot_degrees;
        }
        self.mallard_in_water = mallard_water_occupancy(entity.position, block_state_at).is_some();
        entity.on_ground =
            !self.mallard_in_water && collide_movement_result(requested, traveled).on_ground;
        self.on_ground = entity.on_ground;
        self.delta_movement = if self.mallard_in_water {
            Vec3d::new(traveled.x * 0.8, traveled.y * 0.6, traveled.z * 0.8)
        } else {
            mob_delta_after_travel(
                requested,
                traveled,
                entity.on_ground,
                mob_block_friction(block_state_at, entity.position),
                mob_block_speed_factor(block_state_at, entity.position),
            )
        };
        true
    }

    fn choose_mallard_habitat_intent<F>(
        &mut self,
        position: Vec3d,
        water: Option<f64>,
        block_state_at: &F,
    ) -> Option<MallardHabitatIntent>
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let preferred_kind = if water.is_some() && self.mallard_seek_shore_next {
            MallardHabitatKind::Shore
        } else {
            MallardHabitatKind::Water
        };
        let preferred_target = match preferred_kind {
            MallardHabitatKind::Water => {
                random_shallow_water_surface(position, &mut self.random, block_state_at)
            }
            MallardHabitatKind::Shore => {
                random_mallard_shore(position, &mut self.random, block_state_at)
            }
            MallardHabitatKind::Nest => None,
        };
        let (kind, target) = if let Some(target) = preferred_target {
            (preferred_kind, target)
        } else if preferred_kind == MallardHabitatKind::Shore {
            (
                MallardHabitatKind::Water,
                random_shallow_water_surface(position, &mut self.random, block_state_at)?,
            )
        } else if let Some(surface_y) = water {
            (
                MallardHabitatKind::Water,
                Vec3d::new(position.x, surface_y, position.z),
            )
        } else {
            return None;
        };

        self.mallard_seek_shore_next = kind == MallardHabitatKind::Water;
        Some(MallardHabitatIntent {
            kind,
            target,
            ticks_remaining: MALLARD_INTENT_MIN_TICKS
                + self.random.next_int_bound(MALLARD_INTENT_RANDOM_TICKS) as u16,
        })
    }

    #[cfg(test)]
    pub(crate) fn move_to_for_test<F>(
        &mut self,
        entity: ServerEntityState,
        target: Vec3d,
        block_state_at: &F,
    ) -> bool
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        let pathfinding_malus = self.pathfinding_malus.clone();
        self.navigation.move_to(
            entity.position,
            target,
            1.0,
            entity.width,
            entity.height,
            self.attributes.follow_range,
            self.attributes.max_up_step,
            block_state_at,
            |path_type| pathfinding_malus.get(path_type),
        )
    }

    #[cfg(test)]
    pub(crate) fn navigation_has_delayed_recomputation(&self) -> bool {
        self.navigation.has_delayed_recomputation()
    }

    #[cfg(test)]
    pub(crate) fn rabbit_escape_target_for_test(&self) -> Option<Vec3d> {
        self.rabbit_habitat_intent
            .filter(|intent| intent.kind == RabbitIntentKind::Escape)
            .map(|intent| intent.target)
    }

    #[cfg(test)]
    pub(crate) fn rabbit_escape_attempts_for_test(&self) -> Option<u8> {
        self.species.rabbit().map(|_| self.rabbit_escape_attempt)
    }

    #[cfg(test)]
    pub(crate) fn chicken_egg_time_for_test(&self) -> Option<i32> {
        self.species.chicken().map(|chicken| chicken.egg_time())
    }

    #[cfg(test)]
    pub(crate) fn set_chicken_egg_time_for_test(&mut self, egg_time: i32) {
        let Some(chicken) = self.species.chicken_mut() else {
            panic!("test expected chicken species state");
        };
        chicken.set_egg_time_for_test(egg_time);
    }

    #[cfg(test)]
    pub(crate) fn chicken_pending_egg_lays_for_test(&self) -> Option<u32> {
        self.species
            .chicken()
            .map(|chicken| chicken.pending_egg_lays())
    }

    #[cfg(test)]
    pub(crate) fn chicken_flap_speed_for_test(&self) -> Option<f32> {
        self.species.chicken().map(|chicken| chicken.flap_speed())
    }

    pub(crate) fn take_chicken_pending_egg_lays(&mut self) -> u32 {
        self.species
            .chicken_mut()
            .map(|chicken| chicken.take_pending_egg_lays())
            .unwrap_or(0)
    }

    pub(crate) fn take_mallard_due_egg(&mut self, habitat_suitable: bool) -> u32 {
        self.species
            .mallard_mut()
            .map(|mallard| mallard.take_due_egg(habitat_suitable, &mut self.random))
            .unwrap_or(0)
    }

    pub(crate) fn take_mallard_due_feather(&mut self, habitat_suitable: bool) -> bool {
        self.species
            .mallard_mut()
            .is_some_and(|mallard| mallard.take_due_feather(habitat_suitable, &mut self.random))
    }

    pub(crate) fn take_mallard_due_call(&mut self) -> bool {
        self.species
            .mallard_mut()
            .is_some_and(|mallard| mallard.take_due_call(&mut self.random))
    }

    #[cfg(test)]
    pub(crate) fn mallard_habitat_intent_for_test(&self) -> Option<(Vec3d, u16)> {
        self.mallard_habitat_intent
            .map(|intent| (intent.target, intent.ticks_remaining))
    }

    #[cfg(test)]
    pub(crate) fn mallard_active_nest_target_for_test(&self) -> Option<BlockPos> {
        self.mallard_habitat_intent.and_then(|intent| {
            (intent.kind == MallardHabitatKind::Nest).then_some(BlockPos::containing(intent.target))
        })
    }

    #[cfg(test)]
    pub(crate) fn set_mallard_egg_time_for_test(&mut self, egg_time: i32) {
        let Some(mallard) = self.species.mallard_mut() else {
            panic!("test expected mallard species state");
        };
        mallard.set_egg_time_for_test(egg_time);
    }

    #[cfg(test)]
    pub(crate) fn set_mallard_trace_times_for_test(&mut self, feather_time: i32, call_time: i32) {
        let Some(mallard) = self.species.mallard_mut() else {
            panic!("test expected mallard species state");
        };
        mallard.set_trace_times_for_test(feather_time, call_time);
    }
}

pub(crate) struct MobGoalContext<'a> {
    position: Vec3d,
    mob_width: f32,
    mob_height: f32,
    eye_height: f64,
    y_body_rot_degrees: f32,
    y_head_rot_degrees: f32,
    attributes: MobAttributes,
    delta_movement: Vec3d,
    no_action_time: u32,
    on_ground: bool,
    nearby_players: Vec<MobPlayerTarget>,
    pathfinding_malus: PathfindingMalusTable,
    random: SimpleRandomSource,
    navigation: GroundPathNavigation,
    move_control: MoveControl,
    jump_control: JumpControl,
    look_control: LookControl,
    block_state_at: &'a dyn Fn(BlockPos) -> Option<BlockStateId>,
}

impl<'a> MobGoalContext<'a> {
    pub(crate) fn position(&self) -> Vec3d {
        self.position
    }

    pub(crate) fn eye_y(&self) -> f64 {
        self.position.y + self.eye_height
    }

    pub(crate) const fn no_action_time(&self) -> u32 {
        self.no_action_time
    }

    pub(crate) const fn is_in_water_or_bubble(&self) -> bool {
        false
    }

    pub(crate) fn random_int_bound(&mut self, bound: i32) -> i32 {
        self.random.next_int_bound(bound)
    }

    pub(crate) fn random_float(&mut self) -> f32 {
        self.random.next_float()
    }

    pub(crate) fn random_double(&mut self) -> f64 {
        self.random.next_double()
    }

    pub(crate) fn nearest_player_within(&self, distance: f64) -> Option<MobPlayerTarget> {
        let max_distance_sqr = distance * distance;
        self.nearby_players
            .iter()
            .copied()
            .filter(|target| self.distance_to_sqr(target.position) <= max_distance_sqr)
            .min_by(|a, b| {
                self.distance_to_sqr(a.position)
                    .total_cmp(&self.distance_to_sqr(b.position))
            })
    }

    pub(crate) fn distance_to_sqr(&self, position: Vec3d) -> f64 {
        self.position.distance_to_sqr(position)
    }

    pub(crate) fn is_navigation_in_progress(&self) -> bool {
        self.navigation.is_in_progress()
    }

    pub(crate) fn move_to(&mut self, position: Vec3d, speed_modifier: f64) -> bool {
        self.navigation.move_to(
            self.position,
            position,
            speed_modifier,
            self.mob_width,
            self.mob_height,
            self.attributes.follow_range,
            self.attributes.max_up_step,
            self.block_state_at,
            |path_type| self.pathfinding_malus.get(path_type),
        )
    }

    pub(crate) fn stop_navigation(&mut self) {
        self.navigation.stop();
        self.move_control.stop();
    }

    pub(crate) fn land_random_pos(
        &mut self,
        horizontal_range: i32,
        vertical_range: i32,
    ) -> Option<Vec3d> {
        navigation::land_random_pos(
            self.position,
            horizontal_range,
            vertical_range,
            &mut self.random,
            &self.navigation,
            self.block_state_at,
            |path_type| self.pathfinding_malus.get(path_type),
        )
    }

    pub(crate) fn default_random_pos(
        &mut self,
        horizontal_range: i32,
        vertical_range: i32,
    ) -> Option<Vec3d> {
        navigation::default_random_pos(
            self.position,
            horizontal_range,
            vertical_range,
            &mut self.random,
            &self.navigation,
            self.block_state_at,
            |path_type| self.pathfinding_malus.get(path_type),
        )
    }

    pub(crate) fn wetland_shore_random_pos(
        &mut self,
        horizontal_range: i32,
        vertical_range: i32,
    ) -> Option<Vec3d> {
        (0..8).find_map(|_| {
            let candidate = self.land_random_pos(horizontal_range, vertical_range)?;
            sample_wetland_habitat(BlockPos::containing(candidate), &mut |pos| {
                (self.block_state_at)(pos)
            })
            .is_ok_and(|sample| sample.suitable())
            .then_some(candidate)
        })
    }

    pub(crate) fn set_look_at(&mut self, position: Vec3d) {
        self.look_control.set_look_at(position);
    }

    pub(crate) fn apply_controls(&mut self, entity: &mut ServerEntityState) -> bool {
        let previous_position = entity.position;
        let previous_y_rot = entity.y_rot_degrees;
        let previous_on_ground = entity.on_ground;

        if let Some(target) = self.navigation.tick(
            self.position,
            entity.width,
            entity.height,
            self.on_ground,
            self.attributes.follow_range,
            self.attributes.max_up_step,
            self.attributes.movement_speed,
            self.block_state_at,
            |path_type| self.pathfinding_malus.get(path_type),
        ) {
            self.move_control
                .set_wanted_position(target.position, target.speed_modifier);
        }

        let move_tick = self.move_control.tick(
            self.position,
            &mut self.y_body_rot_degrees,
            self.attributes.movement_speed,
            self.on_ground,
            self.attributes.max_up_step,
            self.mob_width,
        );
        if move_tick.jump_requested {
            self.jump_control.jump();
        }
        let jumping = self.jump_control.tick();
        let _ = self.look_control.tick(
            self.position,
            &mut self.y_head_rot_degrees,
            self.y_body_rot_degrees,
        );

        let block_jump_factor = mob_block_jump_factor(self.block_state_at, self.position);
        let requested_y = if jumping && self.on_ground {
            self.attributes.jump_power * block_jump_factor
        } else {
            self.delta_movement.y
        };
        let was_on_ground = self.on_ground;
        let movement_block_friction = mob_block_friction(self.block_state_at, self.position);
        let movement_block_speed_factor =
            mob_block_speed_factor(self.block_state_at, self.position);
        let requested = mob_travel_request(
            Vec3d::new(self.delta_movement.x, requested_y, self.delta_movement.z),
            move_tick.speed,
            self.y_body_rot_degrees,
            was_on_ground,
            movement_block_friction,
        );
        let bounding_box = collision_aabb_for_feet_position(
            self.position,
            f64::from(entity.width),
            f64::from(entity.height),
        );
        let traveled = collide_mob_movement(
            self.block_state_at,
            bounding_box,
            requested,
            self.attributes.max_up_step,
            was_on_ground,
        );
        if traveled.length_sqr() > MOB_COLLISION_EPSILON * MOB_COLLISION_EPSILON {
            self.position = self.position.add(traveled);
        }
        let collision = collide_movement_result(requested, traveled);
        self.on_ground = collision.on_ground;
        self.delta_movement = mob_delta_after_travel(
            requested,
            traveled,
            was_on_ground,
            movement_block_friction,
            movement_block_speed_factor,
        );

        entity.position = self.position;
        entity.y_rot_degrees = self.y_body_rot_degrees;
        entity.x_rot_degrees = 0.0;
        entity.on_ground = self.on_ground;

        entity.position != previous_position
            || entity.y_rot_degrees != previous_y_rot
            || entity.on_ground != previous_on_ground
    }

    #[cfg(test)]
    pub(crate) fn from_parts_for_test(
        entity: ServerEntityState,
        eye_height: f64,
        nearby_players: Vec<MobPlayerTarget>,
        random: SimpleRandomSource,
        block_state_at: &'static dyn Fn(BlockPos) -> Option<BlockStateId>,
    ) -> Self {
        let metadata =
            EntityMetadata::for_kind(entity.kind).expect("test mob context requires mob metadata");
        Self {
            position: entity.position,
            mob_width: entity.width,
            mob_height: entity.height,
            eye_height,
            y_body_rot_degrees: entity.y_rot_degrees,
            y_head_rot_degrees: entity.y_rot_degrees,
            attributes: MobAttributes::from_metadata(metadata),
            delta_movement: Vec3d::new(0.0, -MOB_GRAVITY * MOB_VERTICAL_DRAG, 0.0),
            no_action_time: 0,
            on_ground: entity.on_ground,
            nearby_players,
            pathfinding_malus: PathfindingMalusTable::default(),
            random,
            navigation: GroundPathNavigation::default(),
            move_control: MoveControl::default(),
            jump_control: JumpControl::default(),
            look_control: LookControl::default(),
            block_state_at,
        }
    }
}

fn mob_travel_request(
    delta_movement: Vec3d,
    control_speed: f64,
    y_body_rot_degrees: f32,
    on_ground: bool,
    block_friction: f64,
) -> Vec3d {
    if control_speed == 0.0 {
        return delta_movement;
    }

    let travel_input = Vec3d::new(0.0, 0.0, control_speed * MOB_INPUT_DAMPING);
    let friction_influenced_speed = if on_ground {
        control_speed * (MOB_FRICTION_INFLUENCE_NUMERATOR / block_friction.powi(3))
    } else {
        MOB_FLYING_SPEED
    };
    delta_movement.add(relative_movement_input(
        travel_input,
        friction_influenced_speed,
        y_body_rot_degrees,
    ))
}

fn relative_movement_input(input: Vec3d, scale: f64, y_rot_degrees: f32) -> Vec3d {
    let length_sqr = input.length_sqr();
    if length_sqr < MOB_INPUT_EPSILON_SQR {
        return Vec3d::ZERO;
    }

    let normalized = if length_sqr > 1.0 {
        input.scale(1.0 / length_sqr.sqrt())
    } else {
        input
    };
    let scaled = normalized.scale(scale);
    let radians = (y_rot_degrees as f64).to_radians();
    let sin = radians.sin();
    let cos = radians.cos();
    Vec3d::new(
        scaled.x * cos - scaled.z * sin,
        scaled.y,
        scaled.z * cos + scaled.x * sin,
    )
}

fn mob_delta_after_travel(
    requested: Vec3d,
    traveled: Vec3d,
    was_on_ground: bool,
    block_friction: f64,
    block_speed_factor: f64,
) -> Vec3d {
    let horizontal_drag = if was_on_ground {
        block_friction * MOB_GROUND_DRAG_MULTIPLIER
    } else {
        MOB_AIR_DRAG
    };
    let moved_x = if nearly_equal(requested.x, traveled.x) {
        traveled.x * block_speed_factor
    } else {
        0.0
    };
    let moved_z = if nearly_equal(requested.z, traveled.z) {
        traveled.z * block_speed_factor
    } else {
        0.0
    };

    Vec3d::new(
        moved_x * horizontal_drag,
        (traveled.y - MOB_GRAVITY) * MOB_VERTICAL_DRAG,
        moved_z * horizontal_drag,
    )
}

fn mob_block_friction(
    block_state_at: &dyn Fn(BlockPos) -> Option<BlockStateId>,
    position: Vec3d,
) -> f64 {
    f64::from(block_friction(block_state_or_air(
        block_state_at,
        block_pos_below_that_affects_movement(position),
    )))
}

fn mob_block_speed_factor(
    block_state_at: &dyn Fn(BlockPos) -> Option<BlockStateId>,
    position: Vec3d,
) -> f64 {
    let current = block_state_or_air(block_state_at, BlockPos::containing(position));
    let current_factor = block_speed_factor(current);
    if block_fluid_kind(current) == BlockFluidKind::Water {
        return f64::from(current_factor);
    }

    if current_factor == mclone_blocks::DEFAULT_BLOCK_SPEED_FACTOR {
        f64::from(block_speed_factor(block_state_or_air(
            block_state_at,
            block_pos_below_that_affects_movement(position),
        )))
    } else {
        f64::from(current_factor)
    }
}

fn mob_block_jump_factor(
    block_state_at: &dyn Fn(BlockPos) -> Option<BlockStateId>,
    position: Vec3d,
) -> f64 {
    let current = block_state_or_air(block_state_at, BlockPos::containing(position));
    let current_factor = block_jump_factor(current);
    if current_factor == mclone_blocks::DEFAULT_BLOCK_JUMP_FACTOR {
        f64::from(block_jump_factor(block_state_or_air(
            block_state_at,
            block_pos_below_that_affects_movement(position),
        )))
    } else {
        f64::from(current_factor)
    }
}

fn mallard_water_occupancy(
    position: Vec3d,
    block_state_at: &dyn Fn(BlockPos) -> Option<BlockStateId>,
) -> Option<f64> {
    let feet = BlockPos::containing(position);
    (block_state_at(feet).is_some_and(|state| block_fluid_kind(state) == BlockFluidKind::Water))
        .then_some(f64::from(feet.y) + 0.88)
}

fn random_shallow_water_surface(
    position: Vec3d,
    random: &mut SimpleRandomSource,
    block_state_at: &dyn Fn(BlockPos) -> Option<BlockStateId>,
) -> Option<Vec3d> {
    let center = BlockPos::containing(position);
    let mut chosen = None;
    let mut candidate_count = 0_i32;
    for radius in 1_i32..=MALLARD_HABITAT_SEARCH_RADIUS {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if dx.abs() + dz.abs() != radius
                    || f64::from(dx * dx + dz * dz) < MALLARD_MIN_DESTINATION_DISTANCE_SQR
                {
                    continue;
                }
                for dy in -1..=1 {
                    let water = center.offset(dx, dy, dz);
                    let Some(target) = shallow_water_surface_at(water, block_state_at) else {
                        continue;
                    };
                    candidate_count += 1;
                    if random.next_int_bound(candidate_count) == 0 {
                        chosen = Some(target);
                    }
                }
            }
        }
    }
    chosen
}

fn random_mallard_shore(
    position: Vec3d,
    random: &mut SimpleRandomSource,
    block_state_at: &dyn Fn(BlockPos) -> Option<BlockStateId>,
) -> Option<Vec3d> {
    let center = BlockPos::containing(position);
    let mut chosen = None;
    let mut candidate_count = 0_i32;
    for radius in 1_i32..=MALLARD_HABITAT_SEARCH_RADIUS {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if dx.abs() + dz.abs() != radius {
                    continue;
                }
                for dy in -1..=1 {
                    let feet = center.offset(dx, dy, dz);
                    let Some(target) = mallard_shore_surface_at(feet, block_state_at) else {
                        continue;
                    };
                    candidate_count += 1;
                    if random.next_int_bound(candidate_count) == 0 {
                        chosen = Some(target);
                    }
                }
            }
        }
    }
    chosen
}

fn mallard_intent_is_valid(
    intent: MallardHabitatIntent,
    block_state_at: &dyn Fn(BlockPos) -> Option<BlockStateId>,
) -> bool {
    let target = BlockPos::containing(intent.target);
    match intent.kind {
        MallardHabitatKind::Water => shallow_water_surface_at(target, block_state_at).is_some(),
        MallardHabitatKind::Shore => mallard_shore_surface_at(target, block_state_at).is_some(),
        MallardHabitatKind::Nest => mallard_land_surface_at(target, block_state_at).is_some(),
    }
}

fn shallow_water_surface_at(
    water: BlockPos,
    block_state_at: &dyn Fn(BlockPos) -> Option<BlockStateId>,
) -> Option<Vec3d> {
    let state = block_state_at(water)?;
    if block_fluid_kind(state) != BlockFluidKind::Water {
        return None;
    }
    let shallow = (1..=2).any(|depth| {
        let bed = water.offset(0, -depth, 0);
        block_state_at(bed)
            .and_then(|state| mclone_blocks::block_collision_aabb(state, bed))
            .is_some()
    });
    shallow.then_some(Vec3d::new(
        f64::from(water.x) + 0.5,
        f64::from(water.y) + 0.88,
        f64::from(water.z) + 0.5,
    ))
}

fn mallard_shore_surface_at(
    feet: BlockPos,
    block_state_at: &dyn Fn(BlockPos) -> Option<BlockStateId>,
) -> Option<Vec3d> {
    let floor = feet.offset(0, -1, 0);
    let space_clear = block_state_at(feet).is_some_and(|state| {
        block_fluid_kind(state) != BlockFluidKind::Water
            && mclone_blocks::block_collision_aabb(state, feet).is_none()
    });
    let stable_floor = block_state_at(floor)
        .is_some_and(|state| mclone_blocks::block_collision_aabb(state, floor).is_some());
    let near_water = [
        mclone_core::Direction::North,
        mclone_core::Direction::South,
        mclone_core::Direction::West,
        mclone_core::Direction::East,
    ]
    .iter()
    .any(|direction| {
        let water = feet.relative(*direction);
        block_state_at(water).is_some_and(|state| block_fluid_kind(state) == BlockFluidKind::Water)
    });
    (space_clear && stable_floor && near_water).then_some(Vec3d::new(
        f64::from(feet.x) + 0.5,
        f64::from(feet.y),
        f64::from(feet.z) + 0.5,
    ))
}

fn mallard_land_surface_at(
    feet: BlockPos,
    block_state_at: &dyn Fn(BlockPos) -> Option<BlockStateId>,
) -> Option<Vec3d> {
    let floor = feet.offset(0, -1, 0);
    let space_clear = [feet, feet.offset(0, 1, 0)].into_iter().all(|pos| {
        block_state_at(pos).is_some_and(|state| {
            block_fluid_kind(state) != BlockFluidKind::Water
                && mclone_blocks::block_collision_aabb(state, pos).is_none()
        })
    });
    let stable_floor = block_state_at(floor)
        .is_some_and(|state| mclone_blocks::block_collision_aabb(state, floor).is_some());
    (space_clear && stable_floor).then_some(Vec3d::new(
        f64::from(feet.x) + 0.5,
        f64::from(feet.y),
        f64::from(feet.z) + 0.5,
    ))
}

fn nearest_flockmate(
    position: Vec3d,
    flockmates: &[MallardFlockmateTarget],
) -> Option<MallardFlockmateTarget> {
    flockmates
        .iter()
        .copied()
        .filter(|target| {
            let dx = target.position.x - position.x;
            let dz = target.position.z - position.z;
            dx * dx + dz * dz > 1.0e-8
        })
        .min_by(|left, right| {
            squared_horizontal_distance(position, left.position)
                .total_cmp(&squared_horizontal_distance(position, right.position))
        })
}

fn random_deer_forage_target<F>(
    position: Vec3d,
    random: &mut SimpleRandomSource,
    block_state_at: &F,
) -> Option<DeerHabitatIntent>
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let origin = BlockPos::containing(position);
    let mut fallback = None;
    for _ in 0..32 {
        let dx =
            random.next_int_bound(DEER_HABITAT_SEARCH_RADIUS * 2 + 1) - DEER_HABITAT_SEARCH_RADIUS;
        let dz =
            random.next_int_bound(DEER_HABITAT_SEARCH_RADIUS * 2 + 1) - DEER_HABITAT_SEARCH_RADIUS;
        if dx.abs() + dz.abs() < 3 {
            continue;
        }
        for dy in -3..=3 {
            let feet = origin.offset(dx, dy, dz);
            if deer_walkable_feet(feet, block_state_at) {
                let target = Vec3d::new(
                    f64::from(feet.x) + 0.5,
                    f64::from(feet.y),
                    f64::from(feet.z) + 0.5,
                );
                let intent = DeerHabitatIntent {
                    kind: DeerHabitatKind::Forage,
                    target,
                    ticks_remaining: 240,
                };
                fallback.get_or_insert(intent);
                if deer_browse_near(feet, block_state_at) {
                    return Some(intent);
                }
            }
        }
    }
    fallback
}

fn select_bee_flower<F>(
    home: Vec3d,
    preferred_band: usize,
    previous: Option<BlockPos>,
    random: &mut SimpleRandomSource,
    blocks: &F,
) -> Option<BlockPos>
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let center = BlockPos::containing(home);
    let mut candidates: [Vec<BlockPos>; 3] = std::array::from_fn(|_| Vec::new());
    for dy in -4..=4 {
        for dx in -BEE_FLOWER_SEARCH_RADIUS..=BEE_FLOWER_SEARCH_RADIUS {
            for dz in -BEE_FLOWER_SEARCH_RADIUS..=BEE_FLOWER_SEARCH_RADIUS {
                let horizontal_distance_sqr = f64::from(dx * dx + dz * dz);
                if horizontal_distance_sqr > f64::from(BEE_FLOWER_SEARCH_RADIUS.pow(2)) {
                    continue;
                }
                let pos = center.offset(dx, dy, dz);
                if Some(pos) == previous || !blocks(pos).is_some_and(is_flower_state) {
                    continue;
                }
                let band = if horizontal_distance_sqr < 8.0 * 8.0 {
                    0
                } else if horizontal_distance_sqr < 14.0 * 14.0 {
                    1
                } else {
                    2
                };
                if horizontal_distance_sqr >= BEE_MIN_FORAGE_DISTANCE_SQR {
                    candidates[band].push(pos);
                }
            }
        }
    }
    for offset in 0..3 {
        let band = (preferred_band + offset) % 3;
        if !candidates[band].is_empty() {
            let index = random.next_int_bound(candidates[band].len() as i32) as usize;
            return Some(candidates[band][index]);
        }
    }
    previous.filter(|pos| blocks(*pos).is_some_and(is_flower_state))
}

fn is_flower_state(state: BlockStateId) -> bool {
    matches!(
        state,
        value if value == generated_block_state_id(DANDELION)
            || value == generated_block_state_id(POPPY)
    )
}

fn bee_flower_target(pos: BlockPos) -> Vec3d {
    Vec3d::new(
        f64::from(pos.x) + 0.5,
        f64::from(pos.y) + 0.82,
        f64::from(pos.z) + 0.5,
    )
}

fn bee_home_target(home: Vec3d) -> Vec3d {
    home.add(Vec3d::new(0.0, 0.62, 0.0))
}

fn bee_hover_target(home: Vec3d, identity: EntityPersistentId, sequence: u32) -> Vec3d {
    let phase = ((identity.least ^ u64::from(sequence).wrapping_mul(0x9e37_79b9)) % 16) as f64
        * std::f64::consts::TAU
        / 16.0;
    home.add(Vec3d::new(
        phase.cos() * 1.35,
        1.1 + (phase * 2.0).sin() * 0.32,
        phase.sin() * 1.35,
    ))
}

fn bee_cruise_target<F>(
    entity: &ServerEntityState,
    destination: Vec3d,
    sequence: u32,
    returning: bool,
    blocks: &F,
) -> Vec3d
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let displacement = destination.subtract(entity.position);
    let horizontal_length =
        (displacement.x * displacement.x + displacement.z * displacement.z).sqrt();
    if horizontal_length < 5.0 {
        return destination;
    }
    let direction_x = displacement.x / horizontal_length;
    let direction_z = displacement.z / horizontal_length;
    let side = if (entity.persistent_id.least ^ u64::from(sequence) ^ u64::from(returning)) & 1 == 0
    {
        1.0
    } else {
        -1.0
    };
    let candidate = Vec3d::new(
        entity.position.x + displacement.x * 0.48 - direction_z * 1.4 * side,
        entity.position.y.max(destination.y) + 1.5 + f64::from(sequence % 3) * 0.35,
        entity.position.z + displacement.z * 0.48 + direction_x * 1.4 * side,
    );
    bee_waypoint_is_clear(entity, candidate, blocks)
        .then_some(candidate)
        .unwrap_or(destination)
}

fn bee_recovery_target<F>(
    entity: &ServerEntityState,
    destination: Vec3d,
    sequence: u32,
    random: &mut SimpleRandomSource,
    blocks: &F,
) -> Vec3d
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let displacement = destination.subtract(entity.position);
    let horizontal_length = (displacement.x * displacement.x + displacement.z * displacement.z)
        .sqrt()
        .max(1.0e-6);
    let direction_x = displacement.x / horizontal_length;
    let direction_z = displacement.z / horizontal_length;
    let preferred_side = if (u64::from(sequence) ^ entity.persistent_id.least) & 1 == 0 {
        1.0
    } else {
        -1.0
    };
    for side in [preferred_side, -preferred_side] {
        for lift in [1.0, 1.8, 2.8] {
            let advance = 0.9 + random.next_double() * 0.5;
            let candidate = entity.position.add(Vec3d::new(
                direction_x * advance - direction_z * 1.25 * side,
                lift,
                direction_z * advance + direction_x * 1.25 * side,
            ));
            if bee_waypoint_is_clear(entity, candidate, blocks) {
                return candidate;
            }
        }
    }
    entity.position.add(Vec3d::new(0.0, 2.8, 0.0))
}

fn bee_waypoint_is_clear<F>(entity: &ServerEntityState, target: Vec3d, blocks: &F) -> bool
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let feet = BlockPos::containing(target);
    let head = BlockPos::containing(target.add(Vec3d::new(0.0, f64::from(entity.height), 0.0)));
    [feet, head]
        .into_iter()
        .all(|pos| blocks(pos).is_some_and(|state| block_collision_aabb(state, pos).is_none()))
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BeeMoveOutcome {
    reached: bool,
    progressed: bool,
}

fn bee_move_toward<F>(entity: &mut ServerEntityState, target: Vec3d, blocks: &F) -> BeeMoveOutcome
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let displacement = target.subtract(entity.position);
    let distance_sqr = displacement.length_sqr();
    if distance_sqr <= BEE_TARGET_REACHED_DISTANCE_SQR {
        return BeeMoveOutcome {
            reached: true,
            progressed: true,
        };
    }
    let requested = displacement.scale(BEE_FLIGHT_SPEED / distance_sqr.sqrt());
    let bounding_box = collision_aabb_for_feet_position(
        entity.position,
        f64::from(entity.width),
        f64::from(entity.height),
    );
    let traveled = collide_mob_movement(blocks, bounding_box, requested, 0.0, false);
    entity.position = entity.position.add(traveled);
    let horizontal_sqr = traveled.x * traveled.x + traveled.z * traveled.z;
    if horizontal_sqr > 1.0e-8 {
        let wanted_y_rot = (-traveled.x).atan2(traveled.z).to_degrees() as f32;
        entity.y_rot_degrees =
            rotate_degrees_towards(entity.y_rot_degrees, wanted_y_rot, BEE_MAX_TURN_DEGREES);
    }
    BeeMoveOutcome {
        reached: false,
        progressed: traveled.length_sqr() >= BEE_PROGRESS_DISTANCE_SQR,
    }
}

fn set_bee_animation(entity: &mut ServerEntityState, behavior: mclone_protocol::BeeBehavior) {
    let (clip, phase_source) = match behavior {
        mclone_protocol::BeeBehavior::Hover | mclone_protocol::BeeBehavior::AtNest => (
            AnimationClipId::from_static("hover"),
            mclone_core::AnimationPhaseSource::Elapsed,
        ),
        mclone_protocol::BeeBehavior::FlyToFlower | mclone_protocol::BeeBehavior::ReturnHome => (
            AnimationClipId::from_static("fly"),
            mclone_core::AnimationPhaseSource::Elapsed,
        ),
        mclone_protocol::BeeBehavior::Forage => (
            AnimationClipId::from_static("forage"),
            mclone_core::AnimationPhaseSource::Elapsed,
        ),
    };
    let previous = entity.animation;
    if previous
        .is_some_and(|animation| animation.clip == clip && animation.phase_source == phase_source)
    {
        return;
    }
    let epoch = previous.map_or(0, |animation| animation.epoch.wrapping_add(1));
    entity.animation = Some(match phase_source {
        mclone_core::AnimationPhaseSource::Distance => AnimationState::distance(clip, epoch),
        mclone_core::AnimationPhaseSource::Elapsed => {
            AnimationState::elapsed(clip, epoch, entity.tick_count)
        }
    });
}

fn deer_herd_target(position: Vec3d, herdmate: Vec3d) -> Option<DeerHabitatIntent> {
    let distance_sqr = position.distance_to_sqr(herdmate);
    let direction = herdmate.subtract(position);
    let horizontal_length = (direction.x * direction.x + direction.z * direction.z).sqrt();
    if horizontal_length <= 1.0e-6 {
        return None;
    }
    let (direction_x, direction_z) = (
        direction.x / horizontal_length,
        direction.z / horizontal_length,
    );
    let target = if distance_sqr > DEER_HERD_COHESION_DISTANCE_SQR {
        Vec3d::new(
            herdmate.x - direction_x * 5.0,
            herdmate.y,
            herdmate.z - direction_z * 5.0,
        )
    } else if distance_sqr < DEER_HERD_SEPARATION_DISTANCE_SQR {
        Vec3d::new(
            position.x - direction_x * 3.0,
            position.y,
            position.z - direction_z * 3.0,
        )
    } else {
        return None;
    };
    Some(DeerHabitatIntent {
        kind: DeerHabitatKind::Herd,
        target,
        ticks_remaining: 160,
    })
}

fn deer_escape_target<F>(
    position: Vec3d,
    threat: Option<Vec3d>,
    block_state_at: &F,
) -> DeerHabitatIntent
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let (away_x, away_z) = threat.map_or((0.0, 1.0), |threat| {
        let dx = position.x - threat.x;
        let dz = position.z - threat.z;
        let length = (dx * dx + dz * dz).sqrt();
        if length > 1.0e-6 {
            (dx / length, dz / length)
        } else {
            (0.0, 1.0)
        }
    });
    let origin = BlockPos::containing(position);
    let mut best = None;
    let mut best_score = f64::NEG_INFINITY;
    for distance in 8..=16 {
        for lateral in [-5_i32, 0, 5] {
            let side_x = away_z;
            let side_z = -away_x;
            let x = (position.x + away_x * f64::from(distance) + side_x * f64::from(lateral))
                .floor() as i32;
            let z = (position.z + away_z * f64::from(distance) + side_z * f64::from(lateral))
                .floor() as i32;
            for dy in -4..=4 {
                let feet = BlockPos::new(x, origin.y + dy, z);
                if !deer_walkable_feet(feet, block_state_at) {
                    continue;
                }
                let cover = deer_woody_cover_near(feet, block_state_at) as f64;
                let score = f64::from(distance) + cover * 4.0 - f64::from(lateral.abs()) * 0.2;
                if score > best_score {
                    best_score = score;
                    best = Some(Vec3d::new(
                        f64::from(feet.x) + 0.5,
                        f64::from(feet.y),
                        f64::from(feet.z) + 0.5,
                    ));
                }
            }
        }
    }
    DeerHabitatIntent {
        kind: DeerHabitatKind::Escape,
        target: best.unwrap_or_else(|| {
            Vec3d::new(
                position.x + away_x * 12.0,
                position.y,
                position.z + away_z * 12.0,
            )
        }),
        ticks_remaining: 160,
    }
}

fn move_deer_toward<F>(entity: &mut ServerEntityState, target: Vec3d, speed: f64, blocks: &F)
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let dx = target.x - entity.position.x;
    let dz = target.z - entity.position.z;
    let length = (dx * dx + dz * dz).sqrt();
    if length <= 1.0e-6 {
        return;
    }
    let requested = Vec3d::new(dx / length * speed, -MOB_GRAVITY, dz / length * speed);
    let bounding_box = collision_aabb_for_feet_position(
        entity.position,
        f64::from(entity.width),
        f64::from(entity.height),
    );
    let traveled = collide_mob_movement(blocks, bounding_box, requested, 1.0, entity.on_ground);
    entity.position = entity.position.add(traveled);
    entity.on_ground = collide_movement_result(requested, traveled).on_ground;
    if traveled.x * traveled.x + traveled.z * traveled.z > 1.0e-8 {
        let wanted_y_rot = (-traveled.x).atan2(traveled.z).to_degrees() as f32;
        entity.y_rot_degrees = rotate_degrees_towards(
            entity.y_rot_degrees,
            wanted_y_rot,
            if speed >= DEER_FLEE_SPEED {
                DEER_FLEE_MAX_TURN_DEGREES
            } else {
                DEER_WALK_MAX_TURN_DEGREES
            },
        );
    }
}

fn deer_walkable_feet<F>(feet: BlockPos, blocks: &F) -> bool
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let Some(floor) = blocks(feet.below()) else {
        return false;
    };
    block_collision_aabb(floor, feet.below()).is_some()
        && blocks(feet).is_some_and(|state| {
            block_fluid_kind(state) != BlockFluidKind::Water
                && block_collision_aabb(state, feet).is_none()
        })
        && blocks(feet.offset(0, 1, 0)).is_some_and(|state| {
            block_fluid_kind(state) != BlockFluidKind::Water
                && block_collision_aabb(state, feet.offset(0, 1, 0)).is_none()
        })
}

fn random_deer_safe_bank_target<F>(
    position: Vec3d,
    random: &mut SimpleRandomSource,
    blocks: &F,
) -> Option<DeerHabitatIntent>
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let origin = BlockPos::containing(position);
    let mut chosen = None;
    let mut count = 0_i32;
    for radius in 2_i32..=DEER_HABITAT_SEARCH_RADIUS {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if dx.abs() + dz.abs() != radius {
                    continue;
                }
                for dy in -3..=3 {
                    let feet = origin.offset(dx, dy, dz);
                    if !deer_walkable_feet(feet, blocks) || !deer_bank_has_water(feet, blocks) {
                        continue;
                    }
                    count += 1;
                    if random.next_int_bound(count) == 0 {
                        chosen = Some(DeerHabitatIntent {
                            kind: DeerHabitatKind::Water,
                            target: Vec3d::new(
                                f64::from(feet.x) + 0.5,
                                f64::from(feet.y),
                                f64::from(feet.z) + 0.5,
                            ),
                            ticks_remaining: 240,
                        });
                    }
                }
            }
        }
    }
    chosen
}

fn deer_bank_has_water<F>(feet: BlockPos, blocks: &F) -> bool
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    [
        mclone_core::Direction::North,
        mclone_core::Direction::South,
        mclone_core::Direction::West,
        mclone_core::Direction::East,
    ]
    .iter()
    .any(|direction| {
        let neighbor = feet.relative(*direction);
        [neighbor, neighbor.below()].iter().any(|position| {
            blocks(*position).is_some_and(|state| block_fluid_kind(state) == BlockFluidKind::Water)
        })
    })
}

fn deer_bedding_site<F>(feet: BlockPos, blocks: &F) -> bool
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    deer_walkable_feet(feet, blocks) && deer_woody_cover_near(feet, blocks) >= 4
}

fn deer_browse_near<F>(feet: BlockPos, blocks: &F) -> bool
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    for dx in -2..=2 {
        for dz in -2..=2 {
            for dy in 0..=2 {
                let Some(state) = blocks(feet.offset(dx, dy, dz)) else {
                    continue;
                };
                if matches!(
                    state,
                    value if value == generated_block_state_id(GRASS)
                        || value == generated_block_state_id(FERN)
                        || value == generated_block_state_id(LARGE_FERN_LOWER)
                        || value == generated_block_state_id(LARGE_FERN_UPPER)
                        || value == generated_block_state_id(TALL_GRASS_LOWER)
                        || value == generated_block_state_id(TALL_GRASS_UPPER)
                        || value == generated_block_state_id(DANDELION)
                        || value == generated_block_state_id(POPPY)
                        || value == generated_block_state_id(OAK_LEAVES)
                        || value == generated_block_state_id(BIRCH_LEAVES)
                        || value == generated_block_state_id(SPRUCE_LEAVES)
                        || value == generated_block_state_id(DARK_OAK_LEAVES)
                        || value == generated_block_state_id(ACACIA_LEAVES)
                        || value == generated_block_state_id(JUNGLE_LEAVES)
                ) {
                    return true;
                }
            }
        }
    }
    false
}

fn deer_woody_cover_near<F>(feet: BlockPos, blocks: &F) -> u8
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let woody = [
        OAK_LOG,
        OAK_LEAVES,
        BIRCH_LOG,
        BIRCH_LEAVES,
        SPRUCE_LOG,
        SPRUCE_LEAVES,
        DARK_OAK_LOG,
        DARK_OAK_LEAVES,
        ACACIA_LOG,
        ACACIA_LEAVES,
    ]
    .map(generated_block_state_id);
    let mut count = 0_u8;
    for dx in -3..=3 {
        for dz in -3..=3 {
            for dy in 0..=5 {
                count = count.saturating_add(u8::from(
                    blocks(feet.offset(dx, dy, dz)).is_some_and(|state| woody.contains(&state)),
                ));
            }
        }
    }
    count
}

fn set_deer_animation(entity: &mut ServerEntityState, behavior: mclone_protocol::DeerBehavior) {
    let (clip, phase_source) = match behavior {
        mclone_protocol::DeerBehavior::Walk => ("walk", false),
        mclone_protocol::DeerBehavior::Flee => ("flee", false),
        mclone_protocol::DeerBehavior::Graze => ("graze", true),
        mclone_protocol::DeerBehavior::Drink => ("graze", true),
        mclone_protocol::DeerBehavior::Alert => ("alert", true),
        mclone_protocol::DeerBehavior::LieDown => ("lie_down", true),
        mclone_protocol::DeerBehavior::Bedded => ("bedded_idle", true),
        mclone_protocol::DeerBehavior::StandUp => ("stand_up", true),
        mclone_protocol::DeerBehavior::Hit => ("hit", true),
        mclone_protocol::DeerBehavior::Fall => ("fall", true),
        mclone_protocol::DeerBehavior::Idle => ("idle", true),
    };
    let clip = AnimationClipId::from_static(clip);
    let should_replace = entity.animation.is_none_or(|current| current.clip != clip);
    if should_replace {
        let epoch = entity
            .animation
            .map_or(0, |current| current.epoch.wrapping_add(1));
        entity.animation = Some(if phase_source {
            AnimationState::elapsed(clip, epoch, entity.tick_count)
        } else {
            AnimationState::distance(clip, epoch)
        });
    }
}

fn rabbit_active_time(day_time: u64) -> bool {
    matches!(day_time % 24_000, 0..=5_000 | 10_500..=14_500)
}

fn select_rabbit_dig_site<F>(position: Vec3d, blocks: &F) -> Option<BlockPos>
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let origin = BlockPos::containing(position);
    let soil = [
        generated_block_state_id(GRASS_BLOCK),
        generated_block_state_id(DIRT),
    ];
    let directions = [
        mclone_core::Direction::North,
        mclone_core::Direction::South,
        mclone_core::Direction::West,
        mclone_core::Direction::East,
    ];
    let mut best = None;
    let mut best_distance = f64::INFINITY;
    for radius in 1_i32..=RABBIT_SEARCH_RADIUS {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if dx.abs() + dz.abs() != radius {
                    continue;
                }
                for dy in -2..=2 {
                    let target = origin.offset(dx, dy, dz);
                    if !blocks(target).is_some_and(|state| soil.contains(&state))
                        || !blocks(target.offset(0, 1, 0))
                            .is_some_and(|state| soil.contains(&state))
                        || block_fluid_kind(blocks(target).unwrap()) == BlockFluidKind::Water
                    {
                        continue;
                    }
                    let has_threshold = directions.iter().any(|direction| {
                        let front = target.relative(*direction);
                        let rear = target.relative(direction.opposite());
                        rabbit_walkable_feet(front, blocks)
                            && blocks(rear)
                                .is_some_and(|state| block_collision_aabb(state, rear).is_some())
                    });
                    if !has_threshold {
                        continue;
                    }
                    let center = Vec3d::new(
                        f64::from(target.x) + 0.5,
                        f64::from(target.y),
                        f64::from(target.z) + 0.5,
                    );
                    let distance = position.distance_to_sqr(center);
                    if distance < best_distance {
                        best_distance = distance;
                        best = Some(target);
                    }
                }
            }
        }
    }
    best
}

fn rabbit_entrance_position(target: BlockPos, position: Vec3d) -> Vec3d {
    let center = Vec3d::new(
        f64::from(target.x) + 0.5,
        f64::from(target.y),
        f64::from(target.z) + 0.5,
    );
    let dx = position.x - center.x;
    let dz = position.z - center.z;
    if dx.abs() >= dz.abs() {
        center.add(Vec3d::new(dx.signum() * 0.8, 0.0, 0.0))
    } else {
        center.add(Vec3d::new(0.0, 0.0, dz.signum() * 0.8))
    }
}

fn rabbit_carrot_targets<F>(position: Vec3d, blocks: &F) -> Vec<BlockPos>
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let origin = BlockPos::containing(position);
    let mature = generated_block_state_id(CARROTS_AGE_7);
    let mut targets = Vec::new();
    for dx in -RABBIT_SEARCH_RADIUS..=RABBIT_SEARCH_RADIUS {
        for dz in -RABBIT_SEARCH_RADIUS..=RABBIT_SEARCH_RADIUS {
            for dy in -3..=3 {
                let crop = origin.offset(dx, dy, dz);
                if blocks(crop) != Some(mature) || !rabbit_walkable_feet(crop, blocks) {
                    continue;
                }
                let center = Vec3d::new(
                    f64::from(crop.x) + 0.5,
                    f64::from(crop.y),
                    f64::from(crop.z) + 0.5,
                );
                let distance = position.distance_to_sqr(center);
                targets.push((distance, crop));
            }
        }
    }
    targets.sort_by(|(left_distance, left), (right_distance, right)| {
        left_distance
            .total_cmp(right_distance)
            .then_with(|| left.cmp(right))
    });
    targets.into_iter().map(|(_, target)| target).collect()
}

fn random_rabbit_forage_target<F>(
    position: Vec3d,
    random: &mut SimpleRandomSource,
    blocks: &F,
) -> Option<RabbitHabitatIntent>
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let origin = BlockPos::containing(position);
    for _ in 0..24 {
        let dx = random.next_int_bound(RABBIT_SEARCH_RADIUS * 2 + 1) - RABBIT_SEARCH_RADIUS;
        let dz = random.next_int_bound(RABBIT_SEARCH_RADIUS * 2 + 1) - RABBIT_SEARCH_RADIUS;
        for dy in -3..=3 {
            let feet = origin.offset(dx, dy, dz);
            if rabbit_walkable_feet(feet, blocks) {
                return Some(RabbitHabitatIntent {
                    kind: RabbitIntentKind::Forage,
                    target: Vec3d::new(
                        f64::from(feet.x) + 0.5,
                        f64::from(feet.y),
                        f64::from(feet.z) + 0.5,
                    ),
                    block: None,
                    ticks_remaining: RABBIT_INTENT_TICKS,
                    stall_ticks: 0,
                });
            }
        }
    }
    None
}

fn rabbit_player_threat(
    position: Vec3d,
    behavior: mclone_protocol::RabbitBehavior,
    intent: Option<RabbitHabitatIntent>,
    underground_threat_hold: bool,
    nearby_players: &[MobPlayerTarget],
) -> Option<MobPlayerTarget> {
    let nearest = nearby_players.iter().copied().min_by(|left, right| {
        position
            .distance_to_sqr(left.position)
            .total_cmp(&position.distance_to_sqr(right.position))
    })?;
    if nearest.tempting_carrot {
        return None;
    }
    let continuing = matches!(behavior, mclone_protocol::RabbitBehavior::Flee)
        || (matches!(
            behavior,
            mclone_protocol::RabbitBehavior::EnterBurrow
                | mclone_protocol::RabbitBehavior::Underground
        ) && underground_threat_hold)
        || intent.is_some_and(|intent| intent.kind == RabbitIntentKind::Escape);
    let radius_sqr = if continuing {
        RABBIT_FLEE_EXIT_RADIUS_SQR
    } else {
        RABBIT_FLEE_ENTER_RADIUS_SQR
    };
    (position.distance_to_sqr(nearest.position) <= radius_sqr).then_some(nearest)
}

fn rabbit_escape_candidates<F>(position: Vec3d, threat: Vec3d, blocks: &F) -> Vec<Vec3d>
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let away = position.subtract(threat);
    let length = (away.x * away.x + away.z * away.z).sqrt();
    let (away_x, away_z) = if length > 1.0e-6 {
        (away.x / length, away.z / length)
    } else {
        (0.0, 1.0)
    };
    let origin = BlockPos::containing(position);
    let mut candidate_blocks = Vec::new();
    for (distance, lateral) in [
        (10.0, 0.0),
        (10.0, 0.35),
        (10.0, -0.35),
        (9.0, 0.7),
        (9.0, -0.7),
        (8.0, 0.0),
        (7.0, 1.0),
        (7.0, -1.0),
        (6.0, 0.0),
    ] {
        let direction_x = away_x - away_z * lateral;
        let direction_z = away_z + away_x * lateral;
        let direction_length = (direction_x * direction_x + direction_z * direction_z).sqrt();
        let candidate = BlockPos::new(
            (position.x + direction_x / direction_length * distance).floor() as i32,
            origin.y,
            (position.z + direction_z / direction_length * distance).floor() as i32,
        );
        for dy in [0, 1, -1, 2, -2] {
            let feet = candidate.offset(0, dy, 0);
            let target = Vec3d::new(
                f64::from(feet.x) + 0.5,
                f64::from(feet.y),
                f64::from(feet.z) + 0.5,
            );
            if rabbit_walkable_feet(feet, blocks)
                && target.distance_to_sqr(threat) > position.distance_to_sqr(threat) + 0.25
                && !candidate_blocks.contains(&feet)
            {
                candidate_blocks.push(feet);
                break;
            }
        }
    }
    candidate_blocks
        .into_iter()
        .map(|feet| {
            Vec3d::new(
                f64::from(feet.x) + 0.5,
                f64::from(feet.y),
                f64::from(feet.z) + 0.5,
            )
        })
        .collect()
}

fn rabbit_walkable_feet<F>(feet: BlockPos, blocks: &F) -> bool
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    deer_walkable_feet(feet, blocks)
}

fn resolve_rabbit_refuge(
    rabbit: RabbitRuntimeSaveData,
    candidates: &[RabbitRefugeCandidate],
) -> Availability<RabbitRefugeCandidate> {
    let mut saw_memory = false;
    let mut saw_unsuitable = false;
    let mut available = None;
    for known in rabbit.known_refuges.iter().flatten() {
        saw_memory = true;
        let Some(candidate) = candidates
            .iter()
            .copied()
            .find(|candidate| candidate.persistent_id == known.locator.persistent_id)
        else {
            continue;
        };
        let has_capacity = candidate.occupancy < candidate.capacity
            || rabbit.sheltered_in == Some(candidate.persistent_id);
        if candidate.disturbed || !has_capacity {
            saw_unsuitable = true;
            continue;
        }
        let score = (
            known.familiarity,
            known.last_confirmed_tick,
            known.locator.persistent_id,
        );
        if available
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            available = Some((score, candidate));
        }
    }
    if let Some((_, candidate)) = available {
        Availability::Available(candidate)
    } else if saw_unsuitable {
        Availability::ConfirmedUnsuitable
    } else if saw_memory {
        Availability::CurrentlyUnavailable
    } else {
        Availability::CurrentlyUnavailable
    }
}

fn nearby_rabbit_refuge(
    position: Vec3d,
    rabbit: RabbitRuntimeSaveData,
    candidates: &[RabbitRefugeCandidate],
) -> Option<RabbitRefugeCandidate> {
    candidates
        .iter()
        .copied()
        .filter(|candidate| {
            !candidate.disturbed
                && (candidate.occupancy < candidate.capacity
                    || rabbit.sheltered_in == Some(candidate.persistent_id))
                && position.distance_to_sqr(candidate.position)
                    <= RABBIT_REFUGE_DISCOVERY_RADIUS_SQR
        })
        .min_by(|left, right| {
            position
                .distance_to_sqr(left.position)
                .total_cmp(&position.distance_to_sqr(right.position))
                .then_with(|| left.persistent_id.cmp(&right.persistent_id))
        })
}

fn nearby_rabbit_refuge_count(position: Vec3d, candidates: &[RabbitRefugeCandidate]) -> usize {
    candidates
        .iter()
        .filter(|candidate| {
            position.distance_to_sqr(candidate.position) <= RABBIT_REFUGE_DISCOVERY_RADIUS_SQR
        })
        .count()
}

fn rabbit_active_rest_due(entity: ServerEntityState, intent: Option<RabbitHabitatIntent>) -> bool {
    if entity.tick_count < RABBIT_ACTIVE_REST_WARMUP_TICKS
        || intent.is_some_and(|intent| {
            matches!(
                intent.kind,
                RabbitIntentKind::Raid | RabbitIntentKind::Tempt | RabbitIntentKind::Dig
            )
        })
    {
        return false;
    }
    let phase = entity
        .persistent_id
        .least
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .rotate_left(17)
        % RABBIT_ACTIVE_REST_INTERVAL_TICKS;
    (entity.tick_count + phase) % RABBIT_ACTIVE_REST_INTERVAL_TICKS < 40
}

fn move_rabbit_toward<F>(entity: &mut ServerEntityState, target: Vec3d, speed: f64, blocks: &F)
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    move_deer_toward(entity, target, speed, blocks);
}

fn move_squirrel_to_canopy_perch<F>(
    entity: &mut ServerEntityState,
    route: SquirrelRefugeCandidate,
    blocks: &F,
) -> bool
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let trunk_top = squirrel_route_position(route.trunk_top);
    let canopy_edge_bottom = squirrel_route_position(route.canopy_edge_bottom);
    let canopy_edge_top = squirrel_route_position(route.canopy_edge_top);
    let refuge = squirrel_route_position(route.refuge);
    let target = if entity.position.y + 0.01 < trunk_top.y
        && squared_horizontal_distance(entity.position, trunk_top) <= 0.35 * 0.35
    {
        trunk_top
    } else if entity.position.y <= trunk_top.y + 0.01
        && squared_horizontal_distance(entity.position, canopy_edge_bottom) > 0.04 * 0.04
    {
        canopy_edge_bottom
    } else if entity.position.y + 0.01 < canopy_edge_top.y
        && squared_horizontal_distance(entity.position, canopy_edge_top) <= 0.35 * 0.35
    {
        canopy_edge_top
    } else if entity.position.distance_to_sqr(refuge) > 0.04 * 0.04 {
        refuge
    } else {
        entity.position = refuge;
        return true;
    };
    move_squirrel_along_route(entity, target, route, blocks);
    false
}

fn move_squirrel_from_canopy_perch<F>(
    entity: &mut ServerEntityState,
    route: SquirrelRefugeCandidate,
    blocks: &F,
) -> bool
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let approach = squirrel_route_position(route.approach);
    let trunk_top = squirrel_route_position(route.trunk_top);
    let canopy_edge_bottom = squirrel_route_position(route.canopy_edge_bottom);
    let canopy_edge_top = squirrel_route_position(route.canopy_edge_top);
    let target = if entity.position.y >= canopy_edge_top.y - 0.01
        && squared_horizontal_distance(entity.position, canopy_edge_top) > 0.04 * 0.04
    {
        canopy_edge_top
    } else if squared_horizontal_distance(entity.position, canopy_edge_top) <= 0.35 * 0.35
        && entity.position.y > canopy_edge_bottom.y + 0.01
    {
        canopy_edge_bottom
    } else if entity.position.y <= canopy_edge_bottom.y + 0.01
        && squared_horizontal_distance(entity.position, trunk_top) > 0.04 * 0.04
    {
        trunk_top
    } else if squared_horizontal_distance(entity.position, trunk_top) <= 0.35 * 0.35
        && entity.position.y > approach.y + 0.01
    {
        approach
    } else if entity.position.distance_to_sqr(approach) <= 0.04 * 0.04 {
        entity.position = approach;
        return true;
    } else {
        approach
    };
    move_squirrel_along_route(entity, target, route, blocks);
    false
}

fn squirrel_route_position(pos: BlockPos) -> Vec3d {
    Vec3d::new(
        f64::from(pos.x) + 0.5,
        f64::from(pos.y),
        f64::from(pos.z) + 0.5,
    )
}

fn move_squirrel_along_route<F>(
    entity: &mut ServerEntityState,
    target: Vec3d,
    route: SquirrelRefugeCandidate,
    blocks: &F,
) where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let delta = target.subtract(entity.position);
    let distance = delta.length_sqr().sqrt().max(1.0e-9);
    let requested = delta.scale((SQUIRREL_CLIMB_SPEED / distance).min(1.0));
    let bounding_box = collision_aabb_for_feet_position(
        entity.position,
        f64::from(entity.width),
        f64::from(entity.height),
    );
    let traveled = collide_movement(blocks, bounding_box, requested);
    entity.position = entity.position.add(traveled);
    entity.on_ground = false;
    let horizontal_motion_sqr = traveled.x * traveled.x + traveled.z * traveled.z;
    let (dx, dz) = if horizontal_motion_sqr > 1.0e-8 {
        (traveled.x, traveled.z)
    } else {
        (
            f64::from(route.trunk.x) + 0.5 - entity.position.x,
            f64::from(route.trunk.z) + 0.5 - entity.position.z,
        )
    };
    if dx * dx + dz * dz > 1.0e-8 {
        entity.y_rot_degrees = (-dx).atan2(dz).to_degrees() as f32;
    }
}

fn set_squirrel_animation(
    entity: &mut ServerEntityState,
    behavior: mclone_protocol::SquirrelBehavior,
) {
    let (clip, elapsed) = match behavior {
        mclone_protocol::SquirrelBehavior::Idle => ("idle", true),
        mclone_protocol::SquirrelBehavior::Bound
        | mclone_protocol::SquirrelBehavior::TrunkApproach => ("bound", false),
        mclone_protocol::SquirrelBehavior::Forage => ("forage", true),
        mclone_protocol::SquirrelBehavior::Alarm => ("alarm", true),
        mclone_protocol::SquirrelBehavior::Flee => ("flee", false),
        mclone_protocol::SquirrelBehavior::Climb => ("climb", false),
        mclone_protocol::SquirrelBehavior::RefugeEnter => ("refuge_enter", true),
        mclone_protocol::SquirrelBehavior::RefugeIdle => ("refuge_idle", true),
        mclone_protocol::SquirrelBehavior::RefugeExit => ("refuge_exit", true),
    };
    let clip = AnimationClipId::from_static(clip);
    if entity.animation.is_some_and(|current| current.clip == clip) {
        return;
    }
    let epoch = entity
        .animation
        .map_or(0, |current| current.epoch.wrapping_add(1));
    entity.animation = Some(if elapsed {
        AnimationState::elapsed(clip, epoch, entity.tick_count)
    } else {
        AnimationState::distance(clip, epoch)
    });
}

fn set_rabbit_animation(entity: &mut ServerEntityState, behavior: mclone_protocol::RabbitBehavior) {
    let (clip, elapsed) = match behavior {
        mclone_protocol::RabbitBehavior::Idle | mclone_protocol::RabbitBehavior::Underground => {
            ("idle", true)
        }
        mclone_protocol::RabbitBehavior::Hop => ("hop", false),
        mclone_protocol::RabbitBehavior::Flee => ("flee", false),
        mclone_protocol::RabbitBehavior::Forage | mclone_protocol::RabbitBehavior::Raid => {
            ("forage", true)
        }
        mclone_protocol::RabbitBehavior::Dig => ("dig", true),
        mclone_protocol::RabbitBehavior::EnterBurrow => ("enter_burrow", true),
        mclone_protocol::RabbitBehavior::Emerge => ("emerge", true),
        mclone_protocol::RabbitBehavior::Courtship => ("courtship", true),
    };
    let clip = AnimationClipId::from_static(clip);
    if entity.animation.is_some_and(|current| current.clip == clip) {
        return;
    }
    let epoch = entity
        .animation
        .map_or(0, |current| current.epoch.wrapping_add(1));
    entity.animation = Some(if elapsed {
        AnimationState::elapsed(clip, epoch, entity.tick_count)
    } else {
        AnimationState::distance(clip, epoch)
    });
}

fn squared_horizontal_distance(left: Vec3d, right: Vec3d) -> f64 {
    let dx = left.x - right.x;
    let dz = left.z - right.z;
    dx * dx + dz * dz
}

fn rotate_degrees_towards(current: f32, wanted: f32, max_delta: f32) -> f32 {
    let mut difference = (wanted - current) % 360.0;
    if difference >= 180.0 {
        difference -= 360.0;
    }
    if difference < -180.0 {
        difference += 360.0;
    }
    let mut result = current + difference.clamp(-max_delta, max_delta);
    result %= 360.0;
    if result >= 180.0 {
        result -= 360.0;
    }
    if result < -180.0 {
        result += 360.0;
    }
    result
}

fn block_pos_below_that_affects_movement(position: Vec3d) -> BlockPos {
    BlockPos::containing(Vec3d::new(position.x, position.y - 0.5000001, position.z))
}

fn block_state_or_air(
    block_state_at: &dyn Fn(BlockPos) -> Option<BlockStateId>,
    position: BlockPos,
) -> BlockStateId {
    block_state_at(position).unwrap_or(BlockStateId(mclone_blocks::terrain_id::AIR))
}

fn collide_mob_movement(
    block_state_at: &dyn Fn(BlockPos) -> Option<BlockStateId>,
    bounding_box: Aabb,
    requested: Vec3d,
    max_up_step: f64,
    was_on_ground: bool,
) -> Vec3d {
    let collided = collide_movement(block_state_at, bounding_box, requested);
    let collision = collide_movement_result(requested, collided);
    let can_try_step = max_up_step > 0.0
        && (was_on_ground || (collision.vertical_collision && requested.y < 0.0))
        && collision.horizontal_collision;
    if !can_try_step {
        return collided;
    }

    let direct_step = collide_movement(
        block_state_at,
        bounding_box,
        Vec3d::new(requested.x, max_up_step, requested.z),
    );
    let vertical_step = collide_movement(
        block_state_at,
        bounding_box.expand_towards(Vec3d::new(requested.x, 0.0, requested.z)),
        Vec3d::new(0.0, max_up_step, 0.0),
    );
    let stepped = if vertical_step.y < max_up_step {
        let horizontal_after_step = collide_movement(
            block_state_at,
            bounding_box.move_by(vertical_step),
            Vec3d::new(requested.x, 0.0, requested.z),
        )
        .add(vertical_step);
        if horizontal_distance_sqr(horizontal_after_step) > horizontal_distance_sqr(direct_step) {
            horizontal_after_step
        } else {
            direct_step
        }
    } else {
        direct_step
    };

    if horizontal_distance_sqr(stepped) <= horizontal_distance_sqr(collided) {
        return collided;
    }

    let step_down = collide_movement(
        block_state_at,
        bounding_box.move_by(stepped),
        Vec3d::new(0.0, -stepped.y + requested.y, 0.0),
    );
    stepped.add(step_down)
}

fn horizontal_distance_sqr(movement: Vec3d) -> f64 {
    movement.x * movement.x + movement.z * movement.z
}

fn nearly_equal(a: f64, b: f64) -> bool {
    (a - b).abs() < MOB_COLLISION_EPSILON
}

fn mob_random_seed(id: EntityId, kind: EntityKind) -> i64 {
    let kind_id = match kind {
        EntityKind::Cow => 0x00c0_0001_u64,
        EntityKind::Chicken => 0x00c0_0002_u64,
        EntityKind::Mallard => 0x00c0_0003_u64,
        EntityKind::Deer => 0x00c0_0007_u64,
        EntityKind::Item => 0x00c0_0003_u64,
        EntityKind::Mannequin => 0x00c0_0004_u64,
        EntityKind::DebugCube => 0x00c0_00ff_u64,
        EntityKind::MallardNest => 0x00c0_0006_u64,
        EntityKind::DeerBed => 0x00c0_0008_u64,
        EntityKind::Bee => 0x00c0_0009_u64,
        EntityKind::BeeNest => 0x00c0_000a_u64,
        EntityKind::BeeHotel => 0x00c0_000b_u64,
        EntityKind::Rabbit => 0x00c0_000c_u64,
        EntityKind::RabbitBurrow => 0x00c0_000d_u64,
        EntityKind::WildlifeRemains => 0x00c0_000e_u64,
        EntityKind::SleepingMat => 0x00c0_000f_u64,
        EntityKind::Squirrel => 0x00c0_0010_u64,
    };
    let mixed = id.0.wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(17) ^ kind_id;
    mixed as i64
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::entity::metadata::EntityMetadata;

    fn stepped_ground(pos: BlockPos) -> Option<BlockStateId> {
        let floor_y = if pos.x <= 0 { 63 } else { 62 };
        Some(if pos.y == floor_y {
            BlockStateId(1)
        } else {
            BlockStateId(mclone_blocks::terrain_id::AIR)
        })
    }

    fn flat_ground(pos: BlockPos) -> Option<BlockStateId> {
        Some(if pos.y == 63 {
            BlockStateId(1)
        } else {
            BlockStateId(mclone_blocks::terrain_id::AIR)
        })
    }

    fn ice_ground(pos: BlockPos) -> Option<BlockStateId> {
        Some(if pos.y == 63 {
            BlockStateId(mclone_blocks::terrain_id::ICE)
        } else {
            BlockStateId(mclone_blocks::terrain_id::AIR)
        })
    }

    fn no_blocks(_pos: BlockPos) -> Option<BlockStateId> {
        None
    }

    fn squirrel_tree(pos: BlockPos) -> Option<BlockStateId> {
        use mclone_worldgen::block::{AIR, GRASS_BLOCK, OAK_LEAVES, OAK_LOG};

        Some(generated_block_state_id(if pos.y <= 63 {
            GRASS_BLOCK
        } else if pos.x == 3 && pos.z == 0 && (64..=68).contains(&pos.y) {
            OAK_LOG
        } else if (68..=70).contains(&pos.y) && (pos.x - 3).abs() <= 2 && pos.z.abs() <= 2 {
            OAK_LEAVES
        } else {
            AIR
        }))
    }

    fn saved_rabbit(
        refuge: EntityPersistentId,
        behavior: mclone_protocol::RabbitBehavior,
        behavior_ticks: u32,
        raid_cooldown: u32,
    ) -> RabbitRuntimeSaveData {
        RabbitRuntimeSaveData {
            known_refuges: [
                Some(KnownPlace::observed(refuge, BlockPos::new(0, 64, 0), 0)),
                None,
                None,
            ],
            sheltered_in: matches!(
                behavior,
                mclone_protocol::RabbitBehavior::EnterBurrow
                    | mclone_protocol::RabbitBehavior::Underground
            )
            .then_some(refuge),
            dig_target: None,
            decision_schedule: crate::ecology::DecisionSchedule::new(0, 0),
            dig_cooldown: 0,
            life_stage: mclone_protocol::RabbitLifeStage::Adult,
            parents: [None, None],
            behavior,
            behavior_ticks,
            health: 3,
            max_health: 3,
            love_ticks: 0,
            raid_cooldown,
            lifecycle: crate::ecology::WildlifeLifeState::founder(refuge, 24_000, 480_000, 120_000),
        }
    }

    #[test]
    fn squirrel_alarms_flees_climbs_and_reacts_to_refuge_loss() {
        use mclone_protocol::SquirrelBehavior;

        let id = EntityId(94);
        let mut entity = ServerEntityState::from_metadata(
            id,
            EntityPersistentId::new(0, 94),
            EntityMetadata::SQUIRREL,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        let mut mob = MobRuntimeState::from_spawn(id, EntityMetadata::SQUIRREL, true, 0.0);
        let threat = MobPlayerTarget::from_position(Vec3d::new(0.5, 64.0, 1.5));
        let mut observed = Vec::new();
        let mut climb_heights = Vec::new();

        for _ in 0..240 {
            entity.tick_count += 1;
            mob.tick_entity(&mut entity, &[threat], &[], &[], &squirrel_tree);
            let behavior = mob.squirrel_behavior().expect("squirrel behavior");
            if observed.last() != Some(&behavior) {
                observed.push(behavior);
            }
            if behavior == SquirrelBehavior::Climb {
                climb_heights.push(entity.position.y);
                let body = collision_aabb_for_feet_position(
                    entity.position,
                    f64::from(entity.width),
                    f64::from(entity.height),
                );
                assert!(
                    mclone_blocks::solid_block_aabbs_in(squirrel_tree, body).is_empty(),
                    "squirrel climb body intersected tree geometry at {:?}",
                    entity.position
                );
            }
            if behavior == SquirrelBehavior::RefugeIdle {
                break;
            }
        }

        for expected in [
            SquirrelBehavior::Alarm,
            SquirrelBehavior::Flee,
            SquirrelBehavior::TrunkApproach,
            SquirrelBehavior::Climb,
            SquirrelBehavior::RefugeEnter,
            SquirrelBehavior::RefugeIdle,
        ] {
            assert!(
                observed.contains(&expected),
                "missing {expected:?}: {observed:?}"
            );
        }
        assert!(climb_heights.len() > 2, "climb must span ordinary ticks");
        assert!(
            climb_heights
                .windows(2)
                .all(|pair| pair[1] - pair[0] <= SQUIRREL_CLIMB_SPEED + 1.0e-9),
            "refuge entry must not teleport"
        );
        let refuge_height = entity.position.y;
        assert!(entity.on_ground, "canopy perch must have leaf support");
        let route = mob.squirrel_refuge_route.expect("accepted refuge route");

        let support_present = Cell::new(true);
        let damaged_tree = |pos: BlockPos| {
            if !support_present.get() && pos == route.refuge.below() {
                Some(generated_block_state_id(mclone_worldgen::block::AIR))
            } else {
                squirrel_tree(pos)
            }
        };
        support_present.set(false);
        entity.tick_count += 1;
        mob.tick_entity(&mut entity, &[], &[], &[], &damaged_tree);
        assert!(mob.squirrel_save_data().unwrap().refuge.is_none());
        assert!(entity.position.y < refuge_height);
        assert_ne!(
            mob.squirrel_behavior(),
            Some(SquirrelBehavior::RefugeIdle),
            "lost support must invalidate refuge occupancy"
        );
    }

    #[test]
    fn squirrel_reuses_a_valid_refuge_route_after_descending() {
        use mclone_protocol::SquirrelBehavior;

        let id = EntityId(95);
        let mut entity = ServerEntityState::from_metadata(
            id,
            EntityPersistentId::new(0, 95),
            EntityMetadata::SQUIRREL,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        let mut mob = MobRuntimeState::from_spawn(id, EntityMetadata::SQUIRREL, true, 0.0);
        let threat = MobPlayerTarget::from_position(Vec3d::new(0.5, 64.0, 1.5));

        for _ in 0..240 {
            entity.tick_count += 1;
            mob.tick_entity(&mut entity, &[threat], &[], &[], &squirrel_tree);
            if mob.squirrel_behavior() == Some(SquirrelBehavior::RefugeIdle) {
                break;
            }
        }
        assert_eq!(
            mob.squirrel_behavior(),
            Some(SquirrelBehavior::RefugeIdle),
            "first threat must drive the squirrel into its refuge"
        );

        for _ in 0..240 {
            let behavior_before = mob.squirrel_behavior();
            entity.tick_count += 1;
            mob.tick_entity(&mut entity, &[], &[], &[], &squirrel_tree);
            if behavior_before == Some(SquirrelBehavior::RefugeExit)
                || mob.squirrel_behavior() == Some(SquirrelBehavior::RefugeExit)
            {
                let body = collision_aabb_for_feet_position(
                    entity.position,
                    f64::from(entity.width),
                    f64::from(entity.height),
                );
                assert!(
                    mclone_blocks::solid_block_aabbs_in(squirrel_tree, body).is_empty(),
                    "squirrel descent body intersected tree geometry at {:?}",
                    entity.position
                );
            }
            if mob.squirrel_behavior() == Some(SquirrelBehavior::Idle) && entity.on_ground {
                break;
            }
        }
        assert_eq!(mob.squirrel_behavior(), Some(SquirrelBehavior::Idle));
        assert!(entity.on_ground, "squirrel must finish descending");

        let mut observed = Vec::new();
        for _ in 0..240 {
            entity.tick_count += 1;
            mob.tick_entity_at_time_with_ecology(
                &mut entity,
                &[threat],
                &[],
                &[],
                6_000,
                &[],
                RabbitEcologyAdmission::default(),
                SquirrelEcologyAdmission::default(),
                &squirrel_tree,
            );
            let behavior = mob.squirrel_behavior().expect("squirrel behavior");
            if observed.last() != Some(&behavior) {
                observed.push(behavior);
            }
            if behavior == SquirrelBehavior::RefugeIdle {
                break;
            }
        }

        for expected in [
            SquirrelBehavior::Alarm,
            SquirrelBehavior::Flee,
            SquirrelBehavior::TrunkApproach,
            SquirrelBehavior::Climb,
            SquirrelBehavior::RefugeEnter,
            SquirrelBehavior::RefugeIdle,
        ] {
            assert!(
                observed.contains(&expected),
                "second threat missed {expected:?}: {observed:?}"
            );
        }
    }

    #[test]
    fn rabbit_commits_to_a_complete_open_ground_escape_away_from_player() {
        let rabbit_id = EntityId(90);
        let mut entity = ServerEntityState::from_metadata(
            rabbit_id,
            EntityPersistentId::new(0, 90),
            EntityMetadata::RABBIT,
            Vec3d::new(5.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        let mut mob = MobRuntimeState::from_spawn(rabbit_id, EntityMetadata::RABBIT, true, 0.0);
        let player = MobPlayerTarget::from_position(Vec3d::new(0.5, 64.0, 0.5));
        let start_distance = entity.position.distance_to_sqr(player.position);

        mob.tick_entity_at_time(&mut entity, &[player], &[], &[], 12_000, &flat_ground);
        let committed_target = mob
            .rabbit_escape_target_for_test()
            .expect("threat should receive one complete escape path");
        assert_eq!(
            mob.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::Flee)
        );
        assert!(committed_target.distance_to_sqr(player.position) > start_distance);

        for _ in 0..10 {
            entity.tick_count += 1;
            mob.tick_entity_at_time(&mut entity, &[player], &[], &[], 12_000, &flat_ground);
            assert_eq!(
                mob.rabbit_escape_target_for_test(),
                Some(committed_target),
                "an accepted route should not be replaced every gameplay tick"
            );
        }

        for _ in 0..120 {
            entity.tick_count += 1;
            mob.tick_entity_at_time(&mut entity, &[player], &[], &[], 12_000, &flat_ground);
            if entity.position.distance_to_sqr(player.position) > RABBIT_FLEE_EXIT_RADIUS_SQR {
                break;
            }
        }
        assert!(entity.position.distance_to_sqr(player.position) > RABBIT_FLEE_EXIT_RADIUS_SQR);
        assert!(entity.position.x > 10.5);
    }

    #[test]
    fn rabbit_uses_only_the_selected_carrot_threat_signal() {
        let position = Vec3d::new(5.5, 64.0, 0.5);
        let player_position = Vec3d::new(0.5, 64.0, 0.5);
        let mut unselected_entity = ServerEntityState::from_metadata(
            EntityId(91),
            EntityPersistentId::new(0, 91),
            EntityMetadata::RABBIT,
            position,
            0.0,
            0.0,
            None,
            true,
        );
        let mut unselected =
            MobRuntimeState::from_spawn(EntityId(91), EntityMetadata::RABBIT, true, 0.0);
        unselected.tick_entity_at_time(
            &mut unselected_entity,
            &[MobPlayerTarget::from_position_with_carrot(
                player_position,
                false,
            )],
            &[],
            &[],
            12_000,
            &flat_ground,
        );
        assert_eq!(
            unselected.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::Flee)
        );

        let mut selected_entity = ServerEntityState::from_metadata(
            EntityId(92),
            EntityPersistentId::new(0, 92),
            EntityMetadata::RABBIT,
            position,
            0.0,
            0.0,
            None,
            true,
        );
        let mut selected =
            MobRuntimeState::from_spawn(EntityId(92), EntityMetadata::RABBIT, true, 0.0);
        selected.tick_entity_at_time(
            &mut selected_entity,
            &[MobPlayerTarget::from_position_with_carrot(
                player_position,
                true,
            )],
            &[],
            &[],
            12_000,
            &flat_ground,
        );
        assert_eq!(
            selected.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::Hop)
        );
        assert_eq!(
            selected.rabbit_habitat_intent.map(|intent| intent.kind),
            Some(RabbitIntentKind::Tempt)
        );
    }

    #[test]
    fn rabbit_escape_does_not_route_to_a_known_refuge() {
        let rabbit_id = EntityId(93);
        let refuge_id = EntityPersistentId::new(0, 930);
        let refuge_position = Vec3d::new(2.5, 64.0, 0.5);
        let mut entity = ServerEntityState::from_metadata(
            rabbit_id,
            EntityPersistentId::new(0, 93),
            EntityMetadata::RABBIT,
            Vec3d::new(5.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        let mut mob = MobRuntimeState::from_saved(
            rabbit_id,
            EntityMetadata::RABBIT,
            true,
            0.0,
            Vec3d::ZERO,
            None,
            None,
            None,
            None,
            Some(saved_rabbit(
                refuge_id,
                mclone_protocol::RabbitBehavior::Idle,
                0,
                0,
            )),
        );
        mob.rabbit_refuge = Some((refuge_id, refuge_position));
        let refuge = RabbitRefugeCandidate {
            persistent_id: refuge_id,
            position: refuge_position,
            capacity: 1,
            occupancy: 1,
            disturbed: false,
        };
        let player = MobPlayerTarget::from_position(Vec3d::new(0.5, 64.0, 0.5));

        mob.tick_entity_at_time_with_ecology(
            &mut entity,
            &[player],
            &[],
            &[],
            12_000,
            &[refuge],
            RabbitEcologyAdmission::UNBOUNDED,
            SquirrelEcologyAdmission::UNBOUNDED,
            &flat_ground,
        );

        let escape = mob.rabbit_habitat_intent.expect("escape intent");
        assert_eq!(escape.kind, RabbitIntentKind::Escape);
        assert_ne!(escape.target, refuge_position);
        assert!(escape.target.x > entity.position.x);
        assert_eq!(mob.rabbit_refuge_claim(), None);
    }

    #[test]
    fn rabbit_retries_an_off_axis_escape_when_direct_endpoint_is_enclosed() {
        let rabbit_id = EntityId(94);
        let mut entity = ServerEntityState::from_metadata(
            rabbit_id,
            EntityPersistentId::new(0, 94),
            EntityMetadata::RABBIT,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        let mut mob = MobRuntimeState::from_spawn(rabbit_id, EntityMetadata::RABBIT, true, 0.0);
        let blocks = |pos: BlockPos| {
            let enclosed_direct_target = (-11..=-9).contains(&pos.x)
                && (-1..=1).contains(&pos.z)
                && (64..=65).contains(&pos.y)
                && pos != BlockPos::new(-10, 64, 0);
            Some(generated_block_state_id(if pos.y == 63 {
                GRASS_BLOCK
            } else if enclosed_direct_target {
                DIRT
            } else {
                mclone_worldgen::block::AIR
            }))
        };
        let player = MobPlayerTarget::from_position(Vec3d::new(6.5, 64.0, 0.5));

        mob.tick_entity_at_time(&mut entity, &[player], &[], &[], 12_000, &blocks);
        assert_eq!(mob.rabbit_escape_target_for_test(), None);
        entity.tick_count += 1;
        mob.tick_entity_at_time(&mut entity, &[player], &[], &[], 12_000, &blocks);
        let target = mob
            .rabbit_escape_target_for_test()
            .expect("the next bounded candidate should have a complete route");

        assert!(target.z.abs() > 1.5);
        assert!(
            target.distance_to_sqr(player.position)
                > entity.position.distance_to_sqr(player.position)
        );
    }

    #[test]
    fn nearby_player_cannot_interrupt_entry_or_force_underground_emergence() {
        let rabbit_id = EntityId(95);
        let home = EntityPersistentId::new(0, 950);
        let position = Vec3d::new(0.5, 64.0, 0.5);
        let mut entity = ServerEntityState::from_metadata(
            rabbit_id,
            EntityPersistentId::new(0, 95),
            EntityMetadata::RABBIT,
            position,
            0.0,
            0.0,
            None,
            true,
        );
        let mut mob = MobRuntimeState::from_saved(
            rabbit_id,
            EntityMetadata::RABBIT,
            true,
            0.0,
            Vec3d::ZERO,
            None,
            None,
            None,
            None,
            Some(saved_rabbit(
                home,
                mclone_protocol::RabbitBehavior::EnterBurrow,
                0,
                0,
            )),
        );
        mob.rabbit_refuge = Some((home, position));
        let close_player = MobPlayerTarget::from_position(Vec3d::new(1.5, 64.0, 0.5));

        for _ in 0..RABBIT_ENTRY_TICKS {
            mob.tick_entity_at_time(&mut entity, &[close_player], &[], &[], 12_000, &flat_ground);
            entity.tick_count += 1;
        }
        assert_eq!(
            mob.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::Underground)
        );
        assert!(entity.hidden_from_clients);

        // Nine blocks is outside the initial alarm radius but inside the
        // continuation radius. Because this rabbit saw the closer threat while
        // entering, it should wait for the larger radius to clear.
        let continuation_player = MobPlayerTarget::from_position(Vec3d::new(9.5, 64.0, 0.5));
        for _ in 0..=RABBIT_UNDERGROUND_MIN_TICKS + 20 {
            mob.tick_entity_at_time(
                &mut entity,
                &[continuation_player],
                &[],
                &[],
                12_000,
                &flat_ground,
            );
            entity.tick_count += 1;
        }
        assert_eq!(
            mob.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::Underground)
        );
        assert!(entity.hidden_from_clients);
    }

    #[test]
    fn bee_flight_uses_a_continuous_elapsed_wingbeat_phase() {
        let mut entity = ServerEntityState::from_metadata(
            EntityId(41),
            mclone_protocol::EntityPersistentId::new(0, 41),
            EntityMetadata::BEE,
            Vec3d::new(0.5, 66.0, 0.5),
            0.0,
            0.0,
            None,
            false,
        );
        entity.tick_count = 120;
        entity.animation = Some(AnimationState::distance(
            AnimationClipId::from_static("fly"),
            5,
        ));

        set_bee_animation(&mut entity, mclone_protocol::BeeBehavior::FlyToFlower);
        let outbound = entity.animation.expect("outbound flight animation");
        assert_eq!(outbound.clip.as_str(), "fly");
        assert_eq!(
            outbound.phase_source,
            mclone_core::AnimationPhaseSource::Elapsed
        );
        assert_eq!(outbound.start_tick, 120);
        assert_eq!(outbound.epoch, 6);

        entity.tick_count = 145;
        set_bee_animation(&mut entity, mclone_protocol::BeeBehavior::FlyToFlower);
        assert_eq!(entity.animation, Some(outbound));

        set_bee_animation(&mut entity, mclone_protocol::BeeBehavior::Forage);
        entity.tick_count = 180;
        set_bee_animation(&mut entity, mclone_protocol::BeeBehavior::ReturnHome);
        let returning = entity.animation.expect("return flight animation");
        assert_eq!(returning.clip.as_str(), "fly");
        assert_eq!(
            returning.phase_source,
            mclone_core::AnimationPhaseSource::Elapsed
        );
        assert_eq!(returning.start_tick, 180);
        assert!(returning.epoch > outbound.epoch);
    }

    #[test]
    fn unhomed_rabbit_forages_before_shelter_need_drives_one_real_dig() {
        let metadata = EntityMetadata::RABBIT;
        let mut entity = ServerEntityState::from_metadata(
            EntityId(77),
            EntityPersistentId::new(0, 77),
            metadata,
            Vec3d::new(1.5, 64.0, 0.5),
            -90.0,
            0.0,
            None,
            true,
        );
        let mut mob = MobRuntimeState::from_spawn(EntityId(77), metadata, true, -90.0);
        let bank = |pos: BlockPos| {
            let raw = if pos.y <= 62 {
                DIRT
            } else if pos.y == 63 {
                GRASS_BLOCK
            } else if matches!((pos.x, pos.y, pos.z), (3, 64, 0) | (3, 65, 0) | (4, 64, 0)) {
                DIRT
            } else {
                mclone_worldgen::block::AIR
            };
            Some(generated_block_state_id(raw))
        };

        for _ in 0..40 {
            mob.tick_entity_at_time(&mut entity, &[], &[], &[], 12_000, &bank);
            entity.tick_count += 1;
            assert_eq!(mob.take_rabbit_completed_dig(), None);
        }

        let mut completed = None;
        for _ in 0..220 {
            mob.tick_entity_at_time(&mut entity, &[], &[], &[], 6_000, &bank);
            entity.tick_count += 1;
            completed = mob.take_rabbit_completed_dig();
            if completed.is_some() {
                break;
            }
        }

        assert_eq!(completed, Some(BlockPos::new(3, 64, 0)));
        assert_eq!(
            mob.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::Idle)
        );
        assert!(entity.position.x > 2.0);
    }

    #[test]
    fn rabbit_reuses_a_negative_dig_search_until_movement_or_block_change() {
        let metadata = EntityMetadata::RABBIT;
        let mut entity = ServerEntityState::from_metadata(
            EntityId(78),
            EntityPersistentId::new(0, 78),
            metadata,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        let mut mob = MobRuntimeState::from_spawn(EntityId(78), metadata, true, 0.0);
        let calls = Cell::new(0_u32);
        let unavailable = |_pos: BlockPos| {
            calls.set(calls.get() + 1);
            None
        };

        assert_eq!(
            mob.select_rabbit_dig_site(entity.position, &unavailable),
            None
        );
        let first_calls = calls.get();
        assert!(first_calls > 0);
        assert_eq!(
            mob.select_rabbit_dig_site(entity.position, &unavailable),
            None
        );
        assert_eq!(calls.get(), first_calls);

        entity.position = entity.position.add(Vec3d::new(1.0, 0.0, 0.0));
        assert_eq!(
            mob.select_rabbit_dig_site(entity.position, &unavailable),
            None
        );
        let moved_calls = calls.get();
        assert!(moved_calls > first_calls);

        assert!(mob.on_block_changed(entity, BlockPos::new(1, 64, 1)));
        assert_eq!(
            mob.select_rabbit_dig_site(entity.position, &unavailable),
            None
        );
        assert!(calls.get() > moved_calls);
    }

    #[test]
    fn unavailable_familiar_refuge_stays_known_while_local_capacity_is_reused() {
        let rabbit_id = EntityId(86);
        let familiar = EntityPersistentId::new(0, 860);
        let local = EntityPersistentId::new(0, 861);
        let mut entity = ServerEntityState::from_metadata(
            rabbit_id,
            EntityPersistentId::new(0, 86),
            EntityMetadata::RABBIT,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        let mut saved = saved_rabbit(familiar, mclone_protocol::RabbitBehavior::Idle, 0, 0);
        saved.known_refuges[0] = Some(KnownPlace::observed(familiar, BlockPos::new(96, 64, 0), 3));
        let mut mob = MobRuntimeState::from_saved(
            rabbit_id,
            EntityMetadata::RABBIT,
            true,
            0.0,
            Vec3d::ZERO,
            None,
            None,
            None,
            None,
            Some(saved),
        );
        let local_refuge = RabbitRefugeCandidate {
            persistent_id: local,
            position: Vec3d::new(4.5, 64.0, 0.5),
            capacity: 2,
            occupancy: 0,
            disturbed: false,
        };

        mob.tick_entity_at_time_with_ecology(
            &mut entity,
            &[],
            &[],
            &[],
            6_000,
            &[local_refuge],
            RabbitEcologyAdmission::UNBOUNDED,
            SquirrelEcologyAdmission::UNBOUNDED,
            &flat_ground,
        );

        let saved = mob.rabbit_save_data().unwrap();
        assert!(saved.known_refuges.iter().flatten().any(|known| {
            known.locator.persistent_id == familiar
                && known.locator.last_known_position == Some(BlockPos::new(96, 64, 0))
        }));
        assert!(
            saved
                .known_refuges
                .iter()
                .flatten()
                .any(|known| known.locator.persistent_id == local)
        );
        assert_eq!(mob.rabbit_refuge_claim(), Some(local));
        assert_eq!(mob.take_rabbit_completed_dig(), None);
        assert_eq!(
            mob.rabbit_habitat_intent.map(|intent| intent.kind),
            Some(RabbitIntentKind::Home)
        );
    }

    #[test]
    fn rabbit_orders_real_mature_carrot_candidates_by_distance() {
        let crop = BlockPos::new(8, 64, 0);
        let blocks = |pos: BlockPos| {
            Some(generated_block_state_id(if pos == crop {
                CARROTS_AGE_7
            } else if pos.y == 63 {
                mclone_worldgen::block::FARMLAND_MOISTURE_7
            } else {
                mclone_worldgen::block::AIR
            }))
        };

        assert_eq!(
            rabbit_carrot_targets(Vec3d::new(0.5, 64.0, 0.5), &blocks),
            vec![crop]
        );
    }

    #[test]
    fn rabbit_finishes_foraging_and_reconsiders_a_mature_carrot() {
        let rabbit_id = EntityId(78);
        let persistent_id = EntityPersistentId::new(0, 78);
        let home = EntityPersistentId::new(0, 79);
        let mut entity = ServerEntityState::from_metadata(
            rabbit_id,
            persistent_id,
            EntityMetadata::RABBIT,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        let mut mob = MobRuntimeState::from_saved(
            rabbit_id,
            EntityMetadata::RABBIT,
            true,
            0.0,
            Vec3d::ZERO,
            None,
            None,
            None,
            None,
            Some(saved_rabbit(
                home,
                mclone_protocol::RabbitBehavior::Forage,
                RABBIT_FORAGE_TICKS,
                0,
            )),
        );
        mob.rabbit_refuge = Some((home, entity.position));
        mob.rabbit_habitat_intent = Some(RabbitHabitatIntent {
            kind: RabbitIntentKind::Forage,
            target: entity.position,
            block: None,
            ticks_remaining: RABBIT_INTENT_TICKS,
            stall_ticks: 0,
        });
        let crop = BlockPos::new(8, 64, 0);
        let blocks = |pos: BlockPos| {
            Some(generated_block_state_id(if pos == crop {
                CARROTS_AGE_7
            } else if pos.y == 63 {
                mclone_worldgen::block::FARMLAND_MOISTURE_7
            } else {
                mclone_worldgen::block::AIR
            }))
        };

        mob.tick_entity_at_time(&mut entity, &[], &[], &[], 12_000, &blocks);

        assert_eq!(
            mob.rabbit_habitat_intent.map(|intent| intent.kind),
            Some(RabbitIntentKind::Raid)
        );
        assert_eq!(
            mob.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::Hop)
        );
        assert!(entity.position.x > 0.5);
    }

    #[test]
    fn rabbit_cancels_a_raid_immediately_when_its_crop_changes() {
        let crop = BlockPos::new(1, 64, 0);
        let rabbit_id = EntityId(82);
        let mut entity = ServerEntityState::from_metadata(
            rabbit_id,
            EntityPersistentId::new(0, 82),
            EntityMetadata::RABBIT,
            Vec3d::new(1.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        let mut mob = MobRuntimeState::from_saved(
            rabbit_id,
            EntityMetadata::RABBIT,
            true,
            0.0,
            Vec3d::ZERO,
            None,
            None,
            None,
            None,
            Some(saved_rabbit(
                EntityPersistentId::new(0, 83),
                mclone_protocol::RabbitBehavior::Raid,
                4,
                0,
            )),
        );
        mob.rabbit_refuge = Some((EntityPersistentId::new(0, 83), Vec3d::new(0.5, 64.0, 0.5)));
        mob.rabbit_habitat_intent = Some(RabbitHabitatIntent {
            kind: RabbitIntentKind::Raid,
            target: entity.position,
            block: Some(crop),
            ticks_remaining: RABBIT_INTENT_TICKS,
            stall_ticks: 0,
        });

        assert!(mob.on_block_changed(entity, crop));
        assert_eq!(mob.rabbit_habitat_intent, None);
        assert_eq!(
            mob.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::Forage)
        );
        mob.tick_entity_at_time(&mut entity, &[], &[], &[], 12_000, &flat_ground);
        assert_eq!(mob.take_rabbit_completed_raid(), None);
        assert_ne!(
            mob.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::Raid)
        );
    }

    #[test]
    fn housed_rabbit_takes_a_staggered_active_rest_and_reemerges_with_same_identity() {
        let rabbit_id = EntityId(84);
        let persistent_id = EntityPersistentId::new(0, 84);
        let home = EntityPersistentId::new(0, 85);
        let home_position = Vec3d::new(0.5, 64.0, 0.5);
        let mut entity = ServerEntityState::from_metadata(
            rabbit_id,
            persistent_id,
            EntityMetadata::RABBIT,
            home_position,
            0.0,
            0.0,
            None,
            true,
        );
        entity.tick_count = (RABBIT_ACTIVE_REST_WARMUP_TICKS
            ..RABBIT_ACTIVE_REST_WARMUP_TICKS + RABBIT_ACTIVE_REST_INTERVAL_TICKS)
            .find(|tick| {
                entity.tick_count = *tick;
                rabbit_active_rest_due(entity, None)
            })
            .expect("one rest phase must occur in each interval");
        let mut mob = MobRuntimeState::from_saved(
            rabbit_id,
            EntityMetadata::RABBIT,
            true,
            0.0,
            Vec3d::ZERO,
            None,
            None,
            None,
            None,
            Some(saved_rabbit(
                home,
                mclone_protocol::RabbitBehavior::Idle,
                0,
                600,
            )),
        );
        mob.rabbit_refuge = Some((home, home_position));

        mob.tick_entity_at_time(&mut entity, &[], &[], &[], 12_000, &flat_ground);
        assert_eq!(
            mob.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::EnterBurrow)
        );
        assert!(!entity.hidden_from_clients);
        for _ in 0..RABBIT_ENTRY_TICKS {
            entity.tick_count += 1;
            mob.tick_entity_at_time(&mut entity, &[], &[], &[], 12_000, &flat_ground);
        }
        assert_eq!(
            mob.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::Underground)
        );
        assert!(entity.hidden_from_clients);

        for _ in 0..=RABBIT_UNDERGROUND_MIN_TICKS {
            entity.tick_count += 1;
            mob.tick_entity_at_time(&mut entity, &[], &[], &[], 12_000, &flat_ground);
            if !entity.hidden_from_clients {
                break;
            }
        }
        assert_eq!(
            mob.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::Emerge)
        );
        assert!(!entity.hidden_from_clients);
        assert_eq!(entity.persistent_id, persistent_id);
    }

    #[test]
    fn rabbit_replans_through_an_off_axis_fence_gap_opened_after_targeting() {
        use std::cell::Cell;

        let rabbit_id = EntityId(79);
        let home = EntityPersistentId::new(0, 80);
        let mut entity = ServerEntityState::from_metadata(
            rabbit_id,
            EntityPersistentId::new(0, 79),
            EntityMetadata::RABBIT,
            Vec3d::new(0.5, 64.0, 0.5),
            -90.0,
            0.0,
            None,
            true,
        );
        let mut mob = MobRuntimeState::from_saved(
            rabbit_id,
            EntityMetadata::RABBIT,
            true,
            -90.0,
            Vec3d::ZERO,
            None,
            None,
            None,
            None,
            Some(saved_rabbit(
                home,
                mclone_protocol::RabbitBehavior::Forage,
                RABBIT_FORAGE_TICKS,
                0,
            )),
        );
        mob.rabbit_refuge = Some((home, entity.position));

        let crop = BlockPos::new(8, 64, 0);
        let gap = BlockPos::new(4, 64, 3);
        let gap_open = Cell::new(false);
        let blocks = |pos: BlockPos| {
            Some(if pos == crop {
                generated_block_state_id(CARROTS_AGE_7)
            } else if pos.y == 63 {
                generated_block_state_id(mclone_worldgen::block::FARMLAND_MOISTURE_7)
            } else if pos.x == 4
                && pos.y == 64
                && (-20..=20).contains(&pos.z)
                && !(gap_open.get() && pos == gap)
            {
                BlockStateId(mclone_blocks::terrain_id::OAK_FENCE_STATE_START)
            } else {
                BlockStateId(mclone_blocks::terrain_id::AIR)
            })
        };

        mob.tick_entity_at_time(&mut entity, &[], &[], &[], 12_000, &blocks);
        entity.tick_count += 1;
        assert_ne!(
            mob.rabbit_habitat_intent.map(|intent| intent.kind),
            Some(RabbitIntentKind::Raid),
            "a bounded partial path must not make the enclosed crop reachable"
        );
        assert!(!mob.on_block_changed(entity, BlockPos::new(40, 64, 40)));

        gap_open.set(true);
        assert!(mob.on_block_changed(entity, gap));
        let mut max_z = entity.position.z;
        let mut completed_raid = None;
        for _ in 0..700 {
            mob.tick_entity_at_time(&mut entity, &[], &[], &[], 12_000, &blocks);
            entity.tick_count += 1;
            max_z = max_z.max(entity.position.z);
            completed_raid = mob.take_rabbit_completed_raid();
            if completed_raid.is_some() {
                break;
            }
        }

        assert_eq!(completed_raid, Some(crop));
        assert!(
            max_z > 2.5,
            "rabbit must take the off-axis opening instead of steering through the fence"
        );
        assert!(entity.position.x > 7.0);
    }

    #[test]
    fn rabbit_is_visible_during_entry_hidden_deep_inside_and_visible_on_emerge() {
        let rabbit_id = EntityId(81);
        let persistent_id = EntityPersistentId::new(0, 81);
        let home = EntityPersistentId::new(0, 82);
        let mut entity = ServerEntityState::from_metadata(
            rabbit_id,
            persistent_id,
            EntityMetadata::RABBIT,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        let mut mob = MobRuntimeState::from_saved(
            rabbit_id,
            EntityMetadata::RABBIT,
            true,
            0.0,
            Vec3d::ZERO,
            None,
            None,
            None,
            None,
            Some(saved_rabbit(
                home,
                mclone_protocol::RabbitBehavior::EnterBurrow,
                0,
                0,
            )),
        );
        mob.rabbit_refuge = Some((home, entity.position));

        mob.tick_entity_at_time(&mut entity, &[], &[], &[], 6_000, &flat_ground);
        assert_eq!(
            mob.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::EnterBurrow)
        );
        assert!(!entity.hidden_from_clients);

        for _ in 1..RABBIT_ENTRY_TICKS {
            mob.tick_entity_at_time(&mut entity, &[], &[], &[], 6_000, &flat_ground);
        }
        assert_eq!(
            mob.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::Underground)
        );
        assert!(entity.hidden_from_clients);
        assert_eq!(entity.persistent_id, persistent_id);

        for _ in 0..RABBIT_UNDERGROUND_MIN_TICKS {
            mob.tick_entity_at_time(&mut entity, &[], &[], &[], 6_000, &flat_ground);
        }
        assert!(entity.hidden_from_clients);
        let player_outside_initial_radius =
            MobPlayerTarget::from_position(Vec3d::new(9.5, 64.0, 0.5));
        mob.tick_entity_at_time(
            &mut entity,
            &[player_outside_initial_radius],
            &[],
            &[],
            12_000,
            &flat_ground,
        );
        assert_eq!(
            mob.rabbit_behavior(),
            Some(mclone_protocol::RabbitBehavior::Emerge)
        );
        assert!(!entity.hidden_from_clients);
        assert_eq!(entity.persistent_id, persistent_id);
        assert_eq!(entity.width, EntityMetadata::RABBIT.dimensions.width);
    }

    fn one_block_ledge(pos: BlockPos) -> Option<BlockStateId> {
        let floor_y = if pos.x <= 0 { 63 } else { 64 };
        Some(if pos.y == floor_y {
            BlockStateId(1)
        } else {
            BlockStateId(mclone_blocks::terrain_id::AIR)
        })
    }

    fn default_block_friction() -> f64 {
        f64::from(mclone_blocks::DEFAULT_BLOCK_FRICTION)
    }

    #[test]
    fn mob_travel_uses_ground_friction_instead_of_raw_speed_step() {
        let block_friction = default_block_friction();
        let requested = mob_travel_request(
            Vec3d::ZERO,
            EntityMetadata::CHICKEN.movement_speed,
            0.0,
            true,
            block_friction,
        );
        let expected_z = EntityMetadata::CHICKEN.movement_speed
            * MOB_INPUT_DAMPING
            * EntityMetadata::CHICKEN.movement_speed
            * (MOB_FRICTION_INFLUENCE_NUMERATOR / block_friction.powi(3));

        assert_eq!(requested.x, 0.0);
        assert!(requested.z > 0.0);
        assert!(requested.z < EntityMetadata::CHICKEN.movement_speed);
        assert!((requested.z - expected_z).abs() < 1.0e-12);

        let next_delta = mob_delta_after_travel(requested, requested, true, block_friction, 1.0);
        let expected_delta_z = requested.z * block_friction * MOB_GROUND_DRAG_MULTIPLIER;
        assert!((next_delta.z - expected_delta_z).abs() < 1.0e-12);
    }

    #[test]
    fn mob_travel_carries_horizontal_delta_between_control_ticks() {
        let block_friction = default_block_friction();
        let first = mob_travel_request(Vec3d::ZERO, 0.2, 0.0, true, block_friction);
        let first_delta = mob_delta_after_travel(first, first, true, block_friction, 1.0);
        let second = mob_travel_request(first_delta, 0.2, 0.0, true, block_friction);

        assert!(first.z > 0.0);
        assert!(second.z > first.z);
        assert!(second.z < 0.2);
    }

    #[test]
    fn mob_block_movement_facts_sample_below_movement_position() {
        let position = Vec3d::new(0.5, 64.0, 0.5);

        assert_eq!(
            mob_block_friction(&ice_ground, position),
            f64::from(block_friction(BlockStateId(mclone_blocks::terrain_id::ICE)))
        );
        assert_eq!(
            mob_block_speed_factor(&ice_ground, position),
            f64::from(mclone_blocks::DEFAULT_BLOCK_SPEED_FACTOR)
        );
        assert_eq!(
            mob_block_jump_factor(&ice_ground, position),
            f64::from(mclone_blocks::DEFAULT_BLOCK_JUMP_FACTOR)
        );
    }

    #[test]
    fn mob_delta_after_travel_applies_block_speed_factor_before_drag() {
        let requested = Vec3d::new(0.1, 0.0, 0.2);
        let speed_factor = 0.4;
        let block_friction = default_block_friction();
        let delta =
            mob_delta_after_travel(requested, requested, true, block_friction, speed_factor);

        let expected_x = requested.x * speed_factor * block_friction * MOB_GROUND_DRAG_MULTIPLIER;
        let expected_z = requested.z * speed_factor * block_friction * MOB_GROUND_DRAG_MULTIPLIER;
        assert!((delta.x - expected_x).abs() < 1.0e-12);
        assert!((delta.z - expected_z).abs() < 1.0e-12);
    }

    #[test]
    fn cow_runtime_uses_default_water_malus_and_metadata_speed() {
        let metadata = EntityMetadata::for_kind(EntityKind::Cow).unwrap();
        let mob = MobRuntimeState::from_spawn(EntityId(1), metadata, true, 135.0);

        assert_eq!(mob.no_action_time(), 0);
        assert!(mob.on_ground());
        assert_eq!(mob.y_body_rot_degrees(), 135.0);
        assert_eq!(mob.y_head_rot_degrees(), 135.0);
        assert_eq!(mob.movement_speed(), 0.2);
        assert_eq!(mob.follow_range(), 16.0);
        assert_eq!(mob.max_up_step(), 0.6);
        assert_eq!(mob.jump_power(), 0.42);
        assert_eq!(mob.pathfinding_malus(BlockPathType::Water), 8.0);
    }

    #[test]
    fn chicken_runtime_applies_java_water_malus_override() {
        let metadata = EntityMetadata::for_kind(EntityKind::Chicken).unwrap();
        let mob = MobRuntimeState::from_spawn(EntityId(1), metadata, true, 0.0);

        assert_eq!(mob.movement_speed(), 0.25);
        assert_eq!(mob.pathfinding_malus(BlockPathType::Water), 0.0);
        assert_eq!(mob.available_goal_count(), 3);
        let egg_time = mob.chicken_egg_time_for_test().expect("chicken egg time");
        assert!((6_000..12_000).contains(&egg_time));
        assert_eq!(mob.chicken_pending_egg_lays_for_test(), Some(0));
        assert_eq!(mob.chicken_flap_speed_for_test(), Some(0.0));
    }

    #[test]
    fn chicken_runtime_dampens_airborne_falling_delta() {
        let metadata = EntityMetadata::for_kind(EntityKind::Chicken).unwrap();
        let mut mob = MobRuntimeState::from_spawn(EntityId(1), metadata, true, 0.0);
        let mut entity = ServerEntityState::from_metadata(
            EntityId(1),
            mclone_protocol::EntityPersistentId::new(0, 1),
            metadata,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );

        mob.tick_entity(&mut entity, &[], &[], &[], &no_blocks);

        let expected = ((-MOB_GRAVITY * MOB_VERTICAL_DRAG - MOB_GRAVITY) * MOB_VERTICAL_DRAG) * 0.6;
        assert!(!entity.on_ground);
        assert!(entity.position.y < 64.0);
        assert!((mob.delta_movement().y - expected).abs() < 1.0e-12);
        assert_eq!(mob.chicken_flap_speed_for_test(), Some(1.0));
    }

    #[test]
    fn chicken_runtime_counts_and_resets_egg_timer_without_item_spawn() {
        let metadata = EntityMetadata::for_kind(EntityKind::Chicken).unwrap();
        let mut mob = MobRuntimeState::from_spawn(EntityId(1), metadata, true, 0.0);
        let mut entity = ServerEntityState::from_metadata(
            EntityId(1),
            mclone_protocol::EntityPersistentId::new(0, 1),
            metadata,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        mob.set_chicken_egg_time_for_test(1);

        mob.tick_entity(&mut entity, &[], &[], &[], &flat_ground);

        let egg_time = mob
            .chicken_egg_time_for_test()
            .expect("chicken should reset egg time");
        assert!((6_000..12_000).contains(&egg_time));
        assert_eq!(mob.chicken_pending_egg_lays_for_test(), Some(1));
    }

    #[test]
    fn mob_runtime_random_source_is_deterministic_per_entity() {
        let metadata = EntityMetadata::for_kind(EntityKind::Cow).unwrap();
        let mut first = MobRuntimeState::from_spawn(EntityId(42), metadata, true, 0.0);
        let mut second = MobRuntimeState::from_spawn(EntityId(42), metadata, true, 0.0);

        assert_eq!(
            first.next_random_int_bound(10_000),
            second.next_random_int_bound(10_000)
        );
    }

    #[test]
    fn bee_flower_selection_avoids_the_previous_flower_when_an_alternative_exists() {
        let flower_state = generated_block_state_id(DANDELION);
        let air_state = generated_block_state_id(mclone_worldgen::block::AIR);
        let previous = BlockPos::new(4, 64, 0);
        let alternative = BlockPos::new(5, 64, 0);
        let blocks = |pos| {
            Some(
                if matches!(pos, value if value == previous || value == alternative) {
                    flower_state
                } else {
                    air_state
                },
            )
        };
        let mut random = SimpleRandomSource::new(42);

        assert_eq!(
            select_bee_flower(
                Vec3d::new(0.5, 64.0, 0.5),
                0,
                Some(previous),
                &mut random,
                &blocks,
            ),
            Some(alternative)
        );
    }

    #[test]
    fn deer_alerts_flees_and_requires_distance_before_calming() {
        let mut mob = MobRuntimeState::from_spawn(EntityId(77), EntityMetadata::DEER, true, 0.0);
        let mut entity = ServerEntityState::from_metadata(
            EntityId(77),
            mclone_protocol::EntityPersistentId::new(0, 77),
            EntityMetadata::DEER,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        let flat_ground = |pos: BlockPos| {
            Some(generated_block_state_id(if pos.y <= 63 {
                mclone_worldgen::block::GRASS_BLOCK
            } else {
                mclone_worldgen::block::AIR
            }))
        };

        mob.tick_entity(
            &mut entity,
            &[MobPlayerTarget::from_position(Vec3d::new(12.0, 64.0, 0.0))],
            &[],
            &[],
            &flat_ground,
        );
        assert_eq!(
            mob.deer_behavior(),
            Some(mclone_protocol::DeerBehavior::Alert)
        );
        assert_eq!(entity.animation.unwrap().clip.as_str(), "alert");

        mob.tick_entity(
            &mut entity,
            &[MobPlayerTarget::from_position(Vec3d::new(5.0, 64.0, 0.0))],
            &[],
            &[],
            &flat_ground,
        );
        assert_eq!(
            mob.deer_behavior(),
            Some(mclone_protocol::DeerBehavior::Flee)
        );
        assert_eq!(entity.animation.unwrap().clip.as_str(), "flee");

        for _ in 0..DEER_FLEE_MIN_TICKS {
            mob.tick_entity(
                &mut entity,
                &[MobPlayerTarget::from_position(Vec3d::new(80.0, 64.0, 0.0))],
                &[],
                &[],
                &flat_ground,
            );
        }
        assert_eq!(
            mob.deer_behavior(),
            Some(mclone_protocol::DeerBehavior::Alert)
        );
        assert_eq!(entity.animation.unwrap().clip.as_str(), "alert");
    }

    #[test]
    fn deer_bedding_uses_authored_transition_clips() {
        let mut mob = MobRuntimeState::from_spawn(EntityId(78), EntityMetadata::DEER, true, 0.0);
        let mut entity = ServerEntityState::from_metadata(
            EntityId(78),
            mclone_protocol::EntityPersistentId::new(0, 78),
            EntityMetadata::DEER,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        let flat_ground = |pos: BlockPos| {
            Some(generated_block_state_id(if pos.y <= 63 {
                mclone_worldgen::block::GRASS_BLOCK
            } else {
                mclone_worldgen::block::AIR
            }))
        };
        mob.set_deer_behavior_for_test(mclone_protocol::DeerBehavior::LieDown);
        for _ in 0..DEER_TRANSITION_TICKS {
            mob.tick_entity(&mut entity, &[], &[], &[], &flat_ground);
        }
        assert_eq!(
            mob.deer_behavior(),
            Some(mclone_protocol::DeerBehavior::Bedded)
        );
        assert_eq!(entity.animation.unwrap().clip.as_str(), "bedded_idle");
        for _ in 0..160 {
            mob.tick_entity(&mut entity, &[], &[], &[], &flat_ground);
        }
        assert_eq!(
            mob.deer_behavior(),
            Some(mclone_protocol::DeerBehavior::StandUp)
        );
        assert_eq!(entity.animation.unwrap().clip.as_str(), "stand_up");
    }

    #[test]
    fn deer_drink_from_a_dry_bank_without_entering_water() {
        let bank_ground = |pos: BlockPos| {
            use mclone_worldgen::block::{AIR, GRASS_BLOCK, WATER, generated_block_state_id};

            Some(generated_block_state_id(if pos.y == 63 {
                GRASS_BLOCK
            } else if pos.y == 64 && pos.x == 4 {
                WATER
            } else {
                AIR
            }))
        };
        let mut random = SimpleRandomSource::new(31);
        let intent =
            random_deer_safe_bank_target(Vec3d::new(0.5, 64.0, 0.5), &mut random, &bank_ground)
                .expect("reachable dry bank beside water");
        let feet = BlockPos::containing(intent.target);
        assert_eq!(intent.kind, DeerHabitatKind::Water);
        assert!(deer_walkable_feet(feet, &bank_ground));
        assert!(deer_bank_has_water(feet, &bank_ground));

        let mut mob = MobRuntimeState::from_spawn(EntityId(79), EntityMetadata::DEER, true, 0.0);
        let mut entity = ServerEntityState::from_metadata(
            EntityId(79),
            mclone_protocol::EntityPersistentId::new(0, 79),
            EntityMetadata::DEER,
            intent.target,
            0.0,
            0.0,
            None,
            true,
        );
        mob.set_deer_behavior_for_test(mclone_protocol::DeerBehavior::Walk);
        mob.deer_habitat_intent = Some(intent);
        mob.tick_entity(&mut entity, &[], &[], &[], &bank_ground);

        assert_eq!(
            mob.deer_behavior(),
            Some(mclone_protocol::DeerBehavior::Drink)
        );
        assert_eq!(entity.animation.unwrap().clip.as_str(), "graze");
        assert!(deer_walkable_feet(
            BlockPos::containing(entity.position),
            &bank_ground
        ));
    }

    #[test]
    fn sync_from_entity_mirrors_ground_and_body_rotations() {
        let metadata = EntityMetadata::for_kind(EntityKind::Cow).unwrap();
        let mut mob = MobRuntimeState::from_spawn(EntityId(1), metadata, true, 0.0);
        let entity = ServerEntityState::from_metadata(
            EntityId(1),
            mclone_protocol::EntityPersistentId::new(0, 1),
            metadata,
            mclone_core::Vec3d::new(0.0, 64.0, 0.0),
            90.0,
            0.0,
            None,
            false,
        );

        mob.sync_from_entity(entity);

        assert!(!mob.on_ground());
        assert_eq!(mob.y_body_rot_degrees(), 90.0);
        assert_eq!(mob.y_head_rot_degrees(), 90.0);
    }

    #[test]
    fn navigation_move_to_lower_floor_descends_with_collision() {
        let metadata = EntityMetadata::for_kind(EntityKind::Cow).unwrap();
        let mut entity = ServerEntityState::from_metadata(
            EntityId(1),
            mclone_protocol::EntityPersistentId::new(0, 1),
            metadata,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        let mut context = MobGoalContext::from_parts_for_test(
            entity,
            metadata.standing_eye_height() as f64,
            Vec::new(),
            SimpleRandomSource::new(1),
            &stepped_ground,
        );

        assert!(context.move_to(Vec3d::new(2.5, 63.0, 0.5), 1.0));
        for _ in 0..80 {
            context.apply_controls(&mut entity);
            if entity.on_ground && entity.position.x > 1.4 && entity.position.y < 64.0 {
                break;
            }
        }

        assert!(entity.position.x > 1.4);
        assert!(entity.position.y < 64.0);
        assert!(entity.on_ground);
    }

    #[test]
    fn navigation_move_to_one_block_ledge_uses_jump_control() {
        let metadata = EntityMetadata::for_kind(EntityKind::Cow).unwrap();
        let mut entity = ServerEntityState::from_metadata(
            EntityId(1),
            mclone_protocol::EntityPersistentId::new(0, 1),
            metadata,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        let mut context = MobGoalContext::from_parts_for_test(
            entity,
            metadata.standing_eye_height() as f64,
            Vec::new(),
            SimpleRandomSource::new(1),
            &one_block_ledge,
        );

        assert!(context.move_to(Vec3d::new(2.5, 65.0, 0.5), 1.0));
        for _ in 0..240 {
            context.apply_controls(&mut entity);
            if entity.on_ground && entity.position.x > 1.1 && entity.position.y >= 65.0 {
                break;
            }
        }

        assert!(entity.position.x > 1.1);
        assert!(entity.position.y >= 65.0);
        assert!(entity.on_ground);
    }
}
