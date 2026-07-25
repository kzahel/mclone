use std::collections::BTreeSet;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use mclone_assets::{
    AssetPackAvailability, AssetPackCatalog, AssetPackId, AssetPackSelection, TexturePresentation,
};
use serde::{Deserialize, Serialize};

pub const ASSET_PACK_PREFERENCE_SCHEMA: u32 = 1;
pub const ASSET_PACK_PREFERENCE_FILE_NAME: &str = "asset-packs.v1.json";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AssetPackPreference {
    enabled_ids: BTreeSet<AssetPackId>,
    presentation: TexturePresentation,
}

impl AssetPackPreference {
    pub fn new(enabled_ids: impl IntoIterator<Item = AssetPackId>) -> Self {
        Self::with_presentation(enabled_ids, TexturePresentation::Textured)
    }

    pub fn with_presentation(
        enabled_ids: impl IntoIterator<Item = AssetPackId>,
        presentation: TexturePresentation,
    ) -> Self {
        Self {
            enabled_ids: enabled_ids.into_iter().collect(),
            presentation,
        }
    }

    pub fn from_selection(selection: &AssetPackSelection) -> Self {
        Self::new(selection.enabled_ids().cloned())
    }

    pub fn from_profile(selection: &AssetPackSelection, presentation: TexturePresentation) -> Self {
        Self::with_presentation(selection.enabled_ids().cloned(), presentation)
    }

    pub fn enabled_ids(&self) -> impl Iterator<Item = &AssetPackId> {
        self.enabled_ids.iter()
    }

    pub const fn presentation(&self) -> TexturePresentation {
        self.presentation
    }

    pub fn reconcile(&self, catalog: &AssetPackCatalog) -> AssetPackPreferenceResolution {
        let mut applicable = Vec::new();
        let mut unavailable = Vec::new();
        let mut undiscovered = Vec::new();
        for id in &self.enabled_ids {
            match catalog.get(id) {
                Some(descriptor)
                    if descriptor.disableable
                        && descriptor.availability == AssetPackAvailability::Available =>
                {
                    applicable.push(id.clone());
                }
                Some(descriptor) if descriptor.disableable => unavailable.push(id.clone()),
                Some(_) => {}
                None => undiscovered.push(id.clone()),
            }
        }
        AssetPackPreferenceResolution {
            selection: AssetPackSelection::new(applicable),
            presentation: self.presentation,
            unavailable,
            undiscovered,
        }
    }

    pub fn after_successful_apply(
        &self,
        catalog: &AssetPackCatalog,
        selection: &AssetPackSelection,
        presentation: TexturePresentation,
    ) -> Self {
        let mut enabled_ids = selection.enabled_ids().cloned().collect::<BTreeSet<_>>();
        for id in &self.enabled_ids {
            if catalog.get(id).is_none_or(|descriptor| {
                descriptor.disableable && !descriptor.availability.is_available()
            }) {
                enabled_ids.insert(id.clone());
            }
        }
        Self {
            enabled_ids,
            presentation,
        }
    }

