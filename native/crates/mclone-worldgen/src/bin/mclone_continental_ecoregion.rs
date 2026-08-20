use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mclone_worldgen::continental_ecoregion::{
    ContinentalEcoregionDescriptor, ContinentalEcoregionPlan, LandscapePlanDetail,
    LandscapeWindowRequest,
};
use mclone_worldgen::continental_ecoregion_atlas::{
    ClearingPlanDistribution, ContinentalEcoregionAtlasRequest, HabitatConnectivityMetrics,
    QuantityDistribution, compile_continental_ecoregion_atlas,
};
use mclone_worldgen::continental_ecoregion_harness::{
    CONTINENTAL_ECOREGION_HARNESS_SCHEMA_REVISION, run_continental_ecoregion_suite,
};
use serde::Serialize;

const DEFAULT_OUTPUT: &str = "/tmp/mclone-continental-ecoregion/receipt.json";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunMetadata {
    source_commit: String,
    git_dirty: bool,
    command: Vec<String>,
    started_utc: String,
    started_unix_ms: u128,
    host: String,
    target: String,
    rustc: String,
    optimization: &'static str,
    thread_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DirectTiming {
    detail: LandscapePlanDetail,
    iterations: u32,
    elapsed_ns: u128,
    average_ns: u128,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowTiming {
    blocks_across: u32,
    sample_step_blocks: u32,
    width_samples: u32,
    depth_samples: u32,
    elapsed_ns: u128,
    nanoseconds_per_sample: u128,
    land_samples: u32,
    ocean_samples: u32,
    semantic_sha256: String,
    work: mclone_worldgen::continental_ecoregion::PlanConstructionCounts,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AtlasTiming {
    blocks_across: u32,
    sample_step_blocks: u32,
    sample_count: u32,
    cold_elapsed_ns: u128,
    cold_nanoseconds_per_sample: u128,
    warm_elapsed_ns: u128,
    warm_nanoseconds_per_sample: u128,
    candidate_sha256: String,
    production_control_sha256: String,
    candidate_exact_chunks: u64,
    production_control_exact_chunks: u64,
    production_control_field_samples: u64,
    candidate_ecoregion_components: u32,
    production_control_biome_components: u32,
    production_control_open_components: u32,
    arid_fraction: f32,
    rain_shadow_fraction: f32,
    permanent_drainage_fraction: f32,
    transition_width_blocks: QuantityDistribution,
    clearing_plans: ClearingPlanDistribution,
    regional_signature_recurrence_blocks: QuantityDistribution,
    habitat_connectivity: HabitatConnectivityMetrics,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunReceipt {
    receipt_schema: &'static str,
    run: RunMetadata,
    suite: mclone_worldgen::continental_ecoregion_harness::ContinentalEcoregionSuiteReceipt,
    direct_timings: Vec<DirectTiming>,
    window_timings: Vec<WindowTiming>,
    atlas_timings: Vec<AtlasTiming>,
    artifact_path: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("continental/ecoregion harness failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let arguments = env::args().collect::<Vec<_>>();
    let output = parse_output(&arguments)?;
    let started_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes Unix epoch: {error}"))?
        .as_millis();
    let suite = run_continental_ecoregion_suite()?;
    if !suite.suite_passed {
        return Err("one or more exact corpus checks failed".to_owned());
    }
    let receipt = RunReceipt {
        receipt_schema: CONTINENTAL_ECOREGION_HARNESS_SCHEMA_REVISION,
        run: RunMetadata {
            source_commit: command_text("git", &["rev-parse", "HEAD"]),
            git_dirty: !command_text("git", &["status", "--porcelain"]).is_empty(),
            command: arguments,
            started_utc: command_text("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"]),
            started_unix_ms,
            host: command_text("uname", &["-a"]),
            target: rustc_host(),
            rustc: command_text("rustc", &["--version"]),
            optimization: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
            thread_count: std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1),
        },
        direct_timings: direct_timings()?,
        window_timings: window_timings()?,
        atlas_timings: atlas_timings()?,
        suite,
        artifact_path: output.display().to_string(),
    };
    let parent = output
        .parent()
        .ok_or_else(|| format!("output path {} has no parent", output.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("create {}: {error}", parent.display()))?;
    let encoded = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("serialize receipt: {error}"))?;
    fs::write(&output, encoded).map_err(|error| format!("write {}: {error}", output.display()))?;
    println!("receipt: {}", output.display());
    println!("witness: {}", receipt.suite.witness_sha256);
    println!("exact comparisons: {}", receipt.suite.exact_comparisons);
    for timing in &receipt.direct_timings {
        println!(
            "{} point: {} ns/query",
            timing.detail.label(),
            timing.average_ns
        );
    }
    for timing in &receipt.window_timings {
        println!(
            "{}-block atlas at {} blocks/sample: {} ns ({} ns/sample)",
            timing.blocks_across,
            timing.sample_step_blocks,
            timing.elapsed_ns,
            timing.nanoseconds_per_sample
        );
    }
    for timing in &receipt.atlas_timings {
        println!(
            "{}-block candidate+control atlas: cold {} ns ({} ns/sample), warm {} ns ({} ns/sample)",
            timing.blocks_across,
            timing.cold_elapsed_ns,
            timing.cold_nanoseconds_per_sample,
            timing.warm_elapsed_ns,
            timing.warm_nanoseconds_per_sample
        );
    }
    Ok(())
}

fn direct_timings() -> Result<Vec<DirectTiming>, String> {
    const ITERATIONS: u32 = 20_000;
    let plan = ContinentalEcoregionPlan::new(ContinentalEcoregionDescriptor::plane(12_345))
        .map_err(|error| error.to_string())?;
    let mut timings = Vec::new();
    for detail in LandscapePlanDetail::ALL {
        let started = Instant::now();
        for iteration in 0..ITERATIONS {
            let sample = plan.query_point(
                detail,
                -18_000 + (iteration as i32 * 1_009).rem_euclid(36_000),
                -18_000 + (iteration as i32 * 1_021).rem_euclid(36_000),
            );
            std::hint::black_box(sample);
        }
        let elapsed_ns = started.elapsed().as_nanos();
        timings.push(DirectTiming {
            detail,
            iterations: ITERATIONS,
            elapsed_ns,
            average_ns: elapsed_ns / u128::from(ITERATIONS),
        });
    }
    Ok(timings)
}

fn window_timings() -> Result<Vec<WindowTiming>, String> {
    let plan = ContinentalEcoregionPlan::new(ContinentalEcoregionDescriptor::plane(12_345))
        .map_err(|error| error.to_string())?;
    let mut timings = Vec::new();
    for (blocks_across, sample_step_blocks) in [(65_536_u32, 256_u32), (131_072, 512)] {
        let width_samples = blocks_across / sample_step_blocks;
        let depth_samples = width_samples;
        let request = LandscapeWindowRequest::new(
            -(blocks_across as i32) / 2,
            -(blocks_across as i32) / 2,
            width_samples,
            depth_samples,
            sample_step_blocks,
            LandscapePlanDetail::Mosaic,
        );
        let started = Instant::now();
        let window = plan
            .query_window(request)
            .map_err(|error| error.to_string())?;
        let elapsed_ns = started.elapsed().as_nanos();
        timings.push(WindowTiming {
            blocks_across,
            sample_step_blocks,
            width_samples,
            depth_samples,
            elapsed_ns,
            nanoseconds_per_sample: elapsed_ns / window.samples.len() as u128,
            land_samples: window
                .samples
                .iter()
                .filter(|sample| sample.continent.is_some())
                .count() as u32,
            ocean_samples: window
                .samples
                .iter()
                .filter(|sample| sample.continent.is_none())
                .count() as u32,
            semantic_sha256: window.semantic_sha256,
            work: window.work,
        });
    }
    Ok(timings)
}

fn atlas_timings() -> Result<Vec<AtlasTiming>, String> {
    let mut timings = Vec::new();
    for blocks_across in [65_536_u32, 131_072] {
        let request = ContinentalEcoregionAtlasRequest::plane(12_345, 0, 0, blocks_across);
        let started = Instant::now();
        let atlas =
            compile_continental_ecoregion_atlas(request).map_err(|error| error.to_string())?;
        let cold_elapsed_ns = started.elapsed().as_nanos();
        let warm_started = Instant::now();
        let warm_atlas =
            compile_continental_ecoregion_atlas(request).map_err(|error| error.to_string())?;
        let warm_elapsed_ns = warm_started.elapsed().as_nanos();
        if warm_atlas.metadata.semantic_sha256 != atlas.metadata.semantic_sha256
            || warm_atlas.metadata.production_control_sha256
                != atlas.metadata.production_control_sha256
            || warm_atlas.metadata.work != atlas.metadata.work
            || warm_atlas.metadata.production_control_work != atlas.metadata.production_control_work
        {
            return Err(format!(
                "cold and warm {}-block atlas receipts diverged",
                blocks_across
            ));
        }
        timings.push(AtlasTiming {
            blocks_across,
            sample_step_blocks: atlas.metadata.sample_step_blocks,
            sample_count: atlas.metadata.sample_count,
            cold_elapsed_ns,
            cold_nanoseconds_per_sample: cold_elapsed_ns / u128::from(atlas.metadata.sample_count),
            warm_elapsed_ns,
            warm_nanoseconds_per_sample: warm_elapsed_ns / u128::from(atlas.metadata.sample_count),
            candidate_sha256: atlas.metadata.semantic_sha256,
            production_control_sha256: atlas.metadata.production_control_sha256,
            candidate_exact_chunks: atlas.metadata.work.exact_chunks,
            production_control_exact_chunks: atlas.metadata.production_control_work.exact_chunks,
            production_control_field_samples: atlas.metadata.production_control_work.field_samples,
            candidate_ecoregion_components: atlas
                .metadata
                .metrics
                .ecoregion_components
                .component_count,
            production_control_biome_components: atlas
                .metadata
                .production_control_metrics
                .biome_components
                .component_count,
            production_control_open_components: atlas
                .metadata
                .production_control_metrics
                .open_components
                .component_count,
            arid_fraction: atlas.metadata.metrics.arid_fraction,
            rain_shadow_fraction: atlas.metadata.metrics.rain_shadow_fraction,
            permanent_drainage_fraction: atlas.metadata.metrics.permanent_drainage_fraction,
            transition_width_blocks: atlas.metadata.metrics.transition_width_blocks,
            clearing_plans: atlas.metadata.metrics.clearing_plans.clone(),
            regional_signature_recurrence_blocks: atlas
                .metadata
                .metrics
                .regional_signature_recurrence_blocks,
            habitat_connectivity: atlas.metadata.metrics.habitat_connectivity,
        });
    }
    Ok(timings)
}

fn parse_output(arguments: &[String]) -> Result<PathBuf, String> {
    let mut output = PathBuf::from(DEFAULT_OUTPUT);
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--output" => {
                index += 1;
                output = PathBuf::from(
                    arguments
                        .get(index)
                        .ok_or_else(|| "--output requires a path".to_owned())?,
                );
            }
            "-h" | "--help" => {
                println!(
                    "usage: mclone_continental_ecoregion [--output PATH]\n\
                     default output: {DEFAULT_OUTPUT}"
                );
                std::process::exit(0);
            }
            argument => return Err(format!("unknown argument: {argument}")),
        }
        index += 1;
    }
    Ok(output)
}

fn command_text(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn rustc_host() -> String {
    command_text("rustc", &["-vV"])
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap_or("unknown")
        .to_owned()
}
