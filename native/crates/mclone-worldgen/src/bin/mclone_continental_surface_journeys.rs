use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use mclone_worldgen::{
    continental_ecoregion::ContinentalEcoregionDescriptor,
    continental_surface_journey::{
        ContinentalSurfaceJourneyCatalog, compile_continental_surface_journeys,
    },
};
use serde::Serialize;

const DEFAULT_OUTPUT: &str = "/tmp/mclone-continental-surface-journeys.json";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JourneyRunReceipt {
    source_commit: String,
    git_dirty: bool,
    elapsed_ns: u128,
    nanoseconds_per_scan_sample: u128,
    catalog: ContinentalSurfaceJourneyCatalog,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("continental surface journey selection failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let output = parse_output()?;
    let started = Instant::now();
    let catalog =
        compile_continental_surface_journeys(ContinentalEcoregionDescriptor::plane(12_345))
            .map_err(|error| error.to_string())?;
    let elapsed_ns = started.elapsed().as_nanos();
    let receipt = JourneyRunReceipt {
        source_commit: command_text("git", &["rev-parse", "HEAD"]),
        git_dirty: !command_text("git", &["status", "--porcelain"]).is_empty(),
        elapsed_ns,
        nanoseconds_per_scan_sample: elapsed_ns / u128::from(catalog.scan_sample_count),
        catalog,
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(
        &output,
        serde_json::to_vec_pretty(&receipt)
            .map_err(|error| format!("serialize journey receipt: {error}"))?,
    )
    .map_err(|error| format!("write {}: {error}", output.display()))?;
    println!("receipt: {}", output.display());
    println!("catalog: {}", receipt.catalog.semantic_sha256);
    println!(
        "selection: {} ns ({} ns/scan sample)",
        receipt.elapsed_ns, receipt.nanoseconds_per_scan_sample
    );
    for journey in &receipt.catalog.journeys {
        println!(
            "{}: ({}, {}) heading=({}, {}) span={} score={:.3}",
            journey.kind.label(),
            journey.center_x,
            journey.center_z,
            journey.heading_x,
            journey.heading_z,
            journey.span_blocks,
            journey.fitness_score,
        );
    }
    Ok(())
}

fn parse_output() -> Result<PathBuf, String> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let mut output = PathBuf::from(DEFAULT_OUTPUT);
    while let Some(argument) = arguments.next() {
        if argument == "--output" {
            output = PathBuf::from(
                arguments
                    .next()
                    .ok_or_else(|| "--output requires a path".to_owned())?,
            );
        } else {
            return Err(format!("unknown argument {argument:?}"));
        }
    }
    Ok(output)
}

fn command_text(program: &str, arguments: &[&str]) -> String {
    std::process::Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}
