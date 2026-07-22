#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use mclone_input::{ControllerInputPreferences, TouchControlsMode, TouchInputSettings};
use serde::{Deserialize, Serialize};

pub const TOUCH_LOOK_SENSITIVITY_STORAGE_KEY: &str = "mclone.web.lookSensitivity";
pub const TOUCH_CONTROLS_MODE_STORAGE_KEY: &str = "mclone.web.touchControlsMode";
pub const INPUT_PREFERENCE_STORAGE_KEY: &str = "mclone.input.preferences.v1";
pub const INPUT_PREFERENCE_SCHEMA: u32 = 1;
pub const INPUT_PREFERENCE_FILE_NAME: &str = "input-preferences.v1.json";

/// Physical key/value storage used by the shared input-preference codec.
///
/// Implementations know how to reach a platform store. They do not know what
/// either key means or how a value is validated.
pub trait PreferenceKeyValueStore {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn label(&self) -> &str;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClientInputPreferences {
    pub touch_look_sensitivity: f32,
    pub touch_controls_mode: TouchControlsMode,
    pub controller: ControllerInputPreferences,
}

impl ClientInputPreferences {
    pub fn load(store: &impl PreferenceKeyValueStore) -> Result<Self> {
        if let Some(json) = store.get(INPUT_PREFERENCE_STORAGE_KEY)? {
            return Self::from_json(&json).with_context(|| format!("load {}", store.label()));
        }
        let touch_look_sensitivity = store
            .get(TOUCH_LOOK_SENSITIVITY_STORAGE_KEY)?
            .and_then(|value| value.parse::<f32>().ok())
            .unwrap_or(TouchInputSettings::DEFAULT_LOOK_SENSITIVITY);
        let touch_controls_mode = store
            .get(TOUCH_CONTROLS_MODE_STORAGE_KEY)?
            .as_deref()
            .and_then(parse_touch_controls_mode)
            .unwrap_or_default();
        Ok(Self {
            touch_look_sensitivity: TouchInputSettings {
                look_sensitivity: touch_look_sensitivity,
                ..TouchInputSettings::default()
            }
            .normalized()
            .look_sensitivity,
            touch_controls_mode,
            controller: ControllerInputPreferences::default(),
        })
    }

    pub fn store(&self, store: &impl PreferenceKeyValueStore) -> Result<()> {
        let normalized = self.clone().normalized();
        store.set(INPUT_PREFERENCE_STORAGE_KEY, &normalized.to_json()?)?;
        // Keep the two legacy browser keys synchronized during the migration
        // window. Older builds can still read touch settings, while every new
        // controller field remains owned by the versioned document.
        store.set(
            TOUCH_LOOK_SENSITIVITY_STORAGE_KEY,
            &normalized.touch_look_sensitivity.to_string(),
        )?;
        store.set(
            TOUCH_CONTROLS_MODE_STORAGE_KEY,
            touch_controls_mode_label(normalized.touch_controls_mode),
        )
    }

    pub fn to_json(&self) -> Result<String> {
        let document = InputPreferenceDocument {
            schema: INPUT_PREFERENCE_SCHEMA,
            preferences: self.clone().normalized(),
        };
        serde_json::to_string_pretty(&document).context("serialize input preferences")
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let document: InputPreferenceDocument =
            serde_json::from_str(json).context("parse input preferences")?;
        if document.schema != INPUT_PREFERENCE_SCHEMA {
            bail!("unsupported input preference schema {}", document.schema);
        }
        Ok(document.preferences.normalized())
    }

