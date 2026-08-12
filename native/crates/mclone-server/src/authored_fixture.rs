use std::collections::BTreeMap;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use mclone_core::{ChunkPos, ChunkRevision, ChunkStatus, Vec3d, block_to_chunk_coord};
use mclone_protocol::EntityRotation;
use mclone_worldgen::block::{BRICKS, DIRT, GRASS_BLOCK, SAND, STONE, WATER};
use mclone_worldgen::levelgen::{GeneratedChunk, MutableChunkBlockBuffer};
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use crate::ChunkStoreError;
use crate::light_status::{PendingLightStatus, PendingLightStatusBatch};
use crate::light_world::RetainedInitialLightState;
#[cfg(not(target_arch = "wasm32"))]
use crate::persistence::SqliteWorldStore;
use crate::persistence::{
    ChunkRecord, EntityChunkRecord, EntityPersistentId, EntitySavePayload, EntitySaveRecord,
};
use crate::persistence::{MemoryWorldStore, WorldStore};
use crate::{
    AUTHORED_WORLD_HEIGHT, AUTHORED_WORLD_MIN_Y, ChunkStoreResult, WorldGenerationProfile,
};

pub const AUTHORED_WORLD_FIXTURE_MARKER_FILE: &str = "mclone-authored-fixture.json";
pub const AUTHORED_WORLD_FIXTURE_SCHEMA_VERSION: u32 = 1;
pub const AUTHORED_WORLD_FIXTURE_CENTER: ChunkPos = ChunkPos::new(0, 0);
// The first preview may retain radius two; one extra authored void ring closes
// its meshing boundary. Radius three also covers the desktop client's minimum
// render-distance-two tracking view, avoiding expensive first-run lighting for
// dozens of persistence misses during independent fixture inspection.
pub const AUTHORED_WORLD_FIXTURE_VOID_PADDING_RADIUS: i32 = 3;
pub const AUTHORED_LOBBY_COW_PERSISTENT_ID: EntityPersistentId =
    EntityPersistentId::new(0x6d63_6c6f_6e65_0002, 1);
pub const AUTHORED_LOBBY_CHICKEN_PERSISTENT_ID: EntityPersistentId =
    EntityPersistentId::new(0x6d63_6c6f_6e65_0002, 2);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthoredWorldFixtureKind {
    Table,
    Island,
    MallardWetland,
    DeerForestEdge,
    BeeFloweringMeadow,
    LobbyTableV2,
    LobbyIslandV2,
}

