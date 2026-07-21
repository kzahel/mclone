use std::collections::BTreeMap;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use mclone_core::{BlockPos, ChunkPos, ChunkRevision, ChunkStatus};
use mclone_worldgen::block::{
    AIR, ALLIUM, BRICKS, COARSE_DIRT, COBBLESTONE, CORNFLOWER, DANDELION, DIRT, GRASS_BLOCK,
    GRAVEL, HAY_BLOCK, OAK_LOG, OAK_LOG_X, OAK_LOG_Z, OAK_PLANKS, POPPY, RED_TERRACOTTA,
    SPRUCE_LOG, SPRUCE_LOG_X, SPRUCE_LOG_Z, SPRUCE_PLANKS, STONE, STONE_BRICKS, TORCH,
    WALL_TORCH_SOUTH, WHITE_TERRACOTTA,
};
use mclone_worldgen::levelgen::{GeneratedChunk, MutableChunkBlockBuffer};
use mclone_worldgen::structure_template::{
    PlacedStructureTemplate, StructureMaterialTheme, StructurePlaceSettings, StructureTemplate,
    StructureTemplateBuilder, TemplateBlockState, TemplateError, TemplateMaterialRole,
    TemplateMirror, TemplateRotation,
};
use serde::{Deserialize, Serialize};

use crate::light_status::{PendingLightStatus, PendingLightStatusBatch};
use crate::light_world::RetainedInitialLightState;
#[cfg(not(target_arch = "wasm32"))]
use crate::persistence::SqliteWorldStore;
use crate::persistence::{ChunkRecord, MemoryWorldStore, WorldStore};
use crate::{
    AUTHORED_WORLD_HEIGHT, AUTHORED_WORLD_MIN_Y, ChunkStoreError, ChunkStoreResult,
    WorldGenerationProfile,
};

pub const BUILDING_LAB_GALLERY_ID: &str = "standalone-building-lab-v1";
pub const BUILDING_LAB_MARKER_FILE: &str = "mclone-building-lab.json";
pub const BUILDING_LAB_SCHEMA_VERSION: u32 = 1;
pub const BUILDING_LAB_SEED: i64 = 20_801;
pub const BUILDING_LAB_VOID_PADDING_RADIUS: i32 = 3;

