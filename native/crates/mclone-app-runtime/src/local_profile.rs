use std::fmt;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use mclone_protocol::{ClientIdentity, PlayerProfileId as ProtocolPlayerProfileId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const LOCAL_PLAYER_PROFILE_SCHEMA: u32 = 1;
pub const LOCAL_PLAYER_PROFILE_FILE_NAME: &str = "player-profile.v1.json";
pub const WEB_LOCAL_PLAYER_PROFILE_KEY: &str = "mclone.playerProfile.v1";
pub const WEB_ASSET_PACK_PREFERENCE_KEY: &str = "mclone.assetPacks.v1";
pub const DEFAULT_LOCAL_PLAYER_DISPLAY_NAME: &str = "Player";
pub const MAX_LOCAL_PLAYER_DISPLAY_NAME_BYTES: usize = 16;

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlayerProfileId([u8; 16]);

impl PlayerProfileId {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 16] {
        self.0
    }

    pub fn new_random() -> Self {
        Self(*Uuid::new_v4().as_bytes())
    }

    pub fn parse(value: &str) -> Result<Self> {
        let id = Uuid::parse_str(value).context("parse local player profile UUID")?;
        if id.is_nil() {
            bail!("local player profile UUID must not be nil");
        }
        Ok(Self(*id.as_bytes()))
    }

    pub fn hyphenated(self) -> String {
        Uuid::from_bytes(self.0).hyphenated().to_string()
    }
}

impl fmt::Debug for PlayerProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PlayerProfileId")
            .field(&self.hyphenated())
            .finish()
    }
}

impl fmt::Display for PlayerProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hyphenated())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalPlayerProfile {
    pub id: PlayerProfileId,
    pub display_name: String,
    pub created_at_unix_ms: u64,
}

impl LocalPlayerProfile {
    pub fn new(
        id: PlayerProfileId,
        display_name: impl Into<String>,
        created_at_unix_ms: u64,
    ) -> Result<Self> {
        Ok(Self {
            id,
            display_name: validate_display_name(display_name.into())?,
            created_at_unix_ms,
        })
    }

    pub fn random_default() -> Result<Self> {
        Self::new(
            PlayerProfileId::new_random(),
            DEFAULT_LOCAL_PLAYER_DISPLAY_NAME,
            now_unix_millis(),
        )
    }

    pub fn client_identity(&self) -> ClientIdentity {
        ClientIdentity::new(
            ProtocolPlayerProfileId::new(self.id.as_bytes()),
            self.display_name.clone(),
        )
        .expect("stored local player profile must satisfy protocol identity limits")
    }

    pub fn to_json(&self) -> Result<String> {
        let document = LocalPlayerProfileDocument {
            schema: LOCAL_PLAYER_PROFILE_SCHEMA,
            profile_id: self.id.hyphenated(),
            display_name: self.display_name.clone(),
            created_at_unix_ms: self.created_at_unix_ms,
        };
        serde_json::to_string_pretty(&document).context("serialize local player profile")
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let document: LocalPlayerProfileDocument =
            serde_json::from_str(json).context("parse local player profile")?;
        if document.schema != LOCAL_PLAYER_PROFILE_SCHEMA {
            bail!(
                "unsupported local player profile schema {}; expected {}",
                document.schema,
                LOCAL_PLAYER_PROFILE_SCHEMA
            );
        }
        Self::new(
            PlayerProfileId::parse(&document.profile_id)?,
            document.display_name,
            document.created_at_unix_ms,
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalPlayerProfileDocument {
    schema: u32,
    profile_id: String,
    display_name: String,
    created_at_unix_ms: u64,
}

pub trait LocalPlayerProfileStorage {
    fn load(&self) -> Result<Option<LocalPlayerProfile>>;
    fn store(&self, profile: &LocalPlayerProfile) -> Result<()>;
    fn delete(&self) -> Result<()>;
    fn label(&self) -> &str;
}

pub fn load_or_create_local_player_profile(
    storage: &dyn LocalPlayerProfileStorage,
) -> Result<LocalPlayerProfile> {
    if let Some(profile) = storage.load()? {
        return Ok(profile);
    }
    let profile = LocalPlayerProfile::random_default()?;
    storage.store(&profile)?;
    Ok(profile)
}

pub fn reset_local_player_profile(
    storage: &dyn LocalPlayerProfileStorage,
) -> Result<LocalPlayerProfile> {
    let profile = LocalPlayerProfile::random_default()?;
    storage.store(&profile)?;
    Ok(profile)
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct FileLocalPlayerProfileStorage {
    path: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileLocalPlayerProfileStorage {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl LocalPlayerProfileStorage for FileLocalPlayerProfileStorage {
    fn load(&self) -> Result<Option<LocalPlayerProfile>> {
        let json = match std::fs::read_to_string(&self.path) {
            Ok(json) => json,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("read local player profile {}", self.path.display()));
            }
        };
        LocalPlayerProfile::from_json(&json)
            .with_context(|| format!("load local player profile {}", self.path.display()))
            .map(Some)
    }

    fn store(&self, profile: &LocalPlayerProfile) -> Result<()> {
        let parent = self
            .path
            .parent()
            .context("local player profile path has no parent")?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create preference directory {}", parent.display()))?;
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, profile.to_json()?)
            .with_context(|| format!("write local player profile {}", temporary.display()))?;
        #[cfg(target_os = "windows")]
        if self.path.exists() {
            std::fs::remove_file(&self.path).with_context(|| {
                format!(
                    "replace existing local player profile {}",
                    self.path.display()
                )
            })?;
        }
        std::fs::rename(&temporary, &self.path)
            .with_context(|| format!("commit local player profile {}", self.path.display()))
    }

    fn delete(&self) -> Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error)
                .with_context(|| format!("delete local player profile {}", self.path.display())),
        }
    }