impl AuthoredWorldFixtureKind {
    pub const fn fixture_id(self) -> &'static str {
        match self {
            Self::Table => "live-diorama-table-a-v1",
            Self::Island => "live-diorama-island-b-v1",
            Self::MallardWetland => "mallard-wetland-v1",
            Self::DeerForestEdge => "deer-forest-edge-v1",
            Self::BeeFloweringMeadow => "bee-flowering-meadow-v1",
            Self::LobbyTableV2 => "lobby-table-a-v2",
            Self::LobbyIslandV2 => "lobby-island-b-v2",
        }
    }

    pub const fn directory_name(self) -> &'static str {
        match self {
            Self::Table => "table-a",
            Self::Island => "island-b",
            Self::MallardWetland => "mallard-wetland-v1",
            Self::DeerForestEdge => "deer-forest-edge-v1",
            Self::BeeFloweringMeadow => "bee-flowering-meadow-v1",
            Self::LobbyTableV2 => "lobby-table-a-v2",
            Self::LobbyIslandV2 => "lobby-island-b-v2",
        }
    }

    pub const fn seed(self) -> i64 {
        match self {
            Self::Table | Self::LobbyTableV2 => 17_501,
            Self::Island | Self::LobbyIslandV2 => 17_502,
            Self::MallardWetland => 17_503,
            Self::DeerForestEdge => 17_504,
            Self::BeeFloweringMeadow => 17_505,
        }
    }

    pub const fn expected_spawn(self) -> [f64; 3] {
        match self {
            Self::Table | Self::LobbyTableV2 => [6.5, 64.0, 7.5],
            Self::Island | Self::LobbyIslandV2 => [7.5, 66.0, 7.5],
            Self::MallardWetland => [8.5, 65.0, 18.5],
            Self::DeerForestEdge => [8.5, 65.0, 29.5],
            Self::BeeFloweringMeadow => [8.5, 65.0, 24.5],
        }
    }

    pub const fn preview_anchor(self) -> [f64; 3] {
        match self {
            // The table is a two-by-two brick plinth whose top is y=65. Keep
            // the composition plane a small, deliberate distance above that
            // surface instead of making it coplanar and depth-unstable.
            Self::Table | Self::LobbyTableV2 => [8.0, 65.03125, 8.0],
            Self::Island | Self::LobbyIslandV2 => [8.5, 65.0, 8.5],
            Self::MallardWetland => [8.5, 64.88, 7.5],
            Self::DeerForestEdge => [8.5, 65.0, 8.5],
            Self::BeeFloweringMeadow => [8.5, 65.0, 8.5],
        }
    }

    /// Tabletop composition point used when this fixture is the active world.
    ///
    /// The original Table fixture's preview anchor already has this meaning.
    /// Island gains its paired return display in Tactical 175 Slice 6.
    pub const fn preview_display_anchor(self) -> [f64; 3] {
        match self {
            Self::Table | Self::LobbyTableV2 => self.preview_anchor(),
            Self::Island | Self::LobbyIslandV2 => [4.0, 67.03125, 8.0],
            Self::MallardWetland => self.preview_anchor(),
            Self::DeerForestEdge => self.preview_anchor(),
            Self::BeeFloweringMeadow => self.preview_anchor(),
        }
    }

    pub const fn mutation_block(self) -> [i32; 3] {
        match self {
            Self::Table | Self::LobbyTableV2 => [7, 64, 7],
            // A visible grass block inside the source region and ordinary
            // debug-creative reach of the fixture's accepted spawn.
            Self::Island | Self::LobbyIslandV2 => [5, 65, 8],
            Self::MallardWetland => [8, 64, 18],
            Self::DeerForestEdge => [8, 64, 29],
            Self::BeeFloweringMeadow => [8, 64, 24],
        }
    }

    pub const fn has_authored_passive_entities(self) -> bool {
        matches!(self, Self::LobbyIslandV2)
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
) -> ChunkStoreResult<(
    AuthoredWorldFixtureManifest,
    Vec<ChunkRecord>,
    Vec<EntityChunkRecord>,
)> {
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
            match kind {
                AuthoredWorldFixtureKind::Table | AuthoredWorldFixtureKind::LobbyTableV2 => {
                    author_flat_grass_chunk(&mut buffer);
                    if pos == AUTHORED_WORLD_FIXTURE_CENTER {
                        author_table_display(&mut buffer);
                    } else if pos == ChunkPos::new(0, -1) {
                        author_table_comparison_pool(&mut buffer, 14..=15);
                    } else if pos == ChunkPos::new(0, 1) {
                        author_table_comparison_pool(&mut buffer, 0..=1);
                    }
                }
                AuthoredWorldFixtureKind::Island | AuthoredWorldFixtureKind::LobbyIslandV2
                    if pos == AUTHORED_WORLD_FIXTURE_CENTER =>
                {
                    author_island_chunk(&mut buffer);
                }
                AuthoredWorldFixtureKind::Island | AuthoredWorldFixtureKind::LobbyIslandV2 => {}
                AuthoredWorldFixtureKind::MallardWetland => {
                    author_mallard_wetland_chunk(&mut buffer)
                }
                AuthoredWorldFixtureKind::DeerForestEdge => {
                    author_deer_forest_edge_chunk(&mut buffer)
                }
                AuthoredWorldFixtureKind::BeeFloweringMeadow => {
                    author_bee_flowering_meadow_chunk(&mut buffer)
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
    let entity_records = if kind.has_authored_passive_entities() {
        vec![authored_lobby_island_entities()]
    } else {
        Vec::new()
    };
    Ok((manifest, records, entity_records))
}

pub fn write_authored_world_fixture_to_store(
    store: &mut dyn WorldStore,
    kind: AuthoredWorldFixtureKind,
) -> ChunkStoreResult<AuthoredWorldFixtureManifest> {
    let (manifest, records, entity_records) = authored_world_fixture_records(kind)?;
    for record in &records {
        store.save_chunk(&mclone_protocol::DimensionKey::overworld(), record)?;
    }
    for record in &entity_records {
        store.save_entity_chunk(&mclone_protocol::DimensionKey::overworld(), record)?;
    }
    store.flush()?;
    Ok(manifest)
}

/// Build one fresh session-local authored world through the ordinary
/// `WorldStore` contract.
///
/// The returned store has no path or durable identity. Callers hand it to the
/// normal persistence mailbox/actor just like any other store; native and
/// browser startup use the same authored records without routing them through
/// platform glue.
pub fn authored_world_fixture_memory_store(
    kind: AuthoredWorldFixtureKind,
) -> ChunkStoreResult<(AuthoredWorldFixtureManifest, MemoryWorldStore)> {
    let mut store = MemoryWorldStore::new();
    let manifest = write_authored_world_fixture_to_store(&mut store, kind)?;
    Ok((manifest, store))
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
pub fn authored_world_fixture_marker_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join(AUTHORED_WORLD_FIXTURE_MARKER_FILE)
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

fn author_flat_grass_chunk(chunk: &mut MutableChunkBlockBuffer) {
    for z in 0..16 {
        for x in 0..16 {
            chunk.set_block_at_y(x, 60, z, STONE);
            chunk.set_block_at_y(x, 61, z, STONE);
            chunk.set_block_at_y(x, 62, z, DIRT);
            chunk.set_block_at_y(x, 63, z, GRASS_BLOCK);
        }
    }
}

fn author_table_display(chunk: &mut MutableChunkBlockBuffer) {
    // Four blocks on the grass make a player-scale display plinth. The live
    // miniature is placed just above its y=65 top surface.
    for z in 7..=8 {
        for x in 7..=8 {
            chunk.set_block_at_y(x, 64, z, BRICKS);
        }
    }
}

fn author_table_comparison_pool(
    chunk: &mut MutableChunkBlockBuffer,
    local_z: std::ops::RangeInclusive<i32>,
) {
    // The two pools occupy distinct A chunk/section keys around the z=8 table,
    // allowing B's placed water section to sort between them. Their y=65
    // surfaces remain 1/32 block below the preview baseline.
    for z in local_z {
        for x in 6..=9 {
            chunk.set_block_at_y(x, 63, z, DIRT);
            chunk.set_block_at_y(x, 64, z, WATER);
        }
    }
}

fn author_island_chunk(chunk: &mut MutableChunkBlockBuffer) {
    for z in 0..16 {
        for x in 0..16 {
            // A persisted two-block-deep ocean surrounds the island. This is
            // ordinary authored world data, so the normal client mesh and live
            // mutation paths own it exactly like generated terrain.
            for y in 58..62 {
                chunk.set_block_at_y(x, y, z, STONE);
            }
            chunk.set_block_at_y(x, 62, z, SAND);
            chunk.set_block_at_y(x, 63, z, WATER);
            chunk.set_block_at_y(x, 64, z, WATER);

            let dx = x - 8;
            let dz = z - 8;
            let distance_squared = dx * dx + dz * dz;
            if distance_squared > 49 {
                continue;
            }
            let surface_y = if distance_squared <= 25 { 65 } else { 64 };
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

    // Paired four-block display for showing world A after the warm whole-slot
    // selection makes this island the active world. Its top is y=67.
    for z in 7..=8 {
        for x in 3..=4 {
            chunk.set_block_at_y(x, 66, z, BRICKS);
        }
    }
}

fn author_mallard_wetland_chunk(chunk: &mut MutableChunkBlockBuffer) {
    for local_z in 0..16 {
        for local_x in 0..16 {
            let world_x = chunk.chunk_x * 16 + local_x;
            let world_z = chunk.chunk_z * 16 + local_z;
            for y in 59..=61 {
                chunk.set_block_at_y(local_x, y, local_z, STONE);
            }
            chunk.set_block_at_y(local_x, 62, local_z, DIRT);
            chunk.set_block_at_y(local_x, 63, local_z, DIRT);

            // A long, one-block-deep pond gives retained flock destinations
            // room to read while keeping every bank reachable on foot. The
            // integer ellipse is exact across chunk boundaries.
            let dx = world_x - 8;
            let dz = world_z - 3;
            let shallow_pond = dx * dx * 81 + dz * dz * 576 <= 46_656;
            chunk.set_block_at_y(
                local_x,
                64,
                local_z,
                if shallow_pond { WATER } else { GRASS_BLOCK },
            );
        }
    }
}

fn author_deer_forest_edge_chunk(chunk: &mut MutableChunkBlockBuffer) {
    use mclone_worldgen::block::{AIR, OAK_LEAVES, OAK_LOG};

    for local_z in 0..16 {
        for local_x in 0..16 {
            let world_x = chunk.chunk_x * 16 + local_x;
            let world_z = chunk.chunk_z * 16 + local_z;
            for y in 59..=62 {
                chunk.set_block_at_y(local_x, y, local_z, STONE);
            }
            chunk.set_block_at_y(local_x, 63, local_z, DIRT);
            let stream = (world_x - 8).abs() <= 1 && (-11..=-2).contains(&world_z);
            chunk.set_block_at_y(
                local_x,
                64,
                local_z,
                if stream { WATER } else { GRASS_BLOCK },
            );
            if stream {
                chunk.set_block_at_y(local_x, 65, local_z, AIR);
            }
        }
    }

    // Clumped trunks around the north, east, and west shoulders leave a
    // broad south-facing meadow and connected central sight line. The same
    // live blocks drive cover, safe-bank, browse, and bedding queries.
    for (x, z) in [
        (-8, -6),
        (-3, -9),
        (3, -8),
        (9, -6),
        (15, -4),
        (20, 1),
        (-9, 2),
        (-5, 8),
        (19, 9),
        (23, 14),
        (-7, 15),
        (21, 21),
    ] {
        if block_to_chunk_coord(x) != chunk.chunk_x || block_to_chunk_coord(z) != chunk.chunk_z {
            continue;
        }
        let local_x = x.rem_euclid(16);
        let local_z = z.rem_euclid(16);
        for y in 65..=68 {
            chunk.set_block_at_y(local_x, y, local_z, OAK_LOG);
        }
        for dz in -2_i32..=2 {
            for dx in -2_i32..=2 {
                for y in 68..=70 {
                    if dx.abs() + dz.abs() > 3 {
                        continue;
                    }
                    let leaf_x = x + dx;
                    let leaf_z = z + dz;
                    if block_to_chunk_coord(leaf_x) == chunk.chunk_x
                        && block_to_chunk_coord(leaf_z) == chunk.chunk_z
                    {
                        chunk.set_block_at_y(
                            leaf_x.rem_euclid(16),
                            y,
                            leaf_z.rem_euclid(16),
                            OAK_LEAVES,
                        );
                    }
                }
            }
        }
    }
}

fn author_bee_flowering_meadow_chunk(chunk: &mut MutableChunkBlockBuffer) {
    use mclone_worldgen::block::{DANDELION, OAK_LEAVES, OAK_LOG, POPPY};

    for local_z in 0..16 {
        for local_x in 0..16 {
            let world_x = chunk.chunk_x * 16 + local_x;
            let world_z = chunk.chunk_z * 16 + local_z;
            for y in 59..=62 {
                chunk.set_block_at_y(local_x, y, local_z, STONE);
            }
            chunk.set_block_at_y(local_x, 63, local_z, DIRT);
            chunk.set_block_at_y(local_x, 64, local_z, GRASS_BLOCK);

            let in_patch = [(4, 8), (13, 8), (8, 17), (-7, 5), (22, 15)]
                .into_iter()
                .any(|(center_x, center_z)| {
                    let dx = world_x - center_x;
                    let dz = world_z - center_z;
                    dx * dx + dz * dz <= 18
                });
            let flower_cell = (world_x * 17 + world_z * 31).rem_euclid(7) <= 2;
            if in_patch && flower_cell {
                chunk.set_block_at_y(
                    local_x,
                    65,
                    local_z,
                    if (world_x + world_z).rem_euclid(3) == 0 {
                        POPPY
                    } else {
                        DANDELION
                    },
                );
            }
        }
    }

    // Sparse perimeter trees provide real woody shelter while leaving a broad
    // central flight corridor and an unobstructed south-facing review view.
    for (x, z) in [(-7, -1), (23, 1), (-10, 18), (26, 22), (8, -10)] {
        if block_to_chunk_coord(x) != chunk.chunk_x || block_to_chunk_coord(z) != chunk.chunk_z {
            continue;
        }
        let local_x = x.rem_euclid(16);
        let local_z = z.rem_euclid(16);
        for y in 65..=68 {
            chunk.set_block_at_y(local_x, y, local_z, OAK_LOG);
        }
        for dz in -2_i32..=2 {
            for dx in -2_i32..=2 {
                if dx.abs() + dz.abs() > 3 {
                    continue;
                }
                for y in 68..=70 {
                    let leaf_x = x + dx;
                    let leaf_z = z + dz;
                    if block_to_chunk_coord(leaf_x) == chunk.chunk_x
                        && block_to_chunk_coord(leaf_z) == chunk.chunk_z
                    {
                        chunk.set_block_at_y(
                            leaf_x.rem_euclid(16),
                            y,
                            leaf_z.rem_euclid(16),
                            OAK_LEAVES,
                        );
                    }
                }
            }
        }
    }
}

fn authored_lobby_island_entities() -> EntityChunkRecord {
    EntityChunkRecord::new(
        AUTHORED_WORLD_FIXTURE_CENTER,
        1,
        vec![
            EntitySaveRecord {
                persistent_id: AUTHORED_LOBBY_COW_PERSISTENT_ID,
                kind: "minecraft:cow".to_owned(),
                position: Vec3d::new(6.5, 66.0, 8.5),
                delta_movement: Vec3d::ZERO,
                y_rot_degrees: 90.0,
                x_rot_degrees: 0.0,
                rotation: Some(EntityRotation::IDENTITY),
                on_ground: true,
                animation: None,
                payload: EntitySavePayload::Cow,
            },
            EntitySaveRecord {
                persistent_id: AUTHORED_LOBBY_CHICKEN_PERSISTENT_ID,
                kind: "minecraft:chicken".to_owned(),
                position: Vec3d::new(10.5, 66.0, 8.5),
                delta_movement: Vec3d::ZERO,
                y_rot_degrees: -90.0,
                x_rot_degrees: 0.0,
                rotation: Some(EntityRotation::IDENTITY),
                on_ground: true,
                animation: None,
                payload: EntitySavePayload::Chicken { egg_time: 6_000 },
            },
        ],
    )
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use mclone_core::{AIR_BLOCK_STATE_ID, BlockPos, BlockStateId, ChunkSnapshot, Vec3d};
    use mclone_protocol::{ChunkView, ClientCommand, EntityKind, EntitySnapshot, ServerUpdate};
    use mclone_worldgen::block::{AIR, generated_block_state_id};

    use super::*;
    use crate::LocalRealmSession;

    static TEST_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn fixture_records_are_lit_persistent_chunks_with_void_padding() {
        for kind in [
            AuthoredWorldFixtureKind::Table,
            AuthoredWorldFixtureKind::Island,
            AuthoredWorldFixtureKind::MallardWetland,
            AuthoredWorldFixtureKind::DeerForestEdge,
            AuthoredWorldFixtureKind::BeeFloweringMeadow,
            AuthoredWorldFixtureKind::LobbyTableV2,
            AuthoredWorldFixtureKind::LobbyIslandV2,
        ] {
            let (manifest, records, entity_records) = authored_world_fixture_records(kind).unwrap();
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
            if matches!(
                kind,
                AuthoredWorldFixtureKind::Island | AuthoredWorldFixtureKind::LobbyIslandV2
            ) {
                for record in records
                    .iter()
                    .filter(|record| record.pos() != AUTHORED_WORLD_FIXTURE_CENTER)
                {
                    assert!(snapshot_is_all_air(&record.snapshot));
                }
            } else {
                assert!(
                    records
                        .iter()
                        .all(|record| !snapshot_is_all_air(&record.snapshot))
                );
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
            assert_eq!(
                entity_records.len(),
                usize::from(kind.has_authored_passive_entities())
            );
        }
    }

    #[test]
    fn table_fixture_is_player_scale_and_preview_clears_its_top() {
        let (manifest, records, _) =
            authored_world_fixture_records(AuthoredWorldFixtureKind::Table).unwrap();
        let center = records
            .iter()
            .find(|record| record.pos() == AUTHORED_WORLD_FIXTURE_CENTER)
            .unwrap();
        let bricks = generated_block_state_id(BRICKS);
        let mut brick_positions = Vec::new();
        for y in 64..=68 {
            for z in 0..16 {
                for x in 0..16 {
                    if snapshot_block_state(&center.snapshot, BlockPos::new(x, y, z)) == bricks {
                        brick_positions.push([x, y, z]);
                    }
                }
            }
        }

        assert_eq!(
            brick_positions,
            vec![[7, 64, 7], [8, 64, 7], [7, 64, 8], [8, 64, 8]]
        );
        assert_eq!(manifest.preview_anchor, [8.0, 65.03125, 8.0]);
        assert!(manifest.preview_anchor[1] > 65.0);
        assert!(manifest.preview_anchor[1] < 65.1);

        let water = generated_block_state_id(WATER);
        let water_positions = (0..16)
            .flat_map(|z| (0..16).map(move |x| [x, 64, z]))
            .filter(|[x, y, z]| {
                snapshot_block_state(&center.snapshot, BlockPos::new(*x, *y, *z)) == water
            })
            .collect::<Vec<_>>();
        assert!(water_positions.is_empty());

        let front = records
            .iter()
            .find(|record| record.pos() == ChunkPos::new(0, -1))
            .unwrap();
        let back = records
            .iter()
            .find(|record| record.pos() == ChunkPos::new(0, 1))
            .unwrap();
        assert_eq!(
            snapshot_block_state(&front.snapshot, BlockPos::new(7, 64, -1)),
            water
        );
        assert_eq!(
            snapshot_block_state(&back.snapshot, BlockPos::new(7, 64, 16)),
            water
        );
    }

    #[test]
    fn island_fixture_contains_persisted_ocean_and_dry_mutation_block() {
        let (manifest, records, _) =
            authored_world_fixture_records(AuthoredWorldFixtureKind::Island).unwrap();
        let center = records
            .iter()
            .find(|record| record.pos() == AUTHORED_WORLD_FIXTURE_CENTER)
            .unwrap();
        let water = generated_block_state_id(WATER);
        let grass = generated_block_state_id(GRASS_BLOCK);
        let bricks = generated_block_state_id(BRICKS);

        assert_eq!(
            snapshot_block_state(&center.snapshot, BlockPos::new(0, 64, 0)),
            water
        );
        assert_eq!(
            snapshot_block_state(&center.snapshot, BlockPos::new(8, 65, 8)),
            grass
        );
        let mutation = manifest.mutation_block;
        assert_eq!(
            snapshot_block_state(
                &center.snapshot,
                BlockPos::new(mutation[0], mutation[1], mutation[2])
            ),
            grass
        );
        assert_eq!(
            snapshot_block_state(&center.snapshot, BlockPos::new(3, 66, 7)),
            bricks
        );
        assert_eq!(manifest.kind.preview_display_anchor(), [4.0, 67.03125, 8.0]);
    }

    #[test]
    fn lobby_v2_island_authors_stable_passive_entity_records() {
        let (_, _, entity_records) =
            authored_world_fixture_records(AuthoredWorldFixtureKind::LobbyIslandV2).unwrap();

        assert_eq!(entity_records.len(), 1);
        let record = &entity_records[0];
        assert_eq!(record.pos, AUTHORED_WORLD_FIXTURE_CENTER);
        assert_eq!(record.entities.len(), 2);
        assert_eq!(
            record.entities[0].persistent_id,
            AUTHORED_LOBBY_COW_PERSISTENT_ID
        );
        assert_eq!(record.entities[0].kind, "minecraft:cow");
        assert_eq!(record.entities[0].position, Vec3d::new(6.5, 66.0, 8.5));
        assert_eq!(record.entities[0].payload, EntitySavePayload::Cow);
        assert_eq!(
            record.entities[1].persistent_id,
            AUTHORED_LOBBY_CHICKEN_PERSISTENT_ID
        );
        assert_eq!(record.entities[1].kind, "minecraft:chicken");
        assert_eq!(record.entities[1].position, Vec3d::new(10.5, 66.0, 8.5));
        assert_eq!(
            record.entities[1].payload,
            EntitySavePayload::Chicken { egg_time: 6_000 }
        );
    }

    #[test]
    fn memory_bootstrap_is_exact_and_fresh_for_every_session() {
        let kind = AuthoredWorldFixtureKind::LobbyTableV2;
        let (expected_manifest, expected_chunks, expected_entities) =
            authored_world_fixture_records(kind).unwrap();
        let (first_manifest, mut first) = authored_world_fixture_memory_store(kind).unwrap();
        let (second_manifest, mut second) = authored_world_fixture_memory_store(kind).unwrap();

        assert_eq!(first_manifest, expected_manifest);
        assert_eq!(second_manifest, expected_manifest);
        for expected in &expected_chunks {
            assert_eq!(
                first
                    .load_chunk(&mclone_protocol::DimensionKey::overworld(), expected.pos())
                    .unwrap()
                    .as_ref(),
                Some(expected)
            );
            assert_eq!(
                second
                    .load_chunk(&mclone_protocol::DimensionKey::overworld(), expected.pos())
                    .unwrap()
                    .as_ref(),
                Some(expected)
            );
        }
        assert!(expected_entities.is_empty());

        let (_, island_chunks, _) =
            authored_world_fixture_records(AuthoredWorldFixtureKind::LobbyIslandV2).unwrap();
        let replacement = island_chunks
            .into_iter()
            .find(|record| record.pos() == AUTHORED_WORLD_FIXTURE_CENTER)
            .unwrap();
        first
            .save_chunk(&mclone_protocol::DimensionKey::overworld(), &replacement)
            .unwrap();

        let expected_center = expected_chunks
            .iter()
            .find(|record| record.pos() == AUTHORED_WORLD_FIXTURE_CENTER)
            .unwrap();
        assert_ne!(
            first
                .load_chunk(
                    &mclone_protocol::DimensionKey::overworld(),
                    AUTHORED_WORLD_FIXTURE_CENTER,
                )
                .unwrap()
                .as_ref(),
            Some(expected_center)
        );
        assert_eq!(
            second
                .load_chunk(
                    &mclone_protocol::DimensionKey::overworld(),
                    AUTHORED_WORLD_FIXTURE_CENTER,
                )
                .unwrap()
                .as_ref(),
            Some(expected_center)
        );
    }

    #[test]
    fn memory_bootstrap_preserves_authored_entity_records() {
        let kind = AuthoredWorldFixtureKind::LobbyIslandV2;
        let (_, _, expected_entities) = authored_world_fixture_records(kind).unwrap();
        let (_, mut store) = authored_world_fixture_memory_store(kind).unwrap();

        assert_eq!(expected_entities.len(), 1);
        assert_eq!(
            store
                .load_entity_chunk(
                    &mclone_protocol::DimensionKey::overworld(),
                    AUTHORED_WORLD_FIXTURE_CENTER,
                )
                .unwrap()
                .as_ref(),
            expected_entities.first()
        );
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
    fn memory_bootstrap_starts_a_normal_realm_without_worldgen() {
        let (manifest, store) =
            authored_world_fixture_memory_store(AuthoredWorldFixtureKind::LobbyTableV2).unwrap();
        let mut definition =
            crate::DimensionDefinition::overworld(manifest.seed, manifest.world_generation_profile);
        definition.topology = mclone_core::HorizontalTopology::UNBOUNDED;
        let mut server =
            LocalRealmSession::local_integrated_with_world_store_and_dimension_definition(
                definition,
                Box::new(store),
            );
        server.set_world_behavior_profile(crate::WorldBehaviorProfile::ProtectedLobby);
        server.initialize_world_metadata_blocking().unwrap();

        let updates = load_view_until_idle(&mut server, AUTHORED_WORLD_FIXTURE_CENTER);
        let expected = manifest.expected_spawn;
        assert!(updates.iter().any(|update| matches!(
            update,
            ServerUpdate::PlayerPosition(position)
                if position.position == Vec3d::new(expected[0], expected[1], expected[2])
        )));
        assert_eq!(server.scheduler().job_count(), 0);
        assert_eq!(server.scheduler().worldgen_mailbox_pending_count(), 0);
        server.shutdown_persistence().unwrap();
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
                    LocalRealmSession::try_with_threaded_sqlite_world_dir(manifest.seed, &root)
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
                    LocalRealmSession::try_with_threaded_sqlite_world_dir(manifest.seed, &root)
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
    fn lobby_v2_passive_entities_move_and_reload_without_duplication() {
        let root = unique_test_root("lobby-v2-entities");
        let manifest =
            write_authored_world_fixture_dir(&root, AuthoredWorldFixtureKind::LobbyIslandV2)
                .unwrap();
        let first_session_entities = {
            let mut server =
                LocalRealmSession::try_with_threaded_sqlite_world_dir(manifest.seed, &root)
                    .unwrap();
            server
                .set_world_generation_profile(manifest.world_generation_profile)
                .unwrap();
            server.set_debug_passive_showcase_enabled(false);
            let updates = load_view_until_idle(&mut server, AUTHORED_WORLD_FIXTURE_CENTER);
            let mut entities = passive_snapshots(&updates);
            assert_eq!(
                entities
                    .iter()
                    .map(|entity| entity.kind)
                    .collect::<Vec<_>>(),
                vec![EntityKind::Cow, EntityKind::Chicken]
            );
            let initial_positions = entities
                .iter()
                .map(|entity| (entity.id, entity.position))
                .collect::<BTreeMap<_, _>>();

            let mut moved = false;
            for _ in 0..2_000 {
                let report = server.try_simulation_tick_report().unwrap();
                for update in report.updates {
                    if let ServerUpdate::EntityUpdate(update) = update
                        && let Some(entity) =
                            entities.iter_mut().find(|entity| entity.id == update.id)
                    {
                        entity.position = update.position;
                        entity.tick_count = update.tick_count;
                        moved |= initial_positions[&update.id] != update.position;
                    }
                }
                if moved {
                    break;
                }
            }
            assert!(
                moved,
                "an authored passive actor should move through ordinary AI"
            );
            assert!(entities.iter().all(|entity| entity.tick_count > 0));
            server.shutdown_persistence().unwrap();
            entities
        };

        let first_saved_record = {
            let mut store = SqliteWorldStore::open_world_dir_read_only(&root).unwrap();
            store
                .load_entity_chunk(
                    &mclone_protocol::DimensionKey::overworld(),
                    AUTHORED_WORLD_FIXTURE_CENTER,
                )
                .unwrap()
                .expect("saved authored entity chunk")
        };
        assert_eq!(first_saved_record.entities.len(), 2);
        let saved_by_persistent_id = first_saved_record
            .entities
            .iter()
            .map(|entity| (entity.persistent_id, entity))
            .collect::<BTreeMap<_, _>>();
        for entity in &first_session_entities {
            let saved = saved_by_persistent_id
                .get(&entity.persistent_id)
                .expect("runtime actor must save under its durable identity");
            assert_eq!(saved.position, entity.position);
            assert_eq!(
                saved.kind,
                match entity.kind {
                    EntityKind::Cow => "minecraft:cow",
                    EntityKind::Chicken => "minecraft:chicken",
                    _ => unreachable!("passive fixture contains only cow and chicken"),
                }
            );
        }
        let saved_chicken_egg_time = first_saved_record
            .entities
            .iter()
            .find_map(|entity| match entity.payload {
                EntitySavePayload::Chicken { egg_time } => Some(egg_time),
                _ => None,
            })
            .expect("saved authored chicken timer");
        assert!(
            (1..6_000).contains(&saved_chicken_egg_time),
            "ordinary simulation must persist the chicken-owned egg timer"
        );

        let mut server =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(manifest.seed, &root).unwrap();
        server
            .set_world_generation_profile(manifest.world_generation_profile)
            .unwrap();
        server.set_debug_passive_showcase_enabled(false);
        let updates = load_view_until_idle(&mut server, AUTHORED_WORLD_FIXTURE_CENTER);
        let reloaded = passive_snapshots(&updates);
        assert_eq!(
            reloaded.len(),
            2,
            "reload must not duplicate authored actors"
        );
        assert!(
            reloaded.iter().all(|entity| entity.tick_count == 0),
            "base entity tickCount is reconstructed runtime state"
        );
        for entity in &reloaded {
            let saved = saved_by_persistent_id
                .get(&entity.persistent_id)
                .expect("reloaded actor must keep its durable identity");
            assert_eq!(
                entity.position, saved.position,
                "relaunch must restore the saved pose instead of reauthoring"
            );
        }
        server.shutdown_persistence().unwrap();

        let second_saved_record = {
            let mut store = SqliteWorldStore::open_world_dir_read_only(&root).unwrap();
            store
                .load_entity_chunk(
                    &mclone_protocol::DimensionKey::overworld(),
                    AUTHORED_WORLD_FIXTURE_CENTER,
                )
                .unwrap()
                .expect("resaved authored entity chunk")
        };
        let reloaded_chicken_egg_time = second_saved_record
            .entities
            .iter()
            .find_map(|entity| match entity.payload {
                EntitySavePayload::Chicken { egg_time } => Some(egg_time),
                _ => None,
            })
            .expect("reloaded authored chicken timer");
        assert_eq!(
            reloaded_chicken_egg_time, saved_chicken_egg_time,
            "closed-world time must not catch up the chicken-owned timer"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn authored_fixture_missing_chunk_is_lit_void_after_stored_world_load() {
        let root = unique_test_root("missing-void");
        let manifest =
            write_authored_world_fixture_dir(&root, AuthoredWorldFixtureKind::Table).unwrap();
        let mut server =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(manifest.seed, &root).unwrap();
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
            LocalRealmSession::try_with_threaded_sqlite_world_dir(manifest.seed, &root).unwrap();
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

    fn load_view_until_idle(server: &mut LocalRealmSession, center: ChunkPos) -> Vec<ServerUpdate> {
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

    fn passive_snapshots(updates: &[ServerUpdate]) -> Vec<EntitySnapshot> {
        let mut entities = updates
            .iter()
            .filter_map(|update| match update {
                ServerUpdate::EntitySnapshot(snapshot)
                    if matches!(snapshot.kind, EntityKind::Cow | EntityKind::Chicken) =>
                {
                    Some(*snapshot)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        entities.sort_by_key(|entity| match entity.kind {
            EntityKind::Cow => 0,
            EntityKind::Chicken => 1,
            _ => 2,
        });
        entities
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
