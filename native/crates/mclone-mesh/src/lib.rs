#![forbid(unsafe_code)]

mod ambient_occlusion;
mod builder;
mod catalog;
mod data;
mod packed;
mod render_facts;
mod terrain_assets;
mod tint;
mod visibility;

use mclone_core::BlockStateId;

pub use builder::{
    BUSHY_LEAF_CARD_OVERHANG, ChunkMeshInput, TexturedChunkMeshInput,
    TexturedRenderSectionBuildOptions, build_textured_render_sections,
    build_textured_render_sections_for_chunk_set,
    build_textured_render_sections_for_chunk_set_with_stats,
    build_textured_render_sections_for_section_set_with_stats,
    build_textured_render_sections_for_section_set_with_stats_and_options,
    build_textured_render_sections_with_stats, build_textured_visible_chunk_area_mesh,
    build_textured_visible_chunk_mesh, build_visible_chunk_area_mesh, build_visible_chunk_mesh,
};
pub use catalog::{
    AtlasSpriteUv, LeafDetail, TexturedBlockFace, TexturedBlockModel, TexturedColorMap,
    TexturedColorMaps, TexturedFluidKind, TexturedFluidModel, TexturedLeafCardModel,
    TexturedMeshCatalog, TexturedMeshError, TexturedTerrainRenderLayer,
};
pub use data::{
    ChunkVertex, GrassPatch, RenderSectionKey, SectionMeshStats, TexturedChunkVertex,
    TexturedRenderSectionBuildReport, TexturedRenderSectionMesh, TexturedRenderSectionMetadata,
    TexturedVisibleChunkMesh, VisibilityGraphBuildStats, VisibleChunkMesh,
    quad_face_count_from_indices,
};
pub use mclone_core::{CHUNK_WIDTH, SECTION_HEIGHT as RENDER_SECTION_HEIGHT};
pub use packed::{
    PackedTexturedSectionError, pack_textured_render_sections, unpack_textured_render_sections,
};
pub use terrain_assets::{
    TextureAtlasImage, TexturedTerrainAssetError, TexturedTerrainAssets,
    TexturedTerrainMaterialSummary, collect_textured_terrain_materials,
    load_first_party_textured_terrain_assets,
    load_first_party_textured_terrain_assets_with_presentation, load_textured_terrain_assets,
    load_textured_terrain_assets_with_presentation,
};
pub use visibility::{SectionFace, VisGraph, VisibilitySet};

pub const QUAD_FACE_INDEX_COUNT: u32 = 6;
pub const AIR_BLOCK_ID: u8 = 0;
pub const CAVE_AIR_BLOCK_ID: u8 = 71;
pub const CAVE_AIR_BLOCK_STATE_ID: BlockStateId = BlockStateId(71);

#[cfg(test)]
mod tests;
