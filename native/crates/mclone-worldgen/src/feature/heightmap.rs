use crate::block::{RawBlockId, has_fluid, is_air_like, is_leaves, material_blocks_motion};
use crate::levelgen::MutableChunkBlockBuffer;
use crate::placement::{BlockPos, HeightmapType};

use super::FeatureWorld;

pub(crate) fn project_to_surface<W: FeatureWorld>(
    world: &mut W,
    pos: BlockPos,
) -> Option<BlockPos> {
    Some(BlockPos::new(
        pos.x,
        world.world_surface_height_at(pos.x, pos.z)?,
        pos.z,
    ))
}

pub(crate) fn heightmap_height(
    chunk: &MutableChunkBlockBuffer,
    heightmap: HeightmapType,
    local_x: i32,
    local_z: i32,
) -> i32 {
    if let Some(height) = chunk.cached_worldgen_height(heightmap, local_x, local_z) {
        return height;
    }

    for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
        if heightmap_is_opaque(heightmap, chunk.get_block_at_y(local_x, y, local_z)) {
            return y + 1;
        }
    }
    chunk.min_y
}

fn heightmap_is_opaque(heightmap: HeightmapType, block_id: RawBlockId) -> bool {
    match heightmap {
        HeightmapType::WorldSurfaceWg | HeightmapType::WorldSurface => !is_air_like(block_id),
        HeightmapType::OceanFloorWg | HeightmapType::OceanFloor => material_blocks_motion(block_id),
        HeightmapType::MotionBlocking => material_blocks_motion(block_id) || has_fluid(block_id),
        HeightmapType::MotionBlockingNoLeaves => {
            (material_blocks_motion(block_id) || has_fluid(block_id)) && !is_leaves(block_id)
        }
    }
}
