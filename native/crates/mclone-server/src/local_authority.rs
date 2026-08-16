use mclone_core::HorizontalTopology;
use mclone_protocol::ClientIdentity;

use crate::{
    ChunkPublicationBudgetConfig, LocalRealmSession, SimulationCadenceConfig,
    StarterContentDescriptor, WorldBehaviorProfile, WorldGenerationProfile,
};

/// Host-neutral semantics required to start one local authoritative realm.
///
/// Native threads and browser Workers carry this value whole. Storage,
/// transport, scheduling primitives, and other host resources belong in the
/// surrounding platform configuration instead of being restated here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalAuthorityStartConfig {
    pub seed: i64,
    pub world_generation_profile: WorldGenerationProfile,
    pub starter_content: StarterContentDescriptor,
    pub world_topology: HorizontalTopology,
    pub world_behavior_profile: WorldBehaviorProfile,
    pub lighting_enabled: bool,
    pub light_status_batch_size: usize,
    /// Diagnostic-only scheduler latency injection. Product starts use zero.
    pub debug_light_admission_delay_ticks: u32,
    pub day_time: Option<u64>,
    pub day_time_frozen: bool,
    pub scheduled_fluid_ticks_frozen: bool,
    pub debug_passive_showcase: bool,
    pub debug_auxiliary_player_script: bool,
    pub cadence: SimulationCadenceConfig,
    pub adaptive_chunk_publication_budget: bool,
    pub local_player_identity: Option<ClientIdentity>,
    pub observer_only: bool,
}

impl LocalAuthorityStartConfig {
    pub fn new(seed: i64) -> Self {
        Self {
            seed,
            world_generation_profile: WorldGenerationProfile::McloneOverworldV1,
            starter_content: StarterContentDescriptor::Wild,
            world_topology: HorizontalTopology::UNBOUNDED,
            world_behavior_profile: WorldBehaviorProfile::Mutable,
            lighting_enabled: true,
            light_status_batch_size: crate::DEFAULT_LIGHT_STATUS_BATCH_SIZE,
            debug_light_admission_delay_ticks: 0,
            day_time: None,
            day_time_frozen: false,
            scheduled_fluid_ticks_frozen: false,
            debug_passive_showcase: true,
            debug_auxiliary_player_script: false,
            cadence: SimulationCadenceConfig::default(),
            adaptive_chunk_publication_budget: false,
            local_player_identity: None,
            observer_only: false,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        self.world_topology
            .validate()
            .map_err(|error| error.to_string())?;
        self.world_generation_profile
            .validate_topology(self.world_topology)?;
        if self.light_status_batch_size == 0 {
            return Err("local authority light-status batch size must be positive".to_owned());
        }
        if !self.cadence.is_valid() {
            return Err(format!(
                "local authority simulation cadence is invalid: {:?}",
                self.cadence
            ));
        }
        Ok(())
    }

    pub fn publication_budget(&self) -> ChunkPublicationBudgetConfig {
        if self.adaptive_chunk_publication_budget {
            ChunkPublicationBudgetConfig::adaptive_for_gameplay_rate_hz(
                self.cadence.gameplay_rate_hz,
            )
        } else {
            ChunkPublicationBudgetConfig::disabled()
        }
    }

    /// Apply runtime policy after the host has constructed the realm and
    /// resolved any stored metadata. Identity and observer promotion remain
    /// lifecycle operations because persistence-backed and transient hosts
    /// intentionally use different readiness fences for them.
    pub fn apply_runtime_policy(&self, server: &mut LocalRealmSession) {
        server.set_lighting_enabled(self.lighting_enabled);
        server.set_light_status_batch_size(self.light_status_batch_size);
        server.set_debug_light_admission_delay_ticks(self.debug_light_admission_delay_ticks);
        server.set_publication_budget_config(self.publication_budget());
        server.set_day_time_frozen(self.day_time_frozen);
        server.set_scheduled_fluid_ticks_frozen(self.scheduled_fluid_ticks_frozen);
        server.set_debug_passive_showcase_enabled(self.debug_passive_showcase);
        server.set_debug_auxiliary_player_script_enabled(self.debug_auxiliary_player_script);
        if let Some(day_time) = self.day_time {
            server.set_day_time(day_time);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_non_default_authority_semantics() {
        let mut config = LocalAuthorityStartConfig::new(-42);
        config.world_generation_profile = WorldGenerationProfile::FlatGrassV1;
        config.world_topology = HorizontalTopology::cylinder_x(0, 32);
        config.world_behavior_profile = WorldBehaviorProfile::ProtectedLobby;
        config.lighting_enabled = false;
        config.light_status_batch_size = 17;
        config.debug_light_admission_delay_ticks = 40;
        config.day_time = Some(6_000);
        config.day_time_frozen = true;
        config.scheduled_fluid_ticks_frozen = true;
        config.debug_passive_showcase = false;
        config.debug_auxiliary_player_script = true;
        config.cadence = SimulationCadenceConfig::new(120, 30, 60);
        config.adaptive_chunk_publication_budget = true;
        config.observer_only = true;

        config.validate().unwrap();
        assert!(config.publication_budget().enabled);
        assert_eq!(config.publication_budget().gameplay_rate_hz, 30);
    }

    #[test]
    fn rejects_implicit_normalization_at_the_shared_boundary() {
        let mut config = LocalAuthorityStartConfig::new(0);
        config.light_status_batch_size = 0;
        assert!(config.validate().is_err());
    }
}
