use std::collections::{BTreeMap, BTreeSet};

use mclone_blocks::{collide_movement, collision_aabb_for_feet_position};
use mclone_core::{
    Aabb, AnimationClipId, AnimationState, AxisTopology, BlockPos, BlockStateId, ChunkPos,
    HorizontalTopology, Vec3d,
};
#[cfg(feature = "physics-engine")]
use mclone_protocol::EntityRotation;
use mclone_protocol::{
    BeeBehavior, BeeSoundCue, DeerSoundCue, DeerSoundKind, EntityId, EntityKind,
    EntityPersistentId, ItemKind, ItemStackSnapshot, MallardCallCue, MallardLifeStage,
    MallardNestSnapshotData, MallardSex, MallardSnapshotData, MallardTrackCue, RabbitBehavior,
    RabbitLifeStage, RabbitSoundCue,
};

use crate::ecology::{
    DecisionSchedule, EcologyWorkBudget, EcologyWorkClass, EcologyWorkDiagnostics, KnownPlace,
    WildlifeEcologyEvent, WildlifeEcologyEventKind, WildlifeLifeState, WildlifeLifecycleTuning,
    WildlifeReproductionSuppression, WildlifeSpecies, WorldFactLocator,
};
use crate::persistence::{
    ChunkStoreError, ChunkStoreResult, ENTITY_CHUNK_RECORD_VERSION, EntityChunkRecord,
    EntitySavePayload, EntitySaveRecord, ItemStackSaveRecord, RabbitRefugeSaveRecord,
    WildlifeRemainsCause, WildlifeRemainsSpecies,
};
use crate::players::ServerPlayerId;
use crate::wildlife_resources::{
    DEER_DIET, MALLARD_DIET, RABBIT_DIET, WildlifeDietEntry, WildlifeForageConsumer,
    WildlifeResourceLedger,
};

use super::ServerEntityState;
use super::item::{ITEM_ENTITY_LIFETIME_TICKS, ItemEntityRuntimeState};
use super::metadata::{EntityMetadata, PASSIVE_MOB_KINDS};
use super::mob::{
    BeeRuntimeSaveData, DeerHerdmateTarget, DeerRuntimeSaveData, MallardFlockmateTarget,
    MallardRuntimeSaveData, MobPlayerTarget, MobRuntimeState, RabbitEcologyAdmission,
    RabbitRefugeCandidate, RabbitRuntimeSaveData, identity_mallard_sex,
};
use super::spawning::habitat::sample_wetland_habitat;
use super::spawning::mob_category::MobCategory;
use super::spawning::spawn_state::MobCategoryCounts;
use super::tick_list::ServerEntityTickList;

