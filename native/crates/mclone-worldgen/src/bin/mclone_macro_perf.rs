use std::hint::black_box;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mclone_worldgen::levelgen::{
    McloneOverworldSampler, McloneOverworldSamplingTopology, McloneOverworldTerrainSample,
};
use mclone_worldgen::terrain_preview::{
    TerrainPreviewCompileWork, TerrainPreviewContentStage, TerrainPreviewReferenceGrid,
    TerrainPreviewRequest,
};
use serde::Serialize;

const DEFAULT_SEED: i64 = 12_345;
const DEFAULT_ITERATIONS: usize = 5;
const DEFAULT_WARMUP_ITERATIONS: usize = 1;
const MAX_ITERATIONS: usize = 100;
const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

const POINT_WORKLOADS: [PointWorkload; 5] = [
    PointWorkload::new("regional-65k", 65_536, 1_024, 65),
    PointWorkload::new("horizon-131k", 131_072, 1_024, 129),
    PointWorkload::new("continental-500k-coarse", 499_712, 2_048, 245),
    PointWorkload::new("continental-500k-medium", 499_712, 1_024, 489),
    PointWorkload::new("continental-500k-fine", 499_712, 512, 977),
];

const PREVIEW_WORKLOADS: [PreviewWorkload; 4] = [
    PreviewWorkload::new("base-65k", 64, 1_024, TerrainPreviewContentStage::Base),
    PreviewWorkload::new("base-131k", 128, 1_024, TerrainPreviewContentStage::Base),
    PreviewWorkload::new(
        "surface-65k",
        64,
        1_024,
        TerrainPreviewContentStage::Surface,
    ),
    PreviewWorkload::new("cover-65k", 64, 1_024, TerrainPreviewContentStage::Cover),
];

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse(std::env::args().skip(1))?;
    let point_workloads = POINT_WORKLOADS
        .iter()
        .copied()
        .filter(|workload| config.includes(point_workload_id(workload.name)))
        .map(|workload| run_point_workload(&config, workload))
        .collect::<Result<Vec<_>, _>>()?;
    let preview_workloads = PREVIEW_WORKLOADS
        .iter()
        .copied()
        .filter(|workload| config.includes(preview_workload_id(workload.name)))
        .map(|workload| run_preview_workload(&config, workload))
        .collect::<Result<Vec<_>, _>>()?;

    if point_workloads.is_empty() && preview_workloads.is_empty() {
        return Err(format!(
            "no workloads matched --only values {:?}\n{}",
            config.only,
            usage()
        ));
    }

    let receipt = Receipt {
        schema: "mclone-macro-perf-v1",
        benchmark: "mclone_macro_terrain",
        recorded_unix_seconds: current_unix_seconds(),
        git_commit: git_commit(),
        git_dirty: git_dirty(),
        build_profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        host: Host {
            operating_system: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            logical_cpu_count: std::thread::available_parallelism()
                .ok()
                .map(std::num::NonZeroUsize::get),
        },
        seed: config.seed,
        center: Coordinates {
            x: config.center_x,
            z: config.center_z,
        },
        topology: topology_label(config.topology),
        iterations: config.iterations,
        warmup_iterations: config.warmup_iterations,
        point_workloads,
        preview_workloads,
        absent_stages: [
            "transfer",
            "gpu_execution",
            "presentation",
            "exact_chunk_generation",
            "streaming",
        ],
    };
    let json = serde_json::to_string_pretty(&receipt)
        .map_err(|error| format!("failed to serialize benchmark receipt: {error}"))?;
    if let Some(path) = config.output {
        std::fs::write(&path, format!("{json}\n"))
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    } else {
        println!("{json}");
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct Config {
    seed: i64,
    center_x: i32,
    center_z: i32,
    topology: McloneOverworldSamplingTopology,
    iterations: usize,
    warmup_iterations: usize,
    output: Option<PathBuf>,
    only: Vec<String>,
}

impl Config {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut config = Self {
            seed: DEFAULT_SEED,
            center_x: 0,
            center_z: 0,
            topology: McloneOverworldSamplingTopology::Unbounded,
            iterations: DEFAULT_ITERATIONS,
            warmup_iterations: DEFAULT_WARMUP_ITERATIONS,
            output: None,
            only: Vec::new(),
        };
        let mut args = args.into_iter();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--seed" => config.seed = parse_next(&mut args, "--seed")?,
                "--center-x" => config.center_x = parse_next(&mut args, "--center-x")?,
                "--center-z" => config.center_z = parse_next(&mut args, "--center-z")?,
                "--iterations" => config.iterations = parse_next(&mut args, "--iterations")?,
                "--warmup-iterations" => {
                    config.warmup_iterations = parse_next(&mut args, "--warmup-iterations")?;
                }
                "--output" => {
                    config.output = Some(PathBuf::from(
                        args.next().ok_or("--output requires a path")?,
                    ));
                }
                "--only" => {
                    config
                        .only
                        .push(args.next().ok_or("--only requires a workload id")?);
                }
                "--topology" => {
                    config.topology =
                        parse_topology(&args.next().ok_or("--topology requires a value")?)?;
                }
                "--help" | "-h" => return Err(usage()),
                _ => return Err(format!("unknown argument `{argument}`\n{}", usage())),
            }
        }
        if !(1..=MAX_ITERATIONS).contains(&config.iterations) {
            return Err(format!(
                "--iterations must be between 1 and {MAX_ITERATIONS}"
            ));
        }
        if config.warmup_iterations > MAX_ITERATIONS {
            return Err(format!(
                "--warmup-iterations must be between 0 and {MAX_ITERATIONS}"
            ));
        }
        Ok(config)
    }

    fn includes(&self, id: String) -> bool {
        self.only.is_empty() || self.only.iter().any(|candidate| candidate == &id)
    }
}

