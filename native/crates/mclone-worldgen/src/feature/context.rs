use crate::biome::{BiomeDefinition, OverworldBiomeSource};
use crate::block::{GLOW_LICHEN, RawBlockId, block_name};
use crate::levelgen::MutableChunkBlockBuffer;
use crate::placement::{BlockPos, HeightmapType};
use mclone_core::{CHUNK_WIDTH, chunk_min_block_coord};

use super::{Direction, heightmap_height};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecorationStep {
    RawGeneration,
    Lakes,
    LocalModifications,
    UndergroundStructures,
    SurfaceStructures,
    Strongholds,
    UndergroundOres,
    UndergroundDecoration,
    VegetalDecoration,
    TopLayerModification,
}

impl DecorationStep {
    pub const COUNT: usize = 10;
    pub const ALL: [Self; Self::COUNT] = [
        Self::RawGeneration,
        Self::Lakes,
        Self::LocalModifications,
        Self::UndergroundStructures,
        Self::SurfaceStructures,
        Self::Strongholds,
        Self::UndergroundOres,
        Self::UndergroundDecoration,
        Self::VegetalDecoration,
        Self::TopLayerModification,
    ];

    pub const fn index(self) -> i32 {
        match self {
            Self::RawGeneration => 0,
            Self::Lakes => 1,
            Self::LocalModifications => 2,
            Self::UndergroundStructures => 3,
            Self::SurfaceStructures => 4,
            Self::Strongholds => 5,
            Self::UndergroundOres => 6,
            Self::UndergroundDecoration => 7,
            Self::VegetalDecoration => 8,
            Self::TopLayerModification => 9,
        }
    }

    pub const fn key(self) -> &'static str {
        match self {
            Self::RawGeneration => "raw_generation",
            Self::Lakes => "lakes",
            Self::LocalModifications => "local_modifications",
            Self::UndergroundStructures => "underground_structures",
            Self::SurfaceStructures => "surface_structures",
            Self::Strongholds => "strongholds",
            Self::UndergroundOres => "underground_ores",
            Self::UndergroundDecoration => "underground_decoration",
            Self::VegetalDecoration => "vegetal_decoration",
            Self::TopLayerModification => "top_layer_modification",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FeatureDecorationTiming {
    pub step_us: [u128; DecorationStep::COUNT],
}

impl FeatureDecorationTiming {
    pub fn total_us(self) -> u128 {
        self.step_us.iter().sum()
    }

    pub fn add_assign(&mut self, other: Self) {
        for (target, value) in self.step_us.iter_mut().zip(other.step_us) {
            *target += value;
        }
    }
}

pub trait FeatureWorld {
    fn center_chunk_x(&self) -> i32;
    fn center_chunk_z(&self) -> i32;
    fn decoration_chunk_x(&self) -> i32 {
        self.center_chunk_x()
    }
    fn decoration_chunk_z(&self) -> i32 {
        self.center_chunk_z()
    }
    fn min_y(&self) -> i32;
    fn height(&self) -> i32;
    fn non_air_block_count(&self) -> usize;
    fn block_at_world(&mut self, pos: BlockPos) -> Option<RawBlockId>;
    fn height_at(&mut self, heightmap: HeightmapType, world_x: i32, world_z: i32) -> Option<i32>;
    fn world_surface_height_at(&mut self, world_x: i32, world_z: i32) -> Option<i32>;
    fn set_block_world(&mut self, pos: BlockPos, block_id: RawBlockId) -> bool;
    fn glow_lichen_faces_world(&mut self, _pos: BlockPos) -> u8 {
        0
    }
    fn set_glow_lichen_faces_world(&mut self, pos: BlockPos, faces: u8) -> bool {
        if faces == 0 {
            return false;
        }
        self.set_block_world(pos, GLOW_LICHEN)
    }
    fn glow_lichen_spread_direction_order(&mut self) -> [Direction; 6] {
        Direction::ALL
    }
    fn set_glow_lichen_spread_direction_order(&mut self, _directions: [Direction; 6]) {}
    fn schedule_liquid_tick_world(&mut self, _pos: BlockPos, _target: RawBlockId, _delay: i32) {}
}

pub(crate) trait FeatureBiomeResolver {
    fn biome_at(&self, block_x: i32, block_y: i32, block_z: i32) -> BiomeDefinition;
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ConstantFeatureBiomeResolver {
    biome: BiomeDefinition,
}

impl ConstantFeatureBiomeResolver {
    pub(crate) const fn new(biome: BiomeDefinition) -> Self {
        Self { biome }
    }
}

impl FeatureBiomeResolver for ConstantFeatureBiomeResolver {
    fn biome_at(&self, _block_x: i32, _block_y: i32, _block_z: i32) -> BiomeDefinition {
        self.biome
    }
}

#[derive(Clone, Copy)]
pub(crate) struct OverworldFeatureBiomeResolver<'a> {
    seed: i64,
    biome_source: &'a OverworldBiomeSource,
}

impl<'a> OverworldFeatureBiomeResolver<'a> {
    pub(crate) const fn new(seed: i64, biome_source: &'a OverworldBiomeSource) -> Self {
        Self { seed, biome_source }
    }
}

impl FeatureBiomeResolver for OverworldFeatureBiomeResolver<'_> {
    fn biome_at(&self, block_x: i32, _block_y: i32, block_z: i32) -> BiomeDefinition {
        self.biome_source
            .get_block_position_biome_definition(self.seed, block_x, block_z)
    }
}

pub(crate) const DEFAULT_FEATURE_BIOME: BiomeDefinition =
    BiomeDefinition::new(1, "minecraft:plains", 0.125, 0.05000000074505806);

impl FeatureWorld for MutableChunkBlockBuffer {
    fn center_chunk_x(&self) -> i32 {
        self.chunk_x
    }

