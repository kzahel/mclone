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
        if asset.variant_keys.is_empty() {
            return Some(String::new());
        }
        let variant_key = self.variant_key();
        if asset.variants_for_key(&variant_key).is_some() {
            return Some(variant_key);
        }
        if let Some(variant_key) = self.model_variant_key_without_ignored_properties(asset) {
            return Some(variant_key);
        }
        if self.can_use_empty_model_variant(asset) {
            return Some(String::new());
        }
        None
    }

    fn model_variant_key_without_ignored_properties(
        &self,
        asset: &BlockStateAsset,
    ) -> Option<String> {
        let mut filtered = self.properties.clone();
        filtered.retain(|property, _| !self.property_ignored_by_model_variant(property));
        if filtered.len() == self.properties.len() {
            return None;
        }
        let variant_key = filtered
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(",");
        asset.variants_for_key(&variant_key).map(|_| variant_key)
    }

    fn can_use_empty_model_variant(&self, asset: &BlockStateAsset) -> bool {
        !self.properties.is_empty()
            && self.block.namespace() == "minecraft"
            && asset.variant_keys.len() == 1
            && asset.variants_for_key("").is_some()
            && self.properties.keys().all(|property| {
                self.property_uses_empty_model_variant(property)
                    || self.property_ignored_by_model_variant(property)
            })
    }

    fn property_uses_empty_model_variant(&self, property: &str) -> bool {
        matches!(
            (self.block.path(), property),
            ("water" | "lava", "level") | ("cactus" | "sugar_cane" | "kelp", "age")
        )
    }

    fn property_ignored_by_model_variant(&self, property: &str) -> bool {
        property == "waterlogged"
            && matches!(
                self.block.path(),
                "tube_coral"
                    | "brain_coral"
                    | "bubble_coral"
                    | "fire_coral"
                    | "horn_coral"
                    | "tube_coral_fan"
                    | "brain_coral_fan"
                    | "bubble_coral_fan"
                    | "fire_coral_fan"
                    | "horn_coral_fan"
                    | "tube_coral_wall_fan"
                    | "brain_coral_wall_fan"
                    | "bubble_coral_wall_fan"
                    | "fire_coral_wall_fan"
                    | "horn_coral_wall_fan"
            )
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
    pub multipart: Vec<MultipartCase>,
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
        let multipart = parse_multipart(&json)?;
        Ok(Self {
            block,
            path,
            variants,
            variant_keys,
            model_refs,
            multipart,
        })
    }

    pub fn variants_for_key(&self, key: &str) -> Option<&[BlockStateVariant]> {
        self.variants.get(key).map(Vec::as_slice)
    }

    /// Evaluate a `multipart` blockstate for the given state properties, returning the
    /// selected model variant of every case whose `when` condition matches. Each matching
    /// case contributes one part (the first `apply` entry; random weighted alternatives are
    /// not yet distinguished). Vanilla multipart blocks build a cube from several
    /// single-face parts, so callers should render every returned variant.
    pub fn multipart_selections(
        &self,
        properties: &BTreeMap<String, String>,
    ) -> Vec<&BlockStateVariant> {
        self.multipart
            .iter()
            .filter(|case| case.matches(properties))
            .filter_map(|case| case.apply.first())
            .collect()
    }
}

/// A single `{ "when": ..., "apply": ... }` entry of a `multipart` blockstate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultipartCase {
    /// The condition gating this case. `None` means the case always applies.
    pub when: Option<MultipartWhen>,
    /// Candidate model variants. Vanilla uses an array to express a weighted random choice;
    /// we currently take the first entry deterministically.
    pub apply: Vec<BlockStateVariant>,
}

impl MultipartCase {
    fn from_json(value: &Value) -> AssetResult<Self> {
        let object = value.as_object().ok_or_else(|| {
            AssetError::InvalidBlockState("multipart case must be an object".to_owned())
        })?;
        let when = match object.get("when") {
            Some(value) => Some(MultipartWhen::from_json(value)?),
            None => None,
        };
        let apply = object.get("apply").ok_or_else(|| {
            AssetError::InvalidBlockState("multipart case missing `apply`".to_owned())
        })?;
        let apply = match apply {
            Value::Array(values) => {
                if values.is_empty() {
                    return Err(AssetError::InvalidBlockState(
                        "multipart case `apply` array is empty".to_owned(),
                    ));
                }
                values
                    .iter()
                    .map(BlockStateVariant::from_json)
                    .collect::<AssetResult<Vec<_>>>()?
            }
            Value::Object(_) => vec![BlockStateVariant::from_json(apply)?],
            _ => {
                return Err(AssetError::InvalidBlockState(
                    "multipart case `apply` must be an object or array".to_owned(),
                ));
            }
        };
        Ok(Self { when, apply })
    }

