#![recursion_limit = "256"]

use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use image::RgbaImage;
use mclone_core::ChunkPos;
use mclone_worldgen::levelgen::{
    BEACH_BIOME_ID, MCLONE_OVERWORLD_DECORATION_REVISION, MCLONE_OVERWORLD_FIELD_REVISION,
    MCLONE_OVERWORLD_FOREST_BIOME_ID, MCLONE_OVERWORLD_PERIOD_BLOCKS,
    MCLONE_OVERWORLD_RIVER_BIOME_ID, MCLONE_OVERWORLD_SAVANNA_BIOME_ID, MCLONE_OVERWORLD_SEA_LEVEL,
    MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID, MCLONE_OVERWORLD_STREAM_REFERENCE_RADIUS_CHUNKS,
    MCLONE_OVERWORLD_TAIGA_BIOME_ID, McloneOverworldBiomeRecipe, McloneOverworldLandformSample,
    McloneOverworldSampleRegion, McloneOverworldSampleRegionRequest, McloneOverworldSampler,
    McloneOverworldSamplingTopology, McloneOverworldSteppeBand, McloneOverworldStreamPlan,
    McloneOverworldStreamPlanAttempt, McloneOverworldStreamPlanner, McloneOverworldStreamRejection,
    McloneOverworldSurfaceRecipe, McloneOverworldTerrainSample, OCEAN_BIOME_ID, PLAINS_BIOME_ID,
    analyze_mclone_overworld_hydraulic_closure, mclone_overworld_biome_id_for_sample,
    mclone_overworld_biome_recipe, mclone_overworld_spawn_chunk,
    mclone_overworld_spawn_chunk_with_topology, mclone_overworld_steppe_band,
    mclone_overworld_steppe_suitability, mclone_overworld_surface_recipe,
};

