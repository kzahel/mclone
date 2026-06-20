use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use mclone_assets::{
    AssetError, AssetPath, AssetSource, BlockModelLibrary, BlockStateAssetIndex,
    BlockStateRegistry, ResourceLocation, TextureAtlasPlan, TextureMaterial,
};

use crate::{TexturedMeshCatalog, TexturedMeshError};

#[derive(Clone, Debug, PartialEq)]
pub struct TexturedTerrainAssets {
    pub catalog: TexturedMeshCatalog,
    pub atlas: TextureAtlasImage,
    pub atlas_sprite_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextureAtlasImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl TextureAtlasImage {
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }
}

#[derive(Debug)]
pub enum TexturedTerrainAssetError {
    Asset(AssetError),
    Mesh(TexturedMeshError),
    MissingBlockStateAsset(ResourceLocation),
    MissingBlockStateVariant {
        block: ResourceLocation,
        variant_key: String,
    },
    EmptyAtlas,
    TextureDecode {
        path: AssetPath,
        source: image::ImageError,
    },
    TextureDimensions {
        path: AssetPath,
        decoded_width: u32,
        decoded_height: u32,
        expected_width: u32,
        expected_height: u32,
    },
}

impl fmt::Display for TexturedTerrainAssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(error) => write!(f, "{error}"),
            Self::Mesh(error) => write!(f, "{error}"),
            Self::MissingBlockStateAsset(block) => {
                write!(f, "missing blockstate asset for {block}")
            }
            Self::MissingBlockStateVariant { block, variant_key } => {
                write!(f, "missing blockstate variant `{variant_key}` for {block}")
            }
            Self::EmptyAtlas => write!(f, "cannot stitch an empty texture atlas"),
            Self::TextureDecode { path, source } => {
                write!(f, "failed to decode texture {}: {source}", path.as_str())
            }
            Self::TextureDimensions {
                path,
                decoded_width,
                decoded_height,
                expected_width,
                expected_height,
            } => write!(
                f,
                "texture {} decoded as {}x{} but atlas plan expected {}x{}",
                path.as_str(),
                decoded_width,
                decoded_height,
                expected_width,
                expected_height
            ),
        }
    }
}

