use std::collections::{BTreeMap, BTreeSet};

use crate::{AssetError, AssetPath, AssetResult, AssetSource, TextureMaterial};

const DEFAULT_MAX_ATLAS_SIZE: u32 = 4096;
const DEFAULT_MIP_LEVEL: u32 = 4;
// Java 1.17.1 generates mips per sprite and aligns atlas slots with
// Stitcher.smallestFittingMinTexel. mclone generates a portable full-atlas mip
// chain, so each sprite reserves enough extruded edge texels to keep level 4
// from blending neighboring sprites into terrain samples.
const DEFAULT_SPRITE_GUTTER: u32 = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextureSpriteInfo {
    pub material: TextureMaterial,
    pub path: AssetPath,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextureAtlasSprite {
    pub info: TextureSpriteInfo,
    pub x: u32,
    pub y: u32,
    pub gutter: u32,
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

impl TextureAtlasSprite {
    pub fn padded_x(&self) -> u32 {
        self.x - self.gutter
    }

    pub fn padded_y(&self) -> u32 {
        self.y - self.gutter
    }

    pub fn padded_width(&self) -> u32 {
        padded_sprite_dimension(self.info.width)
    }

    pub fn padded_height(&self) -> u32 {
        padded_sprite_dimension(self.info.height)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextureAtlasPlan {
    width: u32,
    height: u32,
    sprites: BTreeMap<TextureMaterial, TextureAtlasSprite>,
}

impl TextureAtlasPlan {
    pub fn build(
        source: &impl AssetSource,
        materials: impl IntoIterator<Item = TextureMaterial>,
    ) -> AssetResult<Self> {
        Self::build_with_max_size(source, materials, DEFAULT_MAX_ATLAS_SIZE)
    }

    pub fn build_with_max_size(
        source: &impl AssetSource,
        materials: impl IntoIterator<Item = TextureMaterial>,
        max_size: u32,
    ) -> AssetResult<Self> {
        let materials = materials.into_iter().collect::<BTreeSet<_>>();
        if materials.is_empty() {
            return Ok(Self::default());
        }

        let mut infos = materials
            .into_iter()
            .map(|material| TextureSpriteInfo::load(source, material))
            .collect::<AssetResult<Vec<_>>>()?;
        infos.sort_by(|left, right| {
            right
                .height
                .cmp(&left.height)
                .then_with(|| right.width.cmp(&left.width))
                .then_with(|| left.material.cmp(&right.material))
        });

        let max_width = infos
            .iter()
            .map(TextureSpriteInfo::padded_width)
            .max()
            .unwrap_or(0);
        let target_width =
            smallest_encompassing_power_of_two(max_width * ceil_sqrt(infos.len() as u32).max(1));
        if target_width > max_size {
            return Err(AssetError::InvalidTexture(format!(
                "atlas width {target_width} exceeds maximum {max_size}"
            )));
        }

        let mut x = 0;
        let mut y = 0;
        let mut row_height = 0;
        let mut placed = Vec::with_capacity(infos.len());
        for info in infos {
            let padded_width = info.padded_width();
            let padded_height = info.padded_height();
            if padded_width > max_size || padded_height > max_size {
                return Err(AssetError::InvalidTexture(format!(
                    "{} is {}x{} with padding, larger than atlas maximum {max_size}",
                    info.path, padded_width, padded_height
                )));
            }
            if x > 0 && x + padded_width > target_width {
                y += row_height;
                x = 0;
                row_height = 0;
            }
            placed.push((info, x, y));
            let current = placed.last().expect("placed sprite was just pushed");
            x += current.0.padded_width();
            row_height = row_height.max(current.0.padded_height());
        }

        let height = smallest_encompassing_power_of_two(y + row_height);
        if height > max_size {
            return Err(AssetError::InvalidTexture(format!(
                "atlas height {height} exceeds maximum {max_size}"
            )));
        }

        let mut sprites = BTreeMap::new();
        for (info, x, y) in placed {
            let sprite_x = x + DEFAULT_SPRITE_GUTTER;
            let sprite_y = y + DEFAULT_SPRITE_GUTTER;
            let sprite = TextureAtlasSprite {
                u0: sprite_x as f32 / target_width as f32,
                v0: sprite_y as f32 / height as f32,
                u1: (sprite_x + info.width) as f32 / target_width as f32,
                v1: (sprite_y + info.height) as f32 / height as f32,
                x: sprite_x,
                y: sprite_y,
                gutter: DEFAULT_SPRITE_GUTTER,
                info,
            };
            sprites.insert(sprite.info.material.clone(), sprite);
        }

        Ok(Self {
            width: target_width,
            height,
            sprites,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn len(&self) -> usize {
        self.sprites.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sprites.is_empty()
    }

    pub fn sprite(&self, material: &TextureMaterial) -> Option<&TextureAtlasSprite> {
        self.sprites.get(material)
    }

    pub fn sprites(&self) -> impl Iterator<Item = &TextureAtlasSprite> {
        self.sprites.values()
    }
}

impl TextureSpriteInfo {
    pub fn load(source: &impl AssetSource, material: TextureMaterial) -> AssetResult<Self> {
        let path = AssetPath::texture_png(&material.texture);
        let bytes = source
            .read(&path)?
            .ok_or_else(|| AssetError::MissingAsset(path.clone()))?;
        let (width, height) = png_dimensions(&path, &bytes)?;
        Ok(Self {
            material,
            path,
            width,
            height,
        })
    }
}

impl TextureSpriteInfo {
    fn padded_width(&self) -> u32 {
        padded_sprite_dimension(self.width)
    }

    fn padded_height(&self) -> u32 {
        padded_sprite_dimension(self.height)
    }
}

fn padded_sprite_dimension(sprite_dimension: u32) -> u32 {
    smallest_fitting_min_texel(
        sprite_dimension + DEFAULT_SPRITE_GUTTER * 2,
        DEFAULT_MIP_LEVEL,
    )
}

fn png_dimensions(path: &AssetPath, bytes: &[u8]) -> AssetResult<(u32, u32)> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[0..8] != PNG_SIGNATURE || &bytes[12..16] != b"IHDR" {
        return Err(AssetError::InvalidTexture(format!(
            "{} is not a PNG with an IHDR header",
            path.as_str()
        )));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("slice has four bytes"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("slice has four bytes"));
    if width == 0 || height == 0 {
        return Err(AssetError::InvalidTexture(format!(
            "{} has invalid dimensions {}x{}",
            path.as_str(),
            width,
            height
        )));
    }
    Ok((width, height))
}

fn smallest_encompassing_power_of_two(value: u32) -> u32 {
    value.checked_next_power_of_two().unwrap_or(value)
}

fn ceil_sqrt(value: u32) -> u32 {
    let mut root = 0;
    while root * root < value {
        root += 1;
    }
    root
}

fn smallest_fitting_min_texel(dimension: u32, mip_level: u32) -> u32 {
    let alignment = 1 << mip_level;
    let remainder = dimension & (alignment - 1);
    ((dimension >> mip_level) + u32::from(remainder != 0)) << mip_level
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryAssetSource, ResourceLocation};

    #[test]
    fn atlas_plan_places_png_textures_with_normalized_uvs() {
        let material =
            TextureMaterial::blocks(ResourceLocation::parse("minecraft:block/stone").unwrap());
        let mut source = MemoryAssetSource::new();
        source.insert(
            AssetPath::new("assets/minecraft/textures/block/stone.png"),
            png_header(16, 16),
        );

        let plan = TextureAtlasPlan::build(&source, [material.clone()]).unwrap();
        let sprite = plan.sprite(&material).unwrap();

        assert_eq!(plan.width(), 32);
        assert_eq!(plan.height(), 32);
        assert_eq!(sprite.x, 8);
        assert_eq!(sprite.y, 8);
        assert_eq!(sprite.gutter, 8);
        assert_eq!(sprite.padded_x(), 0);
        assert_eq!(sprite.padded_y(), 0);
        assert_eq!(sprite.padded_width(), 32);
        assert_eq!(sprite.padded_height(), 32);
        assert_eq!(sprite.u0, 0.25);
        assert_eq!(sprite.v0, 0.25);
        assert_eq!(sprite.u1, 0.75);
        assert_eq!(sprite.v1, 0.75);
    }

    #[test]
    fn atlas_plan_deduplicates_materials() {
        let material =
            TextureMaterial::blocks(ResourceLocation::parse("minecraft:block/stone").unwrap());
        let mut source = MemoryAssetSource::new();
        source.insert(
            AssetPath::new("assets/minecraft/textures/block/stone.png"),
            png_header(16, 16),
        );

        let plan = TextureAtlasPlan::build(&source, [material.clone(), material]).unwrap();

        assert_eq!(plan.len(), 1);
    }

    #[test]
    fn atlas_plan_reserves_gutters_between_sprites() {
        let left = TextureMaterial::blocks(ResourceLocation::parse("minecraft:block/a").unwrap());
        let right = TextureMaterial::blocks(ResourceLocation::parse("minecraft:block/b").unwrap());
        let mut source = MemoryAssetSource::new();
        source.insert(
            AssetPath::new("assets/minecraft/textures/block/a.png"),
            png_header(16, 16),
        );
        source.insert(
            AssetPath::new("assets/minecraft/textures/block/b.png"),
            png_header(16, 16),
        );

        let plan = TextureAtlasPlan::build(&source, [left.clone(), right.clone()]).unwrap();
        let left = plan.sprite(&left).unwrap();
        let right = plan.sprite(&right).unwrap();

        assert_eq!(plan.width(), 64);
        assert_eq!(plan.height(), 32);
        assert_eq!(left.padded_x(), 0);
        assert_eq!(right.padded_x(), 32);
        assert_eq!(
            right.x - (left.x + left.info.width),
            left.gutter + right.gutter
        );
    }

    #[test]
    fn atlas_plan_aligns_padded_slots_to_default_mip_level() {
        let material =
            TextureMaterial::blocks(ResourceLocation::parse("minecraft:block/odd").unwrap());
        let mut source = MemoryAssetSource::new();
        source.insert(
            AssetPath::new("assets/minecraft/textures/block/odd.png"),
            png_header(18, 18),
        );

        let plan = TextureAtlasPlan::build(&source, [material.clone()]).unwrap();
        let sprite = plan.sprite(&material).unwrap();

        assert_eq!(sprite.padded_width(), 48);
        assert_eq!(sprite.padded_height(), 48);
        assert_eq!(sprite.x, 8);
        assert_eq!(sprite.y, 8);
    }

    #[test]
    fn smallest_fitting_min_texel_matches_java_stitcher_alignment() {
        assert_eq!(smallest_fitting_min_texel(16, 4), 16);
        assert_eq!(smallest_fitting_min_texel(17, 4), 32);
        assert_eq!(smallest_fitting_min_texel(32, 4), 32);
        assert_eq!(smallest_fitting_min_texel(33, 4), 48);
    }

    pub(super) fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes
    }
}
