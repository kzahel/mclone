use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::io;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(not(target_arch = "wasm32"))]
use mclone_server::SqliteWorldStore;
use mclone_server::WorldGenerationProfile;
use serde::{Deserialize, Serialize};

pub const LOCAL_WORLD_CATALOG_SCHEMA_VERSION: u32 = 1;
pub const LOCAL_WORLD_TARGET_MINECRAFT_VERSION: &str = "1.17.1";
pub const LOCAL_WORLD_ID_MAX_LEN: usize = 64;
pub const LOCAL_WORLD_DISPLAY_NAME_MAX_CHARS: usize = 64;
pub const NATIVE_WORLD_METADATA_FILE: &str = "world.json";
pub const NATIVE_WORLD_BACKEND_LABEL: &str = "native-sqlite";

const DEFAULT_LOCAL_WORLD_ID: &str = "world";

pub type WorldCatalogResult<T> = Result<T, WorldCatalogError>;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LocalWorldId(String);

impl LocalWorldId {
    pub fn new(value: impl Into<String>) -> WorldCatalogResult<Self> {
        let value = value.into();
        validate_local_world_id(&value)?;
        Ok(Self(value))
    }

    pub fn from_display_name(display_name: &str) -> Self {
        Self(slug_from_display_name(display_name))
    }

