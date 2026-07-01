use mclone_core::{Aabb, BlockHitResult, BlockPos, BlockStateId, Vec3d};

use crate::block_clip::clip_aabb;
use crate::block_facts::{is_fluid, terrain_id};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShapeUse {
    Outline,
    Collision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OffsetKind {
    None,
    Xz,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LocalShape {
    bounds: Aabb,
    offset: OffsetKind,
}

pub fn block_collision_aabb(state: BlockStateId, pos: BlockPos) -> Option<Aabb> {
    shape_for(state, ShapeUse::Collision).map(|shape| shape.world_aabb(pos))
}

pub fn block_outline_aabbs(state: BlockStateId, pos: BlockPos) -> Vec<Aabb> {
    shape_for(state, ShapeUse::Outline)
        .map(|shape| vec![shape.world_aabb(pos)])
        .unwrap_or_default()
}

pub fn clip_block_outline(
    state: BlockStateId,
    from: Vec3d,
    to: Vec3d,
    pos: BlockPos,
) -> Option<BlockHitResult> {
    let shape = shape_for(state, ShapeUse::Outline)?;
    clip_aabb(from, to, pos, shape.world_aabb(pos))
}

fn shape_for(state: BlockStateId, use_case: ShapeUse) -> Option<LocalShape> {
    match use_case {
        ShapeUse::Outline => outline_shape(state),
        ShapeUse::Collision => collision_shape(state),
    }
}

fn outline_shape(state: BlockStateId) -> Option<LocalShape> {
    match state.0 {
        terrain_id::AIR | terrain_id::CAVE_AIR => None,
        id if is_fluid(BlockStateId(id)) => None,
        terrain_id::SNOW => Some(local_box(0.0, 0.0, 0.0, 1.0, 2.0 / 16.0, 1.0)),
        terrain_id::GRASS | terrain_id::FERN | terrain_id::DEAD_BUSH => Some(local_box(
            2.0 / 16.0,
            0.0,
            2.0 / 16.0,
            14.0 / 16.0,
            13.0 / 16.0,
            14.0 / 16.0,
        )),
        terrain_id::DANDELION | terrain_id::POPPY => Some(
            local_box(
                5.0 / 16.0,
                0.0,
                5.0 / 16.0,
                11.0 / 16.0,
                10.0 / 16.0,
                11.0 / 16.0,
            )
            .with_offset(OffsetKind::Xz),
        ),
        terrain_id::POINTED_DRIPSTONE => Some(pointed_dripstone_shape()),
        terrain_id::GLOW_LICHEN => None,
        _ => Some(full_block()),
    }
}

fn collision_shape(state: BlockStateId) -> Option<LocalShape> {
    match state.0 {
        terrain_id::AIR
        | terrain_id::CAVE_AIR
        | terrain_id::SNOW
        | terrain_id::GRASS
        | terrain_id::FERN
        | terrain_id::DANDELION
        | terrain_id::POPPY
        | terrain_id::DEAD_BUSH
        | terrain_id::LARGE_FERN_LOWER
        | terrain_id::LARGE_FERN_UPPER
        | terrain_id::GLOW_LICHEN => None,
        terrain_id::POINTED_DRIPSTONE => Some(pointed_dripstone_shape()),
        id if is_fluid(BlockStateId(id)) => None,
        _ => Some(full_block()),
    }
}

fn local_box(min_x: f64, min_y: f64, min_z: f64, max_x: f64, max_y: f64, max_z: f64) -> LocalShape {
    LocalShape {
        bounds: Aabb::new(min_x, min_y, min_z, max_x, max_y, max_z),
        offset: OffsetKind::None,
    }
}

fn full_block() -> LocalShape {
    local_box(0.0, 0.0, 0.0, 1.0, 1.0, 1.0)
}

fn pointed_dripstone_shape() -> LocalShape {
    local_box(5.0 / 16.0, 0.0, 5.0 / 16.0, 11.0 / 16.0, 1.0, 11.0 / 16.0)
}

impl LocalShape {
    fn with_offset(self, offset: OffsetKind) -> Self {
        Self { offset, ..self }
    }

    fn world_aabb(self, pos: BlockPos) -> Aabb {
        let offset = match self.offset {
            OffsetKind::None => Vec3d::ZERO,
            OffsetKind::Xz => java_block_offset_xz(pos),
        };
        self.bounds.move_by(Vec3d::new(
            pos.x as f64 + offset.x,
            pos.y as f64 + offset.y,
            pos.z as f64 + offset.z,
        ))
    }
}

fn java_block_offset_xz(pos: BlockPos) -> Vec3d {
    let seed = java_mth_seed(pos.x, 0, pos.z);
    let max_horizontal_offset = 0.25;
    let x = ((((seed & 15) as f64) / 15.0 - 0.5) * 0.5)
        .clamp(-max_horizontal_offset, max_horizontal_offset);
    let z = ((((seed >> 8 & 15) as f64) / 15.0 - 0.5) * 0.5)
        .clamp(-max_horizontal_offset, max_horizontal_offset);
    Vec3d::new(x, 0.0, z)
}

fn java_mth_seed(x: i32, y: i32, z: i32) -> i64 {
    let x_term = x.wrapping_mul(3_129_871) as i64;
    let z_term = (z as i64).wrapping_mul(116_129_781);
    let mut seed = x_term ^ z_term ^ y as i64;
    seed = seed
        .wrapping_mul(seed)
        .wrapping_mul(42_317_861)
        .wrapping_add(seed.wrapping_mul(11));
    seed >> 16
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::Direction;

    fn state(id: u32) -> BlockStateId {
        BlockStateId(id)
    }

    #[test]
    fn snow_layer_one_uses_java_outline_and_collision_shapes() {
        let pos = BlockPos::new(4, 2, 1);

        assert_eq!(
            shape_for(state(terrain_id::SNOW), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(4.0, 2.0, 1.0, 5.0, 2.125, 2.0))
        );
        assert_eq!(block_collision_aabb(state(terrain_id::SNOW), pos), None);
    }

    #[test]
    fn java_no_collision_blocks_have_empty_collision_shapes() {
        for id in [
            terrain_id::AIR,
            terrain_id::CAVE_AIR,
            terrain_id::WATER,
            terrain_id::WATER_LEVEL_8,
            terrain_id::LAVA,
            terrain_id::LAVA_LEVEL_8,
            terrain_id::SNOW,
            terrain_id::GRASS,
            terrain_id::FERN,
            terrain_id::DANDELION,
            terrain_id::POPPY,
            terrain_id::DEAD_BUSH,
            terrain_id::LARGE_FERN_LOWER,
            terrain_id::LARGE_FERN_UPPER,
            terrain_id::GLOW_LICHEN,
        ] {
            assert_eq!(
                block_collision_aabb(state(id), BlockPos::ZERO),
                None,
                "state {id}"
            );
        }
    }

    #[test]
    fn terrain_solids_use_full_cube_collision_shapes() {
        for id in [1, 3, 7, 40, 42, 49, terrain_id::DRIPSTONE_BLOCK] {
            assert_eq!(
                block_collision_aabb(state(id), BlockPos::new(1, 2, 3)),
                Some(Aabb::new(1.0, 2.0, 3.0, 2.0, 3.0, 4.0)),
                "state {id}"
            );
        }
    }

    #[test]
    fn plant_outline_boxes_match_java_block_classes() {
        let pos = BlockPos::new(0, 0, 0);

        assert_eq!(
            shape_for(state(terrain_id::GRASS), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.125, 0.0, 0.125, 0.875, 0.8125, 0.875))
        );
        assert_eq!(
            shape_for(state(terrain_id::DEAD_BUSH), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.125, 0.0, 0.125, 0.875, 0.8125, 0.875))
        );
        assert_eq!(
            shape_for(state(terrain_id::LARGE_FERN_LOWER), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0))
        );
    }

    #[test]
    fn flower_outline_uses_java_position_offset() {
        let pos = BlockPos::new(4, 2, 1);
        let offset = java_block_offset_xz(pos);

        assert_eq!(
            shape_for(state(terrain_id::DANDELION), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(
                pos.x as f64 + offset.x + 5.0 / 16.0,
                pos.y as f64,
                pos.z as f64 + offset.z + 5.0 / 16.0,
                pos.x as f64 + offset.x + 11.0 / 16.0,
                pos.y as f64 + 10.0 / 16.0,
                pos.z as f64 + offset.z + 11.0 / 16.0,
            ))
        );
    }

    #[test]
    fn pointed_dripstone_uses_partial_center_shape() {
        let pos = BlockPos::new(1, 2, 3);
        let expected = Aabb::new(1.3125, 2.0, 3.3125, 1.6875, 3.0, 3.6875);

        assert_eq!(
            shape_for(state(terrain_id::POINTED_DRIPSTONE), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(expected)
        );
        assert_eq!(
            block_collision_aabb(state(terrain_id::POINTED_DRIPSTONE), pos),
            Some(expected)
        );
    }

    #[test]
    fn clipping_outline_reports_face_and_inside_hits() {
        let pos = BlockPos::new(0, 0, 0);

        let hit = clip_block_outline(
            state(1),
            Vec3d::new(-1.0, 0.5, 0.5),
            Vec3d::new(2.0, 0.5, 0.5),
            pos,
        )
        .expect("ray should hit full block outline");
        assert!(!hit.miss);
        assert_eq!(hit.direction, Direction::West);
        assert_eq!(hit.block_pos, pos);

        let inside = clip_block_outline(
            state(1),
            Vec3d::new(0.5, 0.5, 0.5),
            Vec3d::new(2.0, 0.5, 0.5),
            pos,
        )
        .expect("inside ray should produce a hit");
        assert!(inside.inside);
    }
}
