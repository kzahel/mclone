use std::collections::{BTreeMap, BTreeSet};

use mclone_core::BlockStateId;
use serde::Deserialize;

use crate::{
    AssetError, AssetPath, AssetResult, AssetSource, BlockStateRegistry, FirstPartyVisualClass,
    ResourceLocation,
};

pub const FIRST_PARTY_VISUAL_CATALOG_PATH: &str = "assets/mclone/visuals/blocks.v1.json";
pub const FIRST_PARTY_MISSING_REGISTRY_PATH: &str = "assets/mclone/missing/registry.v1.json";
pub const FIRST_PARTY_AUDIO_POLICY_PATH: &str = "assets/mclone/audio/missing-policy.v1.json";
pub const FIRST_PARTY_VISUAL_SCHEMA: &str = "mclone-visuals-v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirstPartyVisualDefinition {
    pub state_id: BlockStateId,
    pub state: String,
    pub class: FirstPartyVisualClass,
    pub material: Option<ResourceLocation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirstPartyVisualCatalog {
    definitions: BTreeMap<BlockStateId, FirstPartyVisualDefinition>,
}

impl FirstPartyVisualCatalog {
    pub fn load(source: &impl AssetSource) -> AssetResult<Self> {
        let path = AssetPath::new(FIRST_PARTY_VISUAL_CATALOG_PATH);
        let bytes = source
            .read(&path)?
            .ok_or_else(|| AssetError::MissingAsset(path.clone()))?;
        let raw: RawVisualCatalog =
            serde_json::from_slice(&bytes).map_err(|source| AssetError::Json { path, source })?;
        if raw.asset_schema != FIRST_PARTY_VISUAL_SCHEMA {
            return Err(AssetError::InvalidAssetPack(format!(
                "first-party visual catalog schema `{}` does not match `{FIRST_PARTY_VISUAL_SCHEMA}`",
                raw.asset_schema
            )));
        }

        let registry = BlockStateRegistry::terrain_mvp();
        let canonical = registry.records().collect::<Vec<_>>();
        if raw.block_visuals.len() != canonical.len() {
            return Err(AssetError::InvalidAssetPack(format!(
                "first-party visual catalog has {} entries; expected {}",
                raw.block_visuals.len(),
                canonical.len()
            )));
        }

        let mut definitions = BTreeMap::new();
        for entry in raw.block_visuals {
            let state_id = BlockStateId(entry.state_id);
            let Some(record) = registry.by_id(state_id) else {
                return Err(AssetError::InvalidAssetPack(format!(
                    "first-party visual catalog references unknown state id {}",
                    entry.state_id
                )));
            };
            if entry.state != record.canonical_key() {
                return Err(AssetError::InvalidAssetPack(format!(
                    "first-party state {} names `{}`; expected `{}`",
                    entry.state_id,
                    entry.state,
                    record.canonical_key()
                )));
            }
            let material = entry
                .material
                .as_deref()
                .map(ResourceLocation::parse)
                .transpose()?;
            if entry.class == FirstPartyVisualClass::Empty && material.is_some()
                || entry.class != FirstPartyVisualClass::Empty && material.is_none()
            {
                return Err(AssetError::InvalidAssetPack(format!(
                    "first-party state {} has an invalid material/class combination",
                    entry.state_id
                )));
            }
            let definition = FirstPartyVisualDefinition {
                state_id,
                state: entry.state,
                class: entry.class,
                material,
            };
            if definitions.insert(state_id, definition).is_some() {
                return Err(AssetError::InvalidAssetPack(format!(
                    "duplicate first-party state id {}",
                    entry.state_id
                )));
            }
        }
        Ok(Self { definitions })
    }

    pub fn get(&self, state_id: BlockStateId) -> Option<&FirstPartyVisualDefinition> {
        self.definitions.get(&state_id)
    }

