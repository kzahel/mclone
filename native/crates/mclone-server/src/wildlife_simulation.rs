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
use mclone_protocol::{
    ChunkView, DeerLifeStage, DeerSex, DimensionKey, EntityKind, EntityPersistentId,
    RabbitLifeStage,
};
use serde::{Deserialize, Serialize};

use crate::ecology::{
    WildlifeDeathCause, WildlifeEcologyEvent, WildlifeEcologyEventKind,
    WildlifeReproductionSuppression, WildlifeSpecies,
};
use crate::entity::{WildlifeLifeDiagnostic, WildlifeRemainsDiagnostic};
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
pub struct WildlifeSimulationIdentity {
    pub most: u64,
    pub least: u64,
}

impl From<EntityPersistentId> for WildlifeSimulationIdentity {
    fn from(value: EntityPersistentId) -> Self {
        Self {
            most: value.most,
            least: value.least,
        }
    }
}

impl From<WildlifeSimulationIdentity> for EntityPersistentId {
    fn from(value: WildlifeSimulationIdentity) -> Self {
        Self::new(value.most, value.least)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WildlifeSimulationLifeStage {
    Young,
    Adult,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WildlifeSimulationSex {
    Female,
    Male,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WildlifePopulationSubject {
    pub identity_most: u64,
    pub identity_least: u64,
    pub species: WildlifeSimulationSpecies,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub life_stage: WildlifeSimulationLifeStage,
    pub sex: WildlifeSimulationSex,
    pub age_ticks: u32,
    pub lifespan_ticks: u32,
    pub energy: u16,
    pub deficit_ticks: u32,
    pub recent_intake: u16,
    pub reproductive_condition: u16,
    pub reproduction_cooldown: u32,
    pub parents: [Option<WildlifeSimulationIdentity>; 2],
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WildlifeSimulationDeathCause {
    OldAge,
    Starvation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WildlifeSimulationSuppressionReason {
    LowCondition,
    Cooldown,
    Crowding,
    NoMate,
    NoRefugeCapacity,
    HardOverload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum WildlifeSimulationEventKind {
    Intake {
        amount: u16,
    },
    Birth {
        child: WildlifeSimulationIdentity,
        parents: [WildlifeSimulationIdentity; 2],
    },
    Death {
        cause: WildlifeSimulationDeathCause,
    },
    RemainsCreated {
        biomass: u32,
    },
    RemainsDecayed {
        amount: u32,
    },
    ReproductionSuppressed {
        reason: WildlifeSimulationSuppressionReason,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WildlifeSimulationRemainsSnapshot {
    pub identity: WildlifeSimulationIdentity,
    pub source_species: WildlifeSimulationSpecies,
    pub source: WildlifeSimulationIdentity,
    pub biomass: u32,
    pub cause: WildlifeSimulationDeathCause,
    pub creation_tick: u64,
    pub chunk_x: i32,
    pub chunk_z: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WildlifeSimulationEvent {
    pub tick: u64,
    pub species: WildlifeSimulationSpecies,
    pub subject: WildlifeSimulationIdentity,
    pub event: WildlifeSimulationEventKind,
}

impl WildlifePopulationSnapshot {
    fn from_diagnostics(simulation_tick: u64, states: Vec<WildlifeLifeDiagnostic>) -> Self {
        let mut subjects = states
            .into_iter()
            .filter_map(subject_from_diagnostic)
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
    pending_events: Vec<WildlifeSimulationEvent>,
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
        let previous = WildlifePopulationSnapshot::from_diagnostics(
            session.simulation_tick(),
            session.wildlife_life_diagnostics(),
        );
        validate_population_snapshot(&previous, &immutable_ticking_chunks)?;
        let initial_identities = previous.identities();
        Ok(Self {
            config,
            session,
            immutable_ticking_chunks,
            initial_identities,
            previous,
            pending_events: Vec::new(),
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

    pub fn initial_identities(&self) -> Vec<WildlifeSimulationIdentity> {
        self.initial_identities
            .iter()
            .copied()
            .map(Into::into)
            .collect()
    }

    pub fn advance_tick(&mut self) -> ChunkStoreResult<WildlifePopulationSnapshot> {
        self.session.try_simulation_tick_report()?;
        let events = self
            .session
            .drain_wildlife_ecology_events()
            .into_iter()
            .map(simulation_event_from_ecology)
            .collect::<Vec<_>>();
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
        let snapshot = WildlifePopulationSnapshot::from_diagnostics(
            self.session.simulation_tick(),
            self.session.wildlife_life_diagnostics(),
        );
        validate_population_snapshot(&snapshot, &self.immutable_ticking_chunks)?;

        let previous_identities = self.previous.identities();
        let current_identities = snapshot.identities();
        let added = current_identities
            .difference(&previous_identities)
            .copied()
            .collect::<BTreeSet<_>>();
        let removed = previous_identities
            .difference(&current_identities)
            .copied()
            .collect::<BTreeSet<_>>();
        let expected_births = events
            .iter()
            .filter_map(|event| match event.event {
                WildlifeSimulationEventKind::Birth { child, .. } => Some(child.into()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let expected_deaths = events
            .iter()
            .filter_map(|event| match event.event {
                WildlifeSimulationEventKind::Death { .. } => Some(event.subject.into()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        if added != expected_births || removed != expected_deaths {
            return Err(ChunkStoreError::InvalidData(format!(
                "wildlife identity conservation failed at tick {}: added={added:?}, births={expected_births:?}, removed={removed:?}, deaths={expected_deaths:?}",
                snapshot.simulation_tick,
            )));
        }
        self.pending_events.extend(events);
        self.previous = snapshot.clone();
        Ok(snapshot)
    }

    pub fn drain_events(&mut self) -> Vec<WildlifeSimulationEvent> {
        std::mem::take(&mut self.pending_events)
    }

    pub fn forage_cells(&self) -> Vec<crate::WildlifeForageCellSnapshot> {
        self.session.wildlife_forage_cells()
    }

    pub fn remains(&self) -> Vec<WildlifeSimulationRemainsSnapshot> {
        let mut remains = self
            .session
            .wildlife_remains_diagnostics()
            .into_iter()
            .map(simulation_remains_from_diagnostic)
            .collect::<Vec<_>>();
        remains.sort_unstable_by_key(|entry| entry.identity);
        remains
    }

    pub fn tuning(&self) -> crate::WildlifeLifecycleTuning {
        self.session.wildlife_lifecycle_tuning()
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

fn subject_from_diagnostic(state: WildlifeLifeDiagnostic) -> Option<WildlifePopulationSubject> {
    let species = match state.kind {
        EntityKind::Rabbit => WildlifeSimulationSpecies::Rabbit,
        EntityKind::Deer => WildlifeSimulationSpecies::Deer,
        _ => return None,
    };
    let chunk = BlockPos::containing(state.position).chunk_pos();
    let life_stage = match (state.rabbit_life_stage, state.deer_life_stage) {
        (Some(RabbitLifeStage::Kit), _) | (_, Some(DeerLifeStage::Fawn)) => {
            WildlifeSimulationLifeStage::Young
        }
        (Some(RabbitLifeStage::Adult), _) | (_, Some(DeerLifeStage::Adult)) => {
            WildlifeSimulationLifeStage::Adult
        }
        _ => return None,
    };
    let sex = match state.deer_sex {
        Some(DeerSex::Female) => WildlifeSimulationSex::Female,
        Some(DeerSex::Male) => WildlifeSimulationSex::Male,
        None => WildlifeSimulationSex::Unknown,
    };
    Some(WildlifePopulationSubject {
        identity_most: state.persistent_id.most,
        identity_least: state.persistent_id.least,
        species,
        chunk_x: chunk.x,
        chunk_z: chunk.z,
        life_stage,
        sex,
        age_ticks: state.lifecycle.age_ticks,
        lifespan_ticks: state.lifecycle.lifespan_ticks,
        energy: state.lifecycle.energy,
        deficit_ticks: state.lifecycle.deficit_ticks,
        recent_intake: state.lifecycle.recent_intake,
        reproductive_condition: state.lifecycle.reproductive_condition,
        reproduction_cooldown: state.lifecycle.reproduction_cooldown,
        parents: state.parents.map(|parent| parent.map(Into::into)),
    })
}

fn simulation_event_from_ecology(event: WildlifeEcologyEvent) -> WildlifeSimulationEvent {
    WildlifeSimulationEvent {
        tick: event.tick,
        species: match event.species {
            WildlifeSpecies::Rabbit => WildlifeSimulationSpecies::Rabbit,
            WildlifeSpecies::Deer => WildlifeSimulationSpecies::Deer,
        },
        subject: event.subject.into(),
        event: match event.kind {
            WildlifeEcologyEventKind::Intake { amount } => {
                WildlifeSimulationEventKind::Intake { amount }
            }
            WildlifeEcologyEventKind::Birth { child, parents } => {
                WildlifeSimulationEventKind::Birth {
                    child: child.into(),
                    parents: parents.map(Into::into),
                }
            }
            WildlifeEcologyEventKind::Death { cause } => WildlifeSimulationEventKind::Death {
                cause: match cause {
                    WildlifeDeathCause::OldAge => WildlifeSimulationDeathCause::OldAge,
                    WildlifeDeathCause::Starvation => WildlifeSimulationDeathCause::Starvation,
                },
            },
            WildlifeEcologyEventKind::RemainsCreated { biomass } => {
                WildlifeSimulationEventKind::RemainsCreated { biomass }
            }
            WildlifeEcologyEventKind::RemainsDecayed { amount } => {
                WildlifeSimulationEventKind::RemainsDecayed { amount }
            }
            WildlifeEcologyEventKind::ReproductionSuppressed { reason } => {
                WildlifeSimulationEventKind::ReproductionSuppressed {
                    reason: match reason {
                        WildlifeReproductionSuppression::LowCondition => {
                            WildlifeSimulationSuppressionReason::LowCondition
                        }
                        WildlifeReproductionSuppression::Cooldown => {
                            WildlifeSimulationSuppressionReason::Cooldown
                        }
                        WildlifeReproductionSuppression::Crowding => {
                            WildlifeSimulationSuppressionReason::Crowding
                        }
                        WildlifeReproductionSuppression::NoMate => {
                            WildlifeSimulationSuppressionReason::NoMate
                        }
                        WildlifeReproductionSuppression::NoRefugeCapacity => {
                            WildlifeSimulationSuppressionReason::NoRefugeCapacity
                        }
                        WildlifeReproductionSuppression::HardOverload => {
                            WildlifeSimulationSuppressionReason::HardOverload
                        }
                    },
                }
            }
        },
    }
}

fn simulation_remains_from_diagnostic(
    remains: WildlifeRemainsDiagnostic,
) -> WildlifeSimulationRemainsSnapshot {
    let chunk = BlockPos::containing(remains.position).chunk_pos();
    WildlifeSimulationRemainsSnapshot {
        identity: remains.persistent_id.into(),
        source_species: match remains.source_species {
            crate::WildlifeRemainsSpecies::Rabbit => WildlifeSimulationSpecies::Rabbit,
            crate::WildlifeRemainsSpecies::Deer => WildlifeSimulationSpecies::Deer,
        },
        source: remains.source.into(),
        biomass: remains.biomass,
        cause: match remains.cause {
            crate::WildlifeRemainsCause::OldAge => WildlifeSimulationDeathCause::OldAge,
            crate::WildlifeRemainsCause::Starvation => WildlifeSimulationDeathCause::Starvation,
        },
        creation_tick: remains.creation_tick,
        chunk_x: chunk.x,
        chunk_z: chunk.z,
    }
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
