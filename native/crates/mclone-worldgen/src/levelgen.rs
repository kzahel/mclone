mod alpha;
mod chunk;
mod feature_batch;
mod generator;
mod mclone_overworld;
mod planning;
mod profile;
mod sampler;
mod settings;
mod surface_dependency_cache;
mod timing;

pub use crate::biome::{ConstantBiomeSource, NoiseBiome, NoiseBiomeSource};
pub use chunk::{GeneratedChunk, MutableChunkBlockBuffer, ScheduledTick};
pub use feature_batch::{
    OverworldFeatureBatchResult, OverworldFeatureDependencyCache,
    OverworldFeatureDependencyCacheReport, generate_overworld_features_chunk,
    generate_overworld_features_chunks, generate_overworld_surface_chunk,
};
pub use generator::NoiseBasedChunkGenerator;
pub use mclone_overworld::{
    MCLONE_OVERWORLD_DECORATION_REVISION, MCLONE_OVERWORLD_EXPOSED_STONE_MIN_RELIEF,
    MCLONE_OVERWORLD_EXPOSED_STONE_MIN_Y, MCLONE_OVERWORLD_FIELD_REVISION,
    MCLONE_OVERWORLD_FOREST_BIOME_ID, MCLONE_OVERWORLD_SEA_LEVEL,
    MCLONE_OVERWORLD_WOODED_UPLAND_MIN_Y, McloneOverworldFeatureBatchResult,
    McloneOverworldFeatureDependencyCache, McloneOverworldFeatureDependencyCacheReport,
    McloneOverworldSampleRegion, McloneOverworldSampleRegionRequest, McloneOverworldSampler,
    McloneOverworldSurfaceRecipe, McloneOverworldTerrainSample, generate_mclone_overworld_chunk,
    generate_mclone_overworld_surface_chunk, mclone_overworld_biome_id,
    mclone_overworld_biome_id_for_sample, mclone_overworld_spawn_chunk,
    mclone_overworld_surface_recipe,
};
pub use planning::{ChunkGenerationPlan, ChunkStatusRequirement};
pub use profile::{
    BEACH_BIOME_ID, FLAT_GRASS_HEIGHT, FLAT_GRASS_MIN_Y, FLAT_GRASS_SURFACE_Y, OCEAN_BIOME_ID,
    PLAINS_BIOME_ID, SMALL_ISLAND_ENVELOPE_RADIUS, SMALL_ISLAND_OCEAN_FLOOR_Y,
    SMALL_ISLAND_SEA_LEVEL, SMALL_ISLAND_SPAWN_PATCH_MAX_EXCLUSIVE, SMALL_ISLAND_SPAWN_PATCH_MIN,
    SMALL_ISLAND_SPAWN_SURFACE_Y, SMALL_ISLAND_SUPPORT_RADIUS, SmallIslandFeatureBatchResult,
    SmallIslandFeatureDependencyCache, SmallIslandFeatureDependencyCacheReport,
    generate_flat_grass_chunk, generate_small_island_chunk, generate_small_island_surface_chunk,
    small_island_biome_id, small_island_surface_height,
};
pub use sampler::NoiseSampler;
pub use settings::{
    NoiseGeneratorSettings, NoiseModifier, NoiseSamplingSettings, NoiseSettings, NoiseSlideSettings,
};
pub use timing::{
    OverworldDependencyGenerationTiming, OverworldFeatureBatchTiming, SurfaceFillTiming,
};

#[cfg(test)]
mod tests;
pub use alpha::{
    ALPHA_ACTIVE_HEIGHT, ALPHA_BUILD_HEIGHT, ALPHA_SEA_LEVEL, AlphaFeatureBatchResult,
    AlphaFeatureDependencyCache, AlphaFeatureDependencyCacheReport, AlphaGenerationStage,
    alpha_semantic_block_id, alpha_semantic_bytes, generate_alpha_chunk,
    generate_alpha_stage_chunk,
};
