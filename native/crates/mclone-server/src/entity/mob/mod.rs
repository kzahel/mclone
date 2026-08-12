#![allow(dead_code)]

use std::collections::BTreeMap;

use mclone_blocks::{
    BlockFluidKind, block_fluid_kind, block_friction, block_jump_factor, block_speed_factor,
    collide_movement, collide_movement_result, collision_aabb_for_feet_position,
};
use mclone_core::{Aabb, BlockPos, BlockStateId, Vec3d};
use mclone_protocol::{EntityId, EntityKind};
use mclone_worldgen::prng::SimpleRandomSource;

use super::metadata::EntityMetadata;
use super::spawning::habitat::sample_wetland_habitat;
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
pub(crate) use species::{MALLARD_GROWTH_REQUIRED_TICKS, MallardRuntimeSaveData};

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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MallardFlockmateTarget {
    pub(crate) position: Vec3d,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MallardHabitatKind {
    Water,
    Shore,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MallardHabitatIntent {
    kind: MallardHabitatKind,
    target: Vec3d,
    ticks_remaining: u16,
}

impl MobPlayerTarget {
    pub(crate) fn from_position(position: Vec3d) -> Self {
        Self {
            position,
            eye_y: position.y + PLAYER_EYE_HEIGHT,
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
}

impl MobRuntimeState {
    pub(crate) fn from_spawn(
        id: EntityId,
        metadata: EntityMetadata,
        on_ground: bool,
        y_rot_degrees: f32,
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
        let species = MobSpeciesState::from_spawn(metadata.kind, &mut random);

        let mut goal_selector = GoalSelector::default();
        match metadata.kind {
            EntityKind::Cow => passive::register_cow_goals(&mut goal_selector),
            EntityKind::Chicken => passive::register_chicken_goals(&mut goal_selector),
            EntityKind::Mallard => passive::register_mallard_goals(&mut goal_selector),
            EntityKind::Mannequin => passive::register_mannequin_goals(&mut goal_selector),
            EntityKind::DebugCube | EntityKind::Item | EntityKind::MallardNest => {}
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
        }
    }

    pub(crate) fn from_saved(
        id: EntityId,
        metadata: EntityMetadata,
        on_ground: bool,
        y_rot_degrees: f32,
        delta_movement: Vec3d,
        egg_time: Option<i32>,
        mallard: Option<MallardRuntimeSaveData>,
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
        let species = MobSpeciesState::from_saved(metadata.kind, &mut random, egg_time, mallard);

        let mut goal_selector = GoalSelector::default();
        match metadata.kind {
            EntityKind::Cow => passive::register_cow_goals(&mut goal_selector),
            EntityKind::Chicken => passive::register_chicken_goals(&mut goal_selector),
            EntityKind::Mallard => passive::register_mallard_goals(&mut goal_selector),
            EntityKind::Mannequin => passive::register_mannequin_goals(&mut goal_selector),
            EntityKind::DebugCube | EntityKind::Item | EntityKind::MallardNest => {}
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
        }
    }

    pub(crate) fn sync_from_entity(&mut self, entity: ServerEntityState) {
        self.on_ground = entity.on_ground;
        self.y_body_rot_degrees = entity.y_rot_degrees;
        self.y_head_rot_degrees = entity.y_rot_degrees;
        self.mallard_habitat_intent = None;
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
        self.navigation.recompute_path_around(pos, entity.position)
    }

    pub(crate) fn tick_entity<F>(
        &mut self,
        entity: &mut ServerEntityState,
        nearby_players: &[MobPlayerTarget],
        flockmates: &[MallardFlockmateTarget],
        block_state_at: &F,
    ) where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        self.on_ground = entity.on_ground;
        self.y_body_rot_degrees = entity.y_rot_degrees;

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
        EntityKind::Item => 0x00c0_0003_u64,
        EntityKind::Mannequin => 0x00c0_0004_u64,
        EntityKind::DebugCube => 0x00c0_00ff_u64,
        EntityKind::MallardNest => 0x00c0_0006_u64,
    };
    let mixed = id.0.wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(17) ^ kind_id;
    mixed as i64
}

#[cfg(test)]
mod tests {
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

        mob.tick_entity(&mut entity, &[], &[], &no_blocks);

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

        mob.tick_entity(&mut entity, &[], &[], &flat_ground);

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