impl Error for TexturedTerrainAssetError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Asset(error) => Some(error),
            Self::Mesh(error) => Some(error),
            Self::TextureDecode { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<AssetError> for TexturedTerrainAssetError {
    fn from(value: AssetError) -> Self {
        Self::Asset(value)
    }
}

impl From<TexturedMeshError> for TexturedTerrainAssetError {
    fn from(value: TexturedMeshError) -> Self {
        Self::Mesh(value)
    }
}

pub fn load_textured_terrain_assets(
    source: &impl AssetSource,
) -> Result<TexturedTerrainAssets, TexturedTerrainAssetError> {
    let registry = BlockStateRegistry::terrain_mvp();
    let blockstates = BlockStateAssetIndex::load_namespace(source, "minecraft")?;
    registry.validate_blockstate_assets(&blockstates)?;
    let selected_model_refs = selected_model_refs(&registry, &blockstates)?;
    let models = BlockModelLibrary::load_model_tree(source, selected_model_refs.iter().cloned())?;
    let mut materials = models.collect_materials_for_models(selected_model_refs.iter().cloned())?;
    insert_fluid_materials(&mut materials);
    let atlas_plan = TextureAtlasPlan::build(source, materials)?;
    let atlas_sprite_count = atlas_plan.len();
    let atlas = stitch_texture_atlas(source, &atlas_plan)?;
    let catalog = TexturedMeshCatalog::from_assets(&registry, &blockstates, &models, &atlas_plan)?;

    Ok(TexturedTerrainAssets {
        catalog,
        atlas,
        atlas_sprite_count,
    })
}

fn selected_model_refs(
    registry: &BlockStateRegistry,
    blockstates: &BlockStateAssetIndex,
) -> Result<BTreeSet<ResourceLocation>, TexturedTerrainAssetError> {
    let mut refs = BTreeSet::new();
    for record in registry.records() {
        let asset = blockstates.get(&record.block).ok_or_else(|| {
            TexturedTerrainAssetError::MissingBlockStateAsset(record.block.clone())
        })?;
        if let Some(variant_key) = record.asset_variant_key(asset) {
            let variants = asset.variants_for_key(&variant_key).ok_or_else(|| {
                TexturedTerrainAssetError::MissingBlockStateVariant {
                    block: record.block.clone(),
                    variant_key: variant_key.clone(),
                }
            })?;
            if let Some(variant) = variants.first() {
                refs.insert(variant.model.clone());
            }
        } else if record.variant_key().is_empty() && !asset.model_refs.is_empty() {
            refs.extend(asset.model_refs.iter().cloned());
        } else {
            return Err(TexturedTerrainAssetError::MissingBlockStateVariant {
                block: record.block.clone(),
                variant_key: record.variant_key(),
            });
        }
    }
    Ok(refs)
}

fn insert_fluid_materials(materials: &mut BTreeSet<TextureMaterial>) {
    for texture in [
        "minecraft:block/water_still",
        "minecraft:block/water_flow",
        "minecraft:block/lava_still",
        "minecraft:block/lava_flow",
    ] {
        materials.insert(TextureMaterial::blocks(
            ResourceLocation::parse(texture).expect("fluid texture locations are valid"),
        ));
    }
}

fn stitch_texture_atlas(
    source: &impl AssetSource,
    plan: &TextureAtlasPlan,
) -> Result<TextureAtlasImage, TexturedTerrainAssetError> {
    let width = plan.width();
    let height = plan.height();
    if width == 0 || height == 0 {
        return Err(TexturedTerrainAssetError::EmptyAtlas);
    }
    let mut atlas = vec![0; width as usize * height as usize * 4];

    for sprite in plan.sprites() {
        let bytes = source
            .read(&sprite.info.path)?
            .ok_or_else(|| AssetError::MissingAsset(sprite.info.path.clone()))?;
        let image = image::load_from_memory(&bytes)
            .map_err(|source| TexturedTerrainAssetError::TextureDecode {
                path: sprite.info.path.clone(),
                source,
            })?
            .to_rgba8();
        let (decoded_width, decoded_height) = image.dimensions();
        if decoded_width != sprite.info.width || decoded_height != sprite.info.height {
            return Err(TexturedTerrainAssetError::TextureDimensions {
                path: sprite.info.path.clone(),
                decoded_width,
                decoded_height,
                expected_width: sprite.info.width,
                expected_height: sprite.info.height,
            });
        }

        let image = image.as_raw();
        for row in 0..sprite.info.height {
            let source_start = (row * sprite.info.width * 4) as usize;
            let source_end = source_start + (sprite.info.width * 4) as usize;
            let dest_start = (((sprite.y + row) * width + sprite.x) * 4) as usize;
            let dest_end = dest_start + (sprite.info.width * 4) as usize;
            atlas[dest_start..dest_end].copy_from_slice(&image[source_start..source_end]);
        }
    }

    Ok(TextureAtlasImage {
        width,
        height,
        rgba: atlas,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn loads_real_extracted_textured_terrain_assets_when_present() {
        let root = std::path::PathBuf::from("../reference/minecraft-1.17.1/extracted");
        if !root.exists() {
            return;
        }
        let source = mclone_assets::FilesystemAssetSource::new(root);

        let assets = load_textured_terrain_assets(&source).unwrap();

        assert!(assets.atlas.width > 0);
        assert!(assets.atlas.height > 0);
        assert_eq!(
            assets.atlas.rgba.len(),
            assets.atlas.width as usize * assets.atlas.height as usize * 4
        );
        assert!(assets.atlas_sprite_count > 0);
        assert!(assets.catalog.get(mclone_core::BlockStateId(1)).is_some());
    }
}
