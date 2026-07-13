use std::error::Error;
use std::path::PathBuf;

use mclone_server::{AuthoredWorldFixtureKind, write_authored_world_fixture_dir};

const DEFAULT_FIXTURE_ROOT: &str = "/tmp/mclone-live-diorama-fixtures";

fn main() -> Result<(), Box<dyn Error>> {
    let root = parse_root(std::env::args().skip(1))?;
    for kind in [
        AuthoredWorldFixtureKind::Table,
        AuthoredWorldFixtureKind::Island,
    ] {
        let world_dir = root.join(kind.directory_name());
        let manifest = write_authored_world_fixture_dir(&world_dir, kind)?;
        println!(
            "MCLONE_AUTHORED_FIXTURE id={} kind={:?} seed={} generation={} dir={} spawn={:?} anchor={:?} mutation={:?}",
            manifest.fixture_id,
            manifest.kind,
            manifest.seed,
            manifest.world_generation_profile.label(),
            world_dir.display(),
            manifest.expected_spawn,
            manifest.preview_anchor,
            manifest.mutation_block,
        );
    }
    Ok(())
}

fn parse_root(args: impl IntoIterator<Item = String>) -> Result<PathBuf, Box<dyn Error>> {
    let mut root = PathBuf::from(DEFAULT_FIXTURE_ROOT);
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => {
                root = PathBuf::from(args.next().ok_or("--root requires PATH")?);
            }
            "--help" | "-h" => {
                println!(
                    "authored_world_fixtures [--root PATH]\n\nCreates or rebuilds the table-a and island-b persistent authored worlds.\nDefault root: {DEFAULT_FIXTURE_ROOT}"
                );
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument `{arg}`").into()),
        }
    }
    Ok(root)
}
