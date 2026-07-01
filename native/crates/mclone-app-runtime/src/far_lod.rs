use std::collections::{BTreeMap, VecDeque};

use mclone_core::{CHUNK_WIDTH, ChunkPos, chunk_min_block_coord};
use mclone_render::far_lod::FarTerrainLodMesh;
use mclone_worldgen::block::{
    ANDESITE, BEDROCK, BLACK_TERRACOTTA, BLUE_TERRACOTTA, BROWN_TERRACOTTA, CLAY, COARSE_DIRT,
    CYAN_TERRACOTTA, DIORITE, DIRT, GRANITE, GRASS_BLOCK, GRAVEL, GRAY_TERRACOTTA,
    GREEN_TERRACOTTA, GeneratedBlockId, ICE, LIGHT_BLUE_TERRACOTTA, LIGHT_GRAY_TERRACOTTA,
    LIME_TERRACOTTA, MAGENTA_TERRACOTTA, MYCELIUM, ORANGE_TERRACOTTA, PACKED_ICE, PINK_TERRACOTTA,
    PODZOL, PURPLE_TERRACOTTA, RED_SAND, RED_SANDSTONE, RED_TERRACOTTA, SAND, SANDSTONE, SNOW,
    SNOW_BLOCK, STONE, TERRACOTTA, WHITE_TERRACOTTA, YELLOW_TERRACOTTA, base_block_id, has_fluid,
    is_air_like, is_lava, is_water,
};
use mclone_worldgen::levelgen::{GeneratedChunk, generate_overworld_surface_chunk};

pub const DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS: u32 = 0;
pub const MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS: u32 = 1;
pub const DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS: u32 = 12;
pub const MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS: u32 = 64;
pub const DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS: u32 = 8;
pub const DEFAULT_FAR_TERRAIN_LOD_CHUNK_BUILD_BUDGET: usize = 4;
pub const SEA_LEVEL: f32 = 63.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FarTerrainLodConfig {
    pub enabled: bool,
    pub start_margin_chunks: u32,
    pub extra_radius_chunks: u32,
    pub sample_spacing_blocks: u32,
}

impl FarTerrainLodConfig {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            start_margin_chunks: DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS,
            extra_radius_chunks: DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
            sample_spacing_blocks: DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS,
        }
    }

    pub const fn enabled() -> Self {
        Self {
            enabled: true,
            start_margin_chunks: DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS,
            extra_radius_chunks: DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
            sample_spacing_blocks: DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS,
        }
    }

    pub fn with_extra_radius_chunks(mut self, extra_radius_chunks: u32) -> Self {
        self.extra_radius_chunks = extra_radius_chunks.clamp(
            MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
            MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
        );
        self
    }

    fn normalized(self) -> Self {
        Self {
            enabled: self.enabled,
            start_margin_chunks: self.start_margin_chunks,
            extra_radius_chunks: self.extra_radius_chunks.clamp(
                MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
                MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
            ),
            sample_spacing_blocks: self.sample_spacing_blocks.max(1),
        }
    }
}

impl Default for FarTerrainLodConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FarTerrainLodBuildKey {
    seed: i64,
    center: ChunkPos,
    render_distance: u32,
    start_margin_chunks: u32,
    extra_radius_chunks: u32,
    sample_spacing_blocks: u32,
}

impl FarTerrainLodBuildKey {
    fn new(seed: i64, center: ChunkPos, render_distance: u32, config: FarTerrainLodConfig) -> Self {
        let config = config.normalized();
        Self {
            seed,
            center,
            render_distance,
            start_margin_chunks: config.start_margin_chunks,
            extra_radius_chunks: config.extra_radius_chunks,
            sample_spacing_blocks: config.sample_spacing_blocks,
        }
    }

