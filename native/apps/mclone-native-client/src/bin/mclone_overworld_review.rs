use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use image::RgbaImage;
use mclone_worldgen::levelgen::{
    BEACH_BIOME_ID, MCLONE_OVERWORLD_DECORATION_REVISION, MCLONE_OVERWORLD_FIELD_REVISION,
    MCLONE_OVERWORLD_FOREST_BIOME_ID, MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldLandformSample,
    McloneOverworldSampleRegionRequest, McloneOverworldSampler, McloneOverworldSurfaceRecipe,
    McloneOverworldTerrainSample, OCEAN_BIOME_ID, PLAINS_BIOME_ID,
    mclone_overworld_biome_id_for_sample, mclone_overworld_spawn_chunk,
    mclone_overworld_surface_recipe,
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

    let sampler = McloneOverworldSampler::new(config.seed);
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
    let review_sites = select_review_sites(&landforms, request);
    let center_sample = sampler.sample_landform(center_x, center_z);
    let spawn_chunk = mclone_overworld_spawn_chunk(config.seed);
    let spawn_x = spawn_chunk.min_block_x() + 8;
    let spawn_z = spawn_chunk.min_block_z() + 8;
    let spawn_sample = sampler.sample_landform(spawn_x, spawn_z);

    let prefix = format!(
        "mclone-overworld-v1-seed-{}-chunk-{}-{}",
        config.seed, config.chunk_x, config.chunk_z
    );
    let continentalness_path = config
        .output_dir
        .join(format!("{prefix}-continentalness.png"));
    let relief_path = config.output_dir.join(format!("{prefix}-relief.png"));
    let ruggedness_path = config.output_dir.join(format!("{prefix}-ruggedness.png"));
    let ridges_path = config.output_dir.join(format!("{prefix}-ridges.png"));
    let surface_path = config.output_dir.join(format!("{prefix}-surface-y.png"));
    let slope_path = config.output_dir.join(format!("{prefix}-slope.png"));
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
    let surface = render_map(&region.samples, surface_color);
    let slope = render_landform_map(&landforms, slope_color);
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
    save_rgba(&surface_path, request.width, request.depth, &surface)?;
    save_rgba(&slope_path, request.width, request.depth, &slope)?;
    let combined = combine_maps(
        request.width,
        request.depth,
        [&continentalness, &relief, &ruggedness, &ridges, &surface],
    );
    save_rgba(
        &fields_path,
        request.width * 5 + MAP_GAP_PIXELS * 4,
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
        "schema": 4,
        "profile": "mclone-overworld-v1",
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
            "surfaceY": [facts.min_surface_y, facts.max_surface_y],
            "slope": [facts.min_slope, facts.max_slope],
            "exposure": [facts.min_exposure, facts.max_exposure],
        },
        "surfaceYPercentiles": {
            "p10": facts.surface_y_p10,
            "p50": facts.surface_y_p50,
            "p90": facts.surface_y_p90,
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
        },
        "surfaceRecipeCounts": {
            "oceanFloor": facts.ocean_floor_columns,
            "beach": facts.beach_surface_columns,
            "grassSoil": facts.grass_soil_columns,
            "exposedStone": facts.exposed_stone_columns,
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
        "terrainLanguageFingerprint": facts.terrain_language_fingerprint,
        "maps": {
            "order": ["continentalness", "relief", "ruggedness", "ridges", "surfaceY"],
            "combined": fields_path,
            "continentalness": continentalness_path,
            "relief": relief_path,
            "ruggedness": ruggedness_path,
            "ridges": ridges_path,
            "surfaceY": surface_path,
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

    serde_json::json!({
        "rangeInterior": review_site_json(highest, samples, request),
        "mountainValley": review_site_json(mountain_valley, samples, request),
        "rangeEdge": review_site_json(range_edge, samples, request),
        "lowlandControl": review_site_json(lowland_control, samples, request),
    })
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
        "mountainStrength": terrain.mountain_strength(),
        "surfaceY": terrain.surface_y,
        "slope": sample.slope,
        "exposure": sample.exposure(),
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
     [--step-blocks U32]"
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
    min_surface_y: i32,
    max_surface_y: i32,
    min_slope: f64,
    max_slope: f64,
    min_exposure: f64,
    max_exposure: f64,
    surface_y_p10: i32,
    surface_y_p50: i32,
    surface_y_p90: i32,
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
    ocean_floor_columns: usize,
    beach_surface_columns: usize,
    grass_soil_columns: usize,
    exposed_stone_columns: usize,
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
        let mut min_slope = f64::INFINITY;
        let mut max_slope = f64::NEG_INFINITY;
        let mut min_exposure = f64::INFINITY;
        let mut max_exposure = f64::NEG_INFINITY;
        let mut heights = Vec::with_capacity(samples.len());
        let mut slopes = Vec::with_capacity(samples.len());
        let mut water_columns = 0;
        let mut shore_columns = 0;
        let mut dry_land_columns = 0;
        let mut ocean_biome_columns = 0;
        let mut beach_biome_columns = 0;
        let mut open_lowland_biome_columns = 0;
        let mut wooded_upland_biome_columns = 0;
        let mut ocean_floor_columns = 0;
        let mut beach_surface_columns = 0;
        let mut grass_soil_columns = 0;
        let mut exposed_stone_columns = 0;
        let mut mountain_region_columns = 0;
        let mut mountain_valley_columns = 0;
        let mut mountain_crest_columns = 0;
        let mut highland_columns = 0;
        let mut summit_columns = 0;
        let mut fingerprint = 0xcbf2_9ce4_8422_2325_u64;
        let mut foundation_field_fingerprint = 0xcbf2_9ce4_8422_2325_u64;
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
            min_slope = min_slope.min(landform.slope);
            max_slope = max_slope.max(landform.slope);
            min_exposure = min_exposure.min(landform.exposure());
            max_exposure = max_exposure.max(landform.exposure());
            heights.push(sample.surface_y);
            slopes.push(landform.slope);
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
                _ => {}
            }
            let surface_recipe = mclone_overworld_surface_recipe(*landform);
            match surface_recipe {
                McloneOverworldSurfaceRecipe::OceanFloor => ocean_floor_columns += 1,
                McloneOverworldSurfaceRecipe::Beach => beach_surface_columns += 1,
                McloneOverworldSurfaceRecipe::GrassSoil => grass_soil_columns += 1,
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
        }
        heights.sort_unstable();
        slopes.sort_by(f64::total_cmp);

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
            min_surface_y: heights[0],
            max_surface_y: heights[heights.len() - 1],
            min_slope,
            max_slope,
            min_exposure,
            max_exposure,
            surface_y_p10: percentile(&heights, 10),
            surface_y_p50: percentile(&heights, 50),
            surface_y_p90: percentile(&heights, 90),
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
            ocean_floor_columns,
            beach_surface_columns,
            grass_soil_columns,
            exposed_stone_columns,
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
            terrain_language_fingerprint,
        }
    }
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