const DEFAULT_OUTPUT_DIR: &str = "/tmp/mclone-overworld-review";
const DEFAULT_SEED: i64 = 12_345;
const DEFAULT_RADIUS_BLOCKS: u32 = 3_072;
const DEFAULT_STEP_BLOCKS: u32 = 16;
const MAX_RADIUS_BLOCKS: u32 = 32_768;
const MAP_GAP_PIXELS: u32 = 8;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let config = Config::parse(std::env::args().skip(1))?;
    fs::create_dir_all(&config.output_dir).with_context(|| {
        format!(
            "create mclone overworld review directory {}",
            config.output_dir.display()
        )
    })?;

    let center_x = config
        .chunk_x
        .checked_mul(16)
        .and_then(|value| value.checked_add(8))
        .context("review center x overflow")?;
    let center_z = config
        .chunk_z
        .checked_mul(16)
        .and_then(|value| value.checked_add(8))
        .context("review center z overflow")?;
    let radius = i32::try_from(config.radius_blocks).context("review radius exceeds i32")?;
    let diameter = config
        .radius_blocks
        .checked_mul(2)
        .context("review diameter overflow")?;
    let width = diameter / config.step_blocks + 1;
    let request = McloneOverworldSampleRegionRequest {
        min_x: center_x
            .checked_sub(radius)
            .context("review minimum x overflow")?,
        min_z: center_z
            .checked_sub(radius)
            .context("review minimum z overflow")?,
        width,
        depth: width,
        step: config.step_blocks,
    };

    let sampler = McloneOverworldSampler::new_with_topology(config.seed, config.topology);
    let sample_start = Instant::now();
    let region = sampler.sample_region(request).map_err(anyhow::Error::msg)?;
    let sample_elapsed_ms = sample_start.elapsed().as_secs_f64() * 1_000.0;
    let landform_start = Instant::now();
    let landforms = (0..request.depth)
        .flat_map(|offset_z| {
            (0..request.width).map(move |offset_x| {
                sampler.sample_landform(
                    request.min_x + offset_x as i32 * request.step as i32,
                    request.min_z + offset_z as i32 * request.step as i32,
                )
            })
        })
        .collect::<Vec<_>>();
    let landform_elapsed_ms = landform_start.elapsed().as_secs_f64() * 1_000.0;
    let facts = RegionFacts::from_samples(&landforms, request.width, request.depth);
    let steppe_components = recipe_component_stats(
        &landforms,
        request.width,
        request.depth,
        request.step,
        McloneOverworldBiomeRecipe::WarmDrySteppe,
    );
    let review_sites = select_review_sites(&landforms, request, config.topology);
    let center_sample = sampler.sample_landform(center_x, center_z);
    let spawn_chunk = match config.topology {
        McloneOverworldSamplingTopology::Unbounded => mclone_overworld_spawn_chunk(config.seed),
        McloneOverworldSamplingTopology::PeriodicX => {
            mclone_overworld_spawn_chunk_with_topology(config.seed, config.topology)
        }
    };
    let spawn_x = spawn_chunk.min_block_x() + 8;
    let spawn_z = spawn_chunk.min_block_z() + 8;
    let spawn_sample = sampler.sample_landform(spawn_x, spawn_z);
    let hydraulic_start = Instant::now();
    let hydraulic = analyze_mclone_overworld_hydraulic_closure(
        config.seed,
        config.topology,
        ChunkPos::new(config.chunk_x, config.chunk_z),
        1,
    );
    let hydraulic_elapsed_ms = hydraulic_start.elapsed().as_secs_f64() * 1_000.0;
    let stream_planning_start = Instant::now();
    let stream_planner = McloneOverworldStreamPlanner::new(config.seed, config.topology);
    let min_review_chunk =
        ChunkPos::new(request.min_x.div_euclid(16), request.min_z.div_euclid(16));
    let max_review_x = request.min_x + (request.width as i32 - 1) * request.step as i32;
    let max_review_z = request.min_z + (request.depth as i32 - 1) * request.step as i32;
    let max_review_chunk = ChunkPos::new(max_review_x.div_euclid(16), max_review_z.div_euclid(16));
    let stream_attempts = stream_planner
        .plan_candidates_in_chunks(min_review_chunk, max_review_chunk)
        .map_err(anyhow::Error::msg)?;
    let stream_plans = stream_planner
        .plans_intersecting_chunks(min_review_chunk, max_review_chunk)
        .map_err(anyhow::Error::msg)?;
    let stream_planning_elapsed_ms = stream_planning_start.elapsed().as_secs_f64() * 1_000.0;

    let prefix = format!(
        "mclone-overworld-v1-{}-seed-{}-chunk-{}-{}",
        config.topology.label(),
        config.seed,
        config.chunk_x,
        config.chunk_z
    );
    let continentalness_path = config
        .output_dir
        .join(format!("{prefix}-continentalness.png"));
    let relief_path = config.output_dir.join(format!("{prefix}-relief.png"));
    let ruggedness_path = config.output_dir.join(format!("{prefix}-ruggedness.png"));
    let ridges_path = config.output_dir.join(format!("{prefix}-ridges.png"));
    let mountain_detail_path = config
        .output_dir
        .join(format!("{prefix}-mountain-detail.png"));
    let surface_path = config.output_dir.join(format!("{prefix}-surface-y.png"));
    let base_surface_path = config
        .output_dir
        .join(format!("{prefix}-base-surface-y.png"));
    let river_distance_path = config
        .output_dir
        .join(format!("{prefix}-river-distance.png"));
    let river_level_path = config.output_dir.join(format!("{prefix}-river-level.png"));
    let river_grade_path = config.output_dir.join(format!("{prefix}-river-grade.png"));
    let river_transition_path = config
        .output_dir
        .join(format!("{prefix}-river-transitions.png"));
    let wetland_path = config.output_dir.join(format!("{prefix}-wetland.png"));
    let watercourses_path = config.output_dir.join(format!("{prefix}-watercourses.png"));
    let stream_plans_path = config.output_dir.join(format!("{prefix}-stream-plans.png"));
    let stream_candidates_path = config
        .output_dir
        .join(format!("{prefix}-stream-candidates.png"));
    let stream_costs_path = config
        .output_dir
        .join(format!("{prefix}-stream-plan-costs.png"));
    let water_depth_path = config.output_dir.join(format!("{prefix}-water-depth.png"));
    let shelf_break_path = config.output_dir.join(format!("{prefix}-shelf-break.png"));
    let ocean_basin_path = config.output_dir.join(format!("{prefix}-ocean-basin.png"));
    let seabed_relief_path = config
        .output_dir
        .join(format!("{prefix}-seabed-relief.png"));
    let bathymetry_path = config.output_dir.join(format!("{prefix}-bathymetry.png"));
    let slope_path = config.output_dir.join(format!("{prefix}-slope.png"));
    let temperature_path = config.output_dir.join(format!("{prefix}-temperature.png"));
    let moisture_path = config.output_dir.join(format!("{prefix}-moisture.png"));
    let adjusted_temperature_path = config
        .output_dir
        .join(format!("{prefix}-adjusted-temperature.png"));
    let steppe_suitability_path = config
        .output_dir
        .join(format!("{prefix}-steppe-suitability.png"));
    let recipe_path = config
        .output_dir
        .join(format!("{prefix}-climate-recipes.png"));
    let climate_path = config.output_dir.join(format!("{prefix}-climate.png"));
    let fields_path = config.output_dir.join(format!("{prefix}-fields.png"));
    let biome_path = config.output_dir.join(format!("{prefix}-biomes.png"));
    let surface_recipe_path = config
        .output_dir
        .join(format!("{prefix}-surface-recipes.png"));
    let language_path = config
        .output_dir
        .join(format!("{prefix}-terrain-language.png"));
    let receipt_path = config.output_dir.join(format!("{prefix}-fields.json"));

    let continentalness = render_map(&region.samples, continentalness_color);
    let relief = render_map(&region.samples, relief_color);
    let ruggedness = render_map(&region.samples, ruggedness_color);
    let ridges = render_map(&region.samples, ridges_color);
    let mountain_detail = render_map(&region.samples, mountain_detail_color);
    let surface = render_map(&region.samples, surface_color);
    let base_surface = render_map(&region.samples, base_surface_color);
    let river_distance = render_map(&region.samples, river_distance_color);
    let river_level = render_map(&region.samples, river_level_color);
    let river_grade = render_map(&region.samples, river_grade_color);
    let river_transitions = render_map(&region.samples, river_transition_color);
    let wetland = render_map(&region.samples, wetland_color);
    let stream_plan_map = render_stream_plan_map(&region, &stream_plans);
    let stream_candidate_map =
        render_stream_candidate_map(&region, &stream_attempts, &stream_plans);
    let stream_cost_map = render_stream_plan_cost_map(&region, &stream_plans);
    let water_depth = render_map(&region.samples, water_depth_color);
    let shelf_break = render_map(&region.samples, shelf_break_color);
    let ocean_basin = render_map(&region.samples, ocean_basin_color);
    let seabed_relief = render_map(&region.samples, seabed_relief_color);
    let slope = render_landform_map(&landforms, slope_color);
    let temperature = render_map(&region.samples, temperature_color);
    let moisture = render_map(&region.samples, moisture_color);
    let adjusted_temperature = render_landform_map(&landforms, adjusted_temperature_color);
    let steppe_suitability = render_map(&region.samples, steppe_suitability_color);
    let recipes = render_landform_map(&landforms, biome_recipe_color);
    let biomes = render_landform_map(&landforms, biome_color);
    let surface_recipes = render_landform_map(&landforms, surface_recipe_color);
    save_rgba(
        &continentalness_path,
        request.width,
        request.depth,
        &continentalness,
    )?;
    save_rgba(&relief_path, request.width, request.depth, &relief)?;
    save_rgba(&ruggedness_path, request.width, request.depth, &ruggedness)?;
    save_rgba(&ridges_path, request.width, request.depth, &ridges)?;
    save_rgba(
        &mountain_detail_path,
        request.width,
        request.depth,
        &mountain_detail,
    )?;
    save_rgba(&surface_path, request.width, request.depth, &surface)?;
    save_rgba(
        &base_surface_path,
        request.width,
        request.depth,
        &base_surface,
    )?;
    save_rgba(
        &river_distance_path,
        request.width,
        request.depth,
        &river_distance,
    )?;
    save_rgba(
        &river_level_path,
        request.width,
        request.depth,
        &river_level,
    )?;
    save_rgba(
        &river_grade_path,
        request.width,
        request.depth,
        &river_grade,
    )?;
    save_rgba(
        &river_transition_path,
        request.width,
        request.depth,
        &river_transitions,
    )?;
    save_rgba(&wetland_path, request.width, request.depth, &wetland)?;
    save_rgba(
        &stream_plans_path,
        request.width,
        request.depth,
        &stream_plan_map,
    )?;
    save_rgba(
        &stream_candidates_path,
        request.width,
        request.depth,
        &stream_candidate_map,
    )?;
    save_rgba(
        &stream_costs_path,
        request.width,
        request.depth,
        &stream_cost_map,
    )?;
    save_rgba(
        &water_depth_path,
        request.width,
        request.depth,
        &water_depth,
    )?;
    save_rgba(
        &shelf_break_path,
        request.width,
        request.depth,
        &shelf_break,
    )?;
    save_rgba(
        &ocean_basin_path,
        request.width,
        request.depth,
        &ocean_basin,
    )?;
    save_rgba(
        &seabed_relief_path,
        request.width,
        request.depth,
        &seabed_relief,
    )?;
    let bathymetry = combine_maps(
        request.width,
        request.depth,
        [&water_depth, &shelf_break, &ocean_basin, &seabed_relief],
    );
    save_rgba(
        &bathymetry_path,
        request.width * 4 + MAP_GAP_PIXELS * 3,
        request.depth,
        &bathymetry,
    )?;
    let watercourses = combine_maps(
        request.width,
        request.depth,
        [
            &river_distance,
            &river_level,
            &river_transitions,
            &river_grade,
            &wetland,
        ],
    );
    save_rgba(
        &watercourses_path,
        request.width * 5 + MAP_GAP_PIXELS * 4,
        request.depth,
        &watercourses,
    )?;
    save_rgba(&slope_path, request.width, request.depth, &slope)?;
    save_rgba(
        &temperature_path,
        request.width,
        request.depth,
        &temperature,
    )?;
    save_rgba(&moisture_path, request.width, request.depth, &moisture)?;
    save_rgba(
        &adjusted_temperature_path,
        request.width,
        request.depth,
        &adjusted_temperature,
    )?;
    save_rgba(
        &steppe_suitability_path,
        request.width,
        request.depth,
        &steppe_suitability,
    )?;
    save_rgba(&recipe_path, request.width, request.depth, &recipes)?;
    let climate = combine_maps(
        request.width,
        request.depth,
        [
            &temperature,
            &moisture,
            &adjusted_temperature,
            &steppe_suitability,
            &recipes,
        ],
    );
    save_rgba(
        &climate_path,
        request.width * 5 + MAP_GAP_PIXELS * 4,
        request.depth,
        &climate,
    )?;
    let combined = combine_maps(
        request.width,
        request.depth,
        [
            &continentalness,
            &relief,
            &ruggedness,
            &ridges,
            &mountain_detail,
            &surface,
        ],
    );
    save_rgba(
        &fields_path,
        request.width * 6 + MAP_GAP_PIXELS * 5,
        request.depth,
        &combined,
    )?;
    save_rgba(&biome_path, request.width, request.depth, &biomes)?;
    save_rgba(
        &surface_recipe_path,
        request.width,
        request.depth,
        &surface_recipes,
    )?;
    let terrain_language = combine_maps(
        request.width,
        request.depth,
        [&slope, &biomes, &surface_recipes],
    );
    save_rgba(
        &language_path,
        request.width * 3 + MAP_GAP_PIXELS * 2,
        request.depth,
        &terrain_language,
    )?;

    let (commit, dirty) = git_state();
    let receipt = serde_json::json!({
        "schema": 13,
        "profile": "mclone-overworld-v1",
        "topology": config.topology.label(),
        "fieldRevision": MCLONE_OVERWORLD_FIELD_REVISION,
        "decorationRevision": MCLONE_OVERWORLD_DECORATION_REVISION,
        "commit": commit,
        "dirty": dirty,
        "seed": config.seed,
        "centerChunk": [config.chunk_x, config.chunk_z],
        "centerBlock": [center_x, center_z],
        "centerSample": sample_json(center_sample),
        "spawn": {
            "chunk": [spawn_chunk.x, spawn_chunk.z],
            "block": [spawn_x, spawn_z],
            "sample": sample_json(spawn_sample),
        },
        "boundsBlocks": {
            "min": [request.min_x, request.min_z],
            "max": [
                request.min_x + (request.width as i32 - 1) * request.step as i32,
                request.min_z + (request.depth as i32 - 1) * request.step as i32,
            ],
        },
        "sampleGrid": {
            "width": request.width,
            "depth": request.depth,
            "stepBlocks": request.step,
            "count": region.samples.len(),
            "sampleElapsedMs": sample_elapsed_ms,
            "landformSampleElapsedMs": landform_elapsed_ms,
        },
        "ranges": {
            "continentalness": [facts.min_continentalness, facts.max_continentalness],
            "relief": [facts.min_relief, facts.max_relief],
            "ruggedness": [facts.min_ruggedness, facts.max_ruggedness],
            "ridges": [facts.min_ridges, facts.max_ridges],
            "mountainDetail": [facts.min_mountain_detail, facts.max_mountain_detail],
            "temperature": [facts.min_temperature, facts.max_temperature],
            "moisture": [facts.min_moisture, facts.max_moisture],
            "altitudeAdjustedTemperature": [
                facts.min_adjusted_temperature,
                facts.max_adjusted_temperature,
            ],
            "warmDrySteppeSuitability": [
                facts.min_steppe_suitability,
                facts.max_steppe_suitability,
            ],
            "oceanInterior": [facts.min_ocean_interior, facts.max_ocean_interior],
            "shelfBreakInfluence": [facts.min_shelf_break, facts.max_shelf_break],
            "oceanBasinInfluence": [facts.min_ocean_basin, facts.max_ocean_basin],
            "seabedRelief": [facts.min_seabed_relief, facts.max_seabed_relief],
            "waterDepth": [facts.min_water_depth, facts.max_water_depth],
            "riverDistance": [facts.min_river_distance, facts.max_river_distance],
            "riverHalfWidth": [facts.min_river_half_width, facts.max_river_half_width],
            "riverGrade": [facts.min_river_grade, facts.max_river_grade],
            "riverWaterSurfaceY": [facts.min_river_water_y, facts.max_river_water_y],
            "wetlandInfluence": [facts.min_wetland_influence, facts.max_wetland_influence],
            "wetlandPoolInfluence": [facts.min_wetland_pool_influence, facts.max_wetland_pool_influence],
            "baseSurfaceY": [facts.min_base_surface_y, facts.max_base_surface_y],
            "surfaceY": [facts.min_surface_y, facts.max_surface_y],
            "slope": [facts.min_slope, facts.max_slope],
            "exposure": [facts.min_exposure, facts.max_exposure],
        },
        "surfaceYPercentiles": {
            "p10": facts.surface_y_p10,
            "p50": facts.surface_y_p50,
            "p90": facts.surface_y_p90,
        },
        "oceanWaterDepthPercentiles": {
            "p10": facts.water_depth_p10,
            "p50": facts.water_depth_p50,
            "p90": facts.water_depth_p90,
        },
        "slopePercentiles": {
            "p50": facts.slope_p50,
            "p90": facts.slope_p90,
            "p99": facts.slope_p99,
        },
        "columnCounts": {
            "water": facts.water_columns,
            "shore": facts.shore_columns,
            "dryLand": facts.dry_land_columns,
        },
        "biomeCounts": {
            "ocean": facts.ocean_biome_columns,
            "beach": facts.beach_biome_columns,
            "openLowland": facts.open_lowland_biome_columns,
            "woodedUpland": facts.wooded_upland_biome_columns,
            "river": facts.river_biome_columns,
            "taiga": facts.taiga_biome_columns,
            "snowyMountains": facts.snowy_mountains_biome_columns,
            "savanna": facts.savanna_biome_columns,
        },
        "climateRecipeCounts": {
            "ocean": facts.ocean_recipe_columns,
            "shore": facts.shore_recipe_columns,
            "river": facts.river_recipe_columns,
            "snowyAlpine": facts.snowy_alpine_recipe_columns,
            "coolWetConifer": facts.cool_wet_conifer_recipe_columns,
            "warmDrySteppe": facts.warm_dry_steppe_recipe_columns,
            "warmDrySteppeCore": facts.warm_dry_steppe_core_columns,
            "warmDrySteppeShoulder": facts.warm_dry_steppe_shoulder_columns,
            "temperateWoodland": facts.temperate_woodland_recipe_columns,
            "temperateMeadow": facts.temperate_meadow_recipe_columns,
        },
        "regionalComponents": {
            "warmDrySteppe": steppe_components,
        },
        "surfaceRecipeCounts": {
            "oceanFloor": facts.ocean_floor_columns,
            "beach": facts.beach_surface_columns,
            "riverBed": facts.river_bed_columns,
            "wetlandBed": facts.wetland_bed_columns,
            "riverBank": facts.river_bank_columns,
            "grassSoil": facts.grass_soil_columns,
            "alpineSnow": facts.alpine_snow_columns,
            "exposedStone": facts.exposed_stone_columns,
        },
        "watercourseCounts": {
            "channel": facts.river_channel_columns,
            "majorChannel": facts.major_river_channel_columns,
            "plannedStream": facts.planned_stream_columns,
            "streamHeadwater": facts.stream_headwater_columns,
            "gradedBank": facts.river_bank_influence_columns,
            "distinctReachLevels": facts.reach_level_count,
            "dropTransition": facts.drop_transition_columns,
            "fall": facts.fall_columns,
            "wetlandAboveQuarter": facts.wetland_columns,
            "wetlandPool": facts.wetland_pool_columns,
        },
        "streamPlanning": stream_planning_json(
            &stream_attempts,
            &stream_plans,
            stream_planning_elapsed_ms,
        ),
        "bathymetryCounts": {
            "shelf": facts.shelf_columns,
            "shelfBreak": facts.shelf_break_columns,
            "deepBasin": facts.deep_basin_columns,
        },
        "hydraulicClosure": {
            "radiusChunks": 1,
            "elapsedMs": hydraulic_elapsed_ms,
            "targetChunks": hydraulic.target_chunks,
            "sourceWaterBlocks": hydraulic.source_water_blocks,
            "flowingWaterBlocks": hydraulic.flowing_water_blocks,
            "sourceBoundaryBlocks": hydraulic.source_boundary_blocks,
            "horizontallyOpenSourceFaces": hydraulic.horizontally_open_source_faces,
            "firstOpenSource": hydraulic.first_open_source,
            "unsupportedSourceBlocks": hydraulic.unsupported_source_blocks,
            "unsupportedFlowingBlocks": hydraulic.unsupported_flowing_blocks,
            "slopedSurfaceEdges": hydraulic.sloped_surface_edges,
            "intentionalDropEdges": hydraulic.intentional_drop_edges,
            "scheduledLiquidTicks": hydraulic.scheduled_liquid_ticks,
            "closed": hydraulic.is_closed(),
        },
        "landformCounts": {
            "mountainRegion": facts.mountain_region_columns,
            "mountainValley": facts.mountain_valley_columns,
            "mountainCrest": facts.mountain_crest_columns,
            "highland": facts.highland_columns,
            "summit": facts.summit_columns,
        },
        "reviewSites": review_sites,
        "slopeEdges": {
            "total": facts.slope_edges,
            "atLeastOneBlock": facts.slope_at_least_one,
            "atLeastThreeBlocks": facts.slope_at_least_three,
        },
        "sampleFingerprint": facts.fingerprint,
        "foundationFieldFingerprint": facts.foundation_field_fingerprint,
        "climateFingerprint": facts.climate_fingerprint,
        "terrainLanguageFingerprint": facts.terrain_language_fingerprint,
        "maps": {
            "order": ["continentalness", "relief", "ruggedness", "ridges", "mountainDetail", "surfaceY"],
            "combined": fields_path,
            "continentalness": continentalness_path,
            "relief": relief_path,
            "ruggedness": ruggedness_path,
            "ridges": ridges_path,
            "mountainDetail": mountain_detail_path,
            "surfaceY": surface_path,
            "baseSurfaceY": base_surface_path,
            "climateOrder": [
                "temperature",
                "moisture",
                "altitudeAdjustedTemperature",
                "warmDrySteppeSuitability",
                "recipe",
            ],
            "climate": climate_path,
            "temperature": temperature_path,
            "moisture": moisture_path,
            "altitudeAdjustedTemperature": adjusted_temperature_path,
            "warmDrySteppeSuitability": steppe_suitability_path,
            "climateRecipes": recipe_path,
            "bathymetryOrder": ["waterDepth", "shelfBreak", "oceanBasin", "seabedRelief"],
            "bathymetry": bathymetry_path,
            "waterDepth": water_depth_path,
            "shelfBreak": shelf_break_path,
            "oceanBasin": ocean_basin_path,
            "seabedRelief": seabed_relief_path,
            "watercourseOrder": ["distanceAndInfluence", "waterLevel", "transitions", "grade", "wetland"],
            "watercourses": watercourses_path,
            "riverDistance": river_distance_path,
            "riverLevel": river_level_path,
            "riverTransitions": river_transition_path,
            "riverGrade": river_grade_path,
            "wetland": wetland_path,
            "streamPlans": stream_plans_path,
            "streamCandidates": stream_candidates_path,
            "streamPlanCosts": stream_costs_path,
            "terrainLanguageOrder": ["slope", "biomes", "surfaceRecipes"],
            "terrainLanguage": language_path,
            "slope": slope_path,
            "biomes": biome_path,
            "surfaceRecipes": surface_recipe_path,
        },
    });
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)
        .with_context(|| format!("write review receipt {}", receipt_path.display()))?;

    println!(
        "mclone overworld fields saved to {} (seed={}, center=({}, {}), grid={}x{}, step={}, receipt={})",
        fields_path.display(),
        config.seed,
        config.chunk_x,
        config.chunk_z,
        request.width,
        request.depth,
        request.step,
        receipt_path.display(),
    );
    Ok(())
}