    pub fn matches(&self, properties: &BTreeMap<String, String>) -> bool {
        match &self.when {
            Some(when) => when.matches(properties),
            None => true,
        }
    }
}

/// The `when` condition of a multipart case. Property comparisons treat a property that is
/// absent from the state as the value `"false"`, which keeps directional blocks (mushrooms,
/// vine, glow lichen) matching their "no active side" fallback part.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MultipartWhen {
    And(Vec<MultipartWhen>),
    Or(Vec<MultipartWhen>),
    Match(BTreeMap<String, Vec<String>>),
}

impl MultipartWhen {
    fn from_json(value: &Value) -> AssetResult<Self> {
        let object = value.as_object().ok_or_else(|| {
            AssetError::InvalidBlockState("multipart `when` must be an object".to_owned())
        })?;
        if let Some(list) = object.get("OR") {
            return Ok(Self::Or(Self::parse_list(list)?));
        }
        if let Some(list) = object.get("AND") {
            return Ok(Self::And(Self::parse_list(list)?));
        }
        let mut matches = BTreeMap::new();
        for (name, value) in object {
            let value = value.as_str().ok_or_else(|| {
                AssetError::InvalidBlockState(format!(
                    "multipart `when` value for `{name}` must be a string"
                ))
            })?;
            let values = value.split('|').map(str::to_owned).collect();
            matches.insert(name.clone(), values);
        }
        Ok(Self::Match(matches))
    }

    fn parse_list(value: &Value) -> AssetResult<Vec<Self>> {
        let array = value.as_array().ok_or_else(|| {
            AssetError::InvalidBlockState("multipart `AND`/`OR` must be an array".to_owned())
        })?;
        array.iter().map(Self::from_json).collect()
    }

