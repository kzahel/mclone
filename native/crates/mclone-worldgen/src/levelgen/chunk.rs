use std::collections::BTreeMap;

use crate::block::{
    AIR, GLOW_LICHEN, GeneratedBlockId, RawBlockId, generated_block_state_id, is_air_like,
    material_blocks_motion,
};
use crate::placement::HeightmapType;
use mclone_core::{
    BlockStateId, CHUNK_WIDTH, ChunkPos, ChunkRevision, ChunkSnapshot, ChunkStatus, SECTION_HEIGHT,
    chunk_block_index,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledTick {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub target: String,
    pub delay: i32,
}

impl ScheduledTick {
    pub fn new(x: i32, y: i32, z: i32, target: impl Into<String>, delay: i32) -> Self {
        Self {
            x,
            y,
            z,
            target: target.into(),
            delay,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutableChunkBlockBuffer {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub min_y: i32,
    pub height: i32,
    pub blocks: Vec<u8>,
    worldgen_heightmaps: Option<ChunkWorldgenHeightmaps>,
    glow_lichen_faces: BTreeMap<usize, u8>,
    block_ticks: Vec<ScheduledTick>,
    liquid_ticks: Vec<ScheduledTick>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ChunkWorldgenHeightmaps {
    world_surface_wg: [i32; 256],
    ocean_floor_wg: [i32; 256],
}

impl ChunkWorldgenHeightmaps {
    fn from_chunk(chunk: &MutableChunkBlockBuffer) -> Self {
        let mut world_surface_wg = [chunk.min_y; 256];
        let mut ocean_floor_wg = [chunk.min_y; 256];

        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                let index = heightmap_column_index(local_x, local_z);
                world_surface_wg[index] =
                    scan_heightmap_height(chunk, local_x, local_z, |block_id| {
                        !is_air_like(block_id)
                    });
                ocean_floor_wg[index] =
                    scan_heightmap_height(chunk, local_x, local_z, material_blocks_motion);
            }
        }

        Self {
            world_surface_wg,
            ocean_floor_wg,
        }
    }

    fn height(&self, heightmap: HeightmapType, local_x: i32, local_z: i32) -> Option<i32> {
        let index = heightmap_column_index(local_x, local_z);
        match heightmap {
            HeightmapType::WorldSurfaceWg => Some(self.world_surface_wg[index]),
            HeightmapType::OceanFloorWg => Some(self.ocean_floor_wg[index]),
            _ => None,
        }
    }
}

impl MutableChunkBlockBuffer {
    pub fn new(chunk_x: i32, chunk_z: i32, min_y: i32, height: i32) -> Self {
        if height <= 0 || height % SECTION_HEIGHT != 0 {
            panic!("chunk height {height} must be a positive multiple of {SECTION_HEIGHT}");
        }

        Self {
            chunk_x,
            chunk_z,
            min_y,
            height,
            blocks: vec![AIR; height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize],
            worldgen_heightmaps: None,
            glow_lichen_faces: BTreeMap::new(),
            block_ticks: Vec::new(),
            liquid_ticks: Vec::new(),
        }
    }

    pub fn get_block(&self, local_x: i32, local_y: i32, local_z: i32) -> u8 {
        self.blocks[chunk_block_index(local_x, local_y, local_z)]
    }

    pub fn set_block(&mut self, local_x: i32, local_y: i32, local_z: i32, block_id: u8) {
        let index = chunk_block_index(local_x, local_y, local_z);
        self.blocks[index] = block_id;
        if block_id != GLOW_LICHEN {
            self.glow_lichen_faces.remove(&index);
        }
    }

    pub fn get_block_at_y(&self, local_x: i32, y: i32, local_z: i32) -> u8 {
        self.get_block(local_x, y - self.min_y, local_z)
    }

    pub fn set_block_at_y(&mut self, local_x: i32, y: i32, local_z: i32, block_id: u8) {
        self.set_block(local_x, y - self.min_y, local_z, block_id);
    }

    pub fn glow_lichen_faces_at_y(&self, local_x: i32, y: i32, local_z: i32) -> u8 {
        if self.get_block_at_y(local_x, y, local_z) != GLOW_LICHEN {
            return 0;
        }
        let index = chunk_block_index(local_x, y - self.min_y, local_z);
        *self.glow_lichen_faces.get(&index).unwrap_or(&0)
    }

    pub fn set_glow_lichen_faces_at_y(&mut self, local_x: i32, y: i32, local_z: i32, faces: u8) {
        self.set_block_at_y(local_x, y, local_z, GLOW_LICHEN);
        let index = chunk_block_index(local_x, y - self.min_y, local_z);
        if faces == 0 {
            self.glow_lichen_faces.remove(&index);
        } else {
            self.glow_lichen_faces.insert(index, faces);
        }
    }

    pub fn world_surface_height(&self, local_x: i32, local_z: i32) -> i32 {
        world_surface_height(self, local_x, local_z)
    }

    pub fn prime_worldgen_heightmaps(&mut self) {
        self.worldgen_heightmaps = Some(ChunkWorldgenHeightmaps::from_chunk(self));
    }

    pub fn cached_worldgen_height(
        &self,
        heightmap: HeightmapType,
        local_x: i32,
        local_z: i32,
    ) -> Option<i32> {
        self.worldgen_heightmaps
            .as_ref()
            .and_then(|heightmaps| heightmaps.height(heightmap, local_x, local_z))
    }

    pub fn non_air_block_count(&self) -> usize {
        self.blocks
            .iter()
            .filter(|block_id| !is_air_like(**block_id))
            .count()
    }

    pub fn schedule_block_tick(
        &mut self,
        world_x: i32,
        y: i32,
        world_z: i32,
        target: impl Into<String>,
        delay: i32,
    ) {
        self.block_ticks
            .push(ScheduledTick::new(world_x, y, world_z, target, delay));
    }

    pub fn schedule_liquid_tick(
        &mut self,
        world_x: i32,
        y: i32,
        world_z: i32,
        target: impl Into<String>,
        delay: i32,
    ) {
        self.liquid_ticks
            .push(ScheduledTick::new(world_x, y, world_z, target, delay));
    }

    pub fn block_ticks(&self) -> &[ScheduledTick] {
        &self.block_ticks
    }

    pub fn liquid_ticks(&self) -> &[ScheduledTick] {
        &self.liquid_ticks
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedChunk {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub min_y: i32,
    pub height: i32,
    blocks: Vec<RawBlockId>,
    block_ticks: Vec<ScheduledTick>,
    liquid_ticks: Vec<ScheduledTick>,
}

impl GeneratedChunk {
    pub const WIDTH: i32 = CHUNK_WIDTH;

    pub fn from_raw_parts(
        chunk_x: i32,
        chunk_z: i32,
        min_y: i32,
        height: i32,
        blocks: Vec<RawBlockId>,
    ) -> Self {
        Self::from_raw_parts_with_ticks(
            chunk_x,
            chunk_z,
            min_y,
            height,
            blocks,
            Vec::new(),
            Vec::new(),
        )
    }

    fn from_raw_parts_with_ticks(
        chunk_x: i32,
        chunk_z: i32,
        min_y: i32,
        height: i32,
        blocks: Vec<RawBlockId>,
        block_ticks: Vec<ScheduledTick>,
        liquid_ticks: Vec<ScheduledTick>,
    ) -> Self {
        validate_generated_chunk_shape(height, blocks.len());
        Self {
            chunk_x,
            chunk_z,
            min_y,
            height,
            blocks,
            block_ticks,
            liquid_ticks,
        }
    }

    pub fn from_mutable_buffer(buffer: MutableChunkBlockBuffer) -> Self {
        Self::from_raw_parts_with_ticks(
            buffer.chunk_x,
            buffer.chunk_z,
            buffer.min_y,
            buffer.height,
            buffer.blocks,
            buffer.block_ticks,
            buffer.liquid_ticks,
        )
    }

    pub fn blocks(&self) -> &[RawBlockId] {
        &self.blocks
    }

    pub fn block_ticks(&self) -> &[ScheduledTick] {
        &self.block_ticks
    }

    pub fn liquid_ticks(&self) -> &[ScheduledTick] {
        &self.liquid_ticks
    }

    pub fn block_at_local(&self, local_x: i32, local_y: i32, local_z: i32) -> GeneratedBlockId {
        self.assert_local_position(local_x, local_y, local_z);
        GeneratedBlockId(self.blocks[chunk_block_index(local_x, local_y, local_z)])
    }

    pub fn block_at_y(&self, local_x: i32, y: i32, local_z: i32) -> GeneratedBlockId {
        self.block_at_local(local_x, y - self.min_y, local_z)
    }

    pub fn non_air_block_count(&self) -> usize {
        self.blocks
            .iter()
            .filter(|block_id| !is_air_like(**block_id))
            .count()
    }

    pub fn block_count(&self, block_id: RawBlockId) -> usize {
        self.blocks
            .iter()
            .filter(|current| **current == block_id)
            .count()
    }

    pub fn to_chunk_snapshot(&self, revision: ChunkRevision, status: ChunkStatus) -> ChunkSnapshot {
        let block_state_ids = self
            .blocks
            .iter()
            .map(|block_id| generated_block_state_id(*block_id))
            .collect::<Vec<BlockStateId>>();
        ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(self.chunk_x, self.chunk_z),
            status,
            revision,
            self.min_y,
            self.height,
            &block_state_ids,
        )
    }

    fn assert_local_position(&self, local_x: i32, local_y: i32, local_z: i32) {
        if !(0..CHUNK_WIDTH).contains(&local_x)
            || !(0..self.height).contains(&local_y)
            || !(0..CHUNK_WIDTH).contains(&local_z)
        {
            panic!(
                "local block position ({local_x}, {local_y}, {local_z}) is outside generated chunk {}..{}",
                self.min_y,
                self.min_y + self.height
            );
        }
    }
}

fn validate_generated_chunk_shape(height: i32, block_count: usize) {
    if height <= 0 || height % SECTION_HEIGHT != 0 {
        panic!("chunk height {height} must be a positive multiple of {SECTION_HEIGHT}");
    }
    let expected_len = height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
    if block_count != expected_len {
        panic!("generated chunk block buffer has {block_count} entries; expected {expected_len}");
    }
}

fn heightmap_column_index(local_x: i32, local_z: i32) -> usize {
    (local_x + local_z * CHUNK_WIDTH) as usize
}

fn scan_heightmap_height(
    chunk: &MutableChunkBlockBuffer,
    local_x: i32,
    local_z: i32,
    is_opaque: impl Fn(u8) -> bool,
) -> i32 {
    for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
        if is_opaque(chunk.get_block_at_y(local_x, y, local_z)) {
            return y + 1;
        }
    }
    chunk.min_y
}

pub(super) fn world_surface_height(
    chunk: &MutableChunkBlockBuffer,
    local_x: i32,
    local_z: i32,
) -> i32 {
    for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
        if !is_air_like(chunk.get_block_at_y(local_x, y, local_z)) {
            return y + 1;
        }
    }
    chunk.min_y
}