fn select_review_sites(
    samples: &[McloneOverworldLandformSample],
    request: McloneOverworldSampleRegionRequest,
    topology: McloneOverworldSamplingTopology,
) -> serde_json::Value {
    let highest = samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| sample.terrain.mountain_strength() >= 0.35)
        .max_by_key(|(_, sample)| sample.terrain.surface_y)
        .map(|(index, _)| index);
    let mountain_valley = samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| {
            sample.terrain.surface_y > MCLONE_OVERWORLD_SEA_LEVEL
                && sample.terrain.mountain_strength() >= 0.45
                && sample.terrain.ridges <= 0.25
        })
        .max_by(|(_, left), (_, right)| {
            left.terrain
                .mountain_strength()
                .total_cmp(&right.terrain.mountain_strength())
        })
        .map(|(index, _)| index);
    let range_edge = samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| {
            sample.terrain.surface_y > MCLONE_OVERWORLD_SEA_LEVEL
                && (0.15..=0.35).contains(&sample.terrain.mountain_strength())
                && sample.terrain.ridges >= 0.70
        })
        .max_by_key(|(_, sample)| sample.terrain.surface_y)
        .map(|(index, _)| index);
    let center_x = request.width as i64 / 2;
    let center_z = request.depth as i64 / 2;
    let lowland_control = samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| {
            (67..=78).contains(&sample.terrain.surface_y)
                && sample.terrain.mountain_strength() <= 0.05
        })
        .min_by_key(|(index, _)| {
            let x = *index as i64 % i64::from(request.width);
            let z = *index as i64 / i64::from(request.width);
            (x - center_x).abs() + (z - center_z).abs()
        })
        .map(|(index, _)| index);
    let deep_ocean_basin = samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| sample.terrain.continentalness <= 0.0)
        .max_by_key(|(_, sample)| sample.terrain.bathymetry.water_depth)
        .map(|(index, _)| index);
    let shelf_break = samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| sample.terrain.continentalness <= 0.0)
        .max_by(|(_, left), (_, right)| {
            left.terrain
                .bathymetry
                .shelf_break_influence
                .total_cmp(&right.terrain.bathymetry.shelf_break_influence)
        })
        .map(|(index, _)| index);
    let river = samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| sample.terrain.watercourse.is_channel())
        .min_by(|(_, left), (_, right)| {
            left.terrain
                .watercourse
                .distance
                .total_cmp(&right.terrain.watercourse.distance)
        })
        .map(|(index, _)| index);
    let mountain_river = samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| sample.terrain.watercourse.is_channel())
        .max_by(|(_, left), (_, right)| {
            left.terrain
                .mountain_strength()
                .total_cmp(&right.terrain.mountain_strength())
        })
        .map(|(index, _)| index);
    let coastal_river = samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| sample.terrain.watercourse.is_channel())
        .min_by(|(_, left), (_, right)| {
            left.terrain
                .continentalness
                .total_cmp(&right.terrain.continentalness)
        })
        .map(|(index, _)| index);
    let waterfall = samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| {
            sample.terrain.watercourse.is_channel()
                && sample.terrain.watercourse.is_drop_transition()
        })
        .min_by(|(_, left), (_, right)| {
            left.terrain
                .watercourse
                .drop_distance
                .abs()
                .total_cmp(&right.terrain.watercourse.drop_distance.abs())
        })
        .map(|(index, _)| index);
    let wetland = samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| sample.terrain.watercourse.bank_influence > 0.0)
        .max_by(|(_, left), (_, right)| {
            left.terrain
                .watercourse
                .wetland_influence
                .total_cmp(&right.terrain.watercourse.wetland_influence)
        })
        .map(|(index, _)| index);
    let wetland_pool = samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| sample.terrain.watercourse.is_wetland_pool())
        .max_by(|(_, left), (_, right)| {
            left.terrain
                .watercourse
                .wetland_pool_influence
                .total_cmp(&right.terrain.watercourse.wetland_pool_influence)
        })
        .map(|(index, _)| index);
    let cool_wet_conifer = nearest_recipe_site(
        samples,
        request.width,
        request.depth,
        McloneOverworldBiomeRecipe::CoolWetConifer,
    );
    let warm_dry_steppe = nearest_recipe_site(
        samples,
        request.width,
        request.depth,
        McloneOverworldBiomeRecipe::WarmDrySteppe,
    );
    let warm_dry_steppe_core = nearest_steppe_band_site(
        samples,
        request.width,
        request.depth,
        McloneOverworldSteppeBand::Core,
    );
    let warm_dry_steppe_shoulder = nearest_steppe_band_site(
        samples,
        request.width,
        request.depth,
        McloneOverworldSteppeBand::Shoulder,
    );
    let snowy_alpine = samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| {
            mclone_overworld_biome_recipe(**sample) == McloneOverworldBiomeRecipe::SnowyAlpine
        })
        .max_by_key(|(_, sample)| sample.terrain.surface_y)
        .map(|(index, _)| index);
    let seam_river = if topology == McloneOverworldSamplingTopology::PeriodicX {
        samples
            .iter()
            .enumerate()
            .filter(|(_, sample)| sample.terrain.watercourse.is_channel())
            .min_by_key(|(index, _)| {
                let offset_x = *index % request.width as usize;
                let world_x = request.min_x + offset_x as i32 * request.step as i32;
                let canonical_x = world_x.rem_euclid(MCLONE_OVERWORLD_PERIOD_BLOCKS);
                canonical_x.min(MCLONE_OVERWORLD_PERIOD_BLOCKS - canonical_x)
            })
            .map(|(index, _)| index)
    } else {
        None
    };

    serde_json::json!({
        "rangeInterior": review_site_json(highest, samples, request),
        "mountainValley": review_site_json(mountain_valley, samples, request),
        "rangeEdge": review_site_json(range_edge, samples, request),
        "lowlandControl": review_site_json(lowland_control, samples, request),
        "deepOceanBasin": review_site_json(deep_ocean_basin, samples, request),
        "shelfBreak": review_site_json(shelf_break, samples, request),
        "river": review_site_json(river, samples, request),
        "mountainRiver": review_site_json(mountain_river, samples, request),
        "coastalRiver": review_site_json(coastal_river, samples, request),
        "waterfall": review_site_json(waterfall, samples, request),
        "wetland": review_site_json(wetland, samples, request),
        "wetlandPool": review_site_json(wetland_pool, samples, request),
        "coolWetConifer": review_site_json(cool_wet_conifer, samples, request),
        "warmDrySteppe": review_site_json(warm_dry_steppe, samples, request),
        "warmDrySteppeCore": review_site_json(warm_dry_steppe_core, samples, request),
        "warmDrySteppeShoulder": review_site_json(warm_dry_steppe_shoulder, samples, request),
        "snowyAlpine": review_site_json(snowy_alpine, samples, request),
        "periodicSeamRiver": review_site_json(seam_river, samples, request),
    })
}

fn nearest_recipe_site(
    samples: &[McloneOverworldLandformSample],
    width: u32,
    depth: u32,
    recipe: McloneOverworldBiomeRecipe,
) -> Option<usize> {
    let center_x = i64::from(width) / 2;
    let center_z = i64::from(depth) / 2;
    samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| mclone_overworld_biome_recipe(**sample) == recipe)
        .min_by_key(|(index, _)| {
            let x = *index as i64 % i64::from(width);
            let z = *index as i64 / i64::from(width);
            (x - center_x).abs() + (z - center_z).abs()
        })
        .map(|(index, _)| index)
}

fn nearest_steppe_band_site(
    samples: &[McloneOverworldLandformSample],
    width: u32,
    depth: u32,
    band: McloneOverworldSteppeBand,
) -> Option<usize> {
    let center_x = i64::from(width) / 2;
    let center_z = i64::from(depth) / 2;
    samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| {
            mclone_overworld_biome_recipe(**sample) == McloneOverworldBiomeRecipe::WarmDrySteppe
                && mclone_overworld_steppe_band(sample.terrain.climate) == band
        })
        .min_by_key(|(index, _)| {
            let x = (*index % width as usize) as i64;
            let z = (*index / width as usize) as i64;
            (x - center_x).abs() + (z - center_z).abs()
        })
        .map(|(index, _)| index)
}

