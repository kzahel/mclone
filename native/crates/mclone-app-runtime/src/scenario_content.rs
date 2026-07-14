//! Storage-neutral managed content contracts for built-in scenarios.
//!
//! This module owns the one scenario recipe consumed by native and browser
//! adapters. Filesystem paths, SQLite handles, IndexedDB handles, and async
//! execution stay in platform executors; authored blocks and policy stay here.

use mclone_server::{
    AuthoredWorldFixtureKind, AuthoredWorldFixtureManifest, ChunkRecord, WorldBehaviorProfile,
    authored_world_fixture_records, encode_chunk_record,
};
use serde::{Deserialize, Serialize};

use crate::scenario::{BuiltInScenarioId, ScenarioLaunchIntent};

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

pub const MANAGED_SCENARIO_SCHEMA_VERSION: u32 = 1;
pub const LOBBY_PREVIEW_CONTENT_VERSION: u32 = 1;
pub const LOBBY_PREVIEW_DIRECTORY: &str = "lobby-preview-v1";
pub const MANAGED_SCENARIO_MANIFEST_FILE: &str = "scenario.json";
pub const MANAGED_SCENARIO_LOBBY_DIRECTORY: &str = "lobby";
pub const MANAGED_SCENARIO_ISLAND_DIRECTORY: &str = "demo-island";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ManagedScenarioWorldRole {
    Primary,
    Destination,
}

/// Stable storage identity. This is deliberately unrelated to the live scene
/// instance id, role of the currently active slot, or a native filesystem path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ManagedWorldKey(String);