    pub fn definitions(&self) -> impl Iterator<Item = &FirstPartyVisualDefinition> {
        self.definitions.values()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingAssetRegistryEntry {
    pub code: String,
    pub path: AssetPath,
    pub resource_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingAssetRegistry {
    entries: BTreeMap<AssetPath, MissingAssetRegistryEntry>,
}

impl MissingAssetRegistry {
    pub fn load(source: &impl AssetSource) -> AssetResult<Self> {
        let path = AssetPath::new(FIRST_PARTY_MISSING_REGISTRY_PATH);
        let bytes = source
            .read(&path)?
            .ok_or_else(|| AssetError::MissingAsset(path.clone()))?;
        let raw: RawMissingRegistry =
            serde_json::from_slice(&bytes).map_err(|source| AssetError::Json { path, source })?;
        let mut entries = BTreeMap::new();
        let mut codes = BTreeSet::new();
        for raw_entry in raw.entries {
            let entry = MissingAssetRegistryEntry {
                code: raw_entry.code,
                path: AssetPath::try_new(raw_entry.path)?,
                resource_id: raw_entry.resource_id,
            };
            if !codes.insert(entry.code.clone())
                || entries.insert(entry.path.clone(), entry).is_some()
            {
                return Err(AssetError::InvalidAssetPack(
                    "duplicate generated missing-resource registry entry".to_owned(),
                ));
            }
        }
        Ok(Self { entries })
    }

    pub fn get(&self, path: &AssetPath) -> Option<&MissingAssetRegistryEntry> {
        self.entries.get(path)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirstPartyAudioPolicy {
    pub suppressed: BTreeSet<AssetPath>,
}

impl FirstPartyAudioPolicy {
    pub fn load(source: &impl AssetSource) -> AssetResult<Self> {
        let path = AssetPath::new(FIRST_PARTY_AUDIO_POLICY_PATH);
        let bytes = source
            .read(&path)?
            .ok_or_else(|| AssetError::MissingAsset(path.clone()))?;
        let raw: RawAudioPolicy =
            serde_json::from_slice(&bytes).map_err(|source| AssetError::Json { path, source })?;
        if raw.schema_version != 1 {
            return Err(AssetError::InvalidAssetPack(format!(
                "unsupported first-party audio policy schema {}",
                raw.schema_version
            )));
        }
        Ok(Self {
            suppressed: raw
                .suppressed
                .into_iter()
                .map(AssetPath::try_new)
                .collect::<AssetResult<_>>()?,
        })
    }
}

#[derive(Deserialize)]
struct RawVisualCatalog {
    asset_schema: String,
    block_visuals: Vec<RawVisualDefinition>,
}

#[derive(Deserialize)]
struct RawVisualDefinition {
    state_id: u32,
    state: String,
    class: FirstPartyVisualClass,
    material: Option<String>,
}

#[derive(Deserialize)]
struct RawMissingRegistry {
    entries: Vec<RawMissingRegistryEntry>,
}

#[derive(Deserialize)]
struct RawMissingRegistryEntry {
    code: String,
    path: String,
    resource_id: String,
}

#[derive(Deserialize)]
struct RawAudioPolicy {
    schema_version: u32,
    suppressed: Vec<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{MemoryAssetSource, canonical_first_party_asset_inventory};

    #[test]
    fn first_party_catalog_validates_the_complete_canonical_registry() {
        let inventory = canonical_first_party_asset_inventory();
        let entries = inventory
            .block_visuals
            .iter()
            .map(|visual| {
                json!({
                    "state_id": visual.state.id.0,
                    "state": visual.state.canonical_key(),
                    "class": match visual.class {
                        FirstPartyVisualClass::Empty => "empty",
                        FirstPartyVisualClass::Solid => "solid",
                        FirstPartyVisualClass::Slab => "slab",
                        FirstPartyVisualClass::Stair => "stair",
                        FirstPartyVisualClass::CrossedPlane => "crossed_plane",
                        FirstPartyVisualClass::Flat => "flat",
                        FirstPartyVisualClass::Fluid => "fluid",
                    },
                    "material": visual.material.as_ref().map(ToString::to_string),
                })
            })
            .collect::<Vec<_>>();
        let mut source = MemoryAssetSource::new();
        source.insert_text(
            AssetPath::new(FIRST_PARTY_VISUAL_CATALOG_PATH),
            json!({
                "asset_schema": FIRST_PARTY_VISUAL_SCHEMA,
                "block_visuals": entries,
            })
            .to_string(),
        );

        let catalog = FirstPartyVisualCatalog::load(&source).unwrap();
        assert_eq!(catalog.definitions().count(), 221);
        assert_eq!(
            catalog
                .get(BlockStateId(1))
                .unwrap()
                .material
                .as_ref()
                .unwrap()
                .to_string(),
            "mclone:block/stone"
        );
    }
}