fn stream_planning_json(
    attempts: &[(
        Option<McloneOverworldStreamPlan>,
        McloneOverworldStreamPlanAttempt,
    )],
    plans: &[McloneOverworldStreamPlan],
    elapsed_ms: f64,
) -> serde_json::Value {
    let mut rejection_counts = std::collections::BTreeMap::<&'static str, usize>::new();
    let expanded_nodes = attempts
        .iter()
        .map(|(_, attempt)| u64::from(attempt.expanded_nodes))
        .sum::<u64>();
    for (_, attempt) in attempts {
        if let Some(rejection) = attempt.rejection {
            *rejection_counts
                .entry(stream_rejection_label(rejection))
                .or_default() += 1;
        }
    }
    let candidate_attempts = attempts
        .iter()
        .map(|(plan, attempt)| {
            serde_json::json!({
                "canonicalStartChunk": [
                    attempt.candidate.canonical_start.x,
                    attempt.candidate.canonical_start.z,
                ],
                "workStartChunk": [
                    attempt.candidate.work_start.x,
                    attempt.candidate.work_start.z,
                ],
                "accepted": plan.is_some(),
                "rejection": attempt.rejection.map(stream_rejection_label),
                "expandedNodes": attempt.expanded_nodes,
            })
        })
        .collect::<Vec<_>>();
    let plans = plans
        .iter()
        .map(|plan| {
            serde_json::json!({
                "canonicalStartChunk": [
                    plan.structure.key.canonical_start.x,
                    plan.structure.key.canonical_start.z,
                ],
                "workStartChunk": [
                    plan.structure.work_start.x,
                    plan.structure.work_start.z,
                ],
                "bounds": {
                    "min": [
                        plan.structure.bounds.min_x,
                        plan.structure.bounds.min_y,
                        plan.structure.bounds.min_z,
                    ],
                    "max": [
                        plan.structure.bounds.max_x,
                        plan.structure.bounds.max_y,
                        plan.structure.bounds.max_z,
                    ],
                },
                "metrics": {
                    "routeLengthBlocks": plan.metrics.route_length_blocks,
                    "expandedNodes": plan.metrics.expanded_nodes,
                    "flatReaches": plan.metrics.flat_reaches,
                    "transitions": plan.metrics.transitions,
                    "totalRiseBlocks": plan.metrics.total_rise_blocks,
                    "maximumCutBlocks": plan.metrics.maximum_cut_blocks,
                    "maximumRequiredFillBlocks":
                        plan.metrics.maximum_required_fill_blocks,
                    "minimumBankClearanceBlocks":
                        plan.metrics.minimum_bank_clearance_blocks,
                },
                "referenceFootprintChunks": {
                    "min": [
                        plan.structure.work_start.x
                            - i32::from(MCLONE_OVERWORLD_STREAM_REFERENCE_RADIUS_CHUNKS),
                        plan.structure.work_start.z
                            - i32::from(MCLONE_OVERWORLD_STREAM_REFERENCE_RADIUS_CHUNKS),
                    ],
                    "max": [
                        plan.structure.work_start.x
                            + i32::from(MCLONE_OVERWORLD_STREAM_REFERENCE_RADIUS_CHUNKS),
                        plan.structure.work_start.z
                            + i32::from(MCLONE_OVERWORLD_STREAM_REFERENCE_RADIUS_CHUNKS),
                    ],
                },
                "nodes": plan.nodes.iter().map(|node| {
                    serde_json::json!([
                        node.x,
                        node.base_surface_y,
                        node.z,
                        node.water_y,
                    ])
                }).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "elapsedMs": elapsed_ms,
        "candidateStarts": attempts.len(),
        "acceptedStarts": attempts
            .iter()
            .filter(|(plan, _)| plan.is_some())
            .count(),
        "intersectingPlans": plans.len(),
        "expandedNodes": expanded_nodes,
        "rejections": rejection_counts,
        "candidateAttempts": candidate_attempts,
        "plans": plans,
    })
}

fn stream_rejection_label(rejection: McloneOverworldStreamRejection) -> &'static str {
    match rejection {
        McloneOverworldStreamRejection::NoMajorRiverInStartChunk => "noMajorRiverInStartChunk",
        McloneOverworldStreamRejection::NoBoundedRoute => "noBoundedRoute",
        McloneOverworldStreamRejection::RouteTooShort => "routeTooShort",
        McloneOverworldStreamRejection::RequiresTerrainFill => "requiresTerrainFill",
        McloneOverworldStreamRejection::InsufficientBankClearance => "insufficientBankClearance",
        McloneOverworldStreamRejection::ExcessiveCut => "excessiveCut",
        McloneOverworldStreamRejection::NoRaisedReach => "noRaisedReach",
        McloneOverworldStreamRejection::StructureBounds => "structureBounds",
    }
}

fn review_site_json(
    index: Option<usize>,
    samples: &[McloneOverworldLandformSample],
    request: McloneOverworldSampleRegionRequest,
) -> serde_json::Value {
    let Some(index) = index else {
        return serde_json::Value::Null;
    };
    let offset_x = index % request.width as usize;
    let offset_z = index / request.width as usize;
    let world_x = request.min_x + offset_x as i32 * request.step as i32;
    let world_z = request.min_z + offset_z as i32 * request.step as i32;
    serde_json::json!({
        "block": [world_x, world_z],
        "chunk": [world_x.div_euclid(16), world_z.div_euclid(16)],
        "sample": sample_json(samples[index]),
    })
}

fn sample_json(sample: McloneOverworldLandformSample) -> serde_json::Value {
    let terrain = sample.terrain;
    serde_json::json!({
        "continentalness": terrain.continentalness,
        "relief": terrain.relief,
        "ruggedness": terrain.ruggedness,
        "ridges": terrain.ridges,
        "mountainDetail": terrain.mountain_detail,
        "mountainStrength": terrain.mountain_strength(),
        "climate": {
            "temperature": terrain.climate.temperature,
            "moisture": terrain.climate.moisture,
            "altitudeAdjustedTemperature":
                terrain.climate.altitude_adjusted_temperature(terrain.surface_y),
            "warmDrySteppeSuitability":
                mclone_overworld_steppe_suitability(terrain.climate),
            "warmDrySteppeBand":
                mclone_overworld_steppe_band(terrain.climate).label(),
            "recipe": mclone_overworld_biome_recipe(sample).label(),
        },
        "bathymetry": {
            "oceanInterior": terrain.bathymetry.ocean_interior,
            "shelfInfluence": terrain.bathymetry.shelf_influence,
            "shelfBreakInfluence": terrain.bathymetry.shelf_break_influence,
            "basinInfluence": terrain.bathymetry.basin_influence,
            "seabedRelief": terrain.bathymetry.seabed_relief,
            "waterDepth": terrain.bathymetry.water_depth,
            "floorY": terrain.bathymetry.floor_y(),
        },
        "baseSurfaceY": terrain.base_surface_y,
        "surfaceY": terrain.surface_y,
        "slope": sample.slope,
        "exposure": sample.exposure(),
        "watercourse": {
            "distance": terrain.watercourse.distance,
            "channelInfluence": terrain.watercourse.channel_influence,
            "majorChannelInfluence": terrain.watercourse.major_channel_influence,
            "plannedStreamInfluence": terrain.watercourse.planned_stream_influence,
            "streamHeadwaterInfluence": terrain.watercourse.stream_headwater_influence,
            "bankInfluence": terrain.watercourse.bank_influence,
            "halfWidth": terrain.watercourse.half_width,
            "waterSurfaceY": terrain.watercourse.water_surface_y,
            "bedY": terrain.watercourse.bed_y,
            "tangent": [terrain.watercourse.tangent_x, terrain.watercourse.tangent_z],
            "flow": [terrain.watercourse.flow_x, terrain.watercourse.flow_z],
            "grade": terrain.watercourse.grade,
            "dropDistance": terrain.watercourse.drop_distance,
            "dropHeight": terrain.watercourse.drop_height,
            "dropUpperY": terrain.watercourse.drop_upper_y,
            "dropLowerY": terrain.watercourse.drop_lower_y,
            "fallColumn": terrain.watercourse.is_fall_column(),
            "wetlandInfluence": terrain.watercourse.wetland_influence,
            "wetlandPoolInfluence": terrain.watercourse.wetland_pool_influence,
        },
    })
}

#[derive(Clone, Debug)]
struct Config {
    output_dir: PathBuf,
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    radius_blocks: u32,
    step_blocks: u32,
    topology: McloneOverworldSamplingTopology,
}

impl Config {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut config = Self {
            output_dir: PathBuf::from(DEFAULT_OUTPUT_DIR),
            seed: DEFAULT_SEED,
            chunk_x: 0,
            chunk_z: 0,
            radius_blocks: DEFAULT_RADIUS_BLOCKS,
            step_blocks: DEFAULT_STEP_BLOCKS,
            topology: McloneOverworldSamplingTopology::Unbounded,
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--output-dir" => config.output_dir = parse_next(&mut args, &arg)?,
                "--seed" => config.seed = parse_next(&mut args, &arg)?,
                "--chunk-x" => config.chunk_x = parse_next(&mut args, &arg)?,
                "--chunk-z" => config.chunk_z = parse_next(&mut args, &arg)?,
                "--radius-blocks" => config.radius_blocks = parse_next(&mut args, &arg)?,
                "--step-blocks" => config.step_blocks = parse_next(&mut args, &arg)?,
                "--topology" => {
                    let value: String = parse_next(&mut args, &arg)?;
                    config.topology = match value.as_str() {
                        "plane" => McloneOverworldSamplingTopology::Unbounded,
                        "cylinder-x:384" => McloneOverworldSamplingTopology::PeriodicX,
                        _ => bail!("--topology requires plane or cylinder-x:384; got `{value}`"),
                    };
                }
                "--help" | "-h" => bail!(usage()),
                _ => bail!("unknown argument `{arg}`\n{}", usage()),
            }
        }
        if config.radius_blocks == 0 || config.radius_blocks > MAX_RADIUS_BLOCKS {
            bail!("--radius-blocks must be between 1 and {MAX_RADIUS_BLOCKS}");
        }
        if config.step_blocks == 0 || config.radius_blocks % config.step_blocks != 0 {
            bail!("--step-blocks must be nonzero and divide --radius-blocks");
        }
        Ok(config)
    }
}

fn parse_next<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    let value = args
        .next()
        .with_context(|| format!("{flag} requires a value"))?;
    value
        .parse()
        .map_err(|error| anyhow::anyhow!("invalid {flag} value `{value}`: {error}"))
}

fn usage() -> &'static str {
    "usage: mclone-overworld-review [--output-dir PATH] [--seed I64] \
     [--chunk-x I32] [--chunk-z I32] [--radius-blocks U32] \
     [--step-blocks U32] [--topology plane|cylinder-x:384]"
}

#[derive(Clone, Debug)]
struct RegionFacts {
    min_continentalness: f64,
    max_continentalness: f64,
    min_relief: f64,
    max_relief: f64,
    min_ruggedness: f64,
    max_ruggedness: f64,
    min_ridges: f64,
    max_ridges: f64,
    min_mountain_detail: f64,
    max_mountain_detail: f64,
    min_temperature: f64,
    max_temperature: f64,
    min_moisture: f64,
    max_moisture: f64,
    min_adjusted_temperature: f64,
    max_adjusted_temperature: f64,
    min_steppe_suitability: f64,
    max_steppe_suitability: f64,
    min_ocean_interior: f64,
    max_ocean_interior: f64,
    min_shelf_break: f64,
    max_shelf_break: f64,
    min_ocean_basin: f64,
    max_ocean_basin: f64,
    min_seabed_relief: f64,
    max_seabed_relief: f64,
    min_water_depth: i32,
    max_water_depth: i32,
    min_river_distance: f64,
    max_river_distance: f64,
    min_river_half_width: f64,
    max_river_half_width: f64,
    min_river_grade: f64,
    max_river_grade: f64,
    min_river_water_y: i32,
    max_river_water_y: i32,
    min_wetland_influence: f64,
    max_wetland_influence: f64,
    min_wetland_pool_influence: f64,
    max_wetland_pool_influence: f64,
    min_base_surface_y: i32,
    max_base_surface_y: i32,
    min_surface_y: i32,
    max_surface_y: i32,
    min_slope: f64,
    max_slope: f64,
    min_exposure: f64,
    max_exposure: f64,
    surface_y_p10: i32,
    surface_y_p50: i32,
    surface_y_p90: i32,
    water_depth_p10: i32,
    water_depth_p50: i32,
    water_depth_p90: i32,
    slope_p50: f64,
    slope_p90: f64,
    slope_p99: f64,
    water_columns: usize,
    shore_columns: usize,
    dry_land_columns: usize,
    ocean_biome_columns: usize,
    beach_biome_columns: usize,
    open_lowland_biome_columns: usize,
    wooded_upland_biome_columns: usize,
    river_biome_columns: usize,
    taiga_biome_columns: usize,
    snowy_mountains_biome_columns: usize,
    savanna_biome_columns: usize,
    ocean_recipe_columns: usize,
    shore_recipe_columns: usize,
    river_recipe_columns: usize,
    snowy_alpine_recipe_columns: usize,
    cool_wet_conifer_recipe_columns: usize,
    warm_dry_steppe_recipe_columns: usize,
    warm_dry_steppe_core_columns: usize,
    warm_dry_steppe_shoulder_columns: usize,
    temperate_woodland_recipe_columns: usize,
    temperate_meadow_recipe_columns: usize,
    ocean_floor_columns: usize,
    beach_surface_columns: usize,
    river_bed_columns: usize,
    wetland_bed_columns: usize,
    river_bank_columns: usize,
    grass_soil_columns: usize,
    exposed_stone_columns: usize,
    alpine_snow_columns: usize,
    river_channel_columns: usize,
    major_river_channel_columns: usize,
    planned_stream_columns: usize,
    stream_headwater_columns: usize,
    river_bank_influence_columns: usize,
    reach_level_count: usize,
    drop_transition_columns: usize,
    fall_columns: usize,
    wetland_columns: usize,
    wetland_pool_columns: usize,
    shelf_columns: usize,
    shelf_break_columns: usize,
    deep_basin_columns: usize,
    mountain_region_columns: usize,
    mountain_valley_columns: usize,
    mountain_crest_columns: usize,
    highland_columns: usize,
    summit_columns: usize,
    slope_edges: usize,
    slope_at_least_one: usize,
    slope_at_least_three: usize,
    fingerprint: u64,
    foundation_field_fingerprint: u64,
    climate_fingerprint: u64,
    terrain_language_fingerprint: u64,
}

