use std::error::Error;
use std::fmt;

use mclone_assets::{AssetError, AssetPath, AssetSource};

use crate::asset_lab_figure::{SemanticFigureSet, load_first_party_semantic_figures};
use crate::entity::{ActorTextureAtlas, ActorTextureLayout, ActorTextureRegion};

const COW_TEXTURE_PATH: &str = "assets/minecraft/textures/entity/cow/cow.png";
const COW_TEXTURE_WIDTH: u32 = 64;
const COW_TEXTURE_HEIGHT: u32 = 32;

#[derive(Clone, Debug)]
pub struct ActorTextureAssets {
    pub atlas: ActorTextureImage,
    pub figures: SemanticFigureSet,
}

#[derive(Clone, Debug)]
pub struct ActorTextureImage {
    pub width: u32,
    pub height: u32,
    rgba: Vec<u8>,
    layout: ActorTextureLayout,
}

impl ActorTextureImage {
    pub fn as_upload(&self) -> ActorTextureAtlas<'_> {
        ActorTextureAtlas {
            width: self.width,
            height: self.height,
            rgba: &self.rgba,
            layout: self.layout,
        }
    }

    pub const fn layout(&self) -> ActorTextureLayout {
        self.layout
    }

    pub fn byte_len(&self) -> usize {
        self.rgba.len()
    }
}

pub fn load_actor_texture_assets(
    source: &impl AssetSource,
) -> Result<ActorTextureAssets, ActorTextureAssetError> {
    let cow = read_rgba_texture(
        source,
        &AssetPath::new(COW_TEXTURE_PATH),
        COW_TEXTURE_WIDTH,
        COW_TEXTURE_HEIGHT,
    )?;
    let atlas = stitch_actor_texture_atlas(&cow);
    let figures = load_first_party_semantic_figures(source)
        .map_err(|source| ActorTextureAssetError::Figures { source })?;

    Ok(ActorTextureAssets { atlas, figures })
}

#[derive(Debug)]
pub enum ActorTextureAssetError {
    Asset(AssetError),
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
    Figures {
        source: anyhow::Error,
    },
}

impl fmt::Display for ActorTextureAssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(error) => write!(f, "{error}"),
            Self::TextureDecode { path, source } => {
                write!(
                    f,
                    "failed to decode actor texture {}: {source}",
                    path.as_str()
                )
            }
            Self::TextureDimensions {
                path,
                decoded_width,
                decoded_height,
                expected_width,
                expected_height,
            } => write!(
                f,
                "actor texture {} decoded as {}x{} but expected {}x{}",
                path.as_str(),
                decoded_width,
                decoded_height,
                expected_width,
                expected_height
            ),
            Self::Figures { source } => {
                write!(f, "failed to load actor figure registry: {source}")
            }
        }
    }
}

impl Error for ActorTextureAssetError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Asset(error) => Some(error),
            Self::TextureDecode { source, .. } => Some(source),
            Self::TextureDimensions { .. } => None,
            Self::Figures { source } => Some(source.as_ref()),
        }
    }
}

impl From<AssetError> for ActorTextureAssetError {
    fn from(value: AssetError) -> Self {
        Self::Asset(value)
    }
}

fn read_rgba_texture(
    source: &impl AssetSource,
    path: &AssetPath,
    expected_width: u32,
    expected_height: u32,
) -> Result<RgbaTexture, ActorTextureAssetError> {
    let bytes = source
        .read(path)?
        .ok_or_else(|| AssetError::MissingAsset(path.clone()))?;
    let image = image::load_from_memory(&bytes)
        .map_err(|source| ActorTextureAssetError::TextureDecode {
            path: path.clone(),
            source,
        })?
        .to_rgba8();
    let (width, height) = image.dimensions();
    if width != expected_width || height != expected_height {
        return Err(ActorTextureAssetError::TextureDimensions {
            path: path.clone(),
            decoded_width: width,
            decoded_height: height,
            expected_width,
            expected_height,
        });
    }

    Ok(RgbaTexture {
        width,
        height,
        rgba: image.into_raw(),
    })
}

