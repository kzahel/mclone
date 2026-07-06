use super::*;
use crate::biome::{BiomeDefinition, get_layered_biome_by_id};
use crate::block::{
    ACACIA_LEAVES, ACACIA_LOG, AIR, ANDESITE, BAMBOO, BAMBOO_FINAL_LARGE, BAMBOO_TOP_LARGE,
    BAMBOO_TOP_SMALL, BIRCH_LEAVES, BIRCH_LOG, BLUE_ICE, BLUE_ORCHID, BRAIN_CORAL_BLOCK,
    BROWN_MUSHROOM, BROWN_MUSHROOM_BLOCK, BUBBLE_CORAL_BLOCK, CACTUS, CAVE_AIR, CLAY, COAL_ORE,
    COCOA_AGE0_EAST, COCOA_AGE0_NORTH, COCOA_AGE0_SOUTH, COCOA_AGE0_WEST, COCOA_AGE1_EAST,
    COCOA_AGE1_NORTH, COCOA_AGE1_SOUTH, COCOA_AGE1_WEST, COCOA_AGE2_EAST, COCOA_AGE2_NORTH,
    COCOA_AGE2_SOUTH, COCOA_AGE2_WEST, COPPER_ORE, DANDELION, DARK_OAK_LEAVES, DARK_OAK_LOG,
    DEAD_BUSH, DEEPSLATE, DEEPSLATE_COAL_ORE, DEEPSLATE_COPPER_ORE, DEEPSLATE_DIAMOND_ORE,
    DEEPSLATE_GOLD_ORE, DEEPSLATE_IRON_ORE, DEEPSLATE_LAPIS_ORE, DEEPSLATE_REDSTONE_ORE,
    DIAMOND_ORE, DIORITE, DIRT, FIRE_CORAL_BLOCK, GLOW_LICHEN, GOLD_ORE, GRANITE, GRASS,
    GRASS_BLOCK, GRAVEL, HORN_CORAL_BLOCK, ICE, IRON_ORE, JUNGLE_LEAVES, JUNGLE_LOG, KELP,
    KELP_PLANT, LAPIS_ORE, LARGE_FERN_LOWER, LARGE_FERN_UPPER, LAVA, LILAC_LOWER,
    LILY_OF_THE_VALLEY, LILY_PAD, MELON, MOSSY_COBBLESTONE, MUSHROOM_STEM, MYCELIUM, OAK_LEAVES,
    OAK_LOG, PACKED_ICE, PEONY_LOWER, PODZOL, POPPY, PUMPKIN, RED_MUSHROOM, RED_MUSHROOM_BLOCK,
    RED_SAND, REDSTONE_ORE, ROSE_BUSH_LOWER, ROSE_BUSH_UPPER, SAND, SEA_PICKLE_1, SEA_PICKLE_2,
    SEA_PICKLE_3, SEA_PICKLE_4, SEAGRASS, SNOW, SPRUCE_LEAVES, STONE, SUGAR_CANE, SUNFLOWER_LOWER,
    SWEET_BERRY_BUSH, TALL_GRASS_LOWER, TALL_GRASS_UPPER, TALL_SEAGRASS_LOWER, TALL_SEAGRASS_UPPER,
    TERRACOTTA, TUBE_CORAL, TUBE_CORAL_BLOCK, TUBE_CORAL_WALL_FAN_NORTH, TUFF, VINE_EAST,
    VINE_NORTH, VINE_SOUTH, VINE_UP, VINE_WEST, WATER,
};
use crate::placement::{
    ConfiguredDecorator, CountConfiguration, DecorationContext, HeightProvider, IntProvider,
    VerticalAnchor,
};
use crate::prng::{RandomSource, WorldgenRandom};
use mclone_core::CHUNK_WIDTH;

fn flat_grass_chunk() -> MutableChunkBlockBuffer {
    flat_grass_chunk_at(0, 0)
}

fn flat_grass_chunk_at(chunk_x: i32, chunk_z: i32) -> MutableChunkBlockBuffer {
    let mut chunk = MutableChunkBlockBuffer::new(chunk_x, chunk_z, 0, 32);
    for x in 0..CHUNK_WIDTH {
        for z in 0..CHUNK_WIDTH {
            chunk.set_block_at_y(x, 0, z, STONE);
            chunk.set_block_at_y(x, 1, z, DIRT);
            chunk.set_block_at_y(x, 2, z, GRASS_BLOCK);
        }
    }
    chunk
}

fn flat_ocean_chunk() -> MutableChunkBlockBuffer {
    let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 32);
    for x in 0..CHUNK_WIDTH {
        for z in 0..CHUNK_WIDTH {
            chunk.set_block_at_y(x, 0, z, STONE);
            chunk.set_block_at_y(x, 1, z, SAND);
            for y in 2..=10 {
                chunk.set_block_at_y(x, y, z, WATER);
            }
        }
    }
    chunk
}

fn solid_stone_chunk() -> MutableChunkBlockBuffer {
    let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 64);
    for x in 0..CHUNK_WIDTH {
        for y in 0..64 {
            for z in 0..CHUNK_WIDTH {
                chunk.set_block_at_y(x, y, z, STONE);
            }
        }
    }
    chunk
}

fn count_blocks(chunk: &MutableChunkBlockBuffer, block_id: RawBlockId) -> usize {
    chunk
        .blocks
        .iter()
        .filter(|current| **current == block_id)
        .count()
}

