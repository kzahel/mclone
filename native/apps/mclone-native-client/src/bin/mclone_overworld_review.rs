use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use image::RgbaImage;
use mclone_worldgen::levelgen::{
    MCLONE_OVERWORLD_FIELD_REVISION, MCLONE_OVERWORLD_SEA_LEVEL,
    McloneOverworldSampleRegionRequest, McloneOverworldSampler, McloneOverworldTerrainSample,
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

    let sample_start = Instant::now();
    let region = McloneOverworldSampler::new(config.seed)
        .sample_region(request)
        .map_err(anyhow::Error::msg)?;
    let sample_elapsed_ms = sample_start.elapsed().as_secs_f64() * 1_000.0;
    let facts = RegionFacts::from_samples(&region.samples, request.width, request.depth);

    let prefix = format!(
        "mclone-overworld-v1-seed-{}-chunk-{}-{}",
        config.seed, config.chunk_x, config.chunk_z
    );
    let continentalness_path = config
        .output_dir
        .join(format!("{prefix}-continentalness.png"));
    let relief_path = config.output_dir.join(format!("{prefix}-relief.png"));
    let surface_path = config.output_dir.join(format!("{prefix}-surface-y.png"));
    let fields_path = config.output_dir.join(format!("{prefix}-fields.png"));
    let receipt_path = config.output_dir.join(format!("{prefix}-fields.json"));

    let continentalness = render_map(&region.samples, continentalness_color);
    let relief = render_map(&region.samples, relief_color);
    let surface = render_map(&region.samples, surface_color);
    save_rgba(
        &continentalness_path,
        request.width,
        request.depth,
        &continentalness,
    )?;
    save_rgba(&relief_path, request.width, request.depth, &relief)?;
    save_rgba(&surface_path, request.width, request.depth, &surface)?;
    let combined = combine_maps(
        request.width,
        request.depth,
        [&continentalness, &relief, &surface],
    );
    save_rgba(
        &fields_path,
        request.width * 3 + MAP_GAP_PIXELS * 2,
        request.depth,
        &combined,
    )?;

    let (commit, dirty) = git_state();
    let receipt = serde_json::json!({
        "schema": 1,
        "profile": "mclone-overworld-v1",
        "fieldRevision": MCLONE_OVERWORLD_FIELD_REVISION,
        "commit": commit,
        "dirty": dirty,
        "seed": config.seed,
        "centerChunk": [config.chunk_x, config.chunk_z],
        "centerBlock": [center_x, center_z],
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
        },
        "ranges": {
            "continentalness": [facts.min_continentalness, facts.max_continentalness],
            "relief": [facts.min_relief, facts.max_relief],
            "surfaceY": [facts.min_surface_y, facts.max_surface_y],
        },
        "surfaceYPercentiles": {
            "p10": facts.surface_y_p10,
            "p50": facts.surface_y_p50,
            "p90": facts.surface_y_p90,
        },
        "columnCounts": {
            "water": facts.water_columns,
            "shore": facts.shore_columns,
            "dryLand": facts.dry_land_columns,
        },
        "slopeEdges": {
            "total": facts.slope_edges,
            "atLeastOneBlock": facts.slope_at_least_one,
            "atLeastThreeBlocks": facts.slope_at_least_three,
        },
        "sampleFingerprint": facts.fingerprint,
        "maps": {
            "order": ["continentalness", "relief", "surfaceY"],
            "combined": fields_path,
            "continentalness": continentalness_path,
            "relief": relief_path,
            "surfaceY": surface_path,
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
    min_surface_y: i32,
    max_surface_y: i32,
    surface_y_p10: i32,
    surface_y_p50: i32,
    surface_y_p90: i32,
    water_columns: usize,
    shore_columns: usize,
    dry_land_columns: usize,
    slope_edges: usize,
    slope_at_least_one: usize,
    slope_at_least_three: usize,
    fingerprint: u64,
}

impl RegionFacts {
    fn from_samples(samples: &[McloneOverworldTerrainSample], width: u32, depth: u32) -> Self {
        let mut min_continentalness = f64::INFINITY;
        let mut max_continentalness = f64::NEG_INFINITY;
        let mut min_relief = f64::INFINITY;
        let mut max_relief = f64::NEG_INFINITY;
        let mut heights = Vec::with_capacity(samples.len());
        let mut water_columns = 0;
        let mut shore_columns = 0;
        let mut dry_land_columns = 0;
        let mut fingerprint = 0xcbf2_9ce4_8422_2325_u64;
        for sample in samples {
            min_continentalness = min_continentalness.min(sample.continentalness);
            max_continentalness = max_continentalness.max(sample.continentalness);
            min_relief = min_relief.min(sample.relief);
            max_relief = max_relief.max(sample.relief);
            heights.push(sample.surface_y);
            if sample.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL - 2 {
                water_columns += 1;
            } else if sample.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 3 {
                shore_columns += 1;
            } else {
                dry_land_columns += 1;
            }
            for byte in sample
                .continentalness
                .to_bits()
                .to_le_bytes()
                .into_iter()
                .chain(sample.relief.to_bits().to_le_bytes())
                .chain(sample.surface_y.to_le_bytes())
            {
                fingerprint ^= u64::from(byte);
                fingerprint = fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        heights.sort_unstable();

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
                        samples[index].surface_y,
                        samples[index + 1].surface_y,
                        &mut slope_edges,
                        &mut slope_at_least_one,
                        &mut slope_at_least_three,
                    );
                }
                if z + 1 < depth {
                    add_slope(
                        samples[index].surface_y,
                        samples[index + width].surface_y,
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
            min_surface_y: heights[0],
            max_surface_y: heights[heights.len() - 1],
            surface_y_p10: percentile(&heights, 10),
            surface_y_p50: percentile(&heights, 50),
            surface_y_p90: percentile(&heights, 90),
            water_columns,
            shore_columns,
            dry_land_columns,
            slope_edges,
            slope_at_least_one,
            slope_at_least_three,
            fingerprint,
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

fn render_map(
    samples: &[McloneOverworldTerrainSample],
    color: fn(McloneOverworldTerrainSample) -> [u8; 4],
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
            f64::from(y - 67) / 29.0,
        ),
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

fn combine_maps(width: u32, height: u32, maps: [&[u8]; 3]) -> Vec<u8> {
    let combined_width = width * 3 + MAP_GAP_PIXELS * 2;
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
