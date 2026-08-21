use std::time::{Duration, Instant};

use mclone_core::ChunkPos;
use mclone_worldgen::levelgen::{
    ContinentalCandidateExactGenerator, ContinentalCandidateFeatureDependencyCache,
    McloneOverworldFeatureDependencyCache, McloneOverworldSamplingTopology,
    generate_mclone_overworld_surface_chunks_with_topology,
};
use serde::Serialize;

const DEFAULT_SEED: i64 = 12_345;
const DEFAULT_CENTER_X: i32 = 0;
const DEFAULT_CENTER_Z: i32 = 0;
const DEFAULT_RADIUS: i32 = 2;
const DEFAULT_ITERATIONS: usize = 3;

fn main() {
    if let Err(error) = run() {
        eprintln!("Mclone Overworld profile performance review failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse(std::env::args().skip(1))?;
    let positions = square_positions(config.center_x, config.center_z, config.radius);
    let receipt = ProfileComparisonReceipt {
        schema_revision: "mclone-overworld-profile-performance-v1",
        seed: config.seed,
        center_chunk_x: config.center_x,
        center_chunk_z: config.center_z,
        radius_chunks: config.radius,
        target_chunks: positions.len(),
        iterations: config.iterations,
        v1: ProfileReceipt {
            profile: "mclone-overworld-v1",
            surface: run_v1_surface(config, &positions),
            exact_cold: run_v1_exact(config, &positions, CacheMode::Cold),
            exact_warm: run_v1_exact(config, &positions, CacheMode::Warm),
        },
        v2: ProfileReceipt {
            profile: "mclone-overworld-v2",
            surface: run_v2_surface(config, &positions),
            exact_cold: run_v2_exact(config, &positions, CacheMode::Cold),
            exact_warm: run_v2_exact(config, &positions, CacheMode::Warm),
        },
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&receipt)
            .map_err(|error| format!("serialize performance receipt: {error}"))?
    );
    Ok(())
}

#[derive(Clone, Copy)]
struct Config {
    seed: i64,
    center_x: i32,
    center_z: i32,
    radius: i32,
    iterations: usize,
}

impl Config {
    fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut config = Self {
            seed: DEFAULT_SEED,
            center_x: DEFAULT_CENTER_X,
            center_z: DEFAULT_CENTER_Z,
            radius: DEFAULT_RADIUS,
            iterations: DEFAULT_ITERATIONS,
        };
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--seed" => config.seed = parse_next(&mut arguments, "--seed")?,
                "--center-chunk-x" => {
                    config.center_x = parse_next(&mut arguments, "--center-chunk-x")?
                }
                "--center-chunk-z" => {
                    config.center_z = parse_next(&mut arguments, "--center-chunk-z")?
                }
                "--radius" | "--radius-chunks" => {
                    config.radius = parse_next(&mut arguments, "--radius")?
                }
                "--iterations" => config.iterations = parse_next(&mut arguments, "--iterations")?,
                "--help" | "-h" => return Err(usage()),
                _ => return Err(format!("unknown argument `{argument}`\n{}", usage())),
            }
        }
        if !(0..=16).contains(&config.radius) {
            return Err("--radius must be between 0 and 16".to_owned());
        }
        if !(1..=1_000).contains(&config.iterations) {
            return Err("--iterations must be between 1 and 1000".to_owned());
        }
        Ok(config)
    }
}