    pub fn matches(&self, properties: &BTreeMap<String, String>) -> bool {
        match self {
            Self::And(list) => list.iter().all(|when| when.matches(properties)),
            Self::Or(list) => list.iter().any(|when| when.matches(properties)),
            Self::Match(map) => map.iter().all(|(name, allowed)| {
                let actual = properties.get(name).map(String::as_str).unwrap_or("false");
                allowed.iter().any(|value| value == actual)
            }),
        }
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

fn parse_multipart(value: &Value) -> AssetResult<Vec<MultipartCase>> {
    let Some(multipart) = value.get("multipart") else {
        return Ok(Vec::new());
    };
    let cases = multipart.as_array().ok_or_else(|| {
        AssetError::InvalidBlockState("blockstate multipart must be an array".to_owned())
    })?;
    cases.iter().map(MultipartCase::from_json).collect()
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
// Huge-mushroom caps: skin on the top and all four sides, interior texture on the bottom.
// The huge-mushroom worldgen features only ever leave a cap face exposed when it is an
// outward rim face (skin) or the underside (interior), so this single fixed state renders
// identically to vanilla's per-position states once neighbouring cap blocks cull the shared
// interior faces. See docs/tactical for the parity note.
const MUSHROOM_CAP_FACES: &[(&str, &str)] = &[
    ("down", "false"),
    ("east", "true"),
    ("north", "true"),
    ("south", "true"),
    ("up", "true"),
    ("west", "true"),
];
// Mushroom stem: skin on all four sides, interior texture on top and bottom (both culled by
// the cap above and the ground below). Matches vanilla's stem block-state provider.
const MUSHROOM_STEM_FACES: &[(&str, &str)] = &[
    ("down", "false"),
    ("east", "true"),
    ("north", "true"),
    ("south", "true"),
    ("up", "false"),
    ("west", "true"),
];
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
const AGE_3: &[(&str, &str)] = &[("age", "3")];
const AGE_20: &[(&str, &str)] = &[("age", "20")];
const BAMBOO_TRUNK: &[(&str, &str)] = &[("age", "1"), ("leaves", "none"), ("stage", "0")];
const BAMBOO_TOP_SMALL: &[(&str, &str)] = &[("age", "1"), ("leaves", "small"), ("stage", "0")];
const BAMBOO_TOP_LARGE: &[(&str, &str)] = &[("age", "1"), ("leaves", "large"), ("stage", "0")];
const BAMBOO_FINAL_LARGE: &[(&str, &str)] = &[("age", "1"), ("leaves", "large"), ("stage", "1")];
const PICKLES_1_WATERLOGGED_TRUE: &[(&str, &str)] = &[("pickles", "1"), ("waterlogged", "true")];
const PICKLES_2_WATERLOGGED_TRUE: &[(&str, &str)] = &[("pickles", "2"), ("waterlogged", "true")];
const PICKLES_3_WATERLOGGED_TRUE: &[(&str, &str)] = &[("pickles", "3"), ("waterlogged", "true")];
const PICKLES_4_WATERLOGGED_TRUE: &[(&str, &str)] = &[("pickles", "4"), ("waterlogged", "true")];
const WATERLOGGED_TRUE: &[(&str, &str)] = &[("waterlogged", "true")];
const FACING_NORTH_WATERLOGGED_TRUE: &[(&str, &str)] =
    &[("facing", "north"), ("waterlogged", "true")];
const FACING_EAST_WATERLOGGED_TRUE: &[(&str, &str)] =
    &[("facing", "east"), ("waterlogged", "true")];
const FACING_SOUTH_WATERLOGGED_TRUE: &[(&str, &str)] =
    &[("facing", "south"), ("waterlogged", "true")];
const FACING_WEST_WATERLOGGED_TRUE: &[(&str, &str)] =
    &[("facing", "west"), ("waterlogged", "true")];
const VINE_UP: &[(&str, &str)] = &[
    ("up", "true"),
    ("north", "false"),
    ("east", "false"),
    ("south", "false"),
    ("west", "false"),
];
const VINE_NORTH: &[(&str, &str)] = &[
    ("up", "false"),
    ("north", "true"),
    ("east", "false"),
    ("south", "false"),
    ("west", "false"),
];
const VINE_EAST: &[(&str, &str)] = &[
    ("up", "false"),
    ("north", "false"),
    ("east", "true"),
    ("south", "false"),
    ("west", "false"),
];
const VINE_SOUTH: &[(&str, &str)] = &[
    ("up", "false"),
    ("north", "false"),
    ("east", "false"),
    ("south", "true"),
    ("west", "false"),
];
const VINE_WEST: &[(&str, &str)] = &[
    ("up", "false"),
    ("north", "false"),
    ("east", "false"),
    ("south", "false"),
    ("west", "true"),
];
const COCOA_AGE0_NORTH: &[(&str, &str)] = &[("age", "0"), ("facing", "north")];
const COCOA_AGE0_EAST: &[(&str, &str)] = &[("age", "0"), ("facing", "east")];
const COCOA_AGE0_SOUTH: &[(&str, &str)] = &[("age", "0"), ("facing", "south")];
const COCOA_AGE0_WEST: &[(&str, &str)] = &[("age", "0"), ("facing", "west")];
const COCOA_AGE1_NORTH: &[(&str, &str)] = &[("age", "1"), ("facing", "north")];
const COCOA_AGE1_EAST: &[(&str, &str)] = &[("age", "1"), ("facing", "east")];
const COCOA_AGE1_SOUTH: &[(&str, &str)] = &[("age", "1"), ("facing", "south")];
const COCOA_AGE1_WEST: &[(&str, &str)] = &[("age", "1"), ("facing", "west")];
const COCOA_AGE2_NORTH: &[(&str, &str)] = &[("age", "2"), ("facing", "north")];
const COCOA_AGE2_EAST: &[(&str, &str)] = &[("age", "2"), ("facing", "east")];
const COCOA_AGE2_SOUTH: &[(&str, &str)] = &[("age", "2"), ("facing", "south")];
const COCOA_AGE2_WEST: &[(&str, &str)] = &[("age", "2"), ("facing", "west")];
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
    (107, "minecraft:seagrass", EMPTY_PROPS),
    (108, "minecraft:tall_seagrass", HALF_LOWER),
    (109, "minecraft:tall_seagrass", HALF_UPPER),
    (110, "minecraft:kelp", AGE_20),
    (111, "minecraft:kelp_plant", EMPTY_PROPS),
    (112, "minecraft:tube_coral_block", EMPTY_PROPS),
    (113, "minecraft:brain_coral_block", EMPTY_PROPS),
    (114, "minecraft:bubble_coral_block", EMPTY_PROPS),
    (115, "minecraft:fire_coral_block", EMPTY_PROPS),
    (116, "minecraft:horn_coral_block", EMPTY_PROPS),
    (117, "minecraft:sea_pickle", PICKLES_1_WATERLOGGED_TRUE),
    (118, "minecraft:sea_pickle", PICKLES_2_WATERLOGGED_TRUE),
    (119, "minecraft:sea_pickle", PICKLES_3_WATERLOGGED_TRUE),
    (120, "minecraft:sea_pickle", PICKLES_4_WATERLOGGED_TRUE),
    (121, "minecraft:dark_oak_log", AXIS_Y),
    (122, "minecraft:dark_oak_leaves", EMPTY_PROPS),
    (123, "minecraft:brown_mushroom_block", MUSHROOM_CAP_FACES),
    (124, "minecraft:red_mushroom_block", MUSHROOM_CAP_FACES),
    (125, "minecraft:mushroom_stem", MUSHROOM_STEM_FACES),
    (126, "minecraft:acacia_log", AXIS_Y),
    (127, "minecraft:acacia_leaves", EMPTY_PROPS),
    (128, "minecraft:jungle_log", AXIS_Y),
    (129, "minecraft:jungle_leaves", EMPTY_PROPS),
    (130, "minecraft:bamboo", BAMBOO_TRUNK),
    (131, "minecraft:lily_pad", EMPTY_PROPS),
    (132, "minecraft:sweet_berry_bush", AGE_3),
    (133, "minecraft:allium", EMPTY_PROPS),
    (134, "minecraft:azure_bluet", EMPTY_PROPS),
    (135, "minecraft:red_tulip", EMPTY_PROPS),
    (136, "minecraft:orange_tulip", EMPTY_PROPS),
    (137, "minecraft:white_tulip", EMPTY_PROPS),
    (138, "minecraft:pink_tulip", EMPTY_PROPS),
    (139, "minecraft:oxeye_daisy", EMPTY_PROPS),
    (140, "minecraft:cornflower", EMPTY_PROPS),
    (141, "minecraft:lily_of_the_valley", EMPTY_PROPS),
    (142, "minecraft:lilac", HALF_LOWER),
    (143, "minecraft:lilac", HALF_UPPER),
    (144, "minecraft:rose_bush", HALF_LOWER),
    (145, "minecraft:rose_bush", HALF_UPPER),
    (146, "minecraft:peony", HALF_LOWER),
    (147, "minecraft:peony", HALF_UPPER),
    (148, "minecraft:sunflower", HALF_LOWER),
    (149, "minecraft:sunflower", HALF_UPPER),
    (150, "minecraft:blue_orchid", EMPTY_PROPS),
    (151, "minecraft:brown_mushroom", EMPTY_PROPS),
    (152, "minecraft:red_mushroom", EMPTY_PROPS),
    (153, "minecraft:blue_ice", EMPTY_PROPS),
    (154, "minecraft:pumpkin", EMPTY_PROPS),
    (155, "minecraft:melon", EMPTY_PROPS),
    (156, "minecraft:vine", VINE_EAST),
    (157, "minecraft:tall_grass", HALF_LOWER),
    (158, "minecraft:tall_grass", HALF_UPPER),
    (159, "minecraft:vine", VINE_UP),
    (160, "minecraft:vine", VINE_NORTH),
    (161, "minecraft:vine", VINE_SOUTH),
    (162, "minecraft:vine", VINE_WEST),
    (163, "minecraft:cocoa", COCOA_AGE0_NORTH),
    (164, "minecraft:cocoa", COCOA_AGE0_EAST),
    (165, "minecraft:cocoa", COCOA_AGE0_SOUTH),
    (166, "minecraft:cocoa", COCOA_AGE0_WEST),
    (167, "minecraft:cocoa", COCOA_AGE1_NORTH),
    (168, "minecraft:cocoa", COCOA_AGE1_EAST),
    (169, "minecraft:cocoa", COCOA_AGE1_SOUTH),
    (170, "minecraft:cocoa", COCOA_AGE1_WEST),
    (171, "minecraft:cocoa", COCOA_AGE2_NORTH),
    (172, "minecraft:cocoa", COCOA_AGE2_EAST),
    (173, "minecraft:cocoa", COCOA_AGE2_SOUTH),
    (174, "minecraft:cocoa", COCOA_AGE2_WEST),
    (175, "minecraft:bamboo", BAMBOO_TOP_SMALL),
    (176, "minecraft:bamboo", BAMBOO_TOP_LARGE),
    (177, "minecraft:bamboo", BAMBOO_FINAL_LARGE),
    (178, "minecraft:mossy_cobblestone", EMPTY_PROPS),
    (179, "minecraft:tube_coral", WATERLOGGED_TRUE),
    (180, "minecraft:brain_coral", WATERLOGGED_TRUE),
    (181, "minecraft:bubble_coral", WATERLOGGED_TRUE),
    (182, "minecraft:fire_coral", WATERLOGGED_TRUE),
    (183, "minecraft:horn_coral", WATERLOGGED_TRUE),
    (184, "minecraft:tube_coral_fan", WATERLOGGED_TRUE),
    (185, "minecraft:brain_coral_fan", WATERLOGGED_TRUE),
    (186, "minecraft:bubble_coral_fan", WATERLOGGED_TRUE),
    (187, "minecraft:fire_coral_fan", WATERLOGGED_TRUE),
    (188, "minecraft:horn_coral_fan", WATERLOGGED_TRUE),
    (
        189,
        "minecraft:tube_coral_wall_fan",
        FACING_NORTH_WATERLOGGED_TRUE,
    ),
    (
        190,
        "minecraft:tube_coral_wall_fan",
        FACING_EAST_WATERLOGGED_TRUE,
    ),
    (
        191,
        "minecraft:tube_coral_wall_fan",
        FACING_SOUTH_WATERLOGGED_TRUE,
    ),
    (
        192,
        "minecraft:tube_coral_wall_fan",
        FACING_WEST_WATERLOGGED_TRUE,
    ),
    (
        193,
        "minecraft:brain_coral_wall_fan",
        FACING_NORTH_WATERLOGGED_TRUE,
    ),
    (
        194,
        "minecraft:brain_coral_wall_fan",
        FACING_EAST_WATERLOGGED_TRUE,
    ),
    (
        195,
        "minecraft:brain_coral_wall_fan",
        FACING_SOUTH_WATERLOGGED_TRUE,
    ),
    (
        196,
        "minecraft:brain_coral_wall_fan",
        FACING_WEST_WATERLOGGED_TRUE,
    ),
    (
        197,
        "minecraft:bubble_coral_wall_fan",
        FACING_NORTH_WATERLOGGED_TRUE,
    ),
    (
        198,
        "minecraft:bubble_coral_wall_fan",
        FACING_EAST_WATERLOGGED_TRUE,
    ),
    (
        199,
        "minecraft:bubble_coral_wall_fan",
        FACING_SOUTH_WATERLOGGED_TRUE,
    ),
    (
        200,
        "minecraft:bubble_coral_wall_fan",
        FACING_WEST_WATERLOGGED_TRUE,
    ),
    (
        201,
        "minecraft:fire_coral_wall_fan",
        FACING_NORTH_WATERLOGGED_TRUE,
    ),
    (
        202,
        "minecraft:fire_coral_wall_fan",
        FACING_EAST_WATERLOGGED_TRUE,
    ),
    (
        203,
        "minecraft:fire_coral_wall_fan",
        FACING_SOUTH_WATERLOGGED_TRUE,
    ),
    (
        204,
        "minecraft:fire_coral_wall_fan",
        FACING_WEST_WATERLOGGED_TRUE,
    ),
    (
        205,
        "minecraft:horn_coral_wall_fan",
        FACING_NORTH_WATERLOGGED_TRUE,
    ),
    (
        206,
        "minecraft:horn_coral_wall_fan",
        FACING_EAST_WATERLOGGED_TRUE,
    ),
    (
        207,
        "minecraft:horn_coral_wall_fan",
        FACING_SOUTH_WATERLOGGED_TRUE,
    ),
    (
        208,
        "minecraft:horn_coral_wall_fan",
        FACING_WEST_WATERLOGGED_TRUE,
    ),
    (209, "minecraft:oak_planks", EMPTY_PROPS),
    (210, "minecraft:spruce_planks", EMPTY_PROPS),
    (211, "minecraft:cobblestone", EMPTY_PROPS),
    (212, "minecraft:stone_bricks", EMPTY_PROPS),
    (213, "minecraft:hay_block", AXIS_Y),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FilesystemAssetSource, MemoryAssetSource};

    #[cfg(not(target_arch = "wasm32"))]
    fn extracted_asset_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("reference/minecraft-1.17.1/extracted")
    }

