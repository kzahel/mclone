use mclone_core::{CHUNK_WIDTH, ChunkPos, chunk_min_block_coord};
use mclone_render::far_lod::FarTerrainLodMesh;

pub const DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS: u32 = 0;
pub const MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS: u32 = 1;
pub const DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS: u32 = 12;
pub const MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS: u32 = 64;
pub const DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS: u32 = 8;
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
    mesh: Option<FarTerrainLodMesh>,
}

impl FarTerrainLodCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.key = None;
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
            self.mesh = Some(build_far_terrain_lod_mesh(key));
            self.key = Some(key);
        }
        self.mesh.as_ref()
    }
}

fn build_far_terrain_lod_mesh(key: FarTerrainLodBuildKey) -> FarTerrainLodMesh {
    let inner_chunk_radius = key.render_distance.saturating_add(key.start_margin_chunks);
    let outer_chunk_radius = key
        .render_distance
        .saturating_add(key.extra_radius_chunks)
        .max(inner_chunk_radius.saturating_add(1));
    let spacing = key.sample_spacing_blocks.max(1);
    let side_chunks = outer_chunk_radius.saturating_mul(2).saturating_add(1);
    let side_blocks = side_chunks.saturating_mul(CHUNK_WIDTH as u32);
    let sample_count = ceil_div(side_blocks, spacing).saturating_add(1);
    if sample_count < 2 {
        return FarTerrainLodMesh::empty(key.revision());
    }

    let mut vertices = Vec::with_capacity(sample_count as usize * sample_count as usize * 7);
    let min_x = chunk_min_block_coord(key.center.x - outer_chunk_radius as i32);
    let min_z = chunk_min_block_coord(key.center.z - outer_chunk_radius as i32);
    for z_index in 0..sample_count {
        let z = grid_coord(min_z, z_index, spacing, side_blocks);
        for x_index in 0..sample_count {
            let x = grid_coord(min_x, x_index, spacing, side_blocks);
            let y = approximate_surface_y(key.seed, x, z);
            let color = approximate_surface_color(y);
            vertices.extend_from_slice(&[x as f32, y.max(SEA_LEVEL), z as f32]);
            vertices.extend_from_slice(&color);
        }
    }

    let mut indices = Vec::new();
    let cells_per_side = sample_count - 1;
    for z_cell in 0..cells_per_side {
        for x_cell in 0..cells_per_side {
            let x0 = grid_coord(min_x, x_cell, spacing, side_blocks);
            let x1 = grid_coord(min_x, x_cell + 1, spacing, side_blocks);
            let z0 = grid_coord(min_z, z_cell, spacing, side_blocks);
            let z1 = grid_coord(min_z, z_cell + 1, spacing, side_blocks);
            let cell_center_x = x0 + (x1 - x0) / 2;
            let cell_center_z = z0 + (z1 - z0) / 2;
            let chunk_distance =
                chunk_distance_from_center(key.center, cell_center_x, cell_center_z);
            if chunk_distance <= inner_chunk_radius || chunk_distance > outer_chunk_radius {
                continue;
            }

            let row = z_cell * sample_count;
            let a = row + x_cell;
            let b = a + 1;
            let c = a + sample_count;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    FarTerrainLodMesh::new(vertices, indices, key.revision())
}

fn ceil_div(value: u32, divisor: u32) -> u32 {
    value.saturating_add(divisor.saturating_sub(1)) / divisor.max(1)
}

fn grid_coord(min: i32, index: u32, spacing: u32, side_blocks: u32) -> i32 {
    min + index.saturating_mul(spacing).min(side_blocks) as i32
}

fn chunk_distance_from_center(center: ChunkPos, world_x: i32, world_z: i32) -> u32 {
    let chunk = ChunkPos::from_block_coords(world_x, world_z);
    (chunk.x - center.x)
        .unsigned_abs()
        .max((chunk.z - center.z).unsigned_abs())
}

fn approximate_surface_y(seed: i64, x: i32, z: i32) -> f32 {
    let seed_phase = (seed as f32 * 0.000_031).sin() * 400.0;
    let xf = x as f32 + seed_phase;
    let zf = z as f32 - seed_phase * 0.37;
    let continent = (xf * 0.0075).sin() * 18.0 + (zf * 0.0065).cos() * 14.0;
    let ridge = ((xf + zf) * 0.018).sin().abs() * 10.0;
    let local = seeded_noise(seed, x.div_euclid(24), z.div_euclid(24)) * 5.0;
    (SEA_LEVEL + continent + ridge + local).clamp(44.0, 124.0)
}

fn approximate_surface_color(y: f32) -> [f32; 4] {
    if y <= SEA_LEVEL + 0.5 {
        [0.16, 0.30, 0.62, 1.0]
    } else if y < SEA_LEVEL + 4.0 {
        [0.70, 0.64, 0.42, 1.0]
    } else if y > 102.0 {
        [0.84, 0.86, 0.84, 1.0]
    } else if y > 86.0 {
        [0.44, 0.44, 0.40, 1.0]
    } else {
        [0.30, 0.52, 0.24, 1.0]
    }
}

fn seeded_noise(seed: i64, x: i32, z: i32) -> f32 {
    let mut hash = seed as u64;
    hash ^= (x as u32 as u64).wrapping_mul(0x9e37_79b1);
    hash = mix_hash(hash);
    hash ^= (z as u32 as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    hash = mix_hash(hash);
    let normalized = ((hash >> 40) as u32) as f32 / 16_777_215.0;
    normalized * 2.0 - 1.0
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
        let key = FarTerrainLodBuildKey::new(
            12345,
            ChunkPos::new(0, 0),
            5,
            FarTerrainLodConfig::enabled(),
        );

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
    fn far_terrain_lod_cache_reuses_same_mesh_for_same_key() {
        let mut cache = FarTerrainLodCache::new();
        let first = cache
            .mesh_for_camera(
                FarTerrainLodConfig::enabled(),
                12345,
                ChunkPos::new(0, 0),
                5,
            )
            .expect("enabled LOD returns mesh")
            .revision();
        let second = cache
            .mesh_for_camera(
                FarTerrainLodConfig::enabled(),
                12345,
                ChunkPos::new(0, 0),
                5,
            )
            .expect("enabled LOD returns mesh")
            .revision();

        assert_eq!(first, second);
    }
}