#[derive(Clone, Copy, Debug)]
struct PointWorkload {
    name: &'static str,
    footprint_blocks: u32,
    spacing_blocks: u32,
    samples_per_axis: u32,
}

impl PointWorkload {
    const fn new(
        name: &'static str,
        footprint_blocks: u32,
        spacing_blocks: u32,
        samples_per_axis: u32,
    ) -> Self {
        Self {
            name,
            footprint_blocks,
            spacing_blocks,
            samples_per_axis,
        }
    }

    const fn point_count(self) -> u64 {
        self.samples_per_axis as u64 * self.samples_per_axis as u64
    }
}

#[derive(Clone, Copy, Debug)]
struct PreviewWorkload {
    name: &'static str,
    cells_per_axis: u32,
    spacing_blocks: u32,
    content_stage: TerrainPreviewContentStage,
}

impl PreviewWorkload {
    const fn new(
        name: &'static str,
        cells_per_axis: u32,
        spacing_blocks: u32,
        content_stage: TerrainPreviewContentStage,
    ) -> Self {
        Self {
            name,
            cells_per_axis,
            spacing_blocks,
            content_stage,
        }
    }
}

#[derive(Serialize)]
struct Receipt {
    schema: &'static str,
    benchmark: &'static str,
    recorded_unix_seconds: u64,
    git_commit: String,
    git_dirty: bool,
    build_profile: &'static str,
    host: Host,
    seed: i64,
    center: Coordinates,
    topology: &'static str,
    iterations: usize,
    warmup_iterations: usize,
    point_workloads: Vec<PointReceipt>,
    preview_workloads: Vec<PreviewReceipt>,
    absent_stages: [&'static str; 5],
}

#[derive(Serialize)]
struct Host {
    operating_system: &'static str,
    architecture: &'static str,
    logical_cpu_count: Option<usize>,
}

#[derive(Serialize)]
struct Coordinates {
    x: i32,
    z: i32,
}

#[derive(Serialize)]
struct PointReceipt {
    id: String,
    footprint_blocks: u32,
    spacing_blocks: u32,
    samples_per_axis: u32,
    lattice_points: u64,
    terrain_sample_evaluations_per_iteration: u64,
    generation: TimingSummary,
    validation_ms: f64,
    checksum: String,
    materialized_output_bytes: u64,
}

#[derive(Serialize)]
struct PreviewReceipt {
    id: String,
    footprint_blocks: u32,
    spacing_blocks: u32,
    cells_per_axis: u32,
    samples_per_axis: u32,
    lattice_points: u64,
    content_stage: &'static str,
    compile_work: TerrainCompileWorkReceipt,
    generation: TimingSummary,
    packing: TimingSummary,
    validation_ms: f64,
    checksum: String,
    materialized_sample_bytes: u64,
    packed_output_bytes: u64,
}

#[derive(Serialize)]
struct TerrainCompileWorkReceipt {
    sample_lattice_points: u64,
    terrain_sample_evaluations: u64,
    forest_intent_evaluations: u64,
    forest_footprint_summaries: u64,
}

impl From<TerrainPreviewCompileWork> for TerrainCompileWorkReceipt {
    fn from(work: TerrainPreviewCompileWork) -> Self {
        Self {
            sample_lattice_points: work.sample_lattice_points,
            terrain_sample_evaluations: work.terrain_sample_evaluations,
            forest_intent_evaluations: work.forest_intent_evaluations,
            forest_footprint_summaries: work.forest_footprint_summaries,
        }
    }
}

#[derive(Serialize)]
struct TimingSummary {
    samples_ms: Vec<f64>,
    minimum_ms: f64,
    median_ms: f64,
    mean_ms: f64,
    maximum_ms: f64,
    work_per_second_at_median: f64,
}

impl TimingSummary {
    fn new(samples_ms: Vec<f64>, work_per_iteration: u64) -> Self {
        let mut sorted = samples_ms.clone();
        sorted.sort_by(f64::total_cmp);
        let minimum_ms = sorted[0];
        let maximum_ms = sorted[sorted.len() - 1];
        let median_ms = if sorted.len().is_multiple_of(2) {
            let right = sorted.len() / 2;
            (sorted[right - 1] + sorted[right]) * 0.5
        } else {
            sorted[sorted.len() / 2]
        };
        let mean_ms = samples_ms.iter().sum::<f64>() / samples_ms.len() as f64;
        let work_per_second_at_median = if median_ms <= f64::EPSILON {
            0.0
        } else {
            work_per_iteration as f64 / (median_ms / 1_000.0)
        };
        Self {
            samples_ms,
            minimum_ms,
            median_ms,
            mean_ms,
            maximum_ms,
            work_per_second_at_median,
        }
    }
}

fn run_point_workload(config: &Config, workload: PointWorkload) -> Result<PointReceipt, String> {
    validate_point_workload(config, workload)?;
    let sampler = McloneOverworldSampler::new_with_topology(config.seed, config.topology);
    for _ in 0..config.warmup_iterations {
        sample_point_grid(config, workload, sampler);
    }
    let mut samples_ms = Vec::with_capacity(config.iterations);
    for _ in 0..config.iterations {
        let start = Instant::now();
        sample_point_grid(config, workload, sampler);
        samples_ms.push(elapsed_ms(start.elapsed()));
    }
    let validation_start = Instant::now();
    let checksum = checksum_point_grid(config, workload, sampler);
    let validation_ms = elapsed_ms(validation_start.elapsed());
    Ok(PointReceipt {
        id: point_workload_id(workload.name),
        footprint_blocks: workload.footprint_blocks,
        spacing_blocks: workload.spacing_blocks,
        samples_per_axis: workload.samples_per_axis,
        lattice_points: workload.point_count(),
        terrain_sample_evaluations_per_iteration: workload.point_count(),
        generation: TimingSummary::new(samples_ms, workload.point_count()),
        validation_ms,
        checksum: format!("{checksum:016x}"),
        materialized_output_bytes: 0,
    })
}

fn validate_point_workload(config: &Config, workload: PointWorkload) -> Result<(), String> {
    let derived_footprint = workload
        .samples_per_axis
        .checked_sub(1)
        .and_then(|cells| cells.checked_mul(workload.spacing_blocks))
        .ok_or_else(|| format!("{} workload footprint overflow", workload.name))?;
    if derived_footprint != workload.footprint_blocks {
        return Err(format!(
            "{} workload declares footprint {} but geometry produces {derived_footprint}",
            workload.name, workload.footprint_blocks
        ));
    }
    point_origin(config.center_x, workload.footprint_blocks)?;
    point_origin(config.center_z, workload.footprint_blocks)?;
    Ok(())
}

fn sample_point_grid(config: &Config, workload: PointWorkload, sampler: McloneOverworldSampler) {
    let min_x =
        point_origin(config.center_x, workload.footprint_blocks).expect("validated point X origin");
    let min_z =
        point_origin(config.center_z, workload.footprint_blocks).expect("validated point Z origin");
    let spacing = i32::try_from(workload.spacing_blocks).expect("workload spacing fits i32");
    for sample_z in 0..workload.samples_per_axis {
        let world_z = min_z + i32::try_from(sample_z).expect("sample Z fits i32") * spacing;
        for sample_x in 0..workload.samples_per_axis {
            let world_x = min_x + i32::try_from(sample_x).expect("sample X fits i32") * spacing;
            black_box(sampler.sample(world_x, world_z));
        }
    }
}

fn checksum_point_grid(
    config: &Config,
    workload: PointWorkload,
    sampler: McloneOverworldSampler,
) -> u64 {
    let min_x =
        point_origin(config.center_x, workload.footprint_blocks).expect("validated point X origin");
    let min_z =
        point_origin(config.center_z, workload.footprint_blocks).expect("validated point Z origin");
    let spacing = i32::try_from(workload.spacing_blocks).expect("workload spacing fits i32");
    let mut checksum = FNV_OFFSET_BASIS;
    for sample_z in 0..workload.samples_per_axis {
        let world_z = min_z + i32::try_from(sample_z).expect("sample Z fits i32") * spacing;
        for sample_x in 0..workload.samples_per_axis {
            let world_x = min_x + i32::try_from(sample_x).expect("sample X fits i32") * spacing;
            hash_terrain_sample(&mut checksum, sampler.sample(world_x, world_z));
        }
    }
    checksum
}

fn run_preview_workload(
    config: &Config,
    workload: PreviewWorkload,
) -> Result<PreviewReceipt, String> {
    let request = preview_request(config, workload);
    let validated = request.validate()?;
    for _ in 0..config.warmup_iterations {
        black_box(TerrainPreviewReferenceGrid::compile(request)?);
    }

    let mut generation_ms = Vec::with_capacity(config.iterations);
    let mut packing_ms = Vec::with_capacity(config.iterations);
    let mut final_grid = None;
    let mut final_packed = Vec::new();
    for _ in 0..config.iterations {
        let generation_start = Instant::now();
        let grid = TerrainPreviewReferenceGrid::compile(request)?;
        generation_ms.push(elapsed_ms(generation_start.elapsed()));

        let packing_start = Instant::now();
        let packed = grid.packed_bytes();
        packing_ms.push(elapsed_ms(packing_start.elapsed()));
        black_box(&packed);
        final_grid = Some(grid);
        final_packed = packed;
    }
    let grid = final_grid.expect("at least one measured preview iteration");
    let work = grid.compile_work();
    let validation_start = Instant::now();
    let checksum = hash_bytes(&final_packed);
    let validation_ms = elapsed_ms(validation_start.elapsed());
    let sample_count = u64::from(validated.sample_count());
    let sample_bytes = sample_count
        .checked_mul(std::mem::size_of_val(&grid.samples()[0]) as u64)
        .expect("bounded preview sample byte count");
    let packed_bytes = u64::try_from(final_packed.len()).expect("packed bytes fit u64");
    Ok(PreviewReceipt {
        id: preview_workload_id(workload.name),
        footprint_blocks: validated.footprint_blocks(),
        spacing_blocks: workload.spacing_blocks,
        cells_per_axis: workload.cells_per_axis,
        samples_per_axis: validated.samples_per_axis(),
        lattice_points: sample_count,
        content_stage: workload.content_stage.label(),
        compile_work: work.into(),
        generation: TimingSummary::new(generation_ms, work.terrain_sample_evaluations),
        packing: TimingSummary::new(packing_ms, packed_bytes),
        validation_ms,
        checksum: format!("{checksum:016x}"),
        materialized_sample_bytes: sample_bytes,
        packed_output_bytes: packed_bytes,
    })
}

fn preview_request(config: &Config, workload: PreviewWorkload) -> TerrainPreviewRequest {
    let mut request = TerrainPreviewRequest::new(
        config.seed,
        config.center_x,
        config.center_z,
        workload.spacing_blocks,
    )
    .with_content_stage(workload.content_stage);
    request.cells_per_axis = workload.cells_per_axis;
    request.topology = config.topology;
    request
}

fn point_origin(center: i32, footprint: u32) -> Result<i32, String> {
    let half = i32::try_from(footprint / 2).map_err(|_| "footprint exceeds i32 coordinates")?;
    center
        .checked_sub(half)
        .ok_or_else(|| "point workload origin exceeds i32 coordinates".to_owned())
}

fn hash_terrain_sample(checksum: &mut u64, sample: McloneOverworldTerrainSample) {
    hash_u64(checksum, sample.continentalness.to_bits());
    hash_u64(checksum, sample.relief.to_bits());
    hash_u64(checksum, sample.ruggedness.to_bits());
    hash_u64(checksum, sample.ridges.to_bits());
    hash_u64(checksum, sample.mountain_detail.to_bits());
    hash_u64(checksum, sample.climate.temperature.to_bits());
    hash_u64(checksum, sample.climate.moisture.to_bits());
    hash_u64(checksum, sample.base_surface_y as u64);
    hash_u64(checksum, sample.surface_y as u64);
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut checksum = FNV_OFFSET_BASIS;
    for byte in bytes {
        checksum ^= u64::from(*byte);
        checksum = checksum.wrapping_mul(FNV_PRIME);
    }
    checksum
}

fn hash_u64(checksum: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *checksum ^= u64::from(byte);
        *checksum = checksum.wrapping_mul(FNV_PRIME);
    }
}