impl RegionFacts {
    fn from_samples(samples: &[McloneOverworldLandformSample], width: u32, depth: u32) -> Self {
        let mut min_continentalness = f64::INFINITY;
        let mut max_continentalness = f64::NEG_INFINITY;
        let mut min_relief = f64::INFINITY;
        let mut max_relief = f64::NEG_INFINITY;
        let mut min_ruggedness = f64::INFINITY;
        let mut max_ruggedness = f64::NEG_INFINITY;
        let mut min_ridges = f64::INFINITY;
        let mut max_ridges = f64::NEG_INFINITY;
        let mut min_mountain_detail = f64::INFINITY;
        let mut max_mountain_detail = f64::NEG_INFINITY;
        let mut min_temperature = f64::INFINITY;
        let mut max_temperature = f64::NEG_INFINITY;
        let mut min_moisture = f64::INFINITY;
        let mut max_moisture = f64::NEG_INFINITY;
        let mut min_adjusted_temperature = f64::INFINITY;
        let mut max_adjusted_temperature = f64::NEG_INFINITY;
        let mut min_steppe_suitability = f64::INFINITY;
        let mut max_steppe_suitability = f64::NEG_INFINITY;
        let mut min_ocean_interior = f64::INFINITY;
        let mut max_ocean_interior = f64::NEG_INFINITY;
        let mut min_shelf_break = f64::INFINITY;
        let mut max_shelf_break = f64::NEG_INFINITY;
        let mut min_ocean_basin = f64::INFINITY;
        let mut max_ocean_basin = f64::NEG_INFINITY;
        let mut min_seabed_relief = f64::INFINITY;
        let mut max_seabed_relief = f64::NEG_INFINITY;
        let mut min_water_depth = i32::MAX;
        let mut max_water_depth = i32::MIN;
        let mut min_river_distance = f64::INFINITY;
        let mut max_river_distance = f64::NEG_INFINITY;
        let mut min_river_half_width = f64::INFINITY;
        let mut max_river_half_width = f64::NEG_INFINITY;
        let mut min_river_grade = f64::INFINITY;
        let mut max_river_grade = f64::NEG_INFINITY;
        let mut min_river_water_y = i32::MAX;
        let mut max_river_water_y = i32::MIN;
        let mut min_wetland_influence = f64::INFINITY;
        let mut max_wetland_influence = f64::NEG_INFINITY;
        let mut min_wetland_pool_influence = f64::INFINITY;
        let mut max_wetland_pool_influence = f64::NEG_INFINITY;
        let mut min_base_surface_y = i32::MAX;
        let mut max_base_surface_y = i32::MIN;
        let mut min_slope = f64::INFINITY;
        let mut max_slope = f64::NEG_INFINITY;
        let mut min_exposure = f64::INFINITY;
        let mut max_exposure = f64::NEG_INFINITY;
        let mut heights = Vec::with_capacity(samples.len());
        let mut slopes = Vec::with_capacity(samples.len());
        let mut water_depths = Vec::new();
        let mut water_columns = 0;
        let mut shore_columns = 0;
        let mut dry_land_columns = 0;
        let mut ocean_biome_columns = 0;
        let mut beach_biome_columns = 0;
        let mut open_lowland_biome_columns = 0;
        let mut wooded_upland_biome_columns = 0;
        let mut river_biome_columns = 0;
        let mut taiga_biome_columns = 0;
        let mut snowy_mountains_biome_columns = 0;
        let mut savanna_biome_columns = 0;
        let mut ocean_recipe_columns = 0;
        let mut shore_recipe_columns = 0;
        let mut river_recipe_columns = 0;
        let mut snowy_alpine_recipe_columns = 0;
        let mut cool_wet_conifer_recipe_columns = 0;
        let mut warm_dry_steppe_recipe_columns = 0;
        let mut warm_dry_steppe_core_columns = 0;
        let mut warm_dry_steppe_shoulder_columns = 0;
        let mut temperate_woodland_recipe_columns = 0;
        let mut temperate_meadow_recipe_columns = 0;
        let mut ocean_floor_columns = 0;
        let mut beach_surface_columns = 0;
        let mut river_bed_columns = 0;
        let mut wetland_bed_columns = 0;
        let mut river_bank_columns = 0;
        let mut grass_soil_columns = 0;
        let mut exposed_stone_columns = 0;
        let mut alpine_snow_columns = 0;
        let mut river_channel_columns = 0;
        let mut major_river_channel_columns = 0;
        let mut planned_stream_columns = 0;
        let mut stream_headwater_columns = 0;
        let mut river_bank_influence_columns = 0;
        let mut reach_levels = BTreeSet::new();
        let mut drop_transition_columns = 0;
        let mut fall_columns = 0;
        let mut wetland_columns = 0;
        let mut wetland_pool_columns = 0;
        let mut shelf_columns = 0;
        let mut shelf_break_columns = 0;
        let mut deep_basin_columns = 0;
        let mut mountain_region_columns = 0;
        let mut mountain_valley_columns = 0;
        let mut mountain_crest_columns = 0;
        let mut highland_columns = 0;
        let mut summit_columns = 0;
        let mut fingerprint = 0xcbf2_9ce4_8422_2325_u64;
        let mut foundation_field_fingerprint = 0xcbf2_9ce4_8422_2325_u64;
        let mut climate_fingerprint = 0xcbf2_9ce4_8422_2325_u64;
        let mut terrain_language_fingerprint = 0xcbf2_9ce4_8422_2325_u64;
        for landform in samples {
            let sample = landform.terrain;
            min_continentalness = min_continentalness.min(sample.continentalness);
            max_continentalness = max_continentalness.max(sample.continentalness);
            min_relief = min_relief.min(sample.relief);
            max_relief = max_relief.max(sample.relief);
            min_ruggedness = min_ruggedness.min(sample.ruggedness);
            max_ruggedness = max_ruggedness.max(sample.ruggedness);
            min_ridges = min_ridges.min(sample.ridges);
            max_ridges = max_ridges.max(sample.ridges);
            min_mountain_detail = min_mountain_detail.min(sample.mountain_detail);
            max_mountain_detail = max_mountain_detail.max(sample.mountain_detail);
            min_temperature = min_temperature.min(sample.climate.temperature);
            max_temperature = max_temperature.max(sample.climate.temperature);
            min_moisture = min_moisture.min(sample.climate.moisture);
            max_moisture = max_moisture.max(sample.climate.moisture);
            let adjusted_temperature = sample
                .climate
                .altitude_adjusted_temperature(sample.surface_y);
            min_adjusted_temperature = min_adjusted_temperature.min(adjusted_temperature);
            max_adjusted_temperature = max_adjusted_temperature.max(adjusted_temperature);
            let steppe_suitability = mclone_overworld_steppe_suitability(sample.climate);
            min_steppe_suitability = min_steppe_suitability.min(steppe_suitability);
            max_steppe_suitability = max_steppe_suitability.max(steppe_suitability);
            min_ocean_interior = min_ocean_interior.min(sample.bathymetry.ocean_interior);
            max_ocean_interior = max_ocean_interior.max(sample.bathymetry.ocean_interior);
            min_shelf_break = min_shelf_break.min(sample.bathymetry.shelf_break_influence);
            max_shelf_break = max_shelf_break.max(sample.bathymetry.shelf_break_influence);
            min_ocean_basin = min_ocean_basin.min(sample.bathymetry.basin_influence);
            max_ocean_basin = max_ocean_basin.max(sample.bathymetry.basin_influence);
            min_seabed_relief = min_seabed_relief.min(sample.bathymetry.seabed_relief);
            max_seabed_relief = max_seabed_relief.max(sample.bathymetry.seabed_relief);
            if sample.continentalness <= 0.0 {
                min_water_depth = min_water_depth.min(sample.bathymetry.water_depth);
                max_water_depth = max_water_depth.max(sample.bathymetry.water_depth);
                water_depths.push(sample.bathymetry.water_depth);
                if sample.bathymetry.basin_influence < 0.25 {
                    shelf_columns += 1;
                }
                if sample.bathymetry.shelf_break_influence >= 0.75 {
                    shelf_break_columns += 1;
                }
                if sample.bathymetry.water_depth >= 24 {
                    deep_basin_columns += 1;
                }
            }
            min_river_distance = min_river_distance.min(sample.watercourse.distance);
            max_river_distance = max_river_distance.max(sample.watercourse.distance);
            min_river_half_width = min_river_half_width.min(sample.watercourse.half_width);
            max_river_half_width = max_river_half_width.max(sample.watercourse.half_width);
            min_river_grade = min_river_grade.min(sample.watercourse.grade);
            max_river_grade = max_river_grade.max(sample.watercourse.grade);
            min_wetland_influence = min_wetland_influence.min(sample.watercourse.wetland_influence);
            max_wetland_influence = max_wetland_influence.max(sample.watercourse.wetland_influence);
            min_wetland_pool_influence =
                min_wetland_pool_influence.min(sample.watercourse.wetland_pool_influence);
            max_wetland_pool_influence =
                max_wetland_pool_influence.max(sample.watercourse.wetland_pool_influence);
            min_base_surface_y = min_base_surface_y.min(sample.base_surface_y);
            max_base_surface_y = max_base_surface_y.max(sample.base_surface_y);
            min_slope = min_slope.min(landform.slope);
            max_slope = max_slope.max(landform.slope);
            min_exposure = min_exposure.min(landform.exposure());
            max_exposure = max_exposure.max(landform.exposure());
            heights.push(sample.surface_y);
            slopes.push(landform.slope);
            if sample.watercourse.is_channel() {
                river_channel_columns += 1;
                min_river_water_y = min_river_water_y.min(sample.watercourse.water_surface_y);
                max_river_water_y = max_river_water_y.max(sample.watercourse.water_surface_y);
                reach_levels.insert(sample.watercourse.water_surface_y);
            }
            if sample.watercourse.is_major_channel() {
                major_river_channel_columns += 1;
            }
            if sample.watercourse.is_planned_stream() {
                planned_stream_columns += 1;
            }
            if sample.watercourse.is_stream_headwater() {
                stream_headwater_columns += 1;
            }
            if sample.watercourse.is_drop_transition() {
                drop_transition_columns += 1;
            }
            if sample.watercourse.is_fall_column() {
                fall_columns += 1;
            }
            if sample.watercourse.bank_influence > 0.0 {
                river_bank_influence_columns += 1;
            }
            if sample.watercourse.wetland_influence > 0.25
                && sample.watercourse.bank_influence > 0.0
            {
                wetland_columns += 1;
            }
            if sample.watercourse.is_wetland_pool() {
                wetland_pool_columns += 1;
            }
            let mountain_strength = sample.mountain_strength();
            if mountain_strength >= 0.35 && sample.surface_y > MCLONE_OVERWORLD_SEA_LEVEL {
                mountain_region_columns += 1;
                if sample.ridges <= 0.25 {
                    mountain_valley_columns += 1;
                }
                if sample.ridges >= 0.70 {
                    mountain_crest_columns += 1;
                }
            }
            if sample.surface_y >= 105 {
                highland_columns += 1;
            }
            if sample.surface_y >= 135 {
                summit_columns += 1;
            }
            if sample.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL - 2 {
                water_columns += 1;
            } else if sample.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 3 {
                shore_columns += 1;
            } else {
                dry_land_columns += 1;
            }
            let biome_id = mclone_overworld_biome_id_for_sample(*landform);
            match biome_id {
                OCEAN_BIOME_ID => ocean_biome_columns += 1,
                BEACH_BIOME_ID => beach_biome_columns += 1,
                PLAINS_BIOME_ID => open_lowland_biome_columns += 1,
                MCLONE_OVERWORLD_FOREST_BIOME_ID => wooded_upland_biome_columns += 1,
                MCLONE_OVERWORLD_RIVER_BIOME_ID => river_biome_columns += 1,
                MCLONE_OVERWORLD_TAIGA_BIOME_ID => taiga_biome_columns += 1,
                MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID => snowy_mountains_biome_columns += 1,
                MCLONE_OVERWORLD_SAVANNA_BIOME_ID => savanna_biome_columns += 1,
                _ => {}
            }
            let biome_recipe = mclone_overworld_biome_recipe(*landform);
            match biome_recipe {
                McloneOverworldBiomeRecipe::Ocean => ocean_recipe_columns += 1,
                McloneOverworldBiomeRecipe::Shore => shore_recipe_columns += 1,
                McloneOverworldBiomeRecipe::River => river_recipe_columns += 1,
                McloneOverworldBiomeRecipe::SnowyAlpine => snowy_alpine_recipe_columns += 1,
                McloneOverworldBiomeRecipe::CoolWetConifer => cool_wet_conifer_recipe_columns += 1,
                McloneOverworldBiomeRecipe::WarmDrySteppe => {
                    warm_dry_steppe_recipe_columns += 1;
                    match mclone_overworld_steppe_band(sample.climate) {
                        McloneOverworldSteppeBand::Core => {
                            warm_dry_steppe_core_columns += 1;
                        }
                        McloneOverworldSteppeBand::Shoulder => {
                            warm_dry_steppe_shoulder_columns += 1;
                        }
                        McloneOverworldSteppeBand::Outside => {
                            unreachable!("steppe recipe must have a steppe climate band");
                        }
                    }
                }
                McloneOverworldBiomeRecipe::TemperateWoodland => {
                    temperate_woodland_recipe_columns += 1
                }
                McloneOverworldBiomeRecipe::TemperateMeadow => temperate_meadow_recipe_columns += 1,
            }
            let surface_recipe = mclone_overworld_surface_recipe(*landform);
            match surface_recipe {
                McloneOverworldSurfaceRecipe::OceanFloor => ocean_floor_columns += 1,
                McloneOverworldSurfaceRecipe::Beach => beach_surface_columns += 1,
                McloneOverworldSurfaceRecipe::RiverBed => river_bed_columns += 1,
                McloneOverworldSurfaceRecipe::WetlandBed => wetland_bed_columns += 1,
                McloneOverworldSurfaceRecipe::RiverBank => river_bank_columns += 1,
                McloneOverworldSurfaceRecipe::GrassSoil => grass_soil_columns += 1,
                McloneOverworldSurfaceRecipe::AlpineSnow => alpine_snow_columns += 1,
                McloneOverworldSurfaceRecipe::ExposedStone => exposed_stone_columns += 1,
            }
            for byte in biome_id
                .to_le_bytes()
                .into_iter()
                .chain([surface_recipe_tag(surface_recipe)])
            {
                terrain_language_fingerprint ^= u64::from(byte);
                terrain_language_fingerprint =
                    terrain_language_fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
            }
            for byte in sample
                .continentalness
                .to_bits()
                .to_le_bytes()
                .into_iter()
                .chain(sample.relief.to_bits().to_le_bytes())
                .chain(sample.ruggedness.to_bits().to_le_bytes())
                .chain(sample.ridges.to_bits().to_le_bytes())
                .chain(sample.mountain_detail.to_bits().to_le_bytes())
                .chain(sample.climate.temperature.to_bits().to_le_bytes())
                .chain(sample.climate.moisture.to_bits().to_le_bytes())
                .chain(sample.bathymetry.ocean_interior.to_bits().to_le_bytes())
                .chain(sample.bathymetry.shelf_influence.to_bits().to_le_bytes())
                .chain(
                    sample
                        .bathymetry
                        .shelf_break_influence
                        .to_bits()
                        .to_le_bytes(),
                )
                .chain(sample.bathymetry.basin_influence.to_bits().to_le_bytes())
                .chain(sample.bathymetry.seabed_relief.to_bits().to_le_bytes())
                .chain(sample.bathymetry.water_depth.to_le_bytes())
                .chain(sample.base_surface_y.to_le_bytes())
                .chain(sample.watercourse.distance.to_bits().to_le_bytes())
                .chain(sample.watercourse.channel_influence.to_bits().to_le_bytes())
                .chain(
                    sample
                        .watercourse
                        .major_channel_influence
                        .to_bits()
                        .to_le_bytes(),
                )
                .chain(
                    sample
                        .watercourse
                        .planned_stream_influence
                        .to_bits()
                        .to_le_bytes(),
                )
                .chain(
                    sample
                        .watercourse
                        .stream_headwater_influence
                        .to_bits()
                        .to_le_bytes(),
                )
                .chain(sample.watercourse.bank_influence.to_bits().to_le_bytes())
                .chain(sample.watercourse.half_width.to_bits().to_le_bytes())
                .chain(sample.watercourse.water_surface_y.to_le_bytes())
                .chain(sample.watercourse.bed_y.to_le_bytes())
                .chain(sample.watercourse.tangent_x.to_bits().to_le_bytes())
                .chain(sample.watercourse.tangent_z.to_bits().to_le_bytes())
                .chain(sample.watercourse.flow_x.to_bits().to_le_bytes())
                .chain(sample.watercourse.flow_z.to_bits().to_le_bytes())
                .chain(sample.watercourse.grade.to_bits().to_le_bytes())
                .chain(sample.watercourse.drop_distance.to_bits().to_le_bytes())
                .chain(sample.watercourse.drop_height.to_le_bytes())
                .chain(sample.watercourse.drop_upper_y.to_le_bytes())
                .chain(sample.watercourse.drop_lower_y.to_le_bytes())
                .chain(sample.watercourse.wetland_influence.to_bits().to_le_bytes())
                .chain(
                    sample
                        .watercourse
                        .wetland_pool_influence
                        .to_bits()
                        .to_le_bytes(),
                )
                .chain(sample.surface_y.to_le_bytes())
            {
                fingerprint ^= u64::from(byte);
                fingerprint = fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
            }
            for byte in sample
                .continentalness
                .to_bits()
                .to_le_bytes()
                .into_iter()
                .chain(sample.relief.to_bits().to_le_bytes())
            {
                foundation_field_fingerprint ^= u64::from(byte);
                foundation_field_fingerprint =
                    foundation_field_fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
            }
            for byte in sample
                .climate
                .temperature
                .to_bits()
                .to_le_bytes()
                .into_iter()
                .chain(sample.climate.moisture.to_bits().to_le_bytes())
                .chain(adjusted_temperature.to_bits().to_le_bytes())
                .chain([
                    biome_recipe_tag(biome_recipe),
                    steppe_band_tag(mclone_overworld_steppe_band(sample.climate)),
                ])
            {
                climate_fingerprint ^= u64::from(byte);
                climate_fingerprint = climate_fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        heights.sort_unstable();
        slopes.sort_by(f64::total_cmp);
        water_depths.sort_unstable();
        if water_depths.is_empty() {
            min_water_depth = 0;
            max_water_depth = 0;
        }
        if reach_levels.is_empty() {
            min_river_water_y = MCLONE_OVERWORLD_SEA_LEVEL;
            max_river_water_y = MCLONE_OVERWORLD_SEA_LEVEL;
        }

        let mut slope_edges = 0;
        let mut slope_at_least_one = 0;
        let mut slope_at_least_three = 0;
        let width = width as usize;
        let depth = depth as usize;
        for z in 0..depth {
            for x in 0..width {
                let index = z * width + x;
                if x + 1 < width {
                    add_slope(
                        samples[index].terrain.surface_y,
                        samples[index + 1].terrain.surface_y,
                        &mut slope_edges,
                        &mut slope_at_least_one,
                        &mut slope_at_least_three,
                    );
                }
                if z + 1 < depth {
                    add_slope(
                        samples[index].terrain.surface_y,
                        samples[index + width].terrain.surface_y,
                        &mut slope_edges,
                        &mut slope_at_least_one,
                        &mut slope_at_least_three,
                    );
                }
            }
        }

        Self {
            min_continentalness,
            max_continentalness,
            min_relief,
            max_relief,
            min_ruggedness,
            max_ruggedness,
            min_ridges,
            max_ridges,
            min_mountain_detail,
            max_mountain_detail,
            min_temperature,
            max_temperature,
            min_moisture,
            max_moisture,
            min_adjusted_temperature,
            max_adjusted_temperature,
            min_steppe_suitability,
            max_steppe_suitability,
            min_ocean_interior,
            max_ocean_interior,
            min_shelf_break,
            max_shelf_break,
            min_ocean_basin,
            max_ocean_basin,
            min_seabed_relief,
            max_seabed_relief,
            min_water_depth,
            max_water_depth,
            min_river_distance,
            max_river_distance,
            min_river_half_width,
            max_river_half_width,
            min_river_grade,
            max_river_grade,
            min_river_water_y,
            max_river_water_y,
            min_wetland_influence,
            max_wetland_influence,
            min_wetland_pool_influence,
            max_wetland_pool_influence,
            min_base_surface_y,
            max_base_surface_y,
            min_surface_y: heights[0],
            max_surface_y: heights[heights.len() - 1],
            min_slope,
            max_slope,
            min_exposure,
            max_exposure,
            surface_y_p10: percentile(&heights, 10),
            surface_y_p50: percentile(&heights, 50),
            surface_y_p90: percentile(&heights, 90),
            water_depth_p10: percentile_or_default(&water_depths, 10, 0),
            water_depth_p50: percentile_or_default(&water_depths, 50, 0),
            water_depth_p90: percentile_or_default(&water_depths, 90, 0),
            slope_p50: percentile_f64(&slopes, 50),
            slope_p90: percentile_f64(&slopes, 90),
            slope_p99: percentile_f64(&slopes, 99),
            water_columns,
            shore_columns,
            dry_land_columns,
            ocean_biome_columns,
            beach_biome_columns,
            open_lowland_biome_columns,
            wooded_upland_biome_columns,
            river_biome_columns,
            taiga_biome_columns,
            snowy_mountains_biome_columns,
            savanna_biome_columns,
            ocean_recipe_columns,
            shore_recipe_columns,
            river_recipe_columns,
            snowy_alpine_recipe_columns,
            cool_wet_conifer_recipe_columns,
            warm_dry_steppe_recipe_columns,
            warm_dry_steppe_core_columns,
            warm_dry_steppe_shoulder_columns,
            temperate_woodland_recipe_columns,
            temperate_meadow_recipe_columns,
            ocean_floor_columns,
            beach_surface_columns,
            river_bed_columns,
            wetland_bed_columns,
            river_bank_columns,
            grass_soil_columns,
            exposed_stone_columns,
            alpine_snow_columns,
            river_channel_columns,
            major_river_channel_columns,
            planned_stream_columns,
            stream_headwater_columns,
            river_bank_influence_columns,
            reach_level_count: reach_levels.len(),
            drop_transition_columns,
            fall_columns,
            wetland_columns,
            wetland_pool_columns,
            shelf_columns,
            shelf_break_columns,
            deep_basin_columns,
            mountain_region_columns,
            mountain_valley_columns,
            mountain_crest_columns,
            highland_columns,
            summit_columns,
            slope_edges,
            slope_at_least_one,
            slope_at_least_three,
            fingerprint,
            foundation_field_fingerprint,
            climate_fingerprint,
            terrain_language_fingerprint,
        }
    }
}

fn recipe_component_stats(
    samples: &[McloneOverworldLandformSample],
    width: u32,
    depth: u32,
    step: u32,
    recipe: McloneOverworldBiomeRecipe,
) -> serde_json::Value {
    let width = width as usize;
    let depth = depth as usize;
    assert_eq!(samples.len(), width * depth);

    #[derive(Clone, Copy)]
    struct Component {
        columns: usize,
        span_x: usize,
        span_z: usize,
        touches_boundary: bool,
    }

    let mut visited = vec![false; samples.len()];
    let mut components = Vec::new();
    for start in 0..samples.len() {
        if visited[start] || mclone_overworld_biome_recipe(samples[start]) != recipe {
            continue;
        }

        visited[start] = true;
        let mut queue = VecDeque::from([start]);
        let mut columns = 0;
        let mut min_x = width;
        let mut max_x = 0;
        let mut min_z = depth;
        let mut max_z = 0;
        let mut touches_boundary = false;
        while let Some(index) = queue.pop_front() {
            let x = index % width;
            let z = index / width;
            columns += 1;
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_z = min_z.min(z);
            max_z = max_z.max(z);
            touches_boundary |= x == 0 || z == 0 || x + 1 == width || z + 1 == depth;

            for neighbor in [
                x.checked_sub(1).map(|next_x| z * width + next_x),
                (x + 1 < width).then_some(z * width + x + 1),
                z.checked_sub(1).map(|next_z| next_z * width + x),
                (z + 1 < depth).then_some((z + 1) * width + x),
            ]
            .into_iter()
            .flatten()
            {
                if !visited[neighbor] && mclone_overworld_biome_recipe(samples[neighbor]) == recipe
                {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }

        components.push(Component {
            columns,
            span_x: max_x - min_x + 1,
            span_z: max_z - min_z + 1,
            touches_boundary,
        });
    }

    let total_columns = components
        .iter()
        .map(|component| component.columns)
        .sum::<usize>();
    let boundary_touching = components
        .iter()
        .filter(|component| component.touches_boundary)
        .count();
    let mut by_size = components;
    by_size.sort_by_key(|component| component.columns);
    let median_columns = by_size
        .get(by_size.len().saturating_sub(1) / 2)
        .map_or(0, |component| component.columns);
    let largest = by_size.last().copied();
    let largest_components = by_size
        .iter()
        .rev()
        .take(8)
        .map(|component| component.columns)
        .collect::<Vec<_>>();
    let step = usize::try_from(step).expect("review step exceeds usize");

    serde_json::json!({
        "connectivity": 4,
        "componentCount": by_size.len(),
        "boundaryTouchingComponents": boundary_touching,
        "totalColumns": total_columns,
        "medianColumns": median_columns,
        "largestColumns": largest.map_or(0, |component| component.columns),
        "largestApproximateAreaBlocks": largest.map_or(0, |component| {
            component.columns.saturating_mul(step).saturating_mul(step)
        }),
        "largestSpanBlocks": largest.map_or([0, 0], |component| [
            component.span_x.saturating_mul(step),
            component.span_z.saturating_mul(step),
        ]),
        "largestTouchesBoundary": largest.is_some_and(|component| component.touches_boundary),
        "largestEightColumns": largest_components,
    })
}

fn add_slope(
    left: i32,
    right: i32,
    total: &mut usize,
    at_least_one: &mut usize,
    at_least_three: &mut usize,
) {
    *total += 1;
    let delta = left.abs_diff(right);
    if delta >= 1 {
        *at_least_one += 1;
    }
    if delta >= 3 {
        *at_least_three += 1;
    }
}

fn percentile(sorted: &[i32], percentile: usize) -> i32 {
    let index = (sorted.len() - 1) * percentile / 100;
    sorted[index]
}

fn percentile_or_default(sorted: &[i32], percentile_value: usize, default: i32) -> i32 {
    (!sorted.is_empty())
        .then(|| percentile(sorted, percentile_value))
        .unwrap_or(default)
}

fn percentile_f64(sorted: &[f64], percentile: usize) -> f64 {
    let index = (sorted.len() - 1) * percentile / 100;
    sorted[index]
}

fn render_map(
    samples: &[McloneOverworldTerrainSample],
    color: fn(McloneOverworldTerrainSample) -> [u8; 4],
) -> Vec<u8> {
    samples.iter().copied().flat_map(color).collect::<Vec<_>>()
}

fn render_stream_plan_map(
    region: &McloneOverworldSampleRegion,
    plans: &[McloneOverworldStreamPlan],
) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(region.samples.len() * 4);
    for offset_z in 0..region.request.depth {
        for offset_x in 0..region.request.width {
            let sample = region
                .sample(offset_x, offset_z)
                .expect("review region coordinates are in bounds");
            let world_x = region.request.min_x + offset_x as i32 * region.request.step as i32;
            let world_z = region.request.min_z + offset_z as i32 * region.request.step as i32;
            let mut color = stream_plan_base_color(sample);
            let mut nearest = None;
            for plan in plans {
                if world_x < plan.structure.bounds.min_x
                    || world_x > plan.structure.bounds.max_x
                    || world_z < plan.structure.bounds.min_z
                    || world_z > plan.structure.bounds.max_z
                {
                    continue;
                }
                let column = plan.sample_column(world_x, world_z);
                if nearest.as_ref().is_none_or(
                    |current: &mclone_worldgen::levelgen::McloneOverworldStreamColumnSample| {
                        column.distance.total_cmp(&current.distance).is_lt()
                    },
                ) {
                    nearest = Some(column);
                }
            }
            if let Some(column) = nearest {
                color = if column.distance <= column.half_width {
                    if column.transition.is_some() {
                        [239, 183, 66, 255]
                    } else if column.is_headwater {
                        [66, 222, 215, 255]
                    } else if column.is_confluence {
                        [91, 154, 239, 255]
                    } else {
                        let level = f64::from(column.water_y - MCLONE_OVERWORLD_SEA_LEVEL) / 6.0;
                        lerp_color([42, 102, 214, 255], [99, 226, 230, 255], level)
                    }
                } else if column.distance <= 10.0 {
                    lerp_color(color, [48, 38, 28, 255], 0.42)
                } else {
                    color
                };
            }
            pixels.extend(color);
        }
    }
    pixels
}

fn render_stream_candidate_map(
    region: &McloneOverworldSampleRegion,
    attempts: &[(
        Option<McloneOverworldStreamPlan>,
        McloneOverworldStreamPlanAttempt,
    )],
    plans: &[McloneOverworldStreamPlan],
) -> Vec<u8> {
    let mut pixels = region
        .samples
        .iter()
        .copied()
        .flat_map(stream_plan_base_color)
        .collect::<Vec<_>>();
    let radius = i32::from(MCLONE_OVERWORLD_STREAM_REFERENCE_RADIUS_CHUNKS);
    for plan in plans {
        let footprint_min_x = (plan.structure.work_start.x - radius) * 16;
        let footprint_min_z = (plan.structure.work_start.z - radius) * 16;
        let footprint_max_x = (plan.structure.work_start.x + radius + 1) * 16 - 1;
        let footprint_max_z = (plan.structure.work_start.z + radius + 1) * 16 - 1;
        draw_world_box(
            &mut pixels,
            region,
            footprint_min_x,
            footprint_min_z,
            footprint_max_x,
            footprint_max_z,
            [110, 76, 166, 255],
        );
        draw_world_box(
            &mut pixels,
            region,
            plan.structure.bounds.min_x,
            plan.structure.bounds.min_z,
            plan.structure.bounds.max_x,
            plan.structure.bounds.max_z,
            [242, 189, 67, 255],
        );
    }
    for (plan, attempt) in attempts {
        let x = attempt.candidate.work_start.min_block_x() + 8;
        let z = attempt.candidate.work_start.min_block_z() + 8;
        let color = match attempt.rejection {
            None if plan.is_some() => [57, 225, 121, 255],
            Some(McloneOverworldStreamRejection::NoMajorRiverInStartChunk) => [99, 110, 105, 255],
            Some(McloneOverworldStreamRejection::NoRaisedReach) => [242, 179, 70, 255],
            Some(McloneOverworldStreamRejection::ExcessiveCut) => [221, 92, 54, 255],
            Some(McloneOverworldStreamRejection::RequiresTerrainFill)
            | Some(McloneOverworldStreamRejection::InsufficientBankClearance) => [229, 64, 82, 255],
            Some(_) => [191, 87, 174, 255],
            None => [255, 255, 255, 255],
        };
        draw_world_marker(&mut pixels, region, x, z, color);
    }
    pixels
}

fn render_stream_plan_cost_map(
    region: &McloneOverworldSampleRegion,
    plans: &[McloneOverworldStreamPlan],
) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(region.samples.len() * 4);
    for offset_z in 0..region.request.depth {
        for offset_x in 0..region.request.width {
            let sample = region
                .sample(offset_x, offset_z)
                .expect("review region coordinates are in bounds");
            let world_x = region.request.min_x + offset_x as i32 * region.request.step as i32;
            let world_z = region.request.min_z + offset_z as i32 * region.request.step as i32;
            let mut color = stream_plan_base_color(sample);
            let nearest = plans
                .iter()
                .filter(|plan| {
                    world_x >= plan.structure.bounds.min_x
                        && world_x <= plan.structure.bounds.max_x
                        && world_z >= plan.structure.bounds.min_z
                        && world_z <= plan.structure.bounds.max_z
                })
                .map(|plan| plan.sample_column(world_x, world_z))
                .min_by(|left, right| left.distance.total_cmp(&right.distance));
            if let Some(column) = nearest.filter(|column| column.distance <= column.half_width) {
                let target_bed_y = column.water_y - 2;
                let cut = sample.base_surface_y - target_bed_y;
                color = if cut < 0 {
                    lerp_color(
                        [88, 181, 236, 255],
                        [218, 55, 74, 255],
                        f64::from(-cut) / 2.0,
                    )
                } else {
                    lerp_color(
                        [245, 223, 106, 255],
                        [155, 42, 36, 255],
                        f64::from(cut) / 8.0,
                    )
                };
            }
            pixels.extend(color);
        }
    }
    pixels
}

fn stream_plan_base_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    let y = sample.base_surface_y;
    if sample.continentalness <= 0.0 {
        return lerp_color(
            [64, 147, 202, 255],
            [12, 42, 103, 255],
            f64::from((MCLONE_OVERWORLD_SEA_LEVEL - y).clamp(0, 40)) / 40.0,
        );
    }
    let mut color = if y <= MCLONE_OVERWORLD_SEA_LEVEL + 2 {
        [219, 200, 132, 255]
    } else if y <= 96 {
        lerp_color(
            [92, 154, 77, 255],
            [49, 92, 50, 255],
            f64::from(y - 65) / 31.0,
        )
    } else {
        lerp_color(
            [116, 92, 62, 255],
            [187, 190, 187, 255],
            f64::from(y - 96) / 80.0,
        )
    };
    if y.rem_euclid(4) == 0 {
        color = lerp_color(color, [22, 31, 24, 255], 0.18);
    }
    color
}

fn draw_world_marker(
    pixels: &mut [u8],
    region: &McloneOverworldSampleRegion,
    world_x: i32,
    world_z: i32,
    color: [u8; 4],
) {
    let Some((center_x, center_z)) = world_to_review_pixel(region, world_x, world_z) else {
        return;
    };
    for offset_z in -1..=1 {
        for offset_x in -1..=1 {
            set_review_pixel(
                pixels,
                region.request.width,
                region.request.depth,
                center_x + offset_x,
                center_z + offset_z,
                color,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_world_box(
    pixels: &mut [u8],
    region: &McloneOverworldSampleRegion,
    min_x: i32,
    min_z: i32,
    max_x: i32,
    max_z: i32,
    color: [u8; 4],
) {
    let step = region.request.step as i32;
    let pixel_min_x = (min_x - region.request.min_x).div_euclid(step);
    let pixel_min_z = (min_z - region.request.min_z).div_euclid(step);
    let pixel_max_x = (max_x - region.request.min_x).div_euclid(step);
    let pixel_max_z = (max_z - region.request.min_z).div_euclid(step);
    for x in pixel_min_x..=pixel_max_x {
        set_review_pixel(
            pixels,
            region.request.width,
            region.request.depth,
            x,
            pixel_min_z,
            color,
        );
        set_review_pixel(
            pixels,
            region.request.width,
            region.request.depth,
            x,
            pixel_max_z,
            color,
        );
    }
    for z in pixel_min_z..=pixel_max_z {
        set_review_pixel(
            pixels,
            region.request.width,
            region.request.depth,
            pixel_min_x,
            z,
            color,
        );
        set_review_pixel(
            pixels,
            region.request.width,
            region.request.depth,
            pixel_max_x,
            z,
            color,
        );
    }
}

fn world_to_review_pixel(
    region: &McloneOverworldSampleRegion,
    world_x: i32,
    world_z: i32,
) -> Option<(i32, i32)> {
    let step = region.request.step as i32;
    let x = (world_x - region.request.min_x).div_euclid(step);
    let z = (world_z - region.request.min_z).div_euclid(step);
    (x >= 0 && z >= 0 && x < region.request.width as i32 && z < region.request.depth as i32)
        .then_some((x, z))
}

fn set_review_pixel(pixels: &mut [u8], width: u32, height: u32, x: i32, z: i32, color: [u8; 4]) {
    if x < 0 || z < 0 || x >= width as i32 || z >= height as i32 {
        return;
    }
    let index = (z as usize * width as usize + x as usize) * 4;
    pixels[index..index + 4].copy_from_slice(&color);
}

fn render_landform_map(
    samples: &[McloneOverworldLandformSample],
    color: fn(McloneOverworldLandformSample) -> [u8; 4],
) -> Vec<u8> {
    samples.iter().copied().flat_map(color).collect::<Vec<_>>()
}

fn continentalness_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    if sample.continentalness < 0.0 {
        lerp_color(
            [18, 48, 101, 255],
            [92, 177, 224, 255],
            sample.continentalness + 1.0,
        )
    } else {
        lerp_color(
            [221, 205, 139, 255],
            [42, 112, 61, 255],
            sample.continentalness,
        )
    }
}

fn relief_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    if sample.relief < 0.0 {
        lerp_color(
            [64, 50, 126, 255],
            [213, 220, 229, 255],
            sample.relief + 1.0,
        )
    } else {
        lerp_color([213, 220, 229, 255], [172, 76, 35, 255], sample.relief)
    }
}

fn ruggedness_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    if sample.ruggedness < 0.0 {
        lerp_color(
            [45, 82, 122, 255],
            [218, 220, 210, 255],
            sample.ruggedness + 1.0,
        )
    } else {
        lerp_color([218, 220, 210, 255], [121, 69, 48, 255], sample.ruggedness)
    }
}

fn ridges_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    lerp_color([37, 74, 62, 255], [239, 236, 220, 255], sample.ridges)
}

fn mountain_detail_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    if sample.mountain_detail < 0.0 {
        lerp_color(
            [55, 72, 116, 255],
            [218, 220, 210, 255],
            sample.mountain_detail + 1.0,
        )
    } else {
        lerp_color(
            [218, 220, 210, 255],
            [147, 73, 42, 255],
            sample.mountain_detail,
        )
    }
}

