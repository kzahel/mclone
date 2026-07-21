use std::collections::BTreeMap;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use mclone_core::{BlockPos, ChunkPos, ChunkRevision, ChunkStatus};
use mclone_worldgen::block::{
    ALLIUM, COARSE_DIRT, CORNFLOWER, DANDELION, DIRT, GRASS_BLOCK, GRAVEL, HAY_BLOCK, OAK_LOG_X,
    POPPY, STONE, TORCH,
};
#[cfg(test)]
use mclone_worldgen::block::{
    COBBLESTONE, GLASS, SPRUCE_LOG, SPRUCE_STAIRS_EAST, WHITE_TERRACOTTA,
};
use mclone_worldgen::levelgen::{GeneratedChunk, MutableChunkBlockBuffer};
use mclone_worldgen::structure_json::load_canonical_structure_json;
use mclone_worldgen::structure_template::{
    PlacedStructureTemplate, StructureMaterialTheme, StructurePlaceSettings, StructureTemplate,
    TemplateError, TemplateMirror, TemplateRotation,
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

pub const STRUCTURE_LAB_GALLERY_ID: &str = "standalone-structure-lab-v2";
pub const STRUCTURE_LAB_MARKER_FILE: &str = "mclone-structure-lab.json";
pub const STRUCTURE_LAB_SCHEMA_VERSION: u32 = 1;
pub const STRUCTURE_LAB_SEED: i64 = 20_801;
pub const STRUCTURE_LAB_VOID_PADDING_RADIUS: i32 = 3;
pub const STRUCTURE_FAMILY_LAB_GALLERY_ID: &str = "bounded-structure-families-v1";
pub const STRUCTURE_FAMILY_LAB_MARKER_FILE: &str = "mclone-structure-family-lab.json";
pub const STRUCTURE_FAMILY_LAB_SEED: i64 = 21_001;
pub const STRUCTURE_FAMILY_LAB_VOID_PADDING_RADIUS: i32 = 4;

const COTTAGE_ORIGIN: BlockPos = BlockPos::new(-29, 63, -10);
const BARN_ORIGIN: BlockPos = BlockPos::new(7, 63, -12);
const BARN_LEAN_TO_ORIGIN: BlockPos = BlockPos::new(28, 63, -7);

const FAMILY_COTTAGE_ORIGINS: [BlockPos; 3] = [
    BlockPos::new(-31, 63, 10),
    BlockPos::new(-7, 63, 10),
    BlockPos::new(17, 63, 10),
];
const FAMILY_BARN_ORIGINS: [BlockPos; 3] = [
    BlockPos::new(-45, 63, -34),
    BlockPos::new(-12, 63, -34),
    BlockPos::new(25, 63, -34),
];

/// Curated longitudinal cottage plans. Width, wall height, and roof pitch are
/// authored invariants rather than raw procedural dimensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CottageDepth {
    Snug,
    Standard,
    Deep,
}

impl CottageDepth {
    #[cfg(test)]
    const fn front_z(self) -> i32 {
        match self {
            Self::Snug => 10,
            Self::Standard => 12,
            Self::Deep => 14,
        }
    }
}

/// Authored cottage entry modules. A stoop keeps the doorway accessible but
/// does not claim the footprint or posts of the accepted canopy porch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CottageEntry {
    Stoop,
    CanopyPorch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CottageVariant {
    pub depth: CottageDepth,
    pub entry: CottageEntry,
}

impl CottageVariant {
    pub const STANDARD: Self = Self::new(CottageDepth::Standard, CottageEntry::CanopyPorch);

    pub const fn new(depth: CottageDepth, entry: CottageEntry) -> Self {
        Self { depth, entry }
    }
}

/// Curated barn lengths measured in whole structural bays.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BarnLength {
    Short,
    Standard,
    Long,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BarnVariant {
    pub length: BarnLength,
    pub lean_to: bool,
}

impl BarnVariant {
    pub const STANDARD: Self = Self::new(BarnLength::Standard, true);

