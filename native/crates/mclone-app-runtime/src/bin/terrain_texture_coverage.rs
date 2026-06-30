use std::collections::BTreeSet;
use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use mclone_app_runtime::render_assets::load_asset_source;
use mclone_assets::{AssetPath, AssetSource, PackedAssetSource};
use mclone_mesh::collect_textured_terrain_materials;

const DEFAULT_OVERLAY_PACK: &str = "/tmp/mclone-texture-lab/mclone-default-overlay.pbp";
const DEFAULT_OUTPUT: &str = "/tmp/mclone-texture-lab/mclone-default-overlay-coverage.md";

#[derive(Debug)]
struct Options {
    overlay_packs: Vec<PathBuf>,
    output: PathBuf,
}

#[derive(Debug)]
struct LoadedOverlayPack {
    path: PathBuf,
    asset_set: String,
    file_count: usize,
    texture_paths: BTreeSet<AssetPath>,
}

fn main() -> Result<()> {
    let options = parse_args(env::args().skip(1))?;
    let source = load_asset_source().context("failed to load native asset source chain")?;
    let required = collect_textured_terrain_materials(&source)
        .context("failed to collect native terrain texture materials")?
        .into_iter()
        .map(|material| AssetPath::texture_png(&material.texture))
        .collect::<BTreeSet<_>>();

    let overlay_packs = options
        .overlay_packs
        .iter()
        .map(load_overlay_pack)
        .collect::<Result<Vec<_>>>()?;
    let overlay_paths = overlay_packs
        .iter()
        .flat_map(|pack| pack.texture_paths.iter().cloned())
        .collect::<BTreeSet<_>>();

    let covered = required
        .intersection(&overlay_paths)
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing = required
        .difference(&overlay_paths)
        .cloned()
        .collect::<BTreeSet<_>>();
    let unused = overlay_paths
        .difference(&required)
        .cloned()
        .collect::<BTreeSet<_>>();

    let report = coverage_report(&overlay_packs, &required, &covered, &missing, &unused);
    if let Some(parent) = options.output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(&options.output, report)
        .with_context(|| format!("failed to write {}", options.output.display()))?;

    println!(
        "terrain texture overlay coverage: {}/{} covered ({:.1}%), {} missing, {} unused overlay texture(s)",
        covered.len(),
        required.len(),
        percent(covered.len(), required.len()),
        missing.len(),
        unused.len()
    );
    println!("wrote {}", options.output.display());
    Ok(())
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Options> {
    let mut overlay_packs = Vec::new();
    let mut output = PathBuf::from(DEFAULT_OUTPUT);
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--overlay-pack" => {
                let path = args.next().context("--overlay-pack requires a path")?;
                overlay_packs.push(PathBuf::from(path));
            }
            "--output" => {
                let path = args.next().context("--output requires a path")?;
                output = PathBuf::from(path);
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => bail!("unknown argument `{other}`; use --help"),
        }
    }

    if overlay_packs.is_empty() {
        overlay_packs = env_overlay_packs();
    }
    if overlay_packs.is_empty() {
        overlay_packs.push(PathBuf::from(DEFAULT_OVERLAY_PACK));
    }

    Ok(Options {
        overlay_packs,
        output,
    })
}

fn env_overlay_packs() -> Vec<PathBuf> {
    let Some(value) = env::var_os("MCLONE_ASSET_OVERLAY_PACK") else {
        return Vec::new();
    };
    if value.is_empty() {
        return Vec::new();
    }
    env::split_paths(&value).collect()
}

fn print_help() {
    println!(
        "Usage: terrain_texture_coverage [--overlay-pack <path>]... [--output <path>]\n\
         \n\
         Compares first-party overlay pack textures against the native terrain atlas material set.\n\
         Defaults:\n\
           --overlay-pack {DEFAULT_OVERLAY_PACK}\n\
           --output {DEFAULT_OUTPUT}"
    );
}

fn load_overlay_pack(path: &PathBuf) -> Result<LoadedOverlayPack> {
    let pack = PackedAssetSource::from_file(path)
        .with_context(|| format!("failed to load overlay pack {}", path.display()))?;
    let texture_paths = pack
        .list("assets/", ".png")
        .with_context(|| format!("failed to list overlay pack {}", path.display()))?
        .into_iter()
        .filter(|path| {
            path.as_str()
                .starts_with("assets/minecraft/textures/block/")
        })
        .collect::<BTreeSet<_>>();
    Ok(LoadedOverlayPack {
        path: path.clone(),
        asset_set: pack.manifest().asset_set.clone(),
        file_count: pack.file_count(),
        texture_paths,
    })
}

fn coverage_report(
    overlay_packs: &[LoadedOverlayPack],
    required: &BTreeSet<AssetPath>,
    covered: &BTreeSet<AssetPath>,
    missing: &BTreeSet<AssetPath>,
    unused: &BTreeSet<AssetPath>,
) -> String {
    let mut lines = Vec::new();
    lines.push("# Terrain Texture Overlay Coverage".to_owned());
    lines.push(String::new());
    lines.push("Compares first-party overlay pack PNGs against the texture materials the native terrain atlas requests from `BlockStateRegistry::terrain_mvp()`.".to_owned());
    lines.push(String::new());
    lines.push("## Summary".to_owned());
    lines.push(String::new());
    lines.push(format!("- Required terrain textures: {}", required.len()));
    lines.push(format!(
        "- Covered by overlay: {} ({:.1}%)",
        covered.len(),
        percent(covered.len(), required.len())
    ));
    lines.push(format!("- Missing from overlay: {}", missing.len()));
    lines.push(format!("- Unused overlay block textures: {}", unused.len()));
    lines.push(String::new());
    lines.push("## Overlay Packs".to_owned());
    lines.push(String::new());
    for pack in overlay_packs {
        lines.push(format!(
            "- `{}`: asset_set=`{}`, files={}, block_texture_pngs={}",
            pack.path.display(),
            pack.asset_set,
            pack.file_count,
            pack.texture_paths.len()
        ));
    }
    push_path_section(&mut lines, "Covered Textures", covered);
    push_path_section(&mut lines, "Missing Textures", missing);
    push_path_section(&mut lines, "Unused Overlay Textures", unused);
    lines.push(String::new());
    lines.join("\n")
}

fn push_path_section(lines: &mut Vec<String>, title: &str, paths: &BTreeSet<AssetPath>) {
    lines.push(String::new());
    lines.push(format!("## {title}"));
    lines.push(String::new());
    if paths.is_empty() {
        lines.push("- none".to_owned());
        return;
    }
    for path in paths {
        lines.push(format!("- `{}`", path.as_str()));
    }
}

fn percent(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        return 100.0;
    }
    numerator as f64 * 100.0 / denominator as f64
}
