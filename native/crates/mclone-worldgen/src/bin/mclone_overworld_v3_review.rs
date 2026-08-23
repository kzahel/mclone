use std::{env, fs, path::PathBuf, process::ExitCode};

use mclone_worldgen::mclone_overworld_v3::McloneOverworldV3TerrainPlan;
use serde::Serialize;

const DEFAULT_OUTPUT: &str = "/tmp/mclone-overworld-v3-review.json";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewReceipt {
    schema_revision: &'static str,
    seed: i64,
    site: mclone_worldgen::mclone_overworld_v3::V3MountainReviewSite,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Mclone Overworld V3 review selection failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let (seed, output) = parse_arguments()?;
    let plan = McloneOverworldV3TerrainPlan::new(seed);
    let receipt = ReviewReceipt {
        schema_revision: mclone_worldgen::mclone_overworld_v3::MCLONE_OVERWORLD_V3_SCHEMA_REVISION,
        seed,
        site: plan.select_mountain_review_site(),
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let mut json = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("serialize review receipt: {error}"))?;
    json.push(b'\n');
    fs::write(&output, json).map_err(|error| format!("write {}: {error}", output.display()))?;
    println!("receipt: {}", output.display());
    println!(
        "site: ({}, {}) relief={:.1} y={:.1}..{:.1} score={:.1}",
        receipt.site.center_x,
        receipt.site.center_z,
        receipt.site.relief_blocks,
        receipt.site.minimum_surface_y,
        receipt.site.maximum_surface_y,
        receipt.site.fitness_score,
    );
    Ok(())
}

fn parse_arguments() -> Result<(i64, PathBuf), String> {
    let mut seed = 12_345;
    let mut output = PathBuf::from(DEFAULT_OUTPUT);
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--seed" => {
                seed = arguments
                    .next()
                    .ok_or_else(|| "--seed requires an integer".to_owned())?
                    .parse()
                    .map_err(|error| format!("invalid --seed: {error}"))?;
            }
            "--output" => {
                output = PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--output requires a path".to_owned())?,
                );
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok((seed, output))
}