fn point_workload_id(name: &str) -> String {
    format!("point-{name}")
}

fn preview_workload_id(name: &str) -> String {
    format!("preview-{name}")
}

fn parse_topology(value: &str) -> Result<McloneOverworldSamplingTopology, String> {
    match value {
        "plane" => Ok(McloneOverworldSamplingTopology::Unbounded),
        "cylinder-x:384" => Ok(McloneOverworldSamplingTopology::PeriodicX),
        _ => Err(format!(
            "--topology requires plane or cylinder-x:384; got `{value}`"
        )),
    }
}

const fn topology_label(topology: McloneOverworldSamplingTopology) -> &'static str {
    match topology {
        McloneOverworldSamplingTopology::Unbounded => "plane",
        McloneOverworldSamplingTopology::PeriodicX => "cylinder-x:384",
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
    let point_ids = POINT_WORKLOADS
        .iter()
        .map(|workload| point_workload_id(workload.name))
        .collect::<Vec<_>>()
        .join("|");
    let preview_ids = PREVIEW_WORKLOADS
        .iter()
        .map(|workload| preview_workload_id(workload.name))
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "usage: mclone_macro_perf [--seed N] [--center-x N] [--center-z N] \
         [--topology plane|cylinder-x:384] [--iterations N] \
         [--warmup-iterations N] [--output PATH] \
         [--only {point_ids}|{preview_ids}]..."
    )
}