    fn revision(self) -> u64 {
        let mut hash = 0x9e37_79b9_7f4a_7c15_u64;
        hash = mix_hash(hash ^ self.seed as u64);
        hash = mix_hash(hash ^ self.center.x as u32 as u64);
        hash = mix_hash(hash ^ ((self.center.z as u32 as u64) << 1));
        hash = mix_hash(hash ^ ((self.render_distance as u64) << 2));
        hash = mix_hash(hash ^ ((self.start_margin_chunks as u64) << 8));
        hash = mix_hash(hash ^ ((self.extra_radius_chunks as u64) << 16));
        mix_hash(hash ^ ((self.sample_spacing_blocks as u64) << 24))
    }
}

#[derive(Debug, Default)]
pub struct FarTerrainLodCache {
    key: Option<FarTerrainLodBuildKey>,
    pending_chunks: VecDeque<ChunkPos>,
    surface_chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    vertices: Vec<f32>,
    indices: Vec<u32>,
    mesh_revision: u64,
    mesh: Option<FarTerrainLodMesh>,
}

impl FarTerrainLodCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.key = None;
        self.pending_chunks.clear();
        self.surface_chunks.clear();
        self.vertices.clear();
        self.indices.clear();
        self.mesh_revision = 0;
        self.mesh = None;
    }

    pub fn mesh_for_camera(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        center: ChunkPos,
        render_distance: u32,
    ) -> Option<&FarTerrainLodMesh> {
        if !config.enabled {
            self.clear();
            return None;
        }
        let key = FarTerrainLodBuildKey::new(seed, center, render_distance, config);
        if self.key != Some(key) {
            self.begin_build(key);
        }
        self.advance_build(DEFAULT_FAR_TERRAIN_LOD_CHUNK_BUILD_BUDGET);
        self.mesh.as_ref()
    }

    fn begin_build(&mut self, key: FarTerrainLodBuildKey) {
        self.key = Some(key);
        self.pending_chunks = far_lod_chunk_positions(key);
        self.surface_chunks.clear();
        self.vertices.clear();
        self.indices.clear();
        self.mesh_revision = key.revision();
        self.mesh = Some(FarTerrainLodMesh::empty(self.mesh_revision));
    }

    fn advance_build(&mut self, chunk_budget: usize) {
        let Some(key) = self.key else {
            return;
        };
        let mut generated_chunks = 0;
        for _ in 0..chunk_budget {
            let Some(pos) = self.pending_chunks.pop_front() else {
                break;
            };
            append_far_terrain_lod_chunk_patch(
                &mut self.vertices,
                &mut self.indices,
                &mut self.surface_chunks,
                key.seed,
                pos,
                key,
            );
            generated_chunks += 1;
        }
        if generated_chunks > 0 {
            self.mesh_revision = self.mesh_revision.wrapping_add(generated_chunks as u64);
            self.mesh = Some(FarTerrainLodMesh::new(
                self.vertices.clone(),
                self.indices.clone(),
                self.mesh_revision,
            ));
        }
    }
}

fn far_lod_chunk_radii(key: FarTerrainLodBuildKey) -> (u32, u32) {
    let inner_chunk_radius = key.render_distance.saturating_add(key.start_margin_chunks);
    let outer_chunk_radius = key
        .render_distance
        .saturating_add(key.extra_radius_chunks)
        .max(inner_chunk_radius.saturating_add(1));
    (inner_chunk_radius, outer_chunk_radius)
}

fn far_lod_chunk_positions(key: FarTerrainLodBuildKey) -> VecDeque<ChunkPos> {
    let (inner_chunk_radius, outer_chunk_radius) = far_lod_chunk_radii(key);
    let outer = outer_chunk_radius as i32;
    let mut positions = Vec::new();
    for dz in -outer..=outer {
        for dx in -outer..=outer {
            let distance = dx.unsigned_abs().max(dz.unsigned_abs());
            if distance > inner_chunk_radius && distance <= outer_chunk_radius {
                positions.push(ChunkPos::new(key.center.x + dx, key.center.z + dz));
            }
        }
    }
    positions.sort_by_key(|pos| {
        let distance = (pos.x - key.center.x)
            .unsigned_abs()
            .max((pos.z - key.center.z).unsigned_abs());
        (distance, pos.z, pos.x)
    });
    positions.into()
}

