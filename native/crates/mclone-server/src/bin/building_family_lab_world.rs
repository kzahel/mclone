#[cfg(not(target_arch = "wasm32"))]
use std::error::Error;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use mclone_server::write_building_family_lab_dir;

#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_WORLD_ROOT: &str = "/tmp/mclone-building-family-lab-v1";

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn Error>> {
    let root = parse_root(std::env::args().skip(1))?;
    let manifest = write_building_family_lab_dir(&root)?;
    println!(
        "MCLONE_BUILDING_FAMILY_LAB id={} seed={} generation={} dir={} spawn={:?} placements={}",
        manifest.gallery_id,
        manifest.seed,
        manifest.world_generation_profile.label(),
        root.display(),
        manifest.expected_spawn,
        manifest.placements.len(),
    );
    for placement in manifest.placements {
        println!(
            "MCLONE_BUILDING_TEMPLATE id={} theme={} bounds={:?}..{:?} chunks={} blocks={} markers={}",
            placement.template_id,
            placement.theme_id,
            placement.bounds_min,
            placement.bounds_max_exclusive,
            placement.touched_chunks.len(),
            placement.block_count,
            placement.markers.len(),
        );
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    panic!("building_family_lab_world is a native-only authoring utility");
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_root(args: impl IntoIterator<Item = String>) -> Result<PathBuf, Box<dyn Error>> {
    let mut root = PathBuf::from(DEFAULT_WORLD_ROOT);
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = PathBuf::from(args.next().ok_or("--root requires PATH")?),
            "--help" | "-h" => {
                println!(
                    "building_family_lab_world [--root PATH]\n\nCreates or rebuilds the persistent bounded-family comparison gallery.\nDefault root: {DEFAULT_WORLD_ROOT}"
                );
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument `{arg}`").into()),
        }
    }
    Ok(root)
}
