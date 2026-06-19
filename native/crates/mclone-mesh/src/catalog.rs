use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use mclone_assets::{
    AssetError, BakedBlockModelFace, BlockModelLibrary, BlockStateAssetIndex, BlockStateRegistry,
    ModelFaceDirection, ResourceLocation, TextureAtlasPlan, TextureMaterial,
};
use mclone_core::BlockStateId;

use crate::render_facts::block_render_facts;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtlasSpriteUv {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

impl AtlasSpriteUv {
    pub(crate) fn map(self, u: f32, v: f32) -> [f32; 2] {
        [
            self.u0 + (self.u1 - self.u0) * (u / 16.0),
            self.v0 + (self.v1 - self.v0) * (v / 16.0),
        ]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TexturedBlockFace {
    pub direction: ModelFaceDirection,
    pub cullface: Option<ModelFaceDirection>,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub uv: [f32; 4],
    pub uv_rotation: i32,
    pub sprite: AtlasSpriteUv,
    pub tintindex: i32,
    pub shade: bool,
}

impl TexturedBlockFace {
    fn from_baked(
        face: &BakedBlockModelFace,
        atlas: &TextureAtlasPlan,
    ) -> Result<Self, TexturedMeshError> {
        let sprite = atlas
            .sprite(&face.texture)
            .ok_or_else(|| TexturedMeshError::MissingSprite(face.texture.clone()))?;
        Ok(Self {
            direction: face.direction,
            cullface: face.cullface,
            from: face.from,
            to: face.to,
            uv: face.uv,
            uv_rotation: face.uv_rotation,
            sprite: AtlasSpriteUv {
                u0: sprite.u0,
                v0: sprite.v0,
                u1: sprite.u1,
                v1: sprite.v1,
            },
            tintindex: face.tintindex,
            shade: face.shade,
        })
    }

    pub fn is_full_cube_side(&self) -> bool {
        face_is_full_cube_side(self)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TexturedBlockModel {
    pub faces: Vec<TexturedBlockFace>,
    pub fluid: Option<TexturedFluidModel>,
    pub occludes: bool,
    pub ambient_occlusion: bool,
    pub light_emission: u8,
    pub light_block: u8,
    pub view_blocking: bool,
    pub solid_render: bool,
    pub collision_shape_full_block: bool,
    pub shade_brightness: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TexturedFluidKind {
    Water,
    Lava,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexturedFluidModel {
    pub kind: TexturedFluidKind,
    pub level: u8,
    pub still: AtlasSpriteUv,
    pub flow: AtlasSpriteUv,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TexturedMeshCatalog {
    blocks: BTreeMap<BlockStateId, TexturedBlockModel>,
}

impl TexturedMeshCatalog {
    pub fn from_assets(
        registry: &BlockStateRegistry,
        blockstates: &BlockStateAssetIndex,
        models: &BlockModelLibrary,
        atlas: &TextureAtlasPlan,
    ) -> Result<Self, TexturedMeshError> {
        let mut blocks = BTreeMap::new();
        for record in registry.records() {
            let asset = blockstates
                .get(&record.block)
                .ok_or_else(|| TexturedMeshError::MissingBlockStateAsset(record.block.clone()))?;
            let variant_key = record
                .asset_variant_key(asset)
                .unwrap_or_else(|| record.variant_key());
            let model = if let Some(variants) = asset.variants_for_key(&variant_key) {
                variants
                    .first()
                    .map(|variant| &variant.model)
                    .ok_or_else(|| TexturedMeshError::MissingBlockStateVariant {
                        block: record.block.clone(),
                        variant_key: variant_key.clone(),
                    })?
            } else if variant_key.is_empty() {
                asset.model_refs.iter().next().ok_or_else(|| {
                    TexturedMeshError::MissingBlockStateVariant {
                        block: record.block.clone(),
                        variant_key: variant_key.clone(),
                    }
                })?
            } else {
                return Err(TexturedMeshError::MissingBlockStateVariant {
                    block: record.block.clone(),
                    variant_key,
                });
            };
            let baked = models.bake_model(model)?;
            let faces = baked
                .faces
                .iter()
                .map(|face| TexturedBlockFace::from_baked(face, atlas))
                .collect::<Result<Vec<_>, _>>()?;
            let fluid = textured_fluid_model(record, atlas)?;
            let full_cube_occluder = full_cube_occluder(&faces);
            let facts = block_render_facts(record, full_cube_occluder);
            blocks.insert(
                record.id,
                TexturedBlockModel {
                    faces,
                    fluid,
                    occludes: facts.occludes,
                    ambient_occlusion: baked.ambient_occlusion,
                    light_emission: facts.light_emission,
                    light_block: facts.light_block,
                    view_blocking: facts.view_blocking,
                    solid_render: facts.solid_render,
                    collision_shape_full_block: facts.collision_shape_full_block,
                    shade_brightness: facts.shade_brightness,
                },
            );
        }

        Ok(Self { blocks })
    }

    pub fn get(&self, state_id: BlockStateId) -> Option<&TexturedBlockModel> {
        self.blocks.get(&state_id)
    }

    pub fn occludes(&self, state_id: BlockStateId) -> bool {
        self.blocks
            .get(&state_id)
            .map(|model| model.occludes)
            .unwrap_or(false)
    }

    pub(crate) fn light_block(&self, state_id: BlockStateId) -> u8 {
        self.blocks
            .get(&state_id)
            .map(|model| model.light_block)
            .unwrap_or(0)
    }

    pub(crate) fn view_blocking(&self, state_id: BlockStateId) -> bool {
        self.blocks
            .get(&state_id)
            .map(|model| model.view_blocking)
            .unwrap_or(false)
    }

    pub(crate) fn solid_render(&self, state_id: BlockStateId) -> bool {
        self.blocks
            .get(&state_id)
            .map(|model| model.solid_render)
            .unwrap_or(false)
    }

    pub(crate) fn shade_brightness(&self, state_id: BlockStateId) -> f32 {
        self.blocks
            .get(&state_id)
            .map(|model| model.shade_brightness)
            .unwrap_or(1.0)
    }

    pub(crate) fn fluid(&self, state_id: BlockStateId) -> Option<TexturedFluidModel> {
        self.blocks.get(&state_id).and_then(|model| model.fluid)
    }
}

#[derive(Debug)]
pub enum TexturedMeshError {
    Asset(AssetError),
    InvalidFluidLevel {
        state: String,
        level: String,
    },
    MissingBlockStateAsset(ResourceLocation),
    MissingBlockStateVariant {
        block: ResourceLocation,
        variant_key: String,
    },
    MissingBlockModel(BlockStateId),
    MissingSprite(TextureMaterial),
}

impl fmt::Display for TexturedMeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(error) => write!(f, "{error}"),
            Self::InvalidFluidLevel { state, level } => {
                write!(f, "invalid fluid level `{level}` for {state}")
            }
            Self::MissingBlockStateAsset(block) => {
                write!(f, "missing blockstate asset for {block}")
            }
            Self::MissingBlockStateVariant { block, variant_key } => {
                write!(f, "missing blockstate variant `{variant_key}` for {block}")
            }
            Self::MissingBlockModel(state_id) => {
                write!(
                    f,
                    "missing textured block model for state id {}",
                    state_id.0
                )
            }
            Self::MissingSprite(material) => {
                write!(
                    f,
                    "missing atlas sprite for material {} in {}",
                    material.texture, material.atlas
                )
            }
        }
    }
}

impl Error for TexturedMeshError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Asset(error) => Some(error),
            _ => None,
        }
    }
}

impl From<AssetError> for TexturedMeshError {
    fn from(value: AssetError) -> Self {
        Self::Asset(value)
    }
}

fn full_cube_occluder(faces: &[TexturedBlockFace]) -> bool {
    let mut covered = BTreeSet::new();
    for face in faces {
        if face.cullface == Some(face.direction) && face_is_full_cube_side(face) {
            covered.insert(face.direction);
        }
    }
    covered.len() == 6
}

fn textured_fluid_model(
    record: &mclone_assets::BlockStateRecord,
    atlas: &TextureAtlasPlan,
) -> Result<Option<TexturedFluidModel>, TexturedMeshError> {
    let Some(kind) = textured_fluid_kind(record) else {
        return Ok(None);
    };
    let level = record
        .properties
        .get("level")
        .map(String::as_str)
        .unwrap_or("0");
    let level = level
        .parse::<u8>()
        .ok()
        .filter(|level| *level <= 8)
        .ok_or_else(|| TexturedMeshError::InvalidFluidLevel {
            state: record.canonical_key(),
            level: level.to_owned(),
        })?;
    Ok(Some(TexturedFluidModel {
        kind,
        level,
        still: fluid_sprite(kind, FluidSpriteRole::Still, atlas)?,
        flow: fluid_sprite(kind, FluidSpriteRole::Flow, atlas)?,
    }))
}

fn textured_fluid_kind(record: &mclone_assets::BlockStateRecord) -> Option<TexturedFluidKind> {
    if record.block.namespace() != "minecraft" {
        return None;
    }
    match record.block.path() {
        "water" => Some(TexturedFluidKind::Water),
        "lava" => Some(TexturedFluidKind::Lava),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug)]
enum FluidSpriteRole {
    Still,
    Flow,
}

fn fluid_sprite(
    kind: TexturedFluidKind,
    role: FluidSpriteRole,
    atlas: &TextureAtlasPlan,
) -> Result<AtlasSpriteUv, TexturedMeshError> {
    let material = TextureMaterial::blocks(fluid_sprite_location(kind, role));
    let sprite = atlas
        .sprite(&material)
        .ok_or_else(|| TexturedMeshError::MissingSprite(material.clone()))?;
    Ok(AtlasSpriteUv {
        u0: sprite.u0,
        v0: sprite.v0,
        u1: sprite.u1,
        v1: sprite.v1,
    })
}

fn fluid_sprite_location(kind: TexturedFluidKind, role: FluidSpriteRole) -> ResourceLocation {
    let path = match (kind, role) {
        (TexturedFluidKind::Water, FluidSpriteRole::Still) => "minecraft:block/water_still",
        (TexturedFluidKind::Water, FluidSpriteRole::Flow) => "minecraft:block/water_flow",
        (TexturedFluidKind::Lava, FluidSpriteRole::Still) => "minecraft:block/lava_still",
        (TexturedFluidKind::Lava, FluidSpriteRole::Flow) => "minecraft:block/lava_flow",
    };
    ResourceLocation::parse(path).expect("fluid sprite locations are valid")
}

fn face_is_full_cube_side(face: &TexturedBlockFace) -> bool {
    const EPSILON: f32 = 0.0001;
    let full_bounds = face.from.iter().all(|value| value.abs() <= EPSILON)
        && face.to.iter().all(|value| (*value - 16.0).abs() <= EPSILON);
    if !full_bounds {
        return false;
    }
    match face.direction {
        ModelFaceDirection::Down => (face.from[1] - 0.0).abs() <= EPSILON,
        ModelFaceDirection::Up => (face.to[1] - 16.0).abs() <= EPSILON,
        ModelFaceDirection::North => (face.from[2] - 0.0).abs() <= EPSILON,
        ModelFaceDirection::South => (face.to[2] - 16.0).abs() <= EPSILON,
        ModelFaceDirection::West => (face.from[0] - 0.0).abs() <= EPSILON,
        ModelFaceDirection::East => (face.to[0] - 16.0).abs() <= EPSILON,
    }
}