    pub fn available_from_display_name<'a>(
        display_name: &str,
        existing_ids: impl IntoIterator<Item = &'a LocalWorldId>,
    ) -> Self {
        let existing = existing_ids
            .into_iter()
            .map(LocalWorldId::as_str)
            .collect::<BTreeSet<_>>();
        let base = slug_from_display_name(display_name);
        if !existing.contains(base.as_str()) {
            return Self(base);
        }

        for suffix_index in 2..=u32::MAX {
            let candidate = suffixed_world_id_candidate(&base, suffix_index);
            if !existing.contains(candidate.as_str()) {
                return Self(candidate);
            }
        }

        unreachable!("exhausted local world id suffix space");
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for LocalWorldId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for LocalWorldId {
    type Err = WorldCatalogError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for LocalWorldId {
    type Error = WorldCatalogError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for LocalWorldId {
    type Error = WorldCatalogError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalWorldCreateOptions {
    pub display_name: String,
    pub seed: i64,
    #[serde(default)]
    pub world_generation_profile: WorldGenerationProfile,
    pub requested_id: Option<LocalWorldId>,
}

impl LocalWorldCreateOptions {
    pub fn new(display_name: impl Into<String>, seed: i64) -> WorldCatalogResult<Self> {
        Ok(Self {
            display_name: normalize_display_name(display_name.into())?,
            seed,
            world_generation_profile: WorldGenerationProfile::default(),
            requested_id: None,
        })
    }

    pub fn with_requested_id(mut self, id: LocalWorldId) -> Self {
        self.requested_id = Some(id);
        self
    }

    pub fn with_world_generation_profile(mut self, profile: WorldGenerationProfile) -> Self {
        self.world_generation_profile = profile;
        self
    }

    pub fn resolve_id<'a>(
        &self,
        existing_ids: impl IntoIterator<Item = &'a LocalWorldId>,
    ) -> WorldCatalogResult<LocalWorldId> {
        if let Some(id) = &self.requested_id {
            return Ok(id.clone());
        }

        Ok(LocalWorldId::available_from_display_name(
            &self.display_name,
            existing_ids,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalWorldSummary {
    pub id: LocalWorldId,
    pub display_name: String,
    pub seed: i64,
    #[serde(default)]
    pub world_generation_profile: WorldGenerationProfile,
    pub created_unix_millis: u64,
    pub last_played_unix_millis: Option<u64>,
    pub storage_schema_version: u32,
    pub target_minecraft_version: String,
    pub mclone_version: Option<String>,
    pub backend_label: Option<String>,
    pub locked: bool,
    pub compatible: bool,
}

impl LocalWorldSummary {
    pub fn new(
        id: LocalWorldId,
        display_name: impl Into<String>,
        seed: i64,
        created_unix_millis: u64,
    ) -> WorldCatalogResult<Self> {
        Ok(Self {
            id,
            display_name: normalize_display_name(display_name.into())?,
            seed,
            world_generation_profile: WorldGenerationProfile::default(),
            created_unix_millis,
            last_played_unix_millis: None,
            storage_schema_version: LOCAL_WORLD_CATALOG_SCHEMA_VERSION,
            target_minecraft_version: LOCAL_WORLD_TARGET_MINECRAFT_VERSION.to_owned(),
            mclone_version: None,
            backend_label: None,
            locked: false,
            compatible: true,
        })
    }

    pub fn can_delete(
        &self,
        active_world: Option<&LocalWorldId>,
        capabilities: WorldCatalogCapabilities,
    ) -> WorldCatalogResult<()> {
        if !capabilities.delete_supported {
            return Err(WorldCatalogError::unsupported(
                "world deletion is not supported",
            ));
        }
        validate_delete_inactive_world(&self.id, active_world)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldCatalogCapabilities {
    pub persistent: bool,
    pub list_supported: bool,
    pub create_supported: bool,
    pub open_supported: bool,
    pub delete_supported: bool,
}

impl WorldCatalogCapabilities {
    pub const fn persistent_local() -> Self {
        Self {
            persistent: true,
            list_supported: true,
            create_supported: true,
            open_supported: true,
            delete_supported: true,
        }
    }

    pub const fn transient_only() -> Self {
        Self {
            persistent: false,
            list_supported: false,
            create_supported: false,
            open_supported: false,
            delete_supported: false,
        }
    }

    pub const fn transient_create_only() -> Self {
        Self {
            persistent: false,
            list_supported: false,
            create_supported: true,
            open_supported: false,
            delete_supported: false,
        }
    }

    pub const fn read_only_persistent() -> Self {
        Self {
            persistent: true,
            list_supported: true,
            create_supported: false,
            open_supported: true,
            delete_supported: false,
        }
    }
}

impl Default for WorldCatalogCapabilities {
    fn default() -> Self {
        Self::transient_only()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorldCatalogRequestId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum WorldCatalogRequest {
    ListWorlds,
    CreateWorld { options: LocalWorldCreateOptions },
    OpenWorld { id: LocalWorldId },
    DeleteWorld { id: LocalWorldId },
}

impl WorldCatalogRequest {
    pub fn operation(&self) -> WorldCatalogOperation {
        match self {
            Self::ListWorlds => WorldCatalogOperation::ListWorlds,
            Self::CreateWorld { .. } => WorldCatalogOperation::CreateWorld,
            Self::OpenWorld { .. } => WorldCatalogOperation::OpenWorld,
            Self::DeleteWorld { .. } => WorldCatalogOperation::DeleteWorld,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum WorldCatalogResponse {
    WorldList {
        capabilities: WorldCatalogCapabilities,
        worlds: Vec<LocalWorldSummary>,
    },
    WorldCreated {
        summary: LocalWorldSummary,
    },
    WorldOpened {
        summary: LocalWorldSummary,
    },
    WorldDeleted {
        id: LocalWorldId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorldCatalogOperation {
    ListWorlds,
    CreateWorld,
    OpenWorld,
    DeleteWorld,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldCatalogStatus {
    pub message: String,
    pub ok: bool,
}

impl WorldCatalogStatus {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            ok: true,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            ok: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldCatalogError {
    pub kind: WorldCatalogErrorKind,
    pub message: String,
}

impl WorldCatalogError {
    pub fn new(kind: WorldCatalogErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(WorldCatalogErrorKind::UnsupportedOperation, message)
    }

    pub fn status(&self) -> WorldCatalogStatus {
        WorldCatalogStatus::error(self.message.clone())
    }
}

impl fmt::Display for WorldCatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for WorldCatalogError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorldCatalogErrorKind {
    InvalidWorldId,
    InvalidDisplayName,
    DuplicateWorldId,
    WorldNotFound,
    ActiveWorld,
    UnsupportedOperation,
    StorageFailure,
    IncompatibleSchema,
}

pub fn validate_delete_inactive_world(
    id: &LocalWorldId,
    active_world: Option<&LocalWorldId>,
) -> WorldCatalogResult<()> {
    if active_world.is_some_and(|active| active == id) {
        return Err(WorldCatalogError::new(
            WorldCatalogErrorKind::ActiveWorld,
            format!("cannot delete active local world `{id}`; quit to title first"),
        ));
    }

    Ok(())
}

pub fn validate_local_world_compatible(summary: &LocalWorldSummary) -> WorldCatalogResult<()> {
    if summary.compatible {
        return Ok(());
    }

    Err(WorldCatalogError::new(
        WorldCatalogErrorKind::IncompatibleSchema,
        format!(
            "local world `{}` is not compatible with catalog schema {} and Minecraft target {}",
            summary.id, LOCAL_WORLD_CATALOG_SCHEMA_VERSION, LOCAL_WORLD_TARGET_MINECRAFT_VERSION
        ),
    ))
}

pub fn sort_local_world_summaries(worlds: &mut [LocalWorldSummary]) {
    worlds.sort_by(|a, b| {
        let a_last_played = a.last_played_unix_millis.unwrap_or(a.created_unix_millis);
        let b_last_played = b.last_played_unix_millis.unwrap_or(b.created_unix_millis);
        b_last_played
            .cmp(&a_last_played)
            .then_with(|| a.display_name.cmp(&b.display_name))
            .then_with(|| a.id.cmp(&b.id))
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeWorldCatalog {
    root: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeWorldCatalog {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub const fn capabilities(&self) -> WorldCatalogCapabilities {
        WorldCatalogCapabilities::persistent_local()
    }

    pub fn handle_request(
        &self,
        request: WorldCatalogRequest,
        active_world: Option<&LocalWorldId>,
    ) -> WorldCatalogResult<WorldCatalogResponse> {
        match request {
            WorldCatalogRequest::ListWorlds => Ok(WorldCatalogResponse::WorldList {
                capabilities: self.capabilities(),
                worlds: self.list_worlds()?,
            }),
            WorldCatalogRequest::CreateWorld { options } => {
                Ok(WorldCatalogResponse::WorldCreated {
                    summary: self.create_world(options)?,
                })
            }
            WorldCatalogRequest::OpenWorld { id } => Ok(WorldCatalogResponse::WorldOpened {
                summary: self.open_world(&id)?.summary,
            }),
            WorldCatalogRequest::DeleteWorld { id } => {
                self.delete_world(&id, active_world)?;
                Ok(WorldCatalogResponse::WorldDeleted { id })
            }
        }
    }

    pub fn list_worlds(&self) -> WorldCatalogResult<Vec<LocalWorldSummary>> {
        let mut worlds = Vec::new();
        if !self.root.exists() {
            return Ok(worlds);
        }

        let entries = fs::read_dir(&self.root)
            .map_err(|error| storage_error("failed to read local world catalog", error))?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                storage_error("failed to read local world catalog entry", error)
            })?;
            if !entry
                .file_type()
                .map_err(|error| {
                    storage_error("failed to inspect local world catalog entry", error)
                })?
                .is_dir()
            {
                continue;
            }

            let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(id) = LocalWorldId::new(file_name) else {
                continue;
            };
            if !self.metadata_path(&id).exists() {
                continue;
            }
            worlds.push(self.read_summary(&id)?);
        }

        sort_local_world_summaries(&mut worlds);
        Ok(worlds)
    }

    pub fn create_world(
        &self,
        options: LocalWorldCreateOptions,
    ) -> WorldCatalogResult<LocalWorldSummary> {
        fs::create_dir_all(&self.root)
            .map_err(|error| storage_error("failed to create local world catalog", error))?;

        let existing_ids = self.existing_world_ids()?;
        let id = options.resolve_id(existing_ids.iter())?;
        if existing_ids.contains(&id) {
            return Err(duplicate_world_id(&id));
        }

        let world_dir = self.world_dir(&id);
        fs::create_dir(&world_dir).map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                duplicate_world_id(&id)
            } else {
                storage_error(
                    format!(
                        "failed to create local world directory `{}`",
                        world_dir.display()
                    ),
                    error,
                )
            }
        })?;

        let created_unix_millis = now_unix_millis();
        let mut summary = LocalWorldSummary::new(
            id.clone(),
            options.display_name,
            options.seed,
            created_unix_millis,
        )?;
        summary.world_generation_profile = options.world_generation_profile;
        summary.last_played_unix_millis = Some(created_unix_millis);
        summary.mclone_version = Some(env!("CARGO_PKG_VERSION").to_owned());
        summary.backend_label = Some(NATIVE_WORLD_BACKEND_LABEL.to_owned());

        if let Err(error) = SqliteWorldStore::open_world_dir(&world_dir) {
            let _ = fs::remove_dir_all(&world_dir);
            return Err(storage_error(
                format!(
                    "failed to initialize local world database `{}`",
                    world_dir.display()
                ),
                error,
            ));
        }

        if let Err(error) = self.write_summary(&summary) {
            let _ = fs::remove_dir_all(&world_dir);
            return Err(error);
        }

        Ok(summary)
    }

    pub fn open_world(&self, id: &LocalWorldId) -> WorldCatalogResult<NativeOpenedWorld> {
        let opened = self.open_world_summary(id)?;
        SqliteWorldStore::open_world_dir(&opened.dir).map_err(|error| {
            storage_error(
                format!(
                    "failed to open local world database `{}`",
                    opened.dir.display()
                ),
                error,
            )
        })?;
        Ok(opened)
    }

    pub fn open_sqlite_world_store(
        &self,
        id: &LocalWorldId,
    ) -> WorldCatalogResult<(NativeOpenedWorld, SqliteWorldStore)> {
        let opened = self.open_world_summary(id)?;
        let store = SqliteWorldStore::open_world_dir(&opened.dir).map_err(|error| {
            storage_error(
                format!(
                    "failed to open local world database `{}`",
                    opened.dir.display()
                ),
                error,
            )
        })?;
        Ok((opened, store))
    }

    pub fn delete_world(
        &self,
        id: &LocalWorldId,
        active_world: Option<&LocalWorldId>,
    ) -> WorldCatalogResult<LocalWorldSummary> {
        let summary = self.read_summary(id)?;
        summary.can_delete(active_world, self.capabilities())?;
        let world_dir = self.world_dir(id);
        fs::remove_dir_all(&world_dir).map_err(|error| {
            storage_error(
                format!(
                    "failed to delete local world directory `{}`",
                    world_dir.display()
                ),
                error,
            )
        })?;
        Ok(summary)
    }

    pub fn world_dir(&self, id: &LocalWorldId) -> PathBuf {
        self.root.join(id.as_str())
    }

    pub fn metadata_path(&self, id: &LocalWorldId) -> PathBuf {
        self.world_dir(id).join(NATIVE_WORLD_METADATA_FILE)
    }

    fn open_world_summary(&self, id: &LocalWorldId) -> WorldCatalogResult<NativeOpenedWorld> {
        let mut summary = self.read_summary(id)?;
        validate_local_world_compatible(&summary)?;
        summary.last_played_unix_millis = Some(now_unix_millis());
        self.write_summary(&summary)?;
        Ok(NativeOpenedWorld {
            dir: self.world_dir(id),
            summary,
        })
    }

    fn read_summary(&self, id: &LocalWorldId) -> WorldCatalogResult<LocalWorldSummary> {
        let metadata_path = self.metadata_path(id);
        let bytes = fs::read(&metadata_path).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                world_not_found(id)
            } else {
                storage_error(
                    format!(
                        "failed to read local world metadata `{}`",
                        metadata_path.display()
                    ),
                    error,
                )
            }
        })?;
        let mut summary: LocalWorldSummary = serde_json::from_slice(&bytes).map_err(|error| {
            storage_error(
                format!(
                    "failed to parse local world metadata `{}`",
                    metadata_path.display()
                ),
                error,
            )
        })?;
        if &summary.id != id {
            return Err(storage_failure(format!(
                "local world metadata `{}` identified `{}` but directory is `{}`",
                metadata_path.display(),
                summary.id,
                id
            )));
        }

        normalize_display_name(summary.display_name.clone())?;
        summary.compatible = summary.storage_schema_version == LOCAL_WORLD_CATALOG_SCHEMA_VERSION
            && summary.target_minecraft_version == LOCAL_WORLD_TARGET_MINECRAFT_VERSION;
        Ok(summary)
    }

    fn write_summary(&self, summary: &LocalWorldSummary) -> WorldCatalogResult<()> {
        let metadata_path = self.metadata_path(&summary.id);
        let bytes = serde_json::to_vec_pretty(summary).map_err(|error| {
            storage_error(
                format!(
                    "failed to encode local world metadata `{}`",
                    metadata_path.display()
                ),
                error,
            )
        })?;
        fs::write(&metadata_path, bytes).map_err(|error| {
            storage_error(
                format!(
                    "failed to write local world metadata `{}`",
                    metadata_path.display()
                ),
                error,
            )
        })
    }

    fn existing_world_ids(&self) -> WorldCatalogResult<BTreeSet<LocalWorldId>> {
        let mut ids = BTreeSet::new();
        if !self.root.exists() {
            return Ok(ids);
        }

        let entries = fs::read_dir(&self.root)
            .map_err(|error| storage_error("failed to read local world catalog", error))?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                storage_error("failed to read local world catalog entry", error)
            })?;
            if !entry
                .file_type()
                .map_err(|error| {
                    storage_error("failed to inspect local world catalog entry", error)
                })?
                .is_dir()
            {
                continue;
            }

            let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if let Ok(id) = LocalWorldId::new(file_name) {
                ids.insert(id);
            }
        }
        Ok(ids)
    }
}

/// Backend operation contract used by platform catalog executors.
///
/// The host never owns this backend directly. Native assembly wraps
/// `NativeWorldCatalog` in an immediate executor; browser assembly will submit
/// the same request/response values to IndexedDB asynchronously.
pub trait WorldCatalog {
    fn capabilities(&self) -> WorldCatalogCapabilities;
    fn list_worlds(&self) -> WorldCatalogResult<Vec<LocalWorldSummary>>;
    fn handle_request(
        &self,
        request: WorldCatalogRequest,
        active_world: Option<&LocalWorldId>,
    ) -> WorldCatalogResult<WorldCatalogResponse>;
}

#[cfg(not(target_arch = "wasm32"))]
impl WorldCatalog for NativeWorldCatalog {
    fn capabilities(&self) -> WorldCatalogCapabilities {
        NativeWorldCatalog::capabilities(self)
    }

    fn list_worlds(&self) -> WorldCatalogResult<Vec<LocalWorldSummary>> {
        NativeWorldCatalog::list_worlds(self)
    }

    fn handle_request(
        &self,
        request: WorldCatalogRequest,
        active_world: Option<&LocalWorldId>,
    ) -> WorldCatalogResult<WorldCatalogResponse> {
        NativeWorldCatalog::handle_request(self, request, active_world)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeOpenedWorld {
    pub summary: LocalWorldSummary,
    pub dir: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeOpenedWorld {
    pub fn database_path(&self) -> PathBuf {
        SqliteWorldStore::database_path_for_world_dir(&self.dir)
    }
}

fn normalize_display_name(display_name: String) -> WorldCatalogResult<String> {
    let trimmed = display_name.trim();
    if trimmed.is_empty() {
        return Err(WorldCatalogError::new(
            WorldCatalogErrorKind::InvalidDisplayName,
            "world display name cannot be empty",
        ));
    }
    if trimmed.chars().count() > LOCAL_WORLD_DISPLAY_NAME_MAX_CHARS {
        return Err(WorldCatalogError::new(
            WorldCatalogErrorKind::InvalidDisplayName,
            format!(
                "world display name is too long; maximum is {LOCAL_WORLD_DISPLAY_NAME_MAX_CHARS} characters"
            ),
        ));
    }
    Ok(trimmed.to_owned())
}

fn validate_local_world_id(value: &str) -> WorldCatalogResult<()> {
    if value.is_empty() {
        return Err(invalid_world_id(value, "id cannot be empty"));
    }
    if value.len() > LOCAL_WORLD_ID_MAX_LEN {
        return Err(invalid_world_id(
            value,
            format!("id cannot exceed {LOCAL_WORLD_ID_MAX_LEN} bytes"),
        ));
    }

    let mut chars = value.chars();
    let first = chars.next().expect("empty handled above");
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(invalid_world_id(
            value,
            "id must start with a lowercase ASCII letter or digit",
        ));
    }
    if !value
        .chars()
        .last()
        .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
    {
        return Err(invalid_world_id(
            value,
            "id must end with a lowercase ASCII letter or digit",
        ));
    }

    for ch in value.chars() {
        if !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_') {
            return Err(invalid_world_id(
                value,
                "id may only contain lowercase ASCII letters, digits, '-' and '_'",
            ));
        }
    }

    Ok(())
}

fn invalid_world_id(value: &str, reason: impl Into<String>) -> WorldCatalogError {
    let reason = reason.into();
    let message = if value.is_empty() {
        reason
    } else {
        format!("invalid local world id `{value}`: {reason}")
    };
    WorldCatalogError::new(WorldCatalogErrorKind::InvalidWorldId, message)
}

pub fn world_not_found(id: &LocalWorldId) -> WorldCatalogError {
    WorldCatalogError::new(
        WorldCatalogErrorKind::WorldNotFound,
        format!("local world `{id}` was not found"),
    )
}

pub fn duplicate_world_id(id: &LocalWorldId) -> WorldCatalogError {
    WorldCatalogError::new(
        WorldCatalogErrorKind::DuplicateWorldId,
        format!("local world `{id}` already exists"),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn storage_failure(message: impl Into<String>) -> WorldCatalogError {
    WorldCatalogError::new(WorldCatalogErrorKind::StorageFailure, message)
}

#[cfg(not(target_arch = "wasm32"))]
fn storage_error(context: impl Into<String>, error: impl fmt::Display) -> WorldCatalogError {
    storage_failure(format!("{}: {error}", context.into()))
}

#[cfg(not(target_arch = "wasm32"))]
fn now_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn slug_from_display_name(display_name: &str) -> String {
    let mut slug = String::new();
    let mut pending_separator = false;

    for ch in display_name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            let needs_separator = pending_separator && !slug.is_empty();
            let needed = 1 + usize::from(needs_separator);
            if slug.len() + needed > LOCAL_WORLD_ID_MAX_LEN {
                break;
            }
            if needs_separator {
                slug.push('-');
            }
            slug.push(ch.to_ascii_lowercase());
            pending_separator = false;
        } else {
            pending_separator = !slug.is_empty();
        }
    }

    if slug.is_empty() {
        DEFAULT_LOCAL_WORLD_ID.to_owned()
    } else {
        slug
    }
}

fn suffixed_world_id_candidate(base: &str, suffix_index: u32) -> String {
    let suffix = format!("-{suffix_index}");
    let max_prefix_len = LOCAL_WORLD_ID_MAX_LEN.saturating_sub(suffix.len()).max(1);
    let mut prefix = base.to_owned();
    prefix.truncate(max_prefix_len);
    while prefix.ends_with('-') || prefix.ends_with('_') {
        prefix.pop();
    }
    if prefix.is_empty() {
        prefix.push_str(DEFAULT_LOCAL_WORLD_ID);
        prefix.truncate(max_prefix_len);
    }
    format!("{prefix}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    use std::path::{Path, PathBuf};
    #[cfg(not(target_arch = "wasm32"))]
    use std::sync::atomic::{AtomicU64, Ordering};

    #[cfg(not(target_arch = "wasm32"))]
    use mclone_core::{
        BlockStateId, CHUNK_SECTION_VOLUME, ChunkPos, ChunkRevision, ChunkSnapshot, ChunkStatus,
    };
    #[cfg(not(target_arch = "wasm32"))]
    use mclone_server::{ChunkRecord, WorldStore};

    #[test]
    fn display_names_normalize_to_safe_world_ids() {
        assert_eq!(
            LocalWorldId::from_display_name(" My Cool World! ").as_str(),
            "my-cool-world"
        );
        assert_eq!(
            LocalWorldId::from_display_name("Seed: -123").as_str(),
            "seed-123"
        );
        assert_eq!(LocalWorldId::from_display_name("世界").as_str(), "world");
    }

    #[test]
    fn explicit_world_ids_must_already_be_normalized() {
        assert_eq!(LocalWorldId::new("seed-123").unwrap().as_str(), "seed-123");
        assert_eq!(LocalWorldId::new("smoke_1").unwrap().as_str(), "smoke_1");

        assert_eq!(
            LocalWorldId::new("Seed-123").unwrap_err().kind,
            WorldCatalogErrorKind::InvalidWorldId
        );
        assert_eq!(
            LocalWorldId::new("../seed-123").unwrap_err().kind,
            WorldCatalogErrorKind::InvalidWorldId
        );
        assert_eq!(
            LocalWorldId::new("seed-123-").unwrap_err().kind,
            WorldCatalogErrorKind::InvalidWorldId
        );
    }

    #[test]
    fn available_world_id_suffixes_duplicate_display_names() {
        let base = LocalWorldId::new("my-world").unwrap();
        let second = LocalWorldId::new("my-world-2").unwrap();
        let existing = [base, second];

        let candidate = LocalWorldId::available_from_display_name("My World", existing.iter());

        assert_eq!(candidate.as_str(), "my-world-3");
    }

    #[test]
    fn available_world_id_keeps_suffix_within_max_length() {
        let long_name = "A".repeat(LOCAL_WORLD_ID_MAX_LEN + 12);
        let first = LocalWorldId::from_display_name(&long_name);
        let existing = [first];

        let candidate = LocalWorldId::available_from_display_name(&long_name, existing.iter());

        assert!(candidate.as_str().ends_with("-2"));
        assert!(candidate.as_str().len() <= LOCAL_WORLD_ID_MAX_LEN);
        validate_local_world_id(candidate.as_str()).unwrap();
    }

    #[test]
    fn create_options_trim_display_name_and_resolve_duplicate_id() {
        let existing = [LocalWorldId::new("new-world").unwrap()];
        let options = LocalWorldCreateOptions::new("  New World  ", 12345).unwrap();

        assert_eq!(options.display_name, "New World");
        assert_eq!(
            options.resolve_id(existing.iter()).unwrap().as_str(),
            "new-world-2"
        );
    }

    #[test]
    fn legacy_catalog_records_default_to_overworld_generation() {
        let summary =
            LocalWorldSummary::new(LocalWorldId::new("legacy").unwrap(), "Legacy", 7, 100).unwrap();
        let mut encoded = serde_json::to_value(summary).unwrap();
        encoded
            .as_object_mut()
            .unwrap()
            .remove("worldGenerationProfile");

        let decoded: LocalWorldSummary = serde_json::from_value(encoded).unwrap();

        assert_eq!(
            decoded.world_generation_profile,
            WorldGenerationProfile::Overworld
        );
    }

    #[test]
    fn authored_generation_profile_round_trips_through_catalog_records() {
        let options = LocalWorldCreateOptions::new("Table", 9)
            .unwrap()
            .with_world_generation_profile(WorldGenerationProfile::authored_only());

        let decoded: LocalWorldCreateOptions =
            serde_json::from_slice(&serde_json::to_vec(&options).unwrap()).unwrap();

        assert_eq!(decoded, options);
        assert_eq!(
            decoded.world_generation_profile,
            WorldGenerationProfile::authored_only()
        );
    }

    #[test]
    fn create_options_reject_empty_display_name() {
        let error = LocalWorldCreateOptions::new("   ", 1).unwrap_err();

        assert_eq!(error.kind, WorldCatalogErrorKind::InvalidDisplayName);
        assert_eq!(
            error.status(),
            WorldCatalogStatus {
                message: "world display name cannot be empty".to_owned(),
                ok: false,
            }
        );
    }

    #[test]
    fn delete_active_world_is_rejected_at_shared_layer() {
        let active = LocalWorldId::new("my-world").unwrap();
        let other = LocalWorldId::new("other-world").unwrap();

        let error = validate_delete_inactive_world(&active, Some(&active)).unwrap_err();
        assert_eq!(error.kind, WorldCatalogErrorKind::ActiveWorld);
        validate_delete_inactive_world(&other, Some(&active)).unwrap();
        validate_delete_inactive_world(&other, None).unwrap();
    }

    #[test]
    fn world_summary_checks_delete_capability_and_active_world() {
        let active = LocalWorldId::new("my-world").unwrap();
        let summary = LocalWorldSummary::new(active.clone(), "My World", 77, 100).unwrap();

        assert_eq!(
            summary
                .can_delete(Some(&active), WorldCatalogCapabilities::persistent_local())
                .unwrap_err()
                .kind,
            WorldCatalogErrorKind::ActiveWorld
        );
        assert_eq!(
            summary
                .can_delete(None, WorldCatalogCapabilities::read_only_persistent())
                .unwrap_err()
                .kind,
            WorldCatalogErrorKind::UnsupportedOperation
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_catalog_creates_lists_opens_and_deletes_worlds() {
        let temp = TestTempDir::new("native-catalog-crud");
        let catalog = NativeWorldCatalog::new(temp.path().join("worlds"));

        assert_eq!(
            catalog.capabilities(),
            WorldCatalogCapabilities::persistent_local()
        );
        assert_eq!(catalog.root(), temp.path().join("worlds").as_path());
        assert!(catalog.list_worlds().unwrap().is_empty());

        let first = catalog
            .create_world(
                LocalWorldCreateOptions::new("My World", 123)
                    .unwrap()
                    .with_world_generation_profile(WorldGenerationProfile::authored_only()),
            )
            .unwrap();
        let second = catalog
            .create_world(LocalWorldCreateOptions::new("My World", 456).unwrap())
            .unwrap();

        assert_eq!(first.id.as_str(), "my-world");
        assert_eq!(
            first.world_generation_profile,
            WorldGenerationProfile::authored_only()
        );
        assert_eq!(second.id.as_str(), "my-world-2");
        assert_eq!(
            first.backend_label.as_deref(),
            Some(NATIVE_WORLD_BACKEND_LABEL)
        );
        assert_eq!(
            first.mclone_version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
        assert!(first.last_played_unix_millis.is_some());
        assert!(catalog.metadata_path(&first.id).exists());
        assert!(catalog.world_dir(&first.id).is_dir());

        let listed = catalog.list_worlds().unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().any(|summary| summary.id == first.id));
        assert!(listed.iter().any(|summary| summary.id == second.id));

        let opened = catalog.open_world(&first.id).unwrap();
        assert_eq!(opened.summary.id, first.id);
        assert_eq!(opened.dir, catalog.world_dir(&opened.summary.id));
        assert!(opened.database_path().exists());

        let error = catalog
            .delete_world(&opened.summary.id, Some(&opened.summary.id))
            .unwrap_err();
        assert_eq!(error.kind, WorldCatalogErrorKind::ActiveWorld);

        catalog
            .delete_world(&opened.summary.id, Some(&second.id))
            .unwrap();
        assert!(!catalog.world_dir(&opened.summary.id).exists());

        let remaining = catalog.list_worlds().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, second.id);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_catalog_rejects_duplicate_requested_world_id() {
        let temp = TestTempDir::new("native-catalog-duplicate-id");
        let catalog = NativeWorldCatalog::new(temp.path().join("worlds"));
        let requested_id = LocalWorldId::new("fixed-world").unwrap();
        let first_options = LocalWorldCreateOptions::new("First", 1)
            .unwrap()
            .with_requested_id(requested_id.clone());
        let second_options = LocalWorldCreateOptions::new("Second", 2)
            .unwrap()
            .with_requested_id(requested_id);

        catalog.create_world(first_options).unwrap();
        let error = catalog.create_world(second_options).unwrap_err();

        assert_eq!(error.kind, WorldCatalogErrorKind::DuplicateWorldId);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_catalog_request_dispatch_returns_shared_responses() {
        let temp = TestTempDir::new("native-catalog-dispatch");
        let catalog = NativeWorldCatalog::new(temp.path().join("worlds"));
        let created = catalog
            .handle_request(
                WorldCatalogRequest::CreateWorld {
                    options: LocalWorldCreateOptions::new("Dispatch", 11).unwrap(),
                },
                None,
            )
            .unwrap();
        let id = match created {
            WorldCatalogResponse::WorldCreated { summary } => summary.id,
            response => panic!("unexpected response: {response:?}"),
        };

        let listed = catalog
            .handle_request(WorldCatalogRequest::ListWorlds, None)
            .unwrap();
        match listed {
            WorldCatalogResponse::WorldList {
                capabilities,
                worlds,
            } => {
                assert_eq!(capabilities, WorldCatalogCapabilities::persistent_local());
                assert_eq!(worlds.len(), 1);
                assert_eq!(worlds[0].id, id);
            }
            response => panic!("unexpected response: {response:?}"),
        }

        let opened = catalog
            .handle_request(WorldCatalogRequest::OpenWorld { id: id.clone() }, None)
            .unwrap();
        assert!(matches!(
            opened,
            WorldCatalogResponse::WorldOpened { ref summary } if summary.id == id
        ));

        let deleted = catalog
            .handle_request(WorldCatalogRequest::DeleteWorld { id: id.clone() }, None)
            .unwrap();
        assert_eq!(deleted, WorldCatalogResponse::WorldDeleted { id });
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_catalog_opened_sqlite_store_persists_chunk_records_across_reopen() {
        let temp = TestTempDir::new("native-catalog-sqlite-reopen");
        let catalog = NativeWorldCatalog::new(temp.path().join("worlds"));
        let summary = catalog
            .create_world(LocalWorldCreateOptions::new("Durable", 99).unwrap())
            .unwrap();
        let chunk_pos = ChunkPos::new(4, -5);
        let record = test_record(chunk_pos, 42);

        {
            let (opened, mut store) = catalog.open_sqlite_world_store(&summary.id).unwrap();
            assert_eq!(opened.summary.id, summary.id);
            assert_eq!(store.path(), opened.database_path().as_path());
            store.save_chunk(&record).unwrap();
            store.flush().unwrap();
            store.close().unwrap();
        }

        {
            let (_opened, mut reopened) = catalog.open_sqlite_world_store(&summary.id).unwrap();
            assert_eq!(reopened.load_chunk(chunk_pos).unwrap(), Some(record));
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_catalog_reports_missing_world_for_open_and_delete() {
        let temp = TestTempDir::new("native-catalog-missing");
        let catalog = NativeWorldCatalog::new(temp.path().join("worlds"));
        let id = LocalWorldId::new("missing").unwrap();

        assert_eq!(
            catalog.open_world(&id).unwrap_err().kind,
            WorldCatalogErrorKind::WorldNotFound
        );
        assert_eq!(
            catalog.delete_world(&id, None).unwrap_err().kind,
            WorldCatalogErrorKind::WorldNotFound
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn test_record(pos: ChunkPos, revision: u64) -> ChunkRecord {
        let snapshot = ChunkSnapshot::from_block_state_ids(
            pos,
            ChunkStatus::Features,
            ChunkRevision(revision),
            0,
            16,
            &vec![BlockStateId(1); CHUNK_SECTION_VOLUME],
        );
        ChunkRecord::from_snapshot(snapshot)
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[derive(Debug)]
    struct TestTempDir {
        path: PathBuf,
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl TestTempDir {
        fn new(name: &str) -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);

            let path = std::env::temp_dir().join(format!(
                "mclone-app-runtime-{name}-{}-{}-{}",
                std::process::id(),
                now_unix_millis(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
