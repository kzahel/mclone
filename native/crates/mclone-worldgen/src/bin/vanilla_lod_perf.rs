use std::time::Instant;

use mclone_worldgen::levelgen::{
    VANILLA_OVERWORLD_LOD_REVISION, VANILLA_OVERWORLD_MACRO_LOD_REVISION,
    VANILLA_OVERWORLD_MACRO_VERTICAL_CELL_STEP, VanillaOverworldLodSampler,
    VanillaOverworldMacroSampler,
};
use mclone_worldgen::terrain_preview::{
    TerrainPreviewComparison, TerrainPreviewContentStage, TerrainPreviewProfile,
    TerrainPreviewReferenceGrid, TerrainPreviewRequest, TerrainPreviewSurfaceQuality,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse(std::env::args().skip(1))?;
    let mut request = TerrainPreviewRequest::new(
        config.seed,
        config.center_x,
        config.center_z,
        config.spacing,
    )
    .with_profile(TerrainPreviewProfile::VanillaOverworld)
    .with_content_stage(TerrainPreviewContentStage::Surface)
    .with_surface_quality(config.surface_quality);
    request.cells_per_axis = config.cells_per_axis;
    request.validate()?;

    let mut exact_sampler = VanillaOverworldLodSampler::new(config.seed);
    let exact_start = Instant::now();
    let exact =
        TerrainPreviewReferenceGrid::compile_with_vanilla_sampler(request, &mut exact_sampler)?;
    let exact_ms = exact_start.elapsed().as_secs_f64() * 1_000.0;

    let mut macro_sampler = VanillaOverworldMacroSampler::new(config.seed);
    let macro_start = Instant::now();
    let macro_grid = TerrainPreviewReferenceGrid::compile_with_vanilla_macro_sampler(
        request,
        &mut macro_sampler,
    )?;
    let macro_ms = macro_start.elapsed().as_secs_f64() * 1_000.0;
    let comparison = TerrainPreviewComparison::compare(&exact, macro_grid.samples())?;
    let exact_changed_materials = exact
        .samples()
        .iter()
        .filter(|sample| {
            !sample.is_water()
                && sample.visible_surface_material() != sample.surface_recipe.round() as u8
        })
        .count();
    let macro_changed_materials = macro_grid
        .samples()
        .iter()
        .filter(|sample| {
            !sample.is_water()
                && sample.visible_surface_material() != sample.surface_recipe.round() as u8
        })
        .count();

    println!(
        concat!(
            "{{\"seed\":{},\"center_x\":{},\"center_z\":{},\"spacing\":{},",
            "\"cells_per_axis\":{},\"sample_count\":{},\"surface_quality\":\"{}\",",
            "\"exact_revision\":\"{}\",\"macro_revision\":\"{}\",",
            "\"macro_vertical_cell_step\":{},\"exact_ms\":{:.3},\"macro_ms\":{:.3},",
            "\"speedup\":{:.3},\"solid_mean_error\":{:.3},\"solid_p95_error\":{:.3},",
            "\"solid_max_error\":{:.3},\"display_mean_error\":{:.3},",
            "\"display_p95_error\":{:.3},\"display_max_error\":{:.3},",
            "\"water_agreement\":{:.6},\"visible_material_agreement\":{:.6},",
            "\"exact_changed_land_materials\":{},\"macro_changed_land_materials\":{}}}"
        ),
        config.seed,
        config.center_x,
        config.center_z,
        config.spacing,
        config.cells_per_axis,
        comparison.sample_count,
        config.surface_quality.label(),
        VANILLA_OVERWORLD_LOD_REVISION,
        VANILLA_OVERWORLD_MACRO_LOD_REVISION,
        VANILLA_OVERWORLD_MACRO_VERTICAL_CELL_STEP,
        exact_ms,
        macro_ms,
        exact_ms / macro_ms,
        comparison.mean_absolute_surface_error,
        comparison.p95_absolute_surface_error,
        comparison.max_absolute_surface_error,
        comparison.mean_absolute_display_error,
        comparison.p95_absolute_display_error,
        comparison.max_absolute_display_error,
        comparison.water_presence_agreement,
        comparison.visible_surface_material_agreement,
        exact_changed_materials,
        macro_changed_materials,
    );
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct Config {
    seed: i64,
    center_x: i32,
    center_z: i32,
    spacing: u32,
    cells_per_axis: u32,
    surface_quality: TerrainPreviewSurfaceQuality,
}

impl Config {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut config = Self {
            seed: 12_345,
            center_x: 0,
            center_z: 0,
            spacing: 32,
            cells_per_axis: 64,
            surface_quality: TerrainPreviewSurfaceQuality::Basic,
        };
        let mut args = args.into_iter();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--seed" => config.seed = parse_next(&mut args, "--seed")?,
                "--center-x" => config.center_x = parse_next(&mut args, "--center-x")?,
                "--center-z" => config.center_z = parse_next(&mut args, "--center-z")?,
                "--spacing" => config.spacing = parse_next(&mut args, "--spacing")?,
                "--cells" => config.cells_per_axis = parse_next(&mut args, "--cells")?,
                "--surface-quality" => {
                    config.surface_quality = TerrainPreviewSurfaceQuality::parse_label(
                        &args.next().ok_or("--surface-quality requires a value")?,
                    )?
                }
                "--help" | "-h" => return Err(usage()),
                _ => return Err(format!("unknown argument {argument}\n{}", usage())),
            }
        }
        Ok(config)
    }
}

fn parse_next<T>(args: &mut impl Iterator<Item = String>, argument: &str) -> Result<T, String>
where
    T: std::str::FromStr,
{
    args.next()
        .ok_or_else(|| format!("{argument} requires a value"))?
        .parse()
        .map_err(|_| format!("{argument} has an invalid value"))
}

fn usage() -> String {
    "usage: vanilla_lod_perf [--seed N] [--center-x N] [--center-z N] \
     [--spacing N] [--cells N] [--surface-quality basic|inferred]"
        .to_owned()
}
