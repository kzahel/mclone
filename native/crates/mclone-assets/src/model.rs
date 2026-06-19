use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Deserialize;

use crate::{
    AssetError, AssetPath, AssetResult, AssetSource, BlockStateAssetIndex, ResourceLocation,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextureMaterial {
    pub atlas: ResourceLocation,
    pub texture: ResourceLocation,
}

impl TextureMaterial {
    pub fn blocks(texture: ResourceLocation) -> Self {
        Self {
            atlas: ResourceLocation::parse("minecraft:textures/atlas/blocks.png")
                .expect("block atlas location is valid"),
            texture,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextureReference {
    Direct(TextureMaterial),
    Reference(String),
}

impl TextureReference {
    fn parse(value: &str) -> AssetResult<Self> {
        if let Some(reference) = value.strip_prefix('#') {
            if reference.is_empty() {
                return Err(AssetError::InvalidModel(
                    "texture reference cannot be empty".to_owned(),
                ));
            }
            Ok(Self::Reference(reference.to_owned()))
        } else {
            Ok(Self::Direct(TextureMaterial::blocks(
                ResourceLocation::parse(value)?,
            )))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ModelFaceDirection {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl ModelFaceDirection {
    pub fn parse(value: &str) -> AssetResult<Self> {
        match value {
            "down" => Ok(Self::Down),
            "up" => Ok(Self::Up),
            "north" => Ok(Self::North),
            "south" => Ok(Self::South),
            "west" => Ok(Self::West),
            "east" => Ok(Self::East),
            _ => Err(AssetError::InvalidModel(format!(
                "unknown block model face direction `{value}`"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Down => "down",
            Self::Up => "up",
            Self::North => "north",
            Self::South => "south",
            Self::West => "west",
            Self::East => "east",
        }
    }
}

impl fmt::Display for ModelFaceDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockModelElement {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub faces: BTreeMap<ModelFaceDirection, BlockModelFace>,
    pub shade: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockModelFace {
    pub texture: String,
    pub cullface: Option<ModelFaceDirection>,
    pub tintindex: i32,
    pub uv: [f32; 4],
    pub uv_rotation: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockModel {
    pub location: ResourceLocation,
    pub parent: Option<ResourceLocation>,
    pub textures: BTreeMap<String, TextureReference>,
    pub elements: Vec<BlockModelElement>,
    pub ambient_occlusion: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BakedBlockModel {
    pub location: ResourceLocation,
    pub particle: Option<TextureMaterial>,
    pub faces: Vec<BakedBlockModelFace>,
    pub ambient_occlusion: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BakedBlockModelFace {
    pub direction: ModelFaceDirection,
    pub cullface: Option<ModelFaceDirection>,
    pub texture: TextureMaterial,
    pub tintindex: i32,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub uv: [f32; 4],
    pub uv_rotation: i32,
    pub shade: bool,
}

impl BlockModel {
    pub fn load(source: &impl AssetSource, location: ResourceLocation) -> AssetResult<Self> {
        let path = AssetPath::model_json(&location);
        let bytes = source
            .read(&path)?
            .ok_or_else(|| AssetError::MissingAsset(path.clone()))?;
        Self::from_json_bytes(location, &path, &bytes)
    }

    pub fn from_json_bytes(
        location: ResourceLocation,
        path: &AssetPath,
        bytes: &[u8],
    ) -> AssetResult<Self> {
        let raw: RawBlockModel =
            serde_json::from_slice(bytes).map_err(|source| AssetError::Json {
                path: path.clone(),
                source,
            })?;
        let parent = raw
            .parent
            .as_deref()
            .map(ResourceLocation::parse)
            .transpose()?;
        let textures = raw
            .textures
            .unwrap_or_default()
            .into_iter()
            .map(|(slot, value)| TextureReference::parse(&value).map(|reference| (slot, reference)))
            .collect::<AssetResult<BTreeMap<_, _>>>()?;
        let elements = raw
            .elements
            .unwrap_or_default()
            .into_iter()
            .map(BlockModelElement::try_from)
            .collect::<AssetResult<Vec<_>>>()?;

        Ok(Self {
            location,
            parent,
            textures,
            elements,
            ambient_occlusion: raw.ambient_occlusion,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct BlockModelLibrary {
    models: BTreeMap<ResourceLocation, BlockModel>,
}

impl BlockModelLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_for_blockstates(
        source: &impl AssetSource,
        blockstates: &BlockStateAssetIndex,
    ) -> AssetResult<Self> {
        Self::load_model_tree(
            source,
            blockstates
                .assets()
                .flat_map(|asset| asset.model_refs.iter().cloned()),
        )
    }

    pub fn load_model_tree(
        source: &impl AssetSource,
        roots: impl IntoIterator<Item = ResourceLocation>,
    ) -> AssetResult<Self> {
        let mut library = Self::new();
        for root in roots {
            library.load_recursive(source, root, &mut Vec::new())?;
        }
        Ok(library)
    }

    pub fn get(&self, location: &ResourceLocation) -> Option<&BlockModel> {
        self.models.get(location)
    }

    pub fn len(&self) -> usize {
        self.models.len()
    }

    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    pub fn models(&self) -> impl Iterator<Item = &BlockModel> {
        self.models.values()
    }

    pub fn effective_elements(
        &self,
        location: &ResourceLocation,
    ) -> AssetResult<&[BlockModelElement]> {
        let model = self.require_model(location)?;
        if !model.elements.is_empty() {
            return Ok(&model.elements);
        }
        if let Some(parent) = &model.parent {
            self.effective_elements(parent)
        } else {
            Ok(&[])
        }
    }

    pub fn resolve_texture(
        &self,
        location: &ResourceLocation,
        slot_or_reference: &str,
    ) -> AssetResult<Option<TextureMaterial>> {
        let slot = slot_or_reference
            .strip_prefix('#')
            .unwrap_or(slot_or_reference);
        self.resolve_texture_slot(location, slot, &mut Vec::new())
    }

    pub fn collect_model_materials(
        &self,
        location: &ResourceLocation,
    ) -> AssetResult<BTreeSet<TextureMaterial>> {
        let mut materials = BTreeSet::new();
        if let Some(material) = self.resolve_texture(location, "particle")? {
            materials.insert(material);
        }
        for element in self.effective_elements(location)? {
            for face in element.faces.values() {
                let material = self
                    .resolve_texture(location, &face.texture)?
                    .ok_or_else(|| {
                        AssetError::InvalidModel(format!(
                            "model {location} face references unresolved texture `{}`",
                            face.texture
                        ))
                    })?;
                materials.insert(material);
            }
        }
        Ok(materials)
    }

    pub fn bake_model(&self, location: &ResourceLocation) -> AssetResult<BakedBlockModel> {
        let particle = self.resolve_texture(location, "particle")?;
        let mut faces = Vec::new();
        for element in self.effective_elements(location)? {
            for (direction, face) in &element.faces {
                let texture = self
                    .resolve_texture(location, &face.texture)?
                    .ok_or_else(|| {
                        AssetError::InvalidModel(format!(
                            "model {location} face {direction} references unresolved texture `{}`",
                            face.texture
                        ))
                    })?;
                faces.push(BakedBlockModelFace {
                    direction: *direction,
                    cullface: face.cullface,
                    texture,
                    tintindex: face.tintindex,
                    from: element.from,
                    to: element.to,
                    uv: face.uv,
                    uv_rotation: face.uv_rotation,
                    shade: element.shade,
                });
            }
        }
        Ok(BakedBlockModel {
            location: location.clone(),
            particle,
            faces,
            ambient_occlusion: self.effective_ambient_occlusion(location)?,
        })
    }

    pub fn collect_materials_for_models(
        &self,
        locations: impl IntoIterator<Item = ResourceLocation>,
    ) -> AssetResult<BTreeSet<TextureMaterial>> {
        let mut materials = BTreeSet::new();
        for location in locations {
            materials.extend(self.collect_model_materials(&location)?);
        }
        Ok(materials)
    }

    fn load_recursive(
        &mut self,
        source: &impl AssetSource,
        location: ResourceLocation,
        stack: &mut Vec<ResourceLocation>,
    ) -> AssetResult<()> {
        if self.models.contains_key(&location) {
            return Ok(());
        }
        if stack.contains(&location) {
            let chain = stack
                .iter()
                .chain(std::iter::once(&location))
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(AssetError::InvalidModel(format!(
                "model parent cycle: {chain}"
            )));
        }

        stack.push(location.clone());
        let model = BlockModel::load(source, location.clone())?;
        let parent = model.parent.clone();
        self.models.insert(location, model);
        if let Some(parent) = parent {
            self.load_recursive(source, parent, stack)?;
        }
        stack.pop();
        Ok(())
    }

    fn resolve_texture_slot(
        &self,
        location: &ResourceLocation,
        slot: &str,
        seen: &mut Vec<String>,
    ) -> AssetResult<Option<TextureMaterial>> {
        if seen.iter().any(|entry| entry == slot) {
            let chain = seen
                .iter()
                .chain(std::iter::once(&slot.to_owned()))
                .cloned()
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(AssetError::InvalidModel(format!(
                "texture reference cycle in {location}: {chain}"
            )));
        }

        let Some(reference) = self.find_texture_entry(location, slot)? else {
            return Ok(None);
        };
        match reference {
            TextureReference::Direct(material) => Ok(Some(material.clone())),
            TextureReference::Reference(next) => {
                seen.push(slot.to_owned());
                let resolved = self.resolve_texture_slot(location, next, seen);
                seen.pop();
                resolved
            }
        }
    }

    fn find_texture_entry(
        &self,
        location: &ResourceLocation,
        slot: &str,
    ) -> AssetResult<Option<&TextureReference>> {
        let model = self.require_model(location)?;
        if let Some(reference) = model.textures.get(slot) {
            return Ok(Some(reference));
        }
        if let Some(parent) = &model.parent {
            self.find_texture_entry(parent, slot)
        } else {
            Ok(None)
        }
    }

    fn effective_ambient_occlusion(&self, location: &ResourceLocation) -> AssetResult<bool> {
        let model = self.require_model(location)?;
        if let Some(ambient_occlusion) = model.ambient_occlusion {
            return Ok(ambient_occlusion);
        }
        if let Some(parent) = &model.parent {
            self.effective_ambient_occlusion(parent)
        } else {
            Ok(true)
        }
    }

    fn require_model(&self, location: &ResourceLocation) -> AssetResult<&BlockModel> {
        self.models
            .get(location)
            .ok_or_else(|| AssetError::InvalidModel(format!("model {location} was not loaded")))
    }
}

#[derive(Deserialize)]
struct RawBlockModel {
    parent: Option<String>,
    textures: Option<BTreeMap<String, String>>,
    #[serde(default, rename = "ambientocclusion")]
    ambient_occlusion: Option<bool>,
    elements: Option<Vec<RawBlockElement>>,
}

#[derive(Deserialize)]
struct RawBlockElement {
    from: [f32; 3],
    to: [f32; 3],
    faces: BTreeMap<String, RawBlockFace>,
    #[serde(default)]
    shade: Option<bool>,
}

#[derive(Deserialize)]
struct RawBlockFace {
    texture: String,
    cullface: Option<String>,
    tintindex: Option<i32>,
    uv: Option<[f32; 4]>,
    rotation: Option<i32>,
}

impl TryFrom<RawBlockElement> for BlockModelElement {
    type Error = AssetError;

    fn try_from(value: RawBlockElement) -> Result<Self, Self::Error> {
        if value.faces.is_empty() {
            return Err(AssetError::InvalidModel(
                "block model element must have at least one face".to_owned(),
            ));
        }
        validate_extent("from", value.from)?;
        validate_extent("to", value.to)?;
        let from = value.from;
        let to = value.to;
        Ok(Self {
            from,
            to,
            faces: value
                .faces
                .into_iter()
                .map(|(direction, face)| {
                    let direction = ModelFaceDirection::parse(&direction)?;
                    let cullface = match face.cullface.as_deref() {
                        None | Some("") => None,
                        Some(cullface) => Some(ModelFaceDirection::parse(cullface)?),
                    };
                    let uv_rotation = normalized_uv_rotation(face.rotation.unwrap_or(0))?;
                    Ok((
                        direction,
                        BlockModelFace {
                            texture: face.texture,
                            cullface,
                            tintindex: face.tintindex.unwrap_or(-1),
                            uv: face
                                .uv
                                .unwrap_or_else(|| default_face_uv(direction, from, to)),
                            uv_rotation,
                        },
                    ))
                })
                .collect::<AssetResult<BTreeMap<_, _>>>()?,
            shade: value.shade.unwrap_or(true),
        })
    }
}

fn validate_extent(name: &str, value: [f32; 3]) -> AssetResult<()> {
    if value
        .iter()
        .any(|coordinate| *coordinate < -16.0 || *coordinate > 32.0)
    {
        return Err(AssetError::InvalidModel(format!(
            "`{name}` specifier exceeds allowed block model bounds: {value:?}"
        )));
    }
    Ok(())
}

fn normalized_uv_rotation(value: i32) -> AssetResult<i32> {
    if value >= 0 && value % 90 == 0 && value / 90 <= 3 {
        Ok(value)
    } else {
        Err(AssetError::InvalidModel(format!(
            "invalid face UV rotation {value}; expected 0/90/180/270"
        )))
    }
}

fn default_face_uv(direction: ModelFaceDirection, from: [f32; 3], to: [f32; 3]) -> [f32; 4] {
    match direction {
        ModelFaceDirection::Down => [from[0], 16.0 - to[2], to[0], 16.0 - from[2]],
        ModelFaceDirection::Up => [from[0], from[2], to[0], to[2]],
        ModelFaceDirection::North => [16.0 - to[0], 16.0 - to[1], 16.0 - from[0], 16.0 - from[1]],
        ModelFaceDirection::South => [from[0], 16.0 - to[1], to[0], 16.0 - from[1]],
        ModelFaceDirection::West => [from[2], 16.0 - to[1], to[2], 16.0 - from[1]],
        ModelFaceDirection::East => [16.0 - to[2], 16.0 - to[1], 16.0 - from[2], 16.0 - from[1]],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockStateRegistry, MemoryAssetSource, TextureAtlasPlan};

    #[cfg(not(target_arch = "wasm32"))]
    use crate::{BlockStateAssetIndex, FilesystemAssetSource};

    #[test]
    fn block_model_resolves_parent_texture_reference_chain() {
        let source = model_source_with_cube_and_stone();
        let stone = ResourceLocation::parse("minecraft:block/stone").unwrap();
        let library = BlockModelLibrary::load_model_tree(&source, [stone.clone()]).unwrap();

        assert_eq!(library.len(), 4);
        assert_eq!(library.effective_elements(&stone).unwrap().len(), 1);
        assert_eq!(
            library.resolve_texture(&stone, "#particle").unwrap(),
            Some(TextureMaterial::blocks(
                ResourceLocation::parse("minecraft:block/stone").unwrap()
            ))
        );
        assert_eq!(
            library.collect_model_materials(&stone).unwrap(),
            BTreeSet::from([TextureMaterial::blocks(
                ResourceLocation::parse("minecraft:block/stone").unwrap()
            )])
        );

        let baked = library.bake_model(&stone).unwrap();
        assert_eq!(baked.faces.len(), 6);
        assert!(baked.ambient_occlusion);
        assert_eq!(
            baked.particle,
            Some(TextureMaterial::blocks(
                ResourceLocation::parse("minecraft:block/stone").unwrap()
            ))
        );
        assert!(baked.faces.iter().all(|face| {
            face.texture
                == TextureMaterial::blocks(
                    ResourceLocation::parse("minecraft:block/stone").unwrap(),
                )
                && face.uv == [0.0, 0.0, 16.0, 16.0]
        }));
    }

    #[test]
    fn baked_model_inherits_parent_ambient_occlusion() {
        let mut source = model_source_with_cube_and_stone();
        source.insert_text(
            AssetPath::new("assets/minecraft/models/block/cube_all.json"),
            r##"{
              "ambientocclusion":false,
              "parent":"minecraft:block/cube",
              "textures":{
                "particle":"#all",
                "down":"#all",
                "up":"#all",
                "north":"#all",
                "east":"#all",
                "south":"#all",
                "west":"#all"
              }
            }"##,
        );
        let stone = ResourceLocation::parse("minecraft:block/stone").unwrap();
        let library = BlockModelLibrary::load_model_tree(&source, [stone.clone()]).unwrap();

        assert!(!library.bake_model(&stone).unwrap().ambient_occlusion);
    }

    #[test]
    fn texture_reference_cycles_are_errors() {
        let mut source = MemoryAssetSource::new();
        source.insert_text(
            AssetPath::new("assets/minecraft/models/block/cycle.json"),
            r##"{"textures":{"a":"#b","b":"#a"}}"##,
        );
        let location = ResourceLocation::parse("minecraft:block/cycle").unwrap();
        let library = BlockModelLibrary::load_model_tree(&source, [location.clone()]).unwrap();

        assert!(library.resolve_texture(&location, "#a").is_err());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn real_extracted_minecraft_models_bake_terrain_mvp_atlas_materials() {
        let root = std::path::PathBuf::from("../reference/minecraft-1.17.1/extracted");
        if !root.exists() {
            return;
        }
        let source = FilesystemAssetSource::new(root);
        let index = BlockStateAssetIndex::load_namespace(&source, "minecraft").unwrap();
        let registry = BlockStateRegistry::terrain_mvp();
        registry.validate_blockstate_assets(&index).unwrap();

        let mut model_refs = BTreeSet::new();
        for record in registry.records() {
            let asset = index.get(&record.block).unwrap();
            if let Some(variant_key) = record.asset_variant_key(asset) {
                let variants = asset.variants_for_key(&variant_key).unwrap();
                for variant in variants {
                    model_refs.insert(variant.model.clone());
                }
            } else if record.variant_key().is_empty() {
                model_refs.extend(asset.model_refs.iter().cloned());
            } else {
                panic!(
                    "{} is not covered by blockstate asset variants",
                    record.canonical_key()
                );
            }
        }

        let library = BlockModelLibrary::load_model_tree(&source, model_refs.clone()).unwrap();
        let materials = library
            .collect_materials_for_models(model_refs.iter().cloned())
            .unwrap();
        let stone =
            TextureMaterial::blocks(ResourceLocation::parse("minecraft:block/stone").unwrap());
        let grass_top = TextureMaterial::blocks(
            ResourceLocation::parse("minecraft:block/grass_block_top").unwrap(),
        );
        let water_still = TextureMaterial::blocks(
            ResourceLocation::parse("minecraft:block/water_still").unwrap(),
        );

        assert!(materials.contains(&stone));
        assert!(materials.contains(&grass_top));
        assert!(materials.contains(&water_still));

        let baked_stone = library
            .bake_model(&ResourceLocation::parse("minecraft:block/stone").unwrap())
            .unwrap();
        assert_eq!(baked_stone.faces.len(), 6);
        assert!(baked_stone.faces.iter().all(|face| face.texture == stone));

        let plan = TextureAtlasPlan::build(&source, materials.clone()).unwrap();
        assert_eq!(plan.len(), materials.len());
        assert!(plan.width() >= 16);
        assert!(plan.height() >= 16);
        assert!(plan.sprite(&stone).is_some());
    }

    pub(super) fn model_source_with_cube_and_stone() -> MemoryAssetSource {
        let mut source = MemoryAssetSource::new();
        source.insert_text(
            AssetPath::new("assets/minecraft/models/block/block.json"),
            "{}",
        );
        source.insert_text(
            AssetPath::new("assets/minecraft/models/block/cube.json"),
            r##"{
              "parent":"minecraft:block/block",
              "elements":[{
                "from":[0,0,0],
                "to":[16,16,16],
                "faces":{
                  "down":{"texture":"#down"},
                  "up":{"texture":"#up"},
                  "north":{"texture":"#north"},
                  "south":{"texture":"#south"},
                  "west":{"texture":"#west"},
                  "east":{"texture":"#east"}
                }
              }]
            }"##,
        );
        source.insert_text(
            AssetPath::new("assets/minecraft/models/block/cube_all.json"),
            r##"{
              "parent":"minecraft:block/cube",
              "textures":{
                "particle":"#all",
                "down":"#all",
                "up":"#all",
                "north":"#all",
                "east":"#all",
                "south":"#all",
                "west":"#all"
              }
            }"##,
        );
        source.insert_text(
            AssetPath::new("assets/minecraft/models/block/stone.json"),
            r##"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"##,
        );
        source
    }
}