    fn label(&self) -> &str {
        self.path.to_str().unwrap_or("native local player profile")
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn native_local_player_profile_path(world_root: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MCLONE_PLAYER_PROFILE_FILE") {
        return Some(PathBuf::from(path));
    }
    world_root.and_then(Path::parent).map(|root| {
        root.join("preferences")
            .join(LOCAL_PLAYER_PROFILE_FILE_NAME)
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_or_create_native_local_player_profile(
    world_root: Option<&Path>,
) -> Result<LocalPlayerProfile> {
    let path = native_local_player_profile_path(world_root)
        .context("native local player profile requires an app world root or file override")?;
    load_or_create_local_player_profile(&FileLocalPlayerProfileStorage::new(path))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn reset_native_local_player_profile(world_root: Option<&Path>) -> Result<LocalPlayerProfile> {
    let path = native_local_player_profile_path(world_root)
        .context("native local player profile requires an app world root or file override")?;
    reset_local_player_profile(&FileLocalPlayerProfileStorage::new(path))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn factory_reset_native_local_preferences(
    world_root: Option<&Path>,
) -> Result<LocalPlayerProfile> {
    let world_root = world_root.context("native factory reset requires an app world root")?;
    if let Some(path) =
        crate::asset_pack_preferences::native_asset_pack_preference_path(Some(world_root))
    {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("delete asset-pack preference {}", path.display()));
            }
        }
    }
    let scenario_root =
        crate::scenario_content::native_managed_scenario_root_from_world_root(world_root);
    match std::fs::remove_dir_all(&scenario_root) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "delete managed scenario content {}",
                    scenario_root.display()
                )
            });
        }
    }
    reset_native_local_player_profile(Some(world_root))
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, Default)]
pub struct WebLocalPlayerProfileStorage;

#[cfg(target_arch = "wasm32")]
impl WebLocalPlayerProfileStorage {
    fn storage(self) -> Result<web_sys::Storage> {
        web_sys::window()
            .and_then(|window| window.local_storage().ok().flatten())
            .context("browser localStorage is unavailable")
    }
}

#[cfg(target_arch = "wasm32")]
impl LocalPlayerProfileStorage for WebLocalPlayerProfileStorage {
    fn load(&self) -> Result<Option<LocalPlayerProfile>> {
        let json = self
            .storage()?
            .get_item(WEB_LOCAL_PLAYER_PROFILE_KEY)
            .map_err(|error| anyhow::anyhow!("read browser localStorage: {error:?}"))?;
        json.map(|json| LocalPlayerProfile::from_json(&json))
            .transpose()
    }

    fn store(&self, profile: &LocalPlayerProfile) -> Result<()> {
        self.storage()?
            .set_item(WEB_LOCAL_PLAYER_PROFILE_KEY, &profile.to_json()?)
            .map_err(|error| anyhow::anyhow!("write browser localStorage: {error:?}"))
    }

    fn delete(&self) -> Result<()> {
        self.storage()?
            .remove_item(WEB_LOCAL_PLAYER_PROFILE_KEY)
            .map_err(|error| anyhow::anyhow!("delete browser localStorage: {error:?}"))
    }

    fn label(&self) -> &str {
        "browser localStorage mclone.playerProfile.v1"
    }
}

#[cfg(target_arch = "wasm32")]
pub fn load_or_create_web_local_player_profile() -> Result<LocalPlayerProfile> {
    load_or_create_local_player_profile(&WebLocalPlayerProfileStorage)
}

#[cfg(target_arch = "wasm32")]
pub fn reset_web_local_player_profile() -> Result<LocalPlayerProfile> {
    reset_local_player_profile(&WebLocalPlayerProfileStorage)
}

#[cfg(target_arch = "wasm32")]
pub fn factory_reset_web_local_preferences() -> Result<LocalPlayerProfile> {
    let storage = WebLocalPlayerProfileStorage.storage()?;
    storage
        .remove_item(WEB_ASSET_PACK_PREFERENCE_KEY)
        .map_err(|error| anyhow::anyhow!("delete browser asset-pack preference: {error:?}"))?;
    reset_web_local_player_profile()
}

