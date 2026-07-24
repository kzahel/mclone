use mclone_core::{CHUNK_WIDTH, ChunkPos};
use mclone_worldgen::block::{AIR, WATER};
use mclone_worldgen::levelgen::{
    GeneratedChunk, McloneOverworldFeatureDependencyCache,
    McloneOverworldFeatureDependencyCacheReport, generate_mclone_overworld_surface_chunk,
};

pub const CANONICAL_TERRAIN_MAX_CHUNK_RADIUS: u32 = 4;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum CanonicalTerrainStage {
    Surface,
    #[default]
    FinalFeatures,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalTerrainVisibility {
    pub water: bool,
    pub vegetation: bool,
}

impl Default for CanonicalTerrainVisibility {
    fn default() -> Self {
        Self {
            water: true,
            vegetation: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalTerrainChunk {
    pub seed: i64,
    pub stage: CanonicalTerrainStage,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub min_y: i32,
    pub height: i32,
    pub blocks: Vec<u8>,
    pub biomes: Vec<i32>,
    pub fingerprint: u64,
    pub dependency_cache: McloneOverworldFeatureDependencyCacheReport,
}

impl CanonicalTerrainChunk {
    fn from_generated(
        seed: i64,
        stage: CanonicalTerrainStage,
        chunk: GeneratedChunk,
        dependency_cache: McloneOverworldFeatureDependencyCacheReport,
    ) -> Self {
        let fingerprint = canonical_terrain_fingerprint(&chunk);
        Self {
            seed,
            stage,
            chunk_x: chunk.chunk_x,
            chunk_z: chunk.chunk_z,
            min_y: chunk.min_y,
            height: chunk.height,
            blocks: chunk.blocks().to_vec(),
            biomes: chunk.biomes().to_vec(),
            fingerprint,
            dependency_cache,
        }
    }

    pub fn presentation_blocks(&self, visibility: CanonicalTerrainVisibility) -> Vec<u8> {
        canonical_terrain_presentation_blocks(&self.blocks, visibility)
    }
}

pub fn canonical_terrain_presentation_blocks(
    blocks: &[u8],
    visibility: CanonicalTerrainVisibility,
) -> Vec<u8> {
    if visibility.vegetation {
        return blocks.to_vec();
    }
    blocks
        .iter()
        .copied()
        .map(canonical_preview_without_vegetation)
        .collect()
}

#[derive(Debug)]
pub struct CanonicalTerrainCompiler {
    seed: i64,
    stage: CanonicalTerrainStage,
    feature_dependencies: McloneOverworldFeatureDependencyCache,
}

impl CanonicalTerrainCompiler {
    pub fn new(seed: i64, stage: CanonicalTerrainStage) -> Self {
        Self {
            seed,
            stage,
            feature_dependencies: McloneOverworldFeatureDependencyCache::new(),
        }
    }

    pub fn seed(&self) -> i64 {
        self.seed
    }

    pub fn stage(&self) -> CanonicalTerrainStage {
        self.stage
    }

    pub fn retained_dependency_chunks(&self) -> usize {
        self.feature_dependencies.retained_chunk_count()
    }

    pub fn clear_cache(&mut self) {
        self.feature_dependencies.clear();
    }

    pub fn compile(&mut self, chunk_x: i32, chunk_z: i32) -> CanonicalTerrainChunk {
        match self.stage {
            CanonicalTerrainStage::Surface => CanonicalTerrainChunk::from_generated(
                self.seed,
                self.stage,
                generate_mclone_overworld_surface_chunk(self.seed, chunk_x, chunk_z),
                McloneOverworldFeatureDependencyCacheReport::default(),
            ),
            CanonicalTerrainStage::FinalFeatures => {
                let position = ChunkPos::new(chunk_x, chunk_z);
                let mut batch = self
                    .feature_dependencies
                    .generate_features_chunks(self.seed, [position]);
                let chunk = batch
                    .chunks
                    .remove(&position)
                    .expect("the feature batch returns every requested chunk");
                CanonicalTerrainChunk::from_generated(
                    self.seed,
                    self.stage,
                    chunk,
                    batch.cache_report,
                )
            }
        }
    }
}

pub fn canonical_terrain_chunk_order(center_x: i32, center_z: i32, radius: u32) -> Vec<ChunkPos> {
    let radius = radius.min(CANONICAL_TERRAIN_MAX_CHUNK_RADIUS) as i32;
    let center_chunk_x = center_x.div_euclid(CHUNK_WIDTH);
    let center_chunk_z = center_z.div_euclid(CHUNK_WIDTH);
    let mut positions = (-radius..=radius)
        .flat_map(|offset_z| {
            (-radius..=radius).map(move |offset_x| {
                ChunkPos::new(center_chunk_x + offset_x, center_chunk_z + offset_z)
            })
        })
        .collect::<Vec<_>>();
    positions.sort_by_key(|position| {
        let offset_x = position.x - center_chunk_x;
        let offset_z = position.z - center_chunk_z;
        (
            offset_x.abs().max(offset_z.abs()),
            offset_x * offset_x + offset_z * offset_z,
            offset_z,
            offset_x,
        )
    });
    positions
}

fn canonical_terrain_fingerprint(chunk: &GeneratedChunk) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for bytes in [
        chunk.chunk_x.to_le_bytes().as_slice(),
        chunk.chunk_z.to_le_bytes().as_slice(),
        chunk.min_y.to_le_bytes().as_slice(),
        chunk.height.to_le_bytes().as_slice(),
    ] {
        hash = fnv1a_extend(hash, bytes);
    }
    hash = fnv1a_extend(hash, chunk.blocks());
    for biome in chunk.biomes() {
        hash = fnv1a_extend(hash, &biome.to_le_bytes());
    }
    hash
}

fn fnv1a_extend(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn canonical_preview_without_vegetation(block: u8) -> u8 {
    if matches!(block, 107..=120 | 179..=208) {
        // Sea plants and coral are waterlogged in the production catalogue.
        // Removing their visible model should preserve the water volume.
        WATER
    } else if matches!(block, 41..=51 | 68..=69 | 105..=208) {
        AIR
    } else {
        block
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::block::{OAK_LEAVES, OAK_LOG, SEAGRASS, STONE};
    use mclone_worldgen::levelgen::generate_mclone_overworld_chunk;

    #[test]
    fn canonical_order_is_center_first_and_radius_bounded() {
        let positions = canonical_terrain_chunk_order(-1, 17, 99);
        assert_eq!(positions.len(), 81);
        assert_eq!(positions[0], ChunkPos::new(-1, 1));
        assert!(
            positions[1..9]
                .iter()
                .all(|position| { (position.x + 1).abs().max((position.z - 1).abs()) == 1 })
        );
    }

    #[test]
    fn final_compiler_returns_production_chunk_unchanged() {
        let mut compiler =
            CanonicalTerrainCompiler::new(-98_765, CanonicalTerrainStage::FinalFeatures);
        let result = compiler.compile(-1, 2);
        let direct = generate_mclone_overworld_chunk(-98_765, -1, 2);

        assert_eq!(result.blocks, direct.blocks());
        assert_eq!(result.biomes, direct.biomes());
        assert_eq!(result.fingerprint, canonical_terrain_fingerprint(&direct));
    }

    #[test]
    fn visibility_changes_presentation_without_changing_identity() {
        let chunk = CanonicalTerrainChunk {
            seed: 1,
            stage: CanonicalTerrainStage::FinalFeatures,
            chunk_x: 0,
            chunk_z: 0,
            min_y: 0,
            height: 16,
            blocks: vec![STONE, OAK_LOG, OAK_LEAVES, SEAGRASS],
            biomes: Vec::new(),
            fingerprint: 42,
            dependency_cache: McloneOverworldFeatureDependencyCacheReport::default(),
        };
        let presented = chunk.presentation_blocks(CanonicalTerrainVisibility {
            water: true,
            vegetation: false,
        });

        assert_eq!(presented, vec![STONE, AIR, AIR, WATER]);
        assert_eq!(chunk.blocks, vec![STONE, OAK_LOG, OAK_LEAVES, SEAGRASS]);
        assert_eq!(chunk.fingerprint, 42);
    }
}