    pub fn normalized(self) -> Self {
        Self {
            touch_look_sensitivity: TouchInputSettings {
                look_sensitivity: self.touch_look_sensitivity,
                ..TouchInputSettings::default()
            }
            .normalized()
            .look_sensitivity,
            touch_controls_mode: self.touch_controls_mode,
            controller: self.controller.normalized(),
        }
    }
}

impl Default for ClientInputPreferences {
    fn default() -> Self {
        Self {
            touch_look_sensitivity: TouchInputSettings::DEFAULT_LOOK_SENSITIVITY,
            touch_controls_mode: TouchControlsMode::Auto,
            controller: ControllerInputPreferences::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct InputPreferenceDocument {
    schema: u32,
    preferences: ClientInputPreferences,
}

pub trait ClientInputPreferenceStorage {
    fn load(&self) -> Result<Option<ClientInputPreferences>>;
    fn store(&self, preferences: &ClientInputPreferences) -> Result<()>;
    fn label(&self) -> &str;
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct FileClientInputPreferenceStorage {
    path: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileClientInputPreferenceStorage {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ClientInputPreferenceStorage for FileClientInputPreferenceStorage {
    fn load(&self) -> Result<Option<ClientInputPreferences>> {
        let json = match std::fs::read_to_string(&self.path) {
            Ok(json) => json,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("read input preferences {}", self.path.display()));
            }
        };
        ClientInputPreferences::from_json(&json)
            .with_context(|| format!("load input preferences {}", self.path.display()))
            .map(Some)
    }

    fn store(&self, preferences: &ClientInputPreferences) -> Result<()> {
        let parent = self
            .path
            .parent()
            .context("input preference path has no parent")?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create preference directory {}", parent.display()))?;
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, preferences.to_json()?)
            .with_context(|| format!("write input preferences {}", temporary.display()))?;
        #[cfg(target_os = "windows")]
        if self.path.exists() {
            std::fs::remove_file(&self.path).with_context(|| {
                format!("replace existing input preferences {}", self.path.display())
            })?;
        }
        std::fs::rename(&temporary, &self.path)
            .with_context(|| format!("commit input preferences {}", self.path.display()))
    }

    fn label(&self) -> &str {
        self.path.to_str().unwrap_or("native input preferences")
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn native_input_preference_path(world_root: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MCLONE_INPUT_PREFERENCE_FILE") {
        return Some(PathBuf::from(path));
    }
    world_root
        .and_then(Path::parent)
        .map(|root| root.join("preferences").join(INPUT_PREFERENCE_FILE_NAME))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_native_input_preferences(world_root: Option<&Path>) -> Result<ClientInputPreferences> {
    let Some(path) = native_input_preference_path(world_root) else {
        return Ok(ClientInputPreferences::default());
    };
    Ok(FileClientInputPreferenceStorage::new(path)
        .load()?
        .unwrap_or_default())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn store_native_input_preferences(
    world_root: Option<&Path>,
    preferences: &ClientInputPreferences,
) -> Result<bool> {
    let Some(path) = native_input_preference_path(world_root) else {
        return Ok(false);
    };
    FileClientInputPreferenceStorage::new(path).store(preferences)?;
    Ok(true)
}

pub const fn touch_controls_mode_label(mode: TouchControlsMode) -> &'static str {
    match mode {
        TouchControlsMode::Auto => "auto",
        TouchControlsMode::On => "on",
        TouchControlsMode::Off => "off",
    }
}

pub fn parse_touch_controls_mode(value: &str) -> Option<TouchControlsMode> {
    match value {
        "auto" => Some(TouchControlsMode::Auto),
        "on" => Some(TouchControlsMode::On),
        "off" => Some(TouchControlsMode::Off),
        _ => None,
    }
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
            "memory input preferences"
        }
    }

    #[test]
    fn missing_and_malformed_values_use_shared_defaults() {
        let store = MemoryStore::default();
        assert_eq!(
            ClientInputPreferences::load(&store).unwrap(),
            ClientInputPreferences::default()
        );

        store
            .set(TOUCH_LOOK_SENSITIVITY_STORAGE_KEY, "not-a-number")
            .unwrap();
        store
            .set(TOUCH_CONTROLS_MODE_STORAGE_KEY, "sometimes")
            .unwrap();
        assert_eq!(
            ClientInputPreferences::load(&store).unwrap(),
            ClientInputPreferences::default()
        );
    }

    #[test]
    fn non_finite_and_out_of_range_sensitivity_is_normalized_by_shared_input_policy() {
        let store = MemoryStore::default();
        store
            .set(TOUCH_LOOK_SENSITIVITY_STORAGE_KEY, "inf")
            .unwrap();
        assert_eq!(
            ClientInputPreferences::load(&store)
                .unwrap()
                .touch_look_sensitivity,
            TouchInputSettings::DEFAULT_LOOK_SENSITIVITY
        );

        store
            .set(TOUCH_LOOK_SENSITIVITY_STORAGE_KEY, "999")
            .unwrap();
        assert_eq!(
            ClientInputPreferences::load(&store)
                .unwrap()
                .touch_look_sensitivity,
            TouchInputSettings::MAX_LOOK_SENSITIVITY
        );
    }

    #[test]
    fn typed_preferences_round_trip_through_opaque_key_value_storage() {
        let store = MemoryStore::default();
        let preferences = ClientInputPreferences {
            touch_look_sensitivity: 4.75,
            touch_controls_mode: TouchControlsMode::Off,
            controller: ControllerInputPreferences {
                layout_override: Some(mclone_input::ControllerLayoutFamily::SteamDeckLike),
                ..ControllerInputPreferences::default()
            },
        };
        preferences.store(&store).unwrap();
        assert_eq!(ClientInputPreferences::load(&store).unwrap(), preferences);
    }

    #[test]
    fn versioned_document_defaults_new_fields_and_rejects_future_schema() {
        let legacy_document = r#"{
            "schema": 1,
            "preferences": {
                "touchLookSensitivity": 3.25,
                "touchControlsMode": "auto"
            }
        }"#;
        let preferences = ClientInputPreferences::from_json(legacy_document).unwrap();
        assert_eq!(preferences.touch_look_sensitivity, 3.25);
        assert_eq!(
            preferences.controller,
            ControllerInputPreferences::default()
        );

        assert!(
            ClientInputPreferences::from_json(r#"{"schema":2,"preferences":{}}"#,)
                .unwrap_err()
                .to_string()
                .contains("unsupported input preference schema 2")
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_file_storage_round_trips_atomically() {
        let path = std::env::temp_dir().join(format!(
            "mclone-input-preferences-{}.json",
            std::process::id()
        ));
        let storage = FileClientInputPreferenceStorage::new(&path);
        let preferences = ClientInputPreferences {
            controller: ControllerInputPreferences {
                preferred_input: mclone_input::InputSchemePreset::Gamepad,
                ..ControllerInputPreferences::default()
            },
            ..ClientInputPreferences::default()
        };
        storage.store(&preferences).unwrap();
        assert_eq!(storage.load().unwrap(), Some(preferences));
        assert!(!path.with_extension("json.tmp").exists());
        let _ = std::fs::remove_file(path);
    }
}