impl ManagedWorldKey {
    pub fn new(value: impl Into<String>) -> Result<Self, ManagedScenarioValidationError> {
        let value = value.into();
        if value.is_empty()
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.')
            })
        {
            return Err(ManagedScenarioValidationError::InvalidManagedWorldKey(
                value,
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedScenarioWorldManifest {
    pub content_id: String,
    pub content_version: u32,
    pub directory: String,
    pub behavior_profile: WorldBehaviorProfile,
    pub fixture: AuthoredWorldFixtureManifest,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedScenarioManifest {
    pub schema_version: u32,
    pub scenario_id: BuiltInScenarioId,
    pub content_version: u32,
    pub primary: ManagedScenarioWorldManifest,
    pub destination: ManagedScenarioWorldManifest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedScenarioValidationError {
    UnsupportedScenario(BuiltInScenarioId),
    SchemaVersion { expected: u32, actual: u32 },
    ContentVersion { expected: u32, actual: u32 },
    InvalidManagedWorldKey(String),
    DuplicateWorldKey(String),
    WorldMismatch { role: ManagedScenarioWorldRole },
}

impl std::fmt::Display for ManagedScenarioValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedScenario(id) => write!(formatter, "unsupported scenario {id:?}"),
            Self::SchemaVersion { expected, actual } => {
                write!(formatter, "expected schema {expected}, found {actual}")
            }
            Self::ContentVersion { expected, actual } => {
                write!(
                    formatter,
                    "expected content version {expected}, found {actual}"
                )
            }
            Self::InvalidManagedWorldKey(value) => {
                write!(formatter, "invalid managed-world key `{value}`")
            }
            Self::DuplicateWorldKey(value) => {
                write!(formatter, "duplicate managed-world key `{value}`")
            }
            Self::WorldMismatch { role } => {
                write!(
                    formatter,
                    "managed scenario {role:?} world does not match its recipe"
                )
            }
        }
    }
}

impl std::error::Error for ManagedScenarioValidationError {}

impl ManagedScenarioManifest {
    pub fn lobby_preview_v1() -> Self {
        Self {
            schema_version: MANAGED_SCENARIO_SCHEMA_VERSION,
            scenario_id: BuiltInScenarioId::LobbyPreview,
            content_version: LOBBY_PREVIEW_CONTENT_VERSION,
            primary: ManagedScenarioWorldManifest {
                content_id: "lobby-v1".to_owned(),
                content_version: 1,
                directory: MANAGED_SCENARIO_LOBBY_DIRECTORY.to_owned(),
                behavior_profile: WorldBehaviorProfile::ProtectedLobby,
                fixture: AuthoredWorldFixtureManifest::new(AuthoredWorldFixtureKind::Table),
            },
            destination: ManagedScenarioWorldManifest {
                content_id: "demo-island-v1".to_owned(),
                content_version: 1,
                directory: MANAGED_SCENARIO_ISLAND_DIRECTORY.to_owned(),
                behavior_profile: WorldBehaviorProfile::Mutable,
                fixture: AuthoredWorldFixtureManifest::new(AuthoredWorldFixtureKind::Island),
            },
        }
    }

    pub fn for_intent(intent: ScenarioLaunchIntent) -> Self {
        match intent.id {
            BuiltInScenarioId::LobbyPreview => Self::lobby_preview_v1(),
        }
    }

    pub fn world(&self, role: ManagedScenarioWorldRole) -> &ManagedScenarioWorldManifest {
        match role {
            ManagedScenarioWorldRole::Primary => &self.primary,
            ManagedScenarioWorldRole::Destination => &self.destination,
        }
    }

    pub fn world_key(
        &self,
        role: ManagedScenarioWorldRole,
    ) -> Result<ManagedWorldKey, ManagedScenarioValidationError> {
        ManagedWorldKey::new(format!(
            "managed.{}.{}",
            scenario_key(self.scenario_id),
            self.world(role).content_id
        ))
    }

    pub fn validate(&self) -> Result<(), ManagedScenarioValidationError> {
        let expected = match self.scenario_id {
            BuiltInScenarioId::LobbyPreview => Self::lobby_preview_v1(),
        };
        if self.schema_version != MANAGED_SCENARIO_SCHEMA_VERSION {
            return Err(ManagedScenarioValidationError::SchemaVersion {
                expected: MANAGED_SCENARIO_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }
        if self.content_version != LOBBY_PREVIEW_CONTENT_VERSION {
            return Err(ManagedScenarioValidationError::ContentVersion {
                expected: LOBBY_PREVIEW_CONTENT_VERSION,
                actual: self.content_version,
            });
        }
        for role in [
            ManagedScenarioWorldRole::Primary,
            ManagedScenarioWorldRole::Destination,
        ] {
            if self.world(role) != expected.world(role) {
                return Err(ManagedScenarioValidationError::WorldMismatch { role });
            }
        }
        let primary = self.world_key(ManagedScenarioWorldRole::Primary)?;
        let destination = self.world_key(ManagedScenarioWorldRole::Destination)?;
        if primary == destination {
            return Err(ManagedScenarioValidationError::DuplicateWorldKey(
                primary.as_str().to_owned(),
            ));
        }
        Ok(())
    }
}

fn scenario_key(id: BuiltInScenarioId) -> &'static str {
    match id {
        BuiltInScenarioId::LobbyPreview => "lobby-preview-v1",
    }
}

/// Storage-neutral encoded authored content. Browser persistence writes these
/// exact shared Rust codec bytes; TypeScript never authors blocks or fixtures.
#[derive(Clone, Debug, PartialEq)]
pub struct ManagedScenarioWorldPayload {
    pub role: ManagedScenarioWorldRole,
    pub key: ManagedWorldKey,
    pub manifest: ManagedScenarioWorldManifest,
    pub chunk_records: Vec<ManagedScenarioChunkRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedScenarioChunkRecord {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub bytes: Vec<u8>,
}

pub fn managed_scenario_world_payload(
    manifest: &ManagedScenarioManifest,
    role: ManagedScenarioWorldRole,
) -> Result<ManagedScenarioWorldPayload, ManagedScenarioPayloadError> {
    manifest.validate()?;
    let world = manifest.world(role);
    let (fixture, records) = authored_world_fixture_records(world.fixture.kind)?;
    if fixture != world.fixture {
        return Err(ManagedScenarioValidationError::WorldMismatch { role }.into());
    }
    let chunk_records = encode_records(records)?;
    Ok(ManagedScenarioWorldPayload {
        role,
        key: manifest.world_key(role)?,
        manifest: world.clone(),
        chunk_records,
    })
}

fn encode_records(
    records: Vec<ChunkRecord>,
) -> Result<Vec<ManagedScenarioChunkRecord>, ManagedScenarioPayloadError> {
    records
        .into_iter()
        .map(|record| {
            let pos = record.pos();
            Ok(ManagedScenarioChunkRecord {
                chunk_x: pos.x,
                chunk_z: pos.z,
                bytes: encode_chunk_record(&record)?,
            })
        })
        .collect()
}

/// Deterministic receipt over storage identity, chunk coordinates, and the
/// shared persistence codec bytes. Native and browser tests pin the same
/// values so a platform-local fixture or codec fork cannot pass unnoticed.
pub fn managed_scenario_payload_fingerprint(payload: &ManagedScenarioWorldPayload) -> u64 {
    payload
        .key
        .as_str()
        .as_bytes()
        .iter()
        .copied()
        .chain(payload.chunk_records.iter().flat_map(|record| {
            record
                .chunk_x
                .to_le_bytes()
                .into_iter()
                .chain(record.chunk_z.to_le_bytes())
                .chain(record.bytes.iter().copied())
        }))
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        })
}

#[derive(Debug)]
pub enum ManagedScenarioPayloadError {
    Validation(ManagedScenarioValidationError),
    Records(mclone_server::ChunkStoreError),
}

impl std::fmt::Display for ManagedScenarioPayloadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(error) => error.fmt(formatter),
            Self::Records(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ManagedScenarioPayloadError {}

impl From<ManagedScenarioValidationError> for ManagedScenarioPayloadError {
    fn from(value: ManagedScenarioValidationError) -> Self {
        Self::Validation(value)
    }
}

impl From<mclone_server::ChunkStoreError> for ManagedScenarioPayloadError {
    fn from(value: mclone_server::ChunkStoreError) -> Self {
        Self::Records(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lobby_manifest_serialization_and_storage_keys_are_stable() {
        let manifest = ManagedScenarioManifest::lobby_preview_v1();
        manifest.validate().unwrap();
        assert_eq!(
            manifest
                .world_key(ManagedScenarioWorldRole::Primary)
                .unwrap()
                .as_str(),
            "managed.lobby-preview-v1.lobby-v1"
        );
        assert_eq!(
            manifest
                .world_key(ManagedScenarioWorldRole::Destination)
                .unwrap()
                .as_str(),
            "managed.lobby-preview-v1.demo-island-v1"
        );
        let json = serde_json::to_string(&manifest).unwrap();
        assert_eq!(
            serde_json::from_str::<ManagedScenarioManifest>(&json).unwrap(),
            manifest
        );
        assert!(!json.contains('/') && !json.contains("PathBuf"));
    }

    #[test]
    fn shared_payloads_are_encoded_by_the_server_codec() {
        let manifest = ManagedScenarioManifest::lobby_preview_v1();
        let primary =
            managed_scenario_world_payload(&manifest, ManagedScenarioWorldRole::Primary).unwrap();
        let destination =
            managed_scenario_world_payload(&manifest, ManagedScenarioWorldRole::Destination)
                .unwrap();
        assert_eq!(primary.chunk_records.len(), 49);
        assert_eq!(destination.chunk_records.len(), 49);
        assert_ne!(
            managed_scenario_payload_fingerprint(&primary),
            managed_scenario_payload_fingerprint(&destination)
        );
        assert_eq!(
            managed_scenario_payload_fingerprint(&primary),
            managed_scenario_payload_fingerprint(
                &managed_scenario_world_payload(&manifest, ManagedScenarioWorldRole::Primary,)
                    .unwrap()
            )
        );
        assert_eq!(managed_scenario_payload_fingerprint(&primary), 8_001_097_006_086_081_343);
        assert_eq!(
            managed_scenario_payload_fingerprint(&destination),
            8_764_019_107_988_679_539
        );
    }
}