fn count_any_blocks(chunk: &MutableChunkBlockBuffer, block_ids: &[RawBlockId]) -> usize {
    chunk
        .blocks
        .iter()
        .filter(|current| block_ids.contains(*current))
        .count()
}

fn count_blocks_at_y(chunk: &MutableChunkBlockBuffer, y: i32, block_id: RawBlockId) -> usize {
    let mut count = 0;
    for z in 0..CHUNK_WIDTH {
        for x in 0..CHUNK_WIDTH {
            if chunk.get_block_at_y(x, y, z) == block_id {
                count += 1;
            }
        }
    }
    count
}

fn has_two_by_two_log_square_at(
    chunk: &MutableChunkBlockBuffer,
    base: BlockPos,
    log: RawBlockId,
) -> bool {
    chunk.get_block_at_y(base.x, base.y, base.z) == log
        && chunk.get_block_at_y(base.x + 1, base.y, base.z) == log
        && chunk.get_block_at_y(base.x, base.y, base.z + 1) == log
        && chunk.get_block_at_y(base.x + 1, base.y, base.z + 1) == log
}

fn count_logs_outside_two_by_two_column(
    chunk: &MutableChunkBlockBuffer,
    base: BlockPos,
    log: RawBlockId,
) -> usize {
    let mut count = 0;
    for y in chunk.min_y..chunk.min_y + chunk.height {
        for z in 0..CHUNK_WIDTH {
            for x in 0..CHUNK_WIDTH {
                let in_trunk_column =
                    (base.x..=base.x + 1).contains(&x) && (base.z..=base.z + 1).contains(&z);
                if !in_trunk_column && chunk.get_block_at_y(x, y, z) == log {
                    count += 1;
                }
            }
        }
    }
    count
}

struct BooleanRandom {
    value: bool,
}

impl BooleanRandom {
    const fn new(value: bool) -> Self {
        Self { value }
    }
}

impl RandomSource for BooleanRandom {
    fn set_seed(&mut self, _seed: i64) {}

    fn next_int(&mut self) -> i32 {
        panic!("next_int should not be used by this test random")
    }

    fn next_int_bound(&mut self, _bound: i32) -> i32 {
        panic!("next_int_bound should not be used by this test random")
    }

    fn next_long(&mut self) -> i64 {
        panic!("next_long should not be used by this test random")
    }

    fn next_boolean(&mut self) -> bool {
        self.value
    }

    fn next_float(&mut self) -> f32 {
        panic!("next_float should not be used by this test random")
    }

    fn next_double(&mut self) -> f64 {
        panic!("next_double should not be used by this test random")
    }

    fn next_gaussian(&mut self) -> f64 {
        panic!("next_gaussian should not be used by this test random")
    }
}

struct ScriptedRandom {
    ints: Vec<i32>,
    floats: Vec<f32>,
}

impl ScriptedRandom {
    fn new(ints: Vec<i32>, floats: Vec<f32>) -> Self {
        Self {
            ints: ints.into_iter().rev().collect(),
            floats: floats.into_iter().rev().collect(),
        }
    }
}

impl RandomSource for ScriptedRandom {
    fn set_seed(&mut self, _seed: i64) {}

    fn next_int(&mut self) -> i32 {
        0
    }

    fn next_int_bound(&mut self, bound: i32) -> i32 {
        let value = self.ints.pop().unwrap_or(0);
        assert!(
            (0..bound).contains(&value),
            "scripted random value {value} outside bound {bound}"
        );
        value
    }

    fn next_long(&mut self) -> i64 {
        0
    }

    fn next_boolean(&mut self) -> bool {
        false
    }

    fn next_float(&mut self) -> f32 {
        self.floats.pop().unwrap_or(1.0)
    }

    fn next_double(&mut self) -> f64 {
        0.0
    }

    fn next_gaussian(&mut self) -> f64 {
        0.0
    }
}

#[derive(Default)]
struct OreHeightmapProbeWorld {
    height_queries: Vec<HeightmapType>,
    world_surface_queries: usize,
    writes: usize,
}

impl FeatureWorld for OreHeightmapProbeWorld {
    fn center_chunk_x(&self) -> i32 {
        0
    }

    fn center_chunk_z(&self) -> i32 {
        0
    }

    fn min_y(&self) -> i32 {
        0
    }

    fn height(&self) -> i32 {
        64
    }

    fn non_air_block_count(&self) -> usize {
        0
    }

    fn block_at_world(&mut self, pos: BlockPos) -> Option<RawBlockId> {
        (0..64).contains(&pos.y).then_some(STONE)
    }

    fn height_at(&mut self, heightmap: HeightmapType, _world_x: i32, _world_z: i32) -> Option<i32> {
        self.height_queries.push(heightmap);
        match heightmap {
            HeightmapType::OceanFloorWg => Some(64),
            _ => Some(0),
        }
    }

    fn world_surface_height_at(&mut self, _world_x: i32, _world_z: i32) -> Option<i32> {
        self.world_surface_queries += 1;
        Some(0)
    }

    fn set_block_world(&mut self, pos: BlockPos, _block_id: RawBlockId) -> bool {
        if (0..64).contains(&pos.y) {
            self.writes += 1;
            true
        } else {
            false
        }
    }
}
mod placement;
mod tables_core;
mod tables_defaults;
mod tables_late;
mod tables_support;
mod vegetation;
