use std::collections::{BTreeMap, BTreeSet};

use mclone_core::BlockStateId;
use serde_json::Value;

use crate::{AssetError, AssetPath, AssetResult, AssetSource, ResourceLocation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockStateRecord {
    pub id: BlockStateId,
    pub block: ResourceLocation,
    pub properties: BTreeMap<String, String>,
}

impl BlockStateRecord {
    pub fn new(
        id: BlockStateId,
        block: ResourceLocation,
        properties: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            id,
            block,
            properties: properties
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
        }
    }

    pub fn variant_key(&self) -> String {
        if self.properties.is_empty() {
            return String::new();
        }
        self.properties
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn asset_path(&self) -> AssetPath {
        AssetPath::blockstate_json(&self.block)
    }

    pub fn canonical_key(&self) -> String {
        let variant_key = self.variant_key();
        if variant_key.is_empty() {
            self.block.to_string()
        } else {
            format!("{}[{variant_key}]", self.block)
        }
    }

    pub fn asset_variant_key(&self, asset: &BlockStateAsset) -> Option<String> {
        let variant_key = self.variant_key();
        if asset.variants_for_key(&variant_key).is_some() {
            return Some(variant_key);
        }
        if self.can_use_empty_fluid_model_variant() && asset.variants_for_key("").is_some() {
            return Some(String::new());
        }
        None
    }

    fn can_use_empty_fluid_model_variant(&self) -> bool {
        self.block.namespace() == "minecraft"
            && matches!(self.block.path(), "water" | "lava")
            && self.properties.len() == 1
            && self.properties.contains_key("level")
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockStateRegistry {
    by_id: BTreeMap<BlockStateId, BlockStateRecord>,
    by_key: BTreeMap<String, BlockStateId>,
}

impl BlockStateRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn terrain_mvp() -> Self {
        let mut registry = Self::new();
        for (id, name, properties) in TERRAIN_MVP_STATES {
            registry
                .register(BlockStateRecord::new(
                    BlockStateId(*id),
                    ResourceLocation::parse(name).expect("terrain MVP block names are valid"),
                    properties.iter().copied(),
                ))
                .expect("terrain MVP block ids are unique");
        }
        registry
    }

    pub fn register(&mut self, state: BlockStateRecord) -> AssetResult<()> {
        if self.by_id.contains_key(&state.id) {
            return Err(AssetError::InvalidBlockState(format!(
                "duplicate block state id {}",
                state.id.0
            )));
        }
        let key = state.canonical_key();
        if self.by_key.contains_key(&key) {
            return Err(AssetError::InvalidBlockState(format!(
                "duplicate block state key {key}"
            )));
        }
        self.by_key.insert(key, state.id);
        self.by_id.insert(state.id, state);
        Ok(())
    }

    pub fn by_id(&self, id: BlockStateId) -> Option<&BlockStateRecord> {
        self.by_id.get(&id)
    }

    pub fn id_for_key(&self, key: &str) -> Option<BlockStateId> {
        self.by_key.get(key).copied()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn records(&self) -> impl Iterator<Item = &BlockStateRecord> {
        self.by_id.values()
    }

    pub fn validate_blockstate_assets(&self, index: &BlockStateAssetIndex) -> AssetResult<()> {
        for record in self.records() {
            let asset = index.get(&record.block).ok_or_else(|| {
                AssetError::MissingAsset(AssetPath::blockstate_json(&record.block))
            })?;
            if asset.variant_keys.is_empty() {
                continue;
            }
            if record.asset_variant_key(asset).is_none() {
                return Err(AssetError::InvalidBlockState(format!(
                    "{} references missing blockstate variant `{}`",
                    record.canonical_key(),
                    record.variant_key()
                )));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockStateAsset {
    pub block: ResourceLocation,
    pub path: AssetPath,
    pub variants: BTreeMap<String, Vec<BlockStateVariant>>,
    pub variant_keys: BTreeSet<String>,
    pub model_refs: BTreeSet<ResourceLocation>,
}

impl BlockStateAsset {
    pub fn load(source: &impl AssetSource, block: ResourceLocation) -> AssetResult<Self> {
        let path = AssetPath::blockstate_json(&block);
        let bytes = source
            .read(&path)?
            .ok_or_else(|| AssetError::MissingAsset(path.clone()))?;
        let json: Value = serde_json::from_slice(&bytes).map_err(|source| AssetError::Json {
            path: path.clone(),
            source,
        })?;
        let variants = parse_variants(&json)?;
        let variant_keys = variants.keys().cloned().collect();
        let mut model_refs = BTreeSet::new();
        collect_model_refs(&json, &mut model_refs)?;
        Ok(Self {
            block,
            path,
            variants,
            variant_keys,
            model_refs,
        })
    }

    pub fn variants_for_key(&self, key: &str) -> Option<&[BlockStateVariant]> {
        self.variants.get(key).map(Vec::as_slice)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockStateVariant {
    pub model: ResourceLocation,
    pub x: i32,
    pub y: i32,
    pub uvlock: bool,
    pub weight: u32,
}

impl BlockStateVariant {
    fn from_json(value: &Value) -> AssetResult<Self> {
        let object = value.as_object().ok_or_else(|| {
            AssetError::InvalidBlockState("blockstate variant must be an object".to_owned())
        })?;
        let model = object
            .get("model")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AssetError::InvalidBlockState("blockstate variant missing model".to_owned())
            })
            .and_then(ResourceLocation::parse)?;
        let x = normalized_rotation(get_i32(object, "x", 0)?)?;
        let y = normalized_rotation(get_i32(object, "y", 0)?)?;
        let uvlock = get_bool(object, "uvlock", false)?;
        let weight = get_i32(object, "weight", 1)?;
        if weight < 1 {
            return Err(AssetError::InvalidBlockState(format!(
                "invalid variant weight {weight}; expected integer >= 1"
            )));
        }

        Ok(Self {
            model,
            x,
            y,
            uvlock,
            weight: weight as u32,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockStateAssetIndex {
    assets: BTreeMap<ResourceLocation, BlockStateAsset>,
}

impl BlockStateAssetIndex {
    pub fn load_namespace(source: &impl AssetSource, namespace: &str) -> AssetResult<Self> {
        let prefix = format!("assets/{namespace}/blockstates/");
        let mut assets = BTreeMap::new();
        for path in source.list(&prefix, ".json")? {
            let block_path = path
                .as_str()
                .strip_prefix(&prefix)
                .and_then(|value| value.strip_suffix(".json"))
                .ok_or_else(|| AssetError::InvalidAssetPath(path.as_str().to_owned()))?;
            let block = ResourceLocation::new(namespace, block_path)?;
            let asset = BlockStateAsset::load(source, block.clone())?;
            assets.insert(block, asset);
        }
        Ok(Self { assets })
    }

    pub fn get(&self, block: &ResourceLocation) -> Option<&BlockStateAsset> {
        self.assets.get(block)
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    pub fn assets(&self) -> impl Iterator<Item = &BlockStateAsset> {
        self.assets.values()
    }
}

fn parse_variants(value: &Value) -> AssetResult<BTreeMap<String, Vec<BlockStateVariant>>> {
    let Some(variants) = value.get("variants") else {
        return Ok(BTreeMap::new());
    };
    let variants = variants.as_object().ok_or_else(|| {
        AssetError::InvalidBlockState("blockstate variants must be an object".to_owned())
    })?;
    let mut parsed = BTreeMap::new();
    for (key, value) in variants {
        let variants = match value {
            Value::Array(values) => {
                if values.is_empty() {
                    return Err(AssetError::InvalidBlockState(format!(
                        "blockstate variant `{key}` is an empty array"
                    )));
                }
                values
                    .iter()
                    .map(BlockStateVariant::from_json)
                    .collect::<AssetResult<Vec<_>>>()?
            }
            Value::Object(_) => vec![BlockStateVariant::from_json(value)?],
            _ => {
                return Err(AssetError::InvalidBlockState(format!(
                    "blockstate variant `{key}` must be an object or array"
                )));
            }
        };
        parsed.insert(key.clone(), variants);
    }
    Ok(parsed)
}

fn collect_model_refs(value: &Value, out: &mut BTreeSet<ResourceLocation>) -> AssetResult<()> {
    match value {
        Value::Object(object) => {
            if let Some(model) = object.get("model").and_then(Value::as_str) {
                out.insert(ResourceLocation::parse(model)?);
            }
            for value in object.values() {
                collect_model_refs(value, out)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_model_refs(value, out)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn get_i32(object: &serde_json::Map<String, Value>, name: &str, default: i32) -> AssetResult<i32> {
    let Some(value) = object.get(name) else {
        return Ok(default);
    };
    let Some(value) = value.as_i64() else {
        return Err(AssetError::InvalidBlockState(format!(
            "`{name}` must be an integer"
        )));
    };
    i32::try_from(value).map_err(|_| {
        AssetError::InvalidBlockState(format!("`{name}` value {value} does not fit in i32"))
    })
}

fn get_bool(
    object: &serde_json::Map<String, Value>,
    name: &str,
    default: bool,
) -> AssetResult<bool> {
    let Some(value) = object.get(name) else {
        return Ok(default);
    };
    value
        .as_bool()
        .ok_or_else(|| AssetError::InvalidBlockState(format!("`{name}` must be a boolean")))
}

fn normalized_rotation(value: i32) -> AssetResult<i32> {
    let normalized = value.rem_euclid(360);
    if normalized % 90 != 0 {
        return Err(AssetError::InvalidBlockState(format!(
            "invalid block model rotation {value}; expected a multiple of 90 degrees"
        )));
    }
    Ok(normalized)
}

const EMPTY_PROPS: &[(&str, &str)] = &[];
const SNOWY_FALSE: &[(&str, &str)] = &[("snowy", "false")];
const AXIS_X: &[(&str, &str)] = &[("axis", "x")];
const AXIS_Y: &[(&str, &str)] = &[("axis", "y")];
const AXIS_Z: &[(&str, &str)] = &[("axis", "z")];
const LAYERS_1: &[(&str, &str)] = &[("layers", "1")];
const HALF_LOWER: &[(&str, &str)] = &[("half", "lower")];
const HALF_UPPER: &[(&str, &str)] = &[("half", "upper")];
const POINTED_DRIPSTONE_UP_TIP: &[(&str, &str)] =
    &[("thickness", "tip"), ("vertical_direction", "up")];
const FACING_NORTH: &[(&str, &str)] = &[("facing", "north")];
const FACING_EAST: &[(&str, &str)] = &[("facing", "east")];
const FACING_SOUTH: &[(&str, &str)] = &[("facing", "south")];
const FACING_WEST: &[(&str, &str)] = &[("facing", "west")];
const AGE_0: &[(&str, &str)] = &[("age", "0")];
const LEVEL_0: &[(&str, &str)] = &[("level", "0")];
const LEVEL_1: &[(&str, &str)] = &[("level", "1")];
const LEVEL_2: &[(&str, &str)] = &[("level", "2")];
const LEVEL_3: &[(&str, &str)] = &[("level", "3")];
const LEVEL_4: &[(&str, &str)] = &[("level", "4")];
const LEVEL_5: &[(&str, &str)] = &[("level", "5")];
const LEVEL_6: &[(&str, &str)] = &[("level", "6")];
const LEVEL_7: &[(&str, &str)] = &[("level", "7")];
const LEVEL_8: &[(&str, &str)] = &[("level", "8")];

const TERRAIN_MVP_STATES: &[(u32, &str, &[(&str, &str)])] = &[
    (0, "minecraft:air", EMPTY_PROPS),
    (1, "minecraft:stone", EMPTY_PROPS),
    (2, "minecraft:water", LEVEL_0),
    (3, "minecraft:bedrock", EMPTY_PROPS),
    (4, "minecraft:grass_block", SNOWY_FALSE),
    (5, "minecraft:dirt", EMPTY_PROPS),
    (6, "minecraft:sand", EMPTY_PROPS),
    (7, "minecraft:gravel", EMPTY_PROPS),
    (8, "minecraft:snow", LAYERS_1),
    (9, "minecraft:lava", LEVEL_0),
    (10, "minecraft:granite", EMPTY_PROPS),
    (11, "minecraft:diorite", EMPTY_PROPS),
    (12, "minecraft:andesite", EMPTY_PROPS),
    (13, "minecraft:coarse_dirt", EMPTY_PROPS),
    (14, "minecraft:podzol", SNOWY_FALSE),
    (15, "minecraft:mycelium", SNOWY_FALSE),
    (16, "minecraft:terracotta", EMPTY_PROPS),
    (17, "minecraft:white_terracotta", EMPTY_PROPS),
    (18, "minecraft:orange_terracotta", EMPTY_PROPS),
    (19, "minecraft:magenta_terracotta", EMPTY_PROPS),
    (20, "minecraft:light_blue_terracotta", EMPTY_PROPS),
    (21, "minecraft:yellow_terracotta", EMPTY_PROPS),
    (22, "minecraft:lime_terracotta", EMPTY_PROPS),
    (23, "minecraft:pink_terracotta", EMPTY_PROPS),
    (24, "minecraft:gray_terracotta", EMPTY_PROPS),
    (25, "minecraft:light_gray_terracotta", EMPTY_PROPS),
    (26, "minecraft:cyan_terracotta", EMPTY_PROPS),
    (27, "minecraft:purple_terracotta", EMPTY_PROPS),
    (28, "minecraft:blue_terracotta", EMPTY_PROPS),
    (29, "minecraft:brown_terracotta", EMPTY_PROPS),
    (30, "minecraft:green_terracotta", EMPTY_PROPS),
    (31, "minecraft:red_terracotta", EMPTY_PROPS),
    (32, "minecraft:black_terracotta", EMPTY_PROPS),
    (33, "minecraft:sandstone", EMPTY_PROPS),
    (34, "minecraft:red_sandstone", EMPTY_PROPS),
    (35, "minecraft:packed_ice", EMPTY_PROPS),
    (36, "minecraft:obsidian", EMPTY_PROPS),
    (37, "minecraft:magma_block", EMPTY_PROPS),
    (38, "minecraft:red_sand", EMPTY_PROPS),
    (39, "minecraft:ice", EMPTY_PROPS),
    (40, "minecraft:snow_block", EMPTY_PROPS),
    (41, "minecraft:oak_log", AXIS_Y),
    (42, "minecraft:oak_leaves", EMPTY_PROPS),
    (43, "minecraft:grass", EMPTY_PROPS),
    (44, "minecraft:dandelion", EMPTY_PROPS),
    (45, "minecraft:poppy", EMPTY_PROPS),
    (46, "minecraft:birch_log", AXIS_Y),
    (47, "minecraft:birch_leaves", EMPTY_PROPS),
    (48, "minecraft:spruce_log", AXIS_Y),
    (49, "minecraft:spruce_leaves", EMPTY_PROPS),
    (50, "minecraft:fern", EMPTY_PROPS),
    (51, "minecraft:dead_bush", EMPTY_PROPS),
    (52, "minecraft:tuff", EMPTY_PROPS),
    (53, "minecraft:deepslate", AXIS_Y),
    (54, "minecraft:coal_ore", EMPTY_PROPS),
    (55, "minecraft:deepslate_coal_ore", EMPTY_PROPS),
    (56, "minecraft:copper_ore", EMPTY_PROPS),
    (57, "minecraft:deepslate_copper_ore", EMPTY_PROPS),
    (58, "minecraft:iron_ore", EMPTY_PROPS),
    (59, "minecraft:deepslate_iron_ore", EMPTY_PROPS),
    (60, "minecraft:gold_ore", EMPTY_PROPS),
    (61, "minecraft:deepslate_gold_ore", EMPTY_PROPS),
    (62, "minecraft:redstone_ore", EMPTY_PROPS),
    (63, "minecraft:deepslate_redstone_ore", EMPTY_PROPS),
    (64, "minecraft:diamond_ore", EMPTY_PROPS),
    (65, "minecraft:deepslate_diamond_ore", EMPTY_PROPS),
    (66, "minecraft:lapis_ore", EMPTY_PROPS),
    (67, "minecraft:deepslate_lapis_ore", EMPTY_PROPS),
    (68, "minecraft:large_fern", HALF_LOWER),
    (69, "minecraft:large_fern", HALF_UPPER),
    (70, "minecraft:glow_lichen", EMPTY_PROPS),
    (71, "minecraft:cave_air", EMPTY_PROPS),
    // These ids mirror mclone_worldgen::block fluid level ids. The asset JSON only has
    // a base water/lava model variant; simulation still needs distinct level states.
    (72, "minecraft:water", LEVEL_1),
    (73, "minecraft:water", LEVEL_2),
    (74, "minecraft:water", LEVEL_3),
    (75, "minecraft:water", LEVEL_4),
    (76, "minecraft:water", LEVEL_5),
    (77, "minecraft:water", LEVEL_6),
    (78, "minecraft:water", LEVEL_7),
    (79, "minecraft:water", LEVEL_8),
    (80, "minecraft:lava", LEVEL_1),
    (81, "minecraft:lava", LEVEL_2),
    (82, "minecraft:lava", LEVEL_3),
    (83, "minecraft:lava", LEVEL_4),
    (84, "minecraft:lava", LEVEL_5),
    (85, "minecraft:lava", LEVEL_6),
    (86, "minecraft:lava", LEVEL_7),
    (87, "minecraft:lava", LEVEL_8),
    (88, "minecraft:clay", EMPTY_PROPS),
    (89, "minecraft:dripstone_block", EMPTY_PROPS),
    (90, "minecraft:pointed_dripstone", POINTED_DRIPSTONE_UP_TIP),
    (91, "minecraft:bricks", EMPTY_PROPS),
    (92, "minecraft:oak_log", AXIS_X),
    (93, "minecraft:oak_log", AXIS_Z),
    (94, "minecraft:birch_log", AXIS_X),
    (95, "minecraft:birch_log", AXIS_Z),
    (96, "minecraft:spruce_log", AXIS_X),
    (97, "minecraft:spruce_log", AXIS_Z),
    (98, "minecraft:deepslate", AXIS_X),
    (99, "minecraft:deepslate", AXIS_Z),
    (100, "minecraft:torch", EMPTY_PROPS),
    (101, "minecraft:wall_torch", FACING_NORTH),
    (102, "minecraft:wall_torch", FACING_EAST),
    (103, "minecraft:wall_torch", FACING_SOUTH),
    (104, "minecraft:wall_torch", FACING_WEST),
    (105, "minecraft:cactus", AGE_0),
    (106, "minecraft:sugar_cane", AGE_0),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FilesystemAssetSource, MemoryAssetSource};

    #[test]
    fn terrain_mvp_registry_names_current_generated_ids() {
        let registry = BlockStateRegistry::terrain_mvp();

        assert_eq!(registry.len(), 107);
        assert_eq!(
            registry.by_id(BlockStateId(0)).unwrap().canonical_key(),
            "minecraft:air"
        );
        assert_eq!(
            registry.by_id(BlockStateId(4)).unwrap().canonical_key(),
            "minecraft:grass_block[snowy=false]"
        );
        assert_eq!(
            registry.id_for_key("minecraft:stone"),
            Some(BlockStateId(1))
        );
        assert_eq!(
            registry.by_id(BlockStateId(2)).unwrap().canonical_key(),
            "minecraft:water[level=0]"
        );
        assert_eq!(
            registry.id_for_key("minecraft:water[level=1]"),
            Some(BlockStateId(72))
        );
        assert_eq!(
            registry.id_for_key("minecraft:water[level=8]"),
            Some(BlockStateId(79))
        );
        assert_eq!(
            registry.by_id(BlockStateId(9)).unwrap().canonical_key(),
            "minecraft:lava[level=0]"
        );
        assert_eq!(
            registry.id_for_key("minecraft:lava[level=8]"),
            Some(BlockStateId(87))
        );
        assert_eq!(
            registry.id_for_key("minecraft:clay"),
            Some(BlockStateId(88))
        );
        assert_eq!(
            registry.id_for_key("minecraft:dripstone_block"),
            Some(BlockStateId(89))
        );
        assert_eq!(
            registry.id_for_key("minecraft:pointed_dripstone[thickness=tip,vertical_direction=up]"),
            Some(BlockStateId(90))
        );
        assert_eq!(
            registry.id_for_key("minecraft:bricks"),
            Some(BlockStateId(91))
        );
        assert_eq!(
            registry.by_id(BlockStateId(41)).unwrap().canonical_key(),
            "minecraft:oak_log[axis=y]"
        );
        assert_eq!(
            registry.id_for_key("minecraft:oak_log[axis=x]"),
            Some(BlockStateId(92))
        );
        assert_eq!(
            registry.id_for_key("minecraft:oak_log[axis=z]"),
            Some(BlockStateId(93))
        );
        assert_eq!(
            registry.id_for_key("minecraft:poppy"),
            Some(BlockStateId(45))
        );
        assert_eq!(
            registry.by_id(BlockStateId(48)).unwrap().canonical_key(),
            "minecraft:spruce_log[axis=y]"
        );
        assert_eq!(
            registry.id_for_key("minecraft:birch_log[axis=x]"),
            Some(BlockStateId(94))
        );
        assert_eq!(
            registry.id_for_key("minecraft:spruce_log[axis=z]"),
            Some(BlockStateId(97))
        );
        assert_eq!(
            registry.id_for_key("minecraft:dead_bush"),
            Some(BlockStateId(51))
        );
        assert_eq!(
            registry.id_for_key("minecraft:tuff"),
            Some(BlockStateId(52))
        );
        assert_eq!(
            registry.id_for_key("minecraft:deepslate[axis=y]"),
            Some(BlockStateId(53))
        );
        assert_eq!(
            registry.id_for_key("minecraft:deepslate[axis=x]"),
            Some(BlockStateId(98))
        );
        assert_eq!(
            registry.by_id(BlockStateId(53)).unwrap().canonical_key(),
            "minecraft:deepslate[axis=y]"
        );
        assert_eq!(
            registry.id_for_key("minecraft:coal_ore"),
            Some(BlockStateId(54))
        );
        assert_eq!(
            registry.id_for_key("minecraft:deepslate_lapis_ore"),
            Some(BlockStateId(67))
        );
        assert_eq!(
            registry.id_for_key("minecraft:large_fern[half=lower]"),
            Some(BlockStateId(68))
        );
        assert_eq!(
            registry.by_id(BlockStateId(69)).unwrap().canonical_key(),
            "minecraft:large_fern[half=upper]"
        );
        assert_eq!(
            registry.id_for_key("minecraft:glow_lichen"),
            Some(BlockStateId(70))
        );
        assert_eq!(
            registry.id_for_key("minecraft:cave_air"),
            Some(BlockStateId(71))
        );
        assert_eq!(
            registry.by_id(BlockStateId(8)).unwrap().canonical_key(),
            "minecraft:snow[layers=1]"
        );
        assert_eq!(
            registry.id_for_key("minecraft:magma_block"),
            Some(BlockStateId(37))
        );
        assert_eq!(
            registry.id_for_key("minecraft:torch"),
            Some(BlockStateId(100))
        );
        assert_eq!(
            registry.id_for_key("minecraft:wall_torch[facing=north]"),
            Some(BlockStateId(101))
        );
        assert_eq!(
            registry.id_for_key("minecraft:wall_torch[facing=east]"),
            Some(BlockStateId(102))
        );
        assert_eq!(
            registry.id_for_key("minecraft:wall_torch[facing=south]"),
            Some(BlockStateId(103))
        );
        assert_eq!(
            registry.by_id(BlockStateId(104)).unwrap().canonical_key(),
            "minecraft:wall_torch[facing=west]"
        );
        assert_eq!(
            registry.id_for_key("minecraft:cactus[age=0]"),
            Some(BlockStateId(105))
        );
        assert_eq!(
            registry.by_id(BlockStateId(106)).unwrap().canonical_key(),
            "minecraft:sugar_cane[age=0]"
        );
    }

    #[test]
    fn blockstate_asset_parses_variants_and_model_refs() {
        let mut source = MemoryAssetSource::new();
        source.insert_text(
            AssetPath::new("assets/minecraft/blockstates/grass_block.json"),
            r#"{
              "variants": {
                "snowy=false": [
                  { "model": "minecraft:block/grass_block" },
                  { "model": "minecraft:block/grass_block", "y": 90 }
                ],
                "snowy=true": { "model": "minecraft:block/grass_block_snow" }
              }
            }"#,
        );

        let asset = BlockStateAsset::load(
            &source,
            ResourceLocation::parse("minecraft:grass_block").unwrap(),
        )
        .unwrap();

        assert_eq!(
            asset.variant_keys.iter().cloned().collect::<Vec<_>>(),
            vec!["snowy=false", "snowy=true"]
        );
        assert_eq!(
            asset.variants_for_key("snowy=false").unwrap(),
            [
                BlockStateVariant {
                    model: ResourceLocation::parse("minecraft:block/grass_block").unwrap(),
                    x: 0,
                    y: 0,
                    uvlock: false,
                    weight: 1,
                },
                BlockStateVariant {
                    model: ResourceLocation::parse("minecraft:block/grass_block").unwrap(),
                    x: 0,
                    y: 90,
                    uvlock: false,
                    weight: 1,
                }
            ]
        );
        assert!(
            asset
                .model_refs
                .contains(&ResourceLocation::parse("minecraft:block/grass_block").unwrap())
        );
        assert!(
            asset
                .model_refs
                .contains(&ResourceLocation::parse("minecraft:block/grass_block_snow").unwrap())
        );
    }

    #[test]
    fn blockstate_index_validates_registry_variants() {
        let mut source = MemoryAssetSource::new();
        source.insert_text(
            AssetPath::new("assets/minecraft/blockstates/grass_block.json"),
            r#"{"variants":{"snowy=false":{"model":"minecraft:block/grass_block"}}}"#,
        );
        let index = BlockStateAssetIndex::load_namespace(&source, "minecraft").unwrap();
        let mut registry = BlockStateRegistry::new();
        registry
            .register(BlockStateRecord::new(
                BlockStateId(4),
                ResourceLocation::parse("minecraft:grass_block").unwrap(),
                [("snowy", "false")],
            ))
            .unwrap();

        registry.validate_blockstate_assets(&index).unwrap();
    }

    #[test]
    fn fluid_level_states_use_base_asset_variant() {
        let mut source = MemoryAssetSource::new();
        source.insert_text(
            AssetPath::new("assets/minecraft/blockstates/water.json"),
            r#"{"variants":{"":{"model":"minecraft:block/water"}}}"#,
        );
        let index = BlockStateAssetIndex::load_namespace(&source, "minecraft").unwrap();
        let mut registry = BlockStateRegistry::new();
        registry
            .register(BlockStateRecord::new(
                BlockStateId(72),
                ResourceLocation::parse("minecraft:water").unwrap(),
                [("level", "1")],
            ))
            .unwrap();

        registry.validate_blockstate_assets(&index).unwrap();
        let record = registry.by_id(BlockStateId(72)).unwrap();
        let asset = index
            .get(&ResourceLocation::parse("minecraft:water").unwrap())
            .unwrap();
        assert_eq!(record.asset_variant_key(asset), Some(String::new()));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn real_extracted_minecraft_blockstates_cover_terrain_mvp_registry() {
        let root = std::path::PathBuf::from("../reference/minecraft-1.17.1/extracted");
        if !root.exists() {
            return;
        }
        let source = FilesystemAssetSource::new(root);
        let index = BlockStateAssetIndex::load_namespace(&source, "minecraft").unwrap();
        let registry = BlockStateRegistry::terrain_mvp();

        assert_eq!(index.len(), 900);
        registry.validate_blockstate_assets(&index).unwrap();
        assert!(
            index
                .get(&ResourceLocation::parse("minecraft:stone").unwrap())
                .unwrap()
                .model_refs
                .contains(&ResourceLocation::parse("minecraft:block/stone").unwrap())
        );
    }
}