const COTTAGE_ORIGIN: BlockPos = BlockPos::new(-29, 64, -10);
const BARN_ORIGIN: BlockPos = BlockPos::new(7, 64, -12);
const BARN_LEAN_TO_ORIGIN: BlockPos = BlockPos::new(28, 64, -7);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildingLabMarkerReceipt {
    pub kind: String,
    pub pos: [i32; 3],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildingLabTemplateReceipt {
    pub template_id: String,
    pub theme_id: String,
    pub bounds_min: [i32; 3],
    pub bounds_max_exclusive: [i32; 3],
    pub touched_chunks: Vec<[i32; 2]>,
    pub block_count: usize,
    pub markers: Vec<BuildingLabMarkerReceipt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildingLabManifest {
    pub schema_version: u32,
    pub gallery_id: String,
    pub seed: i64,
    pub world_generation_profile: WorldGenerationProfile,
    pub void_padding_radius: i32,
    pub expected_spawn: [f64; 3],
    pub placements: Vec<BuildingLabTemplateReceipt>,
}

pub fn building_lab_records() -> ChunkStoreResult<(BuildingLabManifest, Vec<ChunkRecord>)> {
    let cottage = cottage_template().map_err(template_error)?;
    let barn = barn_core_template().map_err(template_error)?;
    let lean_to = barn_lean_to_template().map_err(template_error)?;
    let cottage_theme = cottage_theme();
    let barn_theme = barn_theme();

    let placements = vec![
        cottage
            .place(&StructurePlaceSettings {
                origin: COTTAGE_ORIGIN,
                rotation: TemplateRotation::None,
                mirror: TemplateMirror::None,
                theme: &cottage_theme,
            })
            .map_err(template_error)?,
        barn.place(&StructurePlaceSettings {
            origin: BARN_ORIGIN,
            rotation: TemplateRotation::None,
            mirror: TemplateMirror::None,
            theme: &barn_theme,
        })
        .map_err(template_error)?,
        lean_to
            .place(&StructurePlaceSettings {
                origin: BARN_LEAN_TO_ORIGIN,
                rotation: TemplateRotation::None,
                mirror: TemplateMirror::None,
                theme: &barn_theme,
            })
            .map_err(template_error)?,
    ];

    let mut chunks = BTreeMap::new();
    for chunk_z in -BUILDING_LAB_VOID_PADDING_RADIUS..=BUILDING_LAB_VOID_PADDING_RADIUS {
        for chunk_x in -BUILDING_LAB_VOID_PADDING_RADIUS..=BUILDING_LAB_VOID_PADDING_RADIUS {
            let mut buffer = MutableChunkBlockBuffer::new(
                chunk_x,
                chunk_z,
                AUTHORED_WORLD_MIN_Y,
                AUTHORED_WORLD_HEIGHT,
            );
            author_flat_grass_chunk(&mut buffer);
            chunks.insert(ChunkPos::new(chunk_x, chunk_z), buffer);
        }
    }

    author_gallery_landscape(&mut chunks)?;
    for placement in &placements {
        stamp_placement(&mut chunks, placement)?;
    }

    let generated = chunks
        .into_iter()
        .map(|(pos, buffer)| (pos, GeneratedChunk::from_mutable_buffer(buffer)))
        .collect::<BTreeMap<_, _>>();
    let records = light_generated_chunks(&generated);
    let manifest = BuildingLabManifest {
        schema_version: BUILDING_LAB_SCHEMA_VERSION,
        gallery_id: BUILDING_LAB_GALLERY_ID.to_owned(),
        seed: BUILDING_LAB_SEED,
        world_generation_profile: WorldGenerationProfile::authored_only(),
        void_padding_radius: BUILDING_LAB_VOID_PADDING_RADIUS,
        expected_spawn: [3.5, 64.0, 40.5],
        placements: placements.iter().map(template_receipt).collect(),
    };
    Ok((manifest, records))
}

pub fn write_building_lab_to_store(
    store: &mut dyn WorldStore,
) -> ChunkStoreResult<BuildingLabManifest> {
    let (manifest, records) = building_lab_records()?;
    for record in &records {
        store.save_chunk(&mclone_protocol::DimensionKey::overworld(), record)?;
    }
    store.flush()?;
    Ok(manifest)
}

pub fn building_lab_memory_store() -> ChunkStoreResult<(BuildingLabManifest, MemoryWorldStore)> {
    let mut store = MemoryWorldStore::new();
    let manifest = write_building_lab_to_store(&mut store)?;
    Ok((manifest, store))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn write_building_lab_dir(root: impl AsRef<Path>) -> ChunkStoreResult<BuildingLabManifest> {
    let root = root.as_ref();
    validate_lab_root_for_rebuild(root)?;
    fs::create_dir_all(root)?;
    remove_existing_lab_database(root)?;

    let mut store = SqliteWorldStore::open_world_dir(root)?;
    let manifest = write_building_lab_to_store(&mut store)?;
    store.close()?;
    let marker = serde_json::to_vec_pretty(&manifest).map_err(|error| {
        ChunkStoreError::InvalidData(format!("failed to encode building lab marker: {error}"))
    })?;
    fs::write(building_lab_marker_path(root), marker)?;
    Ok(manifest)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn building_lab_marker_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join(BUILDING_LAB_MARKER_FILE)
}

pub fn cottage_template() -> Result<StructureTemplate, TemplateError> {
    let mut builder = StructureTemplateBuilder::new("farmstead-cottage-a-v1", [15, 12, 17])?;
    let foundation = role(TemplateMaterialRole::Foundation);
    let wall = role(TemplateMaterialRole::Wall);
    let timber_y = role(TemplateMaterialRole::TimberY);
    let timber_x = role(TemplateMaterialRole::TimberX);
    let timber_z = role(TemplateMaterialRole::TimberZ);
    let roof = role(TemplateMaterialRole::Roof);
    let floor = role(TemplateMaterialRole::Floor);
    let accent = role(TemplateMaterialRole::Accent);

    builder.fill_box(BlockPos::new(1, 0, 2), BlockPos::new(14, 1, 13), foundation)?;
    builder.fill_box(BlockPos::new(1, 1, 2), BlockPos::new(14, 6, 13), wall)?;
    builder.fill_box(
        BlockPos::new(2, 1, 3),
        BlockPos::new(13, 6, 12),
        TemplateBlockState::Exact(AIR),
    )?;
    builder.fill_box(BlockPos::new(2, 1, 3), BlockPos::new(13, 2, 12), floor)?;

    for &(x, z) in &[(1, 2), (13, 2), (1, 12), (13, 12)] {
        column(&mut builder, x, z, 1, 6, timber_y)?;
    }
    for y in [1, 5] {
        line_x(&mut builder, 1, 14, y, 2, timber_x)?;
        line_x(&mut builder, 1, 14, y, 12, timber_x)?;
        line_z(&mut builder, 1, 2, 13, y, timber_z)?;
        line_z(&mut builder, 13, 2, 13, y, timber_z)?;
    }

    // South facade: a central two-block entrance and two framed windows.
    builder.fill_box(
        BlockPos::new(7, 2, 12),
        BlockPos::new(9, 5, 13),
        TemplateBlockState::Exact(AIR),
    )?;
    framed_window_z(&mut builder, 3, 5, 12)?;
    framed_window_z(&mut builder, 10, 12, 12)?;
    column(&mut builder, 6, 12, 1, 6, timber_y)?;
    column(&mut builder, 9, 12, 1, 6, timber_y)?;
    line_x(&mut builder, 6, 10, 5, 12, timber_x)?;

    // Side windows keep the otherwise solid full-cube prototype readable.
    framed_window_x(&mut builder, 1, 5, 7)?;
    framed_window_x(&mut builder, 13, 5, 7)?;

    // Filled gables sit inside a stepped, full-cube roof silhouette.
    builder.fill_box(BlockPos::new(2, 6, 2), BlockPos::new(13, 7, 3), wall)?;
    builder.fill_box(BlockPos::new(4, 7, 2), BlockPos::new(11, 8, 3), wall)?;
    builder.fill_box(BlockPos::new(6, 8, 2), BlockPos::new(9, 9, 3), wall)?;
    builder.fill_box(BlockPos::new(2, 6, 12), BlockPos::new(13, 7, 13), wall)?;
    builder.fill_box(BlockPos::new(4, 7, 12), BlockPos::new(11, 8, 13), wall)?;
    builder.fill_box(BlockPos::new(6, 8, 12), BlockPos::new(9, 9, 13), wall)?;
    column(&mut builder, 7, 2, 6, 9, timber_y)?;
    column(&mut builder, 7, 12, 6, 9, timber_y)?;
    line_x(&mut builder, 2, 13, 6, 2, timber_x)?;
    line_x(&mut builder, 2, 13, 6, 12, timber_x)?;

    roof_band(&mut builder, 0, 3, 6, 1, 14, roof)?;
    roof_band(&mut builder, 12, 15, 6, 1, 14, roof)?;
    roof_band(&mut builder, 2, 5, 7, 1, 14, roof)?;
    roof_band(&mut builder, 10, 13, 7, 1, 14, roof)?;
    roof_band(&mut builder, 4, 7, 8, 1, 14, roof)?;
    roof_band(&mut builder, 8, 11, 8, 1, 14, roof)?;
    roof_band(&mut builder, 6, 9, 9, 1, 14, roof)?;

    // A shallow porch is part of the authored cottage rather than a random
    // facade mutation. Its marker leaves room for a future path socket.
    builder.fill_box(
        BlockPos::new(5, 0, 13),
        BlockPos::new(11, 1, 16),
        foundation,
    )?;
    builder.fill_box(BlockPos::new(5, 1, 13), BlockPos::new(11, 2, 16), floor)?;
    column(&mut builder, 5, 15, 2, 6, timber_y)?;
    column(&mut builder, 10, 15, 2, 6, timber_y)?;
    builder.fill_box(BlockPos::new(4, 5, 12), BlockPos::new(6, 6, 16), roof)?;
    builder.fill_box(BlockPos::new(10, 5, 12), BlockPos::new(12, 6, 16), roof)?;
    builder.fill_box(BlockPos::new(6, 6, 12), BlockPos::new(10, 7, 16), roof)?;
    builder.set(
        BlockPos::new(6, 3, 13),
        TemplateBlockState::Exact(WALL_TORCH_SOUTH),
    )?;
    builder.set(
        BlockPos::new(9, 3, 13),
        TemplateBlockState::Exact(WALL_TORCH_SOUTH),
    )?;

    // The chimney is deliberately off-center and rises clear of the roof.
    builder.fill_box(BlockPos::new(10, 5, 5), BlockPos::new(12, 11, 7), accent)?;
    builder.set(BlockPos::new(10, 11, 5), accent)?;
    builder.set(BlockPos::new(11, 11, 6), accent)?;

    builder.marker(BlockPos::new(8, 2, 16), "entrance:south")?;
    builder.marker(BlockPos::new(1, 2, 7), "attachment:west-yard")?;
    Ok(builder.build())
}

pub fn barn_core_template() -> Result<StructureTemplate, TemplateError> {
    let mut builder = StructureTemplateBuilder::new("farmstead-barn-core-a-v1", [21, 14, 18])?;
    let foundation = role(TemplateMaterialRole::Foundation);
    let wall = role(TemplateMaterialRole::Wall);
    let timber_y = role(TemplateMaterialRole::TimberY);
    let timber_x = role(TemplateMaterialRole::TimberX);
    let timber_z = role(TemplateMaterialRole::TimberZ);
    let roof = role(TemplateMaterialRole::Roof);
    let floor = role(TemplateMaterialRole::Floor);
    let accent = role(TemplateMaterialRole::Accent);

    builder.fill_box(BlockPos::new(1, 0, 2), BlockPos::new(20, 1, 16), foundation)?;
    builder.fill_box(BlockPos::new(1, 1, 2), BlockPos::new(20, 8, 16), wall)?;
    builder.fill_box(
        BlockPos::new(2, 1, 3),
        BlockPos::new(19, 8, 15),
        TemplateBlockState::Exact(AIR),
    )?;
    builder.fill_box(BlockPos::new(2, 1, 3), BlockPos::new(19, 2, 15), floor)?;

    for &(x, z) in &[(1, 2), (19, 2), (1, 15), (19, 15)] {
        column(&mut builder, x, z, 1, 8, timber_y)?;
    }
    for x in [6, 14] {
        column(&mut builder, x, 2, 1, 8, timber_y)?;
        column(&mut builder, x, 15, 1, 8, timber_y)?;
    }
    for z in [7, 11] {
        column(&mut builder, 1, z, 1, 8, timber_y)?;
        column(&mut builder, 19, z, 1, 8, timber_y)?;
    }
    for y in [1, 5, 7] {
        line_x(&mut builder, 1, 20, y, 2, timber_x)?;
        line_x(&mut builder, 1, 20, y, 15, timber_x)?;
        line_z(&mut builder, 1, 2, 16, y, timber_z)?;
        line_z(&mut builder, 19, 2, 16, y, timber_z)?;
    }

    // The open central bay makes the barn legible at arrival distance.
    builder.fill_box(
        BlockPos::new(7, 2, 15),
        BlockPos::new(14, 7, 16),
        TemplateBlockState::Exact(AIR),
    )?;
    column(&mut builder, 6, 15, 1, 8, timber_y)?;
    column(&mut builder, 14, 15, 1, 8, timber_y)?;
    line_x(&mut builder, 6, 15, 7, 15, timber_x)?;
    framed_window_z(&mut builder, 3, 5, 15)?;
    framed_window_z(&mut builder, 16, 18, 15)?;
    framed_window_x(&mut builder, 1, 5, 7)?;

    builder.fill_box(BlockPos::new(3, 8, 2), BlockPos::new(18, 9, 3), wall)?;
    builder.fill_box(BlockPos::new(6, 9, 2), BlockPos::new(15, 10, 3), wall)?;
    builder.fill_box(BlockPos::new(9, 10, 2), BlockPos::new(12, 11, 3), wall)?;
    builder.fill_box(BlockPos::new(3, 8, 15), BlockPos::new(18, 9, 16), wall)?;
    builder.fill_box(BlockPos::new(6, 9, 15), BlockPos::new(15, 10, 16), wall)?;
    builder.fill_box(BlockPos::new(9, 10, 15), BlockPos::new(12, 11, 16), wall)?;
    builder.fill_box(
        BlockPos::new(9, 8, 15),
        BlockPos::new(12, 10, 16),
        TemplateBlockState::Exact(AIR),
    )?;
    column(&mut builder, 10, 2, 8, 11, timber_y)?;
    column(&mut builder, 10, 15, 10, 11, timber_y)?;
    line_x(&mut builder, 3, 18, 8, 2, timber_x)?;
    line_x(&mut builder, 3, 18, 8, 15, timber_x)?;

    roof_band(&mut builder, 0, 3, 8, 1, 17, roof)?;
    roof_band(&mut builder, 18, 21, 8, 1, 17, roof)?;
    roof_band(&mut builder, 3, 6, 9, 1, 17, roof)?;
    roof_band(&mut builder, 15, 18, 9, 1, 17, roof)?;
    roof_band(&mut builder, 6, 9, 10, 1, 17, roof)?;
    roof_band(&mut builder, 12, 15, 10, 1, 17, roof)?;
    roof_band(&mut builder, 9, 12, 11, 1, 17, roof)?;

    // Hay and interior posts are fixed scene dressing, visible through the
    // broad entrance but still replaceable by later marker processors.
    builder.fill_box(BlockPos::new(3, 2, 5), BlockPos::new(6, 4, 8), accent)?;
    builder.fill_box(BlockPos::new(15, 2, 7), BlockPos::new(18, 3, 11), accent)?;
    column(&mut builder, 7, 8, 2, 8, timber_y)?;
    column(&mut builder, 13, 8, 2, 8, timber_y)?;
    builder.set(
        BlockPos::new(8, 4, 15),
        TemplateBlockState::Exact(WALL_TORCH_SOUTH),
    )?;
    builder.set(
        BlockPos::new(13, 4, 15),
        TemplateBlockState::Exact(WALL_TORCH_SOUTH),
    )?;

    builder.marker(BlockPos::new(10, 2, 17), "entrance:south")?;
    builder.marker(BlockPos::new(20, 2, 8), "attachment:east-lean-to")?;
    builder.marker(BlockPos::new(10, 8, 15), "loft:front")?;
    Ok(builder.build())
}

pub fn barn_lean_to_template() -> Result<StructureTemplate, TemplateError> {
    let mut builder = StructureTemplateBuilder::new("farmstead-barn-lean-to-a-v1", [7, 8, 13])?;
    let foundation = role(TemplateMaterialRole::Foundation);
    let timber_y = role(TemplateMaterialRole::TimberY);
    let timber_z = role(TemplateMaterialRole::TimberZ);
    let roof = role(TemplateMaterialRole::Roof);
    let floor = role(TemplateMaterialRole::Floor);
    let accent = role(TemplateMaterialRole::Accent);

    builder.fill_box(BlockPos::new(0, 0, 0), BlockPos::new(7, 1, 13), foundation)?;
    builder.fill_box(BlockPos::new(0, 1, 0), BlockPos::new(7, 2, 13), floor)?;
    for z in [0, 6, 12] {
        column(&mut builder, 0, z, 2, 7, timber_y)?;
        column(&mut builder, 6, z, 2, 5, timber_y)?;
    }
    line_z(&mut builder, 6, 0, 13, 4, timber_z)?;
    builder.fill_box(BlockPos::new(0, 6, 0), BlockPos::new(3, 7, 13), roof)?;
    builder.fill_box(BlockPos::new(2, 5, 0), BlockPos::new(5, 6, 13), roof)?;
    builder.fill_box(BlockPos::new(4, 4, 0), BlockPos::new(7, 5, 13), roof)?;
    builder.fill_box(BlockPos::new(4, 2, 2), BlockPos::new(7, 4, 5), accent)?;
    builder.fill_box(BlockPos::new(2, 2, 8), BlockPos::new(6, 3, 11), accent)?;
    builder.marker(BlockPos::new(0, 2, 6), "attachment:west-barn")?;
    builder.marker(BlockPos::new(6, 2, 12), "yard:south")?;
    Ok(builder.build())
}

fn cottage_theme() -> StructureMaterialTheme {
    StructureMaterialTheme::new("warm-oak-and-plaster-v1")
        .with(TemplateMaterialRole::Foundation, COBBLESTONE)
        .with(TemplateMaterialRole::Wall, WHITE_TERRACOTTA)
        .with(TemplateMaterialRole::TimberY, OAK_LOG)
        .with(TemplateMaterialRole::TimberX, OAK_LOG_X)
        .with(TemplateMaterialRole::TimberZ, OAK_LOG_Z)
        .with(TemplateMaterialRole::Roof, SPRUCE_PLANKS)
        .with(TemplateMaterialRole::Floor, OAK_PLANKS)
        .with(TemplateMaterialRole::Accent, BRICKS)
}

fn barn_theme() -> StructureMaterialTheme {
    StructureMaterialTheme::new("red-spruce-working-barn-v1")
        .with(TemplateMaterialRole::Foundation, STONE_BRICKS)
        .with(TemplateMaterialRole::Wall, RED_TERRACOTTA)
        .with(TemplateMaterialRole::TimberY, SPRUCE_LOG)
        .with(TemplateMaterialRole::TimberX, SPRUCE_LOG_X)
        .with(TemplateMaterialRole::TimberZ, SPRUCE_LOG_Z)
        .with(TemplateMaterialRole::Roof, SPRUCE_PLANKS)
        .with(TemplateMaterialRole::Floor, OAK_PLANKS)
        .with(TemplateMaterialRole::Accent, HAY_BLOCK)
}

fn role(role: TemplateMaterialRole) -> TemplateBlockState {
    TemplateBlockState::Role(role)
}

fn column(
    builder: &mut StructureTemplateBuilder,
    x: i32,
    z: i32,
    min_y: i32,
    max_y_exclusive: i32,
    state: TemplateBlockState,
) -> Result<(), TemplateError> {
    builder.fill_box(
        BlockPos::new(x, min_y, z),
        BlockPos::new(x + 1, max_y_exclusive, z + 1),
        state,
    )?;
    Ok(())
}

fn line_x(
    builder: &mut StructureTemplateBuilder,
    min_x: i32,
    max_x_exclusive: i32,
    y: i32,
    z: i32,
    state: TemplateBlockState,
) -> Result<(), TemplateError> {
    builder.fill_box(
        BlockPos::new(min_x, y, z),
        BlockPos::new(max_x_exclusive, y + 1, z + 1),
        state,
    )?;
    Ok(())
}

fn line_z(
    builder: &mut StructureTemplateBuilder,
    x: i32,
    min_z: i32,
    max_z_exclusive: i32,
    y: i32,
    state: TemplateBlockState,
) -> Result<(), TemplateError> {
    builder.fill_box(
        BlockPos::new(x, y, min_z),
        BlockPos::new(x + 1, y + 1, max_z_exclusive),
        state,
    )?;
    Ok(())
}

fn roof_band(
    builder: &mut StructureTemplateBuilder,
    min_x: i32,
    max_x_exclusive: i32,
    y: i32,
    min_z: i32,
    max_z_exclusive: i32,
    state: TemplateBlockState,
) -> Result<(), TemplateError> {
    builder.fill_box(
        BlockPos::new(min_x, y, min_z),
        BlockPos::new(max_x_exclusive, y + 1, max_z_exclusive),
        state,
    )?;
    Ok(())
}

fn framed_window_z(
    builder: &mut StructureTemplateBuilder,
    min_x: i32,
    max_x_exclusive: i32,
    z: i32,
) -> Result<(), TemplateError> {
    builder.fill_box(
        BlockPos::new(min_x, 2, z),
        BlockPos::new(max_x_exclusive, 4, z + 1),
        TemplateBlockState::Exact(AIR),
    )?;
    column(
        builder,
        min_x - 1,
        z,
        1,
        5,
        role(TemplateMaterialRole::TimberY),
    )?;
    column(
        builder,
        max_x_exclusive,
        z,
        1,
        5,
        role(TemplateMaterialRole::TimberY),
    )?;
    line_x(
        builder,
        min_x - 1,
        max_x_exclusive + 1,
        4,
        z,
        role(TemplateMaterialRole::TimberX),
    )?;
    Ok(())
}

fn framed_window_x(
    builder: &mut StructureTemplateBuilder,
    x: i32,
    min_z: i32,
    max_z_exclusive: i32,
) -> Result<(), TemplateError> {
    builder.fill_box(
        BlockPos::new(x, 2, min_z),
        BlockPos::new(x + 1, 4, max_z_exclusive),
        TemplateBlockState::Exact(AIR),
    )?;
    column(
        builder,
        x,
        min_z - 1,
        1,
        5,
        role(TemplateMaterialRole::TimberY),
    )?;
    column(
        builder,
        x,
        max_z_exclusive,
        1,
        5,
        role(TemplateMaterialRole::TimberY),
    )?;
    line_z(
        builder,
        x,
        min_z - 1,
        max_z_exclusive + 1,
        4,
        role(TemplateMaterialRole::TimberZ),
    )?;
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

fn author_gallery_landscape(
    chunks: &mut BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
) -> ChunkStoreResult<()> {
    // The straight arrival lane gives the low camera a clear read, then splits
    // into imperfect branches toward each authored entrance.
    for z in 5..=48 {
        for x in 2..=4 {
            set_world_block(chunks, BlockPos::new(x, 63, z), GRAVEL)?;
        }
    }
    for x in -21_i32..=18 {
        for z in 5_i32..=8 {
            let block = if (x + z).rem_euclid(4) == 0 {
                COARSE_DIRT
            } else {
                GRAVEL
            };
            set_world_block(chunks, BlockPos::new(x, 63, z), block)?;
        }
    }
    for x in -22..=-18 {
        for z in 0..=6 {
            set_world_block(chunks, BlockPos::new(x, 63, z), COARSE_DIRT)?;
        }
    }
    for x in 14..=19 {
        for z in 4..=8 {
            set_world_block(chunks, BlockPos::new(x, 63, z), COARSE_DIRT)?;
        }
    }

    for (x, z, block) in [
        (-31, 1, DANDELION),
        (-30, 3, POPPY),
        (-16, 4, ALLIUM),
        (-14, 2, CORNFLOWER),
        (-10, 7, DANDELION),
        (-6, 6, POPPY),
        (34, 8, DANDELION),
        (36, 5, CORNFLOWER),
        (30, 10, ALLIUM),
    ] {
        set_world_block(chunks, BlockPos::new(x, 64, z), block)?;
    }

    // Small work piles keep the gallery from reading as two isolated museum
    // objects while remaining outside the template receipts.
    for x in -6..=-3 {
        set_world_block(chunks, BlockPos::new(x, 64, -1), OAK_LOG_X)?;
        set_world_block(chunks, BlockPos::new(x, 65, -1), OAK_LOG_X)?;
    }
    for &(x, y, z) in &[(34, 64, -4), (35, 64, -4), (34, 65, -4), (35, 64, -3)] {
        set_world_block(chunks, BlockPos::new(x, y, z), HAY_BLOCK)?;
    }
    for &(x, z) in &[(-12, 8), (-8, 8), (25, 8), (29, 8)] {
        set_world_block(chunks, BlockPos::new(x, 64, z), TORCH)?;
    }
    Ok(())
}

fn stamp_placement(
    chunks: &mut BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    placement: &PlacedStructureTemplate,
) -> ChunkStoreResult<()> {
    for placed in &placement.blocks {
        set_world_block(chunks, placed.pos, placed.block)?;
    }
    Ok(())
}

fn set_world_block(
    chunks: &mut BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    pos: BlockPos,
    block: u8,
) -> ChunkStoreResult<()> {
    let chunk_pos = pos.chunk_pos();
    let chunk = chunks.get_mut(&chunk_pos).ok_or_else(|| {
        ChunkStoreError::InvalidData(format!(
            "building lab write {pos:?} escaped authored chunk radius"
        ))
    })?;
    if pos.y < chunk.min_y || pos.y >= chunk.min_y + chunk.height {
        return Err(ChunkStoreError::InvalidData(format!(
            "building lab write {pos:?} escaped authored world height"
        )));
    }
    chunk.set_block_at_y(pos.x.rem_euclid(16), pos.y, pos.z.rem_euclid(16), block);
    Ok(())
}

fn light_generated_chunks(chunks: &BTreeMap<ChunkPos, GeneratedChunk>) -> Vec<ChunkRecord> {
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
    light_state
        .compute_batch(PendingLightStatusBatch::new(pending))
        .into_iter()
        .map(|(status, light_sections, _)| {
            let mut snapshot = status.feature_snapshot;
            snapshot.status = ChunkStatus::Light;
            ChunkRecord::from_snapshot(snapshot.with_light_sections(true, light_sections))
        })
        .collect()
}

fn template_receipt(placement: &PlacedStructureTemplate) -> BuildingLabTemplateReceipt {
    BuildingLabTemplateReceipt {
        template_id: placement.template_id.clone(),
        theme_id: placement.theme_id.clone(),
        bounds_min: block_pos_array(placement.bounds.min),
        bounds_max_exclusive: block_pos_array(placement.bounds.max_exclusive),
        touched_chunks: placement
            .bounds
            .touched_chunks()
            .into_iter()
            .map(|pos| [pos.x, pos.z])
            .collect(),
        block_count: placement.blocks.len(),
        markers: placement
            .markers
            .iter()
            .map(|marker| BuildingLabMarkerReceipt {
                kind: marker.kind.clone(),
                pos: block_pos_array(marker.pos),
            })
            .collect(),
    }
}

const fn block_pos_array(pos: BlockPos) -> [i32; 3] {
    [pos.x, pos.y, pos.z]
}

fn template_error(error: TemplateError) -> ChunkStoreError {
    ChunkStoreError::InvalidData(format!("building lab template is invalid: {error}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_lab_root_for_rebuild(root: &Path) -> ChunkStoreResult<()> {
    if !root.exists() {
        return Ok(());
    }
    if !root.is_dir() {
        return Err(ChunkStoreError::InvalidData(format!(
            "building lab root `{}` is not a directory",
            root.display()
        )));
    }
    let marker_path = building_lab_marker_path(root);
    if marker_path.exists() {
        let bytes = fs::read(&marker_path)?;
        let existing: BuildingLabManifest = serde_json::from_slice(&bytes).map_err(|error| {
            ChunkStoreError::InvalidData(format!(
                "failed to parse building lab marker `{}`: {error}",
                marker_path.display()
            ))
        })?;
        if existing.schema_version != BUILDING_LAB_SCHEMA_VERSION
            || existing.gallery_id != BUILDING_LAB_GALLERY_ID
        {
            return Err(ChunkStoreError::InvalidData(format!(
                "building lab root `{}` belongs to gallery `{}` schema {}, not `{}` schema {}",
                root.display(),
                existing.gallery_id,
                existing.schema_version,
                BUILDING_LAB_GALLERY_ID,
                BUILDING_LAB_SCHEMA_VERSION
            )));
        }
        return Ok(());
    }
    if fs::read_dir(root)?.next().transpose()?.is_some() {
        return Err(ChunkStoreError::InvalidData(format!(
            "refusing to overwrite unrelated non-lab world root `{}`",
            root.display()
        )));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn remove_existing_lab_database(root: &Path) -> ChunkStoreResult<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{BlockStateId, chunk_section_index};

    fn block_at(records: &[ChunkRecord], pos: BlockPos) -> BlockStateId {
        let record = records
            .iter()
            .find(|record| record.pos() == pos.chunk_pos())
            .expect("gallery chunk exists");
        let section_y = pos.y.div_euclid(16);
        let section = record
            .snapshot
            .sections
            .iter()
            .find(|section| section.section_y == section_y)
            .expect("gallery block section exists");
        section.block_state_id_at(chunk_section_index(
            pos.x.rem_euclid(16),
            pos.y.rem_euclid(16),
            pos.z.rem_euclid(16),
        ))
    }

    #[test]
    fn authored_templates_have_expected_scale_and_markers() {
        let cottage = cottage_template().unwrap();
        let barn = barn_core_template().unwrap();
        let lean_to = barn_lean_to_template().unwrap();

        assert_eq!(cottage.size(), [15, 12, 17]);
        assert!(cottage.blocks().len() > 900);
        assert_eq!(cottage.markers().len(), 2);
        assert_eq!(barn.size(), [21, 14, 18]);
        assert!(barn.blocks().len() > 1_600);
        assert_eq!(barn.markers().len(), 3);
        assert_eq!(lean_to.size(), [7, 8, 13]);
        assert_eq!(lean_to.markers().len(), 2);
    }

    #[test]
    fn gallery_builds_lit_persistable_cross_chunk_records() {
        let (manifest, records) = building_lab_records().unwrap();

        assert_eq!(manifest.gallery_id, BUILDING_LAB_GALLERY_ID);
        assert_eq!(manifest.placements.len(), 3);
        assert_eq!(records.len(), 49);
        assert!(records.iter().all(|record| {
            record.snapshot.status == ChunkStatus::Light && record.snapshot.light_correct
        }));
        assert!(manifest.placements[0].touched_chunks.len() >= 4);
        assert_eq!(
            block_at(&records, COTTAGE_ORIGIN.offset(1, 0, 2)),
            BlockStateId(u32::from(COBBLESTONE))
        );
        assert_eq!(
            block_at(&records, COTTAGE_ORIGIN.offset(7, 9, 4)),
            BlockStateId(u32::from(SPRUCE_PLANKS))
        );
        assert_eq!(
            block_at(&records, BARN_ORIGIN.offset(1, 3, 2)),
            BlockStateId(u32::from(SPRUCE_LOG))
        );
        assert_eq!(
            block_at(&records, BARN_ORIGIN.offset(3, 3, 5)),
            BlockStateId(u32::from(HAY_BLOCK))
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn sqlite_gallery_reopens_with_landmark_intact() {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "mclone-building-lab-test-{}-{serial}",
            std::process::id()
        ));
        let manifest = write_building_lab_dir(&root).unwrap();
        assert_eq!(manifest.placements.len(), 3);

        let mut reopened = SqliteWorldStore::open_world_dir(&root).unwrap();
        let chunk = reopened
            .load_chunk(
                &mclone_protocol::DimensionKey::overworld(),
                COTTAGE_ORIGIN.chunk_pos(),
            )
            .unwrap()
            .expect("cottage chunk persisted");
        reopened.close().unwrap();
        assert!(chunk.snapshot.light_correct);
        assert!(building_lab_marker_path(&root).is_file());
        fs::remove_dir_all(&root).unwrap();
    }
}
