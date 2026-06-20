use anyhow::{Context, Result, bail};
use mclone_assets::{AssetPath, AssetSource, FilesystemAssetSource};
use mclone_render::entity::{ActorTextureAtlas, ActorTextureLayout, ActorTextureRegion};

use crate::render_cache::extracted_asset_root;

const COW_TEXTURE_PATH: &str = "assets/minecraft/textures/entity/cow/cow.png";
const COW_TEXTURE_WIDTH: u32 = 64;
const COW_TEXTURE_HEIGHT: u32 = 32;

#[derive(Clone, Debug)]
pub(crate) struct ActorTextureAssets {
    pub(crate) atlas: ActorTextureImage,
}

#[derive(Clone, Debug)]
pub(crate) struct ActorTextureImage {
    pub(crate) width: u32,
    pub(crate) height: u32,
    rgba: Vec<u8>,
    layout: ActorTextureLayout,
}

impl ActorTextureImage {
    pub(crate) fn as_upload(&self) -> ActorTextureAtlas<'_> {
        ActorTextureAtlas {
            width: self.width,
            height: self.height,
            rgba: &self.rgba,
            layout: self.layout,
        }
    }

    #[cfg(test)]
    pub(crate) fn layout(&self) -> ActorTextureLayout {
        self.layout
    }
}

pub(crate) fn load_actor_texture_assets() -> Result<ActorTextureAssets> {
    let root = extracted_asset_root();
    if !root.exists() {
        bail!(
            "missing extracted Minecraft assets at {}; run ./scripts/decompile-mc.sh from the repository root",
            root.display()
        );
    }

    let source = FilesystemAssetSource::new(root);
    let cow = read_rgba_texture(
        &source,
        &AssetPath::new(COW_TEXTURE_PATH),
        COW_TEXTURE_WIDTH,
        COW_TEXTURE_HEIGHT,
    )?;
    let atlas = stitch_actor_texture_atlas(&cow);

    Ok(ActorTextureAssets { atlas })
}

fn read_rgba_texture(
    source: &impl AssetSource,
    path: &AssetPath,
    expected_width: u32,
    expected_height: u32,
) -> Result<RgbaTexture> {
    let bytes = source
        .read(path)?
        .with_context(|| format!("missing actor texture {}", path.as_str()))?;
    let image = image::load_from_memory(&bytes)
        .with_context(|| format!("failed to decode actor texture {}", path.as_str()))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    if width != expected_width || height != expected_height {
        bail!(
            "actor texture {} decoded as {}x{} but expected {}x{}",
            path.as_str(),
            width,
            height,
            expected_width,
            expected_height
        );
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
        assert_eq!(atlas.layout().cow.width, 64);
        assert_eq!(atlas.layout().cow.height, 32);
    }

    #[test]
    fn loads_vanilla_cow_texture_when_extracted_assets_exist() {
        if !extracted_asset_root().exists() {
            return;
        }

        let assets = load_actor_texture_assets().unwrap();

        assert_eq!(assets.atlas.width, 65);
        assert_eq!(assets.atlas.height, 32);
        assert_eq!(assets.atlas.layout().cow.width, COW_TEXTURE_WIDTH);
        assert_eq!(assets.atlas.layout().cow.height, COW_TEXTURE_HEIGHT);
    }
}
