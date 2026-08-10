#![forbid(unsafe_code)]

mod authored_fixture;
#[cfg(test)]
mod block_light_bridge;
mod cadence;
mod dimension;
mod distance_manager;
mod entity;
mod falling_block;
mod fluid;
mod game_mode;
mod holder;
mod integrated;
mod inventory;
mod item_stack;
mod job_codec;
mod level_light_bridge;
mod light_mailbox;
mod light_status;
mod light_world;
mod lighting_seed;
mod loading_progress;
mod persistence;
#[cfg(feature = "physics-engine")]
mod physics_runtime;
#[cfg(feature = "physics")]
mod physics_terrain;
mod placement;
mod player;
mod player_chunk_tracking;
mod player_lifecycle;
mod players;
mod remote_players;
mod runner;
mod scheduler;
#[cfg(test)]
mod sky_light_bridge;
mod spawn;
mod starter_content;
mod structure_lab;
mod timing;
mod types;
#[cfg(target_arch = "wasm32")]
mod wasm_job_worker;
mod world_behavior_profile;
mod world_generation_profile;
mod worldgen_mailbox;

use mclone_core::{
    BlockStateId, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkSnapshot, SECTION_HEIGHT,
    chunk_section_index,
};
use mclone_worldgen::block::RawBlockId;
use mclone_worldgen::levelgen::MutableChunkBlockBuffer;