    pub const fn new(length: BarnLength, lean_to: bool) -> Self {
        Self { length, lean_to }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BarnTemplateSet {
    pub core: StructureTemplate,
    pub lean_to: Option<StructureTemplate>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureLabMarkerReceipt {
    pub kind: String,
    pub pos: [i32; 3],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureLabTemplateReceipt {
    pub template_id: String,
    pub theme_id: String,
    pub bounds_min: [i32; 3],
    pub bounds_max_exclusive: [i32; 3],
    pub touched_chunks: Vec<[i32; 2]>,
    pub block_count: usize,
    pub markers: Vec<StructureLabMarkerReceipt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureLabManifest {
    pub schema_version: u32,
    pub gallery_id: String,
    pub seed: i64,
    pub world_generation_profile: WorldGenerationProfile,
    pub void_padding_radius: i32,
    pub expected_spawn: [f64; 3],
    pub placements: Vec<StructureLabTemplateReceipt>,
}

pub fn structure_lab_records() -> ChunkStoreResult<(StructureLabManifest, Vec<ChunkRecord>)> {
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
    for chunk_z in -STRUCTURE_LAB_VOID_PADDING_RADIUS..=STRUCTURE_LAB_VOID_PADDING_RADIUS {
        for chunk_x in -STRUCTURE_LAB_VOID_PADDING_RADIUS..=STRUCTURE_LAB_VOID_PADDING_RADIUS {
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
    let manifest = StructureLabManifest {
        schema_version: STRUCTURE_LAB_SCHEMA_VERSION,
        gallery_id: STRUCTURE_LAB_GALLERY_ID.to_owned(),
        seed: STRUCTURE_LAB_SEED,
        world_generation_profile: WorldGenerationProfile::authored_only(),
        void_padding_radius: STRUCTURE_LAB_VOID_PADDING_RADIUS,
        expected_spawn: [3.5, 64.0, 40.5],
        placements: placements.iter().map(template_receipt).collect(),
    };
    Ok((manifest, records))
}

pub fn structure_family_lab_records() -> ChunkStoreResult<(StructureLabManifest, Vec<ChunkRecord>)>
{
    let cottage_variants = [
        CottageVariant::new(CottageDepth::Snug, CottageEntry::Stoop),
        CottageVariant::STANDARD,
        CottageVariant::new(CottageDepth::Deep, CottageEntry::CanopyPorch),
    ];
    let barn_variants = [
        BarnVariant::new(BarnLength::Short, false),
        BarnVariant::STANDARD,
        BarnVariant::new(BarnLength::Long, true),
    ];
    let cottage_theme = cottage_theme();
    let barn_theme = barn_theme();
    let mut placements = Vec::with_capacity(8);

    for (variant, origin) in cottage_variants.into_iter().zip(FAMILY_COTTAGE_ORIGINS) {
        placements.push(
            cottage_template_for(variant)
                .map_err(template_error)?
                .place(&StructurePlaceSettings {
                    origin,
                    rotation: TemplateRotation::None,
                    mirror: TemplateMirror::None,
                    theme: &cottage_theme,
                })
                .map_err(template_error)?,
        );
    }
    for (variant, origin) in barn_variants.into_iter().zip(FAMILY_BARN_ORIGINS) {
        let family = barn_templates_for(variant).map_err(template_error)?;
        placements.push(
            family
                .core
                .place(&StructurePlaceSettings {
                    origin,
                    rotation: TemplateRotation::None,
                    mirror: TemplateMirror::None,
                    theme: &barn_theme,
                })
                .map_err(template_error)?,
        );
        if let Some(lean_to) = family.lean_to {
            placements.push(
                lean_to
                    .place(&StructurePlaceSettings {
                        origin: origin.offset(21, 0, 5),
                        rotation: TemplateRotation::None,
                        mirror: TemplateMirror::None,
                        theme: &barn_theme,
                    })
                    .map_err(template_error)?,
            );
        }
    }

    let mut chunks = BTreeMap::new();
    for chunk_z in
        -STRUCTURE_FAMILY_LAB_VOID_PADDING_RADIUS..=STRUCTURE_FAMILY_LAB_VOID_PADDING_RADIUS
    {
        for chunk_x in
            -STRUCTURE_FAMILY_LAB_VOID_PADDING_RADIUS..=STRUCTURE_FAMILY_LAB_VOID_PADDING_RADIUS
        {
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

    author_family_gallery_landscape(&mut chunks)?;
    for placement in &placements {
        stamp_placement(&mut chunks, placement)?;
    }

    let generated = chunks
        .into_iter()
        .map(|(pos, buffer)| (pos, GeneratedChunk::from_mutable_buffer(buffer)))
        .collect::<BTreeMap<_, _>>();
    let records = light_generated_chunks(&generated);
    let manifest = StructureLabManifest {
        schema_version: STRUCTURE_LAB_SCHEMA_VERSION,
        gallery_id: STRUCTURE_FAMILY_LAB_GALLERY_ID.to_owned(),
        seed: STRUCTURE_FAMILY_LAB_SEED,
        world_generation_profile: WorldGenerationProfile::authored_only(),
        void_padding_radius: STRUCTURE_FAMILY_LAB_VOID_PADDING_RADIUS,
        expected_spawn: [10.5, 64.0, 54.5],
        placements: placements.iter().map(template_receipt).collect(),
    };
    Ok((manifest, records))
}

pub fn write_structure_lab_to_store(
    store: &mut dyn WorldStore,
) -> ChunkStoreResult<StructureLabManifest> {
    let (manifest, records) = structure_lab_records()?;
    for record in &records {
        store.save_chunk(&mclone_protocol::DimensionKey::overworld(), record)?;
    }
    store.flush()?;
    Ok(manifest)
}

pub fn structure_lab_memory_store() -> ChunkStoreResult<(StructureLabManifest, MemoryWorldStore)> {
    let mut store = MemoryWorldStore::new();
    let manifest = write_structure_lab_to_store(&mut store)?;
    Ok((manifest, store))
}

pub fn write_structure_family_lab_to_store(
    store: &mut dyn WorldStore,
) -> ChunkStoreResult<StructureLabManifest> {
    let (manifest, records) = structure_family_lab_records()?;
    for record in &records {
        store.save_chunk(&mclone_protocol::DimensionKey::overworld(), record)?;
    }
    store.flush()?;
    Ok(manifest)
}

pub fn structure_family_lab_memory_store()
-> ChunkStoreResult<(StructureLabManifest, MemoryWorldStore)> {
    let mut store = MemoryWorldStore::new();
    let manifest = write_structure_family_lab_to_store(&mut store)?;
    Ok((manifest, store))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn write_structure_lab_dir(root: impl AsRef<Path>) -> ChunkStoreResult<StructureLabManifest> {
    let root = root.as_ref();
    validate_lab_root_for_rebuild(root, STRUCTURE_LAB_MARKER_FILE, STRUCTURE_LAB_GALLERY_ID)?;
    fs::create_dir_all(root)?;
    remove_existing_lab_database(root)?;

    let mut store = SqliteWorldStore::open_world_dir(root)?;
    let manifest = write_structure_lab_to_store(&mut store)?;
    store.close()?;
    let marker = serde_json::to_vec_pretty(&manifest).map_err(|error| {
        ChunkStoreError::InvalidData(format!("failed to encode structure lab marker: {error}"))
    })?;
    fs::write(structure_lab_marker_path(root), marker)?;
    Ok(manifest)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn write_structure_family_lab_dir(
    root: impl AsRef<Path>,
) -> ChunkStoreResult<StructureLabManifest> {
    let root = root.as_ref();
    validate_lab_root_for_rebuild(
        root,
        STRUCTURE_FAMILY_LAB_MARKER_FILE,
        STRUCTURE_FAMILY_LAB_GALLERY_ID,
    )?;
    fs::create_dir_all(root)?;
    remove_existing_lab_database(root)?;

    let mut store = SqliteWorldStore::open_world_dir(root)?;
    let manifest = write_structure_family_lab_to_store(&mut store)?;
    store.close()?;
    let marker = serde_json::to_vec_pretty(&manifest).map_err(|error| {
        ChunkStoreError::InvalidData(format!(
            "failed to encode structure family lab marker: {error}"
        ))
    })?;
    fs::write(structure_family_lab_marker_path(root), marker)?;
    Ok(manifest)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn structure_lab_marker_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join(STRUCTURE_LAB_MARKER_FILE)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn structure_family_lab_marker_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join(STRUCTURE_FAMILY_LAB_MARKER_FILE)
}

pub fn cottage_template() -> Result<StructureTemplate, TemplateError> {
    cottage_template_for(CottageVariant::STANDARD)
}

pub fn cottage_template_for(variant: CottageVariant) -> Result<StructureTemplate, TemplateError> {
    let json = match (variant.depth, variant.entry) {
        (CottageDepth::Snug, CottageEntry::Stoop) => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/mclone/structures/farmstead-cottage-snug-stoop-v1.structure.json"
        )),
        (CottageDepth::Snug, CottageEntry::CanopyPorch) => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/mclone/structures/farmstead-cottage-snug-canopy-porch-v1.structure.json"
        )),
        (CottageDepth::Standard, CottageEntry::Stoop) => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/mclone/structures/farmstead-cottage-standard-stoop-v1.structure.json"
        )),
        (CottageDepth::Standard, CottageEntry::CanopyPorch) => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/mclone/structures/farmstead-cottage-a-v2.structure.json"
        )),
        (CottageDepth::Deep, CottageEntry::Stoop) => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/mclone/structures/farmstead-cottage-deep-stoop-v1.structure.json"
        )),
        (CottageDepth::Deep, CottageEntry::CanopyPorch) => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/mclone/structures/farmstead-cottage-deep-canopy-porch-v1.structure.json"
        )),
    };
    load_promoted_template(json)
}

