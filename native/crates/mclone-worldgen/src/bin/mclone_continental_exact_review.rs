use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use mclone_worldgen::continental_exact_harness::run_continental_exact_review;
use serde::Serialize;

const DEFAULT_OUTPUT: &str = "/tmp/mclone-continental-exact-review/direct-exact.json";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RunReceipt {
    source_commit: String,
    git_dirty: bool,
    elapsed_ns: u128,
    review: mclone_worldgen::continental_exact_harness::ContinentalExactReviewReceipt,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("continental exact review failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let output = parse_output()?;
    let started = Instant::now();
    let review = run_continental_exact_review(12_345)?;
    if !review.suite_passed {
        return Err("direct-versus-exact suite reported mismatches".to_owned());
    }
    let receipt = RunReceipt {
        source_commit: command_text("git", &["rev-parse", "HEAD"]),
        git_dirty: !command_text("git", &["status", "--porcelain"]).is_empty(),
        elapsed_ns: started.elapsed().as_nanos(),
        review,
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let mut json = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("serialize exact review: {error}"))?;
    json.push(b'\n');
    fs::write(&output, json).map_err(|error| format!("write {}: {error}", output.display()))?;
    println!("receipt: {}", output.display());
    println!("semantic: {}", receipt.review.semantic_sha256);
    for site in &receipt.review.sites {
        println!(
            "{}: columns={} biomes={} trees={}/{} tree_voxels={} sha256={}",
            site.label,
            site.compared_columns,
            site.compared_biomes,
            site.owned_tree_bases,
            site.intersecting_tree_records,
            site.exact_tree_voxels,
            site.exact_sha256,
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