const PLAYER_PICKUP_WIDTH: f64 = 0.6;
const PLAYER_PICKUP_HEIGHT: f64 = 1.8;
const PLAYER_PICKUP_INFLATE_XZ: f64 = 1.0;
const PLAYER_PICKUP_INFLATE_Y: f64 = 0.5;
const ITEM_MERGE_INFLATE_XZ: f64 = 0.5;
const ITEM_STATIONARY_MERGE_INTERVAL_TICKS: u64 = 40;
const ITEM_MOVED_BLOCK_MERGE_INTERVAL_TICKS: u64 = 2;
const ENTITY_PERSISTENT_ID_MOST: u64 = 0x6d63_6c6f_6e65_0001;
pub(crate) const MALLARD_NEST_INCUBATION_REQUIRED_TICKS: u32 = 2_400;
pub(crate) const BEE_COLONY_WORK_CAPACITY: u32 = 4;
const BEE_POLLINATION_COOLDOWN_TICKS: u32 = 200;
const BEE_BUZZ_AUDIBLE_RADIUS: f32 = 14.0;
const BEE_BUZZ_COLONY_SUPPRESSION_RADIUS_SQR: f64 = 12.0 * 12.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DeerAttackResult {
    pub(crate) state: ServerEntityState,
    pub(crate) killed: bool,
}
const MALLARD_NEST_ATTENDANCE_RADIUS_SQR: f64 = 8.0 * 8.0;
const MALLARD_NEST_TARGET_REACHED_DISTANCE_SQR: f64 = 0.35 * 0.35;
const MALLARD_CALL_AUDIBLE_RADIUS: f32 = 24.0;
const MALLARD_CALL_FLOCK_SUPPRESSION_RADIUS_SQR: f64 = 12.0 * 12.0;
const MALLARD_TRACK_SPACING_SQR: f64 = 2.0 * 2.0;
const DEER_SOUND_HERD_SUPPRESSION_RADIUS_SQR: f64 = 12.0 * 12.0;
const RABBIT_SOUND_AUDIBLE_RADIUS: f32 = 16.0;
const RABBIT_RAID_COOLDOWN_TICKS: u32 = 600;
const RABBIT_LOVE_TICKS: u32 = 600;
const RABBIT_PAIR_PUSH_MAX: f64 = 0.04;
const RABBIT_NEIGHBORS_PER_CELL: usize = 2;
const RABBIT_BURROW_DISTURBANCE_PER_HIT: u32 = 40;
const RABBIT_BURROW_COLLAPSE_DAMAGE: u8 = 3;
const RABBIT_DECISION_WORK_PER_TICK: u32 = 64;
const RABBIT_HABITAT_WORK_PER_TICK: u32 = 32;
const RABBIT_PATH_WORK_PER_TICK: u32 = 32;
const WILDLIFE_REMAINS_MAX_PER_CELL: usize = 8;
const WILDLIFE_REMAINS_DECAY_DENOMINATOR: u32 = 1_200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MallardNestRuntimeState {
    incubation_progress: u32,
    incubation_required: u32,
    parents: [Option<EntityPersistentId>; 2],
    attended: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeerBedRuntimeState {
    source: EntityPersistentId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BeeColonyRuntimeState {
    colonized: bool,
    stored_work: u32,
    work_capacity: u32,
    spread_cooldown: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RabbitBurrowRuntimeState {
    capacity: u8,
    disturbance_ticks: u32,
    damage: u8,
    last_used_tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WildlifeRemainsRuntimeState {
    source_species: WildlifeRemainsSpecies,
    source: EntityPersistentId,
    biomass: u32,
    cause: WildlifeRemainsCause,
    creation_tick: u64,
    decay_remainder: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BeePollinationEvent {
    pub(crate) source_flower: BlockPos,
    pub(crate) colony: EntityPersistentId,
    pub(crate) position: Vec3d,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RabbitDigEvent {
    pub(crate) rabbit: EntityId,
    pub(crate) target: BlockPos,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RabbitRaidEvent {
    pub(crate) rabbit: EntityId,
    pub(crate) target: BlockPos,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HabitatPropDamageOutcome {
    Disturbed,
    Collapsed,
}

#[derive(Debug, PartialEq)]
pub(crate) struct HabitatPropDamageResult {
    pub(crate) outcome: HabitatPropDamageOutcome,
    pub(crate) updates: Vec<ServerEntityState>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ItemPickupTarget {
    pub(crate) player_id: ServerPlayerId,
    pub(crate) position: Vec3d,
}

#[derive(Debug, Default)]
pub(crate) struct ServerEntityStore {
    topology: HorizontalTopology,
    entities: BTreeMap<EntityId, ServerEntityState>,
    mobs: BTreeMap<EntityId, MobRuntimeState>,
    items: BTreeMap<EntityId, ItemEntityRuntimeState>,
    mallard_nests: BTreeMap<EntityId, MallardNestRuntimeState>,
    deer_beds: BTreeMap<EntityId, DeerBedRuntimeState>,
    bee_colonies: BTreeMap<EntityId, BeeColonyRuntimeState>,
    rabbit_burrows: BTreeMap<EntityId, RabbitBurrowRuntimeState>,
    wildlife_remains: BTreeMap<EntityId, WildlifeRemainsRuntimeState>,
    deer_bedded_site_ticks: BTreeMap<EntityPersistentId, (BlockPos, u32)>,
    mallard_last_tracks: BTreeMap<EntityId, Vec3d>,
    pending_mallard_calls: Vec<MallardCallCue>,
    pending_mallard_tracks: Vec<MallardTrackCue>,
    pending_deer_sounds: Vec<DeerSoundCue>,
    pending_bee_sounds: Vec<BeeSoundCue>,
    pending_rabbit_sounds: Vec<RabbitSoundCue>,
    pending_rabbit_digs: Vec<RabbitDigEvent>,
    pending_rabbit_raids: Vec<RabbitRaidEvent>,
    pending_bee_pollinations: Vec<BeePollinationEvent>,
    hatched_mallard_positions: Vec<Vec3d>,
    mallard_cue_sequence: u64,
    deer_cue_sequence: u64,
    bee_cue_sequence: u64,
    rabbit_cue_sequence: u64,
    last_rabbit_ecology: RabbitEcologyTickDiagnostics,
    wildlife_tuning: WildlifeLifecycleTuning,
    pending_wildlife_events: Vec<WildlifeEcologyEvent>,
    persistent_ids: BTreeMap<EntityId, EntityPersistentId>,
    volatile_entities: BTreeSet<EntityId>,
    tick_list: ServerEntityTickList,
    next_entity_id: u64,
    next_persistent_id: u64,
    debug_passive_showcase_ids: Vec<(EntityKind, EntityId)>,
    provisional_debug_passive_showcase_ids: BTreeSet<EntityId>,
    debug_periodic_showcase_phase: u64,
    debug_periodic_showcase_crossings: u64,
    debug_periodic_showcase_last_x: Option<f64>,
    #[cfg(feature = "physics-engine")]
    debug_physics_cube_id: Option<EntityId>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RabbitEcologyTickDiagnostics {
    pub(crate) active: u32,
    pub(crate) due: u32,
    pub(crate) work: EcologyWorkDiagnostics,
    pub(crate) habitat_candidates: u32,
    pub(crate) neighbor_candidates: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WildlifeLifeDiagnostic {
    pub(crate) persistent_id: EntityPersistentId,
    pub(crate) kind: EntityKind,
    pub(crate) position: Vec3d,
    pub(crate) rabbit_life_stage: Option<RabbitLifeStage>,
    pub(crate) deer_life_stage: Option<mclone_protocol::DeerLifeStage>,
    pub(crate) deer_sex: Option<mclone_protocol::DeerSex>,
    pub(crate) mallard_life_stage: Option<MallardLifeStage>,
    pub(crate) mallard_sex: Option<MallardSex>,
    pub(crate) rabbit_behavior: Option<RabbitBehavior>,
    pub(crate) deer_behavior: Option<mclone_protocol::DeerBehavior>,
    pub(crate) rabbit_has_refuge: bool,
    pub(crate) rabbit_sheltered: bool,
    pub(crate) parents: [Option<EntityPersistentId>; 2],
    pub(crate) lifecycle: WildlifeLifeState,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WildlifeRemainsDiagnostic {
    pub(crate) persistent_id: EntityPersistentId,
    pub(crate) source_species: WildlifeRemainsSpecies,
    pub(crate) source: EntityPersistentId,
    pub(crate) biomass: u32,
    pub(crate) cause: WildlifeRemainsCause,
    pub(crate) creation_tick: u64,
    pub(crate) position: Vec3d,
}

#[cfg(feature = "physics-engine")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DebugPhysicsCubeEntitySpawn {
    pub(crate) removed: Option<ServerEntityState>,
    pub(crate) current: ServerEntityState,
}

impl ServerEntityStore {
    pub(crate) fn with_topology(topology: HorizontalTopology) -> Self {
        topology
            .validate()
            .expect("entity store requires validated topology");
        Self {
            topology,
            ..Self::default()
        }
    }

    pub(crate) fn ensure_debug_passive_showcase_near_spawn(
        &mut self,
        spawn_position: Vec3d,
        enabled: bool,
    ) -> Vec<EntityId> {
        if !enabled {
            return Vec::new();
        }

        let mut ids = Vec::with_capacity(PASSIVE_MOB_KINDS.len());
        for (index, kind) in PASSIVE_MOB_KINDS.iter().copied().enumerate() {
            if let Some(id) = self
                .debug_passive_showcase_ids
                .iter()
                .find_map(|(stored_kind, id)| (*stored_kind == kind).then_some(*id))
            {
                ids.push(id);
                continue;
            }
            if let Some(id) = self.entities.values().find_map(|entity| {
                (entity.alive
                    && entity.kind == kind
                    && !self.volatile_entities.contains(&entity.id)
                    && !self
                        .debug_passive_showcase_ids
                        .iter()
                        .any(|(_, registered_id)| *registered_id == entity.id))
                .then_some(entity.id)
            }) {
                self.debug_passive_showcase_ids.push((kind, id));
                ids.push(id);
                continue;
            }

            let id = self.allocate_entity_id();
            let mut position = debug_passive_showcase_position(spawn_position, index);
            if index == 0
                && let AxisTopology::Periodic {
                    minimum_chunk,
                    period_chunks,
                } = self.topology.x
            {
                let minimum_block = f64::from(minimum_chunk) * 16.0;
                position.x = minimum_block + f64::from(period_chunks) * 16.0 - 2.0;
            }
            let y_rot_degrees = debug_passive_showcase_y_rot(index);
            self.insert_passive_mob(id, kind, position, y_rot_degrees);
            self.debug_passive_showcase_ids.push((kind, id));
            self.provisional_debug_passive_showcase_ids.insert(id);
            ids.push(id);
        }
        ids
    }

    pub(crate) fn adopt_hydrated_debug_passive_showcase(
        &mut self,
        hydrated: &[ServerEntityState],
    ) -> Vec<ServerEntityState> {
        let mut removed = Vec::new();
        for entity in hydrated.iter().copied() {
            let Some(index) = self
                .debug_passive_showcase_ids
                .iter()
                .position(|(kind, _)| *kind == entity.kind)
            else {
                continue;
            };
            let provisional_id = self.debug_passive_showcase_ids[index].1;
            if !self
                .provisional_debug_passive_showcase_ids
                .remove(&provisional_id)
            {
                continue;
            }
            self.debug_passive_showcase_ids[index].1 = entity.id;
            if let Some(removed_entity) = self.remove_entity(provisional_id) {
                removed.push(removed_entity);
            }
        }
        removed
    }

    pub(crate) fn advance_debug_periodic_showcase(&mut self) -> Option<ServerEntityState> {
        let AxisTopology::Periodic {
            minimum_chunk,
            period_chunks,
        } = self.topology.x
        else {
            return None;
        };
        let id = self.debug_passive_showcase_ids.first()?.1;
        let entity = self.entities.get_mut(&id)?;
        if !entity.alive {
            return None;
        }

        const HALF_CYCLE_TICKS: u64 = 80;
        const HALF_SPAN_BLOCKS: f64 = 2.0;
        let phase = self.debug_periodic_showcase_phase % (HALF_CYCLE_TICKS * 2);
        let progress =
            f64::from((phase % HALF_CYCLE_TICKS) as u32) / f64::from(HALF_CYCLE_TICKS as u32);
        let offset = if phase < HALF_CYCLE_TICKS {
            -HALF_SPAN_BLOCKS + progress * HALF_SPAN_BLOCKS * 2.0
        } else {
            HALF_SPAN_BLOCKS - progress * HALF_SPAN_BLOCKS * 2.0
        };
        let minimum_block = f64::from(minimum_chunk) * 16.0;
        let period_blocks = f64::from(period_chunks) * 16.0;
        let lifted_x = minimum_block + period_blocks + offset;
        let canonical_position = self.topology.canonicalize_position(Vec3d::new(
            lifted_x,
            entity.position.y,
            entity.position.z,
        ))?;
        if self
            .debug_periodic_showcase_last_x
            .is_some_and(|previous| (canonical_position.x - previous).abs() > period_blocks * 0.5)
        {
            self.debug_periodic_showcase_crossings =
                self.debug_periodic_showcase_crossings.saturating_add(1);
        }
        self.debug_periodic_showcase_last_x = Some(canonical_position.x);
        self.debug_periodic_showcase_phase = self.debug_periodic_showcase_phase.saturating_add(1);
        entity.position = canonical_position;
        entity.y_rot_degrees = if phase < HALF_CYCLE_TICKS {
            -90.0
        } else {
            90.0
        };
        entity.on_ground = true;
        Some(*entity)
    }

    #[cfg(test)]
    pub(crate) fn debug_periodic_showcase_crossings(&self) -> u64 {
        self.debug_periodic_showcase_crossings
    }

    #[cfg(test)]
    pub(crate) fn insert_passive_mob_for_test(
        &mut self,
        kind: EntityKind,
        position: Vec3d,
        y_rot_degrees: f32,
    ) -> EntityId {
        let id = self.allocate_entity_id();
        self.insert_passive_mob(id, kind, position, y_rot_degrees);
        id
    }

    #[cfg(feature = "physics-engine")]
    pub(crate) fn spawn_debug_physics_cube(
        &mut self,
        position: Vec3d,
        y_rot_degrees: f32,
        x_rot_degrees: f32,
        rotation: EntityRotation,
        tick_count: u64,
    ) -> DebugPhysicsCubeEntitySpawn {
        let removed = self
            .debug_physics_cube_id
            .take()
            .and_then(|id| self.entities.remove(&id))
            .map(|mut state| {
                state.alive = false;
                state
            });
        let current = self.insert_debug_physics_cube(
            position,
            y_rot_degrees,
            x_rot_degrees,
            rotation,
            tick_count,
        );
        DebugPhysicsCubeEntitySpawn { removed, current }
    }

    #[cfg(feature = "physics-engine")]
    pub(crate) fn upsert_debug_physics_cube(
        &mut self,
        position: Vec3d,
        y_rot_degrees: f32,
        x_rot_degrees: f32,
        rotation: EntityRotation,
        tick_count: u64,
    ) -> ServerEntityState {
        let position = self
            .topology
            .canonicalize_position(position)
            .expect("debug physics entity must lie inside the dimension topology");
        if let Some(id) = self.debug_physics_cube_id {
            let persistent_id = self
                .entities
                .get(&id)
                .expect("debug physics cube id must retain entity state")
                .persistent_id;
            let state = debug_physics_cube_state(
                id,
                persistent_id,
                position,
                y_rot_degrees,
                x_rot_degrees,
                rotation,
                tick_count,
            );
            self.entities.insert(id, state);
            return state;
        }
        self.insert_debug_physics_cube(position, y_rot_degrees, x_rot_degrees, rotation, tick_count)
    }

    #[cfg(feature = "physics-engine")]
    fn insert_debug_physics_cube(
        &mut self,
        position: Vec3d,
        y_rot_degrees: f32,
        x_rot_degrees: f32,
        rotation: EntityRotation,
        tick_count: u64,
    ) -> ServerEntityState {
        let position = self
            .topology
            .canonicalize_position(position)
            .expect("debug physics entity must lie inside the dimension topology");
        let id = self.allocate_entity_id();
        let persistent_id = self.allocate_persistent_id();
        self.debug_physics_cube_id = Some(id);
        let state = debug_physics_cube_state(
            id,
            persistent_id,
            position,
            y_rot_degrees,
            x_rot_degrees,
            rotation,
            tick_count,
        );
        self.entities.insert(id, state);
        state
    }

    pub(crate) fn states(&self) -> Vec<ServerEntityState> {
        self.entities.values().copied().collect()
    }

    pub(crate) fn set_wildlife_tuning(&mut self, tuning: WildlifeLifecycleTuning) {
        self.wildlife_tuning = tuning;
    }

    pub(crate) const fn wildlife_tuning(&self) -> WildlifeLifecycleTuning {
        self.wildlife_tuning
    }

    pub(crate) fn drain_wildlife_events(&mut self) -> Vec<WildlifeEcologyEvent> {
        std::mem::take(&mut self.pending_wildlife_events)
    }

    pub(crate) fn wildlife_life_diagnostics(&self) -> Vec<WildlifeLifeDiagnostic> {
        self.entities
            .values()
            .filter(|entity| entity.alive)
            .filter_map(|entity| {
                let mob = self.mobs.get(&entity.id)?;
                match entity.kind {
                    EntityKind::Rabbit => {
                        let rabbit = mob.rabbit_save_data()?;
                        Some(WildlifeLifeDiagnostic {
                            persistent_id: entity.persistent_id,
                            kind: entity.kind,
                            position: entity.position,
                            rabbit_life_stage: Some(rabbit.life_stage),
                            deer_life_stage: None,
                            deer_sex: None,
                            mallard_life_stage: None,
                            mallard_sex: None,
                            rabbit_behavior: Some(rabbit.behavior),
                            deer_behavior: None,
                            rabbit_has_refuge: rabbit.known_refuges.iter().any(Option::is_some),
                            rabbit_sheltered: rabbit.sheltered_in.is_some(),
                            parents: rabbit.parents,
                            lifecycle: rabbit.lifecycle,
                        })
                    }
                    EntityKind::Deer => {
                        let deer = mob.deer_save_data()?;
                        Some(WildlifeLifeDiagnostic {
                            persistent_id: entity.persistent_id,
                            kind: entity.kind,
                            position: entity.position,
                            rabbit_life_stage: None,
                            deer_life_stage: Some(deer.life_stage),
                            deer_sex: Some(deer.sex),
                            mallard_life_stage: None,
                            mallard_sex: None,
                            rabbit_behavior: None,
                            deer_behavior: Some(deer.behavior),
                            rabbit_has_refuge: false,
                            rabbit_sheltered: false,
                            parents: [None; 2],
                            lifecycle: deer.lifecycle,
                        })
                    }
                    EntityKind::Mallard => {
                        let mallard = mob.mallard_save_data()?;
                        Some(WildlifeLifeDiagnostic {
                            persistent_id: entity.persistent_id,
                            kind: entity.kind,
                            position: entity.position,
                            rabbit_life_stage: None,
                            deer_life_stage: None,
                            deer_sex: None,
                            mallard_life_stage: Some(mallard.life_stage),
                            mallard_sex: Some(mallard.sex),
                            rabbit_behavior: None,
                            deer_behavior: None,
                            rabbit_has_refuge: false,
                            rabbit_sheltered: false,
                            parents: mallard.parents,
                            lifecycle: mallard.lifecycle,
                        })
                    }
                    _ => None,
                }
            })
            .collect()
    }

    pub(crate) fn wildlife_remains_diagnostics(&self) -> Vec<WildlifeRemainsDiagnostic> {
        self.wildlife_remains
            .iter()
            .filter_map(|(id, remains)| {
                let entity = self.entities.get(id).filter(|entity| entity.alive)?;
                Some(WildlifeRemainsDiagnostic {
                    persistent_id: entity.persistent_id,
                    source_species: remains.source_species,
                    source: remains.source,
                    biomass: remains.biomass,
                    cause: remains.cause,
                    creation_tick: remains.creation_tick,
                    position: entity.position,
                })
            })
            .collect()
    }

    pub(crate) fn tick_wildlife_lifecycle<F>(
        &mut self,
        simulation_tick: u64,
        entity_ticking_chunks: &[ChunkPos],
        resources: &mut WildlifeResourceLedger,
        block_state_at: &F,
    ) -> Vec<ServerEntityState>
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let tuning = self.wildlife_tuning;
        if tuning.cadence_ticks == 0
            || !simulation_tick.is_multiple_of(u64::from(tuning.cadence_ticks))
        {
            return Vec::new();
        }
        let ticking_chunks = entity_ticking_chunks
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        resources.recover_loaded(&ticking_chunks);
        let mut wildlife_ids = self
            .entities
            .values()
            .filter(|entity| {
                entity.alive
                    && ticking_chunks.contains(&entity.chunk_pos())
                    && matches!(
                        entity.kind,
                        EntityKind::Rabbit | EntityKind::Deer | EntityKind::Mallard
                    )
            })
            .map(|entity| (entity.persistent_id, entity.id))
            .collect::<Vec<_>>();
        wildlife_ids.sort_unstable();
        let overloaded = wildlife_ids.len() >= tuning.hard_population_guard as usize;
        let mut cell_density = BTreeMap::<(WildlifeSpecies, i32, i32), u16>::new();
        for (_, id) in &wildlife_ids {
            let entity = self.entities[id];
            let feet = BlockPos::containing(entity.position);
            let cell = (
                match entity.kind {
                    EntityKind::Rabbit => WildlifeSpecies::Rabbit,
                    EntityKind::Deer => WildlifeSpecies::Deer,
                    EntityKind::Mallard => WildlifeSpecies::Mallard,
                    _ => continue,
                },
                feet.x.div_euclid(64),
                feet.z.div_euclid(64),
            );
            *cell_density.entry(cell).or_default() = cell_density
                .get(&cell)
                .copied()
                .unwrap_or(0)
                .saturating_add(1);
        }

        let mut updated = Vec::new();
        let mut natural_deaths = Vec::new();
        let mut deer_reproduction_blocked = BTreeMap::new();
        let mut mallard_reproduction_blocked = BTreeMap::new();
        for (_, id) in wildlife_ids.iter().copied() {
            let Some(entity) = self.entities.get(&id).copied() else {
                continue;
            };
            let feet = BlockPos::containing(entity.position);
            let density = cell_density
                .get(&(
                    match entity.kind {
                        EntityKind::Rabbit => WildlifeSpecies::Rabbit,
                        EntityKind::Deer => WildlifeSpecies::Deer,
                        EntityKind::Mallard => WildlifeSpecies::Mallard,
                        _ => continue,
                    },
                    feet.x.div_euclid(64),
                    feet.z.div_euclid(64),
                ))
                .copied()
                .unwrap_or(1);
            let (consumer, diet, foraging, cost, soft_density, species): (
                WildlifeForageConsumer,
                &[WildlifeDietEntry],
                bool,
                u16,
                u16,
                WildlifeSpecies,
            ) = match entity.kind {
                EntityKind::Rabbit => {
                    let behavior = self
                        .mobs
                        .get(&id)
                        .and_then(MobRuntimeState::rabbit_behavior)
                        .unwrap_or(RabbitBehavior::Idle);
                    (
                        WildlifeForageConsumer::Rabbit,
                        &RABBIT_DIET,
                        behavior == RabbitBehavior::Forage,
                        match behavior {
                            RabbitBehavior::Flee => 4,
                            RabbitBehavior::Hop => 2,
                            RabbitBehavior::Underground => 0,
                            _ => 1,
                        },
                        tuning.rabbit_soft_cell_density,
                        WildlifeSpecies::Rabbit,
                    )
                }
                EntityKind::Deer => {
                    let behavior = self
                        .mobs
                        .get(&id)
                        .and_then(MobRuntimeState::deer_behavior)
                        .unwrap_or(mclone_protocol::DeerBehavior::Idle);
                    (
                        WildlifeForageConsumer::Deer,
                        &DEER_DIET,
                        behavior == mclone_protocol::DeerBehavior::Graze,
                        match behavior {
                            mclone_protocol::DeerBehavior::Flee => 5,
                            mclone_protocol::DeerBehavior::Walk => 2,
                            mclone_protocol::DeerBehavior::Bedded => 0,
                            _ => 1,
                        },
                        tuning.deer_soft_cell_density,
                        WildlifeSpecies::Deer,
                    )
                }
                EntityKind::Mallard => {
                    let in_water = self
                        .mobs
                        .get(&id)
                        .is_some_and(MobRuntimeState::mallard_in_water);
                    (
                        WildlifeForageConsumer::Mallard,
                        &MALLARD_DIET,
                        true,
                        if in_water { 2 } else { 1 },
                        tuning.mallard_soft_cell_density,
                        WildlifeSpecies::Mallard,
                    )
                }
                _ => continue,
            };
            let energy = match entity.kind {
                EntityKind::Rabbit => self
                    .mobs
                    .get(&id)
                    .and_then(MobRuntimeState::rabbit_save_data)
                    .map_or(0, |rabbit| rabbit.lifecycle.energy),
                EntityKind::Deer => self
                    .mobs
                    .get(&id)
                    .and_then(MobRuntimeState::deer_save_data)
                    .map_or(0, |deer| deer.lifecycle.energy),
                EntityKind::Mallard => self
                    .mobs
                    .get(&id)
                    .and_then(MobRuntimeState::mallard_save_data)
                    .map_or(0, |mallard| mallard.lifecycle.energy),
                _ => 0,
            };
            let intake = if foraging {
                resources.consume_diet_at(
                    consumer,
                    diet,
                    feet,
                    energy,
                    cost,
                    tuning.maximum_energy,
                    simulation_tick,
                    block_state_at,
                )
            } else {
                crate::wildlife_resources::WildlifeDietIntake::default()
            };
            let Some(mob) = self.mobs.get_mut(&id) else {
                continue;
            };
            mob.apply_wildlife_energy_step(
                intake.energy,
                cost,
                tuning.cadence_ticks,
                tuning.maximum_energy,
            );
            if let Some(entity) = self.entities.get_mut(&id) {
                mob.reconcile_wildlife_maturation(entity, tuning);
            }
            let lifecycle = match entity.kind {
                EntityKind::Rabbit => mob.rabbit_save_data().expect("rabbit state").lifecycle,
                EntityKind::Deer => mob.deer_save_data().expect("deer state").lifecycle,
                EntityKind::Mallard => mob.mallard_save_data().expect("mallard state").lifecycle,
                _ => continue,
            };
            let starvation_ticks = match entity.kind {
                EntityKind::Rabbit => tuning.rabbit_starvation_ticks,
                EntityKind::Deer => tuning.deer_starvation_ticks,
                EntityKind::Mallard => tuning.mallard_starvation_ticks,
                _ => continue,
            };
            let death_cause = if lifecycle.age_ticks >= lifecycle.lifespan_ticks {
                Some(crate::ecology::WildlifeDeathCause::OldAge)
            } else if lifecycle.deficit_ticks >= starvation_ticks {
                Some(crate::ecology::WildlifeDeathCause::Starvation)
            } else {
                None
            };
            if let Some(cause) = death_cause {
                natural_deaths.push((id, entity, species, cause));
                continue;
            }
            if intake.energy > 0 {
                self.pending_wildlife_events.push(WildlifeEcologyEvent {
                    tick: simulation_tick,
                    species,
                    subject: entity.persistent_id,
                    kind: WildlifeEcologyEventKind::Intake {
                        resource: intake.resource.expect("positive intake has a resource"),
                        units: intake.units,
                        energy: intake.energy,
                    },
                });
            }
            if entity.kind == EntityKind::Rabbit {
                let reason = if overloaded {
                    Some(WildlifeReproductionSuppression::HardOverload)
                } else if density > soft_density {
                    Some(WildlifeReproductionSuppression::Crowding)
                } else if !mob
                    .rabbit_enter_natural_love(tuning.rabbit_reproductive_energy, RABBIT_LOVE_TICKS)
                {
                    let lifecycle = mob.rabbit_save_data().expect("rabbit state").lifecycle;
                    Some(if lifecycle.reproduction_cooldown > 0 {
                        WildlifeReproductionSuppression::Cooldown
                    } else {
                        WildlifeReproductionSuppression::LowCondition
                    })
                } else {
                    None
                };
                if let Some(reason) = reason {
                    self.pending_wildlife_events.push(WildlifeEcologyEvent {
                        tick: simulation_tick,
                        species,
                        subject: entity.persistent_id,
                        kind: WildlifeEcologyEventKind::ReproductionSuppressed { reason },
                    });
                }
            } else if entity.kind == EntityKind::Deer {
                let reason = if overloaded {
                    Some(WildlifeReproductionSuppression::HardOverload)
                } else if density > soft_density {
                    Some(WildlifeReproductionSuppression::Crowding)
                } else if !mob.deer_can_breed(tuning.deer_reproductive_energy) {
                    let lifecycle = mob.deer_save_data().expect("deer state").lifecycle;
                    Some(if lifecycle.reproduction_cooldown > 0 {
                        WildlifeReproductionSuppression::Cooldown
                    } else {
                        WildlifeReproductionSuppression::LowCondition
                    })
                } else {
                    None
                };
                if let Some(reason) = reason {
                    deer_reproduction_blocked.insert(id, reason);
                }
            } else {
                let reason = if overloaded {
                    Some(WildlifeReproductionSuppression::HardOverload)
                } else if density > soft_density {
                    Some(WildlifeReproductionSuppression::Crowding)
                } else if !(mob.mallard_can_nest(tuning.mallard_reproductive_energy)
                    || mob.mallard_can_fertilize(tuning.mallard_reproductive_energy))
                {
                    let lifecycle = mob.mallard_save_data().expect("mallard state").lifecycle;
                    Some(if lifecycle.reproduction_cooldown > 0 {
                        WildlifeReproductionSuppression::Cooldown
                    } else {
                        WildlifeReproductionSuppression::LowCondition
                    })
                } else {
                    None
                };
                if let Some(reason) = reason {
                    mallard_reproduction_blocked.insert(id, reason);
                }
            }
            updated.push(self.entities[&id]);
        }

        for (id, entity, species, cause) in natural_deaths {
            if let Some(removed) = self.remove_entity(id) {
                updated.push(removed);
            }
            let biomass = match species {
                WildlifeSpecies::Rabbit => 120,
                WildlifeSpecies::Deer => 900,
                WildlifeSpecies::Mallard => 180,
            };
            let source_species = match species {
                WildlifeSpecies::Rabbit => WildlifeRemainsSpecies::Rabbit,
                WildlifeSpecies::Deer => WildlifeRemainsSpecies::Deer,
                WildlifeSpecies::Mallard => WildlifeRemainsSpecies::Mallard,
            };
            let remains_cause = match cause {
                crate::ecology::WildlifeDeathCause::OldAge => WildlifeRemainsCause::OldAge,
                crate::ecology::WildlifeDeathCause::Starvation => WildlifeRemainsCause::Starvation,
            };
            let remains = self.insert_or_merge_wildlife_remains(
                source_species,
                entity.persistent_id,
                entity.position,
                entity.y_rot_degrees,
                biomass,
                remains_cause,
                simulation_tick,
            );
            updated.extend(remains);
            self.pending_wildlife_events.push(WildlifeEcologyEvent {
                tick: simulation_tick,
                species,
                subject: entity.persistent_id,
                kind: WildlifeEcologyEventKind::Death { cause },
            });
            self.pending_wildlife_events.push(WildlifeEcologyEvent {
                tick: simulation_tick,
                species,
                subject: entity.persistent_id,
                kind: WildlifeEcologyEventKind::RemainsCreated { biomass },
            });
        }
        updated.extend(self.tick_wildlife_remains_decay(simulation_tick, &ticking_chunks));
        let (deer_updates, paired_deer) = self.try_deer_birth(
            simulation_tick,
            &deer_reproduction_blocked.keys().copied().collect(),
        );
        updated.extend(deer_updates);
        let (mallard_updates, paired_mallards, mallards_without_site) = self.try_mallard_nest(
            simulation_tick,
            &mallard_reproduction_blocked.keys().copied().collect(),
            block_state_at,
        );
        updated.extend(mallard_updates);
        for (_, id) in wildlife_ids {
            let Some(entity) = self.entities.get(&id) else {
                continue;
            };
            match entity.kind {
                EntityKind::Deer if !paired_deer.contains(&id) => {
                    let reason = deer_reproduction_blocked
                        .get(&id)
                        .copied()
                        .unwrap_or(WildlifeReproductionSuppression::NoMate);
                    self.pending_wildlife_events.push(WildlifeEcologyEvent {
                        tick: simulation_tick,
                        species: WildlifeSpecies::Deer,
                        subject: entity.persistent_id,
                        kind: WildlifeEcologyEventKind::ReproductionSuppressed { reason },
                    });
                }
                EntityKind::Mallard if !paired_mallards.contains(&id) => {
                    let reason = mallard_reproduction_blocked
                        .get(&id)
                        .copied()
                        .or_else(|| {
                            mallards_without_site
                                .contains(&id)
                                .then_some(WildlifeReproductionSuppression::NoNestSite)
                        })
                        .unwrap_or(WildlifeReproductionSuppression::NoMate);
                    self.pending_wildlife_events.push(WildlifeEcologyEvent {
                        tick: simulation_tick,
                        species: WildlifeSpecies::Mallard,
                        subject: entity.persistent_id,
                        kind: WildlifeEcologyEventKind::ReproductionSuppressed { reason },
                    });
                }
                _ => {}
            }
        }
        updated
    }

    fn insert_or_merge_wildlife_remains(
        &mut self,
        source_species: WildlifeRemainsSpecies,
        source: EntityPersistentId,
        position: Vec3d,
        y_rot_degrees: f32,
        biomass: u32,
        cause: WildlifeRemainsCause,
        creation_tick: u64,
    ) -> Vec<ServerEntityState> {
        let feet = BlockPos::containing(position);
        let cell = (feet.x.div_euclid(64), feet.z.div_euclid(64));
        let mut in_cell = self
            .wildlife_remains
            .keys()
            .filter_map(|id| {
                let entity = self.entities.get(id)?;
                let block = BlockPos::containing(entity.position);
                ((block.x.div_euclid(64), block.z.div_euclid(64)) == cell)
                    .then_some((entity.persistent_id, *id))
            })
            .collect::<Vec<_>>();
        in_cell.sort_unstable();
        if in_cell.len() >= WILDLIFE_REMAINS_MAX_PER_CELL
            && let Some((_, merge_id)) = in_cell.first().copied()
            && let Some(remains) = self.wildlife_remains.get_mut(&merge_id)
        {
            remains.biomass = remains.biomass.saturating_add(biomass);
            return self.entities.get(&merge_id).copied().into_iter().collect();
        }
        let id = self.allocate_entity_id();
        let persistent_id = self.allocate_persistent_id();
        let mut entity = ServerEntityState::from_metadata(
            id,
            persistent_id,
            EntityMetadata::WILDLIFE_REMAINS,
            position,
            y_rot_degrees,
            0.0,
            None,
            true,
        );
        entity.animation = None;
        self.wildlife_remains.insert(
            id,
            WildlifeRemainsRuntimeState {
                source_species,
                source,
                biomass,
                cause,
                creation_tick,
                decay_remainder: 0,
            },
        );
        self.entities.insert(id, entity);
        self.persistent_ids.insert(id, persistent_id);
        vec![entity]
    }

    fn tick_wildlife_remains_decay(
        &mut self,
        simulation_tick: u64,
        ticking_chunks: &BTreeSet<ChunkPos>,
    ) -> Vec<ServerEntityState> {
        let mut updated = Vec::new();
        let mut exhausted = Vec::new();
        let mut ids = self.wildlife_remains.keys().copied().collect::<Vec<_>>();
        ids.sort_unstable();
        for id in ids {
            let Some(entity) = self.entities.get(&id).copied() else {
                continue;
            };
            if !ticking_chunks.contains(&entity.chunk_pos()) {
                continue;
            }
            let remains = self.wildlife_remains.get_mut(&id).expect("remains state");
            remains.decay_remainder = remains.decay_remainder.saturating_add(100);
            let decayed =
                (remains.decay_remainder / WILDLIFE_REMAINS_DECAY_DENOMINATOR).min(remains.biomass);
            remains.decay_remainder %= WILDLIFE_REMAINS_DECAY_DENOMINATOR;
            if decayed == 0 {
                continue;
            }
            remains.biomass -= decayed;
            let species = match remains.source_species {
                WildlifeRemainsSpecies::Rabbit => WildlifeSpecies::Rabbit,
                WildlifeRemainsSpecies::Deer => WildlifeSpecies::Deer,
                WildlifeRemainsSpecies::Mallard => WildlifeSpecies::Mallard,
            };
            self.pending_wildlife_events.push(WildlifeEcologyEvent {
                tick: simulation_tick,
                species,
                subject: entity.persistent_id,
                kind: WildlifeEcologyEventKind::RemainsDecayed { amount: decayed },
            });
            if remains.biomass == 0 {
                exhausted.push(id);
            } else {
                updated.push(entity);
            }
        }
        for id in exhausted {
            if let Some(removed) = self.remove_entity(id) {
                updated.push(removed);
            }
        }
        updated
    }

    fn try_deer_birth(
        &mut self,
        simulation_tick: u64,
        blocked: &BTreeSet<EntityId>,
    ) -> (Vec<ServerEntityState>, BTreeSet<EntityId>) {
        let tuning = self.wildlife_tuning;
        let mut females = Vec::new();
        let mut males = Vec::new();
        for (id, mob) in &self.mobs {
            let Some(entity) = self.entities.get(id).filter(|entity| entity.alive) else {
                continue;
            };
            if entity.kind != EntityKind::Deer
                || blocked.contains(id)
                || !mob.deer_can_breed(tuning.deer_reproductive_energy)
            {
                continue;
            }
            let entry = (entity.persistent_id, *id, entity.position);
            match mob.deer_sex() {
                Some(mclone_protocol::DeerSex::Female) => females.push(entry),
                Some(mclone_protocol::DeerSex::Male) => males.push(entry),
                None => {}
            }
        }
        females.sort_unstable_by_key(|entry| entry.0);
        males.sort_unstable_by_key(|entry| entry.0);
        let Some((female_pid, female_id, female_position, male_pid, male_id)) =
            females.iter().find_map(|female| {
                males
                    .iter()
                    .find(|male| squared_distance_xz(female.2, male.2) <= 12.0 * 12.0)
                    .map(|male| (female.0, female.1, female.2, male.0, male.1))
            })
        else {
            return (Vec::new(), BTreeSet::new());
        };
        for parent_id in [female_id, male_id] {
            self.mobs
                .get_mut(&parent_id)
                .expect("selected deer parent")
                .spend_deer_reproduction(
                    tuning.deer_birth_energy_cost,
                    tuning.deer_breeding_cooldown_ticks,
                );
        }
        let child_id = self.allocate_entity_id();
        let child_pid = self.allocate_persistent_id();
        let child = self.insert_deer_fawn_with_runtime(
            child_id,
            child_pid,
            female_position,
            self.entities[&female_id].y_rot_degrees,
        );
        self.pending_wildlife_events.push(WildlifeEcologyEvent {
            tick: simulation_tick,
            species: WildlifeSpecies::Deer,
            subject: child_pid,
            kind: WildlifeEcologyEventKind::Birth {
                child: child_pid,
                parents: [female_pid, male_pid],
            },
        });
        (
            vec![self.entities[&female_id], self.entities[&male_id], child],
            BTreeSet::from([female_id, male_id]),
        )
    }

    fn try_mallard_nest<F>(
        &mut self,
        simulation_tick: u64,
        blocked: &BTreeSet<EntityId>,
        block_state_at: &F,
    ) -> (
        Vec<ServerEntityState>,
        BTreeSet<EntityId>,
        BTreeSet<EntityId>,
    )
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let tuning = self.wildlife_tuning;
        let mut females = Vec::new();
        let mut males = Vec::new();
        for (id, mob) in &self.mobs {
            let Some(entity) = self.entities.get(id).filter(|entity| entity.alive) else {
                continue;
            };
            if entity.kind != EntityKind::Mallard || blocked.contains(id) {
                continue;
            }
            let entry = (entity.persistent_id, *id, entity.position);
            if mob.mallard_can_nest(tuning.mallard_reproductive_energy) {
                females.push(entry);
            } else if mob.mallard_can_fertilize(tuning.mallard_reproductive_energy) {
                males.push(entry);
            }
        }
        females.sort_unstable_by_key(|entry| entry.0);
        males.sort_unstable_by_key(|entry| entry.0);

        let mut without_site = BTreeSet::new();
        let mut attempting = BTreeSet::new();
        for (female_pid, female_id, female_position) in females {
            let Some((male_pid, male_id, _)) = males.iter().copied().find(|male| {
                !attempting.contains(&male.1)
                    && squared_distance_xz(female_position, male.2) <= 12.0 * 12.0
            }) else {
                continue;
            };
            let existing_nest = self.mallard_nests.iter().find_map(|(id, nest)| {
                let Some(nest_entity) = self.entities.get(id).filter(|entity| entity.alive) else {
                    return None;
                };
                (nest.parents.contains(&Some(female_pid))
                    || squared_distance_xz(nest_entity.position, female_position) < 12.0 * 12.0)
                    .then_some(BlockPos::containing(nest_entity.position))
            });
            if let Some(target) = existing_nest {
                for parent_id in [female_id, male_id] {
                    self.mobs
                        .get_mut(&parent_id)
                        .expect("existing mallard parent")
                        .set_mallard_nest_target(Some(target));
                }
                attempting.extend([female_id, male_id]);
                continue;
            }
            let current_target = BlockPos::containing(female_position);
            let persisted = self
                .mobs
                .get(&female_id)
                .and_then(MobRuntimeState::mallard_nest_target);
            let target = persisted
                .filter(|target| {
                    is_valid_mallard_nest_site(mallard_target_position(*target), block_state_at)
                })
                .or_else(|| {
                    is_valid_mallard_nest_site(female_position, block_state_at)
                        .then_some(current_target)
                })
                .or_else(|| {
                    self.mobs
                        .get(&female_id)
                        .and_then(MobRuntimeState::mallard_shore_intent)
                        .filter(|target| {
                            is_valid_mallard_nest_site(
                                mallard_target_position(*target),
                                block_state_at,
                            )
                        })
                });
            let Some(target) = target else {
                self.mobs
                    .get_mut(&female_id)
                    .expect("selected female mallard")
                    .set_mallard_nest_target(None);
                without_site.insert(female_id);
                continue;
            };
            self.mobs
                .get_mut(&female_id)
                .expect("selected female mallard")
                .set_mallard_nest_target(Some(target));
            attempting.extend([female_id, male_id]);
            if squared_distance_xz(female_position, mallard_target_position(target))
                > MALLARD_NEST_TARGET_REACHED_DISTANCE_SQR
            {
                continue;
            }
            self.mobs
                .get_mut(&male_id)
                .expect("selected male mallard")
                .set_mallard_nest_target(Some(target));
            for parent_id in [female_id, male_id] {
                self.mobs
                    .get_mut(&parent_id)
                    .expect("selected mallard parent")
                    .spend_mallard_reproduction(
                        tuning.mallard_birth_energy_cost,
                        tuning.mallard_breeding_cooldown_ticks,
                    );
            }
            let nest_id = self.allocate_entity_id();
            let nest_pid = self.allocate_persistent_id();
            let nest = self.insert_mallard_nest_with_persistent_id(
                nest_id,
                nest_pid,
                mallard_target_position(target),
                self.entities[&female_id].y_rot_degrees,
                MallardNestRuntimeState {
                    incubation_progress: 0,
                    incubation_required: MALLARD_NEST_INCUBATION_REQUIRED_TICKS,
                    parents: [Some(female_pid), Some(male_pid)],
                    attended: true,
                },
            );
            self.pending_wildlife_events.push(WildlifeEcologyEvent {
                tick: simulation_tick,
                species: WildlifeSpecies::Mallard,
                subject: female_pid,
                kind: WildlifeEcologyEventKind::NestEstablished {
                    nest: nest_pid,
                    parents: [female_pid, male_pid],
                },
            });
            return (
                vec![self.entities[&female_id], self.entities[&male_id], nest],
                attempting,
                without_site,
            );
        }
        (Vec::new(), attempting, without_site)
    }

    pub(crate) fn entity_chunk_record(&self, pos: ChunkPos, revision: u64) -> EntityChunkRecord {
        let entities = self
            .entities
            .values()
            .copied()
            .filter(|entity| entity.alive && entity.chunk_pos() == pos)
            .filter(|entity| !self.volatile_entities.contains(&entity.id))
            .filter_map(|entity| self.entity_save_record(entity))
            .collect::<Vec<_>>();
        EntityChunkRecord::new(pos, revision, entities)
    }

    pub(crate) fn hydrate_entity_chunk_record(
        &mut self,
        record: &EntityChunkRecord,
    ) -> ChunkStoreResult<Vec<ServerEntityState>> {
        if !(2..=ENTITY_CHUNK_RECORD_VERSION).contains(&record.codec_version) {
            return Err(ChunkStoreError::InvalidData(format!(
                "unsupported entity chunk record version {}",
                record.codec_version
            )));
        }

        let removed_ids = self
            .entities
            .values()
            .copied()
            .filter(|entity| entity.chunk_pos() == record.pos)
            .filter(|entity| !self.volatile_entities.contains(&entity.id))
            .filter(|entity| entity_kind_code(entity.kind).is_some())
            .map(|entity| entity.id)
            .collect::<Vec<_>>();
        for id in removed_ids {
            self.remove_entity(id);
        }

        let mut seen_persistent_ids = BTreeSet::new();
        let mut states = Vec::with_capacity(record.entities.len());
        for saved in &record.entities {
            if !seen_persistent_ids.insert(saved.persistent_id) {
                return Err(ChunkStoreError::InvalidData(format!(
                    "entity chunk ({}, {}) contained duplicate persistent id {:?}",
                    record.pos.x, record.pos.z, saved.persistent_id
                )));
            }
            let state = self.insert_saved_entity(saved)?;
            if state.chunk_pos() != record.pos {
                return Err(ChunkStoreError::InvalidData(format!(
                    "entity {:?} decoded into chunk {:?}, expected {:?}",
                    saved.persistent_id,
                    state.chunk_pos(),
                    record.pos
                )));
            }
            states.push(state);
        }
        Ok(states)
    }

    pub(crate) fn persistent_entity_chunk_positions(
        &self,
    ) -> BTreeMap<EntityPersistentId, ChunkPos> {
        self.persistent_ids
            .iter()
            .filter_map(|(id, persistent_id)| {
                self.entities
                    .get(id)
                    .filter(|entity| entity.alive)
                    .map(|entity| (*persistent_id, entity.chunk_pos()))
            })
            .collect()
    }

    pub(crate) fn has_persistent_entities_in_chunk(&self, pos: ChunkPos) -> bool {
        self.persistent_ids.keys().any(|id| {
            self.entities
                .get(id)
                .is_some_and(|entity| entity.alive && entity.chunk_pos() == pos)
        })
    }

    pub(crate) fn natural_spawn_category_counts(&self) -> MobCategoryCounts {
        let mut counts = MobCategoryCounts::new();
        for entity in self
            .entities
            .values()
            .copied()
            .filter(|entity| entity.alive)
        {
            let Some(metadata) = EntityMetadata::for_kind(entity.kind) else {
                continue;
            };
            let category = MobCategory::from_entity_category(metadata.category);
            if category.is_natural_spawning() {
                counts.increment(category);
            }
        }
        counts
    }

    pub(crate) fn spawn_volatile_passive_mob(
        &mut self,
        kind: EntityKind,
        position: Vec3d,
        y_rot_degrees: f32,
    ) -> ServerEntityState {
        let id = self.allocate_entity_id();
        let state = self.insert_passive_mob(id, kind, position, y_rot_degrees);
        self.volatile_entities.insert(id);
        state
    }

    pub(crate) fn spawn_persistent_passive_mob(
        &mut self,
        kind: EntityKind,
        position: Vec3d,
        y_rot_degrees: f32,
    ) -> ServerEntityState {
        let id = self.allocate_entity_id();
        self.insert_passive_mob(id, kind, position, y_rot_degrees)
    }

    pub(crate) fn spawn_persistent_bee_colony(
        &mut self,
        kind: EntityKind,
        position: Vec3d,
        y_rot_degrees: f32,
        bee_positions: &[Vec3d],
    ) -> Option<Vec<ServerEntityState>> {
        if !matches!(kind, EntityKind::BeeNest | EntityKind::BeeHotel)
            || self.has_bee_colony_near(position, 12.0)
        {
            return None;
        }
        let position = self.topology.canonicalize_position(position)?;
        let colony_id = self.allocate_entity_id();
        let colony_persistent_id = self.allocate_persistent_id();
        let colony = self.insert_bee_colony_with_persistent_id(
            colony_id,
            colony_persistent_id,
            kind,
            position,
            y_rot_degrees,
            BeeColonyRuntimeState {
                colonized: true,
                stored_work: 0,
                work_capacity: BEE_COLONY_WORK_CAPACITY,
                spread_cooldown: 0,
            },
        );
        let mut spawned = vec![colony];
        for bee_position in bee_positions.iter().copied().take(3) {
            let bee_position = self.topology.canonicalize_position(bee_position)?;
            let id = self.allocate_entity_id();
            let persistent_id = self.allocate_persistent_id();
            spawned.push(self.insert_bee_with_runtime(
                id,
                persistent_id,
                bee_position,
                y_rot_degrees,
                BeeRuntimeSaveData {
                    home: colony_persistent_id,
                    flower: None,
                    behavior: BeeBehavior::Hover,
                    behavior_ticks: 0,
                    carrying_pollen: false,
                },
            ));
        }
        Some(spawned)
    }

    pub(crate) fn spawn_volatile_bee_colony(
        &mut self,
        kind: EntityKind,
        position: Vec3d,
        y_rot_degrees: f32,
        bee_positions: &[Vec3d],
    ) -> Option<Vec<ServerEntityState>> {
        let spawned =
            self.spawn_persistent_bee_colony(kind, position, y_rot_degrees, bee_positions)?;
        self.volatile_entities
            .extend(spawned.iter().map(|entity| entity.id));
        Some(spawned)
    }

    pub(crate) fn place_empty_bee_hotel<F>(
        &mut self,
        position: Vec3d,
        y_rot_degrees: f32,
        block_state_at: F,
    ) -> Option<ServerEntityState>
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let position = self.topology.canonicalize_position(position)?;
        if self.has_bee_colony_near(position, 12.0)
            || !is_valid_bee_colony_site(position, &block_state_at)
        {
            return None;
        }
        let id = self.allocate_entity_id();
        let persistent_id = self.allocate_persistent_id();
        Some(self.insert_bee_colony_with_persistent_id(
            id,
            persistent_id,
            EntityKind::BeeHotel,
            position,
            y_rot_degrees,
            BeeColonyRuntimeState {
                colonized: false,
                stored_work: 0,
                work_capacity: BEE_COLONY_WORK_CAPACITY,
                spread_cooldown: 0,
            },
        ))
    }

    pub(crate) fn has_bee_colony_near(&self, position: Vec3d, radius: f64) -> bool {
        let radius_sqr = radius * radius;
        self.bee_colonies.keys().any(|id| {
            self.entities.get(id).is_some_and(|entity| {
                entity.alive
                    && self
                        .topology
                        .nearest_position_lift(entity.position, position)
                        .distance_to_sqr(position)
                        <= radius_sqr
            })
        })
    }

    pub(crate) fn empty_bee_hotels(&self) -> Vec<ServerEntityState> {
        self.bee_colonies
            .iter()
            .filter_map(|(id, colony)| {
                self.entities
                    .get(id)
                    .copied()
                    .map(|entity| (entity, colony))
            })
            .filter(|(entity, colony)| {
                entity.alive && entity.kind == EntityKind::BeeHotel && !colony.colonized
            })
            .map(|(entity, _)| entity)
            .collect()
    }

    pub(crate) fn colonize_bee_hotel(
        &mut self,
        id: EntityId,
        count: usize,
    ) -> Vec<ServerEntityState> {
        let Some(hotel) = self.entities.get(&id).copied().filter(|entity| {
            entity.alive
                && entity.kind == EntityKind::BeeHotel
                && self.bee_colonies.contains_key(&entity.id)
        }) else {
            return Vec::new();
        };
        let Some(colony) = self.bee_colonies.get_mut(&id) else {
            return Vec::new();
        };
        if colony.colonized {
            return Vec::new();
        }
        colony.colonized = true;
        (0..count.min(3))
            .map(|index| {
                let angle = index as f64 * std::f64::consts::TAU / count.max(1) as f64;
                let id = self.allocate_entity_id();
                let persistent_id = self.allocate_persistent_id();
                self.insert_bee_with_runtime(
                    id,
                    persistent_id,
                    hotel.position.add(Vec3d::new(
                        angle.sin() * 1.2,
                        0.8 + index as f64 * 0.2,
                        angle.cos() * 1.2,
                    )),
                    hotel.y_rot_degrees,
                    BeeRuntimeSaveData {
                        home: hotel.persistent_id,
                        flower: None,
                        behavior: BeeBehavior::Hover,
                        behavior_ticks: 0,
                        carrying_pollen: false,
                    },
                )
            })
            .collect()
    }

    pub(crate) fn harvest_beeswax(&mut self, id: EntityId) -> Option<Vec<ServerEntityState>> {
        let colony = self.bee_colonies.get_mut(&id)?;
        if colony.stored_work < colony.work_capacity {
            return None;
        }
        colony.stored_work = 0;
        let entity = *self.entities.get(&id)?;
        Some(vec![
            entity,
            self.insert_item_entity(
                ItemStackSnapshot {
                    kind: ItemKind::Beeswax,
                    count: 1,
                },
                entity.position.add(Vec3d::new(0.0, 0.5, 0.0)),
                entity.y_rot_degrees,
            ),
        ])
    }

    pub(crate) fn place_mallard_nest<F>(
        &mut self,
        position: Vec3d,
        y_rot_degrees: f32,
        block_state_at: F,
    ) -> Option<ServerEntityState>
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let position = self.topology.canonicalize_position(position)?;
        if !is_valid_mallard_nest_site(position, &block_state_at)
            || self.entities.values().any(|entity| {
                entity.alive
                    && entity.kind == EntityKind::MallardNest
                    && squared_distance_xz(
                        self.topology
                            .nearest_position_lift(entity.position, position),
                        position,
                    ) < 1.0
            })
        {
            return None;
        }
        let id = self.allocate_entity_id();
        let persistent_id = self.allocate_persistent_id();
        Some(self.insert_mallard_nest_with_persistent_id(
            id,
            persistent_id,
            position,
            y_rot_degrees,
            MallardNestRuntimeState {
                incubation_progress: 0,
                incubation_required: MALLARD_NEST_INCUBATION_REQUIRED_TICKS,
                parents: [None; 2],
                attended: false,
            },
        ))
    }

    pub(crate) fn drain_mallard_calls(&mut self) -> Vec<MallardCallCue> {
        std::mem::take(&mut self.pending_mallard_calls)
    }

    pub(crate) fn drain_mallard_tracks(&mut self) -> Vec<MallardTrackCue> {
        std::mem::take(&mut self.pending_mallard_tracks)
    }

    pub(crate) fn drain_deer_sounds(&mut self) -> Vec<DeerSoundCue> {
        std::mem::take(&mut self.pending_deer_sounds)
    }

    pub(crate) fn drain_bee_sounds(&mut self) -> Vec<BeeSoundCue> {
        std::mem::take(&mut self.pending_bee_sounds)
    }

    pub(crate) fn drain_bee_pollinations(&mut self) -> Vec<BeePollinationEvent> {
        std::mem::take(&mut self.pending_bee_pollinations)
    }

    pub(crate) fn drain_rabbit_sounds(&mut self) -> Vec<RabbitSoundCue> {
        std::mem::take(&mut self.pending_rabbit_sounds)
    }

    pub(crate) fn drain_rabbit_digs(&mut self) -> Vec<RabbitDigEvent> {
        std::mem::take(&mut self.pending_rabbit_digs)
    }

    pub(crate) fn drain_rabbit_raids(&mut self) -> Vec<RabbitRaidEvent> {
        std::mem::take(&mut self.pending_rabbit_raids)
    }

    pub(crate) fn complete_rabbit_dig(
        &mut self,
        rabbit_id: EntityId,
        target: BlockPos,
    ) -> Vec<ServerEntityState> {
        let Some(rabbit_entity) = self.entities.get(&rabbit_id).copied() else {
            return Vec::new();
        };
        let burrow_id = self.allocate_entity_id();
        let burrow_persistent_id = self.allocate_persistent_id();
        let burrow_position = Vec3d::new(
            f64::from(target.x) + 0.5,
            f64::from(target.y),
            f64::from(target.z) + 0.5,
        );
        let burrow = self.insert_rabbit_burrow_with_persistent_id(
            burrow_id,
            burrow_persistent_id,
            burrow_position,
            rabbit_entity.y_rot_degrees,
            RabbitBurrowRuntimeState {
                capacity: 6,
                disturbance_ticks: 0,
                damage: 0,
                last_used_tick: rabbit_entity.tick_count,
            },
        );
        let Some(mob) = self.mobs.get_mut(&rabbit_id) else {
            return vec![burrow];
        };
        mob.assign_rabbit_refuge(
            burrow_persistent_id,
            burrow_position,
            rabbit_entity.tick_count,
        );
        let rabbit = self.entities[&rabbit_id];
        self.rabbit_cue_sequence = self.rabbit_cue_sequence.wrapping_add(1);
        self.pending_rabbit_sounds.push(RabbitSoundCue {
            source: rabbit_id,
            position: rabbit.position,
            sequence: self.rabbit_cue_sequence,
            audible_radius: RABBIT_SOUND_AUDIBLE_RADIUS,
            kind: mclone_protocol::RabbitSoundKind::Dig,
        });
        vec![rabbit, burrow]
    }

    pub(crate) fn complete_rabbit_raid(
        &mut self,
        rabbit_id: EntityId,
    ) -> Option<ServerEntityState> {
        let mob = self.mobs.get_mut(&rabbit_id)?;
        mob.complete_rabbit_raid(RABBIT_RAID_COOLDOWN_TICKS);
        let entity = *self.entities.get(&rabbit_id)?;
        self.rabbit_cue_sequence = self.rabbit_cue_sequence.wrapping_add(1);
        self.pending_rabbit_sounds.push(RabbitSoundCue {
            source: rabbit_id,
            position: entity.position,
            sequence: self.rabbit_cue_sequence,
            audible_radius: RABBIT_SOUND_AUDIBLE_RADIUS,
            kind: mclone_protocol::RabbitSoundKind::Rustle,
        });
        Some(entity)
    }

    pub(crate) fn cancel_rabbit_dig(&mut self, rabbit_id: EntityId) -> Option<ServerEntityState> {
        let mob = self.mobs.get_mut(&rabbit_id)?;
        mob.cancel_rabbit_dig().then(|| self.entities[&rabbit_id])
    }

    pub(crate) fn drain_hatched_mallard_positions(&mut self) -> Vec<Vec3d> {
        std::mem::take(&mut self.hatched_mallard_positions)
    }

    pub(crate) fn ensure_persistent_passive_mob(
        &mut self,
        persistent_id: EntityPersistentId,
        kind: EntityKind,
        position: Vec3d,
        y_rot_degrees: f32,
    ) -> ChunkStoreResult<Option<ServerEntityState>> {
        if let Some(existing) = self.entities.values().find(|entity| {
            entity.alive
                && self
                    .persistent_ids
                    .get(&entity.id)
                    .is_some_and(|candidate| *candidate == persistent_id)
        }) {
            if existing.kind != kind {
                return Err(ChunkStoreError::InvalidData(format!(
                    "persistent entity {persistent_id} is {:?}, marker requested {kind:?}",
                    existing.kind
                )));
            }
            return Ok(None);
        }
        if self
            .persistent_ids
            .values()
            .any(|candidate| *candidate == persistent_id)
        {
            return Err(ChunkStoreError::InvalidData(format!(
                "persistent entity {persistent_id} has no live entity state"
            )));
        }
        let position = self
            .topology
            .canonicalize_position(position)
            .ok_or_else(|| {
                ChunkStoreError::InvalidData(format!(
                    "persistent entity {persistent_id} lies outside the dimension topology"
                ))
            })?;
        let id = self.allocate_entity_id();
        Ok(Some(self.insert_passive_mob_with_persistent_id(
            id,
            persistent_id,
            kind,
            position,
            y_rot_degrees,
        )))
    }

    pub(crate) fn discard_volatile_entities_in_chunk(
        &mut self,
        pos: ChunkPos,
    ) -> Vec<ServerEntityState> {
        let ids = self
            .volatile_entities
            .iter()
            .copied()
            .filter(|id| {
                self.entities
                    .get(id)
                    .is_some_and(|entity| entity.chunk_pos() == pos)
            })
            .collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|id| self.remove_entity(id))
            .collect()
    }

    pub(crate) fn remove_entities_in_chunk(&mut self, pos: ChunkPos) -> Vec<ServerEntityState> {
        let ids = self
            .entities
            .values()
            .copied()
            .filter(|entity| entity.chunk_pos() == pos)
            .map(|entity| entity.id)
            .collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|id| self.remove_entity(id))
            .collect()
    }

    pub(crate) fn is_persistent_entity(&self, id: EntityId) -> bool {
        self.persistent_ids.contains_key(&id)
    }

    pub(crate) fn attack_deer(&mut self, id: EntityId, damage: u8) -> Option<DeerAttackResult> {
        let entity = self.entities.get_mut(&id)?;
        if !entity.alive || entity.kind != EntityKind::Deer {
            return None;
        }
        let mob = self.mobs.get_mut(&id)?;
        if !mob.damage_deer(entity, damage) {
            return None;
        }
        self.deer_cue_sequence = self.deer_cue_sequence.wrapping_add(1);
        self.pending_deer_sounds.push(DeerSoundCue {
            source: id,
            position: entity.position,
            sequence: self.deer_cue_sequence,
            audible_radius: 20.0,
            kind: DeerSoundKind::Impact,
        });
        Some(DeerAttackResult {
            state: *entity,
            killed: entity.deer.is_some_and(|deer| deer.health == 0),
        })
    }

    pub(crate) fn targeted_deer(&self, from: Vec3d, to: Vec3d) -> Option<(ServerEntityState, f64)> {
        self.entities
            .values()
            .copied()
            .filter(|entity| {
                entity.alive
                    && entity.kind == EntityKind::Deer
                    && entity.deer.is_some_and(|deer| deer.health > 0)
            })
            .filter_map(|mut entity| {
                entity.position = self.topology.nearest_position_lift(entity.position, from);
                let center =
                    entity
                        .position
                        .add(Vec3d::new(0.0, f64::from(entity.height) * 0.5, 0.0));
                let bounds = Aabb::of_size(
                    center,
                    f64::from(entity.width) + 0.2,
                    f64::from(entity.height) + 0.2,
                    f64::from(entity.width) + 0.2,
                );
                bounds
                    .ray_intersection_fraction(from, to)
                    .map(|fraction| (entity, fraction))
            })
            .min_by(|(_, left), (_, right)| left.total_cmp(right))
    }

    pub(crate) fn damage_habitat_prop(&mut self, id: EntityId) -> Option<HabitatPropDamageResult> {
        let burrow_entity = self
            .entities
            .get(&id)
            .copied()
            .filter(|entity| entity.alive && entity.kind == EntityKind::RabbitBurrow)?;
        let burrow = self.rabbit_burrows.get_mut(&id)?;
        burrow.disturbance_ticks = burrow
            .disturbance_ticks
            .saturating_add(RABBIT_BURROW_DISTURBANCE_PER_HIT);
        burrow.damage = burrow.damage.saturating_add(1);
        let collapsed = burrow.damage >= RABBIT_BURROW_COLLAPSE_DAMAGE;
        let home = burrow_entity.persistent_id;
        let resident_ids = self
            .mobs
            .iter()
            .filter_map(|(rabbit_id, mob)| {
                mob.rabbit_save_data()
                    .filter(|rabbit| {
                        rabbit.sheltered_in == Some(home)
                            || rabbit
                                .known_refuges
                                .iter()
                                .flatten()
                                .any(|known| known.locator.persistent_id == home)
                    })
                    .map(|_| *rabbit_id)
            })
            .collect::<Vec<_>>();
        let mut updates = Vec::new();
        for rabbit_id in resident_ids {
            let release_position =
                rabbit_release_position(burrow_entity, self.entities[&rabbit_id].persistent_id);
            let (entities, mobs) = (&mut self.entities, &mut self.mobs);
            let Some(entity) = entities.get_mut(&rabbit_id) else {
                continue;
            };
            let Some(mob) = mobs.get_mut(&rabbit_id) else {
                continue;
            };
            if mob.rabbit_sheltered_in() == Some(home)
                && mob.release_rabbit_refuge(entity, home, collapsed)
            {
                entity.position = release_position;
                entity.on_ground = true;
                updates.push(*entity);
            } else if collapsed && mob.invalidate_rabbit_refuge(home) {
                updates.push(*entity);
            }
        }
        let outcome = if collapsed {
            updates.push(self.remove_entity(id)?);
            HabitatPropDamageOutcome::Collapsed
        } else {
            updates.push(burrow_entity);
            HabitatPropDamageOutcome::Disturbed
        };
        self.rabbit_cue_sequence = self.rabbit_cue_sequence.wrapping_add(1);
        self.pending_rabbit_sounds.push(RabbitSoundCue {
            source: id,
            position: burrow_entity.position,
            sequence: self.rabbit_cue_sequence,
            audible_radius: RABBIT_SOUND_AUDIBLE_RADIUS,
            kind: mclone_protocol::RabbitSoundKind::Thump,
        });
        Some(HabitatPropDamageResult { outcome, updates })
    }

    pub(crate) fn targeted_bee_colony(
        &self,
        from: Vec3d,
        to: Vec3d,
    ) -> Option<(ServerEntityState, f64)> {
        self.bee_colonies
            .keys()
            .filter_map(|id| self.entities.get(id).copied())
            .filter(|entity| entity.alive)
            .filter_map(|mut entity| {
                entity.position = self.topology.nearest_position_lift(entity.position, from);
                let center =
                    entity
                        .position
                        .add(Vec3d::new(0.0, f64::from(entity.height) * 0.5, 0.0));
                Aabb::of_size(
                    center,
                    f64::from(entity.width) + 0.3,
                    f64::from(entity.height) + 0.3,
                    f64::from(entity.width) + 0.3,
                )
                .ray_intersection_fraction(from, to)
                .map(|fraction| (entity, fraction))
            })
            .min_by(|(_, left), (_, right)| left.total_cmp(right))
    }

    pub(crate) fn feed_rabbit(&mut self, id: EntityId) -> Option<ServerEntityState> {
        if self.entities.get(&id)?.hidden_from_clients {
            return None;
        }
        let mob = self.mobs.get_mut(&id)?;
        mob.feed_rabbit(RABBIT_LOVE_TICKS)
            .then(|| self.entities[&id])
    }

    pub(crate) fn on_block_changed(&mut self, pos: BlockPos) -> usize {
        let mut affected_mobs = 0;
        for (id, mob) in &mut self.mobs {
            let Some(entity) = self.entities.get(id).copied() else {
                continue;
            };
            if mob.on_block_changed(entity, pos) {
                affected_mobs += 1;
            }
        }
        affected_mobs
    }

    pub(crate) fn on_blocks_changed(&mut self, positions: &[BlockPos]) -> usize {
        positions
            .iter()
            .copied()
            .map(|pos| self.on_block_changed(pos))
            .sum()
    }

    #[cfg(test)]
    fn separate_visible_rabbits<F>(
        &mut self,
        ticking_ids: &[EntityId],
        block_state_at: &F,
    ) -> Vec<ServerEntityState>
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        self.separate_visible_rabbits_with_diagnostics(ticking_ids, block_state_at)
            .0
    }

    fn separate_visible_rabbits_with_diagnostics<F>(
        &mut self,
        ticking_ids: &[EntityId],
        block_state_at: &F,
    ) -> (Vec<ServerEntityState>, u32)
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let rabbits = ticking_ids
            .iter()
            .filter_map(|id| self.entities.get(id).copied())
            .filter(|entity| {
                entity.alive
                    && !entity.hidden_from_clients
                    && entity.kind == EntityKind::Rabbit
                    && entity.width > 0.01
            })
            .collect::<Vec<_>>();
        let mut buckets = BTreeMap::<(i32, i32), Vec<ServerEntityState>>::new();
        for rabbit in rabbits.iter().copied() {
            buckets
                .entry((
                    (rabbit.position.x.floor() as i32).div_euclid(2),
                    (rabbit.position.z.floor() as i32).div_euclid(2),
                ))
                .or_default()
                .push(rabbit);
        }
        let mut pushes = BTreeMap::<EntityId, Vec3d>::new();
        let mut visited_pairs = BTreeSet::new();
        let mut neighbor_candidates = 0_u32;
        for left in rabbits.iter().copied() {
            let cell = (
                (left.position.x.floor() as i32).div_euclid(2),
                (left.position.z.floor() as i32).div_euclid(2),
            );
            for cell_x in cell.0 - 1..=cell.0 + 1 {
                for cell_z in cell.1 - 1..=cell.1 + 1 {
                    let Some(candidates) = buckets.get(&(cell_x, cell_z)) else {
                        continue;
                    };
                    let start = (left.tick_count as usize)
                        .wrapping_add(left.persistent_id.least as usize)
                        % candidates.len();
                    let mut admitted_in_cell = 0;
                    for offset in 0..candidates.len() {
                        if admitted_in_cell >= RABBIT_NEIGHBORS_PER_CELL {
                            break;
                        }
                        let mut right = candidates[(start + offset) % candidates.len()];
                        if right.id == left.id {
                            continue;
                        }
                        let pair = if left.id < right.id {
                            (left.id, right.id)
                        } else {
                            (right.id, left.id)
                        };
                        if !visited_pairs.insert(pair) {
                            continue;
                        }
                        admitted_in_cell += 1;
                        neighbor_candidates = neighbor_candidates.saturating_add(1);
                        right.position = self
                            .topology
                            .nearest_position_lift(right.position, left.position);
                        if left.position.y >= right.position.y + f64::from(right.height)
                            || right.position.y >= left.position.y + f64::from(left.height)
                        {
                            continue;
                        }
                        let dx = right.position.x - left.position.x;
                        let dz = right.position.z - left.position.z;
                        let distance = (dx * dx + dz * dz).sqrt();
                        let contact_distance = f64::from(left.width + right.width) * 0.5;
                        if distance >= contact_distance {
                            continue;
                        }
                        let (normal_x, normal_z) = if distance >= 0.01 {
                            (dx / distance, dz / distance)
                        } else {
                            stable_rabbit_pair_normal(left.persistent_id, right.persistent_id)
                        };
                        let amount =
                            ((contact_distance - distance) * 0.5).min(RABBIT_PAIR_PUSH_MAX);
                        let left_push = pushes.entry(left.id).or_insert(Vec3d::ZERO);
                        *left_push =
                            left_push.add(Vec3d::new(-normal_x * amount, 0.0, -normal_z * amount));
                        let right_push = pushes.entry(right.id).or_insert(Vec3d::ZERO);
                        *right_push =
                            right_push.add(Vec3d::new(normal_x * amount, 0.0, normal_z * amount));
                    }
                }
            }
        }

        let mut updates = Vec::new();
        for (id, mut requested) in pushes {
            let horizontal_length = (requested.x * requested.x + requested.z * requested.z).sqrt();
            if horizontal_length > RABBIT_PAIR_PUSH_MAX {
                requested = requested.scale(RABBIT_PAIR_PUSH_MAX / horizontal_length);
            }
            let Some(entity) = self.entities.get_mut(&id) else {
                continue;
            };
            let bounds = collision_aabb_for_feet_position(
                entity.position,
                f64::from(entity.width),
                f64::from(entity.height),
            );
            let traveled = collide_movement(block_state_at, bounds, requested);
            if traveled.length_sqr() <= f64::EPSILON {
                continue;
            }
            let moved = entity.position.add(traveled);
            if let Some(canonical) = self.topology.canonicalize_position(moved) {
                entity.position = canonical;
                updates.push(*entity);
            }
        }
        (updates, neighbor_candidates)
    }

    fn maintain_rabbit_refuges(&mut self) -> Vec<ServerEntityState> {
        let mouths = self
            .rabbit_burrows
            .keys()
            .filter_map(|id| {
                self.entities
                    .get(id)
                    .map(|entity| (entity.persistent_id, *id))
            })
            .collect::<BTreeMap<_, _>>();
        let mut updates = Vec::new();
        let sheltered = self
            .mobs
            .iter()
            .filter_map(|(id, mob)| Some((*id, mob.rabbit_save_data()?.sheltered_in?)))
            .collect::<Vec<_>>();
        for (rabbit_id, refuge) in sheltered {
            if let Some(burrow_id) = mouths.get(&refuge).copied() {
                let last_used_tick = self.entities[&rabbit_id].tick_count;
                if let Some(burrow) = self.rabbit_burrows.get_mut(&burrow_id) {
                    burrow.last_used_tick = burrow.last_used_tick.max(last_used_tick);
                }
                continue;
            }
            let rabbit = self.entities[&rabbit_id];
            let release_position = rabbit.position;
            let (entities, mobs) = (&mut self.entities, &mut self.mobs);
            let entity = entities
                .get_mut(&rabbit_id)
                .expect("sheltered rabbit disappeared during maintenance");
            let mob = mobs
                .get_mut(&rabbit_id)
                .expect("sheltered rabbit lost mob state during maintenance");
            if mob.release_rabbit_refuge(entity, refuge, true) {
                entity.position = release_position;
                entity.on_ground = true;
                updates.push(*entity);
                self.rabbit_cue_sequence = self.rabbit_cue_sequence.wrapping_add(1);
                self.pending_rabbit_sounds.push(RabbitSoundCue {
                    source: rabbit_id,
                    position: release_position,
                    sequence: self.rabbit_cue_sequence,
                    audible_radius: RABBIT_SOUND_AUDIBLE_RADIUS,
                    kind: mclone_protocol::RabbitSoundKind::Rustle,
                });
            }
        }
        updates
    }

    fn decay_rabbit_burrow_disturbance(&mut self) -> Vec<ServerEntityState> {
        let mut updates = Vec::new();
        for (id, burrow) in &mut self.rabbit_burrows {
            if burrow.disturbance_ticks == 0 {
                continue;
            }
            burrow.disturbance_ticks -= 1;
            if let Some(entity) = self.entities.get(id) {
                updates.push(*entity);
            }
        }
        updates
    }

    #[allow(dead_code)]
    pub(crate) fn diagnostics(&self) -> ServerEntityStoreDiagnostics {
        ServerEntityStoreDiagnostics {
            stored_entities: self.entities.len(),
            ticking_entities: self.tick_list.len(),
        }
    }

    pub(crate) const fn rabbit_ecology_diagnostics(&self) -> RabbitEcologyTickDiagnostics {
        self.last_rabbit_ecology
    }

    pub(crate) fn state(&self, id: EntityId) -> Option<ServerEntityState> {
        self.entities.get(&id).copied()
    }

    #[cfg(test)]
    pub(crate) fn mob_state(&self, id: EntityId) -> Option<&MobRuntimeState> {
        self.mobs.get(&id)
    }

    #[cfg(test)]
    pub(crate) fn item_state(&self, id: EntityId) -> Option<&ItemEntityRuntimeState> {
        self.items.get(&id)
    }

    #[cfg(test)]
    pub(crate) fn set_item_pickup_delay_for_test(&mut self, id: EntityId, pickup_delay: i32) {
        if let Some(item) = self.items.get_mut(&id) {
            item.set_pickup_delay_for_test(pickup_delay);
        }
    }

    #[cfg(test)]
    pub(crate) fn set_mallard_egg_time_for_test(&mut self, id: EntityId, egg_time: i32) {
        let mob = self.mobs.get_mut(&id).expect("test mallard mob state");
        mob.set_mallard_egg_time_for_test(egg_time);
    }

    #[cfg(test)]
    pub(crate) fn set_mallard_trace_times_for_test(
        &mut self,
        id: EntityId,
        feather_time: i32,
        call_time: i32,
    ) {
        let mob = self.mobs.get_mut(&id).expect("test mallard mob state");
        mob.set_mallard_trace_times_for_test(feather_time, call_time);
    }

    #[cfg(test)]
    pub(crate) fn set_mallard_sex_for_test(
        &mut self,
        id: EntityId,
        sex: mclone_protocol::MallardSex,
    ) {
        let lifecycle = self
            .mobs
            .get(&id)
            .and_then(MobRuntimeState::mallard_save_data)
            .expect("test mallard state")
            .lifecycle;
        self.mobs
            .get_mut(&id)
            .expect("test mallard mob state")
            .set_mallard_lifecycle_for_test(lifecycle, sex);
    }

    #[cfg(test)]
    pub(crate) fn tick_stationary<F>(
        &mut self,
        entity_ticking_chunks: &[ChunkPos],
        nearby_players: &[MobPlayerTarget],
        block_state_at: F,
    ) -> Vec<ServerEntityState>
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        self.tick_stationary_at_time(entity_ticking_chunks, nearby_players, 6_000, block_state_at)
    }

    pub(crate) fn tick_stationary_at_time<F>(
        &mut self,
        entity_ticking_chunks: &[ChunkPos],
        nearby_players: &[MobPlayerTarget],
        day_time: u64,
        block_state_at: F,
    ) -> Vec<ServerEntityState>
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let entity_ticking_chunks = entity_ticking_chunks
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let available_block_state_at = |pos: BlockPos| {
            entity_ticking_chunks
                .contains(&pos.chunk_pos())
                .then(|| block_state_at(pos))
                .flatten()
        };
        let debug_physics_cube_id = self.debug_physics_cube_id();
        self.tick_list.reconcile(
            self.entities
                .values()
                .copied()
                .filter(|entity| Some(entity.id) != debug_physics_cube_id),
            &entity_ticking_chunks,
        );
        let ticking_ids = self.tick_list.iteration_ids();
        let ticking_id_set = ticking_ids.iter().copied().collect::<BTreeSet<_>>();
        let rabbit_refuge_updates = self.maintain_rabbit_refuges();
        let adult_mallards = self
            .entities
            .values()
            .filter_map(|entity| {
                let mob = self.mobs.get(&entity.id)?;
                (entity.alive
                    && entity.kind == EntityKind::Mallard
                    && mob.mallard_life_stage() == Some(MallardLifeStage::Adult))
                .then_some((entity.persistent_id, entity.position, mob.mallard_sex()?))
            })
            .collect::<Vec<_>>();
        let mallard_positions = self
            .entities
            .values()
            .filter(|entity| entity.alive && entity.kind == EntityKind::Mallard)
            .map(|entity| (entity.id, entity.position))
            .collect::<Vec<_>>();
        let deer_positions = self
            .entities
            .values()
            .filter(|entity| entity.alive && entity.kind == EntityKind::Deer)
            .map(|entity| (entity.id, entity.position, entity.deer.unwrap().behavior))
            .collect::<Vec<_>>();
        let bee_home_positions = self
            .bee_colonies
            .keys()
            .filter_map(|id| {
                self.entities
                    .get(id)
                    .map(|entity| (entity.persistent_id, entity.position))
            })
            .collect::<BTreeMap<_, _>>();
        let rabbit_refuge_states = self
            .rabbit_burrows
            .iter()
            .filter(|(id, _)| ticking_id_set.contains(id))
            .filter_map(|(id, burrow)| {
                self.entities.get(id).map(|entity| {
                    (
                        entity.persistent_id,
                        entity.position,
                        burrow.capacity,
                        burrow.disturbance_ticks > 0,
                    )
                })
            })
            .collect::<Vec<_>>();
        let rabbit_refuge_by_id = rabbit_refuge_states
            .iter()
            .copied()
            .map(|state| (state.0, state))
            .collect::<BTreeMap<_, _>>();
        let mut rabbit_refuge_grid =
            BTreeMap::<(i32, i32), Vec<(EntityPersistentId, Vec3d, u8, bool)>>::new();
        for state in rabbit_refuge_states.iter().copied() {
            let block = BlockPos::containing(state.1);
            rabbit_refuge_grid
                .entry((block.x.div_euclid(16), block.z.div_euclid(16)))
                .or_default()
                .push(state);
        }
        let mut rabbit_occupancy = BTreeMap::<EntityPersistentId, u8>::new();
        for mob in self.mobs.values() {
            if let Some(refuge) = mob.rabbit_refuge_claim() {
                let occupancy = rabbit_occupancy.entry(refuge).or_default();
                *occupancy = occupancy.saturating_add(1);
            }
        }
        let mut due_rabbits = ticking_ids
            .iter()
            .filter_map(|id| {
                let entity = self.entities.get(id)?;
                let mob = self.mobs.get(id)?;
                let schedule = mob.rabbit_decision_schedule()?;
                schedule.is_due(entity.tick_count).then_some((
                    *id,
                    entity.persistent_id,
                    entity.tick_count,
                    schedule,
                ))
            })
            .collect::<Vec<_>>();
        let fairness_origin = day_time.wrapping_mul(u64::from(RABBIT_DECISION_WORK_PER_TICK));
        due_rabbits.sort_unstable_by(|left, right| {
            let left_debt = left.2.saturating_sub(left.3.next_due_tick);
            let right_debt = right.2.saturating_sub(right.3.next_due_tick);
            right_debt.cmp(&left_debt).then_with(|| {
                left.1
                    .least
                    .wrapping_sub(fairness_origin)
                    .cmp(&right.1.least.wrapping_sub(fairness_origin))
            })
        });
        let mut ecology_budget = EcologyWorkBudget::new(
            RABBIT_DECISION_WORK_PER_TICK,
            RABBIT_HABITAT_WORK_PER_TICK,
            RABBIT_PATH_WORK_PER_TICK,
        );
        let threatened_rabbits = ticking_ids
            .iter()
            .filter_map(|id| {
                let entity = self.entities.get(id)?;
                let mob = self.mobs.get(id)?;
                mob.rabbit_has_player_threat(*entity, nearby_players)
                    .then_some((*id, entity.persistent_id))
            })
            .collect::<Vec<_>>();
        let threatened_ids = threatened_rabbits
            .iter()
            .map(|(id, _)| *id)
            .collect::<BTreeSet<_>>();
        let mut urgent_paths = threatened_rabbits
            .iter()
            .filter_map(|(id, persistent_id)| {
                let entity = self.entities.get(id)?;
                let mob = self.mobs.get(id)?;
                mob.rabbit_escape_path_needed(*entity, nearby_players)
                    .then_some((*id, *persistent_id))
            })
            .collect::<Vec<_>>();
        urgent_paths.sort_unstable_by_key(|(_, persistent_id)| *persistent_id);
        if !urgent_paths.is_empty() {
            let fairness_offset = day_time
                .wrapping_mul(u64::from(RABBIT_PATH_WORK_PER_TICK))
                .rem_euclid(urgent_paths.len() as u64) as usize;
            urgent_paths.rotate_left(fairness_offset);
        }
        let mut rabbit_admissions = BTreeMap::new();
        for (id, _) in urgent_paths {
            let path_request = ecology_budget.admit(EcologyWorkClass::PathRequest, 0);
            rabbit_admissions.insert(
                id,
                RabbitEcologyAdmission {
                    path_request,
                    ..RabbitEcologyAdmission::default()
                },
            );
        }
        for (id, _, now, schedule) in &due_rabbits {
            if threatened_ids.contains(id) {
                continue;
            }
            let debt = now.saturating_sub(schedule.next_due_tick);
            let decision = ecology_budget.admit(EcologyWorkClass::Decision, debt);
            let habitat_query =
                decision && ecology_budget.admit(EcologyWorkClass::HabitatQuery, debt);
            let path_request =
                decision && ecology_budget.admit(EcologyWorkClass::PathRequest, debt);
            rabbit_admissions
                .entry(*id)
                .and_modify(|admission: &mut RabbitEcologyAdmission| {
                    admission.decision |= decision;
                    admission.habitat_query |= habitat_query;
                    admission.path_request |= path_request;
                })
                .or_insert(RabbitEcologyAdmission {
                    decision,
                    habitat_query,
                    path_request,
                });
        }
        let mut bee_colony_members = BTreeMap::<EntityPersistentId, Vec<EntityPersistentId>>::new();
        for (id, mob) in &self.mobs {
            let Some(entity) = self.entities.get(id).filter(|entity| entity.alive) else {
                continue;
            };
            if let Some(home) = mob.bee_home() {
                bee_colony_members
                    .entry(home)
                    .or_default()
                    .push(entity.persistent_id);
            }
        }
        for members in bee_colony_members.values_mut() {
            members.sort_unstable();
        }
        let mut deer_bed_sources = self
            .deer_beds
            .values()
            .map(|bed| bed.source)
            .collect::<BTreeSet<_>>();
        let mut updated = rabbit_refuge_updates;
        let mut egg_spawns = Vec::new();
        let mut feather_spawns = Vec::new();
        let mut call_candidates = Vec::new();
        let mut track_candidates = Vec::new();
        let mut merge_due_ids = Vec::new();
        let mut removed_ids = Vec::new();
        let mut hatched_nests = Vec::new();
        let mut deer_harvests = Vec::new();
        let mut deer_bed_spawns = Vec::new();
        let mut antler_spawns = Vec::new();
        let mut deer_sound_candidates = Vec::new();
        let mut bee_deposits = Vec::new();
        let mut bee_sound_candidates = Vec::new();
        let mut rabbit_breeding_candidates = Vec::new();
        for id in &ticking_ids {
            let id = *id;
            let rabbit_candidates = self.entities.get(&id).map_or_else(Vec::new, |entity| {
                if entity.kind != EntityKind::Rabbit {
                    return Vec::new();
                }
                let block = BlockPos::containing(entity.position);
                let cell = (block.x.div_euclid(16), block.z.div_euclid(16));
                let mut candidates = Vec::new();
                for cell_x in cell.0 - 2..=cell.0 + 2 {
                    for cell_z in cell.1 - 2..=cell.1 + 2 {
                        let Some(states) = rabbit_refuge_grid.get(&(cell_x, cell_z)) else {
                            continue;
                        };
                        candidates.extend(states.iter().map(
                            |(persistent_id, position, capacity, disturbed)| {
                                RabbitRefugeCandidate {
                                    persistent_id: *persistent_id,
                                    position: self
                                        .topology
                                        .nearest_position_lift(*position, entity.position),
                                    capacity: *capacity,
                                    occupancy: rabbit_occupancy
                                        .get(persistent_id)
                                        .copied()
                                        .unwrap_or(0),
                                    disturbed: *disturbed,
                                }
                            },
                        ));
                    }
                }
                candidates
            });
            let rabbit_admission = rabbit_admissions.get(&id).copied().unwrap_or_default();
            if let Some(entity) = self.entities.get_mut(&id) {
                if let Some(mob) = self.mobs.get_mut(&id) {
                    let previous_position = entity.position;
                    let previous_deer_behavior = entity.deer.map(|deer| deer.behavior);
                    let previous_rabbit_behavior = mob.rabbit_behavior();
                    let previous_rabbit_shelter = mob.rabbit_refuge_claim();
                    if entity.kind == EntityKind::Bee {
                        let home = mob.bee_home();
                        mob.set_bee_home_position(
                            home.and_then(|home| bee_home_positions.get(&home).copied()),
                        );
                        let band = home
                            .and_then(|home| bee_colony_members.get(&home))
                            .and_then(|members| {
                                members.iter().position(|id| *id == entity.persistent_id)
                            })
                            .unwrap_or(0);
                        mob.set_bee_foraging_band(band);
                    }
                    if entity.kind == EntityKind::Rabbit {
                        let refuge = mob.rabbit_save_data().and_then(|rabbit| {
                            rabbit
                                .sheltered_in
                                .or_else(|| {
                                    rabbit
                                        .known_refuges
                                        .iter()
                                        .flatten()
                                        .max_by_key(|known| {
                                            (
                                                known.familiarity,
                                                known.last_confirmed_tick,
                                                known.locator.persistent_id,
                                            )
                                        })
                                        .map(|known| known.locator.persistent_id)
                                })
                                .and_then(|persistent_id| {
                                    rabbit_refuge_by_id.get(&persistent_id).map(
                                        |(_, position, _, _)| {
                                            (
                                                persistent_id,
                                                self.topology.nearest_position_lift(
                                                    *position,
                                                    entity.position,
                                                ),
                                            )
                                        },
                                    )
                                })
                        });
                        mob.set_rabbit_refuge(refuge, entity.tick_count);
                    }
                    let flockmates = if entity.kind == EntityKind::Mallard {
                        mallard_positions
                            .iter()
                            .filter(|(candidate_id, _)| *candidate_id != id)
                            .map(|(_, position)| MallardFlockmateTarget {
                                position: self
                                    .topology
                                    .nearest_position_lift(*position, entity.position),
                            })
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    };
                    let herdmates = if entity.kind == EntityKind::Deer {
                        deer_positions
                            .iter()
                            .filter(|(candidate_id, _, _)| *candidate_id != id)
                            .map(|(_, position, behavior)| DeerHerdmateTarget {
                                position: self
                                    .topology
                                    .nearest_position_lift(*position, entity.position),
                                behavior: *behavior,
                            })
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    };
                    mob.tick_entity_at_time_with_ecology(
                        entity,
                        nearby_players,
                        &flockmates,
                        &herdmates,
                        day_time,
                        &rabbit_candidates,
                        rabbit_admission,
                        &available_block_state_at,
                    );
                    if !entity_ticking_chunks.contains(&entity.chunk_pos()) {
                        entity.position = previous_position;
                        mob.reject_unavailable_movement(*entity);
                    }
                    let current_rabbit_shelter = mob.rabbit_refuge_claim();
                    if previous_rabbit_shelter != current_rabbit_shelter {
                        if let Some(previous) = previous_rabbit_shelter
                            && let Some(occupancy) = rabbit_occupancy.get_mut(&previous)
                        {
                            *occupancy = occupancy.saturating_sub(1);
                        }
                        if let Some(current) = current_rabbit_shelter {
                            let occupancy = rabbit_occupancy.entry(current).or_default();
                            *occupancy = occupancy.saturating_add(1);
                        }
                    }
                    let chicken_egg_count = mob.take_chicken_pending_egg_lays();
                    egg_spawns.extend(
                        (0..chicken_egg_count)
                            .map(|_| (ItemKind::Egg, entity.position, entity.y_rot_degrees)),
                    );
                    let mallard_habitat = entity.kind == EntityKind::Mallard
                        && is_mallard_egg_habitat(entity.position, &available_block_state_at);
                    let mallard_egg_count = mob.take_mallard_due_egg(mallard_habitat);
                    egg_spawns
                        .extend((0..mallard_egg_count).map(|_| {
                            (ItemKind::MallardEgg, entity.position, entity.y_rot_degrees)
                        }));
                    if entity.kind == EntityKind::Mallard
                        && mob.take_mallard_due_feather(mallard_habitat)
                    {
                        feather_spawns.push((entity.position, entity.y_rot_degrees));
                    }
                    if entity.kind == EntityKind::Mallard && mob.take_mallard_due_call() {
                        call_candidates.push((entity.id, entity.position));
                    }
                    if entity.kind == EntityKind::Mallard
                        && entity.on_ground
                        && !mob.mallard_in_water()
                        && squared_distance_xz(previous_position, entity.position) > 1.0e-8
                        && mallard_habitat
                    {
                        track_candidates.push((
                            entity.id,
                            entity.persistent_id,
                            entity.position,
                            entity.y_rot_degrees,
                        ));
                    }
                    if entity.kind == EntityKind::Mallard {
                        let life_stage =
                            mob.mallard_life_stage().unwrap_or(MallardLifeStage::Adult);
                        entity.mallard = Some(MallardSnapshotData {
                            life_stage,
                            in_water: mob.mallard_in_water(),
                        });
                        let scale = if life_stage == MallardLifeStage::Duckling {
                            0.58
                        } else {
                            1.0
                        };
                        entity.width = EntityMetadata::MALLARD.dimensions.width * scale;
                        entity.height = EntityMetadata::MALLARD.dimensions.height * scale;
                    }
                    if entity.kind == EntityKind::Deer {
                        if mob.take_deer_due_antler_shed() {
                            antler_spawns.push((entity.position, entity.y_rot_degrees));
                        }
                        entity.deer = mob.deer_snapshot_data();
                        let deer_behavior = entity.deer.map(|deer| deer.behavior);
                        if previous_deer_behavior != deer_behavior {
                            let kind = match deer_behavior {
                                Some(mclone_protocol::DeerBehavior::Alert) => {
                                    Some(DeerSoundKind::Alarm)
                                }
                                Some(mclone_protocol::DeerBehavior::Graze)
                                | Some(mclone_protocol::DeerBehavior::Drink)
                                | Some(mclone_protocol::DeerBehavior::Walk) => {
                                    Some(DeerSoundKind::Contact)
                                }
                                _ => None,
                            };
                            if let Some(kind) = kind {
                                deer_sound_candidates.push((id, entity.position, kind));
                            }
                        }
                        if entity.deer.is_some_and(|deer| {
                            deer.behavior == mclone_protocol::DeerBehavior::Bedded
                        }) && !deer_bed_sources.contains(&entity.persistent_id)
                        {
                            let site = BlockPos::containing(entity.position);
                            let (recorded_site, used_ticks) = self
                                .deer_bedded_site_ticks
                                .entry(entity.persistent_id)
                                .or_insert((site, 0));
                            if *recorded_site != site {
                                *recorded_site = site;
                                *used_ticks = 0;
                            }
                            *used_ticks = used_ticks.saturating_add(1);
                            if *used_ticks >= 80 {
                                deer_bed_sources.insert(entity.persistent_id);
                                deer_bed_spawns.push((
                                    entity.persistent_id,
                                    entity.position,
                                    entity.y_rot_degrees,
                                ));
                            }
                        }
                        if mob.deer_harvest_ready() == Some(true) {
                            deer_harvests.push((
                                id,
                                entity.position,
                                entity.y_rot_degrees,
                                mob.deer_antlered() == Some(true),
                            ));
                        }
                    }
                    if entity.kind == EntityKind::Bee {
                        if let (Some(home), Some(source_flower)) =
                            (mob.bee_home(), mob.take_bee_completed_deposit())
                        {
                            bee_deposits.push((home, source_flower, entity.position));
                        }
                        if entity.tick_count % 120 == id.0 % 120 {
                            bee_sound_candidates.push((id, entity.position));
                        }
                    }
                    if entity.kind == EntityKind::Rabbit {
                        if let Some(target) = mob.take_rabbit_completed_dig() {
                            self.pending_rabbit_digs
                                .push(RabbitDigEvent { rabbit: id, target });
                        }
                        if let Some(target) = mob.take_rabbit_completed_raid() {
                            self.pending_rabbit_raids
                                .push(RabbitRaidEvent { rabbit: id, target });
                        }
                        if mob.rabbit_can_breed() {
                            rabbit_breeding_candidates.push((
                                id,
                                entity.persistent_id,
                                entity.position,
                                mob.rabbit_familiar_refuge()
                                    .map(|known| known.locator.persistent_id),
                            ));
                        }
                        if previous_rabbit_behavior != mob.rabbit_behavior()
                            && mob.rabbit_behavior() == Some(RabbitBehavior::Flee)
                        {
                            self.rabbit_cue_sequence = self.rabbit_cue_sequence.wrapping_add(1);
                            self.pending_rabbit_sounds.push(RabbitSoundCue {
                                source: id,
                                position: entity.position,
                                sequence: self.rabbit_cue_sequence,
                                audible_radius: RABBIT_SOUND_AUDIBLE_RADIUS,
                                kind: mclone_protocol::RabbitSoundKind::Thump,
                            });
                        }
                    }
                }
                if let Some(nest) = self.mallard_nests.get_mut(&id) {
                    let habitat_valid =
                        is_valid_mallard_nest_site(entity.position, &available_block_state_at);
                    let attending = |persistent_id: EntityPersistentId| {
                        adult_mallards.iter().any(|(candidate, position, _)| {
                            if *candidate != persistent_id {
                                return false;
                            }
                            let lifted = self
                                .topology
                                .nearest_position_lift(*position, entity.position);
                            squared_distance_xz(lifted, entity.position)
                                <= MALLARD_NEST_ATTENDANCE_RADIUS_SQR
                        })
                    };
                    let parents = if nest.parents.iter().all(Option::is_some) {
                        nest.parents
                    } else {
                        let female =
                            adult_mallards
                                .iter()
                                .find_map(|(persistent_id, position, sex)| {
                                    let lifted = self
                                        .topology
                                        .nearest_position_lift(*position, entity.position);
                                    (*sex == MallardSex::Female
                                        && squared_distance_xz(lifted, entity.position)
                                            <= MALLARD_NEST_ATTENDANCE_RADIUS_SQR)
                                        .then_some(*persistent_id)
                                });
                        let male =
                            adult_mallards
                                .iter()
                                .find_map(|(persistent_id, position, sex)| {
                                    let lifted = self
                                        .topology
                                        .nearest_position_lift(*position, entity.position);
                                    (*sex == MallardSex::Male
                                        && squared_distance_xz(lifted, entity.position)
                                            <= MALLARD_NEST_ATTENDANCE_RADIUS_SQR)
                                        .then_some(*persistent_id)
                                });
                        [female, male]
                    };
                    nest.attended = habitat_valid && parents.iter().all(Option::is_some);
                    if nest.attended {
                        nest.attended = parents.iter().flatten().copied().all(attending);
                    }
                    if nest.attended {
                        nest.parents = parents;
                        nest.incubation_progress = nest
                            .incubation_progress
                            .saturating_add(1)
                            .min(nest.incubation_required);
                    }
                    entity.mallard_nest = Some(MallardNestSnapshotData {
                        incubation_progress: nest.incubation_progress,
                        incubation_required: nest.incubation_required,
                        attended: nest.attended,
                    });
                    if nest.incubation_progress >= nest.incubation_required {
                        hatched_nests.push((
                            id,
                            entity.position,
                            entity.y_rot_degrees,
                            nest.parents,
                        ));
                    }
                }
                if let Some(colony) = self.bee_colonies.get_mut(&id) {
                    colony.spread_cooldown = colony.spread_cooldown.saturating_sub(1);
                }
                let mut item_block_changed = false;
                let mut is_item = false;
                if let Some(item) = self.items.get_mut(&id) {
                    is_item = true;
                    let previous_position = entity.position;
                    item.tick_entity(entity, &available_block_state_at);
                    item_block_changed = BlockPos::containing(previous_position)
                        != BlockPos::containing(entity.position);
                    if item.age() >= ITEM_ENTITY_LIFETIME_TICKS {
                        entity.alive = false;
                        removed_ids.push(id);
                    }
                }
                entity.tick_count = entity.tick_count.saturating_add(1);
                if is_item && entity.alive && item_merge_due(entity.tick_count, item_block_changed)
                {
                    merge_due_ids.push(id);
                }
                if entity.alive {
                    if let Some(position) = self.topology.canonicalize_position(entity.position) {
                        entity.position = position;
                    } else {
                        entity.alive = false;
                        removed_ids.push(id);
                    }
                }
                updated.push(*entity);
            }
        }
        let (rabbit_separation_updates, rabbit_neighbor_candidates) =
            self.separate_visible_rabbits_with_diagnostics(&ticking_ids, &available_block_state_at);
        updated.extend(rabbit_separation_updates);
        updated.extend(self.decay_rabbit_burrow_disturbance());
        for id in removed_ids {
            self.remove_entity(id);
        }
        if let Some((left, right, home)) = rabbit_breeding_pair(&rabbit_breeding_candidates) {
            let home_details = home.and_then(|home| {
                self.entities.iter().find_map(|(id, entity)| {
                    (entity.persistent_id == home && self.rabbit_burrows.contains_key(id))
                        .then_some((home, *id, entity.position))
                })
            });
            let has_capacity = home.is_none()
                || home_details.is_some_and(|(home, burrow_id, _)| {
                    let occupancy = self
                        .mobs
                        .values()
                        .filter(|mob| mob.rabbit_refuge_claim() == Some(home))
                        .count();
                    self.rabbit_burrows
                        .get(&burrow_id)
                        .is_some_and(|burrow| occupancy < usize::from(burrow.capacity))
                });
            if has_capacity {
                let tuning = self.wildlife_tuning;
                for parent_id in [left.0, right.0] {
                    if let Some(parent) = self.mobs.get_mut(&parent_id) {
                        parent.complete_rabbit_breeding(tuning.rabbit_breeding_cooldown_ticks);
                        parent.spend_rabbit_reproduction(
                            tuning.rabbit_birth_energy_cost,
                            tuning.rabbit_breeding_cooldown_ticks,
                        );
                    }
                }
                let kit_id = self.allocate_entity_id();
                let kit_persistent_id = self.allocate_persistent_id();
                let kit_position = home_details.map_or_else(
                    || {
                        Vec3d::new(
                            (left.2.x + right.2.x) * 0.5,
                            (left.2.y + right.2.y) * 0.5,
                            (left.2.z + right.2.z) * 0.5,
                        )
                    },
                    |(_, _, position)| position,
                );
                let now = self.entities[&left.0].tick_count;
                let known_refuges = home_details.map_or([None; 3], |(home, _, position)| {
                    [
                        Some(KnownPlace::observed(
                            home,
                            BlockPos::containing(position),
                            now,
                        )),
                        None,
                        None,
                    ]
                });
                let kit = self.insert_rabbit_with_runtime(
                    kit_id,
                    kit_persistent_id,
                    kit_position,
                    self.entities[&left.0].y_rot_degrees,
                    RabbitRuntimeSaveData {
                        known_refuges,
                        sheltered_in: None,
                        dig_target: None,
                        decision_schedule: DecisionSchedule::new(now.saturating_add(20), 0),
                        dig_cooldown: 1_200,
                        life_stage: RabbitLifeStage::Kit,
                        parents: [Some(left.1), Some(right.1)],
                        behavior: RabbitBehavior::Courtship,
                        behavior_ticks: 0,
                        health: 3,
                        max_health: 3,
                        love_ticks: 0,
                        raid_cooldown: 0,
                        lifecycle: WildlifeLifeState::offspring(
                            kit_persistent_id,
                            tuning.rabbit_lifespan_ticks,
                            tuning.rabbit_lifespan_variance_ticks,
                            tuning.rabbit_breeding_cooldown_ticks,
                        ),
                    },
                );
                if let Some((home, _, home_position)) = home_details
                    && let Some(mob) = self.mobs.get_mut(&kit_id)
                {
                    mob.set_rabbit_refuge(Some((home, home_position)), now);
                }
                for parent_id in [left.0, right.0] {
                    if let Some(parent) = self.entities.get_mut(&parent_id) {
                        parent.animation = Some(AnimationState::elapsed(
                            AnimationClipId::from_static("courtship"),
                            parent
                                .animation
                                .map_or(0, |animation| animation.epoch.wrapping_add(1)),
                            parent.tick_count,
                        ));
                        updated.push(*parent);
                    }
                }
                updated.push(kit);
                self.pending_wildlife_events.push(WildlifeEcologyEvent {
                    tick: now,
                    species: WildlifeSpecies::Rabbit,
                    subject: kit_persistent_id,
                    kind: WildlifeEcologyEventKind::Birth {
                        child: kit_persistent_id,
                        parents: [left.1, right.1],
                    },
                });
                self.rabbit_cue_sequence = self.rabbit_cue_sequence.wrapping_add(1);
                self.pending_rabbit_sounds.push(RabbitSoundCue {
                    source: kit_id,
                    position: kit_position,
                    sequence: self.rabbit_cue_sequence,
                    audible_radius: RABBIT_SOUND_AUDIBLE_RADIUS,
                    kind: mclone_protocol::RabbitSoundKind::Rustle,
                });
            } else {
                self.pending_wildlife_events.push(WildlifeEcologyEvent {
                    tick: self.entities[&left.0].tick_count,
                    species: WildlifeSpecies::Rabbit,
                    subject: left.1,
                    kind: WildlifeEcologyEventKind::ReproductionSuppressed {
                        reason: WildlifeReproductionSuppression::NoRefugeCapacity,
                    },
                });
            }
        }
        for (id, position, y_rot_degrees, parents) in hatched_nests {
            if let Some(removed) = self.remove_entity(id) {
                updated.push(removed);
            }
            let child = self.insert_mallard_duckling(position, y_rot_degrees, parents);
            updated.push(child);
            if let [Some(first), Some(second)] = parents {
                self.pending_wildlife_events.push(WildlifeEcologyEvent {
                    tick: day_time,
                    species: WildlifeSpecies::Mallard,
                    subject: child.persistent_id,
                    kind: WildlifeEcologyEventKind::Birth {
                        child: child.persistent_id,
                        parents: [first, second],
                    },
                });
            }
            self.hatched_mallard_positions.push(position);
        }
        for (id, position, y_rot_degrees, antlered) in deer_harvests {
            if let Some(removed) = self.remove_entity(id) {
                updated.push(removed);
            }
            for (kind, count) in [
                (ItemKind::Venison, 3),
                (ItemKind::DeerHide, 1),
                (ItemKind::ShedAntler, u8::from(antlered)),
            ] {
                if count > 0 {
                    updated.push(self.insert_item_entity(
                        ItemStackSnapshot { kind, count },
                        position,
                        y_rot_degrees,
                    ));
                }
            }
        }
        for (source, position, y_rot_degrees) in deer_bed_spawns {
            updated.push(self.insert_deer_bed(source, position, y_rot_degrees));
        }
        for (position, y_rot_degrees) in antler_spawns {
            updated.push(self.insert_item_entity(
                ItemStackSnapshot {
                    kind: ItemKind::ShedAntler,
                    count: 1,
                },
                position,
                y_rot_degrees,
            ));
        }
        updated.extend(self.merge_item_entities(&merge_due_ids));
        for (kind, position, y_rot_degrees) in egg_spawns {
            updated.push(self.insert_item_entity(
                ItemStackSnapshot { kind, count: 1 },
                position,
                y_rot_degrees,
            ));
        }
        for (position, y_rot_degrees) in feather_spawns {
            updated.push(self.insert_item_entity(
                ItemStackSnapshot {
                    kind: ItemKind::MallardFeather,
                    count: 1,
                },
                position,
                y_rot_degrees,
            ));
        }
        let mut admitted_calls: Vec<Vec3d> = Vec::new();
        for (source, position) in call_candidates {
            if admitted_calls.iter().any(|admitted| {
                squared_distance_xz(*admitted, position)
                    <= MALLARD_CALL_FLOCK_SUPPRESSION_RADIUS_SQR
            }) {
                continue;
            }
            admitted_calls.push(position);
            self.mallard_cue_sequence = self.mallard_cue_sequence.wrapping_add(1);
            self.pending_mallard_calls.push(MallardCallCue {
                source,
                position,
                sequence: self.mallard_cue_sequence,
                audible_radius: MALLARD_CALL_AUDIBLE_RADIUS,
            });
        }
        for (source, persistent_id, position, y_rot_degrees) in track_candidates {
            if self
                .mallard_last_tracks
                .get(&source)
                .is_some_and(|previous| {
                    squared_distance_xz(*previous, position) < MALLARD_TRACK_SPACING_SQR
                })
            {
                continue;
            }
            self.mallard_last_tracks.insert(source, position);
            self.mallard_cue_sequence = self.mallard_cue_sequence.wrapping_add(1);
            self.pending_mallard_tracks.push(MallardTrackCue {
                source: persistent_id,
                position,
                y_rot_degrees,
                sequence: self.mallard_cue_sequence,
            });
        }
        let mut admitted_deer_sounds = Vec::new();
        for (source, position, kind) in deer_sound_candidates {
            if admitted_deer_sounds.iter().any(|admitted: &Vec3d| {
                squared_distance_xz(*admitted, position) <= DEER_SOUND_HERD_SUPPRESSION_RADIUS_SQR
            }) {
                continue;
            }
            admitted_deer_sounds.push(position);
            self.deer_cue_sequence = self.deer_cue_sequence.wrapping_add(1);
            self.pending_deer_sounds.push(DeerSoundCue {
                source,
                position,
                sequence: self.deer_cue_sequence,
                audible_radius: if kind == DeerSoundKind::Alarm {
                    28.0
                } else {
                    18.0
                },
                kind,
            });
        }
        for (home, source_flower, position) in bee_deposits {
            let Some((colony_id, colony)) = self.bee_colonies.iter_mut().find(|(id, _)| {
                self.entities
                    .get(id)
                    .is_some_and(|entity| entity.persistent_id == home)
            }) else {
                continue;
            };
            colony.stored_work = colony
                .stored_work
                .saturating_add(1)
                .min(colony.work_capacity);
            if colony.spread_cooldown == 0 {
                colony.spread_cooldown = BEE_POLLINATION_COOLDOWN_TICKS;
                self.pending_bee_pollinations.push(BeePollinationEvent {
                    source_flower,
                    colony: home,
                    position: self.entities[colony_id].position,
                });
            }
            let _ = position;
        }
        let mut admitted_bee_sounds = Vec::new();
        for (source, position) in bee_sound_candidates {
            if admitted_bee_sounds.iter().any(|admitted: &Vec3d| {
                squared_distance_xz(*admitted, position) <= BEE_BUZZ_COLONY_SUPPRESSION_RADIUS_SQR
            }) {
                continue;
            }
            admitted_bee_sounds.push(position);
            self.bee_cue_sequence = self.bee_cue_sequence.wrapping_add(1);
            self.pending_bee_sounds.push(BeeSoundCue {
                source,
                position,
                sequence: self.bee_cue_sequence,
                audible_radius: BEE_BUZZ_AUDIBLE_RADIUS,
            });
        }
        self.last_rabbit_ecology = RabbitEcologyTickDiagnostics {
            active: ticking_ids
                .iter()
                .filter(|id| {
                    self.entities
                        .get(id)
                        .is_some_and(|entity| entity.kind == EntityKind::Rabbit)
                })
                .count() as u32,
            due: due_rabbits.len() as u32,
            work: ecology_budget.diagnostics,
            habitat_candidates: rabbit_refuge_states.len() as u32,
            neighbor_candidates: rabbit_neighbor_candidates,
        };
        updated
    }

    pub(crate) fn collect_item_entities<F>(
        &mut self,
        pickup_targets: &[ItemPickupTarget],
        mut accept_stack: F,
    ) -> Vec<ServerEntityState>
    where
        F: FnMut(ServerPlayerId, ItemStackSnapshot) -> Option<ItemStackSnapshot>,
    {
        let item_ids = self.items.keys().copied().collect::<Vec<_>>();
        let mut updated = Vec::new();
        let mut removed_ids = Vec::new();
        for id in item_ids {
            let Some(entity) = self.entities.get(&id).copied() else {
                continue;
            };
            let Some(item) = self.items.get(&id) else {
                continue;
            };
            if !entity.alive || !item.can_pick_up() {
                continue;
            }
            let item_box = entity_aabb(entity);
            for target in pickup_targets {
                if !player_pickup_box(target.position).intersects(item_box) {
                    continue;
                }
                let Some(item) = self.items.get_mut(&id) else {
                    break;
                };
                let stack = item.stack();
                let remaining = accept_stack(target.player_id, stack);
                if remaining == Some(stack) {
                    continue;
                }
                let Some(entity) = self.entities.get_mut(&id) else {
                    break;
                };
                match remaining {
                    Some(remaining) => {
                        item.replace_stack(remaining);
                        entity.item_stack = Some(remaining);
                        updated.push(*entity);
                    }
                    None => {
                        entity.alive = false;
                        updated.push(*entity);
                        removed_ids.push(id);
                    }
                }
                break;
            }
        }
        for id in removed_ids {
            self.remove_entity(id);
        }
        updated
    }

    fn merge_item_entities(&mut self, ticking_ids: &[EntityId]) -> Vec<ServerEntityState> {
        let mut updated = Vec::new();
        let mut removed_ids = BTreeSet::new();
        for id in ticking_ids {
            if removed_ids.contains(id) {
                continue;
            }
            let Some(entity) = self.entities.get(id).copied() else {
                continue;
            };
            let Some(item) = self.items.get(id) else {
                continue;
            };
            if !item.is_mergeable(entity) {
                continue;
            }
            let merge_box = item_merge_box(entity);
            let candidates = self.items.keys().copied().collect::<Vec<_>>();
            for other_id in candidates {
                if other_id == *id || removed_ids.contains(&other_id) {
                    continue;
                }
                let Some(other_entity) = self.entities.get(&other_id).copied() else {
                    continue;
                };
                if !merge_box.intersects(entity_aabb(other_entity)) {
                    continue;
                }
                if let Some((target, removed, target_update)) = self.merge_item_pair(*id, other_id)
                {
                    removed_ids.insert(removed.id);
                    updated.push(removed);
                    updated.push(target_update);
                    if target != *id {
                        break;
                    }
                }
            }
        }
        for id in removed_ids {
            self.remove_entity(id);
        }
        updated
    }

    fn merge_item_pair(
        &mut self,
        left_id: EntityId,
        right_id: EntityId,
    ) -> Option<(EntityId, ServerEntityState, ServerEntityState)> {
        let left_entity = self.entities.get(&left_id).copied()?;
        let right_entity = self.entities.get(&right_id).copied()?;
        let left_item = self.items.get(&left_id)?.clone();
        let right_item = self.items.get(&right_id)?.clone();
        if !left_item.is_mergeable(left_entity) || !right_item.is_mergeable(right_entity) {
            return None;
        }

        let (target_id, source_id, target_item, source_item) =
            if right_item.stack().count < left_item.stack().count {
                (left_id, right_id, left_item, right_item)
            } else {
                (right_id, left_id, right_item, left_item)
            };
        let (merged_stack, pickup_delay) = target_item.merged_with(&source_item)?;

        let target_entity = self.entities.get_mut(&target_id)?;
        target_entity.item_stack = Some(merged_stack);
        let target_update = *target_entity;

        let target_runtime = self.items.get_mut(&target_id)?;
        target_runtime.set_age(target_item.age().min(source_item.age()));
        target_runtime.replace_stack(merged_stack);
        target_runtime.set_pickup_delay(pickup_delay);

        let removed = self.entities.get_mut(&source_id)?;
        removed.alive = false;
        let removed = *removed;
        Some((target_id, removed, target_update))
    }

    fn allocate_entity_id(&mut self) -> EntityId {
        self.next_entity_id = self.next_entity_id.saturating_add(1);
        EntityId(self.next_entity_id)
    }

    fn allocate_persistent_id(&mut self) -> EntityPersistentId {
        loop {
            self.next_persistent_id = self.next_persistent_id.saturating_add(1);
            let persistent_id =
                EntityPersistentId::new(ENTITY_PERSISTENT_ID_MOST, self.next_persistent_id);
            if !self
                .persistent_ids
                .values()
                .any(|existing| *existing == persistent_id)
            {
                return persistent_id;
            }
        }
    }

    fn remove_entity(&mut self, id: EntityId) -> Option<ServerEntityState> {
        let removed_nest_parents = self.mallard_nests.get(&id).map(|nest| nest.parents);
        let mut state = self.entities.remove(&id)?;
        state.alive = false;
        self.mobs.remove(&id);
        self.items.remove(&id);
        self.mallard_nests.remove(&id);
        self.deer_beds.remove(&id);
        self.bee_colonies.remove(&id);
        self.rabbit_burrows.remove(&id);
        self.wildlife_remains.remove(&id);
        if state.kind == EntityKind::Deer {
            self.deer_bedded_site_ticks.remove(&state.persistent_id);
        }
        self.mallard_last_tracks.remove(&id);
        self.persistent_ids.remove(&id);
        self.volatile_entities.remove(&id);
        self.provisional_debug_passive_showcase_ids.remove(&id);
        self.tick_list.remove(id);
        if let Some(parents) = removed_nest_parents {
            for parent in parents.into_iter().flatten() {
                let parent_id = self
                    .entities
                    .iter()
                    .find_map(|(id, entity)| (entity.persistent_id == parent).then_some(*id));
                if let Some(parent_id) = parent_id
                    && let Some(parent) = self.mobs.get_mut(&parent_id)
                {
                    parent.set_mallard_nest_target(None);
                }
            }
        }
        Some(state)
    }

    fn insert_passive_mob(
        &mut self,
        id: EntityId,
        kind: EntityKind,
        position: Vec3d,
        y_rot_degrees: f32,
    ) -> ServerEntityState {
        let persistent_id = self.allocate_persistent_id();
        self.insert_passive_mob_with_persistent_id(id, persistent_id, kind, position, y_rot_degrees)
    }

    fn insert_passive_mob_with_persistent_id(
        &mut self,
        id: EntityId,
        persistent_id: EntityPersistentId,
        kind: EntityKind,
        position: Vec3d,
        y_rot_degrees: f32,
    ) -> ServerEntityState {
        let position = self
            .topology
            .canonicalize_position(position)
            .expect("spawned passive entity must lie inside the dimension topology");
        let metadata = EntityMetadata::for_kind(kind).expect("passive mob metadata must exist");
        debug_assert!(metadata.is_passive_mob());
        let state = ServerEntityState::from_metadata(
            id,
            persistent_id,
            metadata,
            position,
            y_rot_degrees,
            0.0,
            None,
            true,
        );
        self.mobs.insert(
            id,
            MobRuntimeState::from_spawn_with_persistent(
                id,
                persistent_id,
                metadata,
                state.on_ground,
                state.y_rot_degrees,
                self.wildlife_tuning,
            ),
        );
        let state = if kind == EntityKind::Mallard {
            let mut state = state;
            state.mallard = Some(MallardSnapshotData {
                life_stage: MallardLifeStage::Adult,
                in_water: false,
            });
            state
        } else if kind == EntityKind::Deer {
            let mut state = state;
            state.deer = self
                .mobs
                .get(&id)
                .and_then(MobRuntimeState::deer_snapshot_data);
            if state.deer.unwrap().life_stage == mclone_protocol::DeerLifeStage::Fawn {
                state.width *= 0.72;
                state.height *= 0.72;
            }
            state
        } else {
            state
        };
        self.entities.insert(id, state);
        self.persistent_ids.insert(id, persistent_id);
        state
    }

    fn insert_mallard_duckling(
        &mut self,
        position: Vec3d,
        y_rot_degrees: f32,
        parents: [Option<EntityPersistentId>; 2],
    ) -> ServerEntityState {
        let id = self.allocate_entity_id();
        let persistent_id = self.allocate_persistent_id();
        let saved = MallardRuntimeSaveData {
            egg_time: 8_000,
            sex: identity_mallard_sex(persistent_id),
            life_stage: MallardLifeStage::Duckling,
            parents,
            feather_time: 2_400,
            call_time: 200,
            nest_target: None,
            lifecycle: WildlifeLifeState::offspring(
                persistent_id,
                self.wildlife_tuning.mallard_lifespan_ticks,
                self.wildlife_tuning.mallard_lifespan_variance_ticks,
                self.wildlife_tuning.mallard_breeding_cooldown_ticks,
            ),
        };
        self.insert_mallard_with_runtime(id, persistent_id, position, y_rot_degrees, true, saved)
    }

    fn insert_mallard_with_runtime(
        &mut self,
        id: EntityId,
        persistent_id: EntityPersistentId,
        position: Vec3d,
        y_rot_degrees: f32,
        on_ground: bool,
        saved: MallardRuntimeSaveData,
    ) -> ServerEntityState {
        let metadata = EntityMetadata::MALLARD;
        let mut state = ServerEntityState::from_metadata(
            id,
            persistent_id,
            metadata,
            position,
            y_rot_degrees,
            0.0,
            None,
            on_ground,
        );
        state.mallard = Some(MallardSnapshotData {
            life_stage: saved.life_stage,
            in_water: false,
        });
        if state.mallard.unwrap().life_stage == MallardLifeStage::Duckling {
            state.width *= 0.58;
            state.height *= 0.58;
        }
        self.mobs.insert(
            id,
            MobRuntimeState::from_saved_with_persistent(
                id,
                persistent_id,
                metadata,
                on_ground,
                y_rot_degrees,
                Vec3d::ZERO,
                None,
                Some(saved),
                None,
                None,
                None,
            ),
        );
        self.entities.insert(id, state);
        self.persistent_ids.insert(id, persistent_id);
        state
    }

    fn insert_mallard_nest_with_persistent_id(
        &mut self,
        id: EntityId,
        persistent_id: EntityPersistentId,
        position: Vec3d,
        y_rot_degrees: f32,
        nest: MallardNestRuntimeState,
    ) -> ServerEntityState {
        let mut state = ServerEntityState::from_metadata(
            id,
            persistent_id,
            EntityMetadata::MALLARD_NEST,
            position,
            y_rot_degrees,
            0.0,
            None,
            true,
        );
        state.mallard_nest = Some(MallardNestSnapshotData {
            incubation_progress: nest.incubation_progress,
            incubation_required: nest.incubation_required,
            attended: nest.attended,
        });
        self.mallard_nests.insert(id, nest);
        self.entities.insert(id, state);
        self.persistent_ids.insert(id, persistent_id);
        state
    }

    fn insert_bee_with_runtime(
        &mut self,
        id: EntityId,
        persistent_id: EntityPersistentId,
        position: Vec3d,
        y_rot_degrees: f32,
        saved: BeeRuntimeSaveData,
    ) -> ServerEntityState {
        let metadata = EntityMetadata::BEE;
        let state = ServerEntityState::from_metadata(
            id,
            persistent_id,
            metadata,
            position,
            y_rot_degrees,
            0.0,
            None,
            false,
        );
        self.mobs.insert(
            id,
            MobRuntimeState::from_saved_with_persistent(
                id,
                persistent_id,
                metadata,
                false,
                y_rot_degrees,
                Vec3d::ZERO,
                None,
                None,
                None,
                Some(saved),
                None,
            ),
        );
        self.entities.insert(id, state);
        self.persistent_ids.insert(id, persistent_id);
        state
    }

    fn insert_bee_colony_with_persistent_id(
        &mut self,
        id: EntityId,
        persistent_id: EntityPersistentId,
        kind: EntityKind,
        position: Vec3d,
        y_rot_degrees: f32,
        colony: BeeColonyRuntimeState,
    ) -> ServerEntityState {
        let metadata = EntityMetadata::for_kind(kind).expect("bee colony metadata");
        let mut state = ServerEntityState::from_metadata(
            id,
            persistent_id,
            metadata,
            position,
            y_rot_degrees,
            0.0,
            None,
            true,
        );
        state.animation = None;
        self.bee_colonies.insert(id, colony);
        self.entities.insert(id, state);
        self.persistent_ids.insert(id, persistent_id);
        state
    }

    fn insert_rabbit_with_runtime(
        &mut self,
        id: EntityId,
        persistent_id: EntityPersistentId,
        position: Vec3d,
        y_rot_degrees: f32,
        saved: RabbitRuntimeSaveData,
    ) -> ServerEntityState {
        let metadata = EntityMetadata::RABBIT;
        let mut state = ServerEntityState::from_metadata(
            id,
            persistent_id,
            metadata,
            position,
            y_rot_degrees,
            0.0,
            None,
            true,
        );
        if saved.life_stage == RabbitLifeStage::Kit {
            state.width *= 0.62;
            state.height *= 0.62;
        }
        self.mobs.insert(
            id,
            MobRuntimeState::from_saved_with_persistent(
                id,
                persistent_id,
                metadata,
                true,
                y_rot_degrees,
                Vec3d::ZERO,
                None,
                None,
                None,
                None,
                Some(saved),
            ),
        );
        self.entities.insert(id, state);
        self.persistent_ids.insert(id, persistent_id);
        state
    }

    fn insert_deer_fawn_with_runtime(
        &mut self,
        id: EntityId,
        persistent_id: EntityPersistentId,
        position: Vec3d,
        y_rot_degrees: f32,
    ) -> ServerEntityState {
        let tuning = self.wildlife_tuning;
        let metadata = EntityMetadata::DEER;
        let mut state = ServerEntityState::from_metadata(
            id,
            persistent_id,
            metadata,
            position,
            y_rot_degrees,
            0.0,
            None,
            true,
        );
        let sex = if persistent_id.least.is_multiple_of(2) {
            mclone_protocol::DeerSex::Female
        } else {
            mclone_protocol::DeerSex::Male
        };
        let saved = DeerRuntimeSaveData {
            sex,
            life_stage: mclone_protocol::DeerLifeStage::Fawn,
            antlered: false,
            behavior: mclone_protocol::DeerBehavior::Idle,
            behavior_ticks: 0,
            health: 12,
            max_health: 12,
            antler_shed_time: -1,
            lifecycle: WildlifeLifeState::offspring(
                persistent_id,
                tuning.deer_lifespan_ticks,
                tuning.deer_lifespan_variance_ticks,
                tuning.deer_breeding_cooldown_ticks,
            ),
        };
        state.deer = Some(mclone_protocol::DeerSnapshotData {
            sex,
            life_stage: mclone_protocol::DeerLifeStage::Fawn,
            antlered: false,
            behavior: mclone_protocol::DeerBehavior::Idle,
            health: 12,
            max_health: 12,
        });
        state.width *= 0.72;
        state.height *= 0.72;
        self.mobs.insert(
            id,
            MobRuntimeState::from_saved_with_persistent(
                id,
                persistent_id,
                metadata,
                true,
                y_rot_degrees,
                Vec3d::ZERO,
                None,
                None,
                Some(saved),
                None,
                None,
            ),
        );
        self.entities.insert(id, state);
        self.persistent_ids.insert(id, persistent_id);
        state
    }

    fn insert_rabbit_burrow_with_persistent_id(
        &mut self,
        id: EntityId,
        persistent_id: EntityPersistentId,
        position: Vec3d,
        y_rot_degrees: f32,
        burrow: RabbitBurrowRuntimeState,
    ) -> ServerEntityState {
        let mut state = ServerEntityState::from_metadata(
            id,
            persistent_id,
            EntityMetadata::RABBIT_BURROW,
            position,
            y_rot_degrees,
            0.0,
            None,
            true,
        );
        state.animation = None;
        self.rabbit_burrows.insert(id, burrow);
        self.entities.insert(id, state);
        self.persistent_ids.insert(id, persistent_id);
        state
    }

    fn insert_wildlife_remains_with_persistent_id(
        &mut self,
        id: EntityId,
        persistent_id: EntityPersistentId,
        position: Vec3d,
        y_rot_degrees: f32,
        remains: WildlifeRemainsRuntimeState,
    ) -> ServerEntityState {
        let mut state = ServerEntityState::from_metadata(
            id,
            persistent_id,
            EntityMetadata::WILDLIFE_REMAINS,
            position,
            y_rot_degrees,
            0.0,
            None,
            true,
        );
        state.animation = None;
        self.wildlife_remains.insert(id, remains);
        self.entities.insert(id, state);
        self.persistent_ids.insert(id, persistent_id);
        state
    }

    #[cfg(test)]
    pub(crate) fn remove_persistent_entity_for_test(
        &mut self,
        persistent_id: EntityPersistentId,
    ) -> Option<ServerEntityState> {
        let id = self
            .persistent_ids
            .iter()
            .find_map(|(id, candidate)| (*candidate == persistent_id).then_some(*id))?;
        self.remove_entity(id)
    }

    pub(crate) fn insert_item_entity(
        &mut self,
        stack: ItemStackSnapshot,
        position: Vec3d,
        y_rot_degrees: f32,
    ) -> ServerEntityState {
        let persistent_id = self.allocate_persistent_id();
        let position = self
            .topology
            .canonicalize_position(position)
            .expect("spawned item entity must lie inside the dimension topology");
        let id = self.allocate_entity_id();
        let metadata = EntityMetadata::for_kind(EntityKind::Item).expect("item metadata");
        let mut state = ServerEntityState::from_metadata(
            id,
            persistent_id,
            metadata,
            position,
            y_rot_degrees,
            0.0,
            None,
            false,
        );
        state.item_stack = Some(stack);
        let item = ItemEntityRuntimeState::from_spawn(id, stack);
        debug_assert_eq!(item.stack(), stack);
        self.items.insert(id, item);
        self.entities.insert(id, state);
        self.persistent_ids.insert(id, persistent_id);
        state
    }

    fn insert_deer_bed_with_persistent_id(
        &mut self,
        id: EntityId,
        persistent_id: EntityPersistentId,
        position: Vec3d,
        y_rot_degrees: f32,
        source: EntityPersistentId,
    ) -> ServerEntityState {
        let mut state = ServerEntityState::from_metadata(
            id,
            persistent_id,
            EntityMetadata::DEER_BED,
            position,
            y_rot_degrees,
            0.0,
            None,
            true,
        );
        state.animation = None;
        self.deer_beds.insert(id, DeerBedRuntimeState { source });
        self.entities.insert(id, state);
        self.persistent_ids.insert(id, persistent_id);
        state
    }

    fn insert_deer_bed(
        &mut self,
        source: EntityPersistentId,
        position: Vec3d,
        y_rot_degrees: f32,
    ) -> ServerEntityState {
        let id = self.allocate_entity_id();
        let persistent_id = self.allocate_persistent_id();
        self.insert_deer_bed_with_persistent_id(id, persistent_id, position, y_rot_degrees, source)
    }

    fn insert_saved_entity(
        &mut self,
        saved: &EntitySaveRecord,
    ) -> ChunkStoreResult<ServerEntityState> {
        if !saved.position.is_finite() || !saved.delta_movement.is_finite() {
            return Err(ChunkStoreError::InvalidData(format!(
                "entity {:?} had non-finite position or movement",
                saved.persistent_id
            )));
        }
        let canonical_position = self
            .topology
            .canonicalize_position(saved.position)
            .ok_or_else(|| {
                ChunkStoreError::InvalidData(format!(
                    "entity {:?} lies outside the dimension topology",
                    saved.persistent_id
                ))
            })?;
        if self
            .persistent_ids
            .values()
            .any(|existing| *existing == saved.persistent_id)
        {
            return Err(ChunkStoreError::InvalidData(format!(
                "entity {:?} duplicated an existing persistent id",
                saved.persistent_id
            )));
        }

        let id = self.allocate_entity_id();
        let mut state = match (saved.kind.as_str(), &saved.payload) {
            ("minecraft:cow", EntitySavePayload::Cow) => self.insert_saved_passive_mob(
                id,
                saved,
                canonical_position,
                EntityKind::Cow,
                None,
                None,
                None,
                None,
                None,
            )?,
            ("minecraft:chicken", EntitySavePayload::Chicken { egg_time }) => self
                .insert_saved_passive_mob(
                    id,
                    saved,
                    canonical_position,
                    EntityKind::Chicken,
                    Some(*egg_time),
                    None,
                    None,
                    None,
                    None,
                )?,
            (
                "mclone:mallard",
                EntitySavePayload::Mallard {
                    egg_time,
                    sex,
                    life_stage,
                    parents,
                    feather_time,
                    call_time,
                    nest_target,
                    age_ticks,
                    lifespan_ticks,
                    energy,
                    deficit_ticks,
                    recent_intake,
                    reproductive_condition,
                    reproduction_cooldown,
                },
            ) => self.insert_saved_passive_mob(
                id,
                saved,
                canonical_position,
                EntityKind::Mallard,
                None,
                Some(MallardRuntimeSaveData {
                    egg_time: *egg_time,
                    sex: if *lifespan_ticks == 0 {
                        identity_mallard_sex(saved.persistent_id)
                    } else {
                        *sex
                    },
                    life_stage: *life_stage,
                    parents: *parents,
                    feather_time: *feather_time,
                    call_time: *call_time,
                    nest_target: *nest_target,
                    lifecycle: WildlifeLifeState {
                        age_ticks: *age_ticks,
                        lifespan_ticks: *lifespan_ticks,
                        energy: *energy,
                        deficit_ticks: *deficit_ticks,
                        recent_intake: *recent_intake,
                        reproductive_condition: *reproductive_condition,
                        reproduction_cooldown: *reproduction_cooldown,
                    },
                }),
                None,
                None,
                None,
            )?,
            (
                "mclone:deer",
                EntitySavePayload::Deer {
                    sex,
                    life_stage,
                    antlered,
                    behavior,
                    behavior_ticks,
                    health,
                    max_health,
                    antler_shed_time,
                    age_ticks,
                    lifespan_ticks,
                    energy,
                    deficit_ticks,
                    recent_intake,
                    reproductive_condition,
                    reproduction_cooldown,
                },
            ) => self.insert_saved_passive_mob(
                id,
                saved,
                canonical_position,
                EntityKind::Deer,
                None,
                None,
                Some(DeerRuntimeSaveData {
                    sex: *sex,
                    life_stage: *life_stage,
                    antlered: *antlered,
                    behavior: *behavior,
                    behavior_ticks: *behavior_ticks,
                    health: *health,
                    max_health: *max_health,
                    antler_shed_time: *antler_shed_time,
                    lifecycle: WildlifeLifeState {
                        age_ticks: *age_ticks,
                        lifespan_ticks: *lifespan_ticks,
                        energy: *energy,
                        deficit_ticks: *deficit_ticks,
                        recent_intake: *recent_intake,
                        reproductive_condition: *reproductive_condition,
                        reproduction_cooldown: *reproduction_cooldown,
                    },
                }),
                None,
                None,
            )?,
            (
                "mclone:bee",
                EntitySavePayload::Bee {
                    home,
                    flower,
                    behavior,
                    behavior_ticks,
                    carrying_pollen,
                },
            ) => self.insert_saved_passive_mob(
                id,
                saved,
                canonical_position,
                EntityKind::Bee,
                None,
                None,
                None,
                Some(BeeRuntimeSaveData {
                    home: *home,
                    flower: *flower,
                    behavior: *behavior,
                    behavior_ticks: *behavior_ticks,
                    carrying_pollen: *carrying_pollen,
                }),
                None,
            )?,
            (
                "mclone:rabbit",
                EntitySavePayload::Rabbit {
                    known_refuges,
                    sheltered_in,
                    dig_target,
                    next_decision_tick,
                    decision_generation,
                    dig_cooldown,
                    life_stage,
                    age_ticks,
                    parents,
                    behavior,
                    behavior_ticks,
                    health,
                    max_health,
                    love_ticks,
                    breed_cooldown,
                    raid_cooldown,
                    lifespan_ticks,
                    energy,
                    deficit_ticks,
                    recent_intake,
                    reproductive_condition,
                },
            ) => self.insert_saved_passive_mob(
                id,
                saved,
                canonical_position,
                EntityKind::Rabbit,
                None,
                None,
                None,
                None,
                Some(RabbitRuntimeSaveData {
                    known_refuges: known_refuges.map(|refuge| refuge.map(known_place_from_save)),
                    sheltered_in: *sheltered_in,
                    dig_target: *dig_target,
                    decision_schedule: DecisionSchedule::new(
                        *next_decision_tick,
                        *decision_generation,
                    ),
                    dig_cooldown: *dig_cooldown,
                    life_stage: *life_stage,
                    parents: *parents,
                    behavior: *behavior,
                    behavior_ticks: *behavior_ticks,
                    health: *health,
                    max_health: *max_health,
                    love_ticks: *love_ticks,
                    raid_cooldown: *raid_cooldown,
                    lifecycle: WildlifeLifeState {
                        age_ticks: *age_ticks,
                        lifespan_ticks: *lifespan_ticks,
                        energy: *energy,
                        deficit_ticks: *deficit_ticks,
                        recent_intake: *recent_intake,
                        reproductive_condition: *reproductive_condition,
                        reproduction_cooldown: *breed_cooldown,
                    },
                }),
            )?,
            (
                "mclone:bee_nest" | "mclone:bee_hotel",
                EntitySavePayload::BeeColony {
                    colonized,
                    stored_work,
                    work_capacity,
                    spread_cooldown,
                },
            ) => self.insert_bee_colony_with_persistent_id(
                id,
                saved.persistent_id,
                if saved.kind == "mclone:bee_nest" {
                    EntityKind::BeeNest
                } else {
                    EntityKind::BeeHotel
                },
                canonical_position,
                saved.y_rot_degrees,
                BeeColonyRuntimeState {
                    colonized: *colonized,
                    stored_work: *stored_work,
                    work_capacity: *work_capacity,
                    spread_cooldown: *spread_cooldown,
                },
            ),
            (
                "mclone:rabbit_burrow",
                EntitySavePayload::RabbitBurrow {
                    capacity,
                    disturbance_ticks,
                    damage,
                    last_used_tick,
                },
            ) => self.insert_rabbit_burrow_with_persistent_id(
                id,
                saved.persistent_id,
                canonical_position,
                saved.y_rot_degrees,
                RabbitBurrowRuntimeState {
                    capacity: *capacity,
                    disturbance_ticks: *disturbance_ticks,
                    damage: *damage,
                    last_used_tick: *last_used_tick,
                },
            ),
            (
                "mclone:wildlife_remains",
                EntitySavePayload::WildlifeRemains {
                    source_species,
                    source,
                    biomass,
                    cause,
                    creation_tick,
                    decay_remainder,
                },
            ) => self.insert_wildlife_remains_with_persistent_id(
                id,
                saved.persistent_id,
                canonical_position,
                saved.y_rot_degrees,
                WildlifeRemainsRuntimeState {
                    source_species: *source_species,
                    source: *source,
                    biomass: *biomass,
                    cause: *cause,
                    creation_tick: *creation_tick,
                    decay_remainder: *decay_remainder,
                },
            ),
            (
                "mclone:mallard_nest",
                EntitySavePayload::MallardNest {
                    incubation_progress,
                    incubation_required,
                    parents,
                },
            ) => self.insert_mallard_nest_with_persistent_id(
                id,
                saved.persistent_id,
                canonical_position,
                saved.y_rot_degrees,
                MallardNestRuntimeState {
                    incubation_progress: *incubation_progress,
                    incubation_required: *incubation_required,
                    parents: *parents,
                    attended: false,
                },
            ),
            ("mclone:deer_bed", EntitySavePayload::DeerBed { source }) => self
                .insert_deer_bed_with_persistent_id(
                    id,
                    saved.persistent_id,
                    canonical_position,
                    saved.y_rot_degrees,
                    *source,
                ),
            ("mclone:mannequin", EntitySavePayload::Mannequin) => self.insert_saved_passive_mob(
                id,
                saved,
                canonical_position,
                EntityKind::Mannequin,
                None,
                None,
                None,
                None,
                None,
            )?,
            (
                "minecraft:item",
                EntitySavePayload::Item {
                    stack,
                    age,
                    pickup_delay,
                },
            ) => self.insert_saved_item_entity(
                id,
                saved,
                canonical_position,
                stack,
                *age,
                *pickup_delay,
            )?,
            (kind, payload) => {
                return Err(ChunkStoreError::InvalidData(format!(
                    "entity {:?} had incompatible kind {kind:?} and payload {payload:?}",
                    saved.persistent_id
                )));
            }
        };
        state.animation = saved.animation.or(state.animation);
        self.entities.insert(id, state);
        self.persistent_ids.insert(id, saved.persistent_id);
        if saved.persistent_id.most == ENTITY_PERSISTENT_ID_MOST {
            self.next_persistent_id = self.next_persistent_id.max(saved.persistent_id.least);
        }
        Ok(state)
    }

    fn insert_saved_passive_mob(
        &mut self,
        id: EntityId,
        saved: &EntitySaveRecord,
        position: Vec3d,
        kind: EntityKind,
        chicken_egg_time: Option<i32>,
        mallard: Option<MallardRuntimeSaveData>,
        deer: Option<DeerRuntimeSaveData>,
        bee: Option<BeeRuntimeSaveData>,
        rabbit: Option<RabbitRuntimeSaveData>,
    ) -> ChunkStoreResult<ServerEntityState> {
        let metadata = EntityMetadata::for_kind(kind).ok_or_else(|| {
            ChunkStoreError::InvalidData(format!("entity kind {kind:?} has no metadata"))
        })?;
        let state = ServerEntityState::from_metadata(
            id,
            saved.persistent_id,
            metadata,
            position,
            saved.y_rot_degrees,
            saved.x_rot_degrees,
            saved.rotation,
            saved.on_ground,
        );
        let mob = MobRuntimeState::from_saved_with_persistent(
            id,
            saved.persistent_id,
            metadata,
            state.on_ground,
            state.y_rot_degrees,
            saved.delta_movement,
            chicken_egg_time,
            mallard,
            deer,
            bee,
            rabbit,
        );
        let mut state = state;
        if kind == EntityKind::Mallard {
            let life_stage = mob.mallard_life_stage().unwrap_or(MallardLifeStage::Adult);
            state.mallard = Some(MallardSnapshotData {
                life_stage,
                in_water: false,
            });
            if life_stage == MallardLifeStage::Duckling {
                state.width *= 0.58;
                state.height *= 0.58;
            }
        }
        if kind == EntityKind::Deer {
            state.deer = mob.deer_snapshot_data();
            if state.deer.unwrap().life_stage == mclone_protocol::DeerLifeStage::Fawn {
                state.width *= 0.72;
                state.height *= 0.72;
            }
        }
        if kind == EntityKind::Rabbit {
            if mob.rabbit_behavior() == Some(RabbitBehavior::Underground) {
                state.width = 0.001;
                state.height = 0.001;
                state.hidden_from_clients = true;
            } else if mob.rabbit_life_stage() == Some(RabbitLifeStage::Kit) {
                state.width *= 0.62;
                state.height *= 0.62;
            }
        }
        self.mobs.insert(id, mob);
        self.entities.insert(id, state);
        Ok(state)
    }

    fn insert_saved_item_entity(
        &mut self,
        id: EntityId,
        saved: &EntitySaveRecord,
        position: Vec3d,
        stack: &ItemStackSaveRecord,
        age: u64,
        pickup_delay: i32,
    ) -> ChunkStoreResult<ServerEntityState> {
        let stack = item_stack_snapshot_from_save(stack)?;
        let metadata = EntityMetadata::for_kind(EntityKind::Item).expect("item metadata");
        let mut state = ServerEntityState::from_metadata(
            id,
            saved.persistent_id,
            metadata,
            position,
            saved.y_rot_degrees,
            saved.x_rot_degrees,
            saved.rotation,
            saved.on_ground,
        );
        state.item_stack = Some(stack);
        let item =
            ItemEntityRuntimeState::from_saved(stack, saved.delta_movement, age, pickup_delay);
        self.items.insert(id, item);
        self.entities.insert(id, state);
        Ok(state)
    }

    fn entity_save_record(&self, entity: ServerEntityState) -> Option<EntitySaveRecord> {
        let kind = entity_kind_code(entity.kind)?;
        let persistent_id = entity.persistent_id;
        let delta_movement = self
            .items
            .get(&entity.id)
            .map(ItemEntityRuntimeState::delta_movement)
            .or_else(|| {
                self.mobs
                    .get(&entity.id)
                    .map(MobRuntimeState::delta_movement)
            })
            .unwrap_or(Vec3d::ZERO);
        let payload = match entity.kind {
            EntityKind::Cow => EntitySavePayload::Cow,
            EntityKind::Chicken => EntitySavePayload::Chicken {
                egg_time: self
                    .mobs
                    .get(&entity.id)
                    .and_then(MobRuntimeState::chicken_egg_time)
                    .unwrap_or(0),
            },
            EntityKind::Mallard => {
                let mallard = self.mobs.get(&entity.id)?.mallard_save_data()?;
                EntitySavePayload::Mallard {
                    egg_time: mallard.egg_time,
                    sex: mallard.sex,
                    life_stage: mallard.life_stage,
                    parents: mallard.parents,
                    feather_time: mallard.feather_time,
                    call_time: mallard.call_time,
                    nest_target: mallard.nest_target,
                    age_ticks: mallard.lifecycle.age_ticks,
                    lifespan_ticks: mallard.lifecycle.lifespan_ticks,
                    energy: mallard.lifecycle.energy,
                    deficit_ticks: mallard.lifecycle.deficit_ticks,
                    recent_intake: mallard.lifecycle.recent_intake,
                    reproductive_condition: mallard.lifecycle.reproductive_condition,
                    reproduction_cooldown: mallard.lifecycle.reproduction_cooldown,
                }
            }
            EntityKind::MallardNest => {
                let nest = self.mallard_nests.get(&entity.id)?;
                EntitySavePayload::MallardNest {
                    incubation_progress: nest.incubation_progress,
                    incubation_required: nest.incubation_required,
                    parents: nest.parents,
                }
            }
            EntityKind::Deer => {
                let deer = self.mobs.get(&entity.id)?.deer_save_data()?;
                EntitySavePayload::Deer {
                    sex: deer.sex,
                    life_stage: deer.life_stage,
                    antlered: deer.antlered,
                    behavior: deer.behavior,
                    behavior_ticks: deer.behavior_ticks,
                    health: deer.health,
                    max_health: deer.max_health,
                    antler_shed_time: deer.antler_shed_time,
                    age_ticks: deer.lifecycle.age_ticks,
                    lifespan_ticks: deer.lifecycle.lifespan_ticks,
                    energy: deer.lifecycle.energy,
                    deficit_ticks: deer.lifecycle.deficit_ticks,
                    recent_intake: deer.lifecycle.recent_intake,
                    reproductive_condition: deer.lifecycle.reproductive_condition,
                    reproduction_cooldown: deer.lifecycle.reproduction_cooldown,
                }
            }
            EntityKind::DeerBed => EntitySavePayload::DeerBed {
                source: self.deer_beds.get(&entity.id)?.source,
            },
            EntityKind::Bee => {
                let bee = self.mobs.get(&entity.id)?.bee_save_data()?;
                EntitySavePayload::Bee {
                    home: bee.home,
                    flower: bee.flower,
                    behavior: bee.behavior,
                    behavior_ticks: bee.behavior_ticks,
                    carrying_pollen: bee.carrying_pollen,
                }
            }
            EntityKind::BeeNest | EntityKind::BeeHotel => {
                let colony = self.bee_colonies.get(&entity.id)?;
                EntitySavePayload::BeeColony {
                    colonized: colony.colonized,
                    stored_work: colony.stored_work,
                    work_capacity: colony.work_capacity,
                    spread_cooldown: colony.spread_cooldown,
                }
            }
            EntityKind::Rabbit => {
                let rabbit = self.mobs.get(&entity.id)?.rabbit_save_data()?;
                EntitySavePayload::Rabbit {
                    known_refuges: rabbit
                        .known_refuges
                        .map(|refuge| refuge.map(rabbit_refuge_to_save)),
                    sheltered_in: rabbit.sheltered_in,
                    dig_target: rabbit.dig_target,
                    next_decision_tick: rabbit.decision_schedule.next_due_tick,
                    decision_generation: rabbit.decision_schedule.attempt_generation,
                    dig_cooldown: rabbit.dig_cooldown,
                    life_stage: rabbit.life_stage,
                    age_ticks: rabbit.lifecycle.age_ticks,
                    parents: rabbit.parents,
                    behavior: rabbit.behavior,
                    behavior_ticks: rabbit.behavior_ticks,
                    health: rabbit.health,
                    max_health: rabbit.max_health,
                    love_ticks: rabbit.love_ticks,
                    breed_cooldown: rabbit.lifecycle.reproduction_cooldown,
                    raid_cooldown: rabbit.raid_cooldown,
                    lifespan_ticks: rabbit.lifecycle.lifespan_ticks,
                    energy: rabbit.lifecycle.energy,
                    deficit_ticks: rabbit.lifecycle.deficit_ticks,
                    recent_intake: rabbit.lifecycle.recent_intake,
                    reproductive_condition: rabbit.lifecycle.reproductive_condition,
                }
            }
            EntityKind::RabbitBurrow => {
                let burrow = self.rabbit_burrows.get(&entity.id)?;
                EntitySavePayload::RabbitBurrow {
                    capacity: burrow.capacity,
                    disturbance_ticks: burrow.disturbance_ticks,
                    damage: burrow.damage,
                    last_used_tick: burrow.last_used_tick,
                }
            }
            EntityKind::WildlifeRemains => {
                let remains = self.wildlife_remains.get(&entity.id)?;
                EntitySavePayload::WildlifeRemains {
                    source_species: remains.source_species,
                    source: remains.source,
                    biomass: remains.biomass,
                    cause: remains.cause,
                    creation_tick: remains.creation_tick,
                    decay_remainder: remains.decay_remainder,
                }
            }
            EntityKind::Mannequin => EntitySavePayload::Mannequin,
            EntityKind::Item => EntitySavePayload::Item {
                stack: entity.item_stack.map(ItemStackSaveRecord::from)?,
                age: self.items.get(&entity.id)?.age(),
                pickup_delay: self.items.get(&entity.id)?.pickup_delay(),
            },
            EntityKind::DebugCube => return None,
        };
        Some(EntitySaveRecord {
            persistent_id,
            kind: kind.to_owned(),
            position: entity.position,
            delta_movement,
            y_rot_degrees: entity.y_rot_degrees,
            x_rot_degrees: entity.x_rot_degrees,
            rotation: entity.rotation,
            on_ground: entity.on_ground,
            animation: entity.animation,
            payload,
        })
    }

    #[cfg(test)]
    pub(crate) fn insert_item_entity_for_test(
        &mut self,
        stack: ItemStackSnapshot,
        position: Vec3d,
    ) -> EntityId {
        self.insert_item_entity(stack, position, 0.0).id
    }

    fn debug_physics_cube_id(&self) -> Option<EntityId> {
        #[cfg(feature = "physics-engine")]
        {
            self.debug_physics_cube_id
        }
        #[cfg(not(feature = "physics-engine"))]
        {
            None
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ServerEntityStoreDiagnostics {
    pub(crate) stored_entities: usize,
    pub(crate) ticking_entities: usize,
}

#[cfg(feature = "physics-engine")]
fn debug_physics_cube_state(
    id: EntityId,
    persistent_id: EntityPersistentId,
    position: Vec3d,
    y_rot_degrees: f32,
    x_rot_degrees: f32,
    rotation: EntityRotation,
    tick_count: u64,
) -> ServerEntityState {
    ServerEntityState {
        id,
        persistent_id,
        kind: EntityKind::DebugCube,
        item_stack: None,
        mallard: None,
        mallard_nest: None,
        animation: None,
        position,
        y_rot_degrees,
        x_rot_degrees,
        rotation: Some(rotation),
        on_ground: false,
        width: 1.0,
        height: 1.0,
        tick_count,
        alive: true,
        hidden_from_clients: false,
    }
}

fn debug_passive_showcase_position(spawn_position: Vec3d, index: usize) -> Vec3d {
    let chunk = BlockPos::containing(spawn_position).chunk_pos();
    let offset = debug_passive_showcase_offset(index);
    Vec3d::new(
        offset_within_chunk(spawn_position.x + offset.x, chunk.min_block_x()),
        spawn_position.y,
        offset_within_chunk(spawn_position.z + offset.z, chunk.min_block_z()),
    )
}

fn debug_passive_showcase_offset(index: usize) -> Vec3d {
    const OFFSETS: [Vec3d; 8] = [
        Vec3d::new(2.5, 0.0, 2.5),
        Vec3d::new(-2.5, 0.0, 2.5),
        Vec3d::new(2.5, 0.0, -2.5),
        Vec3d::new(-2.5, 0.0, -2.5),
        Vec3d::new(4.5, 0.0, 0.0),
        Vec3d::new(-4.5, 0.0, 0.0),
        Vec3d::new(0.0, 0.0, 4.5),
        Vec3d::new(0.0, 0.0, -4.5),
    ];
    OFFSETS[index % OFFSETS.len()]
}

fn debug_passive_showcase_y_rot(index: usize) -> f32 {
    45.0 + (index % 8) as f32 * 45.0
}

fn offset_within_chunk(value: f64, chunk_min: i32) -> f64 {
    (value + 2.5).clamp(chunk_min as f64 + 1.5, chunk_min as f64 + 14.5)
}

fn entity_aabb(entity: ServerEntityState) -> Aabb {
    collision_aabb_for_feet_position(
        entity.position,
        f64::from(entity.width),
        f64::from(entity.height),
    )
}

fn item_merge_box(entity: ServerEntityState) -> Aabb {
    let aabb = entity_aabb(entity);
    Aabb::new(
        aabb.min_x - ITEM_MERGE_INFLATE_XZ,
        aabb.min_y,
        aabb.min_z - ITEM_MERGE_INFLATE_XZ,
        aabb.max_x + ITEM_MERGE_INFLATE_XZ,
        aabb.max_y,
        aabb.max_z + ITEM_MERGE_INFLATE_XZ,
    )
}

fn player_pickup_box(position: Vec3d) -> Aabb {
    let aabb =
        collision_aabb_for_feet_position(position, PLAYER_PICKUP_WIDTH, PLAYER_PICKUP_HEIGHT);
    Aabb::new(
        aabb.min_x - PLAYER_PICKUP_INFLATE_XZ,
        aabb.min_y - PLAYER_PICKUP_INFLATE_Y,
        aabb.min_z - PLAYER_PICKUP_INFLATE_XZ,
        aabb.max_x + PLAYER_PICKUP_INFLATE_XZ,
        aabb.max_y + PLAYER_PICKUP_INFLATE_Y,
        aabb.max_z + PLAYER_PICKUP_INFLATE_XZ,
    )
}

fn item_merge_due(tick_count: u64, block_position_changed: bool) -> bool {
    let interval = if block_position_changed {
        ITEM_MOVED_BLOCK_MERGE_INTERVAL_TICKS
    } else {
        ITEM_STATIONARY_MERGE_INTERVAL_TICKS
    };
    tick_count % interval == 0
}

fn entity_kind_code(kind: EntityKind) -> Option<&'static str> {
    match kind {
        EntityKind::Cow => Some("minecraft:cow"),
        EntityKind::Chicken => Some("minecraft:chicken"),
        EntityKind::Mallard => Some("mclone:mallard"),
        EntityKind::MallardNest => Some("mclone:mallard_nest"),
        EntityKind::Deer => Some("mclone:deer"),
        EntityKind::DeerBed => Some("mclone:deer_bed"),
        EntityKind::Bee => Some("mclone:bee"),
        EntityKind::BeeNest => Some("mclone:bee_nest"),
        EntityKind::BeeHotel => Some("mclone:bee_hotel"),
        EntityKind::Rabbit => Some("mclone:rabbit"),
        EntityKind::RabbitBurrow => Some("mclone:rabbit_burrow"),
        EntityKind::WildlifeRemains => Some("mclone:wildlife_remains"),
        EntityKind::Mannequin => Some("mclone:mannequin"),
        EntityKind::Item => Some("minecraft:item"),
        EntityKind::DebugCube => None,
    }
}

fn item_stack_snapshot_from_save(
    stack: &ItemStackSaveRecord,
) -> ChunkStoreResult<ItemStackSnapshot> {
    let kind = match stack.kind.as_str() {
        "minecraft:egg" => ItemKind::Egg,
        "mclone:mallard_egg" => ItemKind::MallardEgg,
        "mclone:mallard_feather" => ItemKind::MallardFeather,
        "mclone:hunting_spear" => ItemKind::HuntingSpear,
        "mclone:venison" => ItemKind::Venison,
        "mclone:deer_hide" => ItemKind::DeerHide,
        "mclone:shed_antler" => ItemKind::ShedAntler,
        "mclone:bee_hotel" => ItemKind::BeeHotel,
        "mclone:beeswax" => ItemKind::Beeswax,
        "minecraft:wooden_hoe" => ItemKind::WoodenHoe,
        "minecraft:wheat_seeds" => ItemKind::WheatSeeds,
        "minecraft:wheat" => ItemKind::Wheat,
        "minecraft:carrot" => ItemKind::Carrot,
        "minecraft:oak_fence" => ItemKind::OakFence,
        "minecraft:oak_fence_gate" => ItemKind::OakFenceGate,
        kind => {
            return Err(ChunkStoreError::InvalidData(format!(
                "unsupported item stack kind {kind:?}"
            )));
        }
    };
    Ok(ItemStackSnapshot {
        kind,
        count: stack.count,
    })
}

fn known_place_from_save(saved: RabbitRefugeSaveRecord) -> KnownPlace {
    if saved.last_known_position.is_none()
        && saved.revision.is_none()
        && saved.last_confirmed_tick == 0
        && saved.familiarity == 1
    {
        return KnownPlace::unresolved(saved.persistent_id);
    }
    KnownPlace {
        locator: WorldFactLocator {
            persistent_id: saved.persistent_id,
            last_known_position: saved.last_known_position,
            revision: saved.revision,
        },
        last_confirmed_tick: saved.last_confirmed_tick,
        familiarity: saved.familiarity,
    }
}

fn rabbit_refuge_to_save(known: KnownPlace) -> RabbitRefugeSaveRecord {
    RabbitRefugeSaveRecord {
        persistent_id: known.locator.persistent_id,
        last_known_position: known.locator.last_known_position,
        revision: known.locator.revision,
        last_confirmed_tick: known.last_confirmed_tick,
        familiarity: known.familiarity,
    }
}

fn is_mallard_egg_habitat(
    position: Vec3d,
    block_state_at: &impl Fn(BlockPos) -> Option<BlockStateId>,
) -> bool {
    sample_wetland_habitat(BlockPos::containing(position), &mut |pos| {
        block_state_at(pos)
    })
    .is_ok_and(|sample| sample.suitable())
}

fn is_valid_mallard_nest_site(
    position: Vec3d,
    block_state_at: &impl Fn(BlockPos) -> Option<BlockStateId>,
) -> bool {
    let feet = BlockPos::containing(position);
    let empty_space = [feet, feet.offset(0, 1, 0)].into_iter().all(|pos| {
        block_state_at(pos)
            .is_some_and(|state| mclone_blocks::block_collision_aabb(state, pos).is_none())
    });
    empty_space
        && sample_wetland_habitat(feet, &mut |pos| block_state_at(pos))
            .is_ok_and(|sample| sample.suitable() && sample.cover_blocks > 0)
}

fn mallard_target_position(target: BlockPos) -> Vec3d {
    Vec3d::new(
        f64::from(target.x) + 0.5,
        f64::from(target.y),
        f64::from(target.z) + 0.5,
    )
}

fn is_valid_bee_colony_site(
    position: Vec3d,
    block_state_at: &impl Fn(BlockPos) -> Option<BlockStateId>,
) -> bool {
    let feet = BlockPos::containing(position);
    let support = feet.offset(0, -1, 0);
    [feet, feet.offset(0, 1, 0)].into_iter().all(|pos| {
        block_state_at(pos)
            .is_some_and(|state| mclone_blocks::block_collision_aabb(state, pos).is_none())
    }) && block_state_at(support)
        .is_some_and(|state| mclone_blocks::block_collision_aabb(state, support).is_some())
}

fn squared_distance_xz(left: Vec3d, right: Vec3d) -> f64 {
    let dx = left.x - right.x;
    let dz = left.z - right.z;
    dx * dx + dz * dz
}

fn stable_rabbit_pair_normal(left: EntityPersistentId, right: EntityPersistentId) -> (f64, f64) {
    let diagonal = std::f64::consts::FRAC_1_SQRT_2;
    match (left.least ^ right.least.rotate_left(23)) & 7 {
        0 => (1.0, 0.0),
        1 => (diagonal, diagonal),
        2 => (0.0, 1.0),
        3 => (-diagonal, diagonal),
        4 => (-1.0, 0.0),
        5 => (-diagonal, -diagonal),
        6 => (0.0, -1.0),
        _ => (diagonal, -diagonal),
    }
}

fn rabbit_release_position(mouth: ServerEntityState, rabbit: EntityPersistentId) -> Vec3d {
    let yaw = f64::from(mouth.y_rot_degrees).to_radians();
    let forward_x = -yaw.sin();
    let forward_z = yaw.cos();
    let right_x = forward_z;
    let right_z = -forward_x;
    let side = match rabbit.least % 3 {
        0 => -0.12,
        1 => 0.0,
        _ => 0.12,
    };
    mouth.position.add(Vec3d::new(
        -forward_x * 0.28 + right_x * side,
        0.0,
        -forward_z * 0.28 + right_z * side,
    ))
}

type RabbitBreedingCandidate = (
    EntityId,
    EntityPersistentId,
    Vec3d,
    Option<EntityPersistentId>,
);

fn rabbit_breeding_pair(
    candidates: &[RabbitBreedingCandidate],
) -> Option<(
    RabbitBreedingCandidate,
    RabbitBreedingCandidate,
    Option<EntityPersistentId>,
)> {
    const CLOSE_COURTSHIP_DISTANCE: f64 = 4.0;
    const REFUGE_HOME_RANGE_COURTSHIP_DISTANCE: f64 = 32.0;

    for (index, left) in candidates.iter().copied().enumerate() {
        for right in candidates[index + 1..].iter().copied() {
            let breeding_home = match (left.3, right.3) {
                (Some(left), Some(right)) => Some(left.min(right)),
                (left, right) => left.or(right),
            };
            let courtship_distance = if breeding_home.is_some() {
                REFUGE_HOME_RANGE_COURTSHIP_DISTANCE
            } else {
                CLOSE_COURTSHIP_DISTANCE
            };
            if squared_distance_xz(left.2, right.2) <= courtship_distance * courtship_distance {
                return Some((left, right, breeding_home));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::block::generated_block_state_id;

    #[test]
    fn rabbit_pairing_uses_a_bounded_refuge_bearing_home_range() {
        let first_refuge = EntityPersistentId { most: 9, least: 9 };
        let second_refuge = EntityPersistentId { most: 9, least: 10 };
        let left = (
            EntityId(1),
            EntityPersistentId { most: 1, least: 1 },
            Vec3d::new(0.0, 64.0, 0.0),
            Some(first_refuge),
        );
        let within_home_range = (
            EntityId(2),
            EntityPersistentId { most: 1, least: 2 },
            Vec3d::new(31.0, 64.0, 0.0),
            Some(second_refuge),
        );
        let beyond_home_range = (
            EntityId(3),
            EntityPersistentId { most: 1, least: 3 },
            Vec3d::new(33.0, 64.0, 0.0),
            Some(first_refuge),
        );
        let shelterless = (
            EntityId(4),
            EntityPersistentId { most: 1, least: 4 },
            Vec3d::new(5.0, 64.0, 0.0),
            None,
        );
        let shelterless_close = (
            EntityId(5),
            EntityPersistentId { most: 1, least: 5 },
            Vec3d::new(3.0, 64.0, 0.0),
            None,
        );
        let shelterless_far = (
            EntityId(6),
            EntityPersistentId { most: 1, least: 6 },
            Vec3d::new(0.0, 64.0, 0.0),
            None,
        );

        assert_eq!(
            rabbit_breeding_pair(&[left, within_home_range]),
            Some((left, within_home_range, Some(first_refuge)))
        );
        assert_eq!(rabbit_breeding_pair(&[left, beyond_home_range]), None);
        assert_eq!(rabbit_breeding_pair(&[shelterless_far, shelterless]), None);
        assert_eq!(
            rabbit_breeding_pair(&[shelterless_close, shelterless]),
            Some((shelterless_close, shelterless, None))
        );
    }

    fn adult_rabbit_with_refuge(
        refuge: EntityPersistentId,
        position: Vec3d,
        sheltered: bool,
    ) -> RabbitRuntimeSaveData {
        RabbitRuntimeSaveData {
            known_refuges: [
                Some(KnownPlace::observed(
                    refuge,
                    BlockPos::containing(position),
                    0,
                )),
                None,
                None,
            ],
            sheltered_in: sheltered.then_some(refuge),
            dig_target: None,
            decision_schedule: DecisionSchedule::new(0, 0),
            dig_cooldown: 0,
            life_stage: RabbitLifeStage::Adult,
            parents: [None; 2],
            behavior: if sheltered {
                RabbitBehavior::Underground
            } else {
                RabbitBehavior::Idle
            },
            behavior_ticks: 0,
            health: 3,
            max_health: 3,
            love_ticks: 0,
            raid_cooldown: 0,
            lifecycle: WildlifeLifeState::founder(
                EntityPersistentId::new(0, 9_001),
                24_000,
                480_000,
                120_000,
            ),
        }
    }

    #[test]
    fn periodic_store_canonicalizes_entity_pose_and_chunk_ownership() {
        let topology = HorizontalTopology::new(
            mclone_core::AxisTopology::periodic(0, 32),
            mclone_core::AxisTopology::Unbounded,
        );
        let mut store = ServerEntityStore::with_topology(topology);

        let id =
            store.insert_passive_mob_for_test(EntityKind::Cow, Vec3d::new(512.5, 64.0, 8.5), 0.0);

        assert_eq!(
            store.state(id).unwrap().position,
            Vec3d::new(0.5, 64.0, 8.5)
        );
        assert_eq!(
            store
                .persistent_entity_chunk_positions()
                .into_values()
                .collect::<Vec<_>>(),
            vec![ChunkPos::new(0, 0)]
        );
    }
    use crate::entity::metadata::{EntityDimensions, EntityMetadata, PASSIVE_MOB_KINDS};
    use crate::entity::mob::BlockPathType;

    fn no_blocks(_pos: BlockPos) -> Option<BlockStateId> {
        None
    }

    fn flat_ground(pos: BlockPos) -> Option<BlockStateId> {
        Some(if pos.y == 63 {
            BlockStateId(1)
        } else {
            BlockStateId(mclone_blocks::terrain_id::AIR)
        })
    }

    fn flat_meadow(pos: BlockPos) -> Option<BlockStateId> {
        Some(if pos.y == 63 {
            generated_block_state_id(mclone_worldgen::block::GRASS_BLOCK)
        } else {
            BlockStateId(mclone_blocks::terrain_id::AIR)
        })
    }

    fn ready_wildlife_lifecycle(age_ticks: u32) -> WildlifeLifeState {
        WildlifeLifeState {
            age_ticks,
            lifespan_ticks: 1_000_000,
            energy: 1_000,
            deficit_ticks: 0,
            recent_intake: 0,
            reproductive_condition: 1_000,
            reproduction_cooldown: 0,
        }
    }

    #[test]
    fn wild_rabbits_breed_without_a_burrow_when_condition_allows() {
        let mut store = ServerEntityStore::default();
        let left =
            store.insert_passive_mob_for_test(EntityKind::Rabbit, Vec3d::new(4.5, 64.0, 4.5), 0.0);
        let right =
            store.insert_passive_mob_for_test(EntityKind::Rabbit, Vec3d::new(5.5, 64.0, 4.5), 0.0);
        for id in [left, right] {
            store
                .mobs
                .get_mut(&id)
                .unwrap()
                .set_wildlife_lifecycle_for_test(ready_wildlife_lifecycle(24_000), None);
        }
        let mut resources = WildlifeResourceLedger::default();
        store.tick_wildlife_lifecycle(20, &[ChunkPos::new(0, 0)], &mut resources, &flat_meadow);
        store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_meadow);

        let rabbits = store
            .wildlife_life_diagnostics()
            .into_iter()
            .filter(|animal| animal.kind == EntityKind::Rabbit)
            .collect::<Vec<_>>();
        assert_eq!(rabbits.len(), 3);
        let kit = rabbits
            .iter()
            .find(|animal| animal.rabbit_life_stage == Some(RabbitLifeStage::Kit))
            .expect("eligible wild pair should create a kit without a refuge");
        assert!(kit.parents.iter().all(Option::is_some));
        assert!(store.drain_wildlife_events().iter().any(|event| {
            matches!(
                event.kind,
                WildlifeEcologyEventKind::Birth { child, .. } if child == kit.persistent_id
            )
        }));
    }

    #[test]
    fn eligible_deer_pair_creates_one_fawn_and_parent_cooldowns() {
        let mut store = ServerEntityStore::default();
        let female =
            store.insert_passive_mob_for_test(EntityKind::Deer, Vec3d::new(4.5, 64.0, 4.5), 0.0);
        let male =
            store.insert_passive_mob_for_test(EntityKind::Deer, Vec3d::new(7.5, 64.0, 4.5), 0.0);
        store
            .mobs
            .get_mut(&female)
            .unwrap()
            .set_wildlife_lifecycle_for_test(
                ready_wildlife_lifecycle(120_000),
                Some(mclone_protocol::DeerSex::Female),
            );
        store
            .mobs
            .get_mut(&male)
            .unwrap()
            .set_wildlife_lifecycle_for_test(
                ready_wildlife_lifecycle(120_000),
                Some(mclone_protocol::DeerSex::Male),
            );

        let mut resources = WildlifeResourceLedger::default();
        store.tick_wildlife_lifecycle(20, &[ChunkPos::new(0, 0)], &mut resources, &flat_meadow);
        let deer = store
            .wildlife_life_diagnostics()
            .into_iter()
            .filter(|animal| animal.kind == EntityKind::Deer)
            .collect::<Vec<_>>();
        assert_eq!(deer.len(), 3);
        assert_eq!(
            deer.iter()
                .filter(
                    |animal| animal.deer_life_stage == Some(mclone_protocol::DeerLifeStage::Fawn)
                )
                .count(),
            1
        );
        assert_eq!(
            deer.iter()
                .filter(
                    |animal| animal.deer_life_stage == Some(mclone_protocol::DeerLifeStage::Adult)
                )
                .filter(|animal| animal.lifecycle.reproduction_cooldown > 0)
                .count(),
            2
        );
        let events = store.drain_wildlife_events();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.kind, WildlifeEcologyEventKind::Birth { .. }))
                .count(),
            1
        );
        assert!(!events.iter().any(|event| matches!(
            event.kind,
            WildlifeEcologyEventKind::ReproductionSuppressed {
                reason: WildlifeReproductionSuppression::NoMate
            }
        )));
    }

    #[test]
    fn fed_mallard_pair_establishes_one_natural_covered_shore_nest() {
        let mut store = ServerEntityStore::default();
        let female =
            store.insert_passive_mob_for_test(EntityKind::Mallard, Vec3d::new(4.5, 64.0, 4.5), 0.0);
        let male =
            store.insert_passive_mob_for_test(EntityKind::Mallard, Vec3d::new(3.5, 64.0, 4.5), 0.0);
        store
            .mobs
            .get_mut(&female)
            .unwrap()
            .set_mallard_lifecycle_for_test(
                ready_wildlife_lifecycle(120_000),
                mclone_protocol::MallardSex::Female,
            );
        store
            .mobs
            .get_mut(&male)
            .unwrap()
            .set_mallard_lifecycle_for_test(
                ready_wildlife_lifecycle(120_000),
                mclone_protocol::MallardSex::Male,
            );

        let mut resources = WildlifeResourceLedger::default();
        store.tick_wildlife_lifecycle(
            20,
            &[ChunkPos::new(0, 0)],
            &mut resources,
            &covered_wetland_ground,
        );

        let nests = store
            .states()
            .into_iter()
            .filter(|entity| entity.kind == EntityKind::MallardNest && entity.alive)
            .collect::<Vec<_>>();
        assert_eq!(nests.len(), 1);
        let events = store.drain_wildlife_events();
        assert!(events.iter().any(|event| matches!(
            event.kind,
            WildlifeEcologyEventKind::NestEstablished { nest, parents }
                if nest == nests[0].persistent_id
                    && parents == [
                        store.state(female).unwrap().persistent_id,
                        store.state(male).unwrap().persistent_id,
                    ]
        )));
        assert!(
            resources.snapshots()[0]
                .strata
                .iter()
                .any(|stratum| stratum.mallard_consumed > 0)
        );
        assert!(
            store
                .wildlife_life_diagnostics()
                .into_iter()
                .filter(|animal| animal.kind == EntityKind::Mallard)
                .all(|animal| animal.lifecycle.reproduction_cooldown > 0)
        );

        store.tick_wildlife_lifecycle(
            40,
            &[ChunkPos::new(0, 0)],
            &mut resources,
            &covered_wetland_ground,
        );
        assert_eq!(
            store
                .states()
                .iter()
                .filter(|entity| entity.kind == EntityKind::MallardNest && entity.alive)
                .count(),
            1
        );
    }

    #[test]
    fn mallard_nest_attempt_persists_and_drives_shore_travel() {
        let mut store = ServerEntityStore::default();
        let female =
            store.insert_passive_mob_for_test(EntityKind::Mallard, Vec3d::new(2.5, 64.0, 4.5), 0.0);
        let male =
            store.insert_passive_mob_for_test(EntityKind::Mallard, Vec3d::new(3.5, 64.0, 4.5), 0.0);
        for (id, sex) in [
            (female, mclone_protocol::MallardSex::Female),
            (male, mclone_protocol::MallardSex::Male),
        ] {
            store
                .mobs
                .get_mut(&id)
                .unwrap()
                .set_mallard_lifecycle_for_test(ready_wildlife_lifecycle(120_000), sex);
        }
        let target = BlockPos::new(6, 64, 4);
        store
            .mobs
            .get_mut(&female)
            .unwrap()
            .set_mallard_nest_target(Some(target));
        let record = store.entity_chunk_record(ChunkPos::new(0, 0), 15);
        assert!(record.entities.iter().any(|entity| matches!(
            entity.payload,
            EntitySavePayload::Mallard {
                nest_target: Some(saved),
                ..
            } if saved == target
        )));

        let mut resources = WildlifeResourceLedger::default();
        let ticking_chunks = local_ticking_chunks();
        for tick in 1..=600_u64 {
            store.tick_stationary_at_time(
                &ticking_chunks,
                &[],
                6_000 + tick,
                covered_wetland_ground,
            );
            store.tick_wildlife_lifecycle(
                tick,
                &ticking_chunks,
                &mut resources,
                &covered_wetland_ground,
            );
            if store
                .states()
                .iter()
                .any(|entity| entity.kind == EntityKind::MallardNest && entity.alive)
            {
                break;
            }
        }
        let nest = store
            .states()
            .into_iter()
            .find(|entity| entity.kind == EntityKind::MallardNest && entity.alive)
            .unwrap_or_else(|| {
                panic!(
                    "persisted nest intent should finish: female={:?} target={:?} intent={:?} lifecycle={:?}",
                    store.state(female),
                    store.mobs[&female].mallard_nest_target(),
                    store.mobs[&female].mallard_habitat_intent_for_test(),
                    store.mobs[&female].mallard_save_data(),
                )
            });
        assert_eq!(BlockPos::containing(nest.position), target);
        assert!(squared_distance_xz(store.state(female).unwrap().position, nest.position) <= 0.5);
    }

    #[test]
    fn natural_death_persists_remains_and_decay_conserves_biomass() {
        let mut store = ServerEntityStore::default();
        let rabbit =
            store.insert_passive_mob_for_test(EntityKind::Rabbit, Vec3d::new(4.5, 64.0, 4.5), 0.0);
        let mut terminal = ready_wildlife_lifecycle(40_000);
        terminal.lifespan_ticks = terminal.age_ticks;
        store
            .mobs
            .get_mut(&rabbit)
            .unwrap()
            .set_wildlife_lifecycle_for_test(terminal, None);
        let source = store.state(rabbit).unwrap().persistent_id;
        let mut resources = WildlifeResourceLedger::default();
        store.tick_wildlife_lifecycle(20, &[ChunkPos::new(0, 0)], &mut resources, &flat_meadow);
        assert!(store.state(rabbit).is_none());
        let remains = store.wildlife_remains_diagnostics();
        assert_eq!(remains.len(), 1);
        assert_eq!(remains[0].source, source);
        assert_eq!(remains[0].biomass, 120);
        let first_events = store.drain_wildlife_events();
        assert!(first_events.iter().any(|event| matches!(
            event.kind,
            WildlifeEcologyEventKind::Death {
                cause: crate::ecology::WildlifeDeathCause::OldAge
            }
        )));
        assert_eq!(
            first_events
                .iter()
                .filter_map(|event| match event.kind {
                    WildlifeEcologyEventKind::RemainsCreated { biomass } => Some(biomass),
                    _ => None,
                })
                .sum::<u32>(),
            120
        );

        let record = store.entity_chunk_record(ChunkPos::new(0, 0), 13);
        assert!(record.entities.iter().any(|entity| matches!(
            entity.payload,
            EntitySavePayload::WildlifeRemains { biomass: 120, .. }
        )));
        let mut loaded = ServerEntityStore::default();
        loaded.hydrate_entity_chunk_record(&record).unwrap();
        assert_eq!(loaded.wildlife_remains_diagnostics()[0].biomass, 120);

        let mut decayed = 0_u32;
        for step in 1..=1_500_u64 {
            loaded.tick_wildlife_lifecycle(
                20 + step * 20,
                &[ChunkPos::new(0, 0)],
                &mut resources,
                &flat_meadow,
            );
            decayed = decayed.saturating_add(
                loaded
                    .drain_wildlife_events()
                    .iter()
                    .filter_map(|event| match event.kind {
                        WildlifeEcologyEventKind::RemainsDecayed { amount } => Some(amount),
                        _ => None,
                    })
                    .sum::<u32>(),
            );
            if loaded.wildlife_remains_diagnostics().is_empty() {
                break;
            }
        }
        assert_eq!(decayed, 120);
        assert!(loaded.wildlife_remains_diagnostics().is_empty());
    }

    #[test]
    fn natural_mallard_death_creates_typed_durable_remains() {
        let mut store = ServerEntityStore::default();
        let mallard =
            store.insert_passive_mob_for_test(EntityKind::Mallard, Vec3d::new(4.5, 64.0, 4.5), 0.0);
        let mut terminal = ready_wildlife_lifecycle(40_000);
        terminal.lifespan_ticks = terminal.age_ticks;
        store
            .mobs
            .get_mut(&mallard)
            .unwrap()
            .set_mallard_lifecycle_for_test(terminal, mclone_protocol::MallardSex::Female);
        let source = store.state(mallard).unwrap().persistent_id;
        let mut resources = WildlifeResourceLedger::default();

        store.tick_wildlife_lifecycle(
            20,
            &[ChunkPos::new(0, 0)],
            &mut resources,
            &covered_wetland_ground,
        );

        assert!(store.state(mallard).is_none());
        let remains = store.wildlife_remains_diagnostics();
        assert_eq!(remains.len(), 1);
        assert_eq!(remains[0].source_species, WildlifeRemainsSpecies::Mallard);
        assert_eq!(remains[0].source, source);
        assert_eq!(remains[0].biomass, 180);
        let record = store.entity_chunk_record(ChunkPos::new(0, 0), 14);
        assert!(record.entities.iter().any(|entity| matches!(
            entity.payload,
            EntitySavePayload::WildlifeRemains {
                source_species: WildlifeRemainsSpecies::Mallard,
                biomass: 180,
                ..
            }
        )));
    }

    fn wetland_ground(pos: BlockPos) -> Option<BlockStateId> {
        use mclone_worldgen::block::{AIR, DIRT, GRASS_BLOCK, WATER, generated_block_state_id};

        let raw = if pos.y <= 62 {
            DIRT
        } else if pos.y == 63 {
            GRASS_BLOCK
        } else if pos.y == 64 && (6..=8).contains(&pos.x) {
            WATER
        } else {
            AIR
        };
        Some(generated_block_state_id(raw))
    }

    fn covered_wetland_ground(pos: BlockPos) -> Option<BlockStateId> {
        use mclone_worldgen::block::{
            AIR, DIRT, GRASS_BLOCK, SUGAR_CANE, WATER, generated_block_state_id,
        };

        let raw = if pos.y <= 62 {
            DIRT
        } else if pos.y == 63 {
            GRASS_BLOCK
        } else if pos.y == 64 && (7..=10).contains(&pos.x) {
            WATER
        } else if pos == BlockPos::new(6, 64, 6) {
            SUGAR_CANE
        } else {
            AIR
        };
        Some(generated_block_state_id(raw))
    }

    fn broad_shallow_water(pos: BlockPos) -> Option<BlockStateId> {
        use mclone_worldgen::block::{AIR, DIRT, GRASS_BLOCK, WATER, generated_block_state_id};

        let raw = if pos.y <= 62 {
            DIRT
        } else if pos.y == 63 {
            GRASS_BLOCK
        } else if pos.y == 64 && (1..=12).contains(&pos.x) {
            WATER
        } else {
            AIR
        };
        Some(generated_block_state_id(raw))
    }

    fn dry_grass_ground(pos: BlockPos) -> Option<BlockStateId> {
        use mclone_worldgen::block::{AIR, GRASS_BLOCK, generated_block_state_id};

        Some(generated_block_state_id(if pos.y == 63 {
            GRASS_BLOCK
        } else {
            AIR
        }))
    }

    fn local_ticking_chunks() -> Vec<ChunkPos> {
        (-1..=1)
            .flat_map(|x| (-1..=1).map(move |z| ChunkPos::new(x, z)))
            .collect()
    }

    #[test]
    fn debug_passive_showcase_spawns_once_near_initial_spawn() {
        let mut store = ServerEntityStore::default();
        let metadata = EntityMetadata::for_kind(EntityKind::Cow).unwrap();

        let first =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(15.0, 64.0, 15.0), true);
        let second =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(1.0, 64.0, 1.0), true);

        assert_eq!(first, second);
        assert_eq!(first.len(), PASSIVE_MOB_KINDS.len());
        let entity = store.state(first[0]).unwrap();
        assert_eq!(entity.kind, EntityKind::Cow);
        assert_eq!(entity.width, metadata.dimensions.width);
        assert_eq!(entity.height, metadata.dimensions.height);
        assert_eq!(entity.position, Vec3d::new(14.5, 64.0, 14.5));
        let mob = store.mob_state(first[0]).expect("starter cow mob state");
        assert_eq!(mob.movement_speed(), metadata.movement_speed);
        assert_eq!(mob.eye_height(), metadata.standing_eye_height() as f64);
        assert_eq!(mob.pathfinding_malus(BlockPathType::Water), 8.0);
        assert_eq!(mob.available_goal_count(), 3);

        let chicken = store.state(first[1]).unwrap();
        assert_eq!(chicken.kind, EntityKind::Chicken);
        let chicken_mob = store
            .mob_state(first[1])
            .expect("starter chicken mob state");
        assert_eq!(chicken_mob.available_goal_count(), 3);
    }

    #[test]
    fn persistent_marker_identity_is_idempotent_and_kind_checked() {
        let mut store = ServerEntityStore::default();
        let persistent_id = EntityPersistentId::new(0x434f_5701_0000_0000, 1);
        let position = Vec3d::new(4.5, 64.0, 4.5);

        let first = store
            .ensure_persistent_passive_mob(persistent_id, EntityKind::Cow, position, 0.0)
            .unwrap()
            .expect("first marker realization must insert the cow");
        let repeated = store
            .ensure_persistent_passive_mob(
                persistent_id,
                EntityKind::Cow,
                Vec3d::new(9.5, 64.0, 9.5),
                180.0,
            )
            .unwrap();

        assert!(repeated.is_none());
        assert_eq!(store.states(), vec![first]);
        assert!(
            store
                .ensure_persistent_passive_mob(persistent_id, EntityKind::Chicken, position, 0.0,)
                .unwrap_err()
                .to_string()
                .contains("marker requested Chicken")
        );
    }

    #[test]
    fn debug_passive_showcase_adopts_hydrated_actors_without_duplication() {
        let mut first = ServerEntityStore::default();
        let first_ids =
            first.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true);
        let first_persistent_ids = first_ids
            .iter()
            .map(|id| first.state(*id).unwrap().persistent_id)
            .collect::<Vec<_>>();
        let record = first.entity_chunk_record(ChunkPos::new(0, 0), 1);

        let mut reloaded = ServerEntityStore::default();
        reloaded.next_entity_id = 100;
        let hydrated = reloaded.hydrate_entity_chunk_record(&record).unwrap();
        let adopted =
            reloaded.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true);

        assert_eq!(adopted.len(), PASSIVE_MOB_KINDS.len());
        assert_eq!(reloaded.states().len(), PASSIVE_MOB_KINDS.len());
        assert_eq!(
            adopted,
            hydrated.iter().map(|entity| entity.id).collect::<Vec<_>>()
        );
        assert_eq!(
            adopted
                .iter()
                .map(|id| reloaded.state(*id).unwrap().persistent_id)
                .collect::<Vec<_>>(),
            first_persistent_ids
        );
    }

    #[test]
    fn hydrated_actors_replace_provisional_showcase_without_duplication() {
        let mut saved = ServerEntityStore::default();
        let saved_ids =
            saved.ensure_debug_passive_showcase_near_spawn(Vec3d::new(40.0, 64.0, 40.0), true);
        let saved_persistent_ids = saved_ids
            .iter()
            .map(|id| saved.state(*id).unwrap().persistent_id)
            .collect::<Vec<_>>();
        let record = saved.entity_chunk_record(ChunkPos::new(2, 2), 1);

        let mut reloaded = ServerEntityStore::default();
        reloaded.next_persistent_id = saved_persistent_ids
            .iter()
            .map(|id| id.least)
            .max()
            .unwrap();
        let provisional =
            reloaded.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true);
        let hydrated = reloaded.hydrate_entity_chunk_record(&record).unwrap();
        let removed = reloaded.adopt_hydrated_debug_passive_showcase(&hydrated);

        assert_eq!(removed.len(), PASSIVE_MOB_KINDS.len());
        assert!(removed.iter().all(|entity| !entity.alive));
        assert_eq!(
            removed.iter().map(|entity| entity.id).collect::<Vec<_>>(),
            provisional
        );
        assert_eq!(reloaded.states(), hydrated);
        assert_eq!(
            hydrated
                .iter()
                .map(|entity| entity.persistent_id)
                .collect::<Vec<_>>(),
            saved_persistent_ids
        );
        assert_eq!(
            reloaded.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true),
            hydrated.iter().map(|entity| entity.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn debug_passive_showcase_can_be_disabled() {
        let mut store = ServerEntityStore::default();

        let ids = store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), false);

        assert!(ids.is_empty());
        assert_eq!(store.diagnostics().stored_entities, 0);
    }

    #[test]
    fn periodic_showcase_keeps_one_identity_through_both_seam_directions() {
        let topology = HorizontalTopology::cylinder_x(0, 32);
        let mut store = ServerEntityStore::with_topology(topology);
        let id =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true)[0];
        let mut chunks = Vec::new();

        for _ in 0..161 {
            let state = store
                .advance_debug_periodic_showcase()
                .expect("periodic showcase actor");
            assert_eq!(state.id, id);
            assert_eq!(
                topology.canonicalize_position(state.position),
                Some(state.position)
            );
            chunks.push(state.chunk_pos().x);
        }

        assert!(chunks.windows(2).any(|pair| pair == [31, 0]));
        assert!(chunks.windows(2).any(|pair| pair == [0, 31]));
        assert_eq!(store.debug_periodic_showcase_crossings(), 2);
        assert_eq!(store.state(id).unwrap().id, id);
    }

    #[test]
    fn natural_spawn_category_counts_include_live_passive_mobs_but_not_items_or_misc() {
        let mut store = ServerEntityStore::default();
        store.insert_passive_mob_for_test(EntityKind::Cow, Vec3d::new(4.0, 64.0, 4.0), 0.0);
        store.insert_passive_mob_for_test(EntityKind::Chicken, Vec3d::new(5.0, 64.0, 4.0), 0.0);
        store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            },
            Vec3d::new(6.0, 64.0, 4.0),
        );

        let counts = store.natural_spawn_category_counts();

        assert_eq!(counts.get(MobCategory::Creature), 2);
        assert_eq!(counts.get(MobCategory::Misc), 0);
    }

    #[test]
    fn volatile_passive_spawn_uses_normal_mob_runtime_and_counts_as_creature() {
        let mut store = ServerEntityStore::default();
        let state =
            store.spawn_volatile_passive_mob(EntityKind::Chicken, Vec3d::new(8.5, 64.0, 8.5), 90.0);

        assert_eq!(state.kind, EntityKind::Chicken);
        assert_eq!(state.position, Vec3d::new(8.5, 64.0, 8.5));
        assert!(state.on_ground);
        assert!(store.mob_state(state.id).is_some());
        assert_eq!(
            store
                .natural_spawn_category_counts()
                .get(MobCategory::Creature),
            1
        );
    }

    #[test]
    fn volatile_passive_entities_discard_only_on_full_chunk_unload() {
        let mut store = ServerEntityStore::default();
        let volatile =
            store.spawn_volatile_passive_mob(EntityKind::Cow, Vec3d::new(8.5, 64.0, 8.5), 0.0);
        let retained =
            store.insert_passive_mob_for_test(EntityKind::Chicken, Vec3d::new(9.5, 64.0, 8.5), 0.0);
        let other_chunk =
            store.spawn_volatile_passive_mob(EntityKind::Chicken, Vec3d::new(24.5, 64.0, 8.5), 0.0);

        store.tick_stationary(
            &[ChunkPos::new(0, 0), ChunkPos::new(1, 0)],
            &[],
            flat_ground,
        );
        store.tick_stationary(&[], &[], flat_ground);
        assert!(store.state(volatile.id).is_some());
        assert!(store.mob_state(volatile.id).is_some());
        assert_eq!(
            store.diagnostics(),
            ServerEntityStoreDiagnostics {
                stored_entities: 3,
                ticking_entities: 0
            }
        );

        let removed = store.discard_volatile_entities_in_chunk(ChunkPos::new(0, 0));

        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].id, volatile.id);
        assert!(!removed[0].alive);
        assert_eq!(store.state(volatile.id), None);
        assert!(store.mob_state(volatile.id).is_none());
        assert!(store.state(retained).is_some());
        assert!(store.state(other_chunk.id).is_some());
        assert_eq!(
            store
                .natural_spawn_category_counts()
                .get(MobCategory::Creature),
            2
        );
    }

    #[test]
    fn chicken_passive_mob_can_snapshot_metadata_dimensions() {
        let mut store = ServerEntityStore::default();
        let metadata = EntityMetadata::for_kind(EntityKind::Chicken).unwrap();

        let id =
            store.insert_passive_mob_for_test(EntityKind::Chicken, Vec3d::new(4.0, 64.0, 4.0), 0.0);
        let snapshot = store.state(id).unwrap().snapshot();

        assert_eq!(snapshot.kind, EntityKind::Chicken);
        assert_eq!(snapshot.width, metadata.dimensions.width);
        assert_eq!(snapshot.height, metadata.dimensions.height);
        assert_eq!(metadata.dimensions, EntityDimensions::scalable(0.4, 0.7));
        assert_eq!(metadata.standing_eye_height(), snapshot.height * 0.92);
        let mob = store.mob_state(id).expect("chicken mob state");
        assert_eq!(mob.movement_speed(), metadata.movement_speed);
        assert_eq!(mob.pathfinding_malus(BlockPathType::Water), 0.0);
        assert_eq!(mob.available_goal_count(), 3);
    }

    #[test]
    fn item_entity_can_snapshot_stack_metadata() {
        let mut store = ServerEntityStore::default();
        let metadata = EntityMetadata::for_kind(EntityKind::Item).unwrap();
        let stack = ItemStackSnapshot {
            kind: ItemKind::Egg,
            count: 1,
        };

        let id = store.insert_item_entity_for_test(stack, Vec3d::new(4.0, 64.0, 4.0));
        let snapshot = store.state(id).unwrap().snapshot();

        assert_eq!(snapshot.kind, EntityKind::Item);
        assert_eq!(snapshot.item_stack, Some(stack));
        assert_eq!(snapshot.width, metadata.dimensions.width);
        assert_eq!(snapshot.height, metadata.dimensions.height);
        assert!(store.item_state(id).is_some());
        assert!(store.mob_state(id).is_none());
    }

    #[test]
    fn entity_chunk_record_packs_and_hydrates_persistent_entities() {
        let mut store = ServerEntityStore::default();
        let chunk = ChunkPos::new(0, 0);
        let chicken_id = store.insert_passive_mob_for_test(
            EntityKind::Chicken,
            Vec3d::new(4.5, 64.0, 4.5),
            45.0,
        );
        let item_id = store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 2,
            },
            Vec3d::new(5.5, 64.0, 4.5),
        );
        let mallard_id = store.insert_passive_mob_for_test(
            EntityKind::Mallard,
            Vec3d::new(7.5, 64.0, 4.5),
            135.0,
        );
        let mallard_egg_id = store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::MallardEgg,
                count: 2,
            },
            Vec3d::new(8.5, 64.0, 4.5),
        );
        let mannequin_id = store.insert_passive_mob_for_test(
            EntityKind::Mannequin,
            Vec3d::new(6.5, 64.0, 4.5),
            90.0,
        );
        store.entities.get_mut(&chicken_id).unwrap().tick_count = 20;
        store.entities.get_mut(&item_id).unwrap().tick_count = 30;
        store.items.get_mut(&item_id).unwrap().set_age(30);
        store
            .mobs
            .get_mut(&chicken_id)
            .unwrap()
            .set_chicken_egg_time_for_test(1234);
        store
            .mobs
            .get_mut(&mallard_id)
            .unwrap()
            .set_mallard_egg_time_for_test(4321);
        store.set_item_pickup_delay_for_test(item_id, 3);
        store.set_item_pickup_delay_for_test(mallard_egg_id, 0);

        let record = store.entity_chunk_record(chunk, 7);

        assert_eq!(record.pos, chunk);
        assert_eq!(record.revision, 7);
        assert_eq!(record.entities.len(), 5);
        let chicken_record = record
            .entities
            .iter()
            .find(|entity| entity.kind == "minecraft:chicken")
            .expect("chicken record");
        assert_eq!(
            chicken_record.payload,
            EntitySavePayload::Chicken { egg_time: 1234 }
        );
        let item_record = record
            .entities
            .iter()
            .find(|entity| entity.kind == "minecraft:item")
            .expect("item record");
        assert_eq!(
            item_record.payload,
            EntitySavePayload::Item {
                stack: ItemStackSaveRecord::new("minecraft:egg", 2),
                age: 30,
                pickup_delay: 3,
            }
        );
        let mannequin_record = record
            .entities
            .iter()
            .find(|entity| entity.kind == "mclone:mannequin")
            .expect("mannequin record");
        assert_eq!(mannequin_record.payload, EntitySavePayload::Mannequin);
        let mallard_record = record
            .entities
            .iter()
            .find(|entity| entity.kind == "mclone:mallard")
            .expect("mallard record");
        assert!(matches!(
            mallard_record.payload,
            EntitySavePayload::Mallard {
                egg_time: 4321,
                age_ticks: crate::entity::mob::MALLARD_GROWTH_REQUIRED_TICKS,
                parents: [None, None],
                feather_time: 2_400..=4_799,
                call_time: 160..=479,
                ..
            }
        ));
        assert!(record.entities.iter().any(|entity| {
            entity.payload
                == EntitySavePayload::Item {
                    stack: ItemStackSaveRecord::new("mclone:mallard_egg", 2),
                    age: 0,
                    pickup_delay: 0,
                }
        }));

        let mut loaded = ServerEntityStore::default();
        loaded.next_entity_id = 100;
        let loaded_states = loaded.hydrate_entity_chunk_record(&record).unwrap();

        assert_eq!(loaded_states.len(), 5);
        let loaded_chicken = loaded_states
            .iter()
            .find(|entity| entity.kind == EntityKind::Chicken)
            .copied()
            .expect("loaded chicken");
        let loaded_item = loaded_states
            .iter()
            .find(|entity| entity.kind == EntityKind::Item)
            .copied()
            .expect("loaded item");
        let loaded_mannequin = loaded_states
            .iter()
            .find(|entity| entity.kind == EntityKind::Mannequin)
            .copied()
            .expect("loaded mannequin");
        assert_ne!(loaded_chicken.id, chicken_id);
        assert_ne!(loaded_item.id, item_id);
        assert_ne!(loaded_mannequin.id, mannequin_id);
        assert_eq!(loaded_chicken.persistent_id, chicken_record.persistent_id);
        assert_eq!(loaded_item.persistent_id, item_record.persistent_id);
        assert_eq!(
            loaded_mannequin.persistent_id,
            mannequin_record.persistent_id
        );
        assert_eq!(
            loaded
                .mob_state(loaded_mannequin.id)
                .expect("loaded mannequin mob")
                .available_goal_count(),
            3
        );
        assert_eq!(
            loaded
                .mobs
                .get(&loaded_chicken.id)
                .unwrap()
                .chicken_egg_time_for_test(),
            Some(1234)
        );
        assert_eq!(loaded.item_state(loaded_item.id).unwrap().pickup_delay(), 3);
        assert_eq!(loaded.item_state(loaded_item.id).unwrap().age(), 30);
        let loaded_mallard = loaded_states
            .iter()
            .find(|entity| entity.kind == EntityKind::Mallard)
            .expect("loaded mallard");
        assert_eq!(
            loaded
                .mobs
                .get(&loaded_mallard.id)
                .unwrap()
                .mallard_egg_time(),
            Some(4321)
        );
        assert!(loaded_states.iter().any(|entity| {
            entity.item_stack
                == Some(ItemStackSnapshot {
                    kind: ItemKind::MallardEgg,
                    count: 2,
                })
        }));
        assert_eq!(loaded_chicken.tick_count, 0);
        assert_eq!(loaded_item.tick_count, 0);
        assert_eq!(loaded_mannequin.tick_count, 0);

        let resaved = loaded.entity_chunk_record(chunk, 8);
        let resaved_chicken = resaved
            .entities
            .iter()
            .find(|entity| entity.kind == "minecraft:chicken")
            .expect("resaved chicken");
        assert_eq!(
            resaved_chicken.persistent_id, chicken_record.persistent_id,
            "persistent id must survive fresh runtime id assignment"
        );
    }

    #[test]
    fn rabbit_and_warren_identity_round_trip_together() {
        let mut store = ServerEntityStore::default();
        let rabbit_id =
            store.insert_passive_mob_for_test(EntityKind::Rabbit, Vec3d::new(4.5, 64.0, 4.5), 90.0);
        let rabbit_persistent_id = store.state(rabbit_id).unwrap().persistent_id;
        let updates = store.complete_rabbit_dig(rabbit_id, BlockPos::new(5, 64, 4));
        let burrow = updates
            .iter()
            .find(|entity| entity.kind == EntityKind::RabbitBurrow)
            .copied()
            .expect("completed dig creates a semantic burrow");
        assert_eq!(
            store.mobs[&rabbit_id]
                .rabbit_familiar_refuge()
                .map(|known| known.locator.persistent_id),
            Some(burrow.persistent_id)
        );

        let mut record = store.entity_chunk_record(ChunkPos::new(0, 0), 11);
        assert!(record.entities.iter().any(|entity| {
            matches!(
                entity.payload,
                EntitySavePayload::Rabbit {
                    known_refuges: [Some(RabbitRefugeSaveRecord {
                        persistent_id: home,
                        ..
                    }), None, None],
                    ..
                } if home == burrow.persistent_id
            )
        }));
        assert!(record.entities.iter().any(|entity| {
            matches!(
                entity.payload,
                EntitySavePayload::RabbitBurrow { capacity: 6, .. }
            )
        }));
        record.entities.reverse();

        let mut loaded = ServerEntityStore::default();
        let states = loaded.hydrate_entity_chunk_record(&record).unwrap();
        let loaded_rabbit = states
            .iter()
            .find(|entity| entity.kind == EntityKind::Rabbit)
            .expect("loaded rabbit");
        let loaded_burrow = states
            .iter()
            .find(|entity| entity.kind == EntityKind::RabbitBurrow)
            .expect("loaded burrow");
        assert_eq!(
            loaded.mobs[&loaded_rabbit.id]
                .rabbit_familiar_refuge()
                .map(|known| known.locator.persistent_id),
            Some(loaded_burrow.persistent_id)
        );
        assert_eq!(loaded_rabbit.persistent_id, rabbit_persistent_id);
    }

    #[test]
    fn visible_rabbit_pairs_separate_without_crossing_a_block_barrier() {
        let mut store = ServerEntityStore::default();
        let left =
            store.insert_passive_mob_for_test(EntityKind::Rabbit, Vec3d::new(1.21, 64.0, 0.5), 0.0);
        let right =
            store.insert_passive_mob_for_test(EntityKind::Rabbit, Vec3d::new(1.22, 64.0, 0.5), 0.0);
        let barrier = |pos: BlockPos| {
            Some(if pos.y == 63 || (pos.x == 0 && pos.y == 64) {
                BlockStateId(1)
            } else {
                BlockStateId(mclone_blocks::terrain_id::AIR)
            })
        };

        for _ in 0..12 {
            store.separate_visible_rabbits(&[left, right], &barrier);
        }
        let left_state = store.state(left).unwrap();
        let right_state = store.state(right).unwrap();
        assert!(
            left_state.position.x - f64::from(left_state.width) * 0.5 >= 1.0 - 1.0e-9,
            "the shared collision owner must keep a soft push outside the wall"
        );
        assert!(
            squared_distance_xz(left_state.position, right_state.position).sqrt() > 0.35,
            "overlapping rabbits must make useful separation progress"
        );

        let coincident_left =
            store.insert_passive_mob_for_test(EntityKind::Rabbit, Vec3d::new(4.5, 64.0, 4.5), 0.0);
        let coincident_right =
            store.insert_passive_mob_for_test(EntityKind::Rabbit, Vec3d::new(4.5, 64.0, 4.5), 0.0);
        for _ in 0..8 {
            store.separate_visible_rabbits(&[coincident_left, coincident_right], &flat_ground);
        }
        assert!(
            squared_distance_xz(
                store.state(coincident_left).unwrap().position,
                store.state(coincident_right).unwrap().position,
            )
            .sqrt()
                > 0.3,
            "stable identity fallback must separate exact coincident centers"
        );
    }

    #[test]
    fn burrow_disturbance_flushes_hidden_residents_then_collapse_preserves_them() {
        let mut source = ServerEntityStore::default();
        let rabbit_id = source.insert_passive_mob_for_test(
            EntityKind::Rabbit,
            Vec3d::new(4.5, 64.0, 4.5),
            90.0,
        );
        let rabbit_persistent_id = source.state(rabbit_id).unwrap().persistent_id;
        let created = source.complete_rabbit_dig(rabbit_id, BlockPos::new(5, 64, 4));
        let burrow = created
            .iter()
            .find(|entity| entity.kind == EntityKind::RabbitBurrow)
            .copied()
            .unwrap();
        let mut record = source.entity_chunk_record(ChunkPos::new(0, 0), 20);
        let rabbit_record = record
            .entities
            .iter_mut()
            .find(|entity| entity.persistent_id == rabbit_persistent_id)
            .unwrap();
        let EntitySavePayload::Rabbit {
            behavior,
            sheltered_in,
            ..
        } = &mut rabbit_record.payload
        else {
            panic!("rabbit payload expected");
        };
        *behavior = RabbitBehavior::Underground;
        *sheltered_in = Some(burrow.persistent_id);

        let mut store = ServerEntityStore::default();
        let loaded = store.hydrate_entity_chunk_record(&record).unwrap();
        let rabbit_id = loaded
            .iter()
            .find(|entity| entity.persistent_id == rabbit_persistent_id)
            .unwrap()
            .id;
        let burrow_id = loaded
            .iter()
            .find(|entity| entity.persistent_id == burrow.persistent_id)
            .unwrap()
            .id;
        assert!(store.state(rabbit_id).unwrap().hidden_from_clients);

        let disturbed = store.damage_habitat_prop(burrow_id).unwrap();
        assert_eq!(disturbed.outcome, HabitatPropDamageOutcome::Disturbed);
        assert!(store.state(burrow_id).is_some());
        assert!(!store.state(rabbit_id).unwrap().hidden_from_clients);
        assert_eq!(
            store.mobs[&rabbit_id]
                .rabbit_familiar_refuge()
                .map(|known| known.locator.persistent_id),
            Some(burrow.persistent_id)
        );
        assert_eq!(
            store.mobs[&rabbit_id].rabbit_behavior(),
            Some(RabbitBehavior::Emerge)
        );
        assert!(
            store
                .entity_chunk_record(ChunkPos::new(0, 0), 21)
                .entities
                .iter()
                .any(|entity| matches!(
                    entity.payload,
                    EntitySavePayload::RabbitBurrow {
                        disturbance_ticks: 40,
                        damage: 1,
                        ..
                    }
                ))
        );

        store.decay_rabbit_burrow_disturbance();
        assert_eq!(store.rabbit_burrows[&burrow_id].disturbance_ticks, 39);
        assert_eq!(
            store.damage_habitat_prop(burrow_id).unwrap().outcome,
            HabitatPropDamageOutcome::Disturbed
        );
        let collapsed = store.damage_habitat_prop(burrow_id).unwrap();
        assert_eq!(collapsed.outcome, HabitatPropDamageOutcome::Collapsed);
        assert!(store.state(burrow_id).is_none());
        let rabbit = store
            .state(rabbit_id)
            .expect("collapse must retain the resident");
        assert!(rabbit.alive);
        assert!(!rabbit.hidden_from_clients);
        assert_eq!(rabbit.persistent_id, rabbit_persistent_id);
        assert_eq!(store.mobs[&rabbit_id].rabbit_familiar_refuge(), None);
        let resaved = store.entity_chunk_record(ChunkPos::new(0, 0), 22);
        assert!(
            !resaved
                .entities
                .iter()
                .any(|entity| entity.kind == "mclone:rabbit_burrow")
        );
        assert!(resaved.entities.iter().any(|entity| {
            entity.persistent_id == rabbit_persistent_id
                && matches!(
                    entity.payload,
                    EntitySavePayload::Rabbit {
                        known_refuges: [None, None, None],
                        ..
                    }
                )
        }));
    }

    #[test]
    fn current_refuge_occupancy_does_not_own_foraging_relatives() {
        let mut store = ServerEntityStore::default();
        let founder_id =
            store.insert_passive_mob_for_test(EntityKind::Rabbit, Vec3d::new(4.5, 64.0, 4.5), 0.0);
        let founder_persistent_id = store.state(founder_id).unwrap().persistent_id;
        let burrow = store
            .complete_rabbit_dig(founder_id, BlockPos::new(5, 64, 4))
            .into_iter()
            .find(|entity| entity.kind == EntityKind::RabbitBurrow)
            .unwrap();
        let offspring_id = store.allocate_entity_id();
        let offspring_persistent_id = store.allocate_persistent_id();
        store.insert_rabbit_with_runtime(
            offspring_id,
            offspring_persistent_id,
            Vec3d::new(4.7, 64.0, 4.5),
            0.0,
            RabbitRuntimeSaveData {
                parents: [
                    Some(founder_persistent_id),
                    Some(EntityPersistentId::new(0, 99)),
                ],
                ..adult_rabbit_with_refuge(burrow.persistent_id, burrow.position, true)
            },
        );
        let burrow_id = burrow.id;
        store.rabbit_burrows.get_mut(&burrow_id).unwrap().capacity = 2;

        let updates = store.maintain_rabbit_refuges();

        assert!(updates.is_empty());
        assert_eq!(
            store.mobs[&founder_id]
                .rabbit_familiar_refuge()
                .map(|known| known.locator.persistent_id),
            Some(burrow.persistent_id),
            "a forager remembers the refuge without occupying it"
        );
        assert_eq!(
            store.mobs[&offspring_id].rabbit_sheltered_in(),
            Some(burrow.persistent_id)
        );
        assert_eq!(
            store.mobs[&offspring_id]
                .rabbit_save_data()
                .unwrap()
                .parents[0],
            Some(founder_persistent_id)
        );
    }

    #[test]
    fn serialized_refuge_selection_does_not_overbook_capacity() {
        let mut store = ServerEntityStore::default();
        let founder =
            store.insert_passive_mob_for_test(EntityKind::Rabbit, Vec3d::new(4.5, 64.0, 4.5), 0.0);
        let burrow = store
            .complete_rabbit_dig(founder, BlockPos::new(5, 64, 4))
            .into_iter()
            .find(|entity| entity.kind == EntityKind::RabbitBurrow)
            .unwrap();
        store.rabbit_burrows.get_mut(&burrow.id).unwrap().capacity = 1;
        store.remove_entity(founder);
        let left =
            store.insert_passive_mob_for_test(EntityKind::Rabbit, Vec3d::new(3.5, 64.0, 4.5), 0.0);
        let right =
            store.insert_passive_mob_for_test(EntityKind::Rabbit, Vec3d::new(3.5, 64.0, 5.5), 0.0);

        store.tick_stationary_at_time(&[ChunkPos::new(0, 0)], &[], 6_000, flat_ground);

        let claims = [left, right]
            .into_iter()
            .filter(|id| store.mobs[id].rabbit_refuge_claim() == Some(burrow.persistent_id))
            .count();
        assert_eq!(claims, 1);
    }

    #[test]
    fn thousand_rabbits_bound_idle_work_and_make_fair_progress() {
        let mut store = ServerEntityStore::default();
        for index in 0..1_000 {
            let offset = f64::from(index % 20) * 0.02;
            store.insert_passive_mob_for_test(
                EntityKind::Rabbit,
                Vec3d::new(8.0 + offset, 64.0, 8.0 + offset),
                0.0,
            );
        }

        for tick in 0..20 {
            store.tick_stationary_at_time(&[ChunkPos::new(0, 0)], &[], 12_000 + tick, flat_ground);
            let diagnostics = store.rabbit_ecology_diagnostics();
            assert_eq!(diagnostics.active, 1_000);
            assert!(diagnostics.work.admitted[EcologyWorkClass::Decision as usize] <= 64);
            assert!(diagnostics.work.admitted[EcologyWorkClass::HabitatQuery as usize] <= 32);
            assert!(diagnostics.work.admitted[EcologyWorkClass::PathRequest as usize] <= 32);
            assert!(diagnostics.neighbor_candidates <= 18_000);
        }

        assert!(store.mobs.values().all(|mob| {
            mob.rabbit_save_data()
                .is_some_and(|rabbit| rabbit.decision_schedule.attempt_generation > 0)
        }));
    }

    #[test]
    fn thousand_threatened_rabbits_bound_and_fairly_receive_escape_paths() {
        let mut store = ServerEntityStore::default();
        for index in 0..1_000 {
            let offset = f64::from(index % 20) * 0.02;
            store.insert_passive_mob_for_test(
                EntityKind::Rabbit,
                Vec3d::new(8.0 + offset, 64.0, 8.0 + offset),
                0.0,
            );
        }
        let player = MobPlayerTarget::from_position(Vec3d::new(8.0, 64.0, 8.0));
        for tick in 0..40 {
            store.tick_stationary_at_time(
                &[ChunkPos::new(0, 0)],
                &[player],
                12_000 + tick,
                no_blocks,
            );
            let diagnostics = store.rabbit_ecology_diagnostics();
            assert_eq!(diagnostics.active, 1_000);
            assert!(diagnostics.work.admitted[EcologyWorkClass::PathRequest as usize] <= 32);
        }

        assert!(store.mobs.values().all(|mob| {
            mob.rabbit_escape_attempts_for_test()
                .is_some_and(|attempts| attempts > 0)
        }));
    }

    #[test]
    fn ticking_domain_boundary_is_unavailable_to_rabbit_paths() {
        let mut store = ServerEntityStore::default();
        let rabbit =
            store.insert_passive_mob_for_test(EntityKind::Rabbit, Vec3d::new(14.5, 64.0, 8.5), 0.0);
        let player = MobPlayerTarget::from_position(Vec3d::new(12.5, 64.0, 8.5));
        let ticking = [ChunkPos::new(0, 0)];

        for tick in 0..240 {
            store.tick_stationary_at_time(&ticking, &[player], 12_000 + tick, flat_ground);
            assert_eq!(
                store.state(rabbit).unwrap().chunk_pos(),
                ChunkPos::new(0, 0),
                "an unavailable non-ticking chunk must never become a path or movement target"
            );
        }

        assert_eq!(
            store.mobs[&rabbit].rabbit_behavior(),
            Some(RabbitBehavior::Flee)
        );
    }

    #[test]
    fn ticking_domain_boundary_rejects_deer_escape_steps() {
        let mut store = ServerEntityStore::default();
        let deer =
            store.insert_passive_mob_for_test(EntityKind::Deer, Vec3d::new(15.7, 64.0, 8.5), 0.0);
        let player = MobPlayerTarget::from_position(Vec3d::new(12.5, 64.0, 8.5));
        let ticking = [ChunkPos::new(0, 0)];

        for tick in 0..240 {
            store.tick_stationary_at_time(&ticking, &[player], 12_000 + tick, flat_ground);
            assert_eq!(
                store.state(deer).unwrap().chunk_pos(),
                ChunkPos::new(0, 0),
                "unavailable chunks must reject physical escape steps as well as paths"
            );
        }
    }

    #[test]
    fn saved_underground_rabbit_hydrates_hidden_without_losing_identity() {
        let mut store = ServerEntityStore::default();
        let rabbit_id =
            store.insert_passive_mob_for_test(EntityKind::Rabbit, Vec3d::new(4.5, 64.0, 4.5), 90.0);
        let persistent_id = store.state(rabbit_id).unwrap().persistent_id;
        let mut record = store.entity_chunk_record(ChunkPos::new(0, 0), 12);
        let saved_rabbit = record
            .entities
            .iter_mut()
            .find(|entity| entity.persistent_id == persistent_id)
            .expect("saved rabbit");
        let EntitySavePayload::Rabbit { behavior, .. } = &mut saved_rabbit.payload else {
            panic!("rabbit must use rabbit persistence payload");
        };
        *behavior = RabbitBehavior::Underground;

        let mut loaded = ServerEntityStore::default();
        let states = loaded.hydrate_entity_chunk_record(&record).unwrap();
        let rabbit = states
            .iter()
            .find(|entity| entity.persistent_id == persistent_id)
            .expect("underground rabbit remains a durable loaded entity");

        assert!(rabbit.alive);
        assert!(rabbit.hidden_from_clients);
        assert!(!rabbit.client_visible());
        assert_eq!(rabbit.width, 0.001);
        assert_eq!(
            loaded.mobs[&rabbit.id].rabbit_behavior(),
            Some(RabbitBehavior::Underground)
        );
        assert!(
            loaded
                .entity_chunk_record(ChunkPos::new(0, 0), 13)
                .entities
                .iter()
                .any(|entity| matches!(
                    entity.payload,
                    EntitySavePayload::Rabbit {
                        behavior: RabbitBehavior::Underground,
                        ..
                    }
                ) && entity.persistent_id == persistent_id)
        );
    }

    #[test]
    fn covered_wetland_nest_pauses_resumes_hatches_once_and_roundtrips() {
        let mut store = ServerEntityStore::default();
        let ticking_chunks = local_ticking_chunks();
        let nest_position = Vec3d::new(4.5, 64.0, 4.5);
        assert!(
            store
                .place_mallard_nest(nest_position, 0.0, dry_grass_ground)
                .is_none()
        );
        let nest = store
            .place_mallard_nest(nest_position, 0.0, covered_wetland_ground)
            .expect("covered wetland shore nest");
        store.insert_passive_mob_for_test(EntityKind::Mallard, Vec3d::new(3.5, 64.0, 4.5), 0.0);
        store.insert_passive_mob_for_test(EntityKind::Mallard, Vec3d::new(4.5, 64.0, 3.5), 0.0);
        store
            .mallard_nests
            .get_mut(&nest.id)
            .unwrap()
            .incubation_progress = MALLARD_NEST_INCUBATION_REQUIRED_TICKS - 1;

        store.tick_stationary(&ticking_chunks, &[], dry_grass_ground);
        let paused = store.state(nest.id).unwrap();
        assert_eq!(
            paused.mallard_nest.unwrap().incubation_progress,
            MALLARD_NEST_INCUBATION_REQUIRED_TICKS - 1
        );
        assert!(!paused.mallard_nest.unwrap().attended);

        let record = store.entity_chunk_record(ChunkPos::new(0, 0), 7);
        let saved_nest = record
            .entities
            .iter()
            .find(|entity| entity.kind == "mclone:mallard_nest")
            .expect("saved nest");
        assert_eq!(
            saved_nest.payload,
            EntitySavePayload::MallardNest {
                incubation_progress: MALLARD_NEST_INCUBATION_REQUIRED_TICKS - 1,
                incubation_required: MALLARD_NEST_INCUBATION_REQUIRED_TICKS,
                parents: [None; 2],
            }
        );

        let mut loaded = ServerEntityStore::default();
        loaded.hydrate_entity_chunk_record(&record).unwrap();
        let updates = loaded.tick_stationary(&ticking_chunks, &[], covered_wetland_ground);
        assert!(updates.iter().any(|entity| {
            entity.kind == EntityKind::Mallard
                && entity
                    .mallard
                    .is_some_and(|mallard| mallard.life_stage == MallardLifeStage::Duckling)
        }));
        assert!(
            !loaded
                .states()
                .iter()
                .any(|entity| entity.kind == EntityKind::MallardNest)
        );
        let duckling = loaded
            .wildlife_life_diagnostics()
            .into_iter()
            .find(|animal| animal.mallard_life_stage == Some(MallardLifeStage::Duckling))
            .expect("hatch creates a shared-lifecycle duckling");
        assert_eq!(duckling.lifecycle.age_ticks, 0);
        assert!(duckling.parents.iter().all(Option::is_some));
        assert!(loaded.drain_wildlife_events().iter().any(|event| matches!(
            event.kind,
            WildlifeEcologyEventKind::Birth { child, .. }
                if child == duckling.persistent_id
        )));

        let second = loaded.tick_stationary(&ticking_chunks, &[], covered_wetland_ground);
        assert_eq!(
            second
                .iter()
                .filter(|entity| {
                    entity.kind == EntityKind::Mallard
                        && entity
                            .mallard
                            .is_some_and(|mallard| mallard.life_stage == MallardLifeStage::Duckling)
                })
                .count(),
            1
        );
        assert_eq!(
            loaded
                .states()
                .iter()
                .filter(|entity| {
                    entity.kind == EntityKind::Mallard
                        && entity
                            .mallard
                            .is_some_and(|mallard| mallard.life_stage == MallardLifeStage::Duckling)
                })
                .count(),
            1
        );
    }

    #[test]
    fn sustained_deer_bedding_creates_one_durable_sign() {
        let mut store = ServerEntityStore::default();
        let deer =
            store.insert_passive_mob_for_test(EntityKind::Deer, Vec3d::new(4.5, 64.0, 4.5), 30.0);
        store
            .mobs
            .get_mut(&deer)
            .unwrap()
            .set_deer_behavior_for_test(mclone_protocol::DeerBehavior::Bedded);

        let mut updates = Vec::new();
        for _ in 0..80 {
            updates = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);
        }
        let bed = updates
            .iter()
            .find(|entity| entity.kind == EntityKind::DeerBed && entity.alive)
            .expect("sustained bedding should leave sign");
        assert_eq!(bed.position, Vec3d::new(4.5, 64.0, 4.5));

        for _ in 0..240 {
            store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);
        }
        assert_eq!(
            store
                .states()
                .iter()
                .filter(|entity| entity.kind == EntityKind::DeerBed && entity.alive)
                .count(),
            1
        );
        let record = store.entity_chunk_record(ChunkPos::new(0, 0), 9);
        assert!(record.entities.iter().any(|record| {
            record.kind == "mclone:deer_bed"
                && matches!(record.payload, EntitySavePayload::DeerBed { .. })
        }));
        let mut loaded = ServerEntityStore::default();
        loaded.hydrate_entity_chunk_record(&record).unwrap();
        assert_eq!(
            loaded
                .states()
                .iter()
                .filter(|entity| entity.kind == EntityKind::DeerBed && entity.alive)
                .count(),
            1
        );
    }

    #[test]
    fn adult_antlers_shed_once_and_persist_as_an_item() {
        let mut store = ServerEntityStore::default();
        let deer =
            store.insert_passive_mob_for_test(EntityKind::Deer, Vec3d::new(4.5, 64.0, 4.5), 0.0);
        store
            .mobs
            .get_mut(&deer)
            .unwrap()
            .make_deer_antler_shed_due_for_test();

        let first = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);
        assert!(first.iter().any(|entity| {
            entity.item_stack
                == Some(ItemStackSnapshot {
                    kind: ItemKind::ShedAntler,
                    count: 1,
                })
        }));
        assert_eq!(store.state(deer).unwrap().deer.unwrap().antlered, false);
        store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);
        assert_eq!(
            store
                .states()
                .iter()
                .filter(|entity| {
                    entity
                        .item_stack
                        .is_some_and(|stack| stack.kind == ItemKind::ShedAntler)
                })
                .count(),
            1
        );

        let record = store.entity_chunk_record(ChunkPos::new(0, 0), 10);
        let deer_payload = &record
            .entities
            .iter()
            .find(|record| record.kind == "mclone:deer")
            .unwrap()
            .payload;
        assert!(matches!(
            deer_payload,
            EntitySavePayload::Deer {
                antlered: false,
                antler_shed_time: -1,
                ..
            }
        ));
        let mut loaded = ServerEntityStore::default();
        loaded.hydrate_entity_chunk_record(&record).unwrap();
        loaded.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);
        assert_eq!(
            loaded
                .states()
                .iter()
                .filter(|entity| {
                    entity
                        .item_stack
                        .is_some_and(|stack| stack.kind == ItemKind::ShedAntler)
                })
                .count(),
            1,
            "hydration must not schedule a second antler drop"
        );
    }

    #[test]
    fn nearby_deer_share_one_alarm_sound_without_repeating_it() {
        let mut store = ServerEntityStore::default();
        store.insert_passive_mob_for_test(EntityKind::Deer, Vec3d::new(0.5, 64.0, 0.5), 0.0);
        store.insert_passive_mob_for_test(EntityKind::Deer, Vec3d::new(2.5, 64.0, 0.5), 0.0);
        let player = MobPlayerTarget::from_position(Vec3d::new(12.5, 64.0, 0.5));

        store.tick_stationary(&[ChunkPos::new(0, 0)], &[player], flat_ground);
        let sounds = store.drain_deer_sounds();
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].kind, DeerSoundKind::Alarm);

        store.tick_stationary(&[ChunkPos::new(0, 0)], &[player], flat_ground);
        assert!(store.drain_deer_sounds().is_empty());
    }

    #[test]
    fn mallards_retain_destinations_and_move_without_stationary_yaw_jitter() {
        let mut store = ServerEntityStore::default();
        let first =
            store.insert_passive_mob_for_test(EntityKind::Mallard, Vec3d::new(5.5, 64.2, 4.5), 0.0);
        let second =
            store.insert_passive_mob_for_test(EntityKind::Mallard, Vec3d::new(7.5, 64.2, 4.5), 0.0);
        let ticking_chunks = (-1..=1)
            .flat_map(|x| (-1..=1).map(move |z| ChunkPos::new(x, z)))
            .collect::<Vec<_>>();
        let starts = [
            store.state(first).unwrap().position,
            store.state(second).unwrap().position,
        ];

        store.tick_stationary(&ticking_chunks, &[], broad_shallow_water);
        let (first_target, first_remaining) = store
            .mob_state(first)
            .and_then(MobRuntimeState::mallard_habitat_intent_for_test)
            .expect("first mallard retained habitat intent");
        assert!(squared_distance_xz(starts[0], first_target) >= 2.5 * 2.5);
        store.tick_stationary(&ticking_chunks, &[], broad_shallow_water);
        let (second_target, second_remaining) = store
            .mob_state(first)
            .and_then(MobRuntimeState::mallard_habitat_intent_for_test)
            .expect("first mallard retained habitat intent on second tick");
        assert_eq!(second_target, first_target);
        assert_eq!(second_remaining + 1, first_remaining);

        let mut previous = [store.state(first).unwrap(), store.state(second).unwrap()];
        let mut traveled = [0.0_f64; 2];
        let mut water_seen = [false; 2];
        let mut shore_seen = [false; 2];
        let mut stationary_yaw_changes = 0_u32;
        let mut healthy_spacing_ticks = 0_u32;
        let mut collapsed_spacing_ticks = 0_u32;
        let mut max_spacing = 0.0_f64;
        let mut turn_reversals = [0_u32; 2];
        let mut previous_turn_sign = [0_i8; 2];
        for _ in 0..1_200 {
            store.tick_stationary(&ticking_chunks, &[], broad_shallow_water);
            let current = [store.state(first).unwrap(), store.state(second).unwrap()];
            for index in 0..2 {
                let distance_sqr =
                    squared_distance_xz(previous[index].position, current[index].position);
                traveled[index] += distance_sqr.sqrt();
                let yaw_delta = wrapped_degrees_delta(
                    previous[index].y_rot_degrees,
                    current[index].y_rot_degrees,
                );
                if distance_sqr <= 1.0e-10 && yaw_delta.abs() > 1.0e-4 {
                    stationary_yaw_changes += 1;
                }
                assert!(
                    yaw_delta.abs() <= 12.001,
                    "mallard turn exceeded bound: {yaw_delta}"
                );
                let sign = if yaw_delta > 1.0 {
                    1
                } else if yaw_delta < -1.0 {
                    -1
                } else {
                    0
                };
                if sign != 0 && previous_turn_sign[index] != 0 && sign != previous_turn_sign[index]
                {
                    turn_reversals[index] += 1;
                }
                if sign != 0 {
                    previous_turn_sign[index] = sign;
                }
                let in_water = current[index]
                    .mallard
                    .is_some_and(|mallard| mallard.in_water);
                water_seen[index] |= in_water;
                shore_seen[index] |= !in_water && current[index].on_ground;
            }
            let spacing = squared_distance_xz(current[0].position, current[1].position).sqrt();
            healthy_spacing_ticks += u32::from((1.0..=12.0).contains(&spacing));
            collapsed_spacing_ticks += u32::from(spacing < 0.8);
            max_spacing = max_spacing.max(spacing);
            previous = current;
        }

        assert_eq!(stationary_yaw_changes, 0);
        assert!(
            traveled.iter().all(|distance| *distance > 12.0),
            "traveled={traveled:?}"
        );
        assert!(
            water_seen.iter().all(|seen| *seen),
            "water_seen={water_seen:?}"
        );
        assert!(
            shore_seen.iter().all(|seen| *seen),
            "shore_seen={shore_seen:?} states={:?} intents={:?}",
            previous,
            [
                store
                    .mob_state(first)
                    .and_then(MobRuntimeState::mallard_habitat_intent_for_test),
                store
                    .mob_state(second)
                    .and_then(MobRuntimeState::mallard_habitat_intent_for_test),
            ]
        );
        assert!(
            healthy_spacing_ticks > 780,
            "healthy_spacing_ticks={healthy_spacing_ticks}"
        );
        assert!(
            collapsed_spacing_ticks < 20,
            "collapsed_spacing_ticks={collapsed_spacing_ticks}"
        );
        assert!(max_spacing < 20.0, "max_spacing={max_spacing}");
        assert!(
            turn_reversals.iter().all(|count| *count < 80),
            "turn_reversals={turn_reversals:?}"
        );
    }

    fn wrapped_degrees_delta(previous: f32, current: f32) -> f32 {
        let mut delta = (current - previous) % 360.0;
        if delta >= 180.0 {
            delta -= 360.0;
        }
        if delta < -180.0 {
            delta += 360.0;
        }
        delta
    }

    #[test]
    fn mallard_calls_are_flock_suppressed_and_feathers_are_collectible_entities() {
        let mut store = ServerEntityStore::default();
        let ticking_chunks = local_ticking_chunks();
        let first =
            store.insert_passive_mob_for_test(EntityKind::Mallard, Vec3d::new(4.5, 64.0, 4.5), 0.0);
        let second =
            store.insert_passive_mob_for_test(EntityKind::Mallard, Vec3d::new(5.5, 64.0, 4.5), 0.0);
        store.set_mallard_trace_times_for_test(first, 0, 0);
        store.set_mallard_trace_times_for_test(second, 0, 0);

        store.tick_stationary(&ticking_chunks, &[], covered_wetland_ground);

        let calls = store.drain_mallard_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].audible_radius, MALLARD_CALL_AUDIBLE_RADIUS);
        for _ in 0..400 {
            if store
                .states()
                .iter()
                .filter(|entity| {
                    entity.item_stack
                        == Some(ItemStackSnapshot {
                            kind: ItemKind::MallardFeather,
                            count: 1,
                        })
                })
                .count()
                == 2
            {
                break;
            }
            store.tick_stationary(&ticking_chunks, &[], covered_wetland_ground);
            store.drain_mallard_calls();
        }
        let feathers = store
            .states()
            .into_iter()
            .filter(|entity| {
                entity.item_stack
                    == Some(ItemStackSnapshot {
                        kind: ItemKind::MallardFeather,
                        count: 1,
                    })
            })
            .collect::<Vec<_>>();
        assert_eq!(feathers.len(), 2);
        assert!(store.drain_mallard_calls().is_empty());
    }

    #[test]
    fn empty_entity_chunk_record_removes_persistent_entities_in_chunk() {
        let mut store = ServerEntityStore::default();
        let chunk = ChunkPos::new(0, 0);
        let id =
            store.insert_passive_mob_for_test(EntityKind::Cow, Vec3d::new(4.5, 64.0, 4.5), 0.0);

        let loaded = store
            .hydrate_entity_chunk_record(&EntityChunkRecord::empty(chunk, 2))
            .unwrap();

        assert!(loaded.is_empty());
        assert_eq!(store.state(id), None);
    }

    #[test]
    fn item_entity_ticks_only_in_entity_ticking_chunks() {
        let mut store = ServerEntityStore::default();
        let id = store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            },
            Vec3d::new(4.0, 64.0, 4.0),
        );
        let start = store.state(id).unwrap();

        assert!(
            store
                .tick_stationary(&[ChunkPos::new(1, 0)], &[], no_blocks)
                .is_empty()
        );
        assert_eq!(store.state(id).unwrap(), start);

        let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], no_blocks);
        let item = updated
            .iter()
            .find(|entity| entity.id == id)
            .expect("item should tick in entity ticking chunk");

        assert_ne!(item.position, start.position);
        assert_eq!(item.tick_count, 1);
        assert_eq!(store.item_state(id).unwrap().pickup_delay(), 9);
    }

    #[test]
    fn item_entity_expires_after_java_lifetime() {
        let mut store = ServerEntityStore::default();
        let id = store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            },
            Vec3d::new(4.0, 64.0, 4.0),
        );
        store
            .items
            .get_mut(&id)
            .unwrap()
            .set_age(ITEM_ENTITY_LIFETIME_TICKS - 1);

        let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], no_blocks);
        let removed = updated
            .iter()
            .find(|entity| entity.id == id)
            .expect("expired item should emit final update");

        assert!(!removed.alive);
        assert_eq!(store.state(id), None);
        assert_eq!(store.item_state(id), None);
    }

    #[test]
    fn item_entity_pickup_waits_for_pickup_delay() {
        let mut store = ServerEntityStore::default();
        let id = store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            },
            Vec3d::new(4.0, 64.0, 4.0),
        );
        let target = ItemPickupTarget {
            player_id: ServerPlayerId::from_raw_for_tests(0),
            position: Vec3d::new(4.0, 64.0, 4.0),
        };

        let blocked = store.collect_item_entities(&[target], |_player_id, _stack| None);
        assert!(blocked.is_empty());
        assert!(store.state(id).unwrap().alive);

        store
            .items
            .get_mut(&id)
            .unwrap()
            .set_pickup_delay_for_test(0);
        let mut collected = 0;
        let picked_up = store.collect_item_entities(&[target], |_player_id, stack| {
            collected += stack.count;
            None
        });

        assert_eq!(collected, 1);
        assert_eq!(picked_up.len(), 1);
        assert_eq!(picked_up[0].id, id);
        assert!(!picked_up[0].alive);
        assert_eq!(store.state(id), None);
        assert_eq!(store.item_state(id), None);
    }

    #[test]
    fn item_entity_pickup_keeps_remaining_stack_after_partial_acceptance() {
        let mut store = ServerEntityStore::default();
        let id = store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 4,
            },
            Vec3d::new(4.0, 64.0, 4.0),
        );
        store
            .items
            .get_mut(&id)
            .unwrap()
            .set_pickup_delay_for_test(0);
        let target = ItemPickupTarget {
            player_id: ServerPlayerId::from_raw_for_tests(0),
            position: Vec3d::new(4.0, 64.0, 4.0),
        };

        let updated = store.collect_item_entities(&[target], |_player_id, stack| {
            Some(ItemStackSnapshot {
                count: stack.count - 2,
                ..stack
            })
        });

        assert_eq!(updated.len(), 1);
        assert!(updated[0].alive);
        assert_eq!(
            updated[0].item_stack,
            Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 2,
            })
        );
        assert_eq!(store.state(id), Some(updated[0]));
        assert_eq!(
            store.item_state(id).unwrap().stack(),
            updated[0].item_stack.unwrap()
        );
    }

    #[test]
    fn item_entities_merge_nearby_egg_stacks() {
        let mut store = ServerEntityStore::default();
        let first = store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            },
            Vec3d::new(4.0, 64.0, 4.0),
        );
        let second = store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            },
            Vec3d::new(4.1, 64.0, 4.0),
        );
        store.entities.get_mut(&first).unwrap().tick_count =
            ITEM_STATIONARY_MERGE_INTERVAL_TICKS - 1;
        store.entities.get_mut(&second).unwrap().tick_count =
            ITEM_STATIONARY_MERGE_INTERVAL_TICKS - 1;
        store.items.get_mut(&first).unwrap().set_age(10);
        store.items.get_mut(&second).unwrap().set_age(20);

        let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], no_blocks);
        let surviving_items = store
            .states()
            .into_iter()
            .filter(|entity| entity.kind == EntityKind::Item)
            .collect::<Vec<_>>();

        assert_eq!(surviving_items.len(), 1);
        assert_eq!(
            surviving_items[0].item_stack,
            Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 2,
            })
        );
        assert!(updated.iter().any(|entity| {
            (entity.id == first || entity.id == second)
                && !entity.alive
                && entity.kind == EntityKind::Item
        }));
        assert!(updated.iter().any(|entity| {
            entity.id == surviving_items[0].id
                && entity.alive
                && entity.item_stack == surviving_items[0].item_stack
        }));
        assert_eq!(
            store.item_state(surviving_items[0].id).unwrap().age(),
            11,
            "merged item keeps the younger semantic age after both items tick"
        );
    }

    #[test]
    fn chicken_egg_timer_spawns_egg_item_entity() {
        let mut store = ServerEntityStore::default();
        let chicken_id =
            store.insert_passive_mob_for_test(EntityKind::Chicken, Vec3d::new(4.0, 64.0, 4.0), 0.0);
        store
            .mobs
            .get_mut(&chicken_id)
            .unwrap()
            .set_chicken_egg_time_for_test(1);

        let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);
        let egg = updated
            .iter()
            .find(|entity| entity.kind == EntityKind::Item)
            .expect("chicken should spawn egg item");

        assert_eq!(
            egg.item_stack,
            Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            })
        );
        assert_eq!(egg.width, 0.25);
        assert_eq!(egg.height, 0.25);
        assert_eq!(egg.position.x, store.state(chicken_id).unwrap().position.x);
        assert_eq!(egg.position.z, store.state(chicken_id).unwrap().position.z);
        assert_eq!(
            store
                .mob_state(chicken_id)
                .unwrap()
                .chicken_pending_egg_lays_for_test(),
            Some(0)
        );
    }

    #[test]
    fn bee_colony_forages_returns_persists_and_produces_bounded_work() {
        let mut store = ServerEntityStore::default();
        let colony_position = Vec3d::new(0.5, 64.0, 0.5);
        let spawned = store
            .spawn_persistent_bee_colony(
                EntityKind::BeeNest,
                colony_position,
                0.0,
                &[Vec3d::new(0.5, 65.0, 1.5)],
            )
            .unwrap();
        let bee_id = spawned
            .iter()
            .find(|entity| entity.kind == EntityKind::Bee)
            .unwrap()
            .id;
        let world = |pos: BlockPos| {
            Some(generated_block_state_id(if pos.y <= 62 {
                mclone_worldgen::block::DIRT
            } else if pos.y == 63 {
                mclone_worldgen::block::GRASS_BLOCK
            } else if pos == BlockPos::new(4, 64, 0) {
                mclone_worldgen::block::DANDELION
            } else {
                mclone_worldgen::block::AIR
            }))
        };
        let start = store.state(bee_id).unwrap().position;
        let mut pollination = None;
        let active_chunks = (-1..=1)
            .flat_map(|x| (-1..=1).map(move |z| ChunkPos::new(x, z)))
            .collect::<Vec<_>>();
        for _ in 0..700 {
            store.tick_stationary(&active_chunks, &[], world);
            if let Some(event) = store.drain_bee_pollinations().into_iter().next() {
                pollination = Some(event);
                break;
            }
        }
        let event = pollination.unwrap_or_else(|| {
            panic!(
                "bee should complete a flower-to-colony trip: state={:?} runtime={:?}",
                store.state(bee_id),
                store
                    .mobs
                    .get(&bee_id)
                    .and_then(MobRuntimeState::bee_save_data)
            )
        });
        assert_eq!(event.source_flower, BlockPos::new(4, 64, 0));
        assert!(store.state(bee_id).unwrap().position.distance_to_sqr(start) > 1.0);
        let records = [ChunkPos::new(0, 0)]
            .into_iter()
            .flat_map(|pos| store.entity_chunk_record(pos, 1).entities)
            .collect::<Vec<_>>();
        assert!(records.iter().any(|record| matches!(
            record.payload,
            EntitySavePayload::Bee { home, .. } if home == event.colony
        )));
        assert!(records.iter().any(|record| matches!(
            record.payload,
            EntitySavePayload::BeeColony { stored_work: 1, .. }
        )));

        let mut hydrated = ServerEntityStore::default();
        hydrated
            .hydrate_entity_chunk_record(&EntityChunkRecord::new(ChunkPos::new(0, 0), 1, records))
            .unwrap();
        assert_eq!(hydrated.bee_colonies.len(), 1);
        assert_eq!(hydrated.mobs.len(), 1);
    }

    #[test]
    fn bee_colony_members_distribute_across_real_flower_distance_bands() {
        let mut store = ServerEntityStore::default();
        let colony_position = Vec3d::new(0.5, 64.0, 0.5);
        let spawned = store
            .spawn_persistent_bee_colony(
                EntityKind::BeeNest,
                colony_position,
                0.0,
                &[
                    Vec3d::new(0.5, 65.0, 1.5),
                    Vec3d::new(1.5, 65.0, 0.5),
                    Vec3d::new(0.5, 66.0, -0.5),
                ],
            )
            .unwrap();
        let bee_ids = spawned
            .iter()
            .filter(|entity| entity.kind == EntityKind::Bee)
            .map(|entity| entity.id)
            .collect::<Vec<_>>();
        let world = |pos: BlockPos| {
            Some(generated_block_state_id(if pos.y <= 62 {
                mclone_worldgen::block::DIRT
            } else if pos.y == 63 {
                mclone_worldgen::block::GRASS_BLOCK
            } else if pos.y == 64 && matches!(pos.x, 4 | 10 | 18) && pos.z == 0 {
                mclone_worldgen::block::DANDELION
            } else {
                mclone_worldgen::block::AIR
            }))
        };
        let active_chunks = (-2..=2)
            .flat_map(|x| (-2..=2).map(move |z| ChunkPos::new(x, z)))
            .collect::<Vec<_>>();
        for _ in 0..80 {
            store.tick_stationary(&active_chunks, &[], world);
        }

        let chosen_x = bee_ids
            .iter()
            .map(|id| {
                store
                    .mobs
                    .get(id)
                    .and_then(MobRuntimeState::bee_save_data)
                    .and_then(|bee| bee.flower)
                    .expect("each colony member should select a real flower")
                    .x
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(chosen_x, BTreeSet::from([4, 10, 18]));
    }

    #[test]
    fn bee_flight_recovers_over_a_blocked_direct_route() {
        let mut store = ServerEntityStore::default();
        let spawned = store
            .spawn_persistent_bee_colony(
                EntityKind::BeeNest,
                Vec3d::new(0.5, 64.0, 0.5),
                0.0,
                &[Vec3d::new(0.5, 65.0, 0.5)],
            )
            .unwrap();
        let bee_id = spawned
            .iter()
            .find(|entity| entity.kind == EntityKind::Bee)
            .unwrap()
            .id;
        let world = |pos: BlockPos| {
            Some(generated_block_state_id(if pos.y <= 62 {
                mclone_worldgen::block::DIRT
            } else if pos.y == 63 {
                mclone_worldgen::block::GRASS_BLOCK
            } else if pos.x == 2 && (64..=68).contains(&pos.y) {
                mclone_worldgen::block::OAK_LOG
            } else if pos == BlockPos::new(8, 64, 0) {
                mclone_worldgen::block::DANDELION
            } else {
                mclone_worldgen::block::AIR
            }))
        };
        let active_chunks = (-2..=2)
            .flat_map(|x| (-2..=2).map(move |z| ChunkPos::new(x, z)))
            .collect::<Vec<_>>();
        let mut max_y = f64::NEG_INFINITY;
        let mut completed = false;
        for _ in 0..1_200 {
            store.tick_stationary(&active_chunks, &[], world);
            max_y = max_y.max(store.state(bee_id).unwrap().position.y);
            if !store.drain_bee_pollinations().is_empty() {
                completed = true;
                break;
            }
        }
        assert!(
            completed,
            "blocked bee should recover and complete its trip"
        );
        assert!(max_y > 68.0, "bee should visibly climb over the wall");
    }

    #[test]
    fn worked_bee_colony_yields_one_beeswax_and_resets_work() {
        let mut store = ServerEntityStore::default();
        let spawned = store
            .spawn_persistent_bee_colony(EntityKind::BeeNest, Vec3d::new(0.5, 64.0, 0.5), 0.0, &[])
            .unwrap();
        let colony_id = spawned[0].id;
        store.bee_colonies.get_mut(&colony_id).unwrap().stored_work = BEE_COLONY_WORK_CAPACITY;
        let updates = store.harvest_beeswax(colony_id).unwrap();
        assert!(updates.iter().any(|entity| {
            entity.item_stack
                == Some(ItemStackSnapshot {
                    kind: ItemKind::Beeswax,
                    count: 1,
                })
        }));
        assert!(store.harvest_beeswax(colony_id).is_none());
    }

    #[test]
    fn mallard_egg_waits_for_wetland_habitat_then_spawns_distinct_item() {
        let mut store = ServerEntityStore::default();
        let ticking_chunks = local_ticking_chunks();
        let mallard_id =
            store.insert_passive_mob_for_test(EntityKind::Mallard, Vec3d::new(4.0, 64.0, 4.0), 0.0);
        store
            .mobs
            .get_mut(&mallard_id)
            .unwrap()
            .set_mallard_egg_time_for_test(1);
        store.set_mallard_sex_for_test(mallard_id, mclone_protocol::MallardSex::Female);

        let dry_updates = store.tick_stationary(&ticking_chunks, &[], dry_grass_ground);
        assert!(
            dry_updates
                .iter()
                .all(|entity| entity.kind != EntityKind::Item)
        );
        assert_eq!(
            store.mob_state(mallard_id).unwrap().mallard_egg_time(),
            Some(0)
        );

        let mut egg = None;
        for _ in 0..400 {
            store.tick_stationary(&ticking_chunks, &[], wetland_ground);
            egg = store.states().into_iter().find(|entity| {
                entity.item_stack
                    == Some(ItemStackSnapshot {
                        kind: ItemKind::MallardEgg,
                        count: 1,
                    })
            });
            if egg.is_some() {
                break;
            }
        }
        let egg = egg.expect("due female should lay after returning to suitable wetland shore");
        assert_eq!(
            egg.item_stack,
            Some(ItemStackSnapshot {
                kind: ItemKind::MallardEgg,
                count: 1,
            })
        );
        assert!(
            store
                .mob_state(mallard_id)
                .unwrap()
                .mallard_egg_time()
                .unwrap()
                >= 8_000
        );
    }

    #[test]
    fn entity_tick_advances_tick_count_in_entity_ticking_chunks() {
        let mut store = ServerEntityStore::default();
        let id =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true)[0];

        assert!(
            store
                .tick_stationary(&[ChunkPos::new(1, 0)], &[], flat_ground)
                .is_empty()
        );
        assert_eq!(store.state(id).unwrap().tick_count, 0);
        assert_eq!(
            store.diagnostics(),
            ServerEntityStoreDiagnostics {
                stored_entities: PASSIVE_MOB_KINDS.len(),
                ticking_entities: 0
            }
        );

        let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);

        assert_eq!(updated.len(), PASSIVE_MOB_KINDS.len());
        let updated = updated
            .iter()
            .find(|entity| entity.id == id)
            .expect("showcase cow should update");
        assert_eq!(updated.tick_count, 1);
        assert_eq!(
            store.diagnostics(),
            ServerEntityStoreDiagnostics {
                stored_entities: PASSIVE_MOB_KINDS.len(),
                ticking_entities: PASSIVE_MOB_KINDS.len()
            }
        );
    }

    #[test]
    fn entity_tick_list_demotes_without_removing_stored_entity() {
        let mut store = ServerEntityStore::default();
        let id =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true)[0];
        store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);

        let updated = store.tick_stationary(&[], &[], flat_ground);

        assert!(updated.is_empty());
        assert_eq!(store.state(id).unwrap().tick_count, 1);
        assert_eq!(
            store.diagnostics(),
            ServerEntityStoreDiagnostics {
                stored_entities: PASSIVE_MOB_KINDS.len(),
                ticking_entities: 0
            }
        );
    }

    #[test]
    fn ticking_starter_cow_eventually_applies_passive_movement() {
        let mut store = ServerEntityStore::default();
        let id =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true)[0];
        let start = store.state(id).unwrap();

        let mut moved = None;
        for _ in 0..2_000 {
            let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);
            let entity = updated
                .into_iter()
                .find(|entity| entity.id == id)
                .expect("starter cow should tick while chunk is entity ticking");
            if entity.position != start.position {
                moved = Some(entity);
                break;
            }
        }

        let moved = moved.expect("cow passive AI should choose a stroll target");
        assert!(moved.tick_count > start.tick_count);
        assert!(moved.position.distance_to_sqr(start.position) > 0.0);
        let mob = store.mob_state(id).expect("starter cow mob state");
        assert!(mob.running_goal_count() > 0);
    }

    #[test]
    fn ticking_starter_chicken_eventually_applies_passive_movement() {
        let mut store = ServerEntityStore::default();
        let id =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true)[1];
        let start = store.state(id).unwrap();

        let mut moved = None;
        for _ in 0..2_000 {
            let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);
            let entity = updated
                .into_iter()
                .find(|entity| entity.id == id)
                .expect("starter chicken should tick while chunk is entity ticking");
            if entity.position != start.position {
                moved = Some(entity);
                break;
            }
        }

        let moved = moved.expect("chicken passive AI should choose a stroll target");
        assert!(moved.tick_count > start.tick_count);
        assert!(moved.position.distance_to_sqr(start.position) > 0.0);
        let mob = store.mob_state(id).expect("starter chicken mob state");
        assert!(mob.running_goal_count() > 0);
    }

    #[test]
    fn ticking_passive_without_support_falls_and_clears_ground_state() {
        let mut store = ServerEntityStore::default();
        let id =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true)[0];
        let start = store.state(id).unwrap();

        let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], no_blocks);

        let entity = updated
            .into_iter()
            .find(|entity| entity.id == id)
            .expect("starter cow should tick while chunk is entity ticking");
        assert!(entity.position.y < start.position.y);
        assert!(!entity.on_ground);
        let mob = store.mob_state(id).expect("starter cow mob state");
        assert!(mob.delta_movement().y < 0.0);
    }

    #[test]
    fn block_change_near_mob_path_marks_navigation_for_recompute() {
        let mut store = ServerEntityStore::default();
        let id =
            store.insert_passive_mob_for_test(EntityKind::Cow, Vec3d::new(0.5, 64.0, 0.5), 0.0);
        let entity = store.state(id).unwrap();
        let mob = store.mobs.get_mut(&id).expect("cow should have mob state");
        assert!(mob.move_to_for_test(entity, Vec3d::new(4.5, 64.0, 0.5), &flat_ground));
        assert!(!mob.navigation_has_delayed_recomputation());

        assert_eq!(store.on_block_changed(BlockPos::new(40, 64, 40)), 0);
        assert!(
            !store
                .mob_state(id)
                .unwrap()
                .navigation_has_delayed_recomputation()
        );

        assert_eq!(store.on_block_changed(BlockPos::new(2, 64, 0)), 1);
        assert!(
            store
                .mob_state(id)
                .unwrap()
                .navigation_has_delayed_recomputation()
        );
    }
}