#[cfg(test)]
fn build_far_terrain_lod_mesh(key: FarTerrainLodBuildKey) -> FarTerrainLodMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut surface_chunks = BTreeMap::new();
    for pos in far_lod_chunk_positions(key) {
        append_far_terrain_lod_chunk_patch(
            &mut vertices,
            &mut indices,
            &mut surface_chunks,
            key.seed,
            pos,
            key,
        );
    }
    FarTerrainLodMesh::new(vertices, indices, key.revision())
}

fn append_far_terrain_lod_chunk_patch(
    vertices: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    surface_chunks: &mut BTreeMap<ChunkPos, GeneratedChunk>,
    seed: i64,
    pos: ChunkPos,
    key: FarTerrainLodBuildKey,
) {
    let spacing = key.sample_spacing_blocks.max(1);
    let offsets = chunk_sample_offsets(spacing);
    debug_assert!(offsets.len() >= 2);

    let mut grid = Vec::with_capacity(offsets.len() * offsets.len());
    let min_x = chunk_min_block_coord(pos.x);
    let min_z = chunk_min_block_coord(pos.z);
    for z_offset in &offsets {
        for x_offset in &offsets {
            let world_x = min_x + *x_offset;
            let world_z = min_z + *z_offset;
            if let Some(sample) = surface_sample_world(surface_chunks, seed, world_x, world_z) {
                let index = u32::try_from(vertices.len() / 7)
                    .expect("far terrain LOD vertex count fits u32");
                vertices.extend_from_slice(&[world_x as f32, sample.y, world_z as f32]);
                vertices.extend_from_slice(&sample.color);
                grid.push(Some(index));
            } else {
                grid.push(None);
            }
        }
    }

    let side = offsets.len();
    for z_cell in 0..side - 1 {
        for x_cell in 0..side - 1 {
            let a = grid[z_cell * side + x_cell];
            let b = grid[z_cell * side + x_cell + 1];
            let c = grid[(z_cell + 1) * side + x_cell];
            let d = grid[(z_cell + 1) * side + x_cell + 1];
            if let (Some(a), Some(b), Some(c), Some(d)) = (a, b, c, d) {
                indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
    }
}

fn chunk_sample_offsets(spacing: u32) -> Vec<i32> {
    let spacing = spacing.max(1).min(CHUNK_WIDTH as u32);
    let mut offsets = Vec::new();
    let mut offset = 0;
    while offset < CHUNK_WIDTH as u32 {
        offsets.push(offset as i32);
        offset = offset.saturating_add(spacing);
    }
    if offsets.last().copied() != Some(CHUNK_WIDTH) {
        offsets.push(CHUNK_WIDTH);
    }
    offsets
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FarTerrainSurfaceSample {
    y: f32,
    color: [f32; 4],
}

fn surface_sample(
    chunk: &GeneratedChunk,
    local_x: i32,
    local_z: i32,
) -> Option<FarTerrainSurfaceSample> {
    for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
        let block = chunk.block_at_y(local_x, y, local_z);
        if is_air_like(block.raw()) {
            continue;
        }
        let surface_y = y + 1;
        return Some(FarTerrainSurfaceSample {
            y: surface_y as f32,
            color: surface_color(block, surface_y),
        });
    }
    None
}

fn surface_sample_world(
    chunks: &mut BTreeMap<ChunkPos, GeneratedChunk>,
    seed: i64,
    world_x: i32,
    world_z: i32,
) -> Option<FarTerrainSurfaceSample> {
    let pos = ChunkPos::from_block_coords(world_x, world_z);
    let chunk = chunks
        .entry(pos)
        .or_insert_with(|| generate_overworld_surface_chunk(seed, pos.x, pos.z));
    let local_x = world_x - chunk_min_block_coord(pos.x);
    let local_z = world_z - chunk_min_block_coord(pos.z);
    surface_sample(chunk, local_x, local_z)
}

#[cfg(test)]
fn chunk_distance_from_center(center: ChunkPos, world_x: i32, world_z: i32) -> u32 {
    let chunk = ChunkPos::from_block_coords(world_x, world_z);
    (chunk.x - center.x)
        .unsigned_abs()
        .max((chunk.z - center.z).unsigned_abs())
}

fn surface_color(block: GeneratedBlockId, surface_y: i32) -> [f32; 4] {
    let block_id = base_block_id(block.raw());
    if is_water(block_id) {
        return [0.14, 0.28, 0.58, 1.0];
    }
    if is_lava(block_id) {
        return [0.88, 0.30, 0.08, 1.0];
    }
    if has_fluid(block_id) {
        return [0.16, 0.30, 0.62, 1.0];
    }

    match block_id {
        GRASS_BLOCK => [0.30, 0.50, 0.22, 1.0],
        DIRT => [0.36, 0.25, 0.15, 1.0],
        COARSE_DIRT | PODZOL => [0.28, 0.20, 0.13, 1.0],
        MYCELIUM => [0.42, 0.36, 0.42, 1.0],
        SAND | SANDSTONE => [0.72, 0.66, 0.42, 1.0],
        RED_SAND | RED_SANDSTONE => [0.64, 0.32, 0.16, 1.0],
        GRAVEL => [0.42, 0.42, 0.40, 1.0],
        STONE | BEDROCK | ANDESITE | DIORITE | GRANITE => {
            if surface_y > 104 {
                [0.58, 0.58, 0.54, 1.0]
            } else {
                [0.44, 0.44, 0.40, 1.0]
            }
        }
        SNOW | SNOW_BLOCK => [0.88, 0.90, 0.86, 1.0],
        ICE | PACKED_ICE => [0.58, 0.74, 0.86, 1.0],
        CLAY => [0.48, 0.50, 0.54, 1.0],
        TERRACOTTA => [0.55, 0.30, 0.20, 1.0],
        WHITE_TERRACOTTA => [0.72, 0.62, 0.52, 1.0],
        ORANGE_TERRACOTTA => [0.64, 0.32, 0.16, 1.0],
        MAGENTA_TERRACOTTA => [0.58, 0.32, 0.44, 1.0],
        LIGHT_BLUE_TERRACOTTA => [0.44, 0.46, 0.58, 1.0],
        YELLOW_TERRACOTTA => [0.70, 0.54, 0.24, 1.0],
        LIME_TERRACOTTA => [0.48, 0.54, 0.28, 1.0],
        PINK_TERRACOTTA => [0.62, 0.38, 0.36, 1.0],
        GRAY_TERRACOTTA => [0.36, 0.30, 0.28, 1.0],
        LIGHT_GRAY_TERRACOTTA => [0.52, 0.46, 0.42, 1.0],
        CYAN_TERRACOTTA => [0.34, 0.38, 0.38, 1.0],
        PURPLE_TERRACOTTA => [0.46, 0.30, 0.42, 1.0],
        BLUE_TERRACOTTA => [0.32, 0.30, 0.44, 1.0],
        BROWN_TERRACOTTA => [0.38, 0.24, 0.16, 1.0],
        GREEN_TERRACOTTA => [0.34, 0.36, 0.20, 1.0],
        RED_TERRACOTTA => [0.56, 0.24, 0.18, 1.0],
        BLACK_TERRACOTTA => [0.18, 0.15, 0.14, 1.0],
        _ => {
            if surface_y <= SEA_LEVEL as i32 + 1 {
                [0.32, 0.44, 0.24, 1.0]
            } else if surface_y > 102 {
                [0.58, 0.58, 0.54, 1.0]
            } else {
                [0.30, 0.50, 0.22, 1.0]
            }
        }
    }
}

fn mix_hash(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn far_terrain_lod_config_defaults_disabled() {
        assert!(!FarTerrainLodConfig::default().enabled);
        assert!(FarTerrainLodConfig::enabled().enabled);
        assert_eq!(
            FarTerrainLodConfig::enabled().start_margin_chunks,
            DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS
        );
        assert_eq!(DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS, 0);
    }

    #[test]
    fn far_terrain_lod_cache_returns_none_when_disabled() {
        let mut cache = FarTerrainLodCache::new();

        let mesh = cache.mesh_for_camera(
            FarTerrainLodConfig::default(),
            12345,
            ChunkPos::new(0, 0),
            5,
        );

        assert!(mesh.is_none());
    }

    #[test]
    fn far_terrain_lod_mesh_has_surface_ring_geometry() {
        let config = FarTerrainLodConfig::enabled().with_extra_radius_chunks(1);
        let key = FarTerrainLodBuildKey::new(12345, ChunkPos::new(0, 0), 0, config);

        let mesh = build_far_terrain_lod_mesh(key);

        assert!(mesh.vertex_count() > 0);
        assert!(mesh.index_count() > 0);
        assert_eq!(mesh.index_count() % 3, 0);
        assert_eq!(mesh.revision(), key.revision());
    }

    #[test]
    fn far_terrain_lod_chunk_distance_matches_render_distance_boundary() {
        let center = ChunkPos::new(0, 0);
        let last_normal_x = chunk_min_block_coord(5) + CHUNK_WIDTH - 1;
        let first_lod_x = chunk_min_block_coord(6);

        assert_eq!(chunk_distance_from_center(center, last_normal_x, 0), 5);
        assert_eq!(chunk_distance_from_center(center, first_lod_x, 0), 6);
    }

    #[test]
    fn far_terrain_lod_range_is_clamped() {
        assert_eq!(
            FarTerrainLodConfig::enabled()
                .with_extra_radius_chunks(0)
                .normalized()
                .extra_radius_chunks,
            MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS
        );
        assert_eq!(
            FarTerrainLodConfig::enabled()
                .with_extra_radius_chunks(MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS + 1)
                .normalized()
                .extra_radius_chunks,
            MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS
        );
    }

    #[test]
    fn far_terrain_lod_mesh_uses_real_surface_chunk_height() {
        let chunk = generate_overworld_surface_chunk(12345, 1, 0);
        let sample = surface_sample(&chunk, 0, 0).expect("surface chunk has terrain");
        let mut expected = None;
        for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
            let block = chunk.block_at_y(0, y, 0);
            if !is_air_like(block.raw()) {
                expected = Some(y + 1);
                break;
            }
        }

        assert_eq!(
            sample.y,
            expected.expect("manual scan found terrain") as f32
        );
    }

    #[test]
    fn far_terrain_lod_world_boundary_sample_uses_owning_chunk() {
        let mut chunks = BTreeMap::new();
        let world_x = chunk_min_block_coord(1);

        let _sample =
            surface_sample_world(&mut chunks, 12345, world_x, 0).expect("surface sample exists");

        assert!(chunks.contains_key(&ChunkPos::new(1, 0)));
        assert!(!chunks.contains_key(&ChunkPos::new(0, 0)));
    }

    #[test]
    fn far_terrain_lod_cache_advances_incrementally_then_stabilizes() {
        let mut cache = FarTerrainLodCache::new();
        let config = FarTerrainLodConfig::enabled().with_extra_radius_chunks(1);
        let first = cache
            .mesh_for_camera(config, 12345, ChunkPos::new(0, 0), 0)
            .expect("enabled LOD returns mesh")
            .revision();
        assert_eq!(cache.pending_chunks.len(), 4);
        let second = cache
            .mesh_for_camera(config, 12345, ChunkPos::new(0, 0), 0)
            .expect("enabled LOD returns mesh")
            .revision();
        assert!(second > first);
        assert!(cache.pending_chunks.is_empty());
        let third = cache
            .mesh_for_camera(config, 12345, ChunkPos::new(0, 0), 0)
            .expect("enabled LOD returns mesh")
            .revision();

        assert_eq!(second, third);
    }
}
