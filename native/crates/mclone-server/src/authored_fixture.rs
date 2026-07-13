use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mclone_core::{ChunkPos, ChunkRevision, ChunkStatus};
use mclone_worldgen::block::{BRICKS, DIRT, GRASS_BLOCK, STONE};
use mclone_worldgen::levelgen::{GeneratedChunk, MutableChunkBlockBuffer};
use serde::{Deserialize, Serialize};

use crate::light_status::{PendingLightStatus, PendingLightStatusBatch};
use crate::light_world::RetainedInitialLightState;
use crate::persistence::{ChunkRecord, SqliteWorldStore, WorldStore};
use crate::{
    AUTHORED_WORLD_HEIGHT, AUTHORED_WORLD_MIN_Y, ChunkStoreError, ChunkStoreResult,
    WorldGenerationProfile,
};

pub const AUTHORED_WORLD_FIXTURE_MARKER_FILE: &str = "mclone-authored-fixture.json";
pub const AUTHORED_WORLD_FIXTURE_SCHEMA_VERSION: u32 = 1;
pub const AUTHORED_WORLD_FIXTURE_CENTER: ChunkPos = ChunkPos::new(0, 0);
// The first preview may retain radius two; one extra authored void ring closes
// its meshing boundary. Radius three also covers the desktop client's minimum
// render-distance-two tracking view, avoiding expensive first-run lighting for
// dozens of persistence misses during independent fixture inspection.
pub const AUTHORED_WORLD_FIXTURE_VOID_PADDING_RADIUS: i32 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthoredWorldFixtureKind {
    Table,
    Island,
}

