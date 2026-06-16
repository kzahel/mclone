use crate::biome::BiomeDefinition;
use crate::block::{ICE, RawBlockId, SNOW, WATER, is_air_like, material_blocks_motion};
use crate::placement::{BlockPos, HeightmapType};
use crate::surface::biome_temperature;
use mclone_core::CHUNK_WIDTH;

use super::{FeatureBiomeResolver, FeatureWorld};

pub(super) fn place_freeze_top_layer<W: FeatureWorld, B: FeatureBiomeResolver>(
    world: &mut W,
    biomes: &B,
    origin: BlockPos,
) -> bool {
    for dx in 0..CHUNK_WIDTH {
        for dz in 0..CHUNK_WIDTH {
            let x = origin.x + dx;
            let z = origin.z + dz;
            let Some(y) = world.height_at(HeightmapType::MotionBlocking, x, z) else {
                continue;
            };
            let surface_pos = BlockPos::new(x, y, z);
            let below = BlockPos::new(x, y - 1, z);
            let biome = biomes.biome_at(x, y, z);

            if should_freeze(world, biome, below) {
                world.set_block_world(below, ICE);
            }
            if should_snow(world, biome, surface_pos) {
                world.set_block_world(surface_pos, SNOW);
            }
        }
    }

    true
}

fn should_freeze<W: FeatureWorld>(world: &mut W, biome: BiomeDefinition, pos: BlockPos) -> bool {
    is_within_build_height(world, pos)
        && biome_temperature(biome, pos.x, pos.y, pos.z) < 0.15
        && world.block_at_world(pos) == Some(WATER)
}

fn should_snow<W: FeatureWorld>(world: &mut W, biome: BiomeDefinition, pos: BlockPos) -> bool {
    if !is_within_build_height(world, pos)
        || biome_temperature(biome, pos.x, pos.y, pos.z) >= 0.15
        || !world.block_at_world(pos).is_some_and(is_air_like)
    {
        return false;
    }

    let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
    world
        .block_at_world(below)
        .is_some_and(snow_layer_can_survive_on)
}

fn is_within_build_height<W: FeatureWorld>(world: &W, pos: BlockPos) -> bool {
    (world.min_y()..world.min_y() + world.height()).contains(&pos.y)
}

fn snow_layer_can_survive_on(block_id: RawBlockId) -> bool {
    material_blocks_motion(block_id)
}
