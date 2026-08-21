mod alpha;
mod beta;
mod chunk;
mod continental_candidate;
mod feature_batch;
mod generator;
mod mclone_overworld;
mod planning;
mod profile;
mod sampler;
mod settings;
mod surface_dependency_cache;
mod timing;
mod topology_probe;
mod vanilla_overworld_lod;

pub use crate::biome::{ConstantBiomeSource, NoiseBiome, NoiseBiomeSource};
pub use chunk::{GeneratedChunk, MutableChunkBlockBuffer, ScheduledTick};
pub use continental_candidate::{
    CONTINENTAL_CANDIDATE_EXACT_REVISION, ContinentalCandidateExactGenerator,
    ContinentalCandidateFeatureDependencyCache, ContinentalCandidateFeatureDependencyCacheReport,
    generate_continental_candidate_chunk, generate_continental_candidate_surface_chunk,
};
pub(crate) use continental_candidate::{
    continental_candidate_stratum, quantized_continental_candidate_surface_y,
};
pub use feature_batch::{
    OverworldFeatureBatchResult, OverworldFeatureDependencyCache,
    OverworldFeatureDependencyCacheReport, generate_overworld_features_chunk,
    generate_overworld_features_chunks, generate_overworld_surface_chunk,
};
pub use generator::NoiseBasedChunkGenerator;
pub(crate) use mclone_overworld::apply_stream_plans;
pub use mclone_overworld::{
    MCLONE_FOREST_EDGE_SAMPLE_RADIUS_BLOCKS, MCLONE_FOREST_SUMMARY_FOOTPRINT_TAPS,
    MCLONE_OVERWORLD_ALPINE_EXPOSED_STONE_MIN_SLOPE, MCLONE_OVERWORLD_ALPINE_MAX_TEMPERATURE,
    MCLONE_OVERWORLD_ALPINE_MIN_Y, MCLONE_OVERWORLD_CONIFER_MAX_TEMPERATURE,
    MCLONE_OVERWORLD_CONIFER_MIN_MOISTURE, MCLONE_OVERWORLD_DECORATION_REVISION,
    MCLONE_OVERWORLD_ERODED_SLOPE_MIN_STRENGTH, MCLONE_OVERWORLD_ERODED_SLOPE_MIN_Y,
    MCLONE_OVERWORLD_EXPOSED_STONE_MIN_STRENGTH, MCLONE_OVERWORLD_EXPOSED_STONE_MIN_Y,
    MCLONE_OVERWORLD_FIELD_REVISION, MCLONE_OVERWORLD_FOREST_BIOME_ID,
    MCLONE_OVERWORLD_GROVE_DOMAIN, MCLONE_OVERWORLD_GROVE_SCALE_BLOCKS,
    MCLONE_OVERWORLD_JUNGLE_BIOME_ID, MCLONE_OVERWORLD_LARGE_FIELD_SPEC,
    MCLONE_OVERWORLD_PERIOD_BLOCKS, MCLONE_OVERWORLD_PERIOD_CHUNKS,
    MCLONE_OVERWORLD_RIVER_BIOME_ID, MCLONE_OVERWORLD_SAVANNA_BIOME_ID, MCLONE_OVERWORLD_SEA_LEVEL,
    MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS, MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID,
    MCLONE_OVERWORLD_STEPPE_MAX_MOISTURE, MCLONE_OVERWORLD_STEPPE_MIN_TEMPERATURE,
    MCLONE_OVERWORLD_STEPPE_SHOULDER_MAX_MOISTURE,
    MCLONE_OVERWORLD_STEPPE_SHOULDER_MIN_SUITABILITY,
    MCLONE_OVERWORLD_STEPPE_SHOULDER_MIN_TEMPERATURE, MCLONE_OVERWORLD_STREAM_MAX_EXPANDED_NODES,
    MCLONE_OVERWORLD_STREAM_MAX_LENGTH_BLOCKS, MCLONE_OVERWORLD_STREAM_MIN_LENGTH_BLOCKS,
    MCLONE_OVERWORLD_STREAM_PLACEMENT_SEPARATION_CHUNKS,
    MCLONE_OVERWORLD_STREAM_PLACEMENT_SPACING_CHUNKS,
    MCLONE_OVERWORLD_STREAM_REFERENCE_RADIUS_CHUNKS, MCLONE_OVERWORLD_STREAM_ROUTE_STEP_BLOCKS,
    MCLONE_OVERWORLD_STREAM_STRUCTURE_TYPE, MCLONE_OVERWORLD_TAIGA_BIOME_ID,
    MCLONE_OVERWORLD_VEGETATION_REVISION, MCLONE_OVERWORLD_WOODED_MAX_EXPOSURE,
    MCLONE_OVERWORLD_WOODED_MAX_SLOPE, MCLONE_OVERWORLD_WOODED_UPLAND_MIN_Y,
    MCLONE_VEGETATION_CANDIDATES_PER_CELL, MCLONE_VEGETATION_PLANNING_CELL_BLOCKS,
    MCLONE_WILDLIFE_CANDIDATES_PER_CELL, MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS,
    MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS, MCLONE_WILDLIFE_POPULATION_REVISION,
    McloneForestDirection, McloneForestEdgeIntentSample, McloneForestIntentSample,
    McloneOverworldBathymetrySample, McloneOverworldBiomeDecision, McloneOverworldBiomeRecipe,
    McloneOverworldBiomeSelectionReason, McloneOverworldClimateSample, McloneOverworldCoastFamily,
    McloneOverworldCoastIntent, McloneOverworldDebugSample, McloneOverworldFeatureBatchResult,
    McloneOverworldFeatureDependencyCache, McloneOverworldFeatureDependencyCacheReport,
    McloneOverworldHydraulicClosureReport, McloneOverworldHydrologyKind,
    McloneOverworldLandformFamily, McloneOverworldLandformIntent, McloneOverworldLandformKind,
    McloneOverworldLandformSample, McloneOverworldLargeFieldBand, McloneOverworldLargeFieldSpec,
    McloneOverworldPreviewColumnProfile, McloneOverworldSampleRegion,
    McloneOverworldSampleRegionRequest, McloneOverworldSampler, McloneOverworldSamplingTopology,
    McloneOverworldSteppeBand, McloneOverworldStreamColumnSample, McloneOverworldStreamNode,
    McloneOverworldStreamPiece, McloneOverworldStreamPlan, McloneOverworldStreamPlanAttempt,
    McloneOverworldStreamPlanCache, McloneOverworldStreamPlanCacheReport,
    McloneOverworldStreamPlanMetrics, McloneOverworldStreamPlanner, McloneOverworldStreamRejection,
    McloneOverworldStreamTerrainIntent, McloneOverworldSurfaceRecipe, McloneOverworldSurveySampler,
    McloneOverworldTerrainSample, McloneOverworldVegetationPlanCache,
    McloneOverworldVegetationPlanner, McloneOverworldWatercourseSample,
    McloneOverworldWildlifePlanner, McloneTreeArchetype, McloneTreeBounds, McloneTreeFamily,
    McloneTreeId, McloneTreeOccurrence, McloneTreeRecord, McloneVegetationBounds,
    McloneVegetationError, McloneVegetationPlanCacheReport, McloneVegetationSource,
    McloneWildlifeCellPlan, McloneWildlifeEncounter, McloneWildlifeHabitatSample,
    McloneWildlifePlanError, McloneWildlifePopulationCell, McloneWildlifeSpecies,
    McloneWildlifeSuitability, analyze_mclone_overworld_hydraulic_closure,
    generate_mclone_overworld_chunk, generate_mclone_overworld_chunk_with_topology,
    generate_mclone_overworld_surface_chunk,
    generate_mclone_overworld_surface_chunk_with_stream_cache,
    generate_mclone_overworld_surface_chunk_with_topology,
    generate_mclone_overworld_surface_chunks_with_topology, mclone_overworld_biome_decision,
    mclone_overworld_biome_id, mclone_overworld_biome_id_for_sample,
    mclone_overworld_biome_id_with_topology, mclone_overworld_biome_recipe,
    mclone_overworld_debug_sample, mclone_overworld_debug_sample_with_streams,
    mclone_overworld_homestead_scout_origin_chunk_with_topology, mclone_overworld_landform_kind,
    mclone_overworld_macro_surface_top_material, mclone_overworld_preview_column_profile,
    mclone_overworld_preview_column_profile_wgsl, mclone_overworld_preview_visible_material,
    mclone_overworld_spawn_chunk, mclone_overworld_spawn_chunk_with_topology,
    mclone_overworld_steppe_band, mclone_overworld_steppe_suitability,
    mclone_overworld_surface_recipe, stream_placement, tree_records_intersecting,
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
pub use topology_probe::{
    TOPOLOGY_PROBE_BASE_SURFACE_Y, TOPOLOGY_PROBE_HEIGHT, TOPOLOGY_PROBE_MIN_PERIOD_CHUNKS,
    TOPOLOGY_PROBE_MIN_Y, TOPOLOGY_PROBE_PLAINS_BIOME_ID, TOPOLOGY_PROBE_SEA_LEVEL,
    TopologyProbeColumnSample, TopologyProbePlan, TopologyProbePlanKind, TopologyProbeSource,
    TopologyProbeWorkPlan, generate_topology_probe_chunk, generate_topology_probe_chunks,
    validate_topology_probe_topology,
};
pub use vanilla_overworld_lod::{
    VANILLA_OVERWORLD_LOD_MAX_RETAINED_DENSITY_COLUMNS, VANILLA_OVERWORLD_LOD_REVISION,
    VANILLA_OVERWORLD_MACRO_LOD_REVISION, VANILLA_OVERWORLD_MACRO_VERTICAL_CELL_STEP,
    VanillaOverworldLodSample, VanillaOverworldLodSampler, VanillaOverworldMacroSampler,
};

#[cfg(test)]
mod tests;
pub use alpha::{
    ALPHA_ACTIVE_HEIGHT, ALPHA_BUILD_HEIGHT, ALPHA_SEA_LEVEL, AlphaFeatureBatchResult,
    AlphaFeatureDependencyCache, AlphaFeatureDependencyCacheReport, AlphaGenerationStage,
    alpha_semantic_block_id, alpha_semantic_bytes, generate_alpha_chunk,
    generate_alpha_stage_chunk,
};
pub use beta::{
    BETA_ACTIVE_HEIGHT, BETA_BUILD_HEIGHT, BETA_SEA_LEVEL, BetaBiome, BetaClimateRegion,
    BetaFeatureBatchResult, BetaFeatureDependencyCache, BetaFeatureDependencyCacheReport,
    BetaGenerationStage, beta_biome_from_climate, beta_biome_id, beta_semantic_block_id,
    beta_semantic_bytes, generate_beta_chunk, generate_beta_climate_region,
    generate_beta_stage_chunk,
};
