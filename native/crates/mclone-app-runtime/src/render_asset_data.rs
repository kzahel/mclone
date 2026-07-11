//! CPU-side render asset data shared by native and browser hosts.
//!
//! Filesystem discovery, render-device upload, and native worker construction
//! intentionally remain in their platform adapters.

use anyhow::{Context, Result};
use mclone_assets::AssetSource;
use mclone_mesh::{
    TextureAtlasImage as MeshTextureAtlasImage, TexturedMeshCatalog, TexturedRenderSectionMesh,
    VisibilityGraphBuildStats, load_first_party_textured_terrain_assets,
    load_textured_terrain_assets,
};
use mclone_render::chunk::ChunkTextureAtlas;

use crate::far_lod::FarTerrainLodMaterialPalette;

#[derive(Clone, Debug)]
pub struct SceneTexturedSections {
    pub sections: Vec<TexturedRenderSectionMesh>,
    pub visibility_graph_stats: VisibilityGraphBuildStats,
    pub atlas: TextureAtlasImage,
}

impl SceneTexturedSections {
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    pub fn index_count(&self) -> u32 {
        self.sections
            .iter()
            .map(|section| section.stats().index_count)
            .sum()
    }
}

#[derive(Clone, Debug)]
pub struct TextureAtlasImage {
    pub width: u32,
    pub height: u32,
    rgba: Vec<u8>,
}

impl TextureAtlasImage {
    pub fn as_upload(&self) -> ChunkTextureAtlas<'_> {
        ChunkTextureAtlas {
            width: self.width,
            height: self.height,
            rgba: &self.rgba,
        }
    }

    pub fn byte_len(&self) -> usize {
        self.rgba.len()
    }
}

impl From<MeshTextureAtlasImage> for TextureAtlasImage {
    fn from(value: MeshTextureAtlasImage) -> Self {
        Self {
            width: value.width,
            height: value.height,
            rgba: value.rgba,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TexturedMeshAssets {
    pub catalog: TexturedMeshCatalog,
    pub atlas: TextureAtlasImage,
    pub far_lod_materials: Option<FarTerrainLodMaterialPalette>,
}

pub fn load_textured_mesh_assets_from_source(
    source: &impl AssetSource,
) -> Result<TexturedMeshAssets> {
    let first_party_catalog =
        mclone_assets::AssetPath::new(mclone_assets::FIRST_PARTY_VISUAL_CATALOG_PATH);
    let assets = if source.read(&first_party_catalog)?.is_some() {
        load_first_party_textured_terrain_assets(source)
            .context("failed to load first-party textured terrain assets")?
    } else {
        load_textured_terrain_assets(source).context("failed to load textured terrain assets")?
    };
    let far_lod_materials = FarTerrainLodMaterialPalette::load_from_asset_source(source)
        .context("failed to load Far LOD material metadata")?;

    Ok(TexturedMeshAssets {
        catalog: assets.catalog,
        atlas: assets.atlas.into(),
        far_lod_materials,
    })
}
