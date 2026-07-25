use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mclone_core::ChunkPos;
use mclone_worldgen::feature::DecorationStep;
use mclone_worldgen::levelgen::{
    McloneOverworldFeatureDependencyCache, McloneOverworldFeatureDependencyCacheReport,
    McloneOverworldSamplingTopology, OverworldDependencyGenerationTiming,
    OverworldFeatureBatchTiming, OverworldFeatureDependencyCache,
    OverworldFeatureDependencyCacheReport, SurfaceFillTiming,
    generate_mclone_overworld_surface_chunks_with_topology, generate_overworld_surface_chunk,
};

const DEFAULT_SEED: i64 = 12_345;
const DEFAULT_CHUNK_X: i32 = 0;
const DEFAULT_CHUNK_Z: i32 = 0;
const DEFAULT_RADIUS: i32 = 1;
const DEFAULT_ITERATIONS: usize = 1;
const MAX_RADIUS: i32 = 16;
const MAX_ITERATIONS: usize = 10_000;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse(std::env::args().skip(1))?;
    let positions = square_positions(config.chunk_x, config.chunk_z, config.radius);
    let total_start = Instant::now();
    let surface = run_surface_phase(&config, &positions);
    let mclone_overworld_surface = run_mclone_overworld_surface_phase(&config, &positions);
    let mclone_features_cold =
        run_mclone_features_phase(&config, &positions, FeatureCacheMode::Cold);
    let mclone_features_warm =
        run_mclone_features_phase(&config, &positions, FeatureCacheMode::Warm);
    let features_cold = run_features_phase(&config, &positions, FeatureCacheMode::Cold);
    let features_warm = run_features_phase(&config, &positions, FeatureCacheMode::Warm);
    let total_elapsed_ms = elapsed_ms(total_start.elapsed());
    print_json(
        &config,
        positions.len(),
        total_elapsed_ms,
        &surface,
        &mclone_overworld_surface,
        &mclone_features_cold,
        &mclone_features_warm,
        &features_cold,
        &features_warm,
    );
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct Config {
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    radius: i32,
    iterations: usize,
    mclone_topology: McloneOverworldSamplingTopology,
}

impl Config {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut config = Self {
            seed: DEFAULT_SEED,
            chunk_x: DEFAULT_CHUNK_X,
            chunk_z: DEFAULT_CHUNK_Z,
            radius: DEFAULT_RADIUS,
            iterations: DEFAULT_ITERATIONS,
            mclone_topology: McloneOverworldSamplingTopology::Unbounded,
        };

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--seed" => config.seed = parse_next(&mut args, "--seed")?,
                "--chunk-x" => config.chunk_x = parse_next(&mut args, "--chunk-x")?,
                "--chunk-z" => config.chunk_z = parse_next(&mut args, "--chunk-z")?,
                "--radius" | "--radius-chunks" => {
                    config.radius = parse_next(&mut args, "--radius")?
                }
                "--iterations" => {
                    config.iterations = parse_next(&mut args, "--iterations")?;
                }
                "--mclone-topology" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--mclone-topology requires a value".to_owned())?;
                    config.mclone_topology = match value.as_str() {
                        "plane" => McloneOverworldSamplingTopology::Unbounded,
                        "cylinder-x:384" => McloneOverworldSamplingTopology::PeriodicX,
                        _ => {
                            return Err(format!(
                                "--mclone-topology requires plane or cylinder-x:384; got `{value}`"
                            ));
                        }
                    };
                }
                "--help" | "-h" => return Err(usage()),
                _ => return Err(format!("unknown argument {arg}\n{}", usage())),
            }
        }

        if !(0..=MAX_RADIUS).contains(&config.radius) {
            return Err(format!("--radius must be between 0 and {MAX_RADIUS}"));
        }
        if !(1..=MAX_ITERATIONS).contains(&config.iterations) {
            return Err(format!(
                "--iterations must be between 1 and {MAX_ITERATIONS}"
            ));
        }

        Ok(config)
    }
}

