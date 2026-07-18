mod chunk;
mod feature_batch;
mod generator;
mod planning;
mod profile;
mod sampler;
mod settings;
mod timing;

pub use crate::biome::{ConstantBiomeSource, NoiseBiome, NoiseBiomeSource};
pub use chunk::{GeneratedChunk, MutableChunkBlockBuffer, ScheduledTick};
pub use feature_batch::{
    OverworldFeatureBatchResult, OverworldFeatureDependencyCache,
    OverworldFeatureDependencyCacheReport, generate_overworld_features_chunk,
    generate_overworld_features_chunks, generate_overworld_surface_chunk,
};
pub use generator::NoiseBasedChunkGenerator;
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