fn slope_color(sample: McloneOverworldLandformSample) -> [u8; 4] {
    lerp_color([32, 65, 84, 255], [239, 223, 190, 255], sample.slope / 1.5)
}

fn biome_color(sample: McloneOverworldLandformSample) -> [u8; 4] {
    match mclone_overworld_biome_id_for_sample(sample) {
        OCEAN_BIOME_ID => [25, 76, 145, 255],
        BEACH_BIOME_ID => [222, 207, 143, 255],
        PLAINS_BIOME_ID => [112, 176, 76, 255],
        MCLONE_OVERWORLD_FOREST_BIOME_ID => [42, 105, 55, 255],
        _ => [211, 64, 198, 255],
    }
}

fn surface_recipe_color(sample: McloneOverworldLandformSample) -> [u8; 4] {
    match mclone_overworld_surface_recipe(sample) {
        McloneOverworldSurfaceRecipe::OceanFloor => [104, 111, 119, 255],
        McloneOverworldSurfaceRecipe::Beach => [222, 207, 143, 255],
        McloneOverworldSurfaceRecipe::GrassSoil => [91, 151, 67, 255],
        McloneOverworldSurfaceRecipe::ExposedStone => [137, 137, 137, 255],
    }
}

fn surface_recipe_tag(recipe: McloneOverworldSurfaceRecipe) -> u8 {
    match recipe {
        McloneOverworldSurfaceRecipe::OceanFloor => 0,
        McloneOverworldSurfaceRecipe::Beach => 1,
        McloneOverworldSurfaceRecipe::GrassSoil => 2,
        McloneOverworldSurfaceRecipe::ExposedStone => 3,
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