#[derive(Clone, Copy, Debug)]
enum FeatureCacheMode {
    Cold,
    Warm,
}

#[derive(Clone, Debug)]
struct SurfacePhaseReport {
    elapsed_ms: f64,
    generated_chunks: usize,
    non_air_blocks: usize,
}

#[derive(Clone, Debug)]
struct FeaturePhaseReport {
    elapsed_ms: f64,
    generated_target_chunks: usize,
    non_air_blocks: usize,
    cache_report: OverworldFeatureDependencyCacheReport,
    timing: OverworldFeatureBatchTiming,
}

#[derive(Clone, Debug)]
struct McloneFeaturePhaseReport {
    elapsed_ms: f64,
    generated_target_chunks: usize,
    non_air_blocks: usize,
    cache_report: McloneOverworldFeatureDependencyCacheReport,
}

fn run_surface_phase(config: &Config, positions: &[ChunkPos]) -> SurfacePhaseReport {
    let start = Instant::now();
    let mut generated_chunks = 0_usize;
    let mut non_air_blocks = 0_usize;
    for _ in 0..config.iterations {
        for pos in positions {
            let chunk = generate_overworld_surface_chunk(config.seed, pos.x, pos.z);
            generated_chunks += 1;
            non_air_blocks += chunk.non_air_block_count();
        }
    }
    SurfacePhaseReport {
        elapsed_ms: elapsed_ms(start.elapsed()),
        generated_chunks,
        non_air_blocks,
    }
}

fn run_mclone_overworld_surface_phase(
    config: &Config,
    positions: &[ChunkPos],
) -> SurfacePhaseReport {
    let start = Instant::now();
    let mut generated_chunks = 0_usize;
    let mut non_air_blocks = 0_usize;
    for _ in 0..config.iterations {
        let chunks = generate_mclone_overworld_surface_chunks_with_topology(
            config.seed,
            config.mclone_topology,
            positions.iter().copied(),
        );
        generated_chunks += chunks.len();
        non_air_blocks += chunks
            .values()
            .map(|chunk| chunk.non_air_block_count())
            .sum::<usize>();
    }
    SurfacePhaseReport {
        elapsed_ms: elapsed_ms(start.elapsed()),
        generated_chunks,
        non_air_blocks,
    }
}

fn run_mclone_features_phase(
    config: &Config,
    positions: &[ChunkPos],
    mode: FeatureCacheMode,
) -> McloneFeaturePhaseReport {
    let mut start = Instant::now();
    let mut generated_target_chunks = 0_usize;
    let mut non_air_blocks = 0_usize;
    let mut cache_report = McloneOverworldFeatureDependencyCacheReport::default();

    match mode {
        FeatureCacheMode::Cold => {
            for _ in 0..config.iterations {
                let mut cache = McloneOverworldFeatureDependencyCache::new();
                let result = cache.generate_features_chunks_with_topology_and_dependencies(
                    config.seed,
                    config.mclone_topology,
                    positions.iter().copied(),
                    std::iter::empty(),
                );
                generated_target_chunks += result.chunks.len();
                non_air_blocks += result
                    .chunks
                    .values()
                    .map(|chunk| chunk.non_air_block_count())
                    .sum::<usize>();
                add_mclone_cache_report(&mut cache_report, result.cache_report);
            }
        }
        FeatureCacheMode::Warm => {
            let mut cache = McloneOverworldFeatureDependencyCache::new();
            let _warmup = cache.generate_features_chunks_with_topology_and_dependencies(
                config.seed,
                config.mclone_topology,
                positions.iter().copied(),
                std::iter::empty(),
            );
            start = Instant::now();
            for _ in 0..config.iterations {
                let result = cache.generate_features_chunks_with_topology_and_dependencies(
                    config.seed,
                    config.mclone_topology,
                    positions.iter().copied(),
                    std::iter::empty(),
                );
                generated_target_chunks += result.chunks.len();
                non_air_blocks += result
                    .chunks
                    .values()
                    .map(|chunk| chunk.non_air_block_count())
                    .sum::<usize>();
                add_mclone_cache_report(&mut cache_report, result.cache_report);
            }
        }
    }

    McloneFeaturePhaseReport {
        elapsed_ms: elapsed_ms(start.elapsed()),
        generated_target_chunks,
        non_air_blocks,
        cache_report,
    }
}

