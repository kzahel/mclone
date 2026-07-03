use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub const LOCAL_WORLD_CATALOG_SCHEMA_VERSION: u32 = 1;
pub const LOCAL_WORLD_TARGET_MINECRAFT_VERSION: &str = "1.17.1";
pub const LOCAL_WORLD_ID_MAX_LEN: usize = 64;
pub const LOCAL_WORLD_DISPLAY_NAME_MAX_CHARS: usize = 64;

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
    pub requested_id: Option<LocalWorldId>,
}

impl LocalWorldCreateOptions {
    pub fn new(display_name: impl Into<String>, seed: i64) -> WorldCatalogResult<Self> {
        Ok(Self {
            display_name: normalize_display_name(display_name.into())?,
            seed,
            requested_id: None,
        })
    }

    pub fn with_requested_id(mut self, id: LocalWorldId) -> Self {
        self.requested_id = Some(id);
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
}
