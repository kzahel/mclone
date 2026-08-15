//! Deterministic closed-domain wildlife population diagnostics.
//!
//! This facade deliberately owns no alternate ecology rules. It establishes
//! one ordinary observer ticket over a real generated realm, freezes that
//! ticket after initial wildlife realization, advances the authoritative
//! server tick, and turns otherwise private entity facts into bounded
//! diagnostic receipts.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use mclone_core::{BlockPos, ChunkPos};
use mclone_protocol::{ChunkView, DimensionKey, EntityKind, EntityPersistentId};
use serde::{Deserialize, Serialize};

use crate::entity::ServerEntityState;
use crate::{
    ChunkStoreError, ChunkStoreResult, DimensionDefinition, LocalRealmSession, MemoryWorldStore,
    ObserverSimulationInterest, WorldGenerationProfile,
};

pub const WILDLIFE_SIMULATION_SCHEMA_VERSION: u32 = 1;
const SETUP_POLL_LIMIT: usize = 200_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WildlifeSimulationConfig {
    pub seed: i64,
    pub center_chunk_x: i32,
    pub center_chunk_z: i32,
    pub ticking_radius_chunks: u32,
}

impl WildlifeSimulationConfig {
    pub const fn center_chunk(self) -> ChunkPos {
        ChunkPos::new(self.center_chunk_x, self.center_chunk_z)
    }

