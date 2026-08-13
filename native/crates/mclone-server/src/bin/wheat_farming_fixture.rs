use std::error::Error;
use std::fs;
use std::path::PathBuf;

use mclone_protocol::{ClientIdentity, PlayerProfileId};
use mclone_server::{
    AuthoredWorldFixtureKind, PlayableShowcaseId, SqliteWorldStore, WorldStore,
    write_authored_world_fixture_dir, write_playable_showcase_to_store,
};

const DEFAULT_ROOT: &str = "/tmp/mclone-wheat-farming-fixture";
const PROFILE_ID: &str = "6d636c6f-6e65-4661-a56d-65722d576865";
const PROFILE_ID_BYTES: [u8; 16] = [
    0x6d, 0x63, 0x6c, 0x6f, 0x6e, 0x65, 0x46, 0x61, 0xa5, 0x6d, 0x65, 0x72, 0x2d, 0x57, 0x68, 0x65,
];

fn main() -> Result<(), Box<dyn Error>> {
    let root = parse_root(std::env::args().skip(1))?;
    let world_dir = root.join("world");
    let profile_path = root.join("player-profile.v1.json");
    let identity = ClientIdentity::new(PlayerProfileId::new(PROFILE_ID_BYTES), "Farmer")?;

    write_authored_world_fixture_dir(&world_dir, AuthoredWorldFixtureKind::WheatFarming)?;
    write_profile(&profile_path)?;
    let mut store = SqliteWorldStore::open_world_dir(&world_dir)?;
    let manifest =
        write_playable_showcase_to_store(&mut store, PlayableShowcaseId::WheatFarming, &identity)?;
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
            "{{\n  \"schema\": 1,\n  \"profileId\": \"{PROFILE_ID}\",\n  \"displayName\": \"Farmer\",\n  \"createdAtUnixMs\": 1\n}}\n"
        ),
    )?;
    Ok(())
}

fn parse_root(args: impl IntoIterator<Item = String>) -> Result<PathBuf, Box<dyn Error>> {
    let mut root = PathBuf::from(DEFAULT_ROOT);
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = PathBuf::from(args.next().ok_or("--root requires PATH")?),
            "--help" | "-h" => {
                println!(
                    "wheat_farming_fixture [--root PATH]\n\nCompiles the wheat-farming playable showcase into a disposable SQLite world for native capture.\nDefault root: {DEFAULT_ROOT}"
                );
                return Ok(root);
            }
            _ => return Err(format!("unknown argument `{arg}`").into()),
        }
    }
    Ok(root)
}
