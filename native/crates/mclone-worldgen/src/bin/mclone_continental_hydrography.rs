use std::{env, fs, path::PathBuf};

use mclone_worldgen::continental_hydrography_harness::run_continental_hydrography_suite;

const DEFAULT_OUTPUT: &str = "/tmp/mclone-continental-hydrography.json";

fn main() {
    if let Err(error) = run() {
        eprintln!("continental hydrography witness failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut output = PathBuf::from(DEFAULT_OUTPUT);
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output" => {
                output = PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--output requires a path".to_owned())?,
                );
            }
            _ => return Err(format!("unknown argument {argument:?}")),
        }
    }
    let receipt = run_continental_hydrography_suite()?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(
        &output,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&receipt)
                .map_err(|error| format!("serialize receipt: {error}"))?
        ),
    )
    .map_err(|error| format!("write {}: {error}", output.display()))?;
    println!("receipt: {}", output.display());
    println!("witness: {}", receipt.witness_sha256);
    Ok(())
}
