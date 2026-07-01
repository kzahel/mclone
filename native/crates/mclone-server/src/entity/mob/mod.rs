#![allow(dead_code)]

use std::collections::BTreeMap;

use mclone_blocks::{collide_movement, collide_movement_result, collision_aabb_for_feet_position};
use mclone_core::{BlockPos, BlockStateId, Vec3d};
use mclone_protocol::{EntityId, EntityKind};
use mclone_worldgen::prng::SimpleRandomSource;

use super::metadata::EntityMetadata;
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

const PLAYER_EYE_HEIGHT: f64 = 1.62;
const MOB_GRAVITY: f64 = 0.08;
const MOB_VERTICAL_DRAG: f64 = 0.98;
const MOB_COLLISION_EPSILON: f64 = 1.0e-7;

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
            EntityKind::DebugCube | EntityKind::Item => {}
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
        }
    }

    pub(crate) fn sync_from_entity(&mut self, entity: ServerEntityState) {
        self.on_ground = entity.on_ground;
        self.y_body_rot_degrees = entity.y_rot_degrees;
        self.y_head_rot_degrees = entity.y_rot_degrees;
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
        block_state_at: &F,
    ) where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        self.on_ground = entity.on_ground;
        self.y_body_rot_degrees = entity.y_rot_degrees;

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

        let previous_control_position = self.position;
        let move_tick = self.move_control.tick(
            &mut self.position,
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
        let looked = self.look_control.tick(
            self.position,
            &mut self.y_head_rot_degrees,
            self.y_body_rot_degrees,
        );
        let controlled_position = self.position;
        self.position = previous_control_position;

        let requested_y = if jumping && self.on_ground {
            self.attributes.jump_power
        } else {
            self.delta_movement.y
        };
        let requested = Vec3d::new(
            controlled_position.x - previous_control_position.x,
            requested_y,
            controlled_position.z - previous_control_position.z,
        );
        let bounding_box = collision_aabb_for_feet_position(
            previous_control_position,
            f64::from(entity.width),
            f64::from(entity.height),
        );
        let traveled = collide_movement(self.block_state_at, bounding_box, requested);
        if traveled.length_sqr() > MOB_COLLISION_EPSILON * MOB_COLLISION_EPSILON {
            self.position = self.position.add(traveled);
        }
        let collision = collide_movement_result(requested, traveled);
        self.on_ground = collision.on_ground;
        self.delta_movement = Vec3d::new(0.0, (traveled.y - MOB_GRAVITY) * MOB_VERTICAL_DRAG, 0.0);

        if !move_tick.moved && looked {
            // The current entity protocol has one yaw field. Until head/body yaw
            // are split, publish head turns as body yaw so passive looks are
            // visible to clients.
            self.y_body_rot_degrees = self.y_head_rot_degrees;
        }

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

fn mob_random_seed(id: EntityId, kind: EntityKind) -> i64 {
    let kind_id = match kind {
        EntityKind::Cow => 0x00c0_0001_u64,
        EntityKind::Chicken => 0x00c0_0002_u64,
        EntityKind::Item => 0x00c0_0003_u64,
        EntityKind::DebugCube => 0x00c0_00ff_u64,
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
            metadata,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );

        mob.tick_entity(&mut entity, &[], &no_blocks);

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
            metadata,
            Vec3d::new(0.5, 64.0, 0.5),
            0.0,
            0.0,
            None,
            true,
        );
        mob.set_chicken_egg_time_for_test(1);

        mob.tick_entity(&mut entity, &[], &flat_ground);

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
        for _ in 0..120 {
            context.apply_controls(&mut entity);
            if entity.on_ground && entity.position.x > 1.4 && entity.position.y >= 65.0 {
                break;
            }
        }

        assert!(entity.position.x > 1.4);
        assert!(entity.position.y >= 65.0);
        assert!(entity.on_ground);
    }
}