fn surface_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    match sample.surface_y {
        y if y <= MCLONE_OVERWORLD_SEA_LEVEL - 2 => lerp_color(
            [13, 39, 91, 255],
            [59, 137, 196, 255],
            f64::from(y - 42) / 19.0,
        ),
        y if y <= MCLONE_OVERWORLD_SEA_LEVEL + 3 => [219, 201, 132, 255],
        y => lerp_color(
            [79, 144, 70, 255],
            [102, 74, 48, 255],
            f64::from(y - 67) / 93.0,
        ),
    }
}

fn base_surface_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    surface_color(McloneOverworldTerrainSample {
        surface_y: sample.base_surface_y,
        ..sample
    })
}

fn water_depth_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    if sample.continentalness > 0.0 {
        return [48, 72, 49, 255];
    }
    lerp_color(
        [112, 205, 224, 255],
        [7, 20, 68, 255],
        f64::from(sample.bathymetry.water_depth - 2) / 50.0,
    )
}

fn shelf_break_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    if sample.continentalness > 0.0 {
        return [32, 44, 42, 255];
    }
    lerp_color(
        [19, 54, 91, 255],
        [242, 194, 75, 255],
        sample.bathymetry.shelf_break_influence,
    )
}

fn ocean_basin_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    if sample.continentalness > 0.0 {
        return [32, 44, 42, 255];
    }
    lerp_color(
        [83, 171, 196, 255],
        [23, 27, 84, 255],
        sample.bathymetry.basin_influence,
    )
}