    #[test]
    fn terrain_mvp_registry_names_current_generated_ids() {
        let registry = BlockStateRegistry::terrain_mvp();

        assert_eq!(registry.len(), 214);
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
            registry.id_for_key("minecraft:oak_planks"),
            Some(BlockStateId(209))
        );
        assert_eq!(
            registry.id_for_key("minecraft:hay_block[axis=y]"),
            Some(BlockStateId(213))
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
            registry.id_for_key("minecraft:mossy_cobblestone"),
            Some(BlockStateId(178))
        );
        assert_eq!(
            registry.id_for_key("minecraft:tube_coral[waterlogged=true]"),
            Some(BlockStateId(179))
        );
        assert_eq!(
            registry.id_for_key("minecraft:horn_coral[waterlogged=true]"),
            Some(BlockStateId(183))
        );
        assert_eq!(
            registry.id_for_key("minecraft:tube_coral_fan[waterlogged=true]"),
            Some(BlockStateId(184))
        );
        assert_eq!(
            registry.id_for_key("minecraft:fire_coral_fan[waterlogged=true]"),
            Some(BlockStateId(187))
        );
        assert_eq!(
            registry.id_for_key("minecraft:tube_coral_wall_fan[facing=north,waterlogged=true]"),
            Some(BlockStateId(189))
        );
        assert_eq!(
            registry.id_for_key("minecraft:brain_coral_wall_fan[facing=east,waterlogged=true]"),
            Some(BlockStateId(194))
        );
        assert_eq!(
            registry.by_id(BlockStateId(208)).unwrap().canonical_key(),
            "minecraft:horn_coral_wall_fan[facing=west,waterlogged=true]"
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
            registry.id_for_key("minecraft:tall_grass[half=lower]"),
            Some(BlockStateId(157))
        );
        assert_eq!(
            registry.by_id(BlockStateId(158)).unwrap().canonical_key(),
            "minecraft:tall_grass[half=upper]"
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
        assert_eq!(
            registry.id_for_key("minecraft:seagrass"),
            Some(BlockStateId(107))
        );
        assert_eq!(
            registry.id_for_key("minecraft:tall_seagrass[half=lower]"),
            Some(BlockStateId(108))
        );
        assert_eq!(
            registry.by_id(BlockStateId(109)).unwrap().canonical_key(),
            "minecraft:tall_seagrass[half=upper]"
        );
        assert_eq!(
            registry.id_for_key("minecraft:kelp[age=20]"),
            Some(BlockStateId(110))
        );
        assert_eq!(
            registry.id_for_key("minecraft:kelp_plant"),
            Some(BlockStateId(111))
        );
        assert_eq!(
            registry.id_for_key("minecraft:tube_coral_block"),
            Some(BlockStateId(112))
        );
        assert_eq!(
            registry.id_for_key("minecraft:brain_coral_block"),
            Some(BlockStateId(113))
        );
        assert_eq!(
            registry.id_for_key("minecraft:bubble_coral_block"),
            Some(BlockStateId(114))
        );
        assert_eq!(
            registry.id_for_key("minecraft:fire_coral_block"),
            Some(BlockStateId(115))
        );
        assert_eq!(
            registry.id_for_key("minecraft:horn_coral_block"),
            Some(BlockStateId(116))
        );
        assert_eq!(
            registry.id_for_key("minecraft:sea_pickle[pickles=1,waterlogged=true]"),
            Some(BlockStateId(117))
        );
        assert_eq!(
            registry.by_id(BlockStateId(120)).unwrap().canonical_key(),
            "minecraft:sea_pickle[pickles=4,waterlogged=true]"
        );
        assert_eq!(
            registry.id_for_key("minecraft:dark_oak_log[axis=y]"),
            Some(BlockStateId(121))
        );
        assert_eq!(
            registry.id_for_key("minecraft:dark_oak_leaves"),
            Some(BlockStateId(122))
        );
        assert_eq!(
            registry.id_for_key(
                "minecraft:brown_mushroom_block[down=false,east=true,north=true,south=true,up=true,west=true]"
            ),
            Some(BlockStateId(123))
        );
        assert_eq!(
            registry.id_for_key(
                "minecraft:red_mushroom_block[down=false,east=true,north=true,south=true,up=true,west=true]"
            ),
            Some(BlockStateId(124))
        );
        assert_eq!(
            registry.by_id(BlockStateId(125)).unwrap().canonical_key(),
            "minecraft:mushroom_stem[down=false,east=true,north=true,south=true,up=false,west=true]"
        );
        assert_eq!(
            registry.id_for_key("minecraft:acacia_log[axis=y]"),
            Some(BlockStateId(126))
        );
        assert_eq!(
            registry.id_for_key("minecraft:acacia_leaves"),
            Some(BlockStateId(127))
        );
        assert_eq!(
            registry.id_for_key("minecraft:jungle_log[axis=y]"),
            Some(BlockStateId(128))
        );
        assert_eq!(
            registry.id_for_key("minecraft:jungle_leaves"),
            Some(BlockStateId(129))
        );
        assert_eq!(
            registry.by_id(BlockStateId(130)).unwrap().canonical_key(),
            "minecraft:bamboo[age=1,leaves=none,stage=0]"
        );
        assert_eq!(
            registry.id_for_key("minecraft:bamboo[age=1,leaves=small,stage=0]"),
            Some(BlockStateId(175))
        );
        assert_eq!(
            registry.id_for_key("minecraft:bamboo[age=1,leaves=large,stage=0]"),
            Some(BlockStateId(176))
        );
        assert_eq!(
            registry.id_for_key("minecraft:bamboo[age=1,leaves=large,stage=1]"),
            Some(BlockStateId(177))
        );
        assert_eq!(
            registry.id_for_key("minecraft:lily_pad"),
            Some(BlockStateId(131))
        );
        assert_eq!(
            registry.id_for_key("minecraft:sweet_berry_bush[age=3]"),
            Some(BlockStateId(132))
        );
        assert_eq!(
            registry.id_for_key("minecraft:allium"),
            Some(BlockStateId(133))
        );
        assert_eq!(
            registry.id_for_key("minecraft:azure_bluet"),
            Some(BlockStateId(134))
        );
        assert_eq!(
            registry.id_for_key("minecraft:red_tulip"),
            Some(BlockStateId(135))
        );
        assert_eq!(
            registry.id_for_key("minecraft:orange_tulip"),
            Some(BlockStateId(136))
        );
        assert_eq!(
            registry.id_for_key("minecraft:white_tulip"),
            Some(BlockStateId(137))
        );
        assert_eq!(
            registry.id_for_key("minecraft:pink_tulip"),
            Some(BlockStateId(138))
        );
        assert_eq!(
            registry.id_for_key("minecraft:oxeye_daisy"),
            Some(BlockStateId(139))
        );
        assert_eq!(
            registry.id_for_key("minecraft:cornflower"),
            Some(BlockStateId(140))
        );
        assert_eq!(
            registry.id_for_key("minecraft:lily_of_the_valley"),
            Some(BlockStateId(141))
        );
        assert_eq!(
            registry.id_for_key("minecraft:lilac[half=lower]"),
            Some(BlockStateId(142))
        );
        assert_eq!(
            registry.id_for_key("minecraft:lilac[half=upper]"),
            Some(BlockStateId(143))
        );
        assert_eq!(
            registry.id_for_key("minecraft:rose_bush[half=lower]"),
            Some(BlockStateId(144))
        );
        assert_eq!(
            registry.id_for_key("minecraft:rose_bush[half=upper]"),
            Some(BlockStateId(145))
        );
        assert_eq!(
            registry.id_for_key("minecraft:peony[half=lower]"),
            Some(BlockStateId(146))
        );
        assert_eq!(
            registry.id_for_key("minecraft:peony[half=upper]"),
            Some(BlockStateId(147))
        );
        assert_eq!(
            registry.id_for_key("minecraft:sunflower[half=lower]"),
            Some(BlockStateId(148))
        );
        assert_eq!(
            registry.id_for_key("minecraft:sunflower[half=upper]"),
            Some(BlockStateId(149))
        );
        assert_eq!(
            registry.id_for_key("minecraft:blue_orchid"),
            Some(BlockStateId(150))
        );
        assert_eq!(
            registry.id_for_key("minecraft:brown_mushroom"),
            Some(BlockStateId(151))
        );
        assert_eq!(
            registry.id_for_key("minecraft:red_mushroom"),
            Some(BlockStateId(152))
        );
        assert_eq!(
            registry.id_for_key("minecraft:blue_ice"),
            Some(BlockStateId(153))
        );
        assert_eq!(
            registry.id_for_key("minecraft:pumpkin"),
            Some(BlockStateId(154))
        );
        assert_eq!(
            registry.id_for_key("minecraft:melon"),
            Some(BlockStateId(155))
        );
        assert_eq!(
            registry.id_for_key(
                "minecraft:vine[east=true,north=false,south=false,up=false,west=false]"
            ),
            Some(BlockStateId(156))
        );
        assert_eq!(
            registry.id_for_key(
                "minecraft:vine[east=false,north=false,south=false,up=true,west=false]"
            ),
            Some(BlockStateId(159))
        );
        assert_eq!(
            registry.id_for_key(
                "minecraft:vine[east=false,north=true,south=false,up=false,west=false]"
            ),
            Some(BlockStateId(160))
        );
        assert_eq!(
            registry.id_for_key(
                "minecraft:vine[east=false,north=false,south=true,up=false,west=false]"
            ),
            Some(BlockStateId(161))
        );
        assert_eq!(
            registry.id_for_key(
                "minecraft:vine[east=false,north=false,south=false,up=false,west=true]"
            ),
            Some(BlockStateId(162))
        );
        assert_eq!(
            registry.id_for_key("minecraft:cocoa[age=0,facing=north]"),
            Some(BlockStateId(163))
        );
        assert_eq!(
            registry.id_for_key("minecraft:cocoa[age=1,facing=east]"),
            Some(BlockStateId(168))
        );
        assert_eq!(
            registry.id_for_key("minecraft:cocoa[age=2,facing=west]"),
            Some(BlockStateId(174))
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

    #[test]
    fn multipart_blockstates_accept_exact_state_properties() {
        let mut source = MemoryAssetSource::new();
        source.insert_text(
            AssetPath::new("assets/minecraft/blockstates/bamboo.json"),
            r#"{"multipart":[{"when":{"age":"1"},"apply":{"model":"minecraft:block/bamboo1_age1"}}]}"#,
        );
        let index = BlockStateAssetIndex::load_namespace(&source, "minecraft").unwrap();
        let mut registry = BlockStateRegistry::new();
        registry
            .register(BlockStateRecord::new(
                BlockStateId(130),
                ResourceLocation::parse("minecraft:bamboo").unwrap(),
                [("age", "1"), ("leaves", "none"), ("stage", "0")],
            ))
            .unwrap();

        registry.validate_blockstate_assets(&index).unwrap();
        let record = registry.by_id(BlockStateId(130)).unwrap();
        let asset = index
            .get(&ResourceLocation::parse("minecraft:bamboo").unwrap())
            .unwrap();
        assert_eq!(record.asset_variant_key(asset), Some(String::new()));
    }

    #[test]
    fn age_states_use_base_asset_variant_when_vanilla_model_is_unkeyed() {
        let mut source = MemoryAssetSource::new();
        for block in ["cactus", "sugar_cane", "kelp"] {
            source.insert_text(
                AssetPath::new(format!("assets/minecraft/blockstates/{block}.json")),
                format!(r#"{{"variants":{{"":{{"model":"minecraft:block/{block}"}}}}}}"#),
            );
        }
        let index = BlockStateAssetIndex::load_namespace(&source, "minecraft").unwrap();
        let mut registry = BlockStateRegistry::new();
        for (id, block) in [(105, "cactus"), (106, "sugar_cane"), (107, "kelp")] {
            registry
                .register(BlockStateRecord::new(
                    BlockStateId(id),
                    ResourceLocation::parse(&format!("minecraft:{block}")).unwrap(),
                    [("age", "0")],
                ))
                .unwrap();
        }

        registry.validate_blockstate_assets(&index).unwrap();
        for block in ["cactus", "sugar_cane", "kelp"] {
            let asset = index
                .get(&ResourceLocation::parse(&format!("minecraft:{block}")).unwrap())
                .unwrap();
            let record = registry
                .by_id(
                    registry
                        .id_for_key(&format!("minecraft:{block}[age=0]"))
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(record.asset_variant_key(asset), Some(String::new()));
        }
    }

    #[test]
    fn unknown_properties_do_not_use_base_asset_variant() {
        let mut source = MemoryAssetSource::new();
        source.insert_text(
            AssetPath::new("assets/minecraft/blockstates/stone.json"),
            r#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#,
        );
        let index = BlockStateAssetIndex::load_namespace(&source, "minecraft").unwrap();
        let mut registry = BlockStateRegistry::new();
        registry
            .register(BlockStateRecord::new(
                BlockStateId(1),
                ResourceLocation::parse("minecraft:stone").unwrap(),
                [("bogus", "true")],
            ))
            .unwrap();

        let error = registry.validate_blockstate_assets(&index).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("minecraft:stone[bogus=true] references missing blockstate variant")
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn real_extracted_minecraft_blockstates_cover_terrain_mvp_registry() {
        let root = extracted_asset_root();
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
