use mclone_blocks::{
    block_collision_aabb, block_collision_aabbs, collision_aabb_for_feet_position,
};
use mclone_core::{Aabb, BlockPos, Vec3d};
use mclone_protocol::EntityKind;
use mclone_worldgen::block::{GRASS_BLOCK, RawBlockId, generated_block_state_id, has_fluid};

use crate::entity::metadata::EntityMetadata;

use super::biome_tables::VanillaSpawnEntity;

pub(crate) const FARM_ANIMAL_PLACEMENTS: [SpawnPlacement; 4] = [
    SpawnPlacement::animal(VanillaSpawnEntity::Sheep),
    SpawnPlacement::animal(VanillaSpawnEntity::Pig),
    SpawnPlacement::animal(VanillaSpawnEntity::Chicken),
    SpawnPlacement::animal(VanillaSpawnEntity::Cow),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpawnPlacementType {
    OnGround,
    InWater,
    NoRestrictions,
    InLava,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpawnHeightmapType {
    MotionBlocking,
    MotionBlockingNoLeaves,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpawnPredicateKind {
    Animal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SpawnPlacement {
    pub(crate) entity: VanillaSpawnEntity,
    pub(crate) placement_type: SpawnPlacementType,
    pub(crate) heightmap: SpawnHeightmapType,
    pub(crate) predicate: SpawnPredicateKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpawnPlacementFailure {
    UnsupportedEntity,
    UnsupportedPlacement,
    MissingBlockData,
    InvalidFloor,
    BlockedFeet,
    BlockedHead,
    NotGrassBlock,
    MissingBrightness,
    TooDark,
    CollisionBlocked,
}

impl SpawnPlacement {
    const fn animal(entity: VanillaSpawnEntity) -> Self {
        Self {
            entity,
            placement_type: SpawnPlacementType::OnGround,
            heightmap: SpawnHeightmapType::MotionBlockingNoLeaves,
            predicate: SpawnPredicateKind::Animal,
        }
    }
}

pub(crate) fn farm_animal_placement_for_kind(kind: EntityKind) -> Option<SpawnPlacement> {
    FARM_ANIMAL_PLACEMENTS
        .iter()
        .copied()
        .find(|placement| placement.entity.implemented_kind() == Some(kind))
}

pub(crate) fn check_farm_animal_natural_spawn<F, B>(
    kind: EntityKind,
    pos: BlockPos,
    block_at: F,
    raw_brightness_at: B,
) -> Result<(), SpawnPlacementFailure>
where
    F: FnMut(BlockPos) -> Option<RawBlockId>,
    B: FnMut(BlockPos) -> Option<u8>,
{
    let placement =
        farm_animal_placement_for_kind(kind).ok_or(SpawnPlacementFailure::UnsupportedEntity)?;
    if placement.placement_type != SpawnPlacementType::OnGround
        || placement.predicate != SpawnPredicateKind::Animal
    {
        return Err(SpawnPlacementFailure::UnsupportedPlacement);
    }

    check_land_creature_natural_spawn(kind, pos, block_at, raw_brightness_at)
}

pub(crate) fn check_land_creature_natural_spawn<F, B>(
    kind: EntityKind,
    pos: BlockPos,
    mut block_at: F,
    mut raw_brightness_at: B,
) -> Result<(), SpawnPlacementFailure>
where
    F: FnMut(BlockPos) -> Option<RawBlockId>,
    B: FnMut(BlockPos) -> Option<u8>,
{
    if !matches!(
        kind,
        EntityKind::Cow | EntityKind::Chicken | EntityKind::Mallard | EntityKind::Deer
    ) {
        return Err(SpawnPlacementFailure::UnsupportedEntity);
    }

    let floor_pos = pos.below();
    let floor = block_at(floor_pos).ok_or(SpawnPlacementFailure::MissingBlockData)?;
    if !is_valid_spawn_floor(floor, floor_pos) {
        return Err(SpawnPlacementFailure::InvalidFloor);
    }

    let feet = block_at(pos).ok_or(SpawnPlacementFailure::MissingBlockData)?;
    if !is_valid_empty_spawn_block(feet, pos) {
        return Err(SpawnPlacementFailure::BlockedFeet);
    }

    let head_pos = pos.offset(0, 1, 0);
    let head = block_at(head_pos).ok_or(SpawnPlacementFailure::MissingBlockData)?;
    if !is_valid_empty_spawn_block(head, head_pos) {
        return Err(SpawnPlacementFailure::BlockedHead);
    }

    if floor != GRASS_BLOCK {
        return Err(SpawnPlacementFailure::NotGrassBlock);
    }

    let brightness = raw_brightness_at(pos).ok_or(SpawnPlacementFailure::MissingBrightness)?;
    if brightness <= 8 {
        return Err(SpawnPlacementFailure::TooDark);
    }

    if entity_aabb_collides(kind, pos, &mut block_at)? {
        return Err(SpawnPlacementFailure::CollisionBlocked);
    }

    Ok(())
}

pub(crate) fn check_debug_actor_placement<F>(
    kind: EntityKind,
    pos: BlockPos,
    mut block_at: F,
) -> Result<(), SpawnPlacementFailure>
where
    F: FnMut(BlockPos) -> Option<RawBlockId>,
{
    let floor_pos = pos.below();
    let floor = block_at(floor_pos).ok_or(SpawnPlacementFailure::MissingBlockData)?;
    if !is_valid_spawn_floor(floor, floor_pos) {
        return Err(SpawnPlacementFailure::InvalidFloor);
    }
    let feet = block_at(pos).ok_or(SpawnPlacementFailure::MissingBlockData)?;
    if !is_valid_empty_spawn_block(feet, pos) {
        return Err(SpawnPlacementFailure::BlockedFeet);
    }
    let head_pos = pos.offset(0, 1, 0);
    let head = block_at(head_pos).ok_or(SpawnPlacementFailure::MissingBlockData)?;
    if !is_valid_empty_spawn_block(head, head_pos) {
        return Err(SpawnPlacementFailure::BlockedHead);
    }
    if entity_aabb_collides(kind, pos, &mut block_at)? {
        return Err(SpawnPlacementFailure::CollisionBlocked);
    }
    Ok(())
}

fn is_valid_spawn_floor(block: RawBlockId, pos: BlockPos) -> bool {
    block_collision_aabb(generated_block_state_id(block), pos).is_some()
}

fn is_valid_empty_spawn_block(block: RawBlockId, pos: BlockPos) -> bool {
    !has_fluid(block) && !is_collision_shape_full_block(block, pos)
}

fn is_collision_shape_full_block(block: RawBlockId, pos: BlockPos) -> bool {
    block_collision_aabb(generated_block_state_id(block), pos) == Some(Aabb::unit_block(pos))
}

fn entity_aabb_collides(
    kind: EntityKind,
    pos: BlockPos,
    block_at: &mut impl FnMut(BlockPos) -> Option<RawBlockId>,
) -> Result<bool, SpawnPlacementFailure> {
    let metadata =
        EntityMetadata::for_kind(kind).ok_or(SpawnPlacementFailure::UnsupportedEntity)?;
    let entity_box = collision_aabb_for_feet_position(
        Vec3d::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5),
        f64::from(metadata.dimensions.width),
        f64::from(metadata.dimensions.height),
    );

    let min_x = entity_box.min_x.floor() as i32;
    let min_y = entity_box.min_y.floor() as i32;
    let min_z = entity_box.min_z.floor() as i32;
    let max_x = entity_box.max_x.floor() as i32;
    let max_y = entity_box.max_y.floor() as i32;
    let max_z = entity_box.max_z.floor() as i32;

    for y in min_y..=max_y {
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                let block_pos = BlockPos::new(x, y, z);
                let block = block_at(block_pos).ok_or(SpawnPlacementFailure::MissingBlockData)?;
                for block_box in block_collision_aabbs(generated_block_state_id(block), block_pos) {
                    if block_box.intersects(entity_box) {
                        return Ok(true);
                    }
                }
            }
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mclone_worldgen::block::{AIR, DIRT, POINTED_DRIPSTONE, STONE, WATER};

    use super::*;

    fn block_map_at(blocks: &BTreeMap<BlockPos, RawBlockId>, pos: BlockPos) -> Option<RawBlockId> {
        Some(*blocks.get(&pos).unwrap_or(&AIR))
    }

    fn valid_animal_spawn_blocks() -> BTreeMap<BlockPos, RawBlockId> {
        BTreeMap::from([(BlockPos::new(0, 63, 0), GRASS_BLOCK)])
    }

    #[test]
    fn cow_and_chicken_use_java_on_ground_animal_placement() {
        for kind in [EntityKind::Cow, EntityKind::Chicken] {
            let placement = farm_animal_placement_for_kind(kind).expect("placement");

            assert_eq!(placement.placement_type, SpawnPlacementType::OnGround);
            assert_eq!(
                placement.heightmap,
                SpawnHeightmapType::MotionBlockingNoLeaves
            );
            assert_eq!(placement.predicate, SpawnPredicateKind::Animal);
        }
    }

    #[test]
    fn debug_actor_placement_accepts_supported_clear_floor_without_natural_rules() {
        let blocks = BTreeMap::from([(BlockPos::new(0, 63, 0), DIRT)]);

        for kind in [EntityKind::Chicken, EntityKind::Mannequin] {
            assert_eq!(
                check_debug_actor_placement(kind, BlockPos::new(0, 64, 0), |pos| {
                    block_map_at(&blocks, pos)
                }),
                Ok(())
            );
        }
    }

    #[test]
    fn original_creatures_do_not_mutate_the_reference_placement_table() {
        assert_eq!(farm_animal_placement_for_kind(EntityKind::Mallard), None);
        assert_eq!(farm_animal_placement_for_kind(EntityKind::Item), None);

        let blocks = valid_animal_spawn_blocks();
        assert_eq!(
            check_land_creature_natural_spawn(
                EntityKind::Mallard,
                BlockPos::new(0, 64, 0),
                |pos| block_map_at(&blocks, pos),
                |_| Some(15),
            ),
            Ok(())
        );
    }

    #[test]
    fn animal_spawn_predicate_accepts_grass_floor_light_and_clear_collision() {
        let blocks = valid_animal_spawn_blocks();

        assert_eq!(
            check_farm_animal_natural_spawn(
                EntityKind::Cow,
                BlockPos::new(0, 64, 0),
                |pos| block_map_at(&blocks, pos),
                |_| Some(15),
            ),
            Ok(())
        );
    }

    #[test]
    fn animal_spawn_predicate_requires_grass_below_after_floor_support() {
        let blocks = BTreeMap::from([(BlockPos::new(0, 63, 0), DIRT)]);

        assert_eq!(
            check_farm_animal_natural_spawn(
                EntityKind::Chicken,
                BlockPos::new(0, 64, 0),
                |pos| block_map_at(&blocks, pos),
                |_| Some(15),
            ),
            Err(SpawnPlacementFailure::NotGrassBlock)
        );
    }

    #[test]
    fn animal_spawn_predicate_rejects_missing_floor_and_blocked_space() {
        assert_eq!(
            check_farm_animal_natural_spawn(
                EntityKind::Cow,
                BlockPos::new(0, 64, 0),
                |_| None,
                |_| Some(15),
            ),
            Err(SpawnPlacementFailure::MissingBlockData)
        );

        let blocks = BTreeMap::from([
            (BlockPos::new(0, 63, 0), GRASS_BLOCK),
            (BlockPos::new(0, 64, 0), STONE),
        ]);
        assert_eq!(
            check_farm_animal_natural_spawn(
                EntityKind::Cow,
                BlockPos::new(0, 64, 0),
                |pos| block_map_at(&blocks, pos),
                |_| Some(15),
            ),
            Err(SpawnPlacementFailure::BlockedFeet)
        );

        let blocks = BTreeMap::from([
            (BlockPos::new(0, 63, 0), GRASS_BLOCK),
            (BlockPos::new(0, 65, 0), STONE),
        ]);
        assert_eq!(
            check_farm_animal_natural_spawn(
                EntityKind::Cow,
                BlockPos::new(0, 64, 0),
                |pos| block_map_at(&blocks, pos),
                |_| Some(15),
            ),
            Err(SpawnPlacementFailure::BlockedHead)
        );
    }

    #[test]
    fn animal_spawn_predicate_keeps_brightness_as_required_input() {
        let blocks = valid_animal_spawn_blocks();

        assert_eq!(
            check_farm_animal_natural_spawn(
                EntityKind::Chicken,
                BlockPos::new(0, 64, 0),
                |pos| block_map_at(&blocks, pos),
                |_| None,
            ),
            Err(SpawnPlacementFailure::MissingBrightness)
        );
        assert_eq!(
            check_farm_animal_natural_spawn(
                EntityKind::Chicken,
                BlockPos::new(0, 64, 0),
                |pos| block_map_at(&blocks, pos),
                |_| Some(8),
            ),
            Err(SpawnPlacementFailure::TooDark)
        );
    }

    #[test]
    fn animal_spawn_predicate_rejects_fluid_and_partial_collision() {
        let blocks = BTreeMap::from([
            (BlockPos::new(0, 63, 0), GRASS_BLOCK),
            (BlockPos::new(0, 64, 0), WATER),
        ]);
        assert_eq!(
            check_farm_animal_natural_spawn(
                EntityKind::Chicken,
                BlockPos::new(0, 64, 0),
                |pos| block_map_at(&blocks, pos),
                |_| Some(15),
            ),
            Err(SpawnPlacementFailure::BlockedFeet)
        );

        let blocks = BTreeMap::from([
            (BlockPos::new(0, 63, 0), GRASS_BLOCK),
            (BlockPos::new(0, 64, 0), POINTED_DRIPSTONE),
        ]);
        assert_eq!(
            check_farm_animal_natural_spawn(
                EntityKind::Cow,
                BlockPos::new(0, 64, 0),
                |pos| block_map_at(&blocks, pos),
                |_| Some(15),
            ),
            Err(SpawnPlacementFailure::CollisionBlocked)
        );
    }
}