fn seabed_relief_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    if sample.continentalness > 0.0 {
        return [32, 44, 42, 255];
    }
    if sample.bathymetry.seabed_relief < 0.0 {
        lerp_color(
            [28, 50, 105, 255],
            [174, 190, 175, 255],
            sample.bathymetry.seabed_relief + 1.0,
        )
    } else {
        lerp_color(
            [174, 190, 175, 255],
            [181, 113, 57, 255],
            sample.bathymetry.seabed_relief,
        )
    }
}

fn river_distance_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    if sample.continentalness <= 0.0 {
        return [15, 35, 72, 255];
    }
    if sample.watercourse.is_channel() {
        if sample.watercourse.is_stream_headwater() {
            return lerp_color(
                [91, 184, 192, 255],
                [25, 98, 153, 255],
                sample.watercourse.stream_headwater_influence,
            );
        }
        if sample.watercourse.is_planned_stream() {
            return lerp_color(
                [86, 190, 213, 255],
                [24, 112, 176, 255],
                sample.watercourse.planned_stream_influence,
            );
        }
        return lerp_color(
            [111, 210, 232, 255],
            [18, 85, 166, 255],
            sample.watercourse.channel_influence,
        );
    }
    if sample.watercourse.bank_influence > 0.0 {
        return lerp_color(
            [193, 181, 118, 255],
            [86, 152, 119, 255],
            sample.watercourse.bank_influence,
        );
    }
    [43, 66, 51, 255]
}