fn run_features_phase(
    config: &Config,
    positions: &[ChunkPos],
    mode: FeatureCacheMode,
) -> FeaturePhaseReport {
    let mut start = Instant::now();
    let mut generated_target_chunks = 0_usize;
    let mut non_air_blocks = 0_usize;
    let mut cache_report = OverworldFeatureDependencyCacheReport::default();
    let mut timing = OverworldFeatureBatchTiming::default();

    match mode {
        FeatureCacheMode::Cold => {
            for _ in 0..config.iterations {
                let mut cache = OverworldFeatureDependencyCache::new();
                let result = cache.generate_features_chunks(config.seed, positions.iter().copied());
                generated_target_chunks += result.chunks.len();
                non_air_blocks += result
                    .chunks
                    .values()
                    .map(|chunk| chunk.non_air_block_count())
                    .sum::<usize>();
                add_cache_report(&mut cache_report, result.cache_report);
                timing.add_assign(result.timing);
            }
        }
        FeatureCacheMode::Warm => {
            let mut cache = OverworldFeatureDependencyCache::new();
            let _warmup = cache.generate_features_chunks(config.seed, positions.iter().copied());
            start = Instant::now();
            for _ in 0..config.iterations {
                let result = cache.generate_features_chunks(config.seed, positions.iter().copied());
                generated_target_chunks += result.chunks.len();
                non_air_blocks += result
                    .chunks
                    .values()
                    .map(|chunk| chunk.non_air_block_count())
                    .sum::<usize>();
                add_cache_report(&mut cache_report, result.cache_report);
                timing.add_assign(result.timing);
            }
        }
    }

    FeaturePhaseReport {
        elapsed_ms: elapsed_ms(start.elapsed()),
        generated_target_chunks,
        non_air_blocks,
        cache_report,
        timing,
    }
}

fn add_cache_report(
    target: &mut OverworldFeatureDependencyCacheReport,
    source: OverworldFeatureDependencyCacheReport,
) {
    target.requested_dependency_chunks += source.requested_dependency_chunks;
    target.cache_hits += source.cache_hits;
    target.generated_dependency_chunks += source.generated_dependency_chunks;
    target.retained_dependency_chunks += source.retained_dependency_chunks;
}

fn add_mclone_cache_report(
    target: &mut McloneOverworldFeatureDependencyCacheReport,
    source: McloneOverworldFeatureDependencyCacheReport,
) {
    target.requested_dependency_chunks += source.requested_dependency_chunks;
    target.cache_hits += source.cache_hits;
    target.generated_dependency_chunks += source.generated_dependency_chunks;
    target.retained_dependency_chunks += source.retained_dependency_chunks;
    // Stream planner counters describe the long-lived cache and are
    // cumulative. Keep the latest observation rather than summing repeated
    // snapshots of the same cache.
    target.stream_plan_requests = source.stream_plan_requests;
    target.stream_plan_cache_hits = source.stream_plan_cache_hits;
    target.stream_plan_cache_misses = source.stream_plan_cache_misses;
    target.stream_intersection_requests = source.stream_intersection_requests;
    target.stream_intersection_cache_hits = source.stream_intersection_cache_hits;
    target.retained_stream_intersection_queries = source.retained_stream_intersection_queries;
    target.accepted_stream_plans = source.accepted_stream_plans;
    target.rejected_stream_candidates = source.rejected_stream_candidates;
    target.vegetation_cell_requests = source.vegetation_cell_requests;
    target.vegetation_cell_cache_hits = source.vegetation_cell_cache_hits;
    target.vegetation_cell_cache_misses = source.vegetation_cell_cache_misses;
    target.retained_vegetation_cells = source.retained_vegetation_cells;
    target.retained_preliminary_tree_candidates = source.retained_preliminary_tree_candidates;
}