impl AuthoredWorldFixtureKind {
    pub const fn fixture_id(self) -> &'static str {
        match self {
            Self::Table => "live-diorama-table-a-v1",
            Self::Island => "live-diorama-island-b-v1",
        }
    }

    pub const fn directory_name(self) -> &'static str {
        match self {
            Self::Table => "table-a",
            Self::Island => "island-b",
        }
    }

    pub const fn seed(self) -> i64 {
        match self {
            Self::Table => 17_501,
            Self::Island => 17_502,
        }
    }

    pub const fn expected_spawn(self) -> [f64; 3] {
        match self {
            Self::Table => [0.5, 64.0, 0.5],
            Self::Island => [1.5, 64.0, 8.5],
        }
    }

    pub const fn preview_anchor(self) -> [f64; 3] {
        match self {
            Self::Table => [8.5, 68.0, 8.5],
            Self::Island => [8.5, 65.0, 8.5],
        }
    }

    pub const fn mutation_block(self) -> [i32; 3] {
        match self {
            Self::Table => [8, 67, 8],
            Self::Island => [13, 67, 10],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoredWorldFixtureManifest {
    pub schema_version: u32,
    pub fixture_id: String,
    pub kind: AuthoredWorldFixtureKind,
    pub seed: i64,
    pub world_generation_profile: WorldGenerationProfile,
    pub center_chunk: [i32; 2],
    pub void_padding_radius: i32,
    pub preview_min_section_y: i32,
    pub preview_max_section_y: i32,
    pub expected_spawn: [f64; 3],
    pub preview_anchor: [f64; 3],
    pub mutation_block: [i32; 3],
}

impl AuthoredWorldFixtureManifest {
    pub fn new(kind: AuthoredWorldFixtureKind) -> Self {
        Self {
            schema_version: AUTHORED_WORLD_FIXTURE_SCHEMA_VERSION,
            fixture_id: kind.fixture_id().to_owned(),
            kind,
            seed: kind.seed(),
            world_generation_profile: WorldGenerationProfile::authored_only(),
            center_chunk: [
                AUTHORED_WORLD_FIXTURE_CENTER.x,
                AUTHORED_WORLD_FIXTURE_CENTER.z,
            ],
            void_padding_radius: AUTHORED_WORLD_FIXTURE_VOID_PADDING_RADIUS,
            preview_min_section_y: 3,
            preview_max_section_y: 5,
            expected_spawn: kind.expected_spawn(),
            preview_anchor: kind.preview_anchor(),
            mutation_block: kind.mutation_block(),
        }
    }
}

pub fn authored_world_fixture_records(
    kind: AuthoredWorldFixtureKind,
) -> ChunkStoreResult<(AuthoredWorldFixtureManifest, Vec<ChunkRecord>)> {
    let manifest = AuthoredWorldFixtureManifest::new(kind);
    let mut chunks = BTreeMap::new();
    for chunk_z in
        -AUTHORED_WORLD_FIXTURE_VOID_PADDING_RADIUS..=AUTHORED_WORLD_FIXTURE_VOID_PADDING_RADIUS
    {
        for chunk_x in
            -AUTHORED_WORLD_FIXTURE_VOID_PADDING_RADIUS..=AUTHORED_WORLD_FIXTURE_VOID_PADDING_RADIUS
        {
            let pos = ChunkPos::new(chunk_x, chunk_z);
            let mut buffer = MutableChunkBlockBuffer::new(
                chunk_x,
                chunk_z,
                AUTHORED_WORLD_MIN_Y,
                AUTHORED_WORLD_HEIGHT,
            );
            if pos == AUTHORED_WORLD_FIXTURE_CENTER {
                match kind {
                    AuthoredWorldFixtureKind::Table => author_table_chunk(&mut buffer),
                    AuthoredWorldFixtureKind::Island => author_island_chunk(&mut buffer),
                }
            }
            chunks.insert(pos, GeneratedChunk::from_mutable_buffer(buffer));
        }
    }

    let mut pending = Vec::with_capacity(chunks.len());
    for (index, (pos, chunk)) in chunks.iter().enumerate() {
        let revision = ChunkRevision(index as u64 + 1);
        let snapshot = chunk.to_chunk_snapshot(revision, ChunkStatus::Features);
        let neighbor_blocks = chunks
            .iter()
            .filter(|(neighbor, _)| {
                **neighbor != *pos
                    && (neighbor.x - pos.x).abs() <= 1
                    && (neighbor.z - pos.z).abs() <= 1
            })
            .map(|(neighbor, chunk)| (*neighbor, chunk.blocks().to_vec()))
            .collect();
        pending.push(PendingLightStatus::from_parts(
            *pos,
            snapshot,
            chunk.blocks().to_vec(),
            neighbor_blocks,
        ));
    }

    let mut light_state = RetainedInitialLightState::new();
    let records = light_state
        .compute_batch(PendingLightStatusBatch::new(pending))
        .into_iter()
        .map(|(status, light_sections, _)| {
            let mut snapshot = status.feature_snapshot;
            snapshot.status = ChunkStatus::Light;
            ChunkRecord::from_snapshot(snapshot.with_light_sections(true, light_sections))
        })
        .collect();
    Ok((manifest, records))
}

pub fn write_authored_world_fixture_to_store(
    store: &mut dyn WorldStore,
    kind: AuthoredWorldFixtureKind,
) -> ChunkStoreResult<AuthoredWorldFixtureManifest> {
    let (manifest, records) = authored_world_fixture_records(kind)?;
    for record in &records {
        store.save_chunk(record)?;
    }
    store.flush()?;
    Ok(manifest)
}

pub fn write_authored_world_fixture_dir(
    root: impl AsRef<Path>,
    kind: AuthoredWorldFixtureKind,
) -> ChunkStoreResult<AuthoredWorldFixtureManifest> {
    let root = root.as_ref();
    let expected = AuthoredWorldFixtureManifest::new(kind);
    validate_fixture_root_for_rebuild(root, &expected)?;
    fs::create_dir_all(root)?;
    remove_existing_fixture_database(root)?;

    let mut store = SqliteWorldStore::open_world_dir(root)?;
    let manifest = write_authored_world_fixture_to_store(&mut store, kind)?;
    store.close()?;
    let marker = serde_json::to_vec_pretty(&manifest).map_err(|error| {
        ChunkStoreError::InvalidData(format!("failed to encode authored fixture marker: {error}"))
    })?;
    fs::write(root.join(AUTHORED_WORLD_FIXTURE_MARKER_FILE), marker)?;
    Ok(manifest)
}

pub fn authored_world_fixture_marker_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join(AUTHORED_WORLD_FIXTURE_MARKER_FILE)
}

fn validate_fixture_root_for_rebuild(
    root: &Path,
    expected: &AuthoredWorldFixtureManifest,
) -> ChunkStoreResult<()> {
    if !root.exists() {
        return Ok(());
    }
    if !root.is_dir() {
        return Err(ChunkStoreError::InvalidData(format!(
            "authored fixture root `{}` is not a directory",
            root.display()
        )));
    }

    let marker_path = authored_world_fixture_marker_path(root);
    if marker_path.exists() {
        let bytes = fs::read(&marker_path)?;
        let existing: AuthoredWorldFixtureManifest =
            serde_json::from_slice(&bytes).map_err(|error| {
                ChunkStoreError::InvalidData(format!(
                    "failed to parse authored fixture marker `{}`: {error}",
                    marker_path.display()
                ))
            })?;
        if existing.schema_version != AUTHORED_WORLD_FIXTURE_SCHEMA_VERSION
            || existing.fixture_id != expected.fixture_id
            || existing.kind != expected.kind
        {
            return Err(ChunkStoreError::InvalidData(format!(
                "authored fixture root `{}` belongs to fixture `{}` schema {}, not `{}` schema {}",
                root.display(),
                existing.fixture_id,
                existing.schema_version,
                expected.fixture_id,
                expected.schema_version
            )));
        }
        return Ok(());
    }

    if fs::read_dir(root)?.next().transpose()?.is_some() {
        return Err(ChunkStoreError::InvalidData(format!(
            "refusing to overwrite unrelated non-fixture world root `{}`",
            root.display()
        )));
    }
    Ok(())
}

fn remove_existing_fixture_database(root: &Path) -> ChunkStoreResult<()> {
    let database = SqliteWorldStore::database_path_for_world_dir(root);
    for path in [
        database.clone(),
        PathBuf::from(format!("{}-wal", database.display())),
        PathBuf::from(format!("{}-shm", database.display())),
    ] {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn author_table_chunk(chunk: &mut MutableChunkBlockBuffer) {
    for z in 0..16 {
        for x in 0..16 {
            chunk.set_block_at_y(x, 60, z, STONE);
            chunk.set_block_at_y(x, 61, z, STONE);
            chunk.set_block_at_y(x, 62, z, DIRT);
            chunk.set_block_at_y(x, 63, z, GRASS_BLOCK);
        }
    }

    for &(x, z) in &[(6, 6), (6, 10), (10, 6), (10, 10)] {
        for y in 64..=66 {
            chunk.set_block_at_y(x, y, z, BRICKS);
        }
    }
    for z in 6..=10 {
        for x in 6..=10 {
            chunk.set_block_at_y(x, 67, z, BRICKS);
        }
    }
}

fn author_island_chunk(chunk: &mut MutableChunkBlockBuffer) {
    for z in 0..16 {
        for x in 0..16 {
            let dx = x - 8;
            let dz = z - 8;
            let distance_squared = dx * dx + dz * dz;
            if distance_squared > 49 {
                continue;
            }
            let surface_y = if distance_squared <= 25 { 64 } else { 63 };
            for y in 58..surface_y - 1 {
                chunk.set_block_at_y(x, y, z, STONE);
            }
            chunk.set_block_at_y(x, surface_y - 1, z, DIRT);
            chunk.set_block_at_y(x, surface_y, z, GRASS_BLOCK);
        }
    }

    for z in 9..=11 {
        for x in 11..=13 {
            let top = 65 + (x + z) % 3;
            for y in 65..=top {
                chunk.set_block_at_y(x, y, z, STONE);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use mclone_core::{AIR_BLOCK_STATE_ID, BlockPos, BlockStateId, ChunkSnapshot, Vec3d};
    use mclone_protocol::{ChunkView, ClientCommand, ServerUpdate};
    use mclone_worldgen::block::{AIR, generated_block_state_id};

    use super::*;
    use crate::IntegratedServer;

    static TEST_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn fixture_records_are_lit_persistent_chunks_with_void_padding() {
        for kind in [
            AuthoredWorldFixtureKind::Table,
            AuthoredWorldFixtureKind::Island,
        ] {
            let (manifest, records) = authored_world_fixture_records(kind).unwrap();
            assert_eq!(
                records.len(),
                (AUTHORED_WORLD_FIXTURE_VOID_PADDING_RADIUS * 2 + 1).pow(2) as usize
            );
            assert_eq!(
                manifest.world_generation_profile,
                WorldGenerationProfile::authored_only()
            );
            for record in &records {
                assert_eq!(record.snapshot.status, ChunkStatus::Light);
                assert!(record.snapshot.light_correct);
                assert_eq!(record.snapshot.min_y, AUTHORED_WORLD_MIN_Y);
                assert_eq!(record.snapshot.height, AUTHORED_WORLD_HEIGHT);
            }
            for record in records
                .iter()
                .filter(|record| record.pos() != AUTHORED_WORLD_FIXTURE_CENTER)
            {
                assert!(snapshot_is_all_air(&record.snapshot));
            }
            let center = records
                .iter()
                .find(|record| record.pos() == AUTHORED_WORLD_FIXTURE_CENTER)
                .unwrap();
            assert!(!snapshot_is_all_air(&center.snapshot));
            let mutation = manifest.mutation_block;
            assert_ne!(
                snapshot_block_state(
                    &center.snapshot,
                    BlockPos::new(mutation[0], mutation[1], mutation[2])
                ),
                AIR_BLOCK_STATE_ID
            );
        }
    }

    #[test]
    fn fixture_directory_is_idempotent_and_refuses_unrelated_roots() {
        let root = unique_test_root("root-safety");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("unrelated.txt"), b"keep").unwrap();
        let error =
            write_authored_world_fixture_dir(&root, AuthoredWorldFixtureKind::Table).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("refusing to overwrite unrelated")
        );
        assert_eq!(fs::read(root.join("unrelated.txt")).unwrap(), b"keep");
        fs::remove_dir_all(&root).unwrap();

        let manifest =
            write_authored_world_fixture_dir(&root, AuthoredWorldFixtureKind::Table).unwrap();
        assert_eq!(
            write_authored_world_fixture_dir(&root, AuthoredWorldFixtureKind::Table).unwrap(),
            manifest
        );
        let error =
            write_authored_world_fixture_dir(&root, AuthoredWorldFixtureKind::Island).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(AuthoredWorldFixtureKind::Table.fixture_id())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sqlite_fixtures_spawn_without_worldgen_and_persist_runtime_mutation() {
        for kind in [
            AuthoredWorldFixtureKind::Table,
            AuthoredWorldFixtureKind::Island,
        ] {
            let root = unique_test_root(kind.directory_name());
            let manifest = write_authored_world_fixture_dir(&root, kind).unwrap();
            let mutation = manifest.mutation_block;
            let mutation = crate::WorldBlockPos::new(mutation[0], mutation[1], mutation[2]);

            {
                let mut server =
                    IntegratedServer::try_with_threaded_sqlite_world_dir(manifest.seed, &root)
                        .unwrap();
                server
                    .set_world_generation_profile(manifest.world_generation_profile)
                    .unwrap();
                let updates = load_view_until_idle(&mut server, AUTHORED_WORLD_FIXTURE_CENTER);
                let expected = manifest.expected_spawn;
                assert!(updates.iter().any(|update| matches!(
                    update,
                    ServerUpdate::PlayerPosition(position)
                        if position.position == Vec3d::new(expected[0], expected[1], expected[2])
                )));
                assert_eq!(server.scheduler().job_count(), 0);
                assert_eq!(server.scheduler().worldgen_mailbox_pending_count(), 0);
                assert!(server.scheduler_mut().set_block_at_world(mutation, AIR));
                server.shutdown_persistence().unwrap();
            }

            {
                let mut server =
                    IntegratedServer::try_with_threaded_sqlite_world_dir(manifest.seed, &root)
                        .unwrap();
                server
                    .set_world_generation_profile(manifest.world_generation_profile)
                    .unwrap();
                load_view_until_idle(&mut server, AUTHORED_WORLD_FIXTURE_CENTER);
                assert_eq!(server.scheduler().block_at_world(mutation), Some(AIR));
                server.shutdown_persistence().unwrap();
            }
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn authored_fixture_missing_chunk_is_lit_void_after_stored_world_load() {
        let root = unique_test_root("missing-void");
        let manifest =
            write_authored_world_fixture_dir(&root, AuthoredWorldFixtureKind::Table).unwrap();
        let mut server =
            IntegratedServer::try_with_threaded_sqlite_world_dir(manifest.seed, &root).unwrap();
        server
            .set_world_generation_profile(manifest.world_generation_profile)
            .unwrap();
        let missing = ChunkPos::new(4, -3);
        let updates = load_view_until_idle(&mut server, missing);
        let snapshot = updates
            .iter()
            .find_map(|update| match update {
                ServerUpdate::ChunkSnapshot(snapshot) if snapshot.pos == missing => Some(snapshot),
                _ => None,
            })
            .expect("missing authored chunk should publish a void snapshot");
        assert_eq!(snapshot.status, ChunkStatus::Light);
        assert!(snapshot.light_correct);
        assert!(snapshot_is_all_air(snapshot));
        assert_eq!(server.scheduler().job_count(), 0);
        assert_eq!(server.scheduler().worldgen_mailbox_pending_count(), 0);
        server.shutdown_persistence().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stored_authored_chunk_publishes_when_runtime_lighting_is_disabled() {
        let root = unique_test_root("unlit-runtime");
        let manifest =
            write_authored_world_fixture_dir(&root, AuthoredWorldFixtureKind::Table).unwrap();
        let mut server =
            IntegratedServer::try_with_threaded_sqlite_world_dir(manifest.seed, &root).unwrap();
        server
            .set_world_generation_profile(manifest.world_generation_profile)
            .unwrap();
        server.set_lighting_enabled(false);

        let updates = load_view_until_idle(&mut server, AUTHORED_WORLD_FIXTURE_CENTER);
        let snapshot = updates
            .iter()
            .find_map(|update| match update {
                ServerUpdate::ChunkSnapshot(snapshot)
                    if snapshot.pos == AUTHORED_WORLD_FIXTURE_CENTER =>
                {
                    Some(snapshot)
                }
                _ => None,
            })
            .expect("stored authored chunk should publish with runtime lighting disabled");
        assert_eq!(snapshot.status, ChunkStatus::Light);
        assert!(!snapshot_is_all_air(snapshot));
        assert_eq!(server.scheduler().job_count(), 0);

        server.shutdown_persistence().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    fn load_view_until_idle(server: &mut IntegratedServer, center: ChunkPos) -> Vec<ServerUpdate> {
        let mut updates = server.handle_command(ClientCommand::SetChunkView(ChunkView {
            center,
            render_distance: 0,
            chunk_tracking_radius: 0,
        }));
        for _ in 0..500 {
            updates.extend(server.poll());
            if server.pending_job_count() == 0
                && server.pending_publication_count() == 0
                && server.scheduler().pending_persistence_load_count() == 0
            {
                return updates;
            }
            if server.scheduler().light_status_mailbox_pending_count() > 0 {
                assert!(server.wait_for_light_completion(Duration::from_secs(5)));
            }
            std::thread::yield_now();
        }
        panic!("fixture server did not become idle");
    }

    fn snapshot_is_all_air(snapshot: &ChunkSnapshot) -> bool {
        snapshot.sections.iter().all(|section| {
            section
                .unpack_block_state_ids()
                .into_iter()
                .all(|state| state == AIR_BLOCK_STATE_ID)
        })
    }

    fn snapshot_block_state(snapshot: &ChunkSnapshot, pos: BlockPos) -> BlockStateId {
        let section_y = pos.y.div_euclid(16);
        let local_x = pos.x.rem_euclid(16);
        let local_y = pos.y.rem_euclid(16);
        let local_z = pos.z.rem_euclid(16);
        snapshot
            .sections
            .iter()
            .find(|section| section.section_y == section_y)
            .map(|section| {
                section.unpack_block_state_ids()
                    [mclone_core::chunk_section_index(local_x, local_y, local_z)]
            })
            .unwrap_or_else(|| generated_block_state_id(0))
    }

    fn unique_test_root(label: &str) -> PathBuf {
        let sequence = TEST_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "mclone-authored-fixture-{label}-{}-{sequence}",
            std::process::id()
        ))
    }
}