    pub fn to_json(&self) -> Result<String> {
        let document = AssetPackPreferenceDocument {
            schema: ASSET_PACK_PREFERENCE_SCHEMA,
            enabled_ids: self
                .enabled_ids
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect(),
            presentation: self.presentation,
        };
        serde_json::to_string_pretty(&document).context("serialize asset-pack preference")
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let document: AssetPackPreferenceDocument =
            serde_json::from_str(json).context("parse asset-pack preference")?;
        if document.schema != ASSET_PACK_PREFERENCE_SCHEMA {
            bail!(
                "unsupported asset-pack preference schema {}",
                document.schema
            );
        }
        let enabled_ids = document
            .enabled_ids
            .into_iter()
            .map(AssetPackId::try_new)
            .collect::<Result<Vec<_>, _>>()
            .context("validate persisted asset-pack id")?;
        Ok(Self::with_presentation(enabled_ids, document.presentation))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetPackPreferenceResolution {
    pub selection: AssetPackSelection,
    pub presentation: TexturePresentation,
    pub unavailable: Vec<AssetPackId>,
    pub undiscovered: Vec<AssetPackId>,
}

pub trait AssetPackPreferenceStorage {
    fn load(&self) -> Result<Option<AssetPackPreference>>;
    fn store(&self, preference: &AssetPackPreference) -> Result<()>;
    fn label(&self) -> &str;
}

#[derive(Debug, Serialize, Deserialize)]
struct AssetPackPreferenceDocument {
    schema: u32,
    enabled_ids: Vec<String>,
    #[serde(default)]
    presentation: TexturePresentation,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct FileAssetPackPreferenceStorage {
    path: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileAssetPackPreferenceStorage {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl AssetPackPreferenceStorage for FileAssetPackPreferenceStorage {
    fn load(&self) -> Result<Option<AssetPackPreference>> {
        let json = match std::fs::read_to_string(&self.path) {
            Ok(json) => json,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("read asset-pack preference {}", self.path.display())
                });
            }
        };
        AssetPackPreference::from_json(&json)
            .with_context(|| format!("load asset-pack preference {}", self.path.display()))
            .map(Some)
    }

    fn store(&self, preference: &AssetPackPreference) -> Result<()> {
        let parent = self
            .path
            .parent()
            .context("asset-pack preference path has no parent")?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create preference directory {}", parent.display()))?;
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, preference.to_json()?)
            .with_context(|| format!("write asset-pack preference {}", temporary.display()))?;
        #[cfg(target_os = "windows")]
        if self.path.exists() {
            std::fs::remove_file(&self.path).with_context(|| {
                format!(
                    "replace existing asset-pack preference {}",
                    self.path.display()
                )
            })?;
        }
        std::fs::rename(&temporary, &self.path)
            .with_context(|| format!("commit asset-pack preference {}", self.path.display()))
    }

    fn label(&self) -> &str {
        self.path.to_str().unwrap_or("native asset-pack preference")
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn native_asset_pack_preference_path(world_root: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MCLONE_ASSET_PACK_PREFERENCE_FILE") {
        return Some(PathBuf::from(path));
    }
    world_root.and_then(Path::parent).map(|root| {
        root.join("preferences")
            .join(ASSET_PACK_PREFERENCE_FILE_NAME)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_assets::{AssetPackDescriptor, AssetPackOrigin};

    fn catalog(authored_available: bool) -> AssetPackCatalog {
        let authored = AssetPackDescriptor::new(
            AssetPackId::new("mclone-authored"),
            "Mclone Original",
            AssetPackOrigin::FirstParty,
            10,
        )
        .unwrap();
        let authored = if authored_available {
            authored
        } else {
            authored.unavailable("not installed")
        };
        let fallback = AssetPackDescriptor::new(
            AssetPackId::new("mclone-generated-fallback"),
            "Generated",
            AssetPackOrigin::Generated,
            30,
        )
        .unwrap()
        .required();
        AssetPackCatalog::new([authored, fallback]).unwrap()
    }

    #[test]
    fn json_roundtrip_is_stable_and_sorted() {
        let preference =
            AssetPackPreference::new([AssetPackId::new("z-pack"), AssetPackId::new("a-pack")]);
        let json = preference.to_json().unwrap();
        assert!(json.find("a-pack").unwrap() < json.find("z-pack").unwrap());
        assert_eq!(AssetPackPreference::from_json(&json).unwrap(), preference);
    }

    #[test]
    fn unavailable_and_undiscovered_ids_survive_fallback_reconciliation() {
        let preference = AssetPackPreference::new([
            AssetPackId::new("mclone-authored"),
            AssetPackId::new("later-pack"),
        ]);
        let resolution = preference.reconcile(&catalog(false));
        assert_eq!(resolution.selection, AssetPackSelection::default());
        assert_eq!(
            resolution.unavailable,
            [AssetPackId::new("mclone-authored")]
        );
        assert_eq!(resolution.undiscovered, [AssetPackId::new("later-pack")]);

        let retained = preference.after_successful_apply(
            &catalog(false),
            &AssetPackSelection::default(),
            TexturePresentation::FlatColors,
        );
        assert_eq!(retained.presentation(), TexturePresentation::FlatColors);
        assert_eq!(
            retained.enabled_ids().collect::<Vec<_>>(),
            preference.enabled_ids().collect::<Vec<_>>()
        );
        let restored = retained.reconcile(&catalog(true));
        assert!(
            restored
                .selection
                .is_enabled(&AssetPackId::new("mclone-authored"))
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn file_adapter_survives_reopen() {
        let path = std::env::temp_dir().join(format!(
            "mclone-asset-pack-preference-{}.json",
            std::process::id()
        ));
        let storage = FileAssetPackPreferenceStorage::new(&path);
        let preference = AssetPackPreference::new([AssetPackId::new("mclone-authored")]);
        storage.store(&preference).unwrap();
        let reopened = FileAssetPackPreferenceStorage::new(&path);
        assert_eq!(reopened.load().unwrap(), Some(preference));
        let _ = std::fs::remove_file(path);
    }
}