    fn center_chunk_z(&self) -> i32 {
        self.chunk_z
    }

    fn min_y(&self) -> i32 {
        self.min_y
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn non_air_block_count(&self) -> usize {
        self.non_air_block_count()
    }

    fn block_at_world(&mut self, pos: BlockPos) -> Option<RawBlockId> {
        let local_x = pos.x - chunk_min_block_coord(self.chunk_x);
        let local_z = pos.z - chunk_min_block_coord(self.chunk_z);
        if !(0..CHUNK_WIDTH).contains(&local_x)
            || !(self.min_y..self.min_y + self.height).contains(&pos.y)
            || !(0..CHUNK_WIDTH).contains(&local_z)
        {
            return None;
        }
        Some(self.get_block_at_y(local_x, pos.y, local_z))
    }

    fn height_at(&mut self, heightmap: HeightmapType, world_x: i32, world_z: i32) -> Option<i32> {
        let local_x = world_x - chunk_min_block_coord(self.chunk_x);
        let local_z = world_z - chunk_min_block_coord(self.chunk_z);
        if !(0..CHUNK_WIDTH).contains(&local_x) || !(0..CHUNK_WIDTH).contains(&local_z) {
            return None;
        }
        Some(heightmap_height(self, heightmap, local_x, local_z))
    }

    fn world_surface_height_at(&mut self, world_x: i32, world_z: i32) -> Option<i32> {
        let local_x = world_x - chunk_min_block_coord(self.chunk_x);
        let local_z = world_z - chunk_min_block_coord(self.chunk_z);
        if !(0..CHUNK_WIDTH).contains(&local_x) || !(0..CHUNK_WIDTH).contains(&local_z) {
            return None;
        }
        Some(self.world_surface_height(local_x, local_z))
    }

    fn set_block_world(&mut self, pos: BlockPos, block_id: RawBlockId) -> bool {
        let local_x = pos.x - chunk_min_block_coord(self.chunk_x);
        let local_z = pos.z - chunk_min_block_coord(self.chunk_z);
        if !(0..CHUNK_WIDTH).contains(&local_x)
            || !(self.min_y..self.min_y + self.height).contains(&pos.y)
            || !(0..CHUNK_WIDTH).contains(&local_z)
        {
            return false;
        }
        self.set_block_at_y(local_x, pos.y, local_z, block_id);
        true
    }

    fn glow_lichen_faces_world(&mut self, pos: BlockPos) -> u8 {
        let local_x = pos.x - chunk_min_block_coord(self.chunk_x);
        let local_z = pos.z - chunk_min_block_coord(self.chunk_z);
        if !(0..CHUNK_WIDTH).contains(&local_x)
            || !(self.min_y..self.min_y + self.height).contains(&pos.y)
            || !(0..CHUNK_WIDTH).contains(&local_z)
        {
            return 0;
        }
        self.glow_lichen_faces_at_y(local_x, pos.y, local_z)
    }

    fn set_glow_lichen_faces_world(&mut self, pos: BlockPos, faces: u8) -> bool {
        let local_x = pos.x - chunk_min_block_coord(self.chunk_x);
        let local_z = pos.z - chunk_min_block_coord(self.chunk_z);
        if !(0..CHUNK_WIDTH).contains(&local_x)
            || !(self.min_y..self.min_y + self.height).contains(&pos.y)
            || !(0..CHUNK_WIDTH).contains(&local_z)
        {
            return false;
        }
        self.set_glow_lichen_faces_at_y(local_x, pos.y, local_z, faces);
        true
    }

    fn schedule_liquid_tick_world(&mut self, pos: BlockPos, target: RawBlockId, delay: i32) {
        let local_x = pos.x - chunk_min_block_coord(self.chunk_x);
        let local_z = pos.z - chunk_min_block_coord(self.chunk_z);
        if !(0..CHUNK_WIDTH).contains(&local_x)
            || !(self.min_y..self.min_y + self.height).contains(&pos.y)
            || !(0..CHUNK_WIDTH).contains(&local_z)
        {
            return;
        }
        self.schedule_liquid_tick(pos.x, pos.y, pos.z, block_name(target), delay);
    }
}
