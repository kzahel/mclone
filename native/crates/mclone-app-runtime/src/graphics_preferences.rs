#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use mclone_ui::GameLeafDetail;
use serde::{Deserialize, Serialize};

use crate::input_preferences::PreferenceKeyValueStore;

pub const GRAPHICS_PREFERENCE_STORAGE_KEY: &str = "mclone.graphics.preferences.v1";
pub const GRAPHICS_PREFERENCE_SCHEMA: u32 = 1;
pub const GRAPHICS_PREFERENCE_FILE_NAME: &str = "graphics-preferences.v1.json";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClientGraphicsPreferences {
    pub leaf_detail: GameLeafDetail,
}

impl ClientGraphicsPreferences {
    pub fn load(store: &impl PreferenceKeyValueStore) -> Result<Self> {
        let Some(json) = store.get(GRAPHICS_PREFERENCE_STORAGE_KEY)? else {
            return Ok(Self::default());
        };
        Self::from_json(&json).with_context(|| format!("load {}", store.label()))
    }

    pub fn store(&self, store: &impl PreferenceKeyValueStore) -> Result<()> {
        store.set(GRAPHICS_PREFERENCE_STORAGE_KEY, &self.to_json()?)
    }

    pub fn to_json(self) -> Result<String> {
        serde_json::to_string_pretty(&GraphicsPreferenceDocument {
            schema: GRAPHICS_PREFERENCE_SCHEMA,
            preferences: StoredGraphicsPreferences::from(self),
        })
        .context("serialize graphics preferences")
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let document: GraphicsPreferenceDocument =
            serde_json::from_str(json).context("parse graphics preferences")?;
        if document.schema != GRAPHICS_PREFERENCE_SCHEMA {
            bail!("unsupported graphics preference schema {}", document.schema);
        }
        Ok(document.preferences.into())
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct StoredGraphicsPreferences {
    leaf_detail: StoredLeafDetail,
}

impl From<ClientGraphicsPreferences> for StoredGraphicsPreferences {
    fn from(value: ClientGraphicsPreferences) -> Self {
        Self {
            leaf_detail: match value.leaf_detail {
                GameLeafDetail::Blocky => StoredLeafDetail::Blocky,
                GameLeafDetail::Bushy => StoredLeafDetail::Bushy,
            },
        }
    }
}

impl From<StoredGraphicsPreferences> for ClientGraphicsPreferences {
    fn from(value: StoredGraphicsPreferences) -> Self {
        Self {
            leaf_detail: match value.leaf_detail {
                StoredLeafDetail::Blocky => GameLeafDetail::Blocky,
                StoredLeafDetail::Bushy => GameLeafDetail::Bushy,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum StoredLeafDetail {
    #[default]
    Blocky,
    Bushy,
}

#[derive(Debug, Deserialize, Serialize)]
struct GraphicsPreferenceDocument {
    schema: u32,
    preferences: StoredGraphicsPreferences,
}

pub trait ClientGraphicsPreferenceStorage {
    fn load(&self) -> Result<Option<ClientGraphicsPreferences>>;
    fn store(&self, preferences: &ClientGraphicsPreferences) -> Result<()>;
    fn label(&self) -> &str;
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct FileClientGraphicsPreferenceStorage {
    path: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileClientGraphicsPreferenceStorage {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ClientGraphicsPreferenceStorage for FileClientGraphicsPreferenceStorage {
    fn load(&self) -> Result<Option<ClientGraphicsPreferences>> {
        let json = match std::fs::read_to_string(&self.path) {
            Ok(json) => json,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("read graphics preferences {}", self.path.display()));
            }
        };
        ClientGraphicsPreferences::from_json(&json)
            .with_context(|| format!("load graphics preferences {}", self.path.display()))
            .map(Some)
    }

    fn store(&self, preferences: &ClientGraphicsPreferences) -> Result<()> {
        let parent = self
            .path
            .parent()
            .context("graphics preference path has no parent")?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create preference directory {}", parent.display()))?;
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, preferences.to_json()?)
            .with_context(|| format!("write graphics preferences {}", temporary.display()))?;
        #[cfg(target_os = "windows")]
        if self.path.exists() {
            std::fs::remove_file(&self.path).with_context(|| {
                format!(
                    "replace existing graphics preferences {}",
                    self.path.display()
                )
            })?;
        }
        std::fs::rename(&temporary, &self.path)
            .with_context(|| format!("commit graphics preferences {}", self.path.display()))
    }

    fn label(&self) -> &str {
        self.path.to_str().unwrap_or("native graphics preferences")
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn native_graphics_preference_path(world_root: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MCLONE_GRAPHICS_PREFERENCE_FILE") {
        return Some(PathBuf::from(path));
    }
    world_root
        .and_then(Path::parent)
        .map(|root| root.join("preferences").join(GRAPHICS_PREFERENCE_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use super::*;

    #[derive(Default)]
    struct MemoryStore {
        values: RefCell<BTreeMap<String, String>>,
    }

    impl PreferenceKeyValueStore for MemoryStore {
        fn get(&self, key: &str) -> Result<Option<String>> {
            Ok(self.values.borrow().get(key).cloned())
        }

        fn set(&self, key: &str, value: &str) -> Result<()> {
            self.values
                .borrow_mut()
                .insert(key.to_owned(), value.to_owned());
            Ok(())
        }

        fn label(&self) -> &str {
            "memory graphics preferences"
        }
    }

    #[test]
    fn missing_graphics_preferences_default_to_blocky() {
        assert_eq!(
            ClientGraphicsPreferences::load(&MemoryStore::default()).unwrap(),
            ClientGraphicsPreferences::default()
        );
        assert_eq!(
            ClientGraphicsPreferences::default().leaf_detail,
            GameLeafDetail::Blocky
        );
    }

    #[test]
    fn graphics_preferences_round_trip_bushy_leaf_detail() {
        let store = MemoryStore::default();
        let preferences = ClientGraphicsPreferences {
            leaf_detail: GameLeafDetail::Bushy,
        };
        preferences.store(&store).unwrap();

        assert_eq!(
            ClientGraphicsPreferences::load(&store).unwrap(),
            preferences
        );
    }

    #[test]
    fn graphics_preferences_reject_unknown_schema_and_values() {
        assert!(
            ClientGraphicsPreferences::from_json(
                r#"{"schema":2,"preferences":{"leafDetail":"blocky"}}"#
            )
            .is_err()
        );
        assert!(
            ClientGraphicsPreferences::from_json(
                r#"{"schema":1,"preferences":{"leafDetail":"ultra"}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn graphics_preferences_default_missing_fields_for_schema_one() {
        assert_eq!(
            ClientGraphicsPreferences::from_json(r#"{"schema":1,"preferences":{}}"#).unwrap(),
            ClientGraphicsPreferences::default()
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn file_graphics_preferences_round_trip() {
        let root = std::env::temp_dir().join(format!(
            "mclone-graphics-preferences-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let path = root.join(GRAPHICS_PREFERENCE_FILE_NAME);
        let storage = FileClientGraphicsPreferenceStorage::new(&path);
        let preferences = ClientGraphicsPreferences {
            leaf_detail: GameLeafDetail::Bushy,
        };

        storage.store(&preferences).unwrap();
        assert_eq!(storage.load().unwrap(), Some(preferences));

        let _ = std::fs::remove_dir_all(root);
    }
}
