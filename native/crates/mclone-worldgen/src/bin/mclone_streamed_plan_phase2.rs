use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use mclone_worldgen::streamed_plan_trials::{
    STREAMED_PLAN_PHASE_TWO_SCHEMA_REVISION, run_streamed_plan_phase_two_suite,
};
use serde::Serialize;

const DEFAULT_OUTPUT: &str = "/tmp/mclone-streamed-plan-phase2/receipt.json";

#[derive(Debug, Serialize)]
struct PhaseTwoRun {
    experiment_id: &'static str,
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
    worker_count: u32,
}

#[derive(Debug, Serialize)]
struct PhaseTwoArtifact {
    kind: &'static str,
    path: String,
    inspected: bool,
}

#[derive(Debug, Serialize)]
struct PhaseTwoRunReceipt {
    receipt_schema: &'static str,
    run: PhaseTwoRun,
    suite: mclone_worldgen::streamed_plan_trials::StreamedPlanPhaseTwoReceipt,
    artifacts: Vec<PhaseTwoArtifact>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("streamed-plan Phase 2 suite failed: {error}");
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
    let started_utc = command_text("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"]);
    let suite = run_streamed_plan_phase_two_suite()?;
    if !suite.suite_passed {
        return Err("one or more Phase 2 comparisons did not pass".to_owned());
    }
    let receipt = PhaseTwoRunReceipt {
        receipt_schema: STREAMED_PLAN_PHASE_TWO_SCHEMA_REVISION,
        run: PhaseTwoRun {
            experiment_id: "tactical-270-phase-2",
            source_commit: command_text("git", &["rev-parse", "HEAD"]),
            git_dirty: !command_text("git", &["status", "--porcelain"]).is_empty(),
            command: arguments,
            started_utc,
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
            worker_count: 0,
        },
        suite,
        artifacts: vec![PhaseTwoArtifact {
            kind: "receipt",
            path: output.display().to_string(),
            inspected: false,
        }],
    };
    let parent = output
        .parent()
        .ok_or_else(|| format!("output path {} has no parent", output.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("create {}: {error}", parent.display()))?;
    let encoded = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("serialize receipt: {error}"))?;
    fs::write(&output, encoded).map_err(|error| format!("write {}: {error}", output.display()))?;
    println!("receipt: {}", output.display());
    println!(
        "Phase 2 witness: {}",
        receipt.suite.phase_two_witness_sha256
    );
    println!(
        "comparisons: {} (fallback {}, hierarchy {}, feature graph {})",
        receipt.suite.exact_comparison_count,
        receipt.suite.fallback.exact_comparison_count,
        receipt.suite.hierarchical.exact_comparison_count,
        receipt.suite.feature_graph.exact_comparison_count
    );
    Ok(())
}

fn parse_output(arguments: &[String]) -> Result<PathBuf, String> {
    let mut output = PathBuf::from(DEFAULT_OUTPUT);
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--output" => {
                index += 1;
                let path = arguments
                    .get(index)
                    .ok_or_else(|| "--output requires a path".to_owned())?;
                output = PathBuf::from(path);
            }
            "-h" | "--help" => {
                println!(
                    "usage: mclone_streamed_plan_phase2 [--output PATH]\n\
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
