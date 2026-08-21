use mclone_core::{CHUNK_WIDTH, ChunkPos};
use mclone_worldgen::block::{AIR, RawBlockId, WATER};
use mclone_worldgen::levelgen::{
    ContinentalCandidateFeatureDependencyCache, ContinentalCandidateFeatureDependencyCacheReport,
    GeneratedChunk, McloneOverworldFeatureDependencyCache,
    McloneOverworldFeatureDependencyCacheReport, OverworldFeatureDependencyCache,
    OverworldFeatureDependencyCacheReport, generate_mclone_overworld_surface_chunk,
    generate_overworld_surface_chunk,
};
use mclone_worldgen::terrain_preview::TerrainPreviewProfile;

pub const CANONICAL_TERRAIN_MAX_CHUNK_RADIUS: u32 = 15;

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
    pub profile: TerrainPreviewProfile,
    pub seed: i64,
    pub stage: CanonicalTerrainStage,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub min_y: i32,
    pub height: i32,
    pub blocks: Vec<RawBlockId>,
    pub biomes: Vec<i32>,
    pub fingerprint: u64,
    pub dependency_cache: CanonicalTerrainDependencyCacheReport,
}

impl CanonicalTerrainChunk {
    fn from_generated(
        profile: TerrainPreviewProfile,
        seed: i64,
        stage: CanonicalTerrainStage,
        chunk: GeneratedChunk,
        dependency_cache: CanonicalTerrainDependencyCacheReport,
    ) -> Self {
        let fingerprint = canonical_terrain_fingerprint(&chunk);
        Self {
            profile,
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

    pub fn presentation_blocks(&self, visibility: CanonicalTerrainVisibility) -> Vec<RawBlockId> {
        canonical_terrain_presentation_blocks(&self.blocks, visibility)
    }
}

pub fn canonical_terrain_presentation_blocks(
    blocks: &[RawBlockId],
    visibility: CanonicalTerrainVisibility,
) -> Vec<RawBlockId> {
    if visibility.vegetation {
        return blocks.to_vec();
    }
    blocks
        .iter()
        .copied()
        .map(canonical_preview_without_vegetation)
        .collect()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CanonicalTerrainDependencyCacheReport {
    pub requested_dependency_chunks: usize,
    pub cache_hits: usize,
    pub generated_dependency_chunks: usize,
    pub retained_dependency_chunks: usize,
}

impl From<McloneOverworldFeatureDependencyCacheReport> for CanonicalTerrainDependencyCacheReport {
    fn from(report: McloneOverworldFeatureDependencyCacheReport) -> Self {
        Self {
            requested_dependency_chunks: report.requested_dependency_chunks,
            cache_hits: report.cache_hits,
            generated_dependency_chunks: report.generated_dependency_chunks,
            retained_dependency_chunks: report.retained_dependency_chunks,
        }
    }
}

impl From<ContinentalCandidateFeatureDependencyCacheReport>
    for CanonicalTerrainDependencyCacheReport
{
    fn from(report: ContinentalCandidateFeatureDependencyCacheReport) -> Self {
        Self {
            requested_dependency_chunks: report.requested_dependency_chunks,
            cache_hits: report.cache_hits,
            generated_dependency_chunks: report.generated_dependency_chunks,
            retained_dependency_chunks: report.retained_dependency_chunks,
        }
    }
}

impl From<OverworldFeatureDependencyCacheReport> for CanonicalTerrainDependencyCacheReport {
    fn from(report: OverworldFeatureDependencyCacheReport) -> Self {
        Self {
            requested_dependency_chunks: report.requested_dependency_chunks,
            cache_hits: report.cache_hits,
            generated_dependency_chunks: report.generated_dependency_chunks,
            retained_dependency_chunks: report.retained_dependency_chunks,
        }
    }
}

#[derive(Debug)]
pub struct CanonicalTerrainCompiler {
    profile: TerrainPreviewProfile,
    seed: i64,
    stage: CanonicalTerrainStage,
    feature_dependencies: CanonicalTerrainFeatureDependencies,
}

#[derive(Debug)]
enum CanonicalTerrainFeatureDependencies {
    Continental(ContinentalCandidateFeatureDependencyCache),
    Mclone(McloneOverworldFeatureDependencyCache),
    Vanilla(OverworldFeatureDependencyCache),
}

impl CanonicalTerrainCompiler {
    pub fn new(seed: i64, stage: CanonicalTerrainStage) -> Self {
        Self::new_with_profile(TerrainPreviewProfile::McloneOverworldV1, seed, stage)
    }

    pub fn new_with_profile(
        profile: TerrainPreviewProfile,
        seed: i64,
        stage: CanonicalTerrainStage,
    ) -> Self {
        Self {
            profile,
            seed,
            stage,
            feature_dependencies: match profile {
                TerrainPreviewProfile::McloneOverworldV1 => {
                    CanonicalTerrainFeatureDependencies::Mclone(
                        McloneOverworldFeatureDependencyCache::new(),
                    )
                }
                TerrainPreviewProfile::VanillaOverworld => {
                    CanonicalTerrainFeatureDependencies::Vanilla(
                        OverworldFeatureDependencyCache::new(),
                    )
                }
                TerrainPreviewProfile::ContinentalEcoregionCandidate
                | TerrainPreviewProfile::McloneOverworldV2 => {
                    CanonicalTerrainFeatureDependencies::Continental(
                        ContinentalCandidateFeatureDependencyCache::new(seed),
                    )
                }
            },
        }
    }

    pub fn profile(&self) -> TerrainPreviewProfile {
        self.profile
    }

    pub fn seed(&self) -> i64 {
        self.seed
    }

    pub fn stage(&self) -> CanonicalTerrainStage {
        self.stage
    }

    pub fn retained_dependency_chunks(&self) -> usize {
        match &self.feature_dependencies {
            CanonicalTerrainFeatureDependencies::Continental(_) => 0,
            CanonicalTerrainFeatureDependencies::Mclone(cache) => cache.retained_chunk_count(),
            CanonicalTerrainFeatureDependencies::Vanilla(cache) => cache.retained_chunk_count(),
        }
    }

    pub fn clear_cache(&mut self) {
        match &mut self.feature_dependencies {
            CanonicalTerrainFeatureDependencies::Continental(_) => {}
            CanonicalTerrainFeatureDependencies::Mclone(cache) => cache.clear(),
            CanonicalTerrainFeatureDependencies::Vanilla(cache) => cache.clear(),
        }
    }

    pub fn compile(&mut self, chunk_x: i32, chunk_z: i32) -> CanonicalTerrainChunk {
        match self.stage {
            CanonicalTerrainStage::Surface => CanonicalTerrainChunk::from_generated(
                self.profile,
                self.seed,
                self.stage,
                match self.profile {
                    TerrainPreviewProfile::McloneOverworldV1 => {
                        generate_mclone_overworld_surface_chunk(self.seed, chunk_x, chunk_z)
                    }
                    TerrainPreviewProfile::VanillaOverworld => {
                        generate_overworld_surface_chunk(self.seed, chunk_x, chunk_z)
                    }
                    TerrainPreviewProfile::ContinentalEcoregionCandidate
                    | TerrainPreviewProfile::McloneOverworldV2 => {
                        match &self.feature_dependencies {
                            CanonicalTerrainFeatureDependencies::Continental(generator) => {
                                generator
                                    .generator()
                                    .generate_surface_chunk(chunk_x, chunk_z)
                            }
                            _ => unreachable!("candidate exact compiler retains its generator"),
                        }
                    }
                },
                CanonicalTerrainDependencyCacheReport::default(),
            ),
            CanonicalTerrainStage::FinalFeatures => {
                let position = ChunkPos::new(chunk_x, chunk_z);
                let (chunk, cache_report) = match &mut self.feature_dependencies {
                    CanonicalTerrainFeatureDependencies::Continental(cache) => {
                        let (chunk, report) = cache.generate_features_chunk(chunk_x, chunk_z);
                        (chunk, report.into())
                    }
                    CanonicalTerrainFeatureDependencies::Mclone(cache) => {
                        let mut batch = cache.generate_features_chunks(self.seed, [position]);
                        (
                            batch
                                .chunks
                                .remove(&position)
                                .expect("the feature batch returns every requested chunk"),
                            batch.cache_report.into(),
                        )
                    }
                    CanonicalTerrainFeatureDependencies::Vanilla(cache) => {
                        let mut batch = cache.generate_features_chunks(self.seed, [position]);
                        (
                            batch
                                .chunks
                                .remove(&position)
                                .expect("the feature batch returns every requested chunk"),
                            batch.cache_report.into(),
                        )
                    }
                };
                CanonicalTerrainChunk::from_generated(
                    self.profile,
                    self.seed,
                    self.stage,
                    chunk,
                    cache_report,
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
    for block in chunk.blocks() {
        hash = fnv1a_extend(hash, &block.to_le_bytes());
    }
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

fn canonical_preview_without_vegetation(block: RawBlockId) -> RawBlockId {
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
    use mclone_worldgen::levelgen::{
        generate_continental_candidate_chunk, generate_continental_candidate_surface_chunk,
        generate_mclone_overworld_chunk,
    };

    #[test]
    fn canonical_order_is_center_first_and_radius_bounded() {
        let positions = canonical_terrain_chunk_order(-1, 17, 99);
        assert_eq!(positions.len(), 961);
        assert_eq!(positions[0], ChunkPos::new(-1, 1));
        assert!(
            positions[1..9]
                .iter()
                .all(|position| { (position.x + 1).abs().max((position.z - 1).abs()) == 1 })
        );
        assert!(positions.iter().all(|position| {
            (position.x + 1).abs().max((position.z - 1).abs())
                <= CANONICAL_TERRAIN_MAX_CHUNK_RADIUS as i32
        }));
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
            profile: TerrainPreviewProfile::McloneOverworldV1,
            seed: 1,
            stage: CanonicalTerrainStage::FinalFeatures,
            chunk_x: 0,
            chunk_z: 0,
            min_y: 0,
            height: 16,
            blocks: vec![STONE, OAK_LOG, OAK_LEAVES, SEAGRASS],
            biomes: Vec::new(),
            fingerprint: 42,
            dependency_cache: CanonicalTerrainDependencyCacheReport::default(),
        };
        let presented = chunk.presentation_blocks(CanonicalTerrainVisibility {
            water: true,
            vegetation: false,
        });

        assert_eq!(presented, vec![STONE, AIR, AIR, WATER]);
        assert_eq!(chunk.blocks, vec![STONE, OAK_LOG, OAK_LEAVES, SEAGRASS]);
        assert_eq!(chunk.fingerprint, 42);
    }

    #[test]
    fn vanilla_surface_compiler_returns_production_chunk_unchanged() {
        let seed = 12_345;
        let mut compiler = CanonicalTerrainCompiler::new_with_profile(
            TerrainPreviewProfile::VanillaOverworld,
            seed,
            CanonicalTerrainStage::Surface,
        );
        let result = compiler.compile(5, 115);
        let direct = generate_overworld_surface_chunk(seed, 5, 115);

        assert_eq!(result.profile, TerrainPreviewProfile::VanillaOverworld);
        assert_eq!(result.blocks, direct.blocks());
        assert_eq!(result.biomes, direct.biomes());
        assert_eq!(result.fingerprint, canonical_terrain_fingerprint(&direct));
    }

    #[test]
    fn candidate_surface_and_final_stages_use_the_detached_exact_lowering() {
        let seed = 12_345;
        for profile in [
            TerrainPreviewProfile::ContinentalEcoregionCandidate,
            TerrainPreviewProfile::McloneOverworldV2,
        ] {
            for (stage, direct) in [
                (
                    CanonicalTerrainStage::Surface,
                    generate_continental_candidate_surface_chunk(seed, -17, 31),
                ),
                (
                    CanonicalTerrainStage::FinalFeatures,
                    generate_continental_candidate_chunk(seed, -17, 31),
                ),
            ] {
                let mut compiler = CanonicalTerrainCompiler::new_with_profile(profile, seed, stage);
                let result = compiler.compile(-17, 31);
                assert_eq!(result.profile, profile);
                assert_eq!(result.blocks, direct.blocks());
                assert_eq!(result.biomes, direct.biomes());
                assert_eq!(result.fingerprint, canonical_terrain_fingerprint(&direct));
                if stage == CanonicalTerrainStage::Surface {
                    assert_eq!(result.dependency_cache, Default::default());
                } else {
                    assert_eq!(result.dependency_cache.requested_dependency_chunks, 9);
                    assert_eq!(result.dependency_cache.generated_dependency_chunks, 9);
                    assert_eq!(result.dependency_cache.retained_dependency_chunks, 9);
                }
            }
        }
    }

    #[test]
    fn vanilla_final_compiler_returns_production_chunk_unchanged() {
        use mclone_worldgen::levelgen::generate_overworld_features_chunk;

        let seed = 16;
        let mut compiler = CanonicalTerrainCompiler::new_with_profile(
            TerrainPreviewProfile::VanillaOverworld,
            seed,
            CanonicalTerrainStage::FinalFeatures,
        );
        let result = compiler.compile(0, 0);
        let direct = generate_overworld_features_chunk(seed, 0, 0);

        assert_eq!(result.blocks, direct.blocks());
        assert_eq!(result.biomes, direct.biomes());
        assert_eq!(result.fingerprint, canonical_terrain_fingerprint(&direct));
        assert!(result.dependency_cache.generated_dependency_chunks > 0);
    }
}