fn validate_display_name(value: String) -> Result<String> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        bail!("local player display name must not be empty");
    }
    if value.len() > MAX_LOCAL_PLAYER_DISPLAY_NAME_BYTES {
        bail!(
            "local player display name is {} bytes; maximum is {}",
            value.len(),
            MAX_LOCAL_PLAYER_DISPLAY_NAME_BYTES
        );
    }
    if value.chars().any(char::is_control) {
        bail!("local player display name must not contain control characters");
    }
    Ok(value)
}

fn now_unix_millis() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now().max(0.0) as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_profile_path(label: &str) -> PathBuf {
        let unique = format!(
            "mclone-profile-{label}-{}-{}",
            std::process::id(),
            now_unix_millis()
        );
        std::env::temp_dir()
            .join(unique)
            .join(LOCAL_PLAYER_PROFILE_FILE_NAME)
    }

    #[test]
    fn profile_json_round_trip_is_stable() {
        let profile = LocalPlayerProfile::new(
            PlayerProfileId::parse("92ffc98e-32c0-4be4-839e-e03a27652b92").unwrap(),
            " Player ",
            123,
        )
        .unwrap();
        let json = profile.to_json().unwrap();
        assert_eq!(LocalPlayerProfile::from_json(&json).unwrap(), profile);
        assert!(json.contains("92ffc98e-32c0-4be4-839e-e03a27652b92"));
    }

    #[test]
    fn malformed_profile_is_not_silently_replaced() {
        let path = temp_profile_path("malformed");
        let storage = FileLocalPlayerProfileStorage::new(&path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{not json").unwrap();

        let error = load_or_create_local_player_profile(&storage).unwrap_err();

        assert!(error.to_string().contains("load local player profile"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{not json");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn file_storage_loads_one_stable_identity_and_reset_replaces_it() {
        let path = temp_profile_path("stable");
        let storage = FileLocalPlayerProfileStorage::new(&path);

        let first = load_or_create_local_player_profile(&storage).unwrap();
        let second = load_or_create_local_player_profile(&storage).unwrap();
        assert_eq!(first, second);

        let replacement = reset_local_player_profile(&storage).unwrap();
        assert_ne!(replacement.id, first.id);
        assert_eq!(storage.load().unwrap(), Some(replacement));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn native_profile_path_is_sibling_to_world_root() {
        let root = Path::new("/tmp/mclone/app-data/worlds");
        assert_eq!(
            native_local_player_profile_path(Some(root)).unwrap(),
            PathBuf::from("/tmp/mclone/app-data/preferences/player-profile.v1.json")
        );
    }

    #[test]
    fn native_factory_reset_replaces_profile_and_only_removes_registered_preferences() {
        let path = temp_profile_path("factory-reset");
        let app_root = path.parent().unwrap();
        let world_root = app_root.join("worlds");
        let preferences = app_root.join("preferences");
        let scenario_root = app_root.join("scenarios");
        let unrelated = app_root.join("keep-me.txt");
        std::fs::create_dir_all(&world_root).unwrap();
        std::fs::create_dir_all(&preferences).unwrap();
        std::fs::create_dir_all(&scenario_root).unwrap();
        std::fs::write(
            preferences.join(crate::asset_pack_preferences::ASSET_PACK_PREFERENCE_FILE_NAME),
            "preference",
        )
        .unwrap();
        std::fs::write(scenario_root.join("managed.bin"), "managed").unwrap();
        std::fs::write(&unrelated, "preserve").unwrap();

        let original = load_or_create_native_local_player_profile(Some(&world_root)).unwrap();
        let replacement = factory_reset_native_local_preferences(Some(&world_root)).unwrap();

        assert_ne!(replacement.id, original.id);
        assert_eq!(
            load_or_create_native_local_player_profile(Some(&world_root)).unwrap(),
            replacement
        );
        assert!(
            !preferences
                .join(crate::asset_pack_preferences::ASSET_PACK_PREFERENCE_FILE_NAME)
                .exists()
        );
        assert!(!scenario_root.exists());
        assert_eq!(std::fs::read_to_string(&unrelated).unwrap(), "preserve");
        let _ = std::fs::remove_dir_all(app_root);
    }

    #[test]
    fn display_name_uses_vanilla_sized_utf8_budget() {
        assert!(LocalPlayerProfile::new(PlayerProfileId::new_random(), "", 0).is_err());
        assert!(
            LocalPlayerProfile::new(PlayerProfileId::new_random(), "abcdefghijklmnop", 0).is_ok()
        );
        assert!(
            LocalPlayerProfile::new(PlayerProfileId::new_random(), "abcdefghijklmnopq", 0).is_err()
        );
    }
}