pub use authored_fixture::{
    AUTHORED_LOBBY_CHICKEN_PERSISTENT_ID, AUTHORED_LOBBY_COW_PERSISTENT_ID,
    AUTHORED_WORLD_FIXTURE_CENTER, AUTHORED_WORLD_FIXTURE_MARKER_FILE,
    AUTHORED_WORLD_FIXTURE_SCHEMA_VERSION, AUTHORED_WORLD_FIXTURE_VOID_PADDING_RADIUS,
    AuthoredWorldFixtureKind, AuthoredWorldFixtureManifest, authored_world_fixture_memory_store,
    authored_world_fixture_records, write_authored_world_fixture_to_store,
};
#[cfg(not(target_arch = "wasm32"))]
pub use authored_fixture::{authored_world_fixture_marker_path, write_authored_world_fixture_dir};
pub use cadence::{
    DEFAULT_GAMEPLAY_RATE_HZ, DEFAULT_HOST_RATE_HZ, DEFAULT_MAX_CATCH_UP_HOST_FRAMES,
    DEFAULT_PHYSICS_RATE_HZ, SimulationCadence, SimulationCadenceAdvance, SimulationCadenceConfig,
    SimulationCadenceFrame,
};
pub use dimension::DimensionRegistry;
pub use holder::{ChunkHolder, ChunkStatusSlot};
pub use integrated::{
    DimensionInterestDiagnostics, DimensionRuntime, INITIAL_DAY_TIME, LocalRealmSession,
    LocalRealmSessionRole, PlayerDimensionTransferDiagnostics, PlayerDimensionTransferPhase,
    RealmInterestDiagnostics, RealmServer,
};
pub use job_codec::{
    ServerJobActor, ServerJobActorDiagnostics, ServerJobActorKind, WorldgenJobSession,
    compute_light_status_job_frame, compute_worldgen_job_frame, decode_server_job_actor_init_frame,
    encode_server_job_actor_init_frame,
};
pub use loading_progress::{
    ChunkLoadingProgress, ChunkLoadingProgressCell, ChunkLoadingProgressSnapshot,
    ChunkLoadingProgressStats,
};
pub use mclone_protocol::EntityPersistentId;
pub use persistence::{
    CHUNK_LIGHT_ALGORITHM_VERSION, ChunkRecord, ChunkSnapshotStore, ChunkSnapshotWorldStore,
    ChunkStoreError, ChunkStoreResult, DIMENSION_RECORD_VERSION, DimensionDefinition,
    DimensionRecord, EntityChunkRecord, EntitySavePayload, EntitySaveRecord, MemoryRecordExecutor,
    MemoryRecordExecutorFault, MemoryWorldStore, NullChunkSnapshotStore, NullRecordExecutor,
    NullWorldStore, PersistenceActor, PersistenceErrorKind, PersistenceExecutorFailureLatch,
    PersistenceMailbox, PersistenceRecordAddress, PersistenceRecordBatch,
    PersistenceRecordExecutor, PersistenceRecordKeyPart, PersistenceRecordMutation,
    PersistenceRecordNamespace, PersistenceRecordPayload, PersistenceRecordRequest,
    PersistenceRecordRequestId, PersistenceRecordResponse, PersistenceRequestId, PlayerRecord,
    PlayerRecordKey, RecordExecutorWorldStore, SaveDurability, ScheduledTickRecord,
    StoreWriteOutcome, SynchronousPersistenceFacade, WORLD_METADATA_TARGET_MINECRAFT_VERSION,
    WORLD_METADATA_VERSION, WorldMetadata, WorldMetadataLoad, WorldRecordKey, WorldStore,
    WorldStoreCompletion, WorldStoreRequest, chunk_record_address, decode_chunk_record,
    decode_dimension_record, decode_entity_chunk_record, decode_player_record,
    decode_world_metadata, dimension_record_address, encode_chunk_record, encode_dimension_record,
    encode_entity_chunk_record, encode_player_record, encode_world_metadata, player_record_address,
    record_read_for_world_store_request, world_metadata_record_address,
    world_store_completion_from_record_read,
};
#[cfg(not(target_arch = "wasm32"))]
pub use persistence::{SQLITE_WORLD_DATABASE_FILE, SqliteWorldStore, WORLD_WRITER_LOCK_FILE};
pub use player_chunk_tracking::{
    DimensionInterestSource, ObserverChunkTrackingDiagnostics, ObserverId,
    ObserverSimulationInterest, PlayerChunkTrackingDiagnostics,
    PlayerChunkTrackingPlayerDiagnostics, validate_local_integrated_chunk_view_topology,
};
pub use players::ServerPlayerId;
pub use runner::{
    IntegratedServerRunner, LightStatusMailboxMetrics, ServerRunnerDiagnostics, ServerRunnerError,
    ServerRunnerKind, ServerRunnerResult, ServerRunnerTickDiagnostics, ServerUpdateEnvelope,
    WorkerFrameMetrics, WorkerFrameTransportKind,
};
pub(crate) use scheduler::FluidTickList;
pub use scheduler::{
    ChunkPublicationBudgetConfig, ChunkScheduler, ChunkSchedulerEvent, ChunkSchedulerMetrics,
    ChunkStatusJob, DEFAULT_LIGHT_STATUS_BATCH_SIZE, FluidTickPhaseReport, TopologyChunkState,
};
pub use spawn::{
    find_safe_surface_spawn_for_loaded_descriptor, find_safe_surface_spawn_for_loaded_profile,
    initial_spawn_center_for_descriptor, initial_spawn_center_for_profile,
    initial_spawn_center_for_seed,
};
pub use starter_content::{RealizedStarterPlanIdentity, StarterContentDescriptor};
pub use structure_lab::{
    BarnLength, BarnTemplateSet, BarnVariant, CottageDepth, CottageEntry, CottageVariant,
    STRUCTURE_FAMILY_LAB_GALLERY_ID, STRUCTURE_FAMILY_LAB_MARKER_FILE, STRUCTURE_FAMILY_LAB_SEED,
    STRUCTURE_FAMILY_LAB_VOID_PADDING_RADIUS, STRUCTURE_LAB_GALLERY_ID, STRUCTURE_LAB_MARKER_FILE,
    STRUCTURE_LAB_SCHEMA_VERSION, STRUCTURE_LAB_SEED, STRUCTURE_LAB_VOID_PADDING_RADIUS,
    StructureLabManifest, StructureLabMarkerReceipt, StructureLabTemplateReceipt,
    barn_core_template, barn_core_template_for, barn_lean_to_template, barn_lean_to_template_for,
    barn_templates_for, cottage_template, cottage_template_for, structure_family_lab_memory_store,
    structure_family_lab_records, structure_lab_memory_store, structure_lab_records,
    write_structure_family_lab_to_store, write_structure_lab_to_store,
};
#[cfg(not(target_arch = "wasm32"))]
pub use structure_lab::{
    structure_family_lab_marker_path, structure_lab_marker_path, write_structure_family_lab_dir,
    write_structure_lab_dir,
};
pub use timing::{
    ChunkSchedulerPublicationDiagnostics, ChunkSchedulerTickReport, ChunkSchedulerTickTiming,
    NaturalSpawningDiagnostics, ServerPhysicsStepReport, ServerPhysicsStepTiming,
    ServerPhysicsTickDiagnostics, ServerSimulationTickReport, ServerSimulationTickTiming,
    ServerTickReport, ServerTickTiming,
};
#[cfg(target_arch = "wasm32")]
pub use types::WasmServerJobWorkerConfig;
pub use types::{
    CHUNK_LEVEL_FULL, ChunkJobId, ChunkJobState, ChunkResidency, ChunkStatusStep, ChunkTicket,
    ChunkTicketKey, ChunkTicketType, FORCED_TICKET_LEVEL, FluidKind, FullChunkStatus,
    LightStatusMailboxKind, MAX_CHUNK_DISTANCE, PLAYER_TICKET_LEVEL, ServerMode,
    UNLOADED_CHUNK_LEVEL, WorldBlockPos, WorldgenMailboxKind,
};
pub use world_behavior_profile::WorldBehaviorProfile;
pub use world_generation_profile::{
    AUTHORED_WORLD_HEIGHT, AUTHORED_WORLD_MIN_Y, AuthoredMissingChunk, WorldGenerationDescriptor,
    WorldGenerationProfile,
};
pub(crate) use world_generation_profile::{
    GenerationExecutionRequest, GenerationInput, GenerationInputArtifact, GenerationPlanRequest,
};

