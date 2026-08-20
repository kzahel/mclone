#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use mclone_core::TerrainLodPreset;
use mclone_ui::{
    GameFogColorMode, GameFogMode, GameFogSettings, GameFogWeatherInfluence, GameGrassDetail,
    GameLeafDetail,
};
use serde::{Deserialize, Serialize};

use crate::input_preferences::PreferenceKeyValueStore;

pub const GRAPHICS_PREFERENCE_STORAGE_KEY: &str = "mclone.graphics.preferences.v1";
pub const GRAPHICS_PREFERENCE_SCHEMA: u32 = 1;
pub const GRAPHICS_PREFERENCE_FILE_NAME: &str = "graphics-preferences.v1.json";

/// Shared host classification used only to resolve an unset graphics default.
///
/// Preset descriptors remain platform-independent; an explicit player or
/// launch selection always replaces this fallback.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ClientGraphicsPlatformProfile {
    #[default]
    NativeDesktopFlat,
    SteamOs,
    Web,
    FlatAndroid,
    DesktopOpenXr,
    AndroidXr,
}

impl ClientGraphicsPlatformProfile {
    pub const fn default_terrain_lod_preset(self) -> TerrainLodPreset {
        match self {
            Self::NativeDesktopFlat => TerrainLodPreset::High,
            Self::SteamOs | Self::Web | Self::DesktopOpenXr | Self::AndroidXr => {
                TerrainLodPreset::Medium
            }
            Self::FlatAndroid => TerrainLodPreset::Low,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClientGraphicsPreferences {
    pub leaf_detail: GameLeafDetail,
    pub grass_detail: GameGrassDetail,
    /// `None` resolves through the current platform profile.
    pub terrain_lod_preset: Option<TerrainLodPreset>,
    pub fog: GameFogSettings,
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
    grass_detail: StoredGrassDetail,
    #[serde(skip_serializing_if = "Option::is_none")]
    terrain_lod_preset: Option<StoredTerrainLodPreset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    terrain_presentation: Option<StoredTerrainPresentation>,
    fog: StoredFogSettings,
}

impl From<ClientGraphicsPreferences> for StoredGraphicsPreferences {
    fn from(value: ClientGraphicsPreferences) -> Self {
        Self {
            leaf_detail: match value.leaf_detail {
                GameLeafDetail::Blocky => StoredLeafDetail::Blocky,
                GameLeafDetail::Bushy => StoredLeafDetail::Bushy,
            },
            grass_detail: match value.grass_detail {
                GameGrassDetail::Off => StoredGrassDetail::Off,
                GameGrassDetail::Sparse => StoredGrassDetail::Sparse,
                GameGrassDetail::Lush => StoredGrassDetail::Lush,
                GameGrassDetail::Ultra => StoredGrassDetail::Ultra,
            },
            terrain_lod_preset: value.terrain_lod_preset.map(StoredTerrainLodPreset::from),
            terrain_presentation: None,
            fog: StoredFogSettings::from(value.fog),
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
            grass_detail: match value.grass_detail {
                StoredGrassDetail::Off => GameGrassDetail::Off,
                StoredGrassDetail::Sparse => GameGrassDetail::Sparse,
                StoredGrassDetail::Lush => GameGrassDetail::Lush,
                StoredGrassDetail::Ultra => GameGrassDetail::Ultra,
            },
            terrain_lod_preset: value.terrain_lod_preset.map(Into::into).or_else(|| {
                value.terrain_presentation.map(|legacy| match legacy {
                    StoredTerrainPresentation::ExactOnly => TerrainLodPreset::Off,
                    StoredTerrainPresentation::Composed => TerrainLodPreset::High,
                })
            }),
            fog: value.fog.into(),
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum StoredGrassDetail {
    #[default]
    Off,
    Sparse,
    Lush,
    Ultra,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum StoredTerrainLodPreset {
    Off,
    Low,
    Medium,
    High,
}

impl From<TerrainLodPreset> for StoredTerrainLodPreset {
    fn from(value: TerrainLodPreset) -> Self {
        match value {
            TerrainLodPreset::Off => Self::Off,
            TerrainLodPreset::Low => Self::Low,
            TerrainLodPreset::Medium => Self::Medium,
            TerrainLodPreset::High => Self::High,
        }
    }
}

impl From<StoredTerrainLodPreset> for TerrainLodPreset {
    fn from(value: StoredTerrainLodPreset) -> Self {
        match value {
            StoredTerrainLodPreset::Off => Self::Off,
            StoredTerrainLodPreset::Low => Self::Low,
            StoredTerrainLodPreset::Medium => Self::Medium,
            StoredTerrainLodPreset::High => Self::High,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum StoredTerrainPresentation {
    ExactOnly,
    #[serde(alias = "experimental")]
    Composed,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct StoredFogSettings {
    mode: StoredFogMode,
    color_mode: StoredFogColorMode,
    custom_color: [f32; 3],
    visibility_blocks: f32,
    classic_start: f32,
    coverage_guard: bool,
    guard_start: f32,
    ground_base_y: f32,
    ground_falloff_blocks: f32,
    max_opacity: f32,
    exponential_squared: bool,
    far_cull: bool,
    weather_influence: StoredFogWeatherInfluence,
}

impl Default for StoredFogSettings {
    fn default() -> Self {
        Self::from(GameFogSettings::default())
    }
}

impl From<GameFogSettings> for StoredFogSettings {
    fn from(value: GameFogSettings) -> Self {
        let value = value.normalized();
        Self {
            mode: match value.mode {
                GameFogMode::Off => StoredFogMode::Off,
                GameFogMode::Classic => StoredFogMode::Classic,
                GameFogMode::Natural => StoredFogMode::Natural,
                GameFogMode::GroundHaze => StoredFogMode::GroundHaze,
            },
            color_mode: match value.color_mode {
                GameFogColorMode::Sky => StoredFogColorMode::Sky,
                GameFogColorMode::Neutral => StoredFogColorMode::Neutral,
                GameFogColorMode::Warm => StoredFogColorMode::Warm,
                GameFogColorMode::Cool => StoredFogColorMode::Cool,
                GameFogColorMode::Custom => StoredFogColorMode::Custom,
            },
            custom_color: value.custom_color,
            visibility_blocks: value.visibility_blocks,
            classic_start: value.classic_start,
            coverage_guard: value.coverage_guard,
            guard_start: value.guard_start,
            ground_base_y: value.ground_base_y,
            ground_falloff_blocks: value.ground_falloff_blocks,
            max_opacity: value.max_opacity,
            exponential_squared: value.exponential_squared,
            far_cull: value.far_cull,
            weather_influence: match value.weather_influence {
                GameFogWeatherInfluence::Off => StoredFogWeatherInfluence::Off,
                GameFogWeatherInfluence::Subtle => StoredFogWeatherInfluence::Subtle,
                GameFogWeatherInfluence::Strong => StoredFogWeatherInfluence::Strong,
            },
        }
    }
}

impl From<StoredFogSettings> for GameFogSettings {
    fn from(value: StoredFogSettings) -> Self {
        Self {
            mode: match value.mode {
                StoredFogMode::Off => GameFogMode::Off,
                StoredFogMode::Classic => GameFogMode::Classic,
                StoredFogMode::Natural => GameFogMode::Natural,
                StoredFogMode::GroundHaze => GameFogMode::GroundHaze,
            },
            color_mode: match value.color_mode {
                StoredFogColorMode::Sky => GameFogColorMode::Sky,
                StoredFogColorMode::Neutral => GameFogColorMode::Neutral,
                StoredFogColorMode::Warm => GameFogColorMode::Warm,
                StoredFogColorMode::Cool => GameFogColorMode::Cool,
                StoredFogColorMode::Custom => GameFogColorMode::Custom,
            },
            custom_color: value.custom_color,
            visibility_blocks: value.visibility_blocks,
            classic_start: value.classic_start,
            coverage_guard: value.coverage_guard,
            guard_start: value.guard_start,
            ground_base_y: value.ground_base_y,
            ground_falloff_blocks: value.ground_falloff_blocks,
            max_opacity: value.max_opacity,
            exponential_squared: value.exponential_squared,
            far_cull: value.far_cull,
            weather_influence: match value.weather_influence {
                StoredFogWeatherInfluence::Off => GameFogWeatherInfluence::Off,
                StoredFogWeatherInfluence::Subtle => GameFogWeatherInfluence::Subtle,
                StoredFogWeatherInfluence::Strong => GameFogWeatherInfluence::Strong,
            },
        }
        .normalized()
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum StoredFogMode {
    Off,
    Classic,
    #[default]
    Natural,
    GroundHaze,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum StoredFogColorMode {
    #[default]
    Sky,
    Neutral,
    Warm,
    Cool,
    Custom,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum StoredFogWeatherInfluence {
    Off,
    #[default]
    Subtle,
    Strong,
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
        assert_eq!(
            ClientGraphicsPreferences::default().grass_detail,
            GameGrassDetail::Off
        );
        assert_eq!(
            ClientGraphicsPreferences::default().terrain_lod_preset,
            None
        );
        assert_eq!(
            ClientGraphicsPreferences::default().fog,
            GameFogSettings::default()
        );
    }

    #[test]
    fn graphics_preferences_round_trip_bushy_leaf_detail() {
        let store = MemoryStore::default();
        let preferences = ClientGraphicsPreferences {
            leaf_detail: GameLeafDetail::Bushy,
            grass_detail: GameGrassDetail::Lush,
            terrain_lod_preset: Some(TerrainLodPreset::Medium),
            fog: GameFogSettings {
                mode: GameFogMode::GroundHaze,
                visibility_blocks: 12_288.0,
                exponential_squared: true,
                far_cull: true,
                weather_influence: GameFogWeatherInfluence::Strong,
                ..GameFogSettings::default()
            },
        };
        preferences.store(&store).unwrap();

        assert_eq!(
            ClientGraphicsPreferences::load(&store).unwrap(),
            preferences
        );
        assert!(
            store
                .get(GRAPHICS_PREFERENCE_STORAGE_KEY)
                .unwrap()
                .expect("stored graphics preferences")
                .contains(r#""terrainLodPreset": "medium""#)
        );
    }

    #[test]
    fn graphics_preferences_migrate_legacy_terrain_presentation() {
        for (legacy, expected) in [
            ("exactOnly", TerrainLodPreset::Off),
            ("composed", TerrainLodPreset::High),
            ("experimental", TerrainLodPreset::High),
        ] {
            let json =
                format!(r#"{{"schema":1,"preferences":{{"terrainPresentation":"{legacy}"}}}}"#);
            let preferences = ClientGraphicsPreferences::from_json(&json).unwrap();
            assert_eq!(preferences.terrain_lod_preset, Some(expected));
            let migrated = preferences.to_json().unwrap();
            assert!(migrated.contains(&format!(
                r#""terrainLodPreset": "{}""#,
                expected.startup_label()
            )));
            assert!(!migrated.contains("terrainPresentation"));
        }
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
        assert!(
            ClientGraphicsPreferences::from_json(
                r#"{"schema":1,"preferences":{"grassDetail":"cinematic"}}"#
            )
            .is_err()
        );
        assert!(
            ClientGraphicsPreferences::from_json(
                r#"{"schema":1,"preferences":{"terrainPresentation":"cinematic"}}"#
            )
            .is_err()
        );
        assert!(
            ClientGraphicsPreferences::from_json(
                r#"{"schema":1,"preferences":{"terrainLodPreset":"cinematic"}}"#
            )
            .is_err()
        );
        assert!(
            ClientGraphicsPreferences::from_json(
                r#"{"schema":1,"preferences":{"fog":{"mode":"volumetric"}}}"#
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

    #[test]
    fn platform_profiles_only_choose_the_unset_default() {
        assert_eq!(
            ClientGraphicsPlatformProfile::NativeDesktopFlat.default_terrain_lod_preset(),
            TerrainLodPreset::High
        );
        assert_eq!(
            ClientGraphicsPlatformProfile::SteamOs.default_terrain_lod_preset(),
            TerrainLodPreset::Medium
        );
        assert_eq!(
            ClientGraphicsPlatformProfile::Web.default_terrain_lod_preset(),
            TerrainLodPreset::Medium
        );
        assert_eq!(
            ClientGraphicsPlatformProfile::FlatAndroid.default_terrain_lod_preset(),
            TerrainLodPreset::Low
        );
        assert_eq!(
            ClientGraphicsPlatformProfile::DesktopOpenXr.default_terrain_lod_preset(),
            TerrainLodPreset::Medium
        );
        assert_eq!(
            ClientGraphicsPlatformProfile::AndroidXr.default_terrain_lod_preset(),
            TerrainLodPreset::Medium
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
            grass_detail: GameGrassDetail::Ultra,
            terrain_lod_preset: Some(TerrainLodPreset::High),
            fog: GameFogSettings {
                mode: GameFogMode::Classic,
                classic_start: 0.6,
                coverage_guard: false,
                ..GameFogSettings::default()
            },
        };

        storage.store(&preferences).unwrap();
        assert_eq!(storage.load().unwrap(), Some(preferences));

        let _ = std::fs::remove_dir_all(root);
    }
}
