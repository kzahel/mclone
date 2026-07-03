use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use mclone_assets::{
    AssetError, BakedBlockModelFace, BlockModelLibrary, BlockStateAssetIndex, BlockStateRecord,
    BlockStateRegistry, BlockStateVariant, ModelFaceDirection, ResourceLocation, TextureAtlasPlan,
    TextureMaterial,
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
    pub tint: TexturedBlockTint,
    pub shade: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TexturedBlockTint {
    #[default]
    None,
    Grass,
    Foliage,
    BirchFoliage,
    EvergreenFoliage,
    LilyPad,
}

impl TexturedBlockFace {
    fn from_baked(
        face: &BakedBlockModelFace,
        atlas: &TextureAtlasPlan,
        rotation: BlockStateModelRotation,
        tint: TexturedBlockTint,
    ) -> Result<Self, TexturedMeshError> {
        let sprite = atlas
            .sprite(&face.texture)
            .ok_or_else(|| TexturedMeshError::MissingSprite(face.texture.clone()))?;
        let (from, to) = rotation.rotate_bounds(face.from, face.to);
        Ok(Self {
            direction: rotation.rotate_direction(face.direction),
            cullface: face
                .cullface
                .map(|direction| rotation.rotate_direction(direction)),
            from,
            to,
            uv: face.uv,
            uv_rotation: face.uv_rotation,
            sprite: AtlasSpriteUv {
                u0: sprite.u0,
                v0: sprite.v0,
                u1: sprite.u1,
                v1: sprite.v1,
            },
            tintindex: face.tintindex,
            tint,
            shade: face.shade,
        })
    }

    pub fn is_full_cube_side(&self) -> bool {
        face_is_full_cube_side(self)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct BlockStateModelRotation {
    x_steps: u8,
    y_steps: u8,
}

impl BlockStateModelRotation {
    fn from_variant(variant: &BlockStateVariant) -> Self {
        Self {
            x_steps: (variant.x.rem_euclid(360) / 90) as u8,
            y_steps: (variant.y.rem_euclid(360) / 90) as u8,
        }
    }

    fn rotate_direction(self, direction: ModelFaceDirection) -> ModelFaceDirection {
        let vector = match direction {
            ModelFaceDirection::Down => [0, -1, 0],
            ModelFaceDirection::Up => [0, 1, 0],
            ModelFaceDirection::North => [0, 0, -1],
            ModelFaceDirection::South => [0, 0, 1],
            ModelFaceDirection::West => [-1, 0, 0],
            ModelFaceDirection::East => [1, 0, 0],
        };
        match self.rotate_i32_vector(vector) {
            [0, -1, 0] => ModelFaceDirection::Down,
            [0, 1, 0] => ModelFaceDirection::Up,
            [0, 0, -1] => ModelFaceDirection::North,
            [0, 0, 1] => ModelFaceDirection::South,
            [-1, 0, 0] => ModelFaceDirection::West,
            [1, 0, 0] => ModelFaceDirection::East,
            other => panic!("invalid rotated face direction vector {other:?}"),
        }
    }

    fn rotate_bounds(self, from: [f32; 3], to: [f32; 3]) -> ([f32; 3], [f32; 3]) {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];

        for x in [from[0], to[0]] {
            for y in [from[1], to[1]] {
                for z in [from[2], to[2]] {
                    let rotated = self.rotate_point([x, y, z]);
                    for axis in 0..3 {
                        min[axis] = min[axis].min(rotated[axis]);
                        max[axis] = max[axis].max(rotated[axis]);
                    }
                }
            }
        }

        (min, max)
    }

    fn rotate_point(self, point: [f32; 3]) -> [f32; 3] {
        let mut vector = [point[0] - 8.0, point[1] - 8.0, point[2] - 8.0];
        for _ in 0..self.x_steps {
            vector = [vector[0], vector[2], -vector[1]];
        }
        for _ in 0..self.y_steps {
            vector = [-vector[2], vector[1], vector[0]];
        }
        [vector[0] + 8.0, vector[1] + 8.0, vector[2] + 8.0]
    }

    fn rotate_i32_vector(self, mut vector: [i32; 3]) -> [i32; 3] {
        for _ in 0..self.x_steps {
            vector = [vector[0], vector[2], -vector[1]];
        }
        for _ in 0..self.y_steps {
            vector = [-vector[2], vector[1], vector[0]];
        }
        vector
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TexturedBlockModel {
    pub faces: Vec<TexturedBlockFace>,
    pub fluid: Option<TexturedFluidModel>,
    pub render_layer: TexturedTerrainRenderLayer,
    pub occludes: bool,
    pub ambient_occlusion: bool,
    pub light_emission: u8,
    pub light_block: u8,
    pub view_blocking: bool,
    pub solid_render: bool,
    pub collision_shape_full_block: bool,
    pub shade_brightness: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TexturedTerrainRenderLayer {
    #[default]
    Solid,
    Cutout,
    Translucent,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TexturedColorMap {
    pixels: Vec<u32>,
}

impl TexturedColorMap {
    pub const WIDTH: u32 = 256;
    pub const HEIGHT: u32 = 256;
    const PIXEL_COUNT: usize = (Self::WIDTH * Self::HEIGHT) as usize;

    pub fn from_rgba(rgba: &[u8]) -> Option<Self> {
        if rgba.len() != Self::PIXEL_COUNT * 4 {
            return None;
        }
        let pixels = rgba
            .chunks_exact(4)
            .map(|pixel| ((pixel[0] as u32) << 16) | ((pixel[1] as u32) << 8) | pixel[2] as u32)
            .collect();
        Some(Self { pixels })
    }

    pub(crate) fn sample(&self, temperature: f32, downfall: f32) -> u32 {
        let temperature = temperature.clamp(0.0, 1.0) as f64;
        let downfall = (downfall.clamp(0.0, 1.0) as f64) * temperature;
        let x = ((1.0 - temperature) * 255.0) as usize;
        let y = ((1.0 - downfall) * 255.0) as usize;
        self.pixels[y * Self::WIDTH as usize + x]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TexturedColorMaps {
    pub grass: TexturedColorMap,
    pub foliage: TexturedColorMap,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TexturedMeshCatalog {
    blocks: BTreeMap<BlockStateId, TexturedBlockModel>,
    color_maps: Option<TexturedColorMaps>,
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
            let (model, rotation) = if let Some(variants) = asset.variants_for_key(&variant_key) {
                let variant = variants.first().ok_or_else(|| {
                    TexturedMeshError::MissingBlockStateVariant {
                        block: record.block.clone(),
                        variant_key: variant_key.clone(),
                    }
                })?;
                (
                    &variant.model,
                    BlockStateModelRotation::from_variant(variant),
                )
            } else if variant_key.is_empty() {
                (
                    multipart_primary_model(asset, record).ok_or_else(|| {
                        TexturedMeshError::MissingBlockStateVariant {
                            block: record.block.clone(),
                            variant_key: variant_key.clone(),
                        }
                    })?,
                    BlockStateModelRotation::default(),
                )
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
                .map(|face| {
                    TexturedBlockFace::from_baked(
                        face,
                        atlas,
                        rotation,
                        textured_block_tint(record.block.path(), face.tintindex),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let fluid = textured_fluid_model(record, atlas)?;
            let full_cube_occluder = full_cube_occluder(&faces);
            let facts = block_render_facts(record, full_cube_occluder);
            let render_layer =
                textured_terrain_render_layer(record.block.path(), facts.solid_render);
            blocks.insert(
                record.id,
                TexturedBlockModel {
                    faces,
                    fluid,
                    render_layer,
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

        Ok(Self {
            blocks,
            color_maps: None,
        })
    }

    pub fn with_color_maps(mut self, color_maps: TexturedColorMaps) -> Self {
        self.color_maps = Some(color_maps);
        self
    }

    pub fn get(&self, state_id: BlockStateId) -> Option<&TexturedBlockModel> {
        self.blocks.get(&state_id)
    }

    pub fn gui_icon_uv(&self, state_id: BlockStateId) -> Option<AtlasSpriteUv> {
        let model = self.blocks.get(&state_id)?;
        if let Some(fluid) = model.fluid {
            return Some(fluid.still);
        }
        model
            .faces
            .iter()
            .find(|face| face.direction == ModelFaceDirection::Up)
            .or_else(|| model.faces.first())
            .map(|face| face.sprite)
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

    pub(crate) fn grass_color_from_colormap(&self, temperature: f32, downfall: f32) -> Option<u32> {
        self.color_maps
            .as_ref()
            .map(|maps| maps.grass.sample(temperature, downfall))
    }

    pub(crate) fn foliage_color_from_colormap(
        &self,
        temperature: f32,
        downfall: f32,
    ) -> Option<u32> {
        self.color_maps
            .as_ref()
            .map(|maps| maps.foliage.sample(temperature, downfall))
    }
}

fn multipart_primary_model<'a>(
    asset: &'a mclone_assets::BlockStateAsset,
    record: &BlockStateRecord,
) -> Option<&'a ResourceLocation> {
    let block_path = record.block.path();
    if block_path == "bamboo" {
        if let Some(age) = record.properties.get("age") {
            let preferred_bamboo = format!("block/bamboo1_age{age}");
            if let Some(model) = asset.model_refs.iter().find(|model| {
                model.namespace() == asset.block.namespace() && model.path() == preferred_bamboo
            }) {
                return Some(model);
            }
        }
    }

    let preferred = format!("block/{block_path}");
    asset
        .model_refs
        .iter()
        .find(|model| model.namespace() == asset.block.namespace() && model.path() == preferred)
        .or_else(|| asset.model_refs.iter().next())
}

fn textured_block_tint(block_path: &str, tintindex: i32) -> TexturedBlockTint {
    if tintindex < 0 {
        return TexturedBlockTint::None;
    }
    match block_path {
        "grass_block" | "grass" | "fern" | "large_fern" | "potted_fern" | "sugar_cane" => {
            TexturedBlockTint::Grass
        }
        "oak_leaves" | "jungle_leaves" | "acacia_leaves" | "dark_oak_leaves" | "vine" => {
            TexturedBlockTint::Foliage
        }
        "birch_leaves" => TexturedBlockTint::BirchFoliage,
        "spruce_leaves" => TexturedBlockTint::EvergreenFoliage,
        "lily_pad" => TexturedBlockTint::LilyPad,
        _ => TexturedBlockTint::None,
    }
}

fn textured_terrain_render_layer(path: &str, solid_render: bool) -> TexturedTerrainRenderLayer {
    if java_translucent_block(path) {
        TexturedTerrainRenderLayer::Translucent
    } else if java_cutout_mipped_block(path) || java_cutout_block(path) {
        TexturedTerrainRenderLayer::Cutout
    } else if solid_render {
        TexturedTerrainRenderLayer::Solid
    } else {
        TexturedTerrainRenderLayer::Cutout
    }
}

// Mirrors Java 1.17.1 `ItemBlockRenderTypes.getChunkRenderType` for the
// block-render-layer distinction that decides whether alpha discard is needed.
fn java_cutout_mipped_block(path: &str) -> bool {
    path.ends_with("_leaves")
        || matches!(
            path,
            "grass_block" | "iron_bars" | "glass_pane" | "tripwire_hook" | "hopper" | "chain"
        )
}

fn java_cutout_block(path: &str) -> bool {
    matches!(
        path,
        "oak_sapling"
            | "spruce_sapling"
            | "birch_sapling"
            | "jungle_sapling"
            | "acacia_sapling"
            | "dark_oak_sapling"
            | "glass"
            | "powered_rail"
            | "detector_rail"
            | "cobweb"
            | "grass"
            | "fern"
            | "dead_bush"
            | "seagrass"
            | "tall_seagrass"
            | "dandelion"
            | "poppy"
            | "blue_orchid"
            | "allium"
            | "azure_bluet"
            | "red_tulip"
            | "orange_tulip"
            | "white_tulip"
            | "pink_tulip"
            | "oxeye_daisy"
            | "cornflower"
            | "wither_rose"
            | "lily_of_the_valley"
            | "brown_mushroom"
            | "red_mushroom"
            | "torch"
            | "wall_torch"
            | "soul_torch"
            | "soul_wall_torch"
            | "fire"
            | "soul_fire"
            | "spawner"
            | "redstone_wire"
            | "wheat"
            | "oak_door"
            | "ladder"
            | "rail"
            | "iron_door"
            | "redstone_torch"
            | "redstone_wall_torch"
            | "cactus"
            | "sugar_cane"
            | "repeater"
            | "oak_trapdoor"
            | "spruce_trapdoor"
            | "birch_trapdoor"
            | "jungle_trapdoor"
            | "acacia_trapdoor"
            | "dark_oak_trapdoor"
            | "crimson_trapdoor"
            | "warped_trapdoor"
            | "attached_pumpkin_stem"
            | "attached_melon_stem"
            | "pumpkin_stem"
            | "melon_stem"
            | "vine"
            | "glow_lichen"
            | "lily_pad"
            | "nether_wart"
            | "brewing_stand"
            | "cocoa"
            | "beacon"
            | "flower_pot"
            | "potted_oak_sapling"
            | "potted_spruce_sapling"
            | "potted_birch_sapling"
            | "potted_jungle_sapling"
            | "potted_acacia_sapling"
            | "potted_dark_oak_sapling"
            | "potted_fern"
            | "potted_dandelion"
            | "potted_poppy"
            | "potted_blue_orchid"
            | "potted_allium"
            | "potted_azure_bluet"
            | "potted_red_tulip"
            | "potted_orange_tulip"
            | "potted_white_tulip"
            | "potted_pink_tulip"
            | "potted_oxeye_daisy"
            | "potted_cornflower"
            | "potted_lily_of_the_valley"
            | "potted_wither_rose"
            | "potted_red_mushroom"
            | "potted_brown_mushroom"
            | "potted_dead_bush"
            | "potted_cactus"
            | "potted_azalea"
            | "potted_flowering_azalea"
            | "carrots"
            | "potatoes"
            | "comparator"
            | "activator_rail"
            | "iron_trapdoor"
            | "sunflower"
            | "lilac"
            | "rose_bush"
            | "peony"
            | "tall_grass"
            | "large_fern"
            | "spruce_door"
            | "birch_door"
            | "jungle_door"
            | "acacia_door"
            | "dark_oak_door"
            | "end_rod"
            | "chorus_plant"
            | "chorus_flower"
            | "beetroots"
            | "kelp"
            | "kelp_plant"
            | "turtle_egg"
            | "sea_pickle"
            | "conduit"
            | "bamboo_sapling"
            | "bamboo"
            | "potted_bamboo"
            | "scaffolding"
            | "stonecutter"
            | "lantern"
            | "soul_lantern"
            | "campfire"
            | "soul_campfire"
            | "sweet_berry_bush"
            | "weeping_vines"
            | "weeping_vines_plant"
            | "twisting_vines"
            | "twisting_vines_plant"
            | "nether_sprouts"
            | "crimson_fungus"
            | "warped_fungus"
            | "crimson_roots"
            | "warped_roots"
            | "potted_crimson_fungus"
            | "potted_warped_fungus"
            | "potted_crimson_roots"
            | "potted_warped_roots"
            | "crimson_door"
            | "warped_door"
            | "pointed_dripstone"
            | "small_amethyst_bud"
            | "medium_amethyst_bud"
            | "large_amethyst_bud"
            | "amethyst_cluster"
            | "lightning_rod"
            | "cave_vines"
            | "cave_vines_plant"
            | "spore_blossom"
            | "flowering_azalea"
            | "azalea"
            | "moss_carpet"
            | "big_dripleaf"
            | "big_dripleaf_stem"
            | "small_dripleaf"
            | "hanging_roots"
            | "sculk_sensor"
    ) || path.ends_with("_bed")
        || path.ends_with("_coral")
        || path.ends_with("_coral_fan")
        || path.ends_with("_coral_wall_fan")
}

fn java_translucent_block(path: &str) -> bool {
    path.ends_with("_stained_glass")
        || path.ends_with("_stained_glass_pane")
        || matches!(
            path,
            "ice"
                | "nether_portal"
                | "slime_block"
                | "honey_block"
                | "frosted_ice"
                | "bubble_column"
                | "tinted_glass"
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_map_samples_java_temperature_downfall_index() {
        let mut rgba = vec![0; TexturedColorMap::PIXEL_COUNT * 4];
        let index = 173 * TexturedColorMap::WIDTH as usize + 50;
        rgba[index * 4..index * 4 + 4].copy_from_slice(&[1, 2, 3, 255]);
        let map = TexturedColorMap::from_rgba(&rgba).unwrap();

        assert_eq!(map.sample(0.8, 0.4), 0x01_02_03);
    }

    #[test]
    fn color_map_rejects_non_vanilla_dimensions() {
        assert!(TexturedColorMap::from_rgba(&[0; 4]).is_none());
    }

    #[test]
    fn terrain_render_layer_keeps_alpha_test_full_cube_exceptions_out_of_solid() {
        assert_eq!(
            textured_terrain_render_layer("stone", true),
            TexturedTerrainRenderLayer::Solid
        );
        assert_eq!(
            textured_terrain_render_layer("grass_block", true),
            TexturedTerrainRenderLayer::Cutout
        );
        assert_eq!(
            textured_terrain_render_layer("oak_leaves", false),
            TexturedTerrainRenderLayer::Cutout
        );
        assert_eq!(
            textured_terrain_render_layer("tinted_glass", false),
            TexturedTerrainRenderLayer::Translucent
        );
    }

    #[test]
    fn textured_block_tint_classifies_vanilla_tintindex_blocks() {
        assert_eq!(
            textured_block_tint("lily_pad", 0),
            TexturedBlockTint::LilyPad
        );
        assert_eq!(textured_block_tint("lily_pad", -1), TexturedBlockTint::None);
    }

    #[test]
    fn multipart_primary_model_prefers_matching_block_model() {
        let block = ResourceLocation::parse("minecraft:red_mushroom_block").unwrap();
        let record = mclone_assets::BlockStateRecord::new(
            BlockStateId(0),
            block.clone(),
            [] as [(&str, &str); 0],
        );
        let asset = mclone_assets::BlockStateAsset {
            block: block.clone(),
            path: mclone_assets::AssetPath::blockstate_json(&block),
            variants: BTreeMap::new(),
            variant_keys: BTreeSet::new(),
            model_refs: [
                ResourceLocation::parse("minecraft:block/mushroom_block_inside").unwrap(),
                ResourceLocation::parse("minecraft:block/red_mushroom_block").unwrap(),
            ]
            .into_iter()
            .collect(),
        };

        assert_eq!(
            multipart_primary_model(&asset, &record).map(ResourceLocation::path),
            Some("block/red_mushroom_block")
        );
    }

    #[test]
    fn multipart_primary_model_prefers_bamboo_age_stem() {
        let block = ResourceLocation::parse("minecraft:bamboo").unwrap();
        let record = mclone_assets::BlockStateRecord::new(
            BlockStateId(130),
            block.clone(),
            [("age", "1"), ("leaves", "none"), ("stage", "0")],
        );
        let asset = mclone_assets::BlockStateAsset {
            block: block.clone(),
            path: mclone_assets::AssetPath::blockstate_json(&block),
            variants: BTreeMap::new(),
            variant_keys: BTreeSet::new(),
            model_refs: [
                ResourceLocation::parse("minecraft:block/bamboo1_age0").unwrap(),
                ResourceLocation::parse("minecraft:block/bamboo1_age1").unwrap(),
                ResourceLocation::parse("minecraft:block/bamboo_large_leaves").unwrap(),
            ]
            .into_iter()
            .collect(),
        };

        assert_eq!(
            multipart_primary_model(&asset, &record).map(ResourceLocation::path),
            Some("block/bamboo1_age1")
        );
    }
}