#[derive(Clone, Copy)]
enum CacheMode {
    Cold,
    Warm,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileComparisonReceipt {
    schema_revision: &'static str,
    seed: i64,
    center_chunk_x: i32,
    center_chunk_z: i32,
    radius_chunks: i32,
    target_chunks: usize,
    iterations: usize,
    v1: ProfileReceipt,
    v2: ProfileReceipt,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileReceipt {
    profile: &'static str,
    surface: PhaseReceipt,
    exact_cold: PhaseReceipt,
    exact_warm: PhaseReceipt,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhaseReceipt {
    elapsed_ms: f64,
    generated_target_chunks: usize,
    non_air_blocks: usize,
    requested_dependency_chunks: usize,
    cache_hits: usize,
    generated_dependency_chunks: usize,
    retained_dependency_chunks: usize,
}

fn run_v1_surface(config: Config, positions: &[ChunkPos]) -> PhaseReceipt {
    let start = Instant::now();
    let mut receipt = PhaseReceipt::default();
    for _ in 0..config.iterations {
        let chunks = generate_mclone_overworld_surface_chunks_with_topology(
            config.seed,
            McloneOverworldSamplingTopology::Unbounded,
            positions.iter().copied(),
        );
        receipt.generated_target_chunks += chunks.len();
        receipt.non_air_blocks += chunks
            .values()
            .map(|chunk| chunk.non_air_block_count())
            .sum::<usize>();
    }
    receipt.elapsed_ms = elapsed_ms(start.elapsed());
    receipt
}

fn run_v2_surface(config: Config, positions: &[ChunkPos]) -> PhaseReceipt {
    let start = Instant::now();
    let mut receipt = PhaseReceipt::default();
    for _ in 0..config.iterations {
        let generator = ContinentalCandidateExactGenerator::new(config.seed);
        for position in positions {
            let chunk = generator.generate_surface_chunk(position.x, position.z);
            receipt.generated_target_chunks += 1;
            receipt.non_air_blocks += chunk.non_air_block_count();
        }
    }
    receipt.elapsed_ms = elapsed_ms(start.elapsed());
    receipt
}

fn run_v1_exact(config: Config, positions: &[ChunkPos], mode: CacheMode) -> PhaseReceipt {
    let start;
    let mut receipt = PhaseReceipt::default();
    match mode {
        CacheMode::Cold => {
            start = Instant::now();
            for _ in 0..config.iterations {
                let mut cache = McloneOverworldFeatureDependencyCache::new();
                let result = cache.generate_features_chunks_with_topology_and_dependencies(
                    config.seed,
                    McloneOverworldSamplingTopology::Unbounded,
                    positions.iter().copied(),
                    std::iter::empty(),
                );
                receipt.generated_target_chunks += result.chunks.len();
                receipt.non_air_blocks += result
                    .chunks
                    .values()
                    .map(|chunk| chunk.non_air_block_count())
                    .sum::<usize>();
                receipt.requested_dependency_chunks +=
                    result.cache_report.requested_dependency_chunks;
                receipt.cache_hits += result.cache_report.cache_hits;
                receipt.generated_dependency_chunks +=
                    result.cache_report.generated_dependency_chunks;
                receipt.retained_dependency_chunks = receipt
                    .retained_dependency_chunks
                    .max(result.cache_report.retained_dependency_chunks);
            }
        }
        CacheMode::Warm => {
            let mut cache = McloneOverworldFeatureDependencyCache::new();
            let _ = cache.generate_features_chunks_with_topology_and_dependencies(
                config.seed,
                McloneOverworldSamplingTopology::Unbounded,
                positions.iter().copied(),
                std::iter::empty(),
            );
            start = Instant::now();
            for _ in 0..config.iterations {
                let result = cache.generate_features_chunks_with_topology_and_dependencies(
                    config.seed,
                    McloneOverworldSamplingTopology::Unbounded,
                    positions.iter().copied(),
                    std::iter::empty(),
                );
                receipt.generated_target_chunks += result.chunks.len();
                receipt.non_air_blocks += result
                    .chunks
                    .values()
                    .map(|chunk| chunk.non_air_block_count())
                    .sum::<usize>();
                receipt.requested_dependency_chunks +=
                    result.cache_report.requested_dependency_chunks;
                receipt.cache_hits += result.cache_report.cache_hits;
                receipt.generated_dependency_chunks +=
                    result.cache_report.generated_dependency_chunks;
                receipt.retained_dependency_chunks = receipt
                    .retained_dependency_chunks
                    .max(result.cache_report.retained_dependency_chunks);
            }
        }
    }
    receipt.elapsed_ms = elapsed_ms(start.elapsed());
    receipt
}

fn run_v2_exact(config: Config, positions: &[ChunkPos], mode: CacheMode) -> PhaseReceipt {
    let start;
    let mut receipt = PhaseReceipt::default();
    match mode {
        CacheMode::Cold => {
            start = Instant::now();
            for _ in 0..config.iterations {
                let mut cache = ContinentalCandidateFeatureDependencyCache::new(config.seed);
                run_v2_exact_iteration(&mut cache, positions, &mut receipt);
            }
        }
        CacheMode::Warm => {
            let mut cache = ContinentalCandidateFeatureDependencyCache::new(config.seed);
            let mut warmup = PhaseReceipt::default();
            run_v2_exact_iteration(&mut cache, positions, &mut warmup);
            start = Instant::now();
            for _ in 0..config.iterations {
                run_v2_exact_iteration(&mut cache, positions, &mut receipt);
            }
        }
    }
    receipt.elapsed_ms = elapsed_ms(start.elapsed());
    receipt
}

fn run_v2_exact_iteration(
    cache: &mut ContinentalCandidateFeatureDependencyCache,
    positions: &[ChunkPos],
    receipt: &mut PhaseReceipt,
) {
    let (chunks, report) = cache.generate_features_chunks(positions.iter().copied());
    receipt.generated_target_chunks += chunks.len();
    receipt.non_air_blocks += chunks
        .values()
        .map(|chunk| chunk.non_air_block_count())
        .sum::<usize>();
    receipt.requested_dependency_chunks += report.requested_dependency_chunks;
    receipt.cache_hits += report.cache_hits;
    receipt.generated_dependency_chunks += report.generated_dependency_chunks;
    receipt.retained_dependency_chunks = receipt
        .retained_dependency_chunks
        .max(report.retained_dependency_chunks);
}

fn square_positions(center_x: i32, center_z: i32, radius: i32) -> Vec<ChunkPos> {
    let mut positions = Vec::with_capacity(((radius * 2 + 1) * (radius * 2 + 1)) as usize);
    for offset_z in -radius..=radius {
        for offset_x in -radius..=radius {
            positions.push(ChunkPos::new(center_x + offset_x, center_z + offset_z));
        }
    }
    positions
}

fn parse_next<T: std::str::FromStr>(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<T, String> {
    let value = arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))?;
    value
        .parse()
        .map_err(|_| format!("invalid value `{value}` for {option}"))
}

fn elapsed_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn usage() -> String {
    "usage: mclone_overworld_profile_perf [--seed N] [--center-chunk-x N] \
     [--center-chunk-z N] [--radius N] [--iterations N]"
        .to_owned()
}
