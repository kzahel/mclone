#![allow(dead_code)]

use std::collections::BTreeMap;

use mclone_core::Vec3d;
use mclone_protocol::{EntityId, EntityKind};
use mclone_worldgen::prng::SimpleRandomSource;

use super::metadata::EntityMetadata;
use super::state::ServerEntityState;

mod control;
pub(crate) mod goals;

use control::{LookControl, MoveControl};
use goals::{GoalSelector, passive};

const PLAYER_EYE_HEIGHT: f64 = 1.62;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum BlockPathType {
    Water,
}

impl BlockPathType {
    pub(crate) const fn default_malus(self) -> f32 {
        match self {
            Self::Water => 8.0,
        }
    }
}

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
    movement_speed: f64,
    eye_height: f64,
    pathfinding_malus: PathfindingMalusTable,
    random: SimpleRandomSource,
    goal_selector: GoalSelector<MobGoalContext>,
    move_control: MoveControl,
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

        let mut goal_selector = GoalSelector::default();
        if metadata.kind == EntityKind::Cow {
            passive::register_cow_goals(&mut goal_selector);
        }

        Self {
            no_action_time: 0,
            on_ground,
            y_body_rot_degrees: y_rot_degrees,
            y_head_rot_degrees: y_rot_degrees,
            movement_speed: metadata.movement_speed,
            eye_height: metadata.standing_eye_height() as f64,
            pathfinding_malus,
            random: SimpleRandomSource::new(mob_random_seed(id, metadata.kind)),
            goal_selector,
            move_control: MoveControl::default(),
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
        self.movement_speed
    }

    pub(crate) const fn eye_height(&self) -> f64 {
        self.eye_height
    }

    pub(crate) fn available_goal_count(&self) -> usize {
        self.goal_selector.available_goal_count()
    }

    pub(crate) fn running_goal_count(&self) -> usize {
        self.goal_selector.running_goal_count()
    }

    pub(crate) fn pathfinding_malus(&self, path_type: BlockPathType) -> f32 {
        self.pathfinding_malus.get(path_type)
    }

    pub(crate) fn next_random_int_bound(&mut self, bound: i32) -> i32 {
        self.random.next_int_bound(bound)
    }

    pub(crate) fn tick_entity(
        &mut self,
        entity: &mut ServerEntityState,
        nearby_players: &[MobPlayerTarget],
    ) {
        self.on_ground = entity.on_ground;
        self.y_body_rot_degrees = entity.y_rot_degrees;

        let random = std::mem::replace(&mut self.random, SimpleRandomSource::new(0));
        let move_control = std::mem::take(&mut self.move_control);
        let look_control = std::mem::take(&mut self.look_control);
        let mut context = MobGoalContext {
            position: entity.position,
            eye_height: self.eye_height,
            y_body_rot_degrees: self.y_body_rot_degrees,
            y_head_rot_degrees: self.y_head_rot_degrees,
            movement_speed: self.movement_speed,
            no_action_time: self.no_action_time,
            on_ground: self.on_ground,
            nearby_players: nearby_players.to_vec(),
            random,
            move_control,
            look_control,
        };

        let mut goal_selector = std::mem::take(&mut self.goal_selector);
        goal_selector.tick(&mut context);
        context.apply_controls(entity);
        self.goal_selector = goal_selector;

        self.no_action_time = context.no_action_time;
        self.on_ground = entity.on_ground;
        self.y_body_rot_degrees = entity.y_rot_degrees;
        self.y_head_rot_degrees = context.y_head_rot_degrees;
        self.random = context.random;
        self.move_control = context.move_control;
        self.look_control = context.look_control;
    }
}

#[derive(Debug)]
pub(crate) struct MobGoalContext {
    position: Vec3d,
    eye_height: f64,
    y_body_rot_degrees: f32,
    y_head_rot_degrees: f32,
    movement_speed: f64,
    no_action_time: u32,
    on_ground: bool,
    nearby_players: Vec<MobPlayerTarget>,
    random: SimpleRandomSource,
    move_control: MoveControl,
    look_control: LookControl,
}

