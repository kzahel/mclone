#![allow(dead_code)]

use std::collections::BTreeMap;

use mclone_protocol::{EntityId, EntityKind};
use mclone_worldgen::prng::SimpleRandomSource;

use super::metadata::EntityMetadata;
use super::state::ServerEntityState;

pub(crate) mod goals;

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

#[derive(Clone, Debug)]
pub(crate) struct MobRuntimeState {
    no_action_time: u32,
    on_ground: bool,
    y_body_rot_degrees: f32,
    y_head_rot_degrees: f32,
    movement_speed: f64,
    pathfinding_malus: PathfindingMalusTable,
    random: SimpleRandomSource,
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

        Self {
            no_action_time: 0,
            on_ground,
            y_body_rot_degrees: y_rot_degrees,
            y_head_rot_degrees: y_rot_degrees,
            movement_speed: metadata.movement_speed,
            pathfinding_malus,
            random: SimpleRandomSource::new(mob_random_seed(id, metadata.kind)),
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

    pub(crate) fn pathfinding_malus(&self, path_type: BlockPathType) -> f32 {
        self.pathfinding_malus.get(path_type)
    }

    pub(crate) fn next_random_int_bound(&mut self, bound: i32) -> i32 {
        self.random.next_int_bound(bound)
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