#[cfg(not(target_arch = "wasm32"))]
pub use persistence::FilesystemChunkSnapshotStore;
#[cfg(not(target_arch = "wasm32"))]
pub use runner::{
    NativeIntegratedServerRunner, NativeIntegratedServerRunnerConfig,
    NativeIntegratedServerWorldStorage, host_tick_interval_for_rate_hz,
};

pub(crate) fn mutable_buffer_from_snapshot(snapshot: &ChunkSnapshot) -> MutableChunkBlockBuffer {
    let mut buffer = MutableChunkBlockBuffer::new(
        snapshot.pos.x,
        snapshot.pos.z,
        snapshot.min_y,
        snapshot.height,
    );
    for section in &snapshot.sections {
        let section_blocks = section.unpack_block_state_ids();
        debug_assert_eq!(section_blocks.len(), CHUNK_SECTION_VOLUME);
        let section_base_y = section.section_y * SECTION_HEIGHT;
        for local_y in 0..SECTION_HEIGHT {
            let y = section_base_y + local_y;
            if y < snapshot.min_y || y >= snapshot.min_y + snapshot.height {
                continue;
            }
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let index = chunk_section_index(local_x, local_y, local_z);
                    buffer.set_block_at_y(
                        local_x,
                        y,
                        local_z,
                        raw_block_id_from_state_id(section_blocks[index]),
                    );
                }
            }
        }
    }
    buffer
}

fn raw_block_id_from_state_id(state_id: BlockStateId) -> RawBlockId {
    RawBlockId::try_from(state_id.0)
        .unwrap_or_else(|_| panic!("block state id {} does not fit native raw id", state_id.0))
}

pub(crate) fn full_chunk_status_for_ticket_level(ticket_level: i32) -> FullChunkStatus {
    match (CHUNK_LEVEL_FULL - ticket_level + 1).clamp(0, 3) {
        0 => FullChunkStatus::Inaccessible,
        1 => FullChunkStatus::Border,
        2 => FullChunkStatus::Ticking,
        3 => FullChunkStatus::EntityTicking,
        _ => unreachable!("clamped full chunk status index must be in 0..=3"),
    }
}

#[cfg(test)]
mod tests;