impl MobGoalContext {
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

    pub(crate) fn has_move_target(&self) -> bool {
        self.move_control.has_wanted()
    }

    pub(crate) fn set_move_target(&mut self, position: Vec3d, speed_modifier: f64) {
        self.move_control
            .set_wanted_position(position, speed_modifier);
    }

    pub(crate) fn stop_navigation(&mut self) {
        self.move_control.stop();
    }

    pub(crate) fn set_look_at(&mut self, position: Vec3d) {
        self.look_control.set_look_at(position);
    }

    pub(crate) fn apply_controls(&mut self, entity: &mut ServerEntityState) -> bool {
        let previous_position = entity.position;
        let previous_y_rot = entity.y_rot_degrees;

        let moved = self.move_control.tick(
            &mut self.position,
            &mut self.y_body_rot_degrees,
            self.movement_speed,
        );
        let looked = self.look_control.tick(
            self.position,
            &mut self.y_head_rot_degrees,
            self.y_body_rot_degrees,
        );

        if !moved && looked {
            // The current entity protocol has one yaw field. Until head/body yaw
            // are split, publish head turns as body yaw so passive looks are
            // visible to clients.
            self.y_body_rot_degrees = self.y_head_rot_degrees;
        }

        entity.position = self.position;
        entity.y_rot_degrees = self.y_body_rot_degrees;
        entity.x_rot_degrees = 0.0;
        self.on_ground = entity.on_ground;

        entity.position != previous_position || entity.y_rot_degrees != previous_y_rot
    }

    #[cfg(test)]
    pub(crate) fn from_parts_for_test(
        entity: ServerEntityState,
        eye_height: f64,
        nearby_players: Vec<MobPlayerTarget>,
        random: SimpleRandomSource,
    ) -> Self {
        Self {
            position: entity.position,
            eye_height,
            y_body_rot_degrees: entity.y_rot_degrees,
            y_head_rot_degrees: entity.y_rot_degrees,
            movement_speed: 0.2,
            no_action_time: 0,
            on_ground: entity.on_ground,
            nearby_players,
            random,
            move_control: MoveControl::default(),
            look_control: LookControl::default(),
        }
    }
}

fn mob_random_seed(id: EntityId, kind: EntityKind) -> i64 {
    let kind_id = match kind {
        EntityKind::Cow => 0x00c0_0001_u64,
        EntityKind::Chicken => 0x00c0_0002_u64,
        EntityKind::DebugCube => 0x00c0_00ff_u64,
    };
    let mixed = id.0.wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(17) ^ kind_id;
    mixed as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::metadata::EntityMetadata;

    #[test]
    fn cow_runtime_uses_default_water_malus_and_metadata_speed() {
        let metadata = EntityMetadata::for_kind(EntityKind::Cow).unwrap();
        let mob = MobRuntimeState::from_spawn(EntityId(1), metadata, true, 135.0);

        assert_eq!(mob.no_action_time(), 0);
        assert!(mob.on_ground());
        assert_eq!(mob.y_body_rot_degrees(), 135.0);
        assert_eq!(mob.y_head_rot_degrees(), 135.0);
        assert_eq!(mob.movement_speed(), 0.2);
        assert_eq!(mob.pathfinding_malus(BlockPathType::Water), 8.0);
    }

    #[test]
    fn chicken_runtime_applies_java_water_malus_override() {
        let metadata = EntityMetadata::for_kind(EntityKind::Chicken).unwrap();
        let mob = MobRuntimeState::from_spawn(EntityId(1), metadata, true, 0.0);

        assert_eq!(mob.movement_speed(), 0.25);
        assert_eq!(mob.pathfinding_malus(BlockPathType::Water), 0.0);
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
}
