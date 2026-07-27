use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mclone_worldgen::multiscale_terrain_witness::MultiscaleWitnessDetail;
use mclone_worldgen::semantic_terrain_sandbox::{
    SEMANTIC_TERRAIN_DEFAULT_SAMPLES_ACROSS, SEMANTIC_TERRAIN_SANDBOX_SCHEMA_REVISION,
    SemanticTerrainFeatureMode, SemanticTerrainSandboxRequest, SemanticTerrainSubstrate,
    compile_semantic_terrain_detail, run_semantic_terrain_sandbox_suite,
};
use mclone_worldgen::streamed_plan_harness::StreamedPlanTopology;
use serde::Serialize;

const DEFAULT_OUTPUT: &str = "/tmp/mclone-semantic-terrain-sandbox/receipt.json";
const ITERATIONS: u32 = 20;

#[derive(Debug, Serialize)]
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
struct DetailTiming {
    blocks_across: u32,
    detail: MultiscaleWitnessDetail,
    samples: u32,
    feature_facts: u32,
    distance_evaluations: u64,
    iterations: u32,
    elapsed_ns: u128,
    average_ns: u128,
    terrain_sha256: String,
}

#[derive(Debug, Serialize)]
struct RunReceipt {
    receipt_schema: &'static str,
    run: RunMetadata,
    suite: mclone_worldgen::semantic_terrain_sandbox::SemanticTerrainSandboxSuiteReceipt,
    native_timings: Vec<DetailTiming>,
    encoded_receipt_bytes: usize,
    artifact_path: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("semantic terrain sandbox failed: {error}");
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
    let suite = run_semantic_terrain_sandbox_suite()?;
    if !suite.passed {
        return Err("one or more semantic terrain sandbox checks failed".to_owned());
    }
    let mut receipt = RunReceipt {
        receipt_schema: SEMANTIC_TERRAIN_SANDBOX_SCHEMA_REVISION,
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
        native_timings: native_timings()?,
        suite,
        encoded_receipt_bytes: 0,
        artifact_path: output.display().to_string(),
    };
    let encoded = loop {
        let encoded = serde_json::to_vec_pretty(&receipt)
            .map_err(|error| format!("serialize receipt: {error}"))?;
        if receipt.encoded_receipt_bytes == encoded.len() {
            break encoded;
        }
        receipt.encoded_receipt_bytes = encoded.len();
    };
    let parent = output
        .parent()
        .ok_or_else(|| format!("output path {} has no parent", output.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("create {}: {error}", parent.display()))?;
    fs::write(&output, encoded).map_err(|error| format!("write {}: {error}", output.display()))?;
    println!("receipt: {}", output.display());
    println!("suite: {}", receipt.suite.suite_sha256);
    for timing in &receipt.native_timings {
        println!(
            "{} blocks · {}: {} facts · {} evals · {:.3} ms",
            timing.blocks_across,
            timing.detail.label(),
            timing.feature_facts,
            timing.distance_evaluations,
            timing.average_ns as f64 / 1_000_000.0,
        );
    }
    Ok(())
}

fn native_timings() -> Result<Vec<DetailTiming>, String> {
    let mut timings = Vec::new();
    for blocks_across in [6_144, 16_384, 65_536] {
        let request = SemanticTerrainSandboxRequest::new(
            12_345,
            StreamedPlanTopology::Plane,
            0.0,
            0.0,
            f64::from(blocks_across),
            1.0,
            SEMANTIC_TERRAIN_DEFAULT_SAMPLES_ACROSS,
            SemanticTerrainSubstrate::Quiet,
            SemanticTerrainFeatureMode::Combined,
        );
        for detail in MultiscaleWitnessDetail::ALL {
            let first = compile_semantic_terrain_detail(request, detail)?;
            let started = Instant::now();
            for _ in 0..ITERATIONS {
                std::hint::black_box(compile_semantic_terrain_detail(request, detail)?);
            }
            let elapsed_ns = started.elapsed().as_nanos();
            timings.push(DetailTiming {
                blocks_across,
                detail,
                samples: first.heights.len() as u32,
                feature_facts: first.metrics.feature_count,
                distance_evaluations: first.metrics.distance_evaluation_count,
                iterations: ITERATIONS,
                elapsed_ns,
                average_ns: elapsed_ns / u128::from(ITERATIONS),
                terrain_sha256: first.terrain_sha256,
            });
        }
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
                    "usage: mclone_semantic_terrain_sandbox [--output PATH]\n\
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