    fn validate(self) -> ChunkStoreResult<Self> {
        if self.ticking_radius_chunks > 16 {
            return Err(ChunkStoreError::InvalidData(format!(
                "wildlife simulation ticking radius {} exceeds bounded maximum 16",
                self.ticking_radius_chunks
            )));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WildlifeSimulationSpecies {
    Rabbit,
    Deer,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WildlifePopulationSubject {
    pub identity_most: u64,
    pub identity_least: u64,
    pub species: WildlifeSimulationSpecies,
    pub chunk_x: i32,
    pub chunk_z: i32,
}

impl WildlifePopulationSubject {
    const fn identity(self) -> EntityPersistentId {
        EntityPersistentId::new(self.identity_most, self.identity_least)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WildlifePopulationSnapshot {
    pub schema_version: u32,
    pub simulation_tick: u64,
    pub rabbits: usize,
    pub deer: usize,
    pub subjects: Vec<WildlifePopulationSubject>,
}

impl WildlifePopulationSnapshot {
    fn from_states(simulation_tick: u64, states: Vec<ServerEntityState>) -> Self {
        let mut subjects = states
            .into_iter()
            .filter_map(subject_from_state)
            .collect::<Vec<_>>();
        subjects.sort_unstable();
        let rabbits = subjects
            .iter()
            .filter(|subject| subject.species == WildlifeSimulationSpecies::Rabbit)
            .count();
        let deer = subjects.len().saturating_sub(rabbits);
        Self {
            schema_version: WILDLIFE_SIMULATION_SCHEMA_VERSION,
            simulation_tick,
            rabbits,
            deer,
            subjects,
        }
    }

    fn identities(&self) -> BTreeSet<EntityPersistentId> {
        self.subjects
            .iter()
            .copied()
            .map(WildlifePopulationSubject::identity)
            .collect()
    }
}

#[derive(Debug)]
pub struct WildlifeSimulationSession {
    config: WildlifeSimulationConfig,
    session: LocalRealmSession,
    immutable_ticking_chunks: BTreeSet<ChunkPos>,
    initial_identities: BTreeSet<EntityPersistentId>,
    previous: WildlifePopulationSnapshot,
}

impl WildlifeSimulationSession {
    pub fn open(config: WildlifeSimulationConfig) -> ChunkStoreResult<Self> {
        let config = config.validate()?;
        let definition =
            DimensionDefinition::overworld(config.seed, WorldGenerationProfile::McloneOverworldV1);
        let mut session =
            LocalRealmSession::local_integrated_with_world_store_and_dimension_definition(
                definition,
                Box::new(MemoryWorldStore::new()),
            );
        session.set_lighting_enabled(false);
        session.set_debug_passive_showcase_enabled(false);
        session.begin_observing(
            DimensionKey::overworld(),
            ChunkView {
                center: config.center_chunk(),
                render_distance: config.ticking_radius_chunks,
                chunk_tracking_radius: config.ticking_radius_chunks,
            },
            ObserverSimulationInterest::BlockAndEntityTicking,
        )?;

        wait_for_initial_domain(&mut session)?;
        let immutable_ticking_chunks = session
            .scheduler()
            .entity_ticking_chunks()
            .into_iter()
            .collect::<BTreeSet<_>>();
        if immutable_ticking_chunks.is_empty() {
            return Err(ChunkStoreError::InvalidData(
                "wildlife simulation established an empty ticking domain".to_owned(),
            ));
        }

        // Initial seed wildlife is now realized. Generic spawning must not
        // refill this closed population during the measurement window.
        session.set_natural_spawning_enabled(false);
        let previous = WildlifePopulationSnapshot::from_states(
            session.simulation_tick(),
            session.wildlife_population_subjects(),
        );
        validate_population_snapshot(&previous, &immutable_ticking_chunks)?;
        let initial_identities = previous.identities();
        Ok(Self {
            config,
            session,
            immutable_ticking_chunks,
            initial_identities,
            previous,
        })
    }

    pub const fn config(&self) -> WildlifeSimulationConfig {
        self.config
    }

    pub fn immutable_ticking_chunks(&self) -> Vec<ChunkPos> {
        self.immutable_ticking_chunks.iter().copied().collect()
    }

    pub fn initial_snapshot(&self) -> &WildlifePopulationSnapshot {
        &self.previous
    }

    pub fn advance_tick(&mut self) -> ChunkStoreResult<WildlifePopulationSnapshot> {
        self.session.try_simulation_tick_report()?;
        let current_chunks = self
            .session
            .scheduler()
            .entity_ticking_chunks()
            .into_iter()
            .collect::<BTreeSet<_>>();
        if current_chunks != self.immutable_ticking_chunks {
            return Err(ChunkStoreError::InvalidData(format!(
                "wildlife simulation ticking domain changed at tick {}",
                self.session.simulation_tick()
            )));
        }
        let snapshot = WildlifePopulationSnapshot::from_states(
            self.session.simulation_tick(),
            self.session.wildlife_population_subjects(),
        );
        validate_population_snapshot(&snapshot, &self.immutable_ticking_chunks)?;

        // Until births and natural mortality are introduced, the first slice
        // requires strict identity conservation. This check is replaced by
        // the event ledger in the lifecycle slice.
        if snapshot.identities() != self.initial_identities {
            return Err(ChunkStoreError::InvalidData(format!(
                "closed wildlife identity set changed without a lifecycle event at tick {}",
                snapshot.simulation_tick
            )));
        }
        self.previous = snapshot.clone();
        Ok(snapshot)
    }
}

fn wait_for_initial_domain(session: &mut LocalRealmSession) -> ChunkStoreResult<()> {
    let mut last_state = String::new();
    for _ in 0..SETUP_POLL_LIMIT {
        session.try_poll()?;
        let ready = session.view_readiness_snapshot().is_some_and(|snapshot| {
            snapshot.stats.target_ready_chunks == snapshot.stats.target_chunk_count
        });
        let interest = session.realm_interest_diagnostics();
        let persistence_loaded = interest
            .dimensions
            .iter()
            .all(|dimension| dimension.pending_persistence_loads == 0);
        last_state = format!(
            "ready={ready}, pendingJobs={}, pendingPublications={}, pendingInitialWildlife={}, dimensions={:?}",
            session.pending_job_count(),
            session.pending_publication_count(),
            session.pending_initial_wildlife_entity_ticking_chunk_count(),
            interest.dimensions
        );
        if ready
            && persistence_loaded
            && session.pending_job_count() == 0
            && session.pending_publication_count() == 0
            && session.pending_initial_wildlife_entity_ticking_chunk_count() == 0
        {
            return Ok(());
        }
        if session.pending_job_count() > 0 && session.pending_publication_count() == 0 {
            let _ = session.wait_for_worldgen_completion(Duration::from_secs(1));
        }
    }
    Err(ChunkStoreError::InvalidData(format!(
        "timed out establishing closed wildlife simulation domain: {last_state}"
    )))
}

fn subject_from_state(state: ServerEntityState) -> Option<WildlifePopulationSubject> {
    let species = match state.kind {
        EntityKind::Rabbit => WildlifeSimulationSpecies::Rabbit,
        EntityKind::Deer => WildlifeSimulationSpecies::Deer,
        _ => return None,
    };
    let chunk = BlockPos::containing(state.position).chunk_pos();
    Some(WildlifePopulationSubject {
        identity_most: state.persistent_id.most,
        identity_least: state.persistent_id.least,
        species,
        chunk_x: chunk.x,
        chunk_z: chunk.z,
    })
}

fn validate_population_snapshot(
    snapshot: &WildlifePopulationSnapshot,
    domain: &BTreeSet<ChunkPos>,
) -> ChunkStoreResult<()> {
    let mut identities = BTreeMap::new();
    for subject in &snapshot.subjects {
        let identity = subject.identity();
        if let Some(previous_species) = identities.insert(identity, subject.species) {
            return Err(ChunkStoreError::InvalidData(format!(
                "duplicate wildlife identity {identity} ({previous_species:?}, {:?})",
                subject.species
            )));
        }
        let chunk = ChunkPos::new(subject.chunk_x, subject.chunk_z);
        if !domain.contains(&chunk) {
            return Err(ChunkStoreError::InvalidData(format!(
                "boundaryViolation: wildlife {identity} reached chunk ({}, {}) at tick {}",
                chunk.x, chunk.z, snapshot.simulation_tick
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_seed_session_freezes_domain_and_initial_identities() {
        let mut simulation = WildlifeSimulationSession::open(WildlifeSimulationConfig {
            seed: 31_415,
            center_chunk_x: 0,
            center_chunk_z: 0,
            ticking_radius_chunks: 1,
        })
        .unwrap();
        let domain = simulation.immutable_ticking_chunks();
        let initial = simulation.initial_snapshot().clone();
        for _ in 0..40 {
            simulation.advance_tick().unwrap();
        }
        assert_eq!(simulation.immutable_ticking_chunks(), domain);
        assert_eq!(simulation.previous.subjects, initial.subjects);
    }
}
