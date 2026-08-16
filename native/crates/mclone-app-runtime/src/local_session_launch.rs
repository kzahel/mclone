use anyhow::{Context, Result};
use mclone_core::ChunkPos;
use mclone_protocol::ChunkView;
use mclone_server::{
    LocalAuthorityStartConfig, initial_spawn_center_for_descriptor,
    validate_local_integrated_chunk_view_topology,
};

/// Why a local session uses its requested or profile-derived entry center.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LocalSessionEntryIntent {
    #[default]
    ProfilePreferred,
    ExplicitCoordinate,
    AuthoredCoordinate,
    PersistedPlayerOrProfilePreferred,
}

impl LocalSessionEntryIntent {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProfilePreferred => "profile-preferred",
            Self::ExplicitCoordinate => "explicit-coordinate",
            Self::AuthoredCoordinate => "authored-coordinate",
            Self::PersistedPlayerOrProfilePreferred => "persisted-player-or-profile-preferred",
        }
    }

    pub const fn uses_profile_preferred_fallback(self) -> bool {
        matches!(
            self,
            Self::ProfilePreferred | Self::PersistedPlayerOrProfilePreferred
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSessionEntryReceipt {
    pub intent: LocalSessionEntryIntent,
    pub requested_center: ChunkPos,
    pub resolved_center: ChunkPos,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSessionLaunchReceipt {
    pub authority: LocalAuthorityStartConfig,
    pub entry: LocalSessionEntryReceipt,
    pub initial_view: ChunkView,
}

/// Fully resolved local-session meaning, before a native thread or browser
/// Worker chooses its physical resources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSessionLaunchPlan {
    pub authority: LocalAuthorityStartConfig,
    pub initial_view: ChunkView,
    receipt: LocalSessionLaunchReceipt,
}

impl LocalSessionLaunchPlan {
    pub fn receipt(&self) -> &LocalSessionLaunchReceipt {
        &self.receipt
    }
}

pub fn resolve_local_session_launch_plan(
    authority: LocalAuthorityStartConfig,
    requested_center: ChunkPos,
    render_distance: u32,
    entry_intent: LocalSessionEntryIntent,
) -> Result<LocalSessionLaunchPlan> {
    authority.validate().map_err(anyhow::Error::msg)?;
    let selected_center = if entry_intent.uses_profile_preferred_fallback() {
        initial_spawn_center_for_descriptor(
            authority.seed,
            authority.world_generation_profile,
            authority.world_topology,
        )
    } else {
        requested_center
    };
    let resolved_center = authority
        .world_topology
        .canonicalize_chunk(selected_center)
        .with_context(|| {
            format!(
                "local entry center ({}, {}) is outside topology {:?}",
                selected_center.x, selected_center.z, authority.world_topology
            )
        })?;
    let initial_view = ChunkView {
        center: resolved_center,
        render_distance,
        chunk_tracking_radius: crate::chunk_tracking_radius_for_render_distance(render_distance),
    };
    validate_local_integrated_chunk_view_topology(authority.world_topology, &initial_view)
        .context("local launch view is incompatible with world topology")?;
    let receipt = LocalSessionLaunchReceipt {
        authority: authority.clone(),
        entry: LocalSessionEntryReceipt {
            intent: entry_intent,
            requested_center,
            resolved_center,
        },
        initial_view: initial_view.clone(),
    };
    Ok(LocalSessionLaunchPlan {
        authority,
        initial_view,
        receipt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::HorizontalTopology;
    use mclone_server::{SimulationCadenceConfig, WorldGenerationProfile};

    #[test]
    fn profile_preferred_entry_uses_the_reproduced_inland_center() {
        let authority = LocalAuthorityStartConfig::new(553_534_047_293_117_028);
        let plan = resolve_local_session_launch_plan(
            authority,
            ChunkPos::new(0, 0),
            5,
            LocalSessionEntryIntent::ProfilePreferred,
        )
        .unwrap();

        assert_eq!(plan.initial_view.center, ChunkPos::new(-48, 20));
        assert_eq!(
            plan.receipt().entry.intent,
            LocalSessionEntryIntent::ProfilePreferred
        );
    }

    #[test]
    fn explicit_entry_is_preserved_and_periodic_coordinates_are_canonicalized() {
        let mut authority = LocalAuthorityStartConfig::new(7);
        authority.world_generation_profile = WorldGenerationProfile::FlatGrassV1;
        authority.world_topology = HorizontalTopology::cylinder_x(0, 32);
        let plan = resolve_local_session_launch_plan(
            authority,
            ChunkPos::new(35, -4),
            2,
            LocalSessionEntryIntent::ExplicitCoordinate,
        )
        .unwrap();

        assert_eq!(plan.initial_view.center, ChunkPos::new(3, -4));
        assert_eq!(plan.receipt().entry.requested_center, ChunkPos::new(35, -4));
    }

    #[test]
    fn authored_entry_preserves_its_declared_coordinate() {
        let mut authority = LocalAuthorityStartConfig::new(17_501);
        authority.world_generation_profile = WorldGenerationProfile::authored_only();
        let plan = resolve_local_session_launch_plan(
            authority,
            ChunkPos::new(3, -2),
            2,
            LocalSessionEntryIntent::AuthoredCoordinate,
        )
        .unwrap();

        assert_eq!(plan.initial_view.center, ChunkPos::new(3, -2));
        assert_eq!(
            plan.receipt().entry.intent,
            LocalSessionEntryIntent::AuthoredCoordinate
        );
    }

    #[test]
    fn persisted_resume_uses_the_profile_center_as_its_preload_fallback() {
        let authority = LocalAuthorityStartConfig::new(553_534_047_293_117_028);
        let plan = resolve_local_session_launch_plan(
            authority,
            ChunkPos::new(9, 9),
            5,
            LocalSessionEntryIntent::PersistedPlayerOrProfilePreferred,
        )
        .unwrap();

        assert_eq!(plan.initial_view.center, ChunkPos::new(-48, 20));
        assert_eq!(plan.receipt().entry.requested_center, ChunkPos::new(9, 9));
    }

    #[test]
    fn receipt_retains_every_non_default_authority_fact() {
        let mut authority = LocalAuthorityStartConfig::new(-99);
        authority.world_generation_profile = WorldGenerationProfile::FlatGrassV1;
        authority.lighting_enabled = false;
        authority.light_status_batch_size = 17;
        authority.day_time = Some(6_000);
        authority.day_time_frozen = true;
        authority.scheduled_fluid_ticks_frozen = true;
        authority.debug_passive_showcase = false;
        authority.debug_auxiliary_player_script = true;
        authority.cadence = SimulationCadenceConfig::new(120, 30, 60);
        authority.adaptive_chunk_publication_budget = true;
        authority.observer_only = true;

        let plan = resolve_local_session_launch_plan(
            authority.clone(),
            ChunkPos::new(4, -3),
            3,
            LocalSessionEntryIntent::AuthoredCoordinate,
        )
        .unwrap();

        assert_eq!(plan.receipt().authority, authority);
        assert_eq!(plan.receipt().initial_view, plan.initial_view);
    }
}