fn stitch_actor_texture_atlas(cow: &RgbaTexture) -> ActorTextureImage {
    let width = cow.width + 1;
    let height = cow.height.max(1);
    let mut rgba = vec![0; width as usize * height as usize * 4];
    rgba[0..4].copy_from_slice(&[255, 255, 255, 255]);

    for row in 0..cow.height {
        let source_start = (row * cow.width * 4) as usize;
        let source_end = source_start + (cow.width * 4) as usize;
        let dest_start = ((row * width + 1) * 4) as usize;
        let dest_end = dest_start + (cow.width * 4) as usize;
        rgba[dest_start..dest_end].copy_from_slice(&cow.rgba[source_start..source_end]);
    }

    ActorTextureImage {
        width,
        height,
        rgba,
        layout: ActorTextureLayout {
            white: ActorTextureRegion {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            cow: ActorTextureRegion {
                x: 1,
                y: 0,
                width: cow.width,
                height: cow.height,
            },
        },
    }
}

#[derive(Clone, Debug)]
struct RgbaTexture {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_assets::{
        chicken_figure_id, chicken_figure_path, cow_figure_id, cow_figure_path,
        default_player_figure_id, default_player_figure_path, mallard_duck_figure_id,
        mallard_duck_figure_path, upright_bear_figure_id, upright_bear_figure_path,
    };
    use std::io::Cursor;

    fn actor_asset_test_source() -> mclone_assets::MemoryAssetSource {
        let mut source = mclone_assets::MemoryAssetSource::new();
        source.insert(AssetPath::new(COW_TEXTURE_PATH), solid_png_bytes(64, 32));
        source.insert_text(
            default_player_figure_path(),
            include_str!("../../../../assets/mclone/figures/player.figure.json"),
        );
        source.insert_text(
            upright_bear_figure_path(),
            include_str!("../../../../assets/mclone/figures/upright_bear.figure.json"),
        );
        source.insert_text(
            chicken_figure_path(),
            include_str!("../../../../assets/mclone/figures/chicken.figure.json"),
        );
        source.insert_text(
            cow_figure_path(),
            include_str!("../../../../assets/mclone/figures/cow.figure.json"),
        );
        source.insert_text(
            mallard_duck_figure_path(),
            include_str!("../../../../assets/mclone/figures/mallard_duck.figure.json"),
        );
        source.insert_text(
            mclone_assets::deer_figure_path(),
            include_str!("../../../../assets/mclone/figures/deer.figure.json"),
        );
        source.insert_text(
            mclone_assets::mallard_nest_figure_path(),
            include_str!("../../../../assets/mclone/figures/mallard_nest.figure.json"),
        );
        source.insert_text(
            mclone_assets::mallard_feather_figure_path(),
            include_str!("../../../../assets/mclone/figures/mallard_feather.figure.json"),
        );
        source
    }

    fn solid_png_bytes(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([128, 128, 128, 255]));
        let mut cursor = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, image::ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    #[test]
    fn actor_texture_atlas_stitches_white_pixel_and_cow_region() {
        let cow = RgbaTexture {
            width: COW_TEXTURE_WIDTH,
            height: COW_TEXTURE_HEIGHT,
            rgba: vec![128; (COW_TEXTURE_WIDTH * COW_TEXTURE_HEIGHT * 4) as usize],
        };

        let atlas = stitch_actor_texture_atlas(&cow);

        assert_eq!(atlas.width, 65);
        assert_eq!(atlas.height, 32);
        assert_eq!(&atlas.rgba[0..4], &[255, 255, 255, 255]);
        assert_eq!(atlas.layout().white.x, 0);
        assert_eq!(atlas.layout().cow.x, 1);
        assert_eq!(atlas.layout().cow.width, COW_TEXTURE_WIDTH);
        assert_eq!(atlas.layout().cow.height, COW_TEXTURE_HEIGHT);
    }

    #[test]
    fn actor_texture_assets_expose_default_player_figure_registry() {
        let source = actor_asset_test_source();
        let assets = load_actor_texture_assets(&source).unwrap();

        assert!(assets.figures.get(default_player_figure_id()).is_some());
        assert!(assets.figures.get(upright_bear_figure_id()).is_some());
        assert!(assets.figures.get(chicken_figure_id()).is_some());
        assert!(assets.figures.get(cow_figure_id()).is_some());
        assert!(assets.figures.get(mallard_duck_figure_id()).is_some());
        assert!(
            assets
                .figures
                .get(mclone_assets::deer_figure_id())
                .is_some()
        );
        assert!(
            assets
                .figures
                .prepared(default_player_figure_id())
                .is_some()
        );
        assert!(assets.figures.prepared(chicken_figure_id()).is_some());
        assert!(assets.figures.prepared(mallard_duck_figure_id()).is_some());
        assert!(
            assets
                .figures
                .prepared(mclone_assets::deer_figure_id())
                .is_some()
        );
        assert!(
            assets
                .figures
                .prepared(mclone_assets::mallard_nest_figure_id())
                .is_some()
        );
        assert!(
            assets
                .figures
                .prepared(mclone_assets::mallard_feather_figure_id())
                .is_some()
        );
        assert_eq!(assets.figures.len(), 8);
    }
}
