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
            let variant_key = record.variant_key();
            if !asset.variant_keys.is_empty() && !asset.variant_keys.contains(&variant_key) {
                return Err(AssetError::InvalidBlockState(format!(
                    "{} references missing blockstate variant `{variant_key}`",
                    record.canonical_key()
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
        let variant_keys = json
            .get("variants")
            .and_then(Value::as_object)
            .map(|variants| variants.keys().cloned().collect())
            .unwrap_or_default();
        let mut model_refs = BTreeSet::new();
        collect_model_refs(&json, &mut model_refs)?;
        Ok(Self {
            block,
            path,
            variant_keys,
            model_refs,
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

const EMPTY_PROPS: &[(&str, &str)] = &[];
const SNOWY_FALSE: &[(&str, &str)] = &[("snowy", "false")];

const TERRAIN_MVP_STATES: &[(u32, &str, &[(&str, &str)])] = &[
    (0, "minecraft:air", EMPTY_PROPS),
    (1, "minecraft:stone", EMPTY_PROPS),
    (2, "minecraft:water", EMPTY_PROPS),
    (3, "minecraft:bedrock", EMPTY_PROPS),
    (4, "minecraft:grass_block", SNOWY_FALSE),
    (5, "minecraft:dirt", EMPTY_PROPS),
    (6, "minecraft:sand", EMPTY_PROPS),
    (7, "minecraft:gravel", EMPTY_PROPS),
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
    (38, "minecraft:red_sand", EMPTY_PROPS),
    (39, "minecraft:ice", EMPTY_PROPS),
    (40, "minecraft:snow_block", EMPTY_PROPS),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FilesystemAssetSource, MemoryAssetSource};

    #[test]
    fn terrain_mvp_registry_names_current_generated_ids() {
        let registry = BlockStateRegistry::terrain_mvp();

        assert_eq!(registry.len(), 34);
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
            asset.variant_keys.into_iter().collect::<Vec<_>>(),
            vec!["snowy=false", "snowy=true"]
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