pub fn barn_core_template() -> Result<StructureTemplate, TemplateError> {
    barn_core_template_for(BarnLength::Standard)
}

pub fn barn_core_template_for(length: BarnLength) -> Result<StructureTemplate, TemplateError> {
    let json = match length {
        BarnLength::Short => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/mclone/structures/farmstead-barn-core-short-v1.structure.json"
        )),
        BarnLength::Standard => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/mclone/structures/farmstead-barn-core-a-v2.structure.json"
        )),
        BarnLength::Long => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/mclone/structures/farmstead-barn-core-long-v1.structure.json"
        )),
    };
    load_promoted_template(json)
}

pub fn barn_lean_to_template() -> Result<StructureTemplate, TemplateError> {
    barn_lean_to_template_for(BarnLength::Standard)
}

pub fn barn_lean_to_template_for(length: BarnLength) -> Result<StructureTemplate, TemplateError> {
    let json = match length {
        BarnLength::Short => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/mclone/structures/farmstead-barn-lean-to-short-v1.structure.json"
        )),
        BarnLength::Standard => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/mclone/structures/farmstead-barn-lean-to-a-v2.structure.json"
        )),
        BarnLength::Long => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/mclone/structures/farmstead-barn-lean-to-long-v1.structure.json"
        )),
    };
    load_promoted_template(json)
}