fn print_json(
    config: &Config,
    target_chunk_count: usize,
    total_elapsed_ms: f64,
    surface: &SurfacePhaseReport,
    mclone_overworld_surface: &SurfacePhaseReport,
    mclone_features_cold: &McloneFeaturePhaseReport,
    mclone_features_warm: &McloneFeaturePhaseReport,
    features_cold: &FeaturePhaseReport,
    features_warm: &FeaturePhaseReport,
) {
    println!("{{");
    print_benchmark_metadata("native_worldgen", "  ", true);
    println!("  \"seed\": {},", config.seed);
    println!(
        "  \"origin\": {{ \"x\": {}, \"z\": {} }},",
        config.chunk_x, config.chunk_z
    );
    println!("  \"radius_chunks\": {},", config.radius);
    println!("  \"target_chunks\": {target_chunk_count},");
    println!("  \"iterations\": {},", config.iterations);
    println!(
        "  \"mclone_topology\": \"{}\",",
        match config.mclone_topology {
            McloneOverworldSamplingTopology::Unbounded => "plane",
            McloneOverworldSamplingTopology::PeriodicX => "cylinder-x:384",
        }
    );
    println!("  \"total_elapsed_ms\": {:.3},", total_elapsed_ms);
    println!("  \"phases\": {{");
    print_surface_phase_json("    ", "surface", surface, true);
    print_surface_phase_json(
        "    ",
        "mclone_overworld_v1_surface",
        mclone_overworld_surface,
        true,
    );
    print_mclone_feature_phase_json(
        "    ",
        "mclone_overworld_v1_features_cold",
        mclone_features_cold,
        true,
    );
    print_mclone_feature_phase_json(
        "    ",
        "mclone_overworld_v1_features_warm",
        mclone_features_warm,
        true,
    );
    print_feature_phase_json("    ", "features_cold", features_cold, true);
    print_feature_phase_json("    ", "features_warm", features_warm, false);
    println!("  }}");
    println!("}}");
}

