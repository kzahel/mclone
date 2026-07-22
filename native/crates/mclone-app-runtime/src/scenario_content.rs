//! Shared, storage-neutral content recipe for the built-in lobby.
//!
//! The authored primary is instantiated into a fresh store for each launch.
//! The fallback is an ordinary persistent app-private world. Platform code
//! resolves only the physical storage behind these identities.

use mclone_server::{
    AuthoredWorldFixtureKind, AuthoredWorldFixtureManifest, WorldBehaviorProfile,
    WorldGenerationProfile, initial_spawn_center_for_seed,
};

use crate::scenario::{BuiltInScenarioId, ScenarioLaunchIntent};
use crate::world_catalog::LocalWorldId;

pub const LOBBY_PREVIEW_FALLBACK_SEED: i64 = 12_345;

/// Existing app-private directory components retained for compatibility with
/// fallback data created before the installer was removed.
pub const LOBBY_FALLBACK_LEGACY_PARENT_DIRECTORY: &str = "lobby-preview-v3";
pub const LOBBY_FALLBACK_DIRECTORY: &str = "fallback-overworld";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LobbyWorldRole {
    Primary,
    Destination,
}

/// Path-free storage route selected by shared lobby policy.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum LobbyWorldSource {
    TransientAuthored(AuthoredWorldFixtureKind),
    AppPrivate(AppPrivateWorldKey),
    Catalog(LocalWorldId),
}

impl LobbyWorldSource {
    pub fn world_id(&self) -> &str {
        match self {
            Self::TransientAuthored(fixture) => fixture.fixture_id(),
            Self::AppPrivate(key) => key.storage_id(),
            Self::Catalog(id) => id.as_str(),
        }
    }

    pub const fn kind_label(&self) -> &'static str {
        match self {
            Self::TransientAuthored(_) => "transient-authored",
            Self::AppPrivate(_) => "app-private",
            Self::Catalog(_) => "catalog",
        }
    }

    pub const fn authored_fixture(&self) -> Option<AuthoredWorldFixtureKind> {
        match self {
            Self::TransientAuthored(fixture) => Some(*fixture),
            Self::AppPrivate(_) | Self::Catalog(_) => None,
        }
    }

    /// Whether this route owns durable world state across runtime sessions.
    ///
    /// Runtime-only fixture decoration may be authored into a transient store,
    /// but persistent worlds must restore their saved actor set without
    /// re-running authoring on every open.
    pub const fn allows_runtime_actor_authoring(&self) -> bool {
        matches!(self, Self::TransientAuthored(_))
    }
}

/// Stable identity for a persistent world owned by the application rather
/// than the user-visible catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AppPrivateWorldKey {
    LobbyFallback,
}

impl AppPrivateWorldKey {
    /// Preserve the original browser storage identity so the cleanup does not
    /// orphan an already-generated private fallback world.
    pub const fn storage_id(self) -> &'static str {
        match self {
            Self::LobbyFallback => "managed.lobby-preview-v3.overworld-v3",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LobbyFallbackContent {
    pub seed: i64,
    pub center_chunk: [i32; 2],
    pub preview_anchor: [f64; 3],
    pub preview_display_anchor: [f64; 3],
    pub behavior_profile: WorldBehaviorProfile,
    pub world_generation_profile: WorldGenerationProfile,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LobbyScenarioContent {
    pub scenario_id: BuiltInScenarioId,
    pub primary: AuthoredWorldFixtureManifest,
    pub fallback: LobbyFallbackContent,
}

impl LobbyScenarioContent {
    pub fn for_intent(intent: ScenarioLaunchIntent) -> Self {
        match intent.id {
            BuiltInScenarioId::LobbyPreview => Self::current(),
        }
    }

    pub fn current() -> Self {
        let center = initial_spawn_center_for_seed(LOBBY_PREVIEW_FALLBACK_SEED);
        let center_block_x = f64::from(center.x * 16 + 8);
        let center_block_z = f64::from(center.z * 16 + 8);
        Self {
            scenario_id: BuiltInScenarioId::LobbyPreview,
            primary: AuthoredWorldFixtureManifest::new(AuthoredWorldFixtureKind::LobbyTableV2),
            fallback: LobbyFallbackContent {
                seed: LOBBY_PREVIEW_FALLBACK_SEED,
                center_chunk: [center.x, center.z],
                preview_anchor: [center_block_x, 72.0, center_block_z],
                preview_display_anchor: [center_block_x, 72.0, center_block_z],
                behavior_profile: WorldBehaviorProfile::Mutable,
                world_generation_profile: WorldGenerationProfile::Overworld,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_lobby_recipe_is_transient_primary_plus_private_fallback() {
        let recipe = LobbyScenarioContent::current();
        assert_eq!(recipe.scenario_id, BuiltInScenarioId::LobbyPreview);
        assert_eq!(recipe.primary.kind, AuthoredWorldFixtureKind::LobbyTableV2);
        assert_eq!(recipe.fallback.seed, LOBBY_PREVIEW_FALLBACK_SEED);
        assert_eq!(
            AppPrivateWorldKey::LobbyFallback.storage_id(),
            "managed.lobby-preview-v3.overworld-v3"
        );
    }

    #[test]
    fn only_transient_lobby_storage_allows_runtime_actor_authoring() {
        assert!(
            LobbyWorldSource::TransientAuthored(AuthoredWorldFixtureKind::LobbyIslandV2)
                .allows_runtime_actor_authoring()
        );
        assert!(
            !LobbyWorldSource::AppPrivate(AppPrivateWorldKey::LobbyFallback)
                .allows_runtime_actor_authoring()
        );
        assert!(
            !LobbyWorldSource::Catalog(LocalWorldId::new("saved-world").unwrap())
                .allows_runtime_actor_authoring()
        );
    }
}
