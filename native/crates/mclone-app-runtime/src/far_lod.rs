use mclone_core::{CHUNK_WIDTH, ChunkPos};
use mclone_render::far_lod::FarTerrainLodMesh;

pub const DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS: u32 = 2;
pub const DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS: u32 = 12;
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

    fn normalized(self) -> Self {
        Self {
            enabled: self.enabled,
            start_margin_chunks: self.start_margin_chunks.max(1),
            extra_radius_chunks: self.extra_radius_chunks.max(1),
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
    let start_blocks =
        (key.render_distance + key.start_margin_chunks).saturating_mul(CHUNK_WIDTH as u32);
    let end_blocks = (key.render_distance + key.extra_radius_chunks)
        .max(key.render_distance + key.start_margin_chunks + 1)
        .saturating_mul(CHUNK_WIDTH as u32);
    let spacing = key.sample_spacing_blocks.max(1);
    if end_blocks <= start_blocks {
        return FarTerrainLodMesh::empty(key.revision());
    }

    let center_x = key.center.middle_block_x();
    let center_z = key.center.middle_block_z();
    let sample_count = (end_blocks.saturating_mul(2) / spacing).saturating_add(1);
    if sample_count < 2 {
        return FarTerrainLodMesh::empty(key.revision());
    }

    let mut vertices = Vec::with_capacity(sample_count as usize * sample_count as usize * 7);
    let min_x = center_x - end_blocks as i32;
    let min_z = center_z - end_blocks as i32;
    for z_index in 0..sample_count {
        let z = min_z + (z_index * spacing) as i32;
        for x_index in 0..sample_count {
            let x = min_x + (x_index * spacing) as i32;
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
            let cell_center_x = min_x + (x_cell * spacing + spacing / 2) as i32;
            let cell_center_z = min_z + (z_cell * spacing + spacing / 2) as i32;
            let dx = (cell_center_x - center_x).unsigned_abs();
            let dz = (cell_center_z - center_z).unsigned_abs();
            let distance = dx.max(dz);
            if distance < start_blocks || distance > end_blocks {
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