pub fn barn_templates_for(variant: BarnVariant) -> Result<BarnTemplateSet, TemplateError> {
    Ok(BarnTemplateSet {
        core: barn_core_template_for(variant.length)?,
        lean_to: variant
            .lean_to
            .then(|| barn_lean_to_template_for(variant.length))
            .transpose()?,
    })
}

fn load_promoted_template(json: &str) -> Result<StructureTemplate, TemplateError> {
    load_canonical_structure_json(json)
        .map(|record| record.template)
        .map_err(|error| TemplateError::InvalidCanonicalRecord(error.to_string()))
}

fn cottage_theme() -> StructureMaterialTheme {
    load_canonical_structure_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../assets/mclone/structures/farmstead-cottage-a-v2.structure.json"
    )))
    .expect("checked canonical cottage must load")
    .default_theme()
    .expect("checked canonical cottage must declare its default theme")
    .clone()
}

fn barn_theme() -> StructureMaterialTheme {
    load_canonical_structure_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../assets/mclone/structures/farmstead-barn-core-a-v2.structure.json"
    )))
    .expect("checked canonical barn must load")
    .default_theme()
    .expect("checked canonical barn must declare its default theme")
    .clone()
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

fn author_family_gallery_landscape(
    chunks: &mut BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
) -> ChunkStoreResult<()> {
    // Two quiet comparison lanes make longitudinal growth legible without
    // turning the family lab into a settlement composition prematurely.
    for x in -36_i32..=36 {
        for z in 32_i32..=34 {
            let block = if (x + z).rem_euclid(5) == 0 {
                COARSE_DIRT
            } else {
                GRAVEL
            };
            set_world_block(chunks, BlockPos::new(x, 63, z), block)?;
        }
    }
    for x in -49_i32..=61 {
        for z in -9_i32..=-7 {
            let block = if (x - z).rem_euclid(6) == 0 {
                COARSE_DIRT
            } else {
                GRAVEL
            };
            set_world_block(chunks, BlockPos::new(x, 63, z), block)?;
        }
    }
    for (entrance_x, entrance_z, lane_z) in [
        (-22, 22, 32),
        (2, 26, 32),
        (26, 30, 32),
        (-35, -21, -9),
        (-2, -17, -9),
        (35, -13, -9),
    ] {
        for z in entrance_z..=lane_z {
            set_world_block(chunks, BlockPos::new(entrance_x, 63, z), COARSE_DIRT)?;
        }
    }
    for z in -7..=54 {
        for x in 9..=11 {
            set_world_block(chunks, BlockPos::new(x, 63, z), GRAVEL)?;
        }
    }

    for (x, z, block) in [
        (-34, 29, DANDELION),
        (-15, 31, POPPY),
        (9, 29, ALLIUM),
        (34, 30, CORNFLOWER),
        (-47, -5, ALLIUM),
        (-22, -11, DANDELION),
        (18, -5, POPPY),
        (59, -11, CORNFLOWER),
    ] {
        set_world_block(chunks, BlockPos::new(x, 64, z), block)?;
    }
    for &(x, z) in &[(-26, 33), (-2, 33), (22, 33), (-39, -8), (-6, -8), (31, -8)] {
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
            "structure lab write {pos:?} escaped authored chunk radius"
        ))
    })?;
    if pos.y < chunk.min_y || pos.y >= chunk.min_y + chunk.height {
        return Err(ChunkStoreError::InvalidData(format!(
            "structure lab write {pos:?} escaped authored world height"
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

fn template_receipt(placement: &PlacedStructureTemplate) -> StructureLabTemplateReceipt {
    StructureLabTemplateReceipt {
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
            .map(|marker| StructureLabMarkerReceipt {
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
    ChunkStoreError::InvalidData(format!("structure lab template is invalid: {error}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_lab_root_for_rebuild(
    root: &Path,
    marker_file: &str,
    gallery_id: &str,
) -> ChunkStoreResult<()> {
    if !root.exists() {
        return Ok(());
    }
    if !root.is_dir() {
        return Err(ChunkStoreError::InvalidData(format!(
            "structure lab root `{}` is not a directory",
            root.display()
        )));
    }
    let marker_path = root.join(marker_file);
    if marker_path.exists() {
        let bytes = fs::read(&marker_path)?;
        let existing: StructureLabManifest = serde_json::from_slice(&bytes).map_err(|error| {
            ChunkStoreError::InvalidData(format!(
                "failed to parse structure lab marker `{}`: {error}",
                marker_path.display()
            ))
        })?;
        if existing.schema_version != STRUCTURE_LAB_SCHEMA_VERSION
            || existing.gallery_id != gallery_id
        {
            return Err(ChunkStoreError::InvalidData(format!(
                "structure lab root `{}` belongs to gallery `{}` schema {}, not `{}` schema {}",
                root.display(),
                existing.gallery_id,
                existing.schema_version,
                gallery_id,
                STRUCTURE_LAB_SCHEMA_VERSION
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

        assert_eq!(cottage.size(), [15, 15, 17]);
        assert!(cottage.blocks().len() > 900);
        assert_eq!(cottage.markers().len(), 2);
        assert_eq!(barn.size(), [21, 14, 18]);
        assert!(barn.blocks().len() > 1_600);
        assert_eq!(barn.markers().len(), 3);
        assert_eq!(lean_to.size(), [7, 8, 13]);
        assert_eq!(lean_to.markers().len(), 2);
    }

    #[test]
    fn bounded_cottage_family_preserves_authored_invariants() {
        assert_eq!(
            cottage_template().unwrap(),
            cottage_template_for(CottageVariant::STANDARD).unwrap()
        );

        let snug = cottage_template_for(CottageVariant::new(
            CottageDepth::Snug,
            CottageEntry::CanopyPorch,
        ))
        .unwrap();
        let standard = cottage_template_for(CottageVariant::STANDARD).unwrap();
        let deep = cottage_template_for(CottageVariant::new(
            CottageDepth::Deep,
            CottageEntry::CanopyPorch,
        ))
        .unwrap();
        assert_eq!(snug.size(), [15, 15, 15]);
        assert_eq!(standard.size(), [15, 15, 17]);
        assert_eq!(deep.size(), [15, 15, 19]);
        assert!(snug.blocks().len() < standard.blocks().len());
        assert!(standard.blocks().len() < deep.blocks().len());

        for depth in [
            CottageDepth::Snug,
            CottageDepth::Standard,
            CottageDepth::Deep,
        ] {
            let stoop =
                cottage_template_for(CottageVariant::new(depth, CottageEntry::Stoop)).unwrap();
            assert_eq!(stoop.size()[0..2], [15, 15]);
            assert_eq!(stoop.size()[2], depth.front_z() + 3);
            assert_eq!(stoop.markers().len(), 2);
            assert_eq!(
                stoop,
                cottage_template_for(CottageVariant::new(depth, CottageEntry::Stoop)).unwrap()
            );
        }
    }

    #[test]
    fn bounded_barn_family_uses_whole_bays_and_optional_lean_to() {
        assert_eq!(
            barn_core_template().unwrap(),
            barn_core_template_for(BarnLength::Standard).unwrap()
        );
        assert_eq!(
            barn_lean_to_template().unwrap(),
            barn_lean_to_template_for(BarnLength::Standard).unwrap()
        );

        let short = barn_templates_for(BarnVariant::new(BarnLength::Short, false)).unwrap();
        let standard = barn_templates_for(BarnVariant::STANDARD).unwrap();
        let long = barn_templates_for(BarnVariant::new(BarnLength::Long, true)).unwrap();
        assert_eq!(short.core.size(), [21, 14, 14]);
        assert_eq!(standard.core.size(), [21, 14, 18]);
        assert_eq!(long.core.size(), [21, 14, 22]);
        assert!(short.core.blocks().len() < standard.core.blocks().len());
        assert!(standard.core.blocks().len() < long.core.blocks().len());
        assert!(short.lean_to.is_none());
        assert_eq!(standard.lean_to.unwrap().size(), [7, 8, 13]);
        assert_eq!(long.lean_to.unwrap().size(), [7, 8, 17]);
    }

    #[test]
    fn family_gallery_builds_lit_comparison_records() {
        let (manifest, records) = structure_family_lab_records().unwrap();

        assert_eq!(manifest.gallery_id, STRUCTURE_FAMILY_LAB_GALLERY_ID);
        assert_eq!(manifest.placements.len(), 8);
        assert_eq!(records.len(), 81);
        assert!(records.iter().all(|record| {
            record.snapshot.status == ChunkStatus::Light && record.snapshot.light_correct
        }));
        for origin in FAMILY_COTTAGE_ORIGINS {
            assert_eq!(
                block_at(&records, origin.offset(1, 0, 2)),
                BlockStateId(u32::from(COBBLESTONE))
            );
        }
        let ids = manifest
            .placements
            .iter()
            .map(|placement| placement.template_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            [
                "farmstead-cottage-snug-stoop-v1",
                "farmstead-cottage-a-v2",
                "farmstead-cottage-deep-canopy-porch-v1",
                "farmstead-barn-core-short-v1",
                "farmstead-barn-core-a-v2",
                "farmstead-barn-lean-to-a-v2",
                "farmstead-barn-core-long-v1",
                "farmstead-barn-lean-to-long-v1",
            ]
        );
    }

    #[test]
    fn gallery_builds_lit_persistable_cross_chunk_records() {
        let (manifest, records) = structure_lab_records().unwrap();

        assert_eq!(manifest.gallery_id, STRUCTURE_LAB_GALLERY_ID);
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
            block_at(&records, COTTAGE_ORIGIN.offset(0, 6, 4)),
            BlockStateId(u32::from(SPRUCE_STAIRS_EAST))
        );
        assert_eq!(
            block_at(&records, COTTAGE_ORIGIN.offset(3, 2, 12)),
            BlockStateId(u32::from(GLASS))
        );
        assert_eq!(
            block_at(&records, BARN_ORIGIN.offset(1, 3, 2)),
            BlockStateId(u32::from(SPRUCE_LOG))
        );
        assert_eq!(
            block_at(&records, BARN_ORIGIN.offset(3, 3, 5)),
            BlockStateId(u32::from(HAY_BLOCK))
        );
        assert_eq!(
            block_at(&records, BARN_ORIGIN.offset(3, 2, 15)),
            BlockStateId(u32::from(GLASS))
        );
        assert_eq!(
            block_at(&records, BARN_ORIGIN.offset(2, 3, 15)),
            BlockStateId(u32::from(WHITE_TERRACOTTA))
        );
        assert_eq!(
            block_at(&records, BARN_ORIGIN.offset(2, 8, 4)),
            BlockStateId(u32::from(SPRUCE_STAIRS_EAST))
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn sqlite_gallery_reopens_with_landmark_intact() {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "mclone-structure-lab-test-{}-{serial}",
            std::process::id()
        ));
        let manifest = write_structure_lab_dir(&root).unwrap();
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
        assert!(structure_lab_marker_path(&root).is_file());
        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn sqlite_family_gallery_reopens_and_protects_unrelated_roots() {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "mclone-structure-family-lab-test-{}-{serial}",
            std::process::id()
        ));
        let manifest = write_structure_family_lab_dir(&root).unwrap();
        assert_eq!(manifest.placements.len(), 8);

        let mut reopened = SqliteWorldStore::open_world_dir(&root).unwrap();
        let chunk = reopened
            .load_chunk(
                &mclone_protocol::DimensionKey::overworld(),
                FAMILY_COTTAGE_ORIGINS[0].chunk_pos(),
            )
            .unwrap()
            .expect("family cottage chunk persisted");
        reopened.close().unwrap();
        assert!(chunk.snapshot.light_correct);
        assert!(structure_family_lab_marker_path(&root).is_file());
        fs::remove_dir_all(&root).unwrap();

        let unrelated = std::env::temp_dir().join(format!(
            "mclone-structure-family-unrelated-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&unrelated).unwrap();
        fs::write(unrelated.join("keep.txt"), b"not a gallery").unwrap();
        assert!(write_structure_family_lab_dir(&unrelated).is_err());
        assert!(unrelated.join("keep.txt").is_file());
        fs::remove_dir_all(unrelated).unwrap();
    }
}
