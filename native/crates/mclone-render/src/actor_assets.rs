use std::error::Error;
use std::fmt;

use mclone_assets::{AssetError, AssetPath, AssetSource, default_player_figure_path};

use crate::asset_lab_figure::{CompiledFigure, load_compiled_player_figure};
use crate::entity::{ActorTextureAtlas, ActorTextureLayout, ActorTextureRegion};

const COW_TEXTURE_PATH: &str = "assets/minecraft/textures/entity/cow/cow.png";
const COW_TEXTURE_WIDTH: u32 = 64;
const COW_TEXTURE_HEIGHT: u32 = 32;

#[derive(Clone, Debug)]
pub struct ActorTextureAssets {
    pub atlas: ActorTextureImage,
    pub player_figure: CompiledFigure,
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
    let figure_path = default_player_figure_path();
    let player_figure =
        load_compiled_player_figure(source).map_err(|source| ActorTextureAssetError::Figure {
            path: figure_path,
            source,
        })?;

    Ok(ActorTextureAssets {
        atlas,
        player_figure,
    })
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
    Figure {
        path: AssetPath,
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
            Self::Figure { path, source } => {
                write!(f, "failed to load actor figure {}: {source}", path.as_str())
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
            Self::Figure { source, .. } => Some(source.as_ref()),
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
}