fn elapsed_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn git_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_point_workloads_have_declared_geometry() {
        let config = Config::parse(std::iter::empty()).expect("default config");
        for workload in POINT_WORKLOADS {
            validate_point_workload(&config, workload).expect(workload.name);
        }
        assert_eq!(POINT_WORKLOADS[2].point_count(), 60_025);
        assert_eq!(POINT_WORKLOADS[3].point_count(), 239_121);
        assert_eq!(POINT_WORKLOADS[4].point_count(), 954_529);
    }

    #[test]
    fn only_filter_uses_unambiguous_lane_prefixes() {
        let config = Config::parse([
            "--only".to_owned(),
            "point-regional-65k".to_owned(),
            "--iterations".to_owned(),
            "2".to_owned(),
            "--warmup-iterations".to_owned(),
            "0".to_owned(),
        ])
        .expect("filtered config");
        assert!(config.includes("point-regional-65k".to_owned()));
        assert!(!config.includes("preview-base-65k".to_owned()));
        assert_eq!(config.iterations, 2);
        assert_eq!(config.warmup_iterations, 0);
    }

    #[test]
    fn timing_summary_uses_sorted_median_without_losing_raw_samples() {
        let summary = TimingSummary::new(vec![4.0, 1.0, 3.0, 2.0], 100);
        assert_eq!(summary.samples_ms, vec![4.0, 1.0, 3.0, 2.0]);
        assert_eq!(summary.minimum_ms, 1.0);
        assert_eq!(summary.median_ms, 2.5);
        assert_eq!(summary.mean_ms, 2.5);
        assert_eq!(summary.maximum_ms, 4.0);
        assert_eq!(summary.work_per_second_at_median, 40_000.0);
    }

    #[test]
    fn point_checksum_is_repeatable() {
        let config = Config::parse(std::iter::empty()).expect("default config");
        let sampler = McloneOverworldSampler::new(config.seed);
        let workload = PointWorkload::new("tiny-test", 16, 8, 3);
        assert_eq!(
            checksum_point_grid(&config, workload, sampler),
            checksum_point_grid(&config, workload, sampler)
        );
    }
}
