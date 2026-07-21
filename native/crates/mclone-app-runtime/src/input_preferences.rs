use anyhow::Result;
use mclone_input::{TouchControlsMode, TouchInputSettings};

pub const TOUCH_LOOK_SENSITIVITY_STORAGE_KEY: &str = "mclone.web.lookSensitivity";
pub const TOUCH_CONTROLS_MODE_STORAGE_KEY: &str = "mclone.web.touchControlsMode";

/// Physical key/value storage used by the shared input-preference codec.
///
/// Implementations know how to reach a platform store. They do not know what
/// either key means or how a value is validated.
pub trait PreferenceKeyValueStore {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn label(&self) -> &str;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClientInputPreferences {
    pub touch_look_sensitivity: f32,
    pub touch_controls_mode: TouchControlsMode,
}

impl ClientInputPreferences {
    pub fn load(store: &impl PreferenceKeyValueStore) -> Result<Self> {
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
        })
    }

    pub fn store(self, store: &impl PreferenceKeyValueStore) -> Result<()> {
        let normalized = self.normalized();
        store.set(
            TOUCH_LOOK_SENSITIVITY_STORAGE_KEY,
            &normalized.touch_look_sensitivity.to_string(),
        )?;
        store.set(
            TOUCH_CONTROLS_MODE_STORAGE_KEY,
            touch_controls_mode_label(normalized.touch_controls_mode),
        )
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
        }
    }
}

impl Default for ClientInputPreferences {
    fn default() -> Self {
        Self {
            touch_look_sensitivity: TouchInputSettings::DEFAULT_LOOK_SENSITIVITY,
            touch_controls_mode: TouchControlsMode::Auto,
        }
    }
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
        };
        preferences.store(&store).unwrap();
        assert_eq!(ClientInputPreferences::load(&store).unwrap(), preferences);
    }
}
