use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mclone_worldgen::multiscale_terrain_witness::{
    MULTISCALE_WITNESS_SCHEMA_REVISION, MultiscaleWitnessDetail, run_multiscale_witness_suite,
};
use mclone_worldgen::streamed_plan_harness::{
    PlanRegion, StreamedPlanControl, StreamedPlanDescriptor, StreamedPlanRequest,
    StreamedPlanTopology,
};
use serde::Serialize;

const DEFAULT_OUTPUT: &str = "/tmp/mclone-multiscale-terrain-witness/receipt.json";

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
    detail: MultiscaleWitnessDetail,
    iterations: u32,
    elapsed_ns: u128,
    average_ns: u128,
    facts_per_query: u32,
}

#[derive(Debug, Serialize)]
struct RunReceipt {
    receipt_schema: &'static str,
    run: RunMetadata,
    suite: mclone_worldgen::multiscale_terrain_witness::MultiscaleWitnessReceipt,
    direct_query_timings: Vec<DetailTiming>,
    artifact_path: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("multiscale terrain witness failed: {error}");
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
    let suite = run_multiscale_witness_suite()?;
    if !suite.suite_passed {
        return Err("one or more multiscale witness checks failed".to_owned());
    }
    let receipt = RunReceipt {
        receipt_schema: MULTISCALE_WITNESS_SCHEMA_REVISION,
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
        direct_query_timings: direct_query_timings()?,
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
    println!(
        "comparisons: {} · corpora: {}",
        receipt.suite.exact_comparison_count,
        receipt.suite.corpora.len()
    );
    for timing in &receipt.direct_query_timings {
        println!(
            "{}: {} facts · {} ns/query",
            timing.detail.label(),
            timing.facts_per_query,
            timing.average_ns
        );
    }
    Ok(())
}

fn direct_query_timings() -> Result<Vec<DetailTiming>, String> {
    use mclone_worldgen::multiscale_terrain_witness::MultiscaleSemanticRefinementControl;

    const ITERATIONS: u32 = 2_000;
    let descriptor = StreamedPlanDescriptor::new(12_345, StreamedPlanTopology::Plane);
    let request = StreamedPlanRequest::new(PlanRegion::new(0, 0), PlanRegion::new(0, 0));
    let mut timings = Vec::new();
    for detail in MultiscaleWitnessDetail::ALL {
        let mut control = MultiscaleSemanticRefinementControl::at_detail(detail);
        let first = control.construct(&descriptor, request)?;
        let started = Instant::now();
        for _ in 0..ITERATIONS {
            let snapshot = control.construct(&descriptor, request)?;
            std::hint::black_box(snapshot);
        }
        let elapsed_ns = started.elapsed().as_nanos();
        timings.push(DetailTiming {
            detail,
            iterations: ITERATIONS,
            elapsed_ns,
            average_ns: elapsed_ns / u128::from(ITERATIONS),
            facts_per_query: first.facts.len() as u32,
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
                    "usage: mclone_multiscale_terrain_witness [--output PATH]\n\
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
