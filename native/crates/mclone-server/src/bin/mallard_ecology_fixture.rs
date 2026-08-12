use std::error::Error;
use std::fs;
use std::path::PathBuf;

use mclone_core::{ChunkRevision, Vec3d};
use mclone_protocol::{
    DimensionKey, EntityRotation, MallardFieldGuideProgress, MallardObservationKind,
};
use mclone_server::{
    AuthoredWorldFixtureKind, EntityChunkRecord, EntityPersistentId, EntitySavePayload,
    EntitySaveRecord, PlayerRecord, PlayerRecordKey, SqliteWorldStore, WorldStore,
    write_authored_world_fixture_dir,
};
use mclone_worldgen::block::{LILY_PAD, generated_block_state_id};

const DEFAULT_ROOT: &str = "/tmp/mclone-mallard-ecology-fixture";
const PROFILE_ID: &str = "6d636c6f-6e65-4272-a665-636f6c6f6779";
const PERSISTENT_ID_MOST: u64 = 0x6d63_6c6f_6e65_0279;

fn main() -> Result<(), Box<dyn Error>> {
    let root = parse_root(std::env::args().skip(1))?;
    let world_dir = root.join("world");
    let profile_path = root.join("player-profile.v1.json");
    let manifest = write_authored_world_fixture_dir(&world_dir, AuthoredWorldFixtureKind::Island)?;

    write_profile(&profile_path)?;
    let mut store = SqliteWorldStore::open_world_dir(&world_dir)?;
    let dimension = DimensionKey::overworld();
    let mut center = store
        .load_chunk(&dimension, mclone_server::AUTHORED_WORLD_FIXTURE_CENTER)?
        .ok_or("authored island fixture omitted its center chunk")?;
    center
        .snapshot
        .patch_section_block(4, 0, 1, 8, generated_block_state_id(LILY_PAD));
    center.snapshot.revision = ChunkRevision(center.snapshot.revision.0.saturating_add(1));
    store.save_chunk(&dimension, &center)?;
    store.save_entity_chunk(&dimension, &mallard_entity_record())?;

    let mut player = PlayerRecord::new(
        PlayerRecordKey::Uuid(PROFILE_ID.to_owned()),
        1,
        "Ecologist",
        Vec3d::new(7.5, 66.0, 8.5),
    );
    player.on_ground = true;
    let mut guide = MallardFieldGuideProgress::default();
    for observation in MallardObservationKind::ALL {
        guide.observe(observation);
    }
    player.mallard_field_guide = guide;
    store.save_player(&player)?;
    store.close()?;

    println!(
        "MCLONE_MALLARD_ECOLOGY_FIXTURE dir={} profile={} seed={} camera_eye=12.5,69.5,18.5 camera_target=2,65,8.5",
        world_dir.display(),
        profile_path.display(),
        manifest.seed,
    );
    Ok(())
}

fn mallard_entity_record() -> EntityChunkRecord {
    let first_parent = EntityPersistentId::new(PERSISTENT_ID_MOST, 1);
    let second_parent = EntityPersistentId::new(PERSISTENT_ID_MOST, 2);
    let parents = [Some(first_parent), Some(second_parent)];
    EntityChunkRecord::new(
        mclone_server::AUTHORED_WORLD_FIXTURE_CENTER,
        1,
        vec![
            mallard(
                first_parent,
                Vec3d::new(0.5, 64.88, 7.5),
                90.0,
                2_400,
                [None; 2],
                false,
            ),
            mallard(
                second_parent,
                Vec3d::new(0.5, 64.88, 9.5),
                90.0,
                2_400,
                [None; 2],
                false,
            ),
            mallard(
                EntityPersistentId::new(PERSISTENT_ID_MOST, 3),
                Vec3d::new(2.5, 65.0, 7.5),
                -35.0,
                320,
                parents,
                true,
            ),
            EntitySaveRecord {
                persistent_id: EntityPersistentId::new(PERSISTENT_ID_MOST, 4),
                kind: "mclone:mallard_nest".to_owned(),
                position: Vec3d::new(5.5, 66.0, 11.5),
                delta_movement: Vec3d::ZERO,
                y_rot_degrees: 0.0,
                x_rot_degrees: 0.0,
                rotation: Some(EntityRotation::IDENTITY),
                on_ground: true,
                payload: EntitySavePayload::MallardNest {
                    incubation_progress: 1_200,
                    incubation_required: 2_400,
                    parents,
                },
            },
        ],
    )
}

fn mallard(
    persistent_id: EntityPersistentId,
    position: Vec3d,
    y_rot_degrees: f32,
    age_ticks: u32,
    parents: [Option<EntityPersistentId>; 2],
    on_ground: bool,
) -> EntitySaveRecord {
    EntitySaveRecord {
        persistent_id,
        kind: "mclone:mallard".to_owned(),
        position,
        delta_movement: Vec3d::ZERO,
        y_rot_degrees,
        x_rot_degrees: 0.0,
        rotation: Some(EntityRotation::IDENTITY),
        on_ground,
        payload: EntitySavePayload::Mallard {
            egg_time: 8_000,
            age_ticks,
            parents,
            feather_time: 2_400,
            call_time: 200,
        },
    }
}

fn write_profile(path: &std::path::Path) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        format!(
            "{{\n  \"schema\": 1,\n  \"profileId\": \"{PROFILE_ID}\",\n  \"displayName\": \"Ecologist\",\n  \"createdAtUnixMs\": 1\n}}\n"
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
                    "mallard_ecology_fixture [--root PATH]\n\nCreates a repeatable persisted wetland, mallard family, attended nest, and completed field-guide player for rendered acceptance.\nDefault root: {DEFAULT_ROOT}"
                );
                return Ok(root);
            }
            _ => return Err(format!("unknown argument `{arg}`").into()),
        }
    }
    Ok(root)
}
