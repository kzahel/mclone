use crate::block::{RawBlockId, block_name};
use crate::levelgen::MutableChunkBlockBuffer;
use crate::placement::{BlockPos, HeightmapType};
use mclone_core::{block_to_chunk_coord, local_block_coord};

use super::{
    Direction, FEATURES_BLOCK_DEPENDENCY_RADIUS, FEATURES_WRITE_RADIUS_CUTOFF, FeatureWorld,
    heightmap_height,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FeatureRegionMetrics {
    pub block_reads: usize,
    pub height_queries: usize,
    pub block_write_attempts: usize,
    pub block_writes: usize,
    pub blocked_block_writes: usize,
}

#[derive(Debug)]
pub struct FeatureRegion {
    center_chunk_x: i32,
    center_chunk_z: i32,
    decoration_chunk_x: i32,
    decoration_chunk_z: i32,
    dependency_radius: i32,
    write_radius_cutoff: i32,
    chunk_min_x: i32,
    chunk_min_z: i32,
    chunk_width: usize,
    chunks: Vec<Option<MutableChunkBlockBuffer>>,
    glow_lichen_spread_directions: [Direction; 6],
    metrics: FeatureRegionMetrics,
}

impl FeatureRegion {
    pub fn new(
        center_chunk_x: i32,
        center_chunk_z: i32,
        chunks: impl IntoIterator<Item = MutableChunkBlockBuffer>,
    ) -> Self {
        Self::with_radii(
            center_chunk_x,
            center_chunk_z,
            FEATURES_BLOCK_DEPENDENCY_RADIUS,
            FEATURES_WRITE_RADIUS_CUTOFF,
            chunks,
        )
    }

    pub fn with_radii(
        center_chunk_x: i32,
        center_chunk_z: i32,
        dependency_radius: i32,
        write_radius_cutoff: i32,
        chunks: impl IntoIterator<Item = MutableChunkBlockBuffer>,
    ) -> Self {
        if dependency_radius < 0 {
            panic!("feature dependency radius must be non-negative");
        }
        if write_radius_cutoff < 0 {
            panic!("feature write radius cutoff must be non-negative");
        }
        if write_radius_cutoff > dependency_radius {
            panic!(
                "feature write radius cutoff {write_radius_cutoff} exceeds dependency radius {dependency_radius}"
            );
        }

        let chunks = chunks.into_iter().collect::<Vec<_>>();
        let chunk_min_x = chunks
            .iter()
            .map(|chunk| chunk.chunk_x)
            .min()
            .unwrap_or(center_chunk_x);
        let chunk_max_x = chunks
            .iter()
            .map(|chunk| chunk.chunk_x)
            .max()
            .unwrap_or(center_chunk_x);
        let chunk_min_z = chunks
            .iter()
            .map(|chunk| chunk.chunk_z)
            .min()
            .unwrap_or(center_chunk_z);
        let chunk_max_z = chunks
            .iter()
            .map(|chunk| chunk.chunk_z)
            .max()
            .unwrap_or(center_chunk_z);
        let chunk_width = usize::try_from(chunk_max_x - chunk_min_x + 1)
            .expect("feature region chunk width must fit usize");
        let chunk_height = usize::try_from(chunk_max_z - chunk_min_z + 1)
            .expect("feature region chunk height must fit usize");
        let mut chunk_slots = Vec::with_capacity(chunk_width * chunk_height);
        chunk_slots.resize_with(chunk_width * chunk_height, || None);
        for chunk in chunks {
            let index = region_chunk_index(
                chunk_min_x,
                chunk_min_z,
                chunk_width,
                chunk.chunk_x,
                chunk.chunk_z,
            )
            .expect("chunk bounds were computed from provided chunks");
            chunk_slots[index] = Some(chunk);
        }

        Self {
            center_chunk_x,
            center_chunk_z,
            decoration_chunk_x: center_chunk_x,
            decoration_chunk_z: center_chunk_z,
            dependency_radius,
            write_radius_cutoff,
            chunk_min_x,
            chunk_min_z,
            chunk_width,
            chunks: chunk_slots,
            // Java mutates MultifaceBlock.DIRECTIONS globally while spawn chunks generate in
            // parallel. This order matches the committed vanilla scheduler fixture at the start
            // of the native 3x3 feature replay.
            glow_lichen_spread_directions: [
                Direction::Down,
                Direction::Up,
                Direction::North,
                Direction::West,
                Direction::South,
                Direction::East,
            ],
            metrics: FeatureRegionMetrics::default(),
        }
    }

    pub fn set_center(&mut self, center_chunk_x: i32, center_chunk_z: i32) {
        self.center_chunk_x = center_chunk_x;
        self.center_chunk_z = center_chunk_z;
        self.decoration_chunk_x = center_chunk_x;
        self.decoration_chunk_z = center_chunk_z;
    }

    pub fn set_center_with_decoration_identity(
        &mut self,
        center_chunk_x: i32,
        center_chunk_z: i32,
        decoration_chunk_x: i32,
        decoration_chunk_z: i32,
    ) {
        self.center_chunk_x = center_chunk_x;
        self.center_chunk_z = center_chunk_z;
        self.decoration_chunk_x = decoration_chunk_x;
        self.decoration_chunk_z = decoration_chunk_z;
    }

    pub fn dependency_radius(&self) -> i32 {
        self.dependency_radius
    }

    pub fn write_radius_cutoff(&self) -> i32 {
        self.write_radius_cutoff
    }

    pub fn metrics(&self) -> FeatureRegionMetrics {
        self.metrics
    }

    pub fn chunk(&self, chunk_x: i32, chunk_z: i32) -> Option<&MutableChunkBlockBuffer> {
        self.chunk_slot(chunk_x, chunk_z).and_then(Option::as_ref)
    }

    pub fn chunk_mut(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Option<&mut MutableChunkBlockBuffer> {
        self.chunk_slot_mut(chunk_x, chunk_z)
            .and_then(Option::as_mut)
    }

    pub fn into_chunk(mut self, chunk_x: i32, chunk_z: i32) -> Option<MutableChunkBlockBuffer> {
        self.remove_chunk(chunk_x, chunk_z)
    }

    pub fn remove_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> Option<MutableChunkBlockBuffer> {
        self.chunk_slot_mut(chunk_x, chunk_z).and_then(Option::take)
    }

    fn get_chunk(&self, chunk_x: i32, chunk_z: i32) -> &MutableChunkBlockBuffer {
        self.ensure_within_dependency_window(chunk_x, chunk_z);
        self.chunk(chunk_x, chunk_z)
            .unwrap_or_else(|| self.missing_dependency_chunk(chunk_x, chunk_z))
    }

    fn get_chunk_mut(&mut self, chunk_x: i32, chunk_z: i32) -> &mut MutableChunkBlockBuffer {
        self.ensure_within_dependency_window(chunk_x, chunk_z);
        if self.chunk(chunk_x, chunk_z).is_none() {
            self.missing_dependency_chunk(chunk_x, chunk_z);
        }
        self.chunk_mut(chunk_x, chunk_z)
            .expect("dependency chunk was checked above")
    }

    fn ensure_within_dependency_window(&self, chunk_x: i32, chunk_z: i32) {
        if (chunk_x - self.center_chunk_x).abs() > self.dependency_radius
            || (chunk_z - self.center_chunk_z).abs() > self.dependency_radius
        {
            panic!(
                "feature region access outside dependency window: center ({}, {}), requested ({chunk_x}, {chunk_z}), radius {}",
                self.center_chunk_x, self.center_chunk_z, self.dependency_radius
            );
        }
    }

    fn can_write_chunk(&self, chunk_x: i32, chunk_z: i32) -> bool {
        (chunk_x - self.center_chunk_x).abs() <= self.write_radius_cutoff
            && (chunk_z - self.center_chunk_z).abs() <= self.write_radius_cutoff
    }

    fn chunk_slot(&self, chunk_x: i32, chunk_z: i32) -> Option<&Option<MutableChunkBlockBuffer>> {
        self.chunk_index(chunk_x, chunk_z)
            .and_then(|index| self.chunks.get(index))
    }

    fn chunk_slot_mut(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Option<&mut Option<MutableChunkBlockBuffer>> {
        self.chunk_index(chunk_x, chunk_z)
            .and_then(|index| self.chunks.get_mut(index))
    }

    fn chunk_index(&self, chunk_x: i32, chunk_z: i32) -> Option<usize> {
        region_chunk_index(
            self.chunk_min_x,
            self.chunk_min_z,
            self.chunk_width,
            chunk_x,
            chunk_z,
        )
        .filter(|index| *index < self.chunks.len())
    }

    fn missing_dependency_chunk(&self, chunk_x: i32, chunk_z: i32) -> ! {
        panic!(
            "feature region missing dependency chunk ({chunk_x}, {chunk_z}) for center ({}, {})",
            self.center_chunk_x, self.center_chunk_z
        )
    }
}

impl FeatureWorld for FeatureRegion {
    fn center_chunk_x(&self) -> i32 {
        self.center_chunk_x
    }

    fn center_chunk_z(&self) -> i32 {
        self.center_chunk_z
    }

    fn decoration_chunk_x(&self) -> i32 {
        self.decoration_chunk_x
    }

    fn decoration_chunk_z(&self) -> i32 {
        self.decoration_chunk_z
    }

    fn min_y(&self) -> i32 {
        self.get_chunk(self.center_chunk_x, self.center_chunk_z)
            .min_y
    }

    fn height(&self) -> i32 {
        self.get_chunk(self.center_chunk_x, self.center_chunk_z)
            .height
    }

    fn non_air_block_count(&self) -> usize {
        self.chunks
            .iter()
            .filter_map(Option::as_ref)
            .map(MutableChunkBlockBuffer::non_air_block_count)
            .sum()
    }

    fn block_at_world(&mut self, pos: BlockPos) -> Option<RawBlockId> {
        let chunk_x = block_to_chunk_coord(pos.x);
        let chunk_z = block_to_chunk_coord(pos.z);
        self.metrics.block_reads += 1;
        let chunk = self.get_chunk(chunk_x, chunk_z);
        if !(chunk.min_y..chunk.min_y + chunk.height).contains(&pos.y) {
            return None;
        }
        Some(chunk.get_block_at_y(local_block_coord(pos.x), pos.y, local_block_coord(pos.z)))
    }

    fn height_at(&mut self, heightmap: HeightmapType, world_x: i32, world_z: i32) -> Option<i32> {
        let chunk_x = block_to_chunk_coord(world_x);
        let chunk_z = block_to_chunk_coord(world_z);
        self.metrics.height_queries += 1;
        let chunk = self.get_chunk(chunk_x, chunk_z);
        Some(heightmap_height(
            chunk,
            heightmap,
            local_block_coord(world_x),
            local_block_coord(world_z),
        ))
    }

    fn world_surface_height_at(&mut self, world_x: i32, world_z: i32) -> Option<i32> {
        let chunk_x = block_to_chunk_coord(world_x);
        let chunk_z = block_to_chunk_coord(world_z);
        self.metrics.height_queries += 1;
        let chunk = self.get_chunk(chunk_x, chunk_z);
        Some(chunk.world_surface_height(local_block_coord(world_x), local_block_coord(world_z)))
    }

    fn set_block_world(&mut self, pos: BlockPos, block_id: RawBlockId) -> bool {
        let chunk_x = block_to_chunk_coord(pos.x);
        let chunk_z = block_to_chunk_coord(pos.z);
        self.ensure_within_dependency_window(chunk_x, chunk_z);
        self.metrics.block_write_attempts += 1;
        if !self.can_write_chunk(chunk_x, chunk_z) {
            self.metrics.blocked_block_writes += 1;
            return false;
        }

        let chunk = self.get_chunk_mut(chunk_x, chunk_z);
        if !(chunk.min_y..chunk.min_y + chunk.height).contains(&pos.y) {
            return false;
        }
        chunk.set_block_at_y(
            local_block_coord(pos.x),
            pos.y,
            local_block_coord(pos.z),
            block_id,
        );
        self.metrics.block_writes += 1;
        true
    }

    fn glow_lichen_faces_world(&mut self, pos: BlockPos) -> u8 {
        let chunk_x = block_to_chunk_coord(pos.x);
        let chunk_z = block_to_chunk_coord(pos.z);
        self.metrics.block_reads += 1;
        let chunk = self.get_chunk(chunk_x, chunk_z);
        if !(chunk.min_y..chunk.min_y + chunk.height).contains(&pos.y) {
            return 0;
        }
        chunk.glow_lichen_faces_at_y(local_block_coord(pos.x), pos.y, local_block_coord(pos.z))
    }

    fn set_glow_lichen_faces_world(&mut self, pos: BlockPos, faces: u8) -> bool {
        let chunk_x = block_to_chunk_coord(pos.x);
        let chunk_z = block_to_chunk_coord(pos.z);
        self.ensure_within_dependency_window(chunk_x, chunk_z);
        self.metrics.block_write_attempts += 1;
        if !self.can_write_chunk(chunk_x, chunk_z) {
            self.metrics.blocked_block_writes += 1;
            return false;
        }

        let chunk = self.get_chunk_mut(chunk_x, chunk_z);
        if !(chunk.min_y..chunk.min_y + chunk.height).contains(&pos.y) {
            return false;
        }
        chunk.set_glow_lichen_faces_at_y(
            local_block_coord(pos.x),
            pos.y,
            local_block_coord(pos.z),
            faces,
        );
        self.metrics.block_writes += 1;
        true
    }

    fn glow_lichen_spread_direction_order(&mut self) -> [Direction; 6] {
        self.glow_lichen_spread_directions
    }

    fn set_glow_lichen_spread_direction_order(&mut self, directions: [Direction; 6]) {
        self.glow_lichen_spread_directions = directions;
    }

    fn schedule_liquid_tick_world(&mut self, pos: BlockPos, target: RawBlockId, delay: i32) {
        let chunk_x = block_to_chunk_coord(pos.x);
        let chunk_z = block_to_chunk_coord(pos.z);
        self.ensure_within_dependency_window(chunk_x, chunk_z);
        if !self.can_write_chunk(chunk_x, chunk_z) {
            return;
        }

        let chunk = self.get_chunk_mut(chunk_x, chunk_z);
        if !(chunk.min_y..chunk.min_y + chunk.height).contains(&pos.y) {
            return;
        }
        chunk.schedule_liquid_tick(pos.x, pos.y, pos.z, block_name(target), delay);
    }
}

fn region_chunk_index(
    min_chunk_x: i32,
    min_chunk_z: i32,
    chunk_width: usize,
    chunk_x: i32,
    chunk_z: i32,
) -> Option<usize> {
    let local_x = usize::try_from(chunk_x.checked_sub(min_chunk_x)?).ok()?;
    let local_z = usize::try_from(chunk_z.checked_sub(min_chunk_z)?).ok()?;
    local_z
        .checked_mul(chunk_width)
        .and_then(|row| row.checked_add(local_x))
}
