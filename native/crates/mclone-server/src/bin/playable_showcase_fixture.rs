use std::error::Error;
use std::fs;
use std::path::PathBuf;

use mclone_protocol::{ClientIdentity, PlayerProfileId};
use mclone_server::{
    PlayableShowcaseId, SqliteWorldStore, WorldStore, write_playable_showcase_to_store,
};

const DEFAULT_ROOT: &str = "/tmp/mclone-playable-showcase-fixture";
const PROFILE_ID: &str = "6d636c6f-6e65-4772-a572-64656e526576";
const PROFILE_ID_BYTES: [u8; 16] = [
    0x6d, 0x63, 0x6c, 0x6f, 0x6e, 0x65, 0x47, 0x72, 0xa5, 0x72, 0x64, 0x65, 0x6e, 0x52, 0x65, 0x76,
];

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = parse_arguments(std::env::args().skip(1))?;
    let world_dir = arguments.root.join("world");
    let profile_path = arguments.root.join("player-profile.v1.json");
    let identity = ClientIdentity::new(PlayerProfileId::new(PROFILE_ID_BYTES), "Reviewer")?;

    write_profile(&profile_path)?;
    let mut store = SqliteWorldStore::open_world_dir(&world_dir)?;
    let manifest = write_playable_showcase_to_store(&mut store, arguments.showcase, &identity)?;
    store.close()?;

    println!(
        "MCLONE_PLAYABLE_SHOWCASE_FIXTURE id={} revision={} dir={} profile={} seed={} day_time={} freeze_time={} entry_eye={} entry_target={} entity_count={}",
        manifest.id.label(),
        manifest.revision,
        world_dir.display(),
        profile_path.display(),
        manifest.seed,
        manifest.day_time,
        manifest.freeze_time,
        coordinates(manifest.entry_eye),
        coordinates(manifest.entry_look_at),
        manifest.entity_count,
    );
    Ok(())
}

struct Arguments {
    root: PathBuf,
    showcase: PlayableShowcaseId,
}

fn parse_arguments(args: impl IntoIterator<Item = String>) -> Result<Arguments, Box<dyn Error>> {
    let mut root = PathBuf::from(DEFAULT_ROOT);
    let mut showcase = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = PathBuf::from(args.next().ok_or("--root requires PATH")?),
            "--showcase" => {
                showcase = Some(PlayableShowcaseId::parse(
                    &args.next().ok_or("--showcase requires ID")?,
                )?)
            }
            "--help" | "-h" => {
                println!(
                    "playable_showcase_fixture --showcase ID [--root PATH]\n\nCompiles a playable showcase into a disposable SQLite world for native capture.\nDefault root: {DEFAULT_ROOT}"
                );
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument `{arg}`").into()),
        }
    }
    Ok(Arguments {
        root,
        showcase: showcase.ok_or("--showcase ID is required")?,
    })
}

fn coordinates(value: [f64; 3]) -> String {
    format!("{},{},{}", value[0], value[1], value[2])
}

fn write_profile(path: &std::path::Path) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        format!(
            "{{\n  \"schema\": 1,\n  \"profileId\": \"{PROFILE_ID}\",\n  \"displayName\": \"Reviewer\",\n  \"createdAtUnixMs\": 1\n}}\n"
        ),
    )?;
    Ok(())
}