fn print_mclone_feature_phase_json(
    indent: &str,
    name: &str,
    report: &McloneFeaturePhaseReport,
    trailing_comma: bool,
) {
    println!("{indent}\"{name}\": {{");
    println!("{indent}  \"elapsed_ms\": {:.3},", report.elapsed_ms);
    println!(
        "{indent}  \"target_chunks\": {},",
        report.generated_target_chunks
    );
    println!(
        "{indent}  \"target_chunks_per_second\": {:.3},",
        chunks_per_second(report.generated_target_chunks, report.elapsed_ms)
    );
    println!("{indent}  \"non_air_blocks\": {},", report.non_air_blocks);
    print_mclone_cache_report_json(&format!("{indent}  "), report.cache_report, false);
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn print_surface_phase_json(
    indent: &str,
    name: &str,
    report: &SurfacePhaseReport,
    trailing_comma: bool,
) {
    println!("{indent}\"{name}\": {{");
    println!("{indent}  \"elapsed_ms\": {:.3},", report.elapsed_ms);
    println!(
        "{indent}  \"generated_chunks\": {},",
        report.generated_chunks
    );
    println!(
        "{indent}  \"chunks_per_second\": {:.3},",
        chunks_per_second(report.generated_chunks, report.elapsed_ms)
    );
    println!("{indent}  \"non_air_blocks\": {}", report.non_air_blocks);
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn print_feature_phase_json(
    indent: &str,
    name: &str,
    report: &FeaturePhaseReport,
    trailing_comma: bool,
) {
    println!("{indent}\"{name}\": {{");
    println!("{indent}  \"elapsed_ms\": {:.3},", report.elapsed_ms);
    println!(
        "{indent}  \"target_chunks\": {},",
        report.generated_target_chunks
    );
    println!(
        "{indent}  \"target_chunks_per_second\": {:.3},",
        chunks_per_second(report.generated_target_chunks, report.elapsed_ms)
    );
    println!("{indent}  \"non_air_blocks\": {},", report.non_air_blocks);
    print_cache_report_json(&format!("{indent}  "), report.cache_report, true);
    print_feature_timing_json(&format!("{indent}  "), report.timing, false);
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn print_cache_report_json(
    indent: &str,
    report: OverworldFeatureDependencyCacheReport,
    trailing_comma: bool,
) {
    println!("{indent}\"cache\": {{");
    println!(
        "{indent}  \"requested_dependency_chunks\": {},",
        report.requested_dependency_chunks
    );
    println!("{indent}  \"cache_hits\": {},", report.cache_hits);
    println!(
        "{indent}  \"generated_dependency_chunks\": {},",
        report.generated_dependency_chunks
    );
    println!(
        "{indent}  \"retained_dependency_chunks\": {}",
        report.retained_dependency_chunks
    );
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn print_mclone_cache_report_json(
    indent: &str,
    report: McloneOverworldFeatureDependencyCacheReport,
    trailing_comma: bool,
) {
    println!("{indent}\"cache\": {{");
    println!(
        "{indent}  \"requested_dependency_chunks\": {},",
        report.requested_dependency_chunks
    );
    println!("{indent}  \"cache_hits\": {},", report.cache_hits);
    println!(
        "{indent}  \"generated_dependency_chunks\": {},",
        report.generated_dependency_chunks
    );
    println!(
        "{indent}  \"retained_dependency_chunks\": {},",
        report.retained_dependency_chunks
    );
    println!(
        "{indent}  \"stream_plan_requests\": {},",
        report.stream_plan_requests
    );
    println!(
        "{indent}  \"stream_plan_cache_hits\": {},",
        report.stream_plan_cache_hits
    );
    println!(
        "{indent}  \"stream_plan_cache_misses\": {},",
        report.stream_plan_cache_misses
    );
    println!(
        "{indent}  \"stream_intersection_requests\": {},",
        report.stream_intersection_requests
    );
    println!(
        "{indent}  \"stream_intersection_cache_hits\": {},",
        report.stream_intersection_cache_hits
    );
    println!(
        "{indent}  \"retained_stream_intersection_queries\": {},",
        report.retained_stream_intersection_queries
    );
    println!(
        "{indent}  \"accepted_stream_plans\": {},",
        report.accepted_stream_plans
    );
    println!(
        "{indent}  \"rejected_stream_candidates\": {},",
        report.rejected_stream_candidates
    );
    println!(
        "{indent}  \"vegetation_cell_requests\": {},",
        report.vegetation_cell_requests
    );
    println!(
        "{indent}  \"vegetation_cell_cache_hits\": {},",
        report.vegetation_cell_cache_hits
    );
    println!(
        "{indent}  \"vegetation_cell_cache_misses\": {},",
        report.vegetation_cell_cache_misses
    );
    println!(
        "{indent}  \"retained_vegetation_cells\": {},",
        report.retained_vegetation_cells
    );
    println!(
        "{indent}  \"retained_preliminary_tree_candidates\": {}",
        report.retained_preliminary_tree_candidates
    );
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn print_feature_timing_json(
    indent: &str,
    timing: OverworldFeatureBatchTiming,
    trailing_comma: bool,
) {
    println!("{indent}\"timing\": {{");
    println!(
        "{indent}  \"total_ms\": {:.3},",
        micros_to_ms(timing.total_us())
    );
    println!(
        "{indent}  \"dependency_generate_ms\": {:.3},",
        micros_to_ms(timing.dependency_generate_us)
    );
    print_dependency_generation_json(&format!("{indent}  "), timing.dependency_generation, true);
    println!(
        "{indent}  \"feature_decoration_ms\": {:.3},",
        micros_to_ms(timing.feature_decoration_us)
    );
    print_decoration_steps_json(&format!("{indent}  "), timing, true);
    println!(
        "{indent}  \"target_extract_ms\": {:.3}",
        micros_to_ms(timing.target_extract_us)
    );
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn print_dependency_generation_json(
    indent: &str,
    timing: OverworldDependencyGenerationTiming,
    trailing_comma: bool,
) {
    println!("{indent}\"dependency_generation\": {{");
    println!(
        "{indent}  \"total_ms\": {:.3},",
        micros_to_ms(timing.total_us())
    );
    print_surface_fill_json(&format!("{indent}  "), timing.surface_fill, true);
    println!(
        "{indent}  \"surface_bedrock_ms\": {:.3},",
        micros_to_ms(timing.surface_bedrock_us)
    );
    println!(
        "{indent}  \"air_carvers_ms\": {:.3},",
        micros_to_ms(timing.air_carvers_us)
    );
    println!(
        "{indent}  \"liquid_carvers_ms\": {:.3},",
        micros_to_ms(timing.liquid_carvers_us)
    );
    println!(
        "{indent}  \"heightmap_prime_ms\": {:.3}",
        micros_to_ms(timing.heightmap_prime_us)
    );
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn print_surface_fill_json(indent: &str, timing: SurfaceFillTiming, trailing_comma: bool) {
    println!("{indent}\"surface_fill\": {{");
    println!(
        "{indent}  \"total_ms\": {:.3},",
        micros_to_ms(timing.total_us())
    );
    println!(
        "{indent}  \"noise_columns_ms\": {:.3},",
        micros_to_ms(timing.noise_columns_us)
    );
    println!(
        "{indent}  \"terrain_fill_ms\": {:.3},",
        micros_to_ms(timing.terrain_fill_us)
    );
    println!(
        "{indent}  \"non_air_blocks_written\": {}",
        timing.non_air_blocks_written
    );
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn print_decoration_steps_json(
    indent: &str,
    timing: OverworldFeatureBatchTiming,
    trailing_comma: bool,
) {
    println!("{indent}\"decoration_steps\": {{");
    for (index, step) in DecorationStep::ALL.iter().copied().enumerate() {
        let suffix = if index + 1 == DecorationStep::COUNT {
            ""
        } else {
            ","
        };
        let step_us = timing.feature_decoration_steps.step_us[step.index() as usize];
        println!(
            "{indent}  \"{}\": {:.3}{suffix}",
            step.key(),
            micros_to_ms(step_us)
        );
    }
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}}}{suffix}");
}

fn square_positions(center_x: i32, center_z: i32, radius: i32) -> Vec<ChunkPos> {
    let mut positions = Vec::new();
    for z in center_z - radius..=center_z + radius {
        for x in center_x - radius..=center_x + radius {
            positions.push(ChunkPos::new(x, z));
        }
    }
    positions
}

fn chunks_per_second(chunks: usize, elapsed_ms: f64) -> f64 {
    if elapsed_ms <= f64::EPSILON {
        0.0
    } else {
        chunks as f64 / (elapsed_ms / 1000.0)
    }
}

fn parse_next<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T, String> {
    let value = args
        .next()
        .ok_or_else(|| format!("{flag} requires a value"))?;
    value
        .parse::<T>()
        .map_err(|_| format!("{flag} received invalid value `{value}`"))
}

fn usage() -> String {
    "usage: worldgen_perf [--seed N] [--chunk-x N] [--chunk-z N] [--radius N] \
     [--iterations N] [--mclone-topology plane|cylinder-x:384]"
        .to_owned()
}

fn elapsed_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn micros_to_ms(micros: u128) -> f64 {
    micros as f64 / 1000.0
}

fn print_benchmark_metadata(name: &str, indent: &str, trailing_comma: bool) {
    println!("{indent}\"benchmark\": \"{}\",", json_escape(name));
    println!(
        "{indent}\"recorded_unix_seconds\": {},",
        current_unix_seconds()
    );
    println!(
        "{indent}\"git_commit\": \"{}\",",
        json_escape(&git_short_commit())
    );
    println!("{indent}\"git_dirty\": {},", git_dirty());
    let suffix = if trailing_comma { "," } else { "" };
    println!(
        "{indent}\"debug_assertions\": {}{suffix}",
        cfg!(debug_assertions)
    );
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn git_short_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn git_dirty() -> bool {
    Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.is_empty())
        .unwrap_or(true)
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