fn river_level_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    if sample.watercourse.bank_influence == 0.0 {
        return [31, 41, 43, 255];
    }
    lerp_color(
        [34, 101, 183, 255],
        [203, 226, 235, 255],
        f64::from(sample.watercourse.water_surface_y - MCLONE_OVERWORLD_SEA_LEVEL) / 8.0,
    )
}

fn river_grade_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    if sample.watercourse.bank_influence == 0.0 {
        return [31, 41, 43, 255];
    }
    lerp_color(
        [43, 138, 104, 255],
        [215, 81, 52, 255],
        sample.watercourse.grade / 0.4,
    )
}

fn river_transition_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    if sample.watercourse.bank_influence == 0.0 {
        return [31, 41, 43, 255];
    }
    if sample.watercourse.is_fall_column() {
        return [246, 243, 218, 255];
    }
    if sample.watercourse.is_drop_transition() {
        if sample.watercourse.drop_distance >= 0.0 {
            return lerp_color(
                [94, 63, 156, 255],
                [234, 128, 48, 255],
                1.0 - sample.watercourse.drop_distance.abs() / 10.0,
            );
        }
        return lerp_color(
            [29, 107, 159, 255],
            [70, 205, 220, 255],
            1.0 - sample.watercourse.drop_distance.abs() / 10.0,
        );
    }
    if sample.watercourse.is_stream_headwater() {
        return lerp_color(
            [48, 105, 148, 255],
            [155, 103, 191, 255],
            sample.watercourse.stream_headwater_influence,
        );
    }
    if sample.watercourse.is_planned_stream() {
        return [51, 145, 173, 255];
    }
    [52, 77, 70, 255]
}

fn wetland_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    if sample.watercourse.bank_influence == 0.0 {
        return [31, 41, 43, 255];
    }
    if sample.watercourse.is_wetland_pool() {
        return lerp_color(
            [47, 126, 139, 255],
            [83, 182, 154, 255],
            sample.watercourse.wetland_pool_influence,
        );
    }
    lerp_color(
        [123, 111, 70, 255],
        [52, 154, 118, 255],
        sample.watercourse.wetland_influence,
    )
}

fn slope_color(sample: McloneOverworldLandformSample) -> [u8; 4] {
    lerp_color([32, 65, 84, 255], [239, 223, 190, 255], sample.slope / 1.5)
}

fn temperature_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    let amount = sample.climate.temperature * 0.5 + 0.5;
    if amount < 0.5 {
        lerp_color([42, 96, 176, 255], [222, 230, 203, 255], amount * 2.0)
    } else {
        lerp_color(
            [222, 230, 203, 255],
            [214, 83, 45, 255],
            (amount - 0.5) * 2.0,
        )
    }
}

fn moisture_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    let amount = sample.climate.moisture * 0.5 + 0.5;
    lerp_color([201, 157, 73, 255], [32, 111, 128, 255], amount)
}

fn steppe_suitability_color(sample: McloneOverworldTerrainSample) -> [u8; 4] {
    let suitability = mclone_overworld_steppe_suitability(sample.climate);
    let base = lerp_color([39, 48, 50, 255], [221, 190, 86, 255], suitability);
    match mclone_overworld_steppe_band(sample.climate) {
        McloneOverworldSteppeBand::Outside => base,
        McloneOverworldSteppeBand::Shoulder => lerp_color(base, [235, 205, 112, 255], 0.45),
        McloneOverworldSteppeBand::Core => lerp_color(base, [196, 132, 47, 255], 0.65),
    }
}

fn adjusted_temperature_color(sample: McloneOverworldLandformSample) -> [u8; 4] {
    let amount = sample
        .terrain
        .climate
        .altitude_adjusted_temperature(sample.terrain.surface_y)
        * 0.5
        + 0.5;
    if amount < 0.5 {
        lerp_color([225, 245, 250, 255], [117, 157, 181, 255], amount * 2.0)
    } else {
        lerp_color(
            [117, 157, 181, 255],
            [203, 106, 50, 255],
            (amount - 0.5) * 2.0,
        )
    }
}

fn biome_recipe_color(sample: McloneOverworldLandformSample) -> [u8; 4] {
    match mclone_overworld_biome_recipe(sample) {
        McloneOverworldBiomeRecipe::Ocean => [25, 76, 145, 255],
        McloneOverworldBiomeRecipe::Shore => [222, 207, 143, 255],
        McloneOverworldBiomeRecipe::River => [42, 119, 181, 255],
        McloneOverworldBiomeRecipe::SnowyAlpine => [229, 240, 242, 255],
        McloneOverworldBiomeRecipe::CoolWetConifer => [44, 92, 75, 255],
        McloneOverworldBiomeRecipe::WarmDrySteppe => {
            match mclone_overworld_steppe_band(sample.terrain.climate) {
                McloneOverworldSteppeBand::Core => [185, 145, 55, 255],
                McloneOverworldSteppeBand::Shoulder => [207, 181, 91, 255],
                McloneOverworldSteppeBand::Outside => {
                    unreachable!("steppe recipe must have a steppe climate band")
                }
            }
        }
        McloneOverworldBiomeRecipe::TemperateWoodland => [42, 105, 55, 255],
        McloneOverworldBiomeRecipe::TemperateMeadow => [112, 176, 76, 255],
    }
}

fn biome_color(sample: McloneOverworldLandformSample) -> [u8; 4] {
    match mclone_overworld_biome_id_for_sample(sample) {
        OCEAN_BIOME_ID => [25, 76, 145, 255],
        BEACH_BIOME_ID => [222, 207, 143, 255],
        MCLONE_OVERWORLD_RIVER_BIOME_ID => [42, 119, 181, 255],
        MCLONE_OVERWORLD_TAIGA_BIOME_ID => [44, 92, 75, 255],
        MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID => [229, 240, 242, 255],
        MCLONE_OVERWORLD_SAVANNA_BIOME_ID => [185, 162, 73, 255],
        PLAINS_BIOME_ID => [112, 176, 76, 255],
        MCLONE_OVERWORLD_FOREST_BIOME_ID => [42, 105, 55, 255],
        _ => [211, 64, 198, 255],
    }
}

fn biome_recipe_tag(recipe: McloneOverworldBiomeRecipe) -> u8 {
    match recipe {
        McloneOverworldBiomeRecipe::Ocean => 0,
        McloneOverworldBiomeRecipe::Shore => 1,
        McloneOverworldBiomeRecipe::River => 2,
        McloneOverworldBiomeRecipe::SnowyAlpine => 3,
        McloneOverworldBiomeRecipe::CoolWetConifer => 4,
        McloneOverworldBiomeRecipe::WarmDrySteppe => 5,
        McloneOverworldBiomeRecipe::TemperateWoodland => 6,
        McloneOverworldBiomeRecipe::TemperateMeadow => 7,
    }
}

fn steppe_band_tag(band: McloneOverworldSteppeBand) -> u8 {
    match band {
        McloneOverworldSteppeBand::Outside => 0,
        McloneOverworldSteppeBand::Shoulder => 1,
        McloneOverworldSteppeBand::Core => 2,
    }
}

fn surface_recipe_color(sample: McloneOverworldLandformSample) -> [u8; 4] {
    match mclone_overworld_surface_recipe(sample) {
        McloneOverworldSurfaceRecipe::OceanFloor => [104, 111, 119, 255],
        McloneOverworldSurfaceRecipe::Beach => [222, 207, 143, 255],
        McloneOverworldSurfaceRecipe::RiverBed => [86, 110, 123, 255],
        McloneOverworldSurfaceRecipe::WetlandBed => [117, 137, 139, 255],
        McloneOverworldSurfaceRecipe::RiverBank => [112, 138, 74, 255],
        McloneOverworldSurfaceRecipe::GrassSoil => [91, 151, 67, 255],
        McloneOverworldSurfaceRecipe::AlpineSnow => [229, 240, 242, 255],
        McloneOverworldSurfaceRecipe::ExposedStone => [137, 137, 137, 255],
    }
}

fn surface_recipe_tag(recipe: McloneOverworldSurfaceRecipe) -> u8 {
    match recipe {
        McloneOverworldSurfaceRecipe::OceanFloor => 0,
        McloneOverworldSurfaceRecipe::Beach => 1,
        McloneOverworldSurfaceRecipe::RiverBed => 2,
        McloneOverworldSurfaceRecipe::WetlandBed => 3,
        McloneOverworldSurfaceRecipe::RiverBank => 4,
        McloneOverworldSurfaceRecipe::GrassSoil => 5,
        McloneOverworldSurfaceRecipe::AlpineSnow => 6,
        McloneOverworldSurfaceRecipe::ExposedStone => 7,
    }
}

fn lerp_color(from: [u8; 4], to: [u8; 4], amount: f64) -> [u8; 4] {
    let amount = amount.clamp(0.0, 1.0);
    std::array::from_fn(|index| {
        (f64::from(from[index]) + f64::from(to[index] as i16 - from[index] as i16) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    })
}

fn combine_maps<const N: usize>(width: u32, height: u32, maps: [&[u8]; N]) -> Vec<u8> {
    let map_count = u32::try_from(N).expect("review map count exceeds u32");
    let combined_width = width * map_count + MAP_GAP_PIXELS * map_count.saturating_sub(1);
    let mut combined = vec![18_u8; (combined_width * height * 4) as usize];
    for pixel in combined.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    for (map_index, map) in maps.into_iter().enumerate() {
        let offset_x = map_index as u32 * (width + MAP_GAP_PIXELS);
        for y in 0..height {
            let source_start = (y * width * 4) as usize;
            let source_end = source_start + (width * 4) as usize;
            let target_start = ((y * combined_width + offset_x) * 4) as usize;
            combined[target_start..target_start + (width * 4) as usize]
                .copy_from_slice(&map[source_start..source_end]);
        }
    }
    combined
}

fn save_rgba(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
    let image = RgbaImage::from_raw(width, height, pixels.to_vec())
        .context("mclone overworld review image dimensions did not match pixels")?;
    image
        .save(path)
        .with_context(|| format!("save review map {}", path.display()))
}

fn git_state() -> (Option<String>, Option<bool>) {
    let commit = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned());
    let dirty = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.is_empty());
    (commit, dirty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_to_a_broad_chunk_aligned_review() {
        let config = Config::parse(Vec::<String>::new()).unwrap();
        assert_eq!(config.output_dir, PathBuf::from(DEFAULT_OUTPUT_DIR));
        assert_eq!(config.seed, DEFAULT_SEED);
        assert_eq!([config.chunk_x, config.chunk_z], [0, 0]);
        assert_eq!(config.radius_blocks, 3_072);
        assert_eq!(config.step_blocks, 16);
    }

    #[test]
    fn config_rejects_a_non_dividing_step() {
        assert!(
            Config::parse(["--radius-blocks", "100", "--step-blocks", "16"].map(str::to_owned))
                .is_err()
        );
    }

    #[test]
    fn combined_map_preserves_panel_order_and_gap() {
        let left = [1, 2, 3, 255];
        let middle = [4, 5, 6, 255];
        let right = [7, 8, 9, 255];
        let combined = combine_maps(1, 1, [&left, &middle, &right]);
        assert_eq!(&combined[0..4], &left);
        let middle_offset = ((1 + MAP_GAP_PIXELS) * 4) as usize;
        let right_offset = ((2 + MAP_GAP_PIXELS * 2) * 4) as usize;
        assert_eq!(&combined[middle_offset..middle_offset + 4], &middle);
        assert_eq!(&combined[right_offset..right_offset + 4], &right);
    }
}
