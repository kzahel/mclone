use std::{env, fs, path::PathBuf};

use mclone_worldgen::{
    continental_catchment_review::compile_continental_catchment_review,
    continental_ecoregion::ContinentalEcoregionDescriptor,
};

const DEFAULT_OUTPUT: &str = "/tmp/mclone-continental-catchment-review.json";

fn main() {
    if let Err(error) = run() {
        eprintln!("continental catchment review selection failed: {error}");
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
                )
            }
            _ => return Err(format!("unknown argument {argument:?}")),
        }
    }
    let catalog =
        compile_continental_catchment_review(ContinentalEcoregionDescriptor::plane(12_345))?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(
        &output,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&catalog)
                .map_err(|error| format!("serialize catalog: {error}"))?
        ),
    )
    .map_err(|error| format!("write {}: {error}", output.display()))?;
    println!("receipt: {}", output.display());
    println!("catalog: {}", catalog.semantic_sha256);
    println!(
        "catchment: owner=({}, {}) score={:.3}",
        catalog.selected_owner_x, catalog.selected_owner_z, catalog.fitness_score
    );
    for site in catalog.sites {
        println!(
            "{}: ({}, {}) y={:.2} water={} relief={:.1} range={:.1}..{:.1} peak=({}, {})",
            site.kind.label(),
            site.center_x,
            site.center_z,
            site.surface_y,
            site.water_kind.label(),
            site.relief.relief_blocks,
            site.relief.minimum_surface_y,
            site.relief.maximum_surface_y,
            site.relief.maximum_x,
            site.relief.maximum_z,
        );
    }
    Ok(())
}
