mod biomes;
mod coast;
mod debug;
mod decoration;
mod feature_batch;
mod fields;
mod spawn;
mod streams;
mod surface;
mod terrain;
mod vegetation;
mod wildlife;

pub use biomes::{
    MCLONE_OVERWORLD_ALPINE_MAX_TEMPERATURE, MCLONE_OVERWORLD_ALPINE_MIN_Y,
    MCLONE_OVERWORLD_CONIFER_MAX_TEMPERATURE, MCLONE_OVERWORLD_CONIFER_MIN_MOISTURE,
    MCLONE_OVERWORLD_FOREST_BIOME_ID, MCLONE_OVERWORLD_RIVER_BIOME_ID,
    MCLONE_OVERWORLD_SAVANNA_BIOME_ID, MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID,
    MCLONE_OVERWORLD_STEPPE_MAX_MOISTURE, MCLONE_OVERWORLD_STEPPE_MIN_TEMPERATURE,
    MCLONE_OVERWORLD_STEPPE_SHOULDER_MAX_MOISTURE,
    MCLONE_OVERWORLD_STEPPE_SHOULDER_MIN_SUITABILITY,
    MCLONE_OVERWORLD_STEPPE_SHOULDER_MIN_TEMPERATURE, MCLONE_OVERWORLD_TAIGA_BIOME_ID,
    MCLONE_OVERWORLD_WOODED_MAX_EXPOSURE, MCLONE_OVERWORLD_WOODED_MAX_SLOPE,
    MCLONE_OVERWORLD_WOODED_UPLAND_MIN_Y, McloneOverworldBiomeDecision, McloneOverworldBiomeRecipe,
    McloneOverworldBiomeSelectionReason, McloneOverworldSteppeBand,
    mclone_overworld_biome_decision, mclone_overworld_biome_id,
    mclone_overworld_biome_id_for_sample, mclone_overworld_biome_id_with_topology,
    mclone_overworld_biome_recipe, mclone_overworld_steppe_band,
    mclone_overworld_steppe_suitability,
};
pub use coast::{McloneOverworldCoastFamily, McloneOverworldCoastIntent};
pub use debug::{
    McloneOverworldDebugSample, McloneOverworldHydrologyKind, McloneOverworldLandformKind,
    mclone_overworld_debug_sample, mclone_overworld_debug_sample_with_streams,
    mclone_overworld_landform_kind,
};
pub use decoration::MCLONE_OVERWORLD_DECORATION_REVISION;
pub use feature_batch::{
    McloneOverworldFeatureBatchResult, McloneOverworldFeatureDependencyCache,
    McloneOverworldFeatureDependencyCacheReport, generate_mclone_overworld_chunk,
    generate_mclone_overworld_chunk_with_topology,
};
pub use fields::{
    MCLONE_OVERWORLD_FIELD_REVISION, MCLONE_OVERWORLD_LARGE_FIELD_SPEC,
    MCLONE_OVERWORLD_PERIOD_BLOCKS, MCLONE_OVERWORLD_PERIOD_CHUNKS, MCLONE_OVERWORLD_SEA_LEVEL,
    MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS, McloneOverworldBathymetrySample,
    McloneOverworldClimateSample, McloneOverworldLandformFamily, McloneOverworldLandformIntent,
    McloneOverworldLandformSample, McloneOverworldLargeFieldBand, McloneOverworldLargeFieldSpec,
    McloneOverworldSampleRegion, McloneOverworldSampleRegionRequest, McloneOverworldSampler,
    McloneOverworldSamplingTopology, McloneOverworldTerrainSample,
    McloneOverworldWatercourseSample,
};
pub use spawn::{
    mclone_overworld_homestead_scout_origin_chunk_with_topology, mclone_overworld_spawn_chunk,
    mclone_overworld_spawn_chunk_with_topology,
};
pub use streams::{
    MCLONE_OVERWORLD_STREAM_MAX_EXPANDED_NODES, MCLONE_OVERWORLD_STREAM_MAX_LENGTH_BLOCKS,
    MCLONE_OVERWORLD_STREAM_MIN_LENGTH_BLOCKS, MCLONE_OVERWORLD_STREAM_PLACEMENT_SEPARATION_CHUNKS,
    MCLONE_OVERWORLD_STREAM_PLACEMENT_SPACING_CHUNKS,
    MCLONE_OVERWORLD_STREAM_REFERENCE_RADIUS_CHUNKS, MCLONE_OVERWORLD_STREAM_ROUTE_STEP_BLOCKS,
    MCLONE_OVERWORLD_STREAM_STRUCTURE_TYPE, McloneOverworldStreamColumnSample,
    McloneOverworldStreamNode, McloneOverworldStreamPiece, McloneOverworldStreamPlan,
    McloneOverworldStreamPlanAttempt, McloneOverworldStreamPlanCache,
    McloneOverworldStreamPlanCacheReport, McloneOverworldStreamPlanMetrics,
    McloneOverworldStreamPlanner, McloneOverworldStreamRejection,
    McloneOverworldStreamTerrainIntent, stream_placement,
};
pub use surface::{
    MCLONE_OVERWORLD_ALPINE_EXPOSED_STONE_MIN_SLOPE, MCLONE_OVERWORLD_ERODED_SLOPE_MIN_STRENGTH,
    MCLONE_OVERWORLD_ERODED_SLOPE_MIN_Y, MCLONE_OVERWORLD_EXPOSED_STONE_MIN_STRENGTH,
    MCLONE_OVERWORLD_EXPOSED_STONE_MIN_Y, McloneOverworldSurfaceRecipe,
    mclone_overworld_macro_surface_top_material, mclone_overworld_preview_visible_material,
    mclone_overworld_surface_recipe,
};
pub(crate) use terrain::apply_stream_plans;
pub use terrain::{
    McloneOverworldHydraulicClosureReport, McloneOverworldSurveySampler,
    analyze_mclone_overworld_hydraulic_closure, generate_mclone_overworld_surface_chunk,
    generate_mclone_overworld_surface_chunk_with_stream_cache,
    generate_mclone_overworld_surface_chunk_with_topology,
    generate_mclone_overworld_surface_chunks_with_topology,
};
pub use vegetation::{
    MCLONE_FOREST_EDGE_SAMPLE_RADIUS_BLOCKS, MCLONE_FOREST_SUMMARY_FOOTPRINT_TAPS,
    MCLONE_OVERWORLD_GROVE_DOMAIN, MCLONE_OVERWORLD_GROVE_SCALE_BLOCKS,
    MCLONE_OVERWORLD_VEGETATION_REVISION, MCLONE_VEGETATION_CANDIDATES_PER_CELL,
    MCLONE_VEGETATION_PLANNING_CELL_BLOCKS, McloneForestDirection, McloneForestEdgeIntentSample,
    McloneForestIntentSample, McloneOverworldVegetationPlanCache, McloneOverworldVegetationPlanner,
    McloneTreeArchetype, McloneTreeBounds, McloneTreeFamily, McloneTreeId, McloneTreeOccurrence,
    McloneTreeRecord, McloneVegetationBounds, McloneVegetationError,
    McloneVegetationPlanCacheReport, McloneVegetationSource, tree_records_intersecting,
};
pub use wildlife::{
    MCLONE_WILDLIFE_CANDIDATES_PER_CELL, MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS,
    MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS, MCLONE_WILDLIFE_POPULATION_REVISION,
    McloneOverworldWildlifePlanner, McloneWildlifeCellPlan, McloneWildlifeEncounter,
    McloneWildlifeHabitatSample, McloneWildlifePlanError, McloneWildlifePopulationCell,
    McloneWildlifeSpecies, McloneWildlifeSuitability,
};
