#![forbid(unsafe_code)]

#[cfg(test)]
mod block_light_bridge;
mod cadence;
mod distance_manager;
mod entities;
mod falling_block;
mod fluid;
mod game_mode;
mod holder;
mod integrated;
mod inventory;
mod job_codec;
mod level_light_bridge;
mod light_mailbox;
mod light_status;
mod light_world;
mod lighting_seed;
mod loading_progress;
mod persistence;
#[cfg(feature = "physics-rapier")]
mod physics_runtime;
#[cfg(feature = "physics")]
mod physics_terrain;
mod placement;
mod player;
mod player_chunk_tracking;
mod players;
mod remote_players;
mod runner;
mod scheduler;
#[cfg(test)]
mod sky_light_bridge;
mod spawn;
mod timing;
mod types;
#[cfg(target_arch = "wasm32")]
mod wasm_job_worker;
mod worldgen_mailbox;

use mclone_core::{
    BlockStateId, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkSnapshot, SECTION_HEIGHT,
    chunk_section_index,
};
use mclone_worldgen::block::RawBlockId;
use mclone_worldgen::levelgen::MutableChunkBlockBuffer;

pub use cadence::{
    DEFAULT_GAMEPLAY_RATE_HZ, DEFAULT_HOST_RATE_HZ, DEFAULT_MAX_CATCH_UP_HOST_FRAMES,
    DEFAULT_PHYSICS_RATE_HZ, SimulationCadence, SimulationCadenceAdvance, SimulationCadenceConfig,
    SimulationCadenceFrame,
};
pub use holder::{ChunkHolder, ChunkStatusSlot};
pub use integrated::{INITIAL_DAY_TIME, IntegratedServer};
pub use job_codec::{
    WorldgenJobSession, compute_light_status_job_frame, compute_worldgen_job_frame,
};
pub use loading_progress::{
    ChunkLoadingProgress, ChunkLoadingProgressCell, ChunkLoadingProgressSnapshot,
    ChunkLoadingProgressStats,
};
pub use persistence::{
    ChunkSnapshotStore, ChunkStoreError, ChunkStoreResult, NullChunkSnapshotStore,
};
pub use player_chunk_tracking::{
    PlayerChunkTrackingDiagnostics, PlayerChunkTrackingPlayerDiagnostics,
};
pub use players::ServerPlayerId;
pub use runner::{
    IntegratedServerRunner, ServerRunnerDiagnostics, ServerRunnerError, ServerRunnerKind,
    ServerRunnerResult, ServerRunnerTickDiagnostics, WorkerFrameMetrics, WorkerFrameTransportKind,
};
pub(crate) use scheduler::FluidTickList;
pub use scheduler::{
    ChunkScheduler, ChunkSchedulerEvent, ChunkSchedulerMetrics, ChunkStatusJob,
    FluidTickPhaseReport,
};
pub use spawn::initial_spawn_center_for_seed;
pub use timing::{
    ChunkSchedulerTickReport, ChunkSchedulerTickTiming, ServerPhysicsStepReport,
    ServerPhysicsStepTiming, ServerPhysicsTickDiagnostics, ServerSimulationTickReport,
    ServerSimulationTickTiming, ServerTickReport, ServerTickTiming,
};
#[cfg(target_arch = "wasm32")]
pub use types::WasmServerJobWorkerConfig;
pub use types::{
    CHUNK_LEVEL_FULL, ChunkJobId, ChunkJobState, ChunkResidency, ChunkStatusStep, ChunkTicket,
    ChunkTicketKey, ChunkTicketType, FORCED_TICKET_LEVEL, FluidKind, FullChunkStatus,
    LightStatusMailboxKind, MAX_CHUNK_DISTANCE, PLAYER_TICKET_LEVEL, ServerMode,
    UNLOADED_CHUNK_LEVEL, WorldBlockPos, WorldgenMailboxKind,
};

#[cfg(test)]
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use lighting_seed::{
    provisional_block_light_sections_for_chunk, provisional_sky_light_sections,
    provisional_sky_light_sections_for_chunk, snapshot_with_provisional_lighting,
};
#[cfg(test)]
use mclone_core::{
    ChunkPos, ChunkRevision, ChunkStatus, PackedLightSection, block_to_section_coord,
    chunk_block_index, local_block_coord, local_section_block_coord,
};
#[cfg(test)]
use mclone_light::LightLayer;
#[cfg(test)]
use mclone_protocol::{AcceptTeleportCommand, ChunkView, ClientCommand, ServerUpdate};
#[cfg(test)]
use mclone_worldgen::block::{LAVA, WATER, generated_block_state_id};
#[cfg(test)]
use scheduler::{DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET, DEFAULT_PENDING_UNLOAD_BUDGET};

#[cfg(not(target_arch = "wasm32"))]
pub use persistence::FilesystemChunkSnapshotStore;
#[cfg(not(target_arch = "wasm32"))]
pub use runner::{
    NativeIntegratedServerRunner, NativeIntegratedServerRunnerConfig,
    host_tick_interval_for_rate_hz,
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
mod tests {
    use super::*;
    use mclone_worldgen::block::{
        AIR, LAVA_LEVEL_2, LAVA_LEVEL_8, OBSIDIAN, STONE, WATER_LEVEL_1, WATER_LEVEL_2,
        WATER_LEVEL_8, lava_block_for_level, water_block_for_level,
    };
    use serde_json::Value;

    fn apply_interest_and_poll(
        scheduler: &mut ChunkScheduler,
        interest: ChunkView,
    ) -> Vec<ChunkSchedulerEvent> {
        let mut events = scheduler.apply_interest(interest).unwrap();
        events.extend(poll_scheduler_until_idle(scheduler));
        events
    }

    fn poll_scheduler_until_idle(scheduler: &mut ChunkScheduler) -> Vec<ChunkSchedulerEvent> {
        let mut events = Vec::new();
        for _ in 0..60_000 {
            events.extend(scheduler.poll().unwrap());
            if scheduler.pending_job_count() == 0 {
                return events;
            }
            if scheduler.pending_publication_count() == 0 {
                wait_for_scheduler_completion(scheduler);
            }
        }
        panic!("timed out waiting for scheduler worldgen jobs");
    }

    #[test]
    fn provisional_sky_light_spreads_sideways_below_overhangs() {
        let mut blocks = vec![AIR; (CHUNK_WIDTH * SECTION_HEIGHT * CHUNK_WIDTH) as usize];
        blocks[chunk_block_index(0, 14, 0)] = STONE;

        let sections = provisional_sky_light_sections(0, SECTION_HEIGHT, &blocks);

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].section_y, 0);
        assert!(sections[0].block.is_none());
        let sky =
            mclone_light::packed_light_section_layer(&sections[0], mclone_light::LightLayer::Sky)
                .unwrap()
                .unwrap();
        assert_eq!(sky.get(0, 15, 0), 15);
        assert_eq!(sky.get(0, 14, 0), 0);
        assert_eq!(sky.get(0, 13, 0), 14);
        assert_eq!(sky.get(1, 0, 0), 15);
    }

    #[test]
    fn provisional_sky_light_keeps_full_roof_dark_below() {
        let mut blocks = vec![AIR; (CHUNK_WIDTH * SECTION_HEIGHT * CHUNK_WIDTH) as usize];
        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                blocks[chunk_block_index(local_x, 14, local_z)] = STONE;
            }
        }

        let sections = provisional_sky_light_sections(0, SECTION_HEIGHT, &blocks);

        assert_eq!(sections.len(), 1);
        let sky =
            mclone_light::packed_light_section_layer(&sections[0], mclone_light::LightLayer::Sky)
                .unwrap()
                .unwrap();
        assert_eq!(sky.get(8, 15, 8), 15);
        assert_eq!(sky.get(8, 14, 8), 0);
        assert_eq!(sky.get(8, 13, 8), 0);
    }

    #[test]
    fn provisional_sky_light_marks_all_dark_chunks_as_lit_data() {
        let blocks = vec![STONE; (CHUNK_WIDTH * SECTION_HEIGHT * CHUNK_WIDTH) as usize];

        let sections = provisional_sky_light_sections(0, SECTION_HEIGHT, &blocks);

        assert_eq!(sections.len(), 1);
        let sky =
            mclone_light::packed_light_section_layer(&sections[0], mclone_light::LightLayer::Sky)
                .unwrap()
                .unwrap();
        assert_eq!(sky.get(8, 8, 8), 0);
    }

    #[test]
    fn provisional_sky_light_preserves_dark_section_below_lit_section() {
        let height = SECTION_HEIGHT * 2;
        let mut blocks = vec![AIR; (CHUNK_WIDTH * height * CHUNK_WIDTH) as usize];
        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                blocks[chunk_block_index(local_x, SECTION_HEIGHT - 1, local_z)] = STONE;
            }
        }

        let sections = provisional_sky_light_sections(0, height, &blocks);

        assert!(
            sections
                .iter()
                .any(|section| section.section_y == 0 && section.sky.is_some()),
            "lower section should carry an explicit empty sky layer"
        );
        assert_eq!(
            mclone_light::packed_sky_light(
                mclone_light::packed_light_at_local_block_or_fullbright(
                    &sections, 0, height, 8, 4, 8,
                )
            ),
            0
        );
        assert_eq!(
            mclone_light::packed_sky_light(
                mclone_light::packed_light_at_local_block_or_fullbright(
                    &sections, 0, height, 8, 20, 8,
                )
            ),
            15
        );
    }

    #[test]
    fn provisional_sky_light_matches_java_synthetic_fixture() {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../test/fixtures/lighting/sky-synthetic.json"
        ))
        .unwrap();
        assert_eq!(fixture["module"], "synthetic-light");
        let min_y = fixture_i32(&fixture["level"], "minY");
        let height = fixture_i32(&fixture["level"], "height");
        assert_eq!(min_y, -SECTION_HEIGHT);
        assert_eq!(height, SECTION_HEIGHT * 3);

        for case_name in ["openColumn", "fullRoof", "singleOverhang", "stoneRoom"] {
            let case = synthetic_light_case(&fixture, case_name);
            let blocks = native_blocks_from_synthetic_light_case(case, min_y, height);
            let sections = provisional_sky_light_sections(min_y, height, &blocks);

            assert_synthetic_light_samples(case, ChunkPos::new(0, 0), &sections);
            let java_section = synthetic_light_section(case, 0, 0, 0);
            assert_eq!(
                sky_section_hex(&sections, 0),
                java_section["dataHex"]
                    .as_str()
                    .expect("synthetic light dataHex must be a string"),
                "sky DataLayer mismatch for synthetic light case `{case_name}`"
            );
        }
    }

    #[test]
    fn provisional_sky_light_matches_java_cross_chunk_synthetic_fixture() {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../test/fixtures/lighting/sky-synthetic.json"
        ))
        .unwrap();
        let min_y = fixture_i32(&fixture["level"], "minY");
        let height = fixture_i32(&fixture["level"], "height");
        let case = synthetic_light_case(&fixture, "fullRoofEastOpenNeighbor");
        let chunks = native_chunk_blocks_from_synthetic_light_case(case, min_y, height);
        let target_pos = ChunkPos::new(0, 0);
        let sections = provisional_sky_light_sections_for_chunk(
            target_pos,
            min_y,
            height,
            chunks.iter().map(|(pos, blocks)| (*pos, blocks.as_slice())),
        );

        assert_synthetic_light_samples(case, target_pos, &sections);
        assert_eq!(sample_sky_light(&sections, 15, 13, 8), 14);
        assert_eq!(sample_sky_light(&sections, 14, 13, 8), 13);

        let java_section = synthetic_light_section(case, 0, 0, 0);
        assert_eq!(
            sky_section_hex(&sections, 0),
            java_section["dataHex"]
                .as_str()
                .expect("synthetic light dataHex must be a string")
        );

        let local_only_sections =
            provisional_sky_light_sections(min_y, height, chunks.get(&target_pos).unwrap());
        assert_eq!(sample_sky_light(&local_only_sections, 15, 13, 8), 0);
    }

    #[test]
    fn provisional_block_light_matches_java_synthetic_fixture() {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../test/fixtures/lighting/sky-synthetic.json"
        ))
        .unwrap();
        let min_y = fixture_i32(&fixture["level"], "minY");
        let height = fixture_i32(&fixture["level"], "height");

        for case_name in ["lavaOpen", "lavaBlockedByStone"] {
            let case = synthetic_block_light_case(&fixture, case_name);
            let chunks = native_chunk_blocks_from_synthetic_light_case(case, min_y, height);
            let target_pos = ChunkPos::new(0, 0);
            let sections = provisional_block_light_sections_for_chunk(
                target_pos,
                min_y,
                height,
                chunks.iter().map(|(pos, blocks)| (*pos, blocks.as_slice())),
            );

            assert_synthetic_block_light_samples(case, target_pos, &sections);
            let java_section = synthetic_block_light_section(case, 0, 0, 0);
            assert_eq!(
                block_section_hex(&sections, 0),
                java_section["dataHex"]
                    .as_str()
                    .expect("synthetic block light dataHex must be a string"),
                "block DataLayer mismatch for synthetic light case `{case_name}`"
            );
        }
    }

    #[test]
    fn provisional_block_light_matches_java_cross_chunk_synthetic_fixture() {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../test/fixtures/lighting/sky-synthetic.json"
        ))
        .unwrap();
        let min_y = fixture_i32(&fixture["level"], "minY");
        let height = fixture_i32(&fixture["level"], "height");
        let case = synthetic_block_light_case(&fixture, "lavaCrossChunk");
        let chunks = native_chunk_blocks_from_synthetic_light_case(case, min_y, height);

        for target_pos in [ChunkPos::new(0, 0), ChunkPos::new(1, 0)] {
            let sections = provisional_block_light_sections_for_chunk(
                target_pos,
                min_y,
                height,
                chunks.iter().map(|(pos, blocks)| (*pos, blocks.as_slice())),
            );

            assert_synthetic_block_light_samples(case, target_pos, &sections);
            let java_section = synthetic_block_light_section(case, target_pos.x, 0, target_pos.z);
            assert_eq!(
                block_section_hex(&sections, 0),
                java_section["dataHex"]
                    .as_str()
                    .expect("synthetic block light dataHex must be a string")
            );
        }
    }

    #[test]
    fn generated_ocean_chunk_light_matches_persisted_java_oracle_fixture() {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../test/fixtures/integration/overworld-seed-12345-chunks-5-115.json"
        ))
        .expect("valid full integration fixture");
        assert_generated_chunk_light_matches_persisted_fixture(fixture, true);
    }

    #[test]
    fn generated_origin_chunk_sky_light_matches_persisted_java_oracle_fixture() {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../test/fixtures/integration/overworld-seed-12345-chunks-0-0.json"
        ))
        .expect("valid full integration fixture");
        assert_generated_chunk_light_matches_persisted_fixture(fixture, false);
    }

    #[test]
    fn generated_origin_chunk_sky_light_matches_scheduler_light_oracle_fixture() {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../test/fixtures/scheduler/vanilla-scheduler-light-snapshot-seed-12345-chunk-0-0.json"
        ))
        .expect("valid scheduler LIGHT fixture");
        assert_generated_chunk_light_matches_scheduler_fixture(fixture, false);
    }

    #[test]
    fn generated_origin_chunk_block_light_strict_matches_scheduler_light_oracle_fixture() {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../test/fixtures/scheduler/vanilla-scheduler-light-snapshot-seed-12345-chunk-0-0.json"
        ))
        .expect("valid scheduler LIGHT fixture");
        assert_generated_chunk_light_matches_scheduler_fixture(fixture, true);
    }

    fn assert_generated_chunk_light_matches_persisted_fixture(
        fixture: Value,
        compare_block_light: bool,
    ) {
        assert_eq!(fixture["module"], "integration");
        assert_eq!(fixture["minecraftVersion"], "1.17.1");

        let chunks = fixture["chunks"]
            .as_array()
            .expect("integration fixture chunks must be an array");
        assert_eq!(chunks.len(), 1);
        let chunk = &chunks[0];
        assert_eq!(chunk["status"], "full");
        assert_eq!(chunk["isLightOn"], true);
        let target = ChunkPos::new(fixture_i32(chunk, "chunkX"), fixture_i32(chunk, "chunkZ"));
        let seed = fixture["seed"]
            .as_str()
            .expect("integration fixture seed must be a string")
            .parse::<i64>()
            .expect("integration fixture seed must fit i64");

        let mut server = IntegratedServer::new(seed);
        let updates = handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: target,
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        let snapshot = snapshot_update_for(&updates, target)
            .expect("native server should publish oracle target snapshot");

        assert_eq!(snapshot.status, ChunkStatus::Light);
        assert!(
            snapshot.light_correct,
            "generated oracle target snapshot must publish light-correct data"
        );
        assert_persisted_light_layer_matches_fixture(
            "sky",
            fixture_light_layer(chunk, "sky"),
            packed_light_layer(&snapshot.light_sections, LightLayer::Sky),
        );
        if compare_block_light {
            assert_persisted_light_layer_matches_fixture(
                "block",
                fixture_light_layer(chunk, "block"),
                packed_light_layer(&snapshot.light_sections, LightLayer::Block),
            );
        }
    }

    fn assert_generated_chunk_light_matches_scheduler_fixture(
        fixture: Value,
        compare_block_light: bool,
    ) {
        assert_eq!(fixture["module"], "scheduler-trace");
        assert_eq!(fixture["minecraftVersion"], "1.17.1");
        assert_eq!(fixture["stopStatus"], "LIGHT");

        let chunks = fixture["chunks"]
            .as_array()
            .expect("scheduler fixture chunks must be an array");
        assert_eq!(chunks.len(), 1);
        let chunk = &chunks[0];
        assert_eq!(chunk["lightCorrect"], true);
        let target = ChunkPos::new(fixture_i32(chunk, "chunkX"), fixture_i32(chunk, "chunkZ"));
        let seed = fixture["seed"]
            .as_str()
            .expect("scheduler fixture seed must be a string")
            .parse::<i64>()
            .expect("scheduler fixture seed must fit i64");

        let mut server = IntegratedServer::new(seed);
        let updates = handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: target,
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        let snapshot = snapshot_update_for(&updates, target)
            .expect("native server should publish oracle target snapshot");

        assert_eq!(snapshot.status, ChunkStatus::Light);
        assert!(
            snapshot.light_correct,
            "generated oracle target snapshot must publish light-correct data"
        );
        assert_light_layer_matches_fixture(
            "sky",
            fixture_light_layer(chunk, "sky"),
            packed_light_layer(&snapshot.light_sections, LightLayer::Sky),
        );
        if compare_block_light {
            assert_light_layer_matches_fixture(
                "block",
                fixture_light_layer(chunk, "block"),
                packed_light_layer(&snapshot.light_sections, LightLayer::Block),
            );
        }
    }

    #[test]
    fn snapshot_with_provisional_lighting_packs_block_light_layer() {
        let min_y = -SECTION_HEIGHT;
        let height = SECTION_HEIGHT * 3;
        let mut blocks = vec![AIR; (CHUNK_WIDTH * height * CHUNK_WIDTH) as usize];
        blocks[chunk_block_index(1, 1 - min_y, 1)] = LAVA;
        let block_state_ids = blocks
            .iter()
            .map(|block_id| generated_block_state_id(*block_id))
            .collect::<Vec<_>>();
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Features,
            ChunkRevision(1),
            min_y,
            height,
            &block_state_ids,
        );

        let snapshot = snapshot_with_provisional_lighting(snapshot, &blocks);

        assert_eq!(sample_block_light(&snapshot.light_sections, 1, 1, 1), 15);
        assert_eq!(sample_block_light(&snapshot.light_sections, 2, 1, 1), 14);
    }

    #[test]
    fn synthetic_light_fixture_records_cross_chunk_boundary_case() {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../test/fixtures/lighting/sky-synthetic.json"
        ))
        .unwrap();
        let cross_light_case = synthetic_light_case(&fixture, "fullRoofEastOpenNeighbor");
        assert_eq!(cross_light_case["skySections"].as_array().unwrap().len(), 6);
        assert_eq!(
            cross_light_case["samples"]
                .as_array()
                .unwrap()
                .iter()
                .find(|sample| {
                    fixture_i32(sample, "x") == 15
                        && fixture_i32(sample, "y") == 13
                        && fixture_i32(sample, "z") == 8
                })
                .map(|sample| fixture_i32(sample, "sky")),
            Some(14)
        );

        let case = synthetic_light_case(&fixture, "chunkBoundaryOverhang");

        assert_eq!(case["skySections"].as_array().unwrap().len(), 6);
        assert_eq!(
            case["samples"]
                .as_array()
                .unwrap()
                .iter()
                .find(|sample| {
                    fixture_i32(sample, "x") == 16
                        && fixture_i32(sample, "y") == 13
                        && fixture_i32(sample, "z") == 0
                })
                .map(|sample| fixture_i32(sample, "sky")),
            Some(15)
        );
    }

    fn synthetic_light_case<'a>(fixture: &'a Value, name: &str) -> &'a Value {
        fixture["cases"]
            .as_array()
            .expect("synthetic light fixture cases must be an array")
            .iter()
            .find(|case| case["name"].as_str() == Some(name))
            .unwrap_or_else(|| panic!("synthetic light fixture missing case `{name}`"))
    }

    fn synthetic_block_light_case<'a>(fixture: &'a Value, name: &str) -> &'a Value {
        fixture["blockCases"]
            .as_array()
            .expect("synthetic light fixture blockCases must be an array")
            .iter()
            .find(|case| case["name"].as_str() == Some(name))
            .unwrap_or_else(|| panic!("synthetic light fixture missing block case `{name}`"))
    }

    fn synthetic_light_section(
        case: &Value,
        section_x: i32,
        section_y: i32,
        section_z: i32,
    ) -> &Value {
        synthetic_light_section_from(case, "skySections", section_x, section_y, section_z)
    }

    fn synthetic_block_light_section(
        case: &Value,
        section_x: i32,
        section_y: i32,
        section_z: i32,
    ) -> &Value {
        synthetic_light_section_from(case, "blockSections", section_x, section_y, section_z)
    }

    fn synthetic_light_section_from<'a>(
        case: &'a Value,
        key: &str,
        section_x: i32,
        section_y: i32,
        section_z: i32,
    ) -> &'a Value {
        case[key]
            .as_array()
            .unwrap_or_else(|| panic!("synthetic light case {key} must be an array"))
            .iter()
            .find(|section| {
                fixture_i32(section, "sectionX") == section_x
                    && fixture_i32(section, "sectionY") == section_y
                    && fixture_i32(section, "sectionZ") == section_z
            })
            .unwrap_or_else(|| {
                panic!(
                    "synthetic light case missing section ({section_x}, {section_y}, {section_z})"
                )
            })
    }

    fn native_blocks_from_synthetic_light_case(
        case: &Value,
        min_y: i32,
        height: i32,
    ) -> Vec<RawBlockId> {
        let mut chunks = native_chunk_blocks_from_synthetic_light_case(case, min_y, height);
        chunks
            .remove(&ChunkPos::new(0, 0))
            .expect("synthetic light case must include chunk (0, 0)")
    }

    fn native_chunk_blocks_from_synthetic_light_case(
        case: &Value,
        min_y: i32,
        height: i32,
    ) -> BTreeMap<ChunkPos, Vec<RawBlockId>> {
        let mut chunks = BTreeMap::new();
        for chunk in case["chunks"]
            .as_array()
            .expect("synthetic light case chunks must be an array")
        {
            let coords = chunk
                .as_array()
                .expect("synthetic light chunk entry must be an array");
            assert_eq!(coords.len(), 2);
            let chunk_pos = ChunkPos::new(
                coords[0].as_i64().unwrap() as i32,
                coords[1].as_i64().unwrap() as i32,
            );
            chunks.insert(
                chunk_pos,
                vec![AIR; (CHUNK_WIDTH * height * CHUNK_WIDTH) as usize],
            );
        }

        for cell in case["opaque"]
            .as_array()
            .expect("synthetic light case opaque must be an array")
        {
            let coords = cell
                .as_array()
                .expect("synthetic light opaque cell must be an array");
            assert_eq!(coords.len(), 3);
            let x = coords[0].as_i64().unwrap() as i32;
            let y = coords[1].as_i64().unwrap() as i32;
            let z = coords[2].as_i64().unwrap() as i32;
            let pos = WorldBlockPos::new(x, y, z);
            let chunk_pos = pos.chunk_pos();
            let blocks = chunks
                .get_mut(&chunk_pos)
                .unwrap_or_else(|| panic!("opaque cell ({x}, {y}, {z}) had no loaded chunk"));
            assert!((min_y..min_y + height).contains(&y));
            blocks[chunk_block_index(local_block_coord(x), y - min_y, local_block_coord(z))] =
                STONE;
        }
        if let Some(lava_cells) = case.get("lava").and_then(Value::as_array) {
            for cell in lava_cells {
                let coords = cell
                    .as_array()
                    .expect("synthetic light lava cell must be an array");
                assert_eq!(coords.len(), 3);
                let x = coords[0].as_i64().unwrap() as i32;
                let y = coords[1].as_i64().unwrap() as i32;
                let z = coords[2].as_i64().unwrap() as i32;
                let pos = WorldBlockPos::new(x, y, z);
                let chunk_pos = pos.chunk_pos();
                let blocks = chunks
                    .get_mut(&chunk_pos)
                    .unwrap_or_else(|| panic!("lava cell ({x}, {y}, {z}) had no loaded chunk"));
                assert!((min_y..min_y + height).contains(&y));
                blocks[chunk_block_index(local_block_coord(x), y - min_y, local_block_coord(z))] =
                    LAVA;
            }
        }
        chunks
    }

    fn assert_synthetic_light_samples(
        case: &Value,
        target_pos: ChunkPos,
        sections: &[PackedLightSection],
    ) {
        let case_name = case["name"].as_str().unwrap();
        let mut checked = 0;
        for sample in case["samples"]
            .as_array()
            .expect("synthetic light case samples must be an array")
        {
            let x = fixture_i32(sample, "x");
            let y = fixture_i32(sample, "y");
            let z = fixture_i32(sample, "z");
            if WorldBlockPos::new(x, y, z).chunk_pos() != target_pos {
                continue;
            }
            let expected_sky = fixture_i32(sample, "sky") as u8;
            assert_eq!(
                sample_sky_light(sections, x, y, z),
                expected_sky,
                "sky sample mismatch for synthetic light case `{case_name}` at ({x}, {y}, {z})"
            );
            assert_eq!(
                fixture_i32(sample, "block"),
                0,
                "synthetic sky fixture should not emit block light"
            );
            checked += 1;
        }
        assert!(
            checked > 0,
            "synthetic light case `{case_name}` had no samples in target chunk {target_pos:?}"
        );
    }

    fn assert_synthetic_block_light_samples(
        case: &Value,
        target_pos: ChunkPos,
        sections: &[PackedLightSection],
    ) {
        let case_name = case["name"].as_str().unwrap();
        let mut checked = 0;
        for sample in case["samples"]
            .as_array()
            .expect("synthetic light case samples must be an array")
        {
            let x = fixture_i32(sample, "x");
            let y = fixture_i32(sample, "y");
            let z = fixture_i32(sample, "z");
            if WorldBlockPos::new(x, y, z).chunk_pos() != target_pos {
                continue;
            }
            let expected_block = fixture_i32(sample, "block") as u8;
            assert_eq!(
                sample_block_light(sections, x, y, z),
                expected_block,
                "block sample mismatch for synthetic light case `{case_name}` at ({x}, {y}, {z})"
            );
            assert_eq!(
                fixture_i32(sample, "sky"),
                0,
                "synthetic block fixture should not contain sky light"
            );
            checked += 1;
        }
        assert!(
            checked > 0,
            "synthetic light case `{case_name}` had no block samples in target chunk {target_pos:?}"
        );
    }

    fn sample_sky_light(sections: &[PackedLightSection], x: i32, y: i32, z: i32) -> u8 {
        sample_light(sections, LightLayer::Sky, x, y, z)
    }

    fn sample_block_light(sections: &[PackedLightSection], x: i32, y: i32, z: i32) -> u8 {
        sample_light(sections, LightLayer::Block, x, y, z)
    }

    fn sample_light(
        sections: &[PackedLightSection],
        layer: LightLayer,
        x: i32,
        y: i32,
        z: i32,
    ) -> u8 {
        let section_y = block_to_section_coord(y);
        let Some(section) = sections
            .iter()
            .find(|section| section.section_y == section_y)
        else {
            return 0;
        };
        let Some(data_layer) = mclone_light::packed_light_section_layer(section, layer).unwrap()
        else {
            return 0;
        };
        data_layer.get(
            local_block_coord(x),
            local_section_block_coord(y),
            local_block_coord(z),
        )
    }

    fn sky_section_hex(sections: &[PackedLightSection], section_y: i32) -> String {
        light_section_hex(sections, section_y, LightLayer::Sky)
    }

    fn block_section_hex(sections: &[PackedLightSection], section_y: i32) -> String {
        light_section_hex(sections, section_y, LightLayer::Block)
    }

    fn light_section_hex(
        sections: &[PackedLightSection],
        section_y: i32,
        layer: LightLayer,
    ) -> String {
        let section = sections
            .iter()
            .find(|section| section.section_y == section_y)
            .unwrap_or_else(|| panic!("missing light section {section_y}"));
        let data_layer = mclone_light::packed_light_section_layer(section, layer)
            .unwrap()
            .unwrap_or_else(mclone_light::DataLayer::new);
        data_layer_hex(&data_layer)
    }

    fn data_layer_hex(layer: &mclone_light::DataLayer) -> String {
        let bytes = layer
            .as_bytes()
            .map(|bytes| bytes.as_slice())
            .unwrap_or(&[0; mclone_light::DATA_LAYER_SIZE]);
        let mut hex = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write;
            write!(&mut hex, "{byte:02x}").unwrap();
        }
        hex
    }

    fn handle_command_and_poll(
        server: &mut IntegratedServer,
        command: ClientCommand,
    ) -> Vec<ServerUpdate> {
        let mut updates = server.handle_command(command);
        updates.extend(poll_server_until_idle(server));
        accept_and_remove_player_position_updates(server, &mut updates);
        updates
    }

    fn try_handle_command_and_poll(
        server: &mut IntegratedServer,
        command: ClientCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let mut updates = server.try_handle_command(command)?;
        updates.extend(try_poll_server_until_idle(server)?);
        try_accept_and_remove_player_position_updates(server, &mut updates)?;
        Ok(updates)
    }

    fn accept_and_remove_player_position_updates(
        server: &mut IntegratedServer,
        updates: &mut Vec<ServerUpdate>,
    ) {
        try_accept_and_remove_player_position_updates(server, updates)
            .expect("failed to accept player position updates");
    }

    fn try_accept_and_remove_player_position_updates(
        server: &mut IntegratedServer,
        updates: &mut Vec<ServerUpdate>,
    ) -> ChunkStoreResult<()> {
        let mut index = 0;
        while index < updates.len() {
            let teleport_id = match &updates[index] {
                ServerUpdate::PlayerPosition(update) => update.teleport_id,
                _ => {
                    index += 1;
                    continue;
                }
            };
            let ack_updates = server.try_handle_command(ClientCommand::AcceptTeleport(
                AcceptTeleportCommand { id: teleport_id },
            ))?;
            assert!(ack_updates.is_empty());
            updates.remove(index);
        }
        Ok(())
    }

    fn poll_server_until_idle(server: &mut IntegratedServer) -> Vec<ServerUpdate> {
        try_poll_server_until_idle(server).unwrap()
    }

    fn try_poll_server_until_idle(
        server: &mut IntegratedServer,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let mut updates = Vec::new();
        for _ in 0..60_000 {
            updates.extend(server.try_poll()?);
            if server.pending_job_count() == 0 {
                return Ok(updates);
            }
            if server.pending_publication_count() == 0 {
                wait_for_server_completion(server);
            }
        }
        panic!("timed out waiting for integrated server worldgen jobs");
    }

    fn assert_liquid_oracle_fixture_matches(server: &IntegratedServer, fixture_json: &str) {
        let fixture = serde_json::from_str::<Value>(fixture_json).unwrap();
        let bounds = &fixture["bounds"];
        let min_x = fixture_i32(bounds, "minX");
        let min_y = fixture_i32(bounds, "minY");
        let min_z = fixture_i32(bounds, "minZ");
        let size_x = fixture_i32(bounds, "sizeX");
        let size_y = fixture_i32(bounds, "sizeY");
        let size_z = fixture_i32(bounds, "sizeZ");
        let palette = fixture["palette"]
            .as_array()
            .expect("liquid fixture palette must be an array")
            .iter()
            .map(fixture_palette_block_id)
            .collect::<Vec<_>>();
        let blocks = fixture["blocks"]
            .as_array()
            .expect("liquid fixture blocks must be an array");
        assert_eq!(
            blocks.len(),
            (size_x * size_y * size_z) as usize,
            "liquid fixture block volume should match bounds"
        );

        for (index, block) in blocks.iter().enumerate() {
            let index = index as i32;
            let x = index % size_x;
            let z = (index / size_x) % size_z;
            let y = index / (size_x * size_z);
            let palette_index = block
                .as_u64()
                .expect("liquid fixture block entry must be a palette index")
                as usize;
            let expected = palette[palette_index];
            let pos = WorldBlockPos::new(min_x + x, min_y + y, min_z + z);
            assert_eq!(
                server.scheduler().block_at_world(pos),
                Some(expected),
                "liquid fixture block mismatch at {pos:?}"
            );
        }

        let mut expected_ticks = fixture["liquidTicks"]
            .as_array()
            .expect("liquid fixture liquidTicks must be an array")
            .iter()
            .map(|tick| {
                (
                    WorldBlockPos::new(
                        fixture_i32(tick, "x"),
                        fixture_i32(tick, "y"),
                        fixture_i32(tick, "z"),
                    ),
                    FluidKind::from_target(
                        tick["target"]
                            .as_str()
                            .expect("liquid tick target must be a string"),
                    )
                    .expect("liquid tick target must be a known fluid"),
                    fixture_i32(tick, "delay"),
                )
            })
            .collect::<Vec<_>>();
        expected_ticks.sort();
        assert_eq!(
            server
                .liquid_ticks
                .scheduled_tick_entries(server.simulation_tick()),
            expected_ticks
        );
    }

    fn new_liquid_oracle_server() -> IntegratedServer {
        new_liquid_oracle_server_with_radius(0)
    }

    fn new_liquid_oracle_server_with_radius(radius_chunks: u32) -> IntegratedServer {
        let mut server = IntegratedServer::new(12_345);
        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: radius_chunks,
                chunk_tracking_radius: radius_chunks,
            }),
        );
        server.liquid_ticks = FluidTickList::new();
        server
    }

    fn fill_blocks(
        server: &mut IntegratedServer,
        min: WorldBlockPos,
        max: WorldBlockPos,
        block: RawBlockId,
    ) {
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    server
                        .scheduler_mut()
                        .set_block_at_world(WorldBlockPos::new(x, y, z), block);
                }
            }
        }
    }

    fn run_simulation_ticks(server: &mut IntegratedServer, ticks: usize) {
        for _ in 0..ticks {
            server.simulation_tick_report();
        }
    }

    fn setup_liquid_slope(
        server: &mut IntegratedServer,
        source_block: RawBlockId,
        fluid: FluidKind,
    ) {
        fill_blocks(
            server,
            WorldBlockPos::new(0, 78, 0),
            WorldBlockPos::new(12, 84, 4),
            AIR,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 79, 0),
            WorldBlockPos::new(12, 79, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 1),
            WorldBlockPos::new(12, 81, 1),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 3),
            WorldBlockPos::new(12, 81, 3),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 2),
            WorldBlockPos::new(0, 81, 2),
            STONE,
        );

        let source = WorldBlockPos::new(1, 80, 2);
        server
            .scheduler_mut()
            .set_block_at_world(source, source_block);
        server.schedule_fluid_tick(source, fluid, fluid.tick_delay());
    }

    fn setup_water_slope(server: &mut IntegratedServer) {
        setup_liquid_slope(server, WATER, FluidKind::Water);
    }

    fn setup_lava_slope(server: &mut IntegratedServer) {
        setup_liquid_slope(server, LAVA, FluidKind::Lava);
    }

    fn setup_cross_chunk_water_slope(server: &mut IntegratedServer) {
        fill_blocks(
            server,
            WorldBlockPos::new(14, 78, 0),
            WorldBlockPos::new(18, 84, 4),
            AIR,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(14, 79, 0),
            WorldBlockPos::new(18, 79, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(14, 80, 1),
            WorldBlockPos::new(18, 81, 1),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(14, 80, 3),
            WorldBlockPos::new(18, 81, 3),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(14, 80, 2),
            WorldBlockPos::new(14, 81, 2),
            STONE,
        );

        let source = WorldBlockPos::new(15, 80, 2);
        server.scheduler_mut().set_block_at_world(source, WATER);
        server.schedule_fluid_tick(source, FluidKind::Water, FluidKind::Water.tick_delay());
    }

    fn setup_liquid_fall(
        server: &mut IntegratedServer,
        source_block: RawBlockId,
        fluid: FluidKind,
    ) {
        fill_blocks(
            server,
            WorldBlockPos::new(0, 78, 0),
            WorldBlockPos::new(4, 88, 4),
            AIR,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 79, 0),
            WorldBlockPos::new(4, 79, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 0),
            WorldBlockPos::new(0, 88, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(4, 80, 0),
            WorldBlockPos::new(4, 88, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 0),
            WorldBlockPos::new(4, 88, 0),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 4),
            WorldBlockPos::new(4, 88, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(1, 80, 1),
            WorldBlockPos::new(3, 87, 1),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(1, 80, 3),
            WorldBlockPos::new(3, 87, 3),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(1, 80, 2),
            WorldBlockPos::new(1, 87, 2),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(3, 80, 2),
            WorldBlockPos::new(3, 87, 2),
            STONE,
        );

        let source = WorldBlockPos::new(2, 86, 2);
        server
            .scheduler_mut()
            .set_block_at_world(source, source_block);
        server.schedule_fluid_tick(source, fluid, fluid.tick_delay());
    }

    fn setup_water_fall(server: &mut IntegratedServer) {
        setup_liquid_fall(server, WATER, FluidKind::Water);
    }

    fn setup_lava_fall(server: &mut IntegratedServer) {
        setup_liquid_fall(server, LAVA, FluidKind::Lava);
    }

    fn setup_water_source_conversion(server: &mut IntegratedServer) {
        fill_blocks(
            server,
            WorldBlockPos::new(0, 78, 0),
            WorldBlockPos::new(4, 84, 4),
            AIR,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 79, 0),
            WorldBlockPos::new(4, 79, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 1),
            WorldBlockPos::new(4, 81, 1),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 3),
            WorldBlockPos::new(4, 81, 3),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 2),
            WorldBlockPos::new(0, 81, 2),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(4, 80, 2),
            WorldBlockPos::new(4, 81, 2),
            STONE,
        );

        for source in [WorldBlockPos::new(1, 80, 2), WorldBlockPos::new(3, 80, 2)] {
            server.scheduler_mut().set_block_at_world(source, WATER);
            server.schedule_fluid_tick(source, FluidKind::Water, 5);
        }
    }

    fn setup_lava_source_water_contact(server: &mut IntegratedServer) {
        fill_blocks(
            server,
            WorldBlockPos::new(0, 78, 0),
            WorldBlockPos::new(4, 84, 4),
            AIR,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 79, 0),
            WorldBlockPos::new(4, 79, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 1),
            WorldBlockPos::new(4, 81, 1),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 3),
            WorldBlockPos::new(4, 81, 3),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 2),
            WorldBlockPos::new(0, 81, 2),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(4, 80, 2),
            WorldBlockPos::new(4, 81, 2),
            STONE,
        );

        let lava = WorldBlockPos::new(1, 80, 2);
        let water = WorldBlockPos::new(2, 80, 2);
        server.scheduler_mut().set_block_at_world(lava, LAVA);
        server.schedule_fluid_tick(lava, FluidKind::Lava, FluidKind::Lava.tick_delay());
        server.scheduler_mut().set_block_at_world(water, WATER);
        server.schedule_fluid_tick(water, FluidKind::Water, FluidKind::Water.tick_delay());
        assert!(
            server.scheduler_mut().resolve_lava_source_contact_at(lava),
            "water neighbor should convert lava source to obsidian"
        );
    }

    fn fixture_i32(value: &Value, key: &str) -> i32 {
        value[key]
            .as_i64()
            .unwrap_or_else(|| panic!("fixture field `{key}` must be an integer")) as i32
    }

    fn fixture_light_layer(chunk: &Value, layer: &str) -> BTreeMap<i32, Vec<u8>> {
        chunk["light"][layer]
            .as_array()
            .unwrap_or_else(|| panic!("integration fixture light.{layer} must be an array"))
            .iter()
            .map(|section| {
                let section_y = fixture_i32(section, "y");
                let data = section["dataBase64"]
                    .as_str()
                    .unwrap_or_else(|| panic!("light.{layer}[Y={section_y}] needs dataBase64"));
                (section_y, decode_base64_light_data(data))
            })
            .collect()
    }

    fn packed_light_layer(
        sections: &[PackedLightSection],
        layer: LightLayer,
    ) -> BTreeMap<i32, Vec<u8>> {
        sections
            .iter()
            .filter_map(|section| {
                let bytes = match layer {
                    LightLayer::Sky => section.sky.as_ref(),
                    LightLayer::Block => section.block.as_ref(),
                }?;
                Some((section.section_y, bytes.clone()))
            })
            .collect()
    }

    fn assert_persisted_light_layer_matches_fixture(
        layer_name: &str,
        expected: BTreeMap<i32, Vec<u8>>,
        actual: BTreeMap<i32, Vec<u8>>,
    ) {
        let expected = normalize_persisted_light_layer(layer_name, expected);
        let actual = normalize_persisted_light_layer(layer_name, actual);
        assert_light_layer_matches_fixture(layer_name, expected, actual);
    }

    fn assert_light_layer_matches_fixture(
        layer_name: &str,
        expected: BTreeMap<i32, Vec<u8>>,
        actual: BTreeMap<i32, Vec<u8>>,
    ) {
        let expected_ys = expected.keys().copied().collect::<Vec<_>>();
        let actual_ys = actual.keys().copied().collect::<Vec<_>>();
        let report = light_layer_mismatch_report(layer_name, &expected, &actual);
        if let Some((section_y, offset, expected_byte, actual_byte)) = report.first_mismatch {
            panic!(
                "{layer_name} light mismatch: {} byte(s), {} nibble(s); first at section {section_y} byte {offset}: expected 0x{expected_byte:02x}, got 0x{actual_byte:02x}",
                report.byte_mismatches, report.nibble_mismatches,
            );
        }
        assert_eq!(
            actual_ys, expected_ys,
            "{layer_name} light section set mismatch"
        );
    }

    #[derive(Debug, Eq, PartialEq)]
    struct LightLayerMismatchReport {
        byte_mismatches: usize,
        nibble_mismatches: usize,
        first_mismatch: Option<(i32, usize, u8, u8)>,
    }

    fn light_layer_mismatch_report(
        layer_name: &str,
        expected: &BTreeMap<i32, Vec<u8>>,
        actual: &BTreeMap<i32, Vec<u8>>,
    ) -> LightLayerMismatchReport {
        let mut report = LightLayerMismatchReport {
            byte_mismatches: 0,
            nibble_mismatches: 0,
            first_mismatch: None,
        };
        for (section_y, expected_bytes) in expected {
            let Some(actual_bytes) = actual.get(section_y) else {
                continue;
            };
            assert_eq!(
                actual_bytes.len(),
                mclone_core::LIGHT_DATA_LAYER_BYTE_COUNT,
                "{layer_name} light section {section_y} has invalid native byte length",
            );
            for (offset, (expected, actual)) in
                expected_bytes.iter().zip(actual_bytes.iter()).enumerate()
            {
                if expected == actual {
                    continue;
                }
                report.byte_mismatches += 1;
                if (expected & 0x0F) != (actual & 0x0F) {
                    report.nibble_mismatches += 1;
                }
                if (expected >> 4) != (actual >> 4) {
                    report.nibble_mismatches += 1;
                }
                report
                    .first_mismatch
                    .get_or_insert((*section_y, offset, *expected, *actual));
            }
        }
        report
    }

    fn normalize_persisted_light_layer(
        layer_name: &str,
        sections: BTreeMap<i32, Vec<u8>>,
    ) -> BTreeMap<i32, Vec<u8>> {
        sections
            .into_iter()
            .filter(|(_, bytes)| {
                !bytes.iter().all(|byte| *byte == 0)
                    && (layer_name != "sky" || !bytes.iter().all(|byte| *byte == 0xFF))
            })
            .collect()
    }

    fn decode_base64_light_data(value: &str) -> Vec<u8> {
        let bytes = value.as_bytes();
        assert_eq!(
            bytes.len() % 4,
            0,
            "base64 light data length must be divisible by 4"
        );
        let mut out = Vec::with_capacity(mclone_core::LIGHT_DATA_LAYER_BYTE_COUNT);
        for quartet in bytes.chunks_exact(4) {
            let pad = quartet.iter().filter(|byte| **byte == b'=').count();
            assert!(pad <= 2, "base64 light data has too much padding");
            let a = base64_digit(quartet[0]);
            let b = base64_digit(quartet[1]);
            let c = if quartet[2] == b'=' {
                0
            } else {
                base64_digit(quartet[2])
            };
            let d = if quartet[3] == b'=' {
                0
            } else {
                base64_digit(quartet[3])
            };
            let combined = (a << 18) | (b << 12) | (c << 6) | d;
            out.push((combined >> 16) as u8);
            if pad < 2 {
                out.push((combined >> 8) as u8);
            }
            if pad < 1 {
                out.push(combined as u8);
            }
        }
        assert_eq!(
            out.len(),
            mclone_core::LIGHT_DATA_LAYER_BYTE_COUNT,
            "decoded light data must be exactly one DataLayer"
        );
        out
    }

    fn base64_digit(byte: u8) -> u32 {
        match byte {
            b'A'..=b'Z' => u32::from(byte - b'A'),
            b'a'..=b'z' => u32::from(byte - b'a' + 26),
            b'0'..=b'9' => u32::from(byte - b'0' + 52),
            b'+' => 62,
            b'/' => 63,
            _ => panic!("invalid base64 byte 0x{byte:02x} in light data"),
        }
    }

    fn fixture_palette_block_id(entry: &Value) -> RawBlockId {
        let name = entry["name"]
            .as_str()
            .expect("fixture palette entry name must be a string");
        match name {
            "minecraft:air" => AIR,
            "minecraft:stone" => STONE,
            "minecraft:obsidian" => OBSIDIAN,
            "minecraft:water" => {
                let level = entry
                    .get("properties")
                    .and_then(|properties| properties.get("level"))
                    .and_then(Value::as_str)
                    .unwrap_or("0")
                    .parse::<u8>()
                    .expect("fixture water level must be a u8");
                water_block_for_level(level).expect("fixture water level must be supported")
            }
            "minecraft:lava" => {
                let level = entry
                    .get("properties")
                    .and_then(|properties| properties.get("level"))
                    .and_then(Value::as_str)
                    .unwrap_or("0")
                    .parse::<u8>()
                    .expect("fixture lava level must be a u8");
                lava_block_for_level(level).expect("fixture lava level must be supported")
            }
            _ => panic!("unsupported liquid fixture palette entry `{name}`"),
        }
    }

    fn wait_for_scheduler_completion(scheduler: &mut ChunkScheduler) {
        let timeout = std::time::Duration::from_millis(1);
        let _ = scheduler.wait_for_worldgen_completion(timeout);
        let _ = scheduler.wait_for_light_completion(timeout);
    }

    fn wait_for_server_completion(server: &mut IntegratedServer) {
        let timeout = std::time::Duration::from_millis(1);
        let _ = server.wait_for_worldgen_completion(timeout);
        let _ = server.wait_for_light_completion(timeout);
    }

    fn active_ticket_square_count(ticket_level: i32) -> usize {
        let radius = usize::try_from(MAX_CHUNK_DISTANCE - ticket_level)
            .expect("ticket level must be within active distance");
        let side = radius * 2 + 1;
        side * side
    }

    fn square_side_for_radius(radius: u32) -> usize {
        usize::try_from(radius).expect("chunk radius must fit usize") * 2 + 1
    }

    fn player_status_counts(radius: u32) -> (usize, usize, usize, usize, usize) {
        let entity_ticking = square_side_for_radius(radius).pow(2);
        let block_ticking = square_side_for_radius(radius + 1).pow(2);
        let ticking = block_ticking - entity_ticking;
        let border_outer = square_side_for_radius(radius + 2).pow(2);
        let border = border_outer - block_ticking;
        let active = square_side_for_radius(
            radius + u32::try_from(MAX_CHUNK_DISTANCE - PLAYER_TICKET_LEVEL).unwrap(),
        )
        .pow(2);
        let inaccessible = active - border_outer;
        (inaccessible, border, ticking, entity_ticking, block_ticking)
    }

    fn chunk_square(center: ChunkPos, radius: i32) -> Vec<ChunkPos> {
        (-radius..=radius)
            .flat_map(|z| {
                (-radius..=radius).map(move |x| ChunkPos::new(center.x + x, center.z + z))
            })
            .collect()
    }

    fn without_fluid_tick_events(events: &[ChunkSchedulerEvent]) -> Vec<ChunkSchedulerEvent> {
        events
            .iter()
            .filter(|event| !matches!(event, ChunkSchedulerEvent::FluidTickScheduled { .. }))
            .cloned()
            .collect()
    }

    fn status_event_count(
        events: &[ChunkSchedulerEvent],
        status: ChunkStatus,
        step: ChunkStatusStep,
    ) -> usize {
        events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    ChunkSchedulerEvent::StatusChanged {
                        status: event_status,
                        step: event_step,
                        ..
                    } if *event_status == status && *event_step == step
                )
            })
            .count()
    }

    fn first_status_event_pos(
        events: &[ChunkSchedulerEvent],
        status: ChunkStatus,
        step: ChunkStatusStep,
    ) -> Option<ChunkPos> {
        events.iter().find_map(|event| match event {
            ChunkSchedulerEvent::StatusChanged {
                pos,
                status: event_status,
                step: event_step,
            } if *event_status == status && *event_step == step => Some(*pos),
            _ => None,
        })
    }

    fn snapshot_ready_count(events: &[ChunkSchedulerEvent]) -> usize {
        events
            .iter()
            .filter(|event| matches!(event, ChunkSchedulerEvent::SnapshotReady(_)))
            .count()
    }

    fn snapshot_update_for(updates: &[ServerUpdate], pos: ChunkPos) -> Option<&ChunkSnapshot> {
        updates.iter().rev().find_map(|update| match update {
            ServerUpdate::ChunkSnapshot(snapshot) if snapshot.pos == pos => Some(snapshot),
            _ => None,
        })
    }

    fn apply_section_updates_to_snapshot(
        snapshot: &mut ChunkSnapshot,
        updates: &[ServerUpdate],
    ) -> usize {
        let mut applied = 0;
        for update in updates {
            let ServerUpdate::SectionBlockUpdates {
                pos,
                section_y,
                updates,
            } = update
            else {
                continue;
            };
            if *pos != snapshot.pos {
                continue;
            }
            for update in updates {
                if snapshot.patch_section_block(
                    *section_y,
                    update.local_x as i32,
                    update.local_y as i32,
                    update.local_z as i32,
                    update.block_state,
                ) {
                    applied += 1;
                }
            }
        }
        applied
    }

    fn snapshot_block_state(snapshot: &ChunkSnapshot, pos: WorldBlockPos) -> BlockStateId {
        assert_eq!(snapshot.pos, pos.chunk_pos());
        assert!(
            pos.y >= snapshot.min_y && pos.y < snapshot.min_y + snapshot.height,
            "world y {} is outside snapshot range {}..{}",
            pos.y,
            snapshot.min_y,
            snapshot.min_y + snapshot.height
        );

        let section_y = block_to_section_coord(pos.y);
        let section_local_y = local_section_block_coord(pos.y);
        let local_x = local_block_coord(pos.x);
        let local_z = local_block_coord(pos.z);
        let Some(section) = snapshot
            .sections
            .iter()
            .find(|section| section.section_y == section_y)
        else {
            return BlockStateId(0);
        };
        let blocks = section.unpack_block_state_ids();
        blocks[chunk_section_index(local_x, section_local_y, local_z)]
    }

    #[test]
    fn distinguishes_integrated_and_dedicated_modes() {
        assert_ne!(ServerMode::Integrated, ServerMode::Dedicated);
    }

    #[test]
    fn chunk_scheduler_uses_platform_worldgen_mailbox() {
        let scheduler = ChunkScheduler::new(12_345);

        #[cfg(not(target_arch = "wasm32"))]
        {
            assert_eq!(
                scheduler.worldgen_mailbox_kind(),
                WorldgenMailboxKind::NativeThread
            );
            assert_eq!(
                scheduler.light_status_mailbox_kind(),
                LightStatusMailboxKind::NativeThread
            );
        }
        #[cfg(target_arch = "wasm32")]
        {
            assert_eq!(
                scheduler.worldgen_mailbox_kind(),
                WorldgenMailboxKind::Inline
            );
            assert_eq!(
                scheduler.light_status_mailbox_kind(),
                LightStatusMailboxKind::Inline
            );
        }
    }

    #[test]
    fn integrated_server_publishes_interested_chunks() {
        let mut server = IntegratedServer::new(12_345);

        let updates = handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 1,
                chunk_tracking_radius: 1,
            }),
        );

        assert_eq!(
            updates
                .iter()
                .filter(|update| matches!(update, ServerUpdate::ChunkSnapshot(_)))
                .count(),
            9
        );
        assert_eq!(
            updates
                .iter()
                .filter(|update| matches!(update, ServerUpdate::EntitySnapshot(_)))
                .count(),
            1
        );
        assert_eq!(server.loaded_chunk_count(), 25);
        assert_eq!(server.scheduler().client_visible_chunk_count(), 9);
        assert_eq!(server.scheduler().holder_count(), 29 * 29);
        assert_eq!(server.scheduler().active_ticketed_chunk_count(), 29 * 29);
        assert_eq!(
            (
                server
                    .scheduler()
                    .full_status_chunk_count(FullChunkStatus::Inaccessible),
                server
                    .scheduler()
                    .full_status_chunk_count(FullChunkStatus::Border),
                server
                    .scheduler()
                    .full_status_chunk_count(FullChunkStatus::Ticking),
                server
                    .scheduler()
                    .full_status_chunk_count(FullChunkStatus::EntityTicking),
                server.scheduler().block_ticking_chunk_count(),
                server.scheduler().entity_ticking_chunk_count(),
            ),
            (792, 24, 16, 9, 25, 9)
        );
        assert_eq!(server.scheduler().ready_dependency_chunk_count(), 23 * 23);
        assert_eq!(
            server.scheduler().metrics(),
            ChunkSchedulerMetrics {
                direct_ticket_chunks: 9,
                active_ticket_chunks: 29 * 29,
                holder_chunks: 29 * 29,
                pending_unload_chunks: 0,
                inaccessible_status_chunks: 792,
                border_status_chunks: 24,
                ticking_status_chunks: 16,
                entity_ticking_status_chunks: 9,
                block_ticking_chunks: 25,
                client_visible_chunks: 9,
                loaded_snapshot_chunks: 25,
                dependency_holder_chunks: 29 * 29 - 25,
                ready_dependency_chunks: 23 * 23,
                dirty_chunks: 25,
                pending_jobs: 0,
                completed_jobs: 1,
                total_seeded_dependency_chunks: 0,
                total_dependency_cache_hits: 0,
                total_dependency_cache_misses: 23 * 23,
                total_retained_dependency_chunks: 23 * 23,
                completed_light_statuses: 25,
                completed_light_batches: server.scheduler().metrics().completed_light_batches,
                total_light_status_compute_us: server
                    .scheduler()
                    .metrics()
                    .total_light_status_compute_us,
                max_light_status_compute_us: server
                    .scheduler()
                    .metrics()
                    .max_light_status_compute_us,
                total_light_status_world_init_us: server
                    .scheduler()
                    .metrics()
                    .total_light_status_world_init_us,
                total_light_status_active_sections_us: server
                    .scheduler()
                    .metrics()
                    .total_light_status_active_sections_us,
                total_light_status_sky_source_scan_us: server
                    .scheduler()
                    .metrics()
                    .total_light_status_sky_source_scan_us,
                total_light_status_block_source_scan_us: server
                    .scheduler()
                    .metrics()
                    .total_light_status_block_source_scan_us,
                total_light_status_engine_init_us: server
                    .scheduler()
                    .metrics()
                    .total_light_status_engine_init_us,
                total_light_status_section_setup_us: server
                    .scheduler()
                    .metrics()
                    .total_light_status_section_setup_us,
                total_light_status_sky_source_enqueue_us: server
                    .scheduler()
                    .metrics()
                    .total_light_status_sky_source_enqueue_us,
                total_light_status_block_source_enqueue_us: server
                    .scheduler()
                    .metrics()
                    .total_light_status_block_source_enqueue_us,
                total_light_status_run_updates_us: server
                    .scheduler()
                    .metrics()
                    .total_light_status_run_updates_us,
                total_light_status_run_update_iterations: server
                    .scheduler()
                    .metrics()
                    .total_light_status_run_update_iterations,
                total_light_status_block_run_update_calls: server
                    .scheduler()
                    .metrics()
                    .total_light_status_block_run_update_calls,
                total_light_status_sky_run_update_calls: server
                    .scheduler()
                    .metrics()
                    .total_light_status_sky_run_update_calls,
                total_light_status_block_run_update_processed_nodes: server
                    .scheduler()
                    .metrics()
                    .total_light_status_block_run_update_processed_nodes,
                total_light_status_sky_run_update_processed_nodes: server
                    .scheduler()
                    .metrics()
                    .total_light_status_sky_run_update_processed_nodes,
                max_light_status_block_run_update_queue_before: server
                    .scheduler()
                    .metrics()
                    .max_light_status_block_run_update_queue_before,
                max_light_status_sky_run_update_queue_before: server
                    .scheduler()
                    .metrics()
                    .max_light_status_sky_run_update_queue_before,
                final_light_status_block_run_update_queue_after: server
                    .scheduler()
                    .metrics()
                    .final_light_status_block_run_update_queue_after,
                final_light_status_sky_run_update_queue_after: server
                    .scheduler()
                    .metrics()
                    .final_light_status_sky_run_update_queue_after,
                total_light_status_block_run_updates_us: server
                    .scheduler()
                    .metrics()
                    .total_light_status_block_run_updates_us,
                total_light_status_sky_run_updates_us: server
                    .scheduler()
                    .metrics()
                    .total_light_status_sky_run_updates_us,
                total_light_status_collect_sections_us: server
                    .scheduler()
                    .metrics()
                    .total_light_status_collect_sections_us,
            }
        );
        assert_eq!(server.scheduler().job_count(), 1);
        let job = server.scheduler().jobs().next().unwrap();
        assert_eq!(job.id, ChunkJobId(1));
        assert_eq!(job.status, ChunkStatus::Features);
        assert_eq!(job.state, ChunkJobState::Complete);
        assert_eq!(job.target_chunks.len(), 25);
        assert_eq!(job.feature_centers.len(), 7 * 7);
        assert_eq!(job.dependency_chunks.len(), 23 * 23);
        assert_eq!(job.seeded_dependency_chunks, 0);
        assert_eq!(job.dependency_cache_hits, 0);
        assert_eq!(job.dependency_cache_misses, 23 * 23);
        assert_eq!(job.retained_dependency_chunks, 23 * 23);
        assert!(job.dependency_chunks.contains(&ChunkPos::new(-11, -11)));
        assert!(job.dependency_chunks.contains(&ChunkPos::new(11, 11)));
        for target in &job.target_chunks {
            assert_eq!(
                server
                    .scheduler()
                    .holder(*target)
                    .unwrap()
                    .status_slot(ChunkStatus::Features)
                    .unwrap()
                    .job_id,
                Some(job.id)
            );
        }
        assert!(updates.iter().all(|update| {
            matches!(
                update,
                ServerUpdate::ChunkSnapshot(_) | ServerUpdate::EntitySnapshot(_)
            )
        }));
    }

    #[test]
    fn player_ticket_levels_define_runtime_status_lanes() {
        let mut scheduler = ChunkScheduler::new(12_345);

        scheduler
            .apply_interest(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
            .unwrap();

        assert_eq!(player_status_counts(0), (704, 16, 8, 1, 9));
        assert_eq!(
            (
                scheduler.full_status_chunk_count(FullChunkStatus::Inaccessible),
                scheduler.full_status_chunk_count(FullChunkStatus::Border),
                scheduler.full_status_chunk_count(FullChunkStatus::Ticking),
                scheduler.full_status_chunk_count(FullChunkStatus::EntityTicking),
                scheduler.block_ticking_chunk_count(),
                scheduler.entity_ticking_chunk_count(),
            ),
            (704, 16, 8, 1, 9, 1)
        );
        assert_eq!(
            scheduler
                .holder(ChunkPos::new(0, 0))
                .map(ChunkHolder::full_status),
            Some(FullChunkStatus::EntityTicking)
        );
        assert_eq!(
            scheduler
                .holder(ChunkPos::new(1, 0))
                .map(ChunkHolder::full_status),
            Some(FullChunkStatus::Ticking)
        );
        assert_eq!(
            scheduler
                .holder(ChunkPos::new(2, 0))
                .map(ChunkHolder::full_status),
            Some(FullChunkStatus::Border)
        );
        assert_eq!(
            scheduler
                .holder(ChunkPos::new(3, 0))
                .map(ChunkHolder::full_status),
            Some(FullChunkStatus::Inaccessible)
        );
        assert!(scheduler.holder(ChunkPos::new(14, 0)).is_none());
    }

    #[test]
    fn client_visibility_is_separate_from_ticking_status() {
        let mut scheduler = ChunkScheduler::new(12_345);

        apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );
        assert!(
            scheduler
                .holder(ChunkPos::new(0, 0))
                .unwrap()
                .is_client_visible()
        );

        apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );

        let old_center = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
        assert!(!old_center.is_client_visible());
        assert_eq!(old_center.full_status(), FullChunkStatus::Ticking);
        assert_eq!(scheduler.client_visible_chunk_count(), 1);
        assert_eq!(scheduler.entity_ticking_chunk_count(), 1);
        assert_eq!(scheduler.block_ticking_chunk_count(), 9);
    }

    #[test]
    fn scheduler_tick_report_lists_runtime_lanes_without_simulation() {
        let mut scheduler = ChunkScheduler::new(12_345);

        apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );

        let report = scheduler.tick_report().unwrap();

        assert_eq!(report.ticket_tick, 1);
        assert_eq!(
            report.block_ticking_chunks,
            chunk_square(ChunkPos::new(0, 0), 1)
        );
        assert_eq!(report.entity_ticking_chunks, vec![ChunkPos::new(0, 0)]);
        assert_eq!(report.pending_unloads_processed, 0);
        assert!(report.events.is_empty());
    }

    #[test]
    fn integrated_server_tick_report_exposes_protocol_updates_and_lanes() {
        let mut server = IntegratedServer::new(12_345);

        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );

        let report = server.tick_report();

        assert_eq!(report.ticket_tick, 1);
        assert_eq!(
            report.block_ticking_chunks,
            chunk_square(ChunkPos::new(0, 0), 1)
        );
        assert_eq!(report.entity_ticking_chunks, vec![ChunkPos::new(0, 0)]);
        assert_eq!(report.pending_unloads_processed, 0);
        assert!(report.updates.is_empty());
    }

    #[test]
    fn integrated_server_simulation_tick_report_records_phases() {
        let mut server = IntegratedServer::new(12_345);

        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        let before_metrics = server.scheduler().metrics();

        let report = server.simulation_tick_report();

        assert_eq!(report.simulation_tick, 1);
        assert_eq!(report.chunk_tick, 1);
        assert_eq!(report.block_tick_chunks, 9);
        assert!(report.fluid_ticks_executed > 0);
        assert_eq!(report.entity_tick_chunks, 1);
        #[cfg(not(feature = "physics-rapier"))]
        assert_eq!(report.physics, ServerPhysicsTickDiagnostics::default());
        #[cfg(feature = "physics-rapier")]
        {
            assert!(report.physics.enabled);
            assert_eq!(report.physics.body_count, 0);
            assert_eq!(report.physics.collider_count, 0);
            assert!(!report.physics.test_cube_spawned);
        }
        assert_eq!(report.pending_unloads_processed, 0);
        assert_eq!(
            server.scheduler().metrics().block_ticking_chunks,
            before_metrics.block_ticking_chunks
        );
    }

    #[test]
    fn fluid_tick_list_dedupes_position_and_fluid() {
        let mut ticks = FluidTickList::new();
        let pos = WorldBlockPos::new(8, 120, 8);

        ticks.schedule_tick(pos, FluidKind::Water, 5, 10);
        ticks.schedule_tick(pos, FluidKind::Water, 1, 10);
        ticks.schedule_tick(pos, FluidKind::Lava, 1, 10);

        assert_eq!(ticks.size(), 2);
        assert!(ticks.has_scheduled_tick(pos, FluidKind::Water));
        assert!(ticks.has_scheduled_tick(pos, FluidKind::Lava));
    }

    #[test]
    fn fluid_kind_accepts_generated_and_persisted_tick_targets() {
        assert_eq!(
            FluidKind::from_target("minecraft:water"),
            Some(FluidKind::Water)
        );
        assert_eq!(
            FluidKind::from_target("minecraft:flowing_water"),
            Some(FluidKind::Water)
        );
        assert_eq!(
            FluidKind::from_target("minecraft:lava"),
            Some(FluidKind::Lava)
        );
        assert_eq!(
            FluidKind::from_target("minecraft:flowing_lava"),
            Some(FluidKind::Lava)
        );
        assert_eq!(FluidKind::from_target("minecraft:empty"), None);
    }

    #[test]
    fn generated_liquid_ticks_are_registered_when_watery_chunk_is_published() {
        let mut server = IntegratedServer::new(12_345);
        let center = ChunkPos::new(117, -128);

        let updates = handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center,
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );

        assert!(
            snapshot_update_for(&updates, center).is_some(),
            "watery oracle chunk should publish a visible snapshot"
        );
        let scheduled_before_tick = server.scheduled_fluid_tick_count();
        assert!(
            scheduled_before_tick >= 90,
            "seed 12345 chunk (117,-128) should carry at least the 90 liquid-carved oracle ticks"
        );

        let report = server.simulation_tick_report();

        assert!(
            report.fluid_ticks_executed > 0,
            "generated liquid ticks should execute once the chunk is entity-ticking"
        );
        assert!(
            report.fluid_ticks_executed < scheduled_before_tick,
            "only liquid ticks in entity-ticking chunks should execute immediately; ticking-lane neighbors remain pending"
        );
    }

    #[test]
    fn scheduled_water_tick_spreads_down_and_publishes_section_update() {
        let mut server = IntegratedServer::new(12_345);
        let initial_updates = handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        let mut client_snapshot = snapshot_update_for(&initial_updates, ChunkPos::new(0, 0))
            .expect("initial interest should publish visible chunk snapshot")
            .clone();
        let source = WorldBlockPos::new(8, 120, 8);
        let below = source.below();

        server.scheduler_mut().set_block_at_world(source, WATER);
        server.scheduler_mut().set_block_at_world(below, AIR);
        server.schedule_fluid_tick(source, FluidKind::Water, 0);

        let report = server.simulation_tick_report();

        assert!(report.fluid_ticks_executed >= 1);
        assert!(report.fluid_mutated_blocks >= 1);
        assert!(report.scheduled_fluid_ticks >= 1);
        assert_eq!(
            server.scheduler().block_at_world(below),
            Some(WATER_LEVEL_8)
        );
        assert_eq!(
            report.fluid_snapshot_events, 0,
            "fluid mutation should no longer publish per-block chunk snapshots"
        );
        assert!(
            snapshot_update_for(&report.updates, ChunkPos::new(0, 0)).is_none(),
            "fluid mutation should publish section deltas instead of chunk snapshots"
        );
        assert!(
            apply_section_updates_to_snapshot(&mut client_snapshot, &report.updates) >= 1,
            "fluid mutation should publish at least one section block update"
        );
        assert_eq!(
            snapshot_block_state(&client_snapshot, source),
            generated_block_state_id(WATER)
        );
        assert_eq!(
            snapshot_block_state(&client_snapshot, below),
            generated_block_state_id(WATER_LEVEL_8)
        );
    }

    #[test]
    fn scheduled_water_slope_matches_oracle_fixture_after_5_ticks() {
        let mut server = new_liquid_oracle_server();
        setup_water_slope(&mut server);

        run_simulation_ticks(&mut server, 5);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(1, 80, 2)),
            Some(WATER)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 80, 2)),
            Some(WATER_LEVEL_1)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(3, 80, 2)),
            Some(AIR)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/water-slope-5-ticks.json"),
        );
    }

    #[test]
    fn scheduled_water_slope_matches_oracle_fixture_after_10_ticks() {
        let mut server = new_liquid_oracle_server();
        setup_water_slope(&mut server);

        run_simulation_ticks(&mut server, 10);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(1, 80, 2)),
            Some(WATER)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 80, 2)),
            Some(WATER_LEVEL_1)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(3, 80, 2)),
            Some(WATER_LEVEL_2)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(4, 80, 2)),
            Some(AIR)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/water-slope-10-ticks.json"),
        );
    }

    #[test]
    fn scheduled_water_cross_chunk_slope_matches_oracle_fixture_after_10_ticks() {
        let mut server = new_liquid_oracle_server_with_radius(1);
        setup_cross_chunk_water_slope(&mut server);

        run_simulation_ticks(&mut server, 10);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(15, 80, 2)),
            Some(WATER)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(16, 80, 2)),
            Some(WATER_LEVEL_1)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(17, 80, 2)),
            Some(WATER_LEVEL_2)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/water-cross-chunk-slope-10-ticks.json"),
        );
    }

    #[test]
    fn cross_chunk_water_tick_waits_until_neighbor_is_entity_ticking() {
        let mut server = new_liquid_oracle_server();
        setup_cross_chunk_water_slope(&mut server);

        run_simulation_ticks(&mut server, 10);

        assert_eq!(
            server
                .scheduler()
                .holder(ChunkPos::new(1, 0))
                .unwrap()
                .full_status(),
            FullChunkStatus::Ticking
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(16, 80, 2)),
            Some(WATER_LEVEL_1),
            "entity-ticking chunk (0,0) should mutate the loaded neighbor chunk"
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(17, 80, 2)),
            Some(AIR),
            "non-entity-ticking chunk (1,0) should not run its due follow-up tick yet"
        );
        let pending_ticks = server
            .liquid_ticks
            .scheduled_tick_entries(server.simulation_tick());
        assert!(
            pending_ticks.contains(&(WorldBlockPos::new(16, 80, 2), FluidKind::Water, 0)),
            "the cross-chunk follow-up tick should remain due and pending: {pending_ticks:?}"
        );

        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        run_simulation_ticks(&mut server, 1);

        assert_eq!(
            server
                .scheduler()
                .holder(ChunkPos::new(1, 0))
                .unwrap()
                .full_status(),
            FullChunkStatus::EntityTicking
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(17, 80, 2)),
            Some(WATER_LEVEL_2),
            "the due cross-chunk tick should run after the neighbor becomes entity-ticking"
        );
    }

    #[test]
    fn scheduled_water_fall_matches_oracle_fixture_after_10_ticks() {
        let mut server = new_liquid_oracle_server();
        setup_water_fall(&mut server);

        run_simulation_ticks(&mut server, 10);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 86, 2)),
            Some(WATER)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 85, 2)),
            Some(WATER_LEVEL_8)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 84, 2)),
            Some(WATER_LEVEL_8)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/water-fall-10-ticks.json"),
        );
    }

    #[test]
    fn scheduled_water_source_conversion_matches_oracle_fixture_after_5_ticks() {
        let mut server = new_liquid_oracle_server();
        setup_water_source_conversion(&mut server);

        run_simulation_ticks(&mut server, 5);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 80, 2)),
            Some(WATER)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/water-source-conversion-5-ticks.json"),
        );
    }

    #[test]
    fn scheduled_lava_slope_matches_oracle_fixture_after_30_ticks() {
        let mut server = new_liquid_oracle_server();
        setup_lava_slope(&mut server);

        run_simulation_ticks(&mut server, 30);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(1, 80, 2)),
            Some(LAVA)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 80, 2)),
            Some(LAVA_LEVEL_2)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/lava-slope-30-ticks.json"),
        );
    }

    #[test]
    fn scheduled_lava_fall_matches_oracle_fixture_after_60_ticks() {
        let mut server = new_liquid_oracle_server();
        setup_lava_fall(&mut server);

        run_simulation_ticks(&mut server, 60);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 86, 2)),
            Some(LAVA)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 85, 2)),
            Some(LAVA_LEVEL_8)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 84, 2)),
            Some(LAVA_LEVEL_8)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/lava-fall-60-ticks.json"),
        );
    }

    #[test]
    fn lava_source_water_contact_matches_oracle_fixture_after_1_script_tick() {
        let mut server = new_liquid_oracle_server();
        setup_lava_source_water_contact(&mut server);

        run_simulation_ticks(&mut server, 2);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(1, 80, 2)),
            Some(OBSIDIAN)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 80, 2)),
            Some(WATER)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/lava-source-water-contact-1-ticks.json"),
        );
    }

    #[test]
    fn scheduled_fluid_tick_waits_until_chunk_is_entity_ticking() {
        let mut server = IntegratedServer::new(12_345);
        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        let source = WorldBlockPos::new(8, 120, 8);
        let below = source.below();
        server.scheduler_mut().set_block_at_world(source, WATER);
        server.scheduler_mut().set_block_at_world(below, AIR);
        server.schedule_fluid_tick(source, FluidKind::Water, 0);

        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        let report = server.simulation_tick_report();

        assert_eq!(
            server
                .scheduler()
                .holder(ChunkPos::new(0, 0))
                .unwrap()
                .full_status(),
            FullChunkStatus::Ticking
        );
        assert!(
            report.scheduled_fluid_ticks >= 1,
            "the manually scheduled tick in the old non-entity-ticking chunk should remain pending"
        );
        assert_ne!(server.scheduler().block_at_world(below), Some(WATER));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn scheduled_fluid_tick_survives_fully_unloaded_chunk_until_reload() {
        let root =
            unique_temp_dir("scheduled_fluid_tick_survives_fully_unloaded_chunk_until_reload");
        let original_interest = ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        };
        let source = WorldBlockPos::new(8, 120, 8);
        let below = source.below();
        let mut server = IntegratedServer::with_chunk_store(
            12_345,
            Box::new(FilesystemChunkSnapshotStore::new(&root)),
        );
        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(original_interest.clone()),
        );
        server.liquid_ticks = FluidTickList::new();
        server.scheduler_mut().set_block_at_world(source, WATER);
        server.scheduler_mut().set_block_at_world(below, AIR);

        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(100, 100),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        server
            .scheduler_mut()
            .process_pending_unloads(usize::MAX)
            .unwrap();
        assert!(
            server.scheduler().holder(ChunkPos::new(0, 0)).is_none(),
            "the original chunk should be fully removed from scheduler holders"
        );

        server.liquid_ticks = FluidTickList::new();
        server.schedule_fluid_tick(source, FluidKind::Water, 0);
        let deferred = server.simulation_tick_report();

        assert_eq!(deferred.fluid_ticks_executed, 0);
        assert_eq!(deferred.deferred_fluid_ticks, 1);
        assert_eq!(deferred.scheduled_fluid_ticks, 1);
        assert!(
            server
                .liquid_ticks
                .has_scheduled_tick(source, FluidKind::Water),
            "a due tick outside any entity-ticking holder should stay pending"
        );

        let updates =
            handle_command_and_poll(&mut server, ClientCommand::SetChunkView(original_interest));
        assert!(
            snapshot_update_for(&updates, ChunkPos::new(0, 0)).is_some(),
            "the original chunk should reload from the snapshot store"
        );
        let executed = server.simulation_tick_report();

        assert_eq!(executed.deferred_fluid_ticks, 0);
        assert_eq!(executed.fluid_ticks_executed, 1);
        assert_eq!(
            server.scheduler().block_at_world(below),
            Some(WATER_LEVEL_8),
            "the overdue fluid tick should run after the chunk becomes entity-ticking again"
        );
        let pending_after_execution = server
            .liquid_ticks
            .scheduled_tick_entries(server.simulation_tick());
        assert!(
            !pending_after_execution.contains(&(source, FluidKind::Water, 0)),
            "the overdue zero-delay source tick should be consumed: {pending_after_execution:?}"
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn integrated_server_tick_uses_simulation_layer() {
        let mut server = IntegratedServer::new(12_345);

        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );

        assert_eq!(server.simulation_tick(), 0);
        let _ = server.tick();
        assert_eq!(server.simulation_tick(), 1);
        let _ = server.tick();
        assert_eq!(server.simulation_tick(), 2);
    }

    #[test]
    fn scheduler_tick_report_counts_bounded_pending_unload_work() {
        let mut scheduler = ChunkScheduler::new(12_345);

        scheduler
            .add_region_ticket(ChunkTicketType::Unknown, ChunkPos::new(0, 0), 0)
            .unwrap();
        poll_scheduler_until_idle(&mut scheduler);

        let first_report = scheduler.tick_report().unwrap();
        assert_eq!(first_report.ticket_tick, 1);
        assert_eq!(first_report.pending_unloads_processed, 0);
        assert!(first_report.block_ticking_chunks.is_empty());
        assert!(first_report.entity_ticking_chunks.is_empty());
        assert!(first_report.events.is_empty());

        let second_report = scheduler.tick_report().unwrap();
        assert_eq!(second_report.ticket_tick, 2);
        assert_eq!(
            second_report.pending_unloads_processed,
            DEFAULT_PENDING_UNLOAD_BUDGET
        );
        assert!(second_report.block_ticking_chunks.is_empty());
        assert!(second_report.entity_ticking_chunks.is_empty());
        assert!(second_report.events.is_empty());
        assert_eq!(
            scheduler.pending_unload_count(),
            active_ticket_square_count(CHUNK_LEVEL_FULL) - DEFAULT_PENDING_UNLOAD_BUDGET
        );
    }

    #[test]
    fn duplicate_interest_does_not_regenerate_loaded_chunks() {
        let mut server = IntegratedServer::new(12_345);
        let interest = ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        };

        let first_updates =
            handle_command_and_poll(&mut server, ClientCommand::SetChunkView(interest.clone()));
        assert!(snapshot_update_for(&first_updates, ChunkPos::new(0, 0)).is_some());
        assert_eq!(server.scheduler().job_count(), 1);
        assert_eq!(
            handle_command_and_poll(&mut server, ClientCommand::SetChunkView(interest)).len(),
            0
        );
        assert_eq!(server.scheduler().job_count(), 1);
    }

    #[test]
    fn adjacent_interest_reuses_retained_dependency_chunks() {
        let mut server = IntegratedServer::new(12_345);

        let first_updates = handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        assert!(snapshot_update_for(&first_updates, ChunkPos::new(0, 0)).is_some());
        assert_eq!(
            server.scheduler().job(ChunkJobId(1)).map(|job| (
                job.seeded_dependency_chunks,
                job.dependency_cache_hits,
                job.dependency_cache_misses,
                job.retained_dependency_chunks
            )),
            Some((0, 0, 21 * 21, 21 * 21))
        );

        let moved_updates = handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        assert!(snapshot_update_for(&moved_updates, ChunkPos::new(1, 0)).is_some());
        assert_eq!(
            server.scheduler().job(ChunkJobId(2)).map(|job| (
                job.seeded_dependency_chunks,
                job.dependency_cache_hits,
                job.dependency_cache_misses,
                job.retained_dependency_chunks
            )),
            Some((18 * 21, 18 * 21, 21, 19 * 21))
        );
    }

    #[test]
    fn chunk_scheduler_apply_interest_enqueues_features_before_poll() {
        let mut scheduler = ChunkScheduler::new(12_345);

        let events = scheduler
            .apply_interest(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
            .unwrap();

        assert_eq!(scheduler.pending_job_count(), 1);
        assert_eq!(scheduler.loaded_chunk_count(), 0);
        assert_eq!(
            scheduler.job(ChunkJobId(1)).map(|job| job.state),
            Some(ChunkJobState::Running)
        );
        assert!(
            events
                .iter()
                .all(|event| !matches!(event, ChunkSchedulerEvent::SnapshotReady(_)))
        );
        assert_eq!(events.len(), 9 * 6);
        assert_eq!(
            status_event_count(&events, ChunkStatus::Features, ChunkStatusStep::Scheduled),
            9
        );
        assert_eq!(
            status_event_count(&events, ChunkStatus::Light, ChunkStatusStep::Scheduled),
            9
        );

        let ready_events = poll_scheduler_until_idle(&mut scheduler);
        assert_eq!(scheduler.pending_job_count(), 0);
        assert_eq!(scheduler.loaded_chunk_count(), 9);
        assert!(
            ready_events
                .iter()
                .any(|event| matches!(event, ChunkSchedulerEvent::FluidTickScheduled { .. }))
        );
        let ready_events_without_fluid = without_fluid_tick_events(&ready_events);
        assert_eq!(
            status_event_count(
                &ready_events_without_fluid,
                ChunkStatus::Features,
                ChunkStatusStep::Ready
            ),
            9
        );
        assert_eq!(
            status_event_count(
                &ready_events_without_fluid,
                ChunkStatus::Light,
                ChunkStatusStep::Ready
            ),
            9
        );
        assert_eq!(snapshot_ready_count(&ready_events_without_fluid), 1);
        assert!(ready_events_without_fluid.iter().any(|event| matches!(
            event,
            ChunkSchedulerEvent::SnapshotReady(snapshot)
                if snapshot.status == ChunkStatus::Light && snapshot.light_correct
        )));
    }

    #[test]
    fn chunk_scheduler_can_publish_features_when_lighting_disabled() {
        let mut scheduler = ChunkScheduler::new(12_345);
        scheduler.set_lighting_enabled(false);

        let events = apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );

        assert_eq!(scheduler.pending_job_count(), 0);
        assert_eq!(scheduler.loaded_chunk_count(), 9);
        assert_eq!(
            status_event_count(&events, ChunkStatus::Features, ChunkStatusStep::Scheduled),
            9
        );
        assert_eq!(
            status_event_count(&events, ChunkStatus::Features, ChunkStatusStep::Ready),
            9
        );
        assert_eq!(
            status_event_count(&events, ChunkStatus::Light, ChunkStatusStep::Scheduled),
            0
        );
        assert_eq!(
            status_event_count(&events, ChunkStatus::Light, ChunkStatusStep::Ready),
            0
        );
        assert!(events.iter().any(|event| matches!(
            event,
            ChunkSchedulerEvent::SnapshotReady(snapshot)
                if snapshot.pos == ChunkPos::new(0, 0)
                    && snapshot.status == ChunkStatus::Features
                    && !snapshot.light_correct
                    && snapshot.light_sections.is_empty()
        )));

        let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(holder.target_status(), Some(ChunkStatus::Features));
        assert!(holder.status_slot(ChunkStatus::Light).is_none());
    }

    #[test]
    fn chunk_scheduler_poll_slices_completed_publication() {
        let mut scheduler = ChunkScheduler::new(12_345);
        scheduler
            .apply_interest(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
            .unwrap();
        assert!(scheduler.wait_for_worldgen_completion(std::time::Duration::from_secs(30)));

        let first_events = scheduler.poll().unwrap();

        assert_eq!(
            scheduler.loaded_chunk_count(),
            DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET
        );
        assert!(scheduler.pending_publication_count() > 0);
        assert_eq!(scheduler.pending_job_count(), 2);
        assert!(
            without_fluid_tick_events(&first_events)
                .iter()
                .filter(|event| matches!(event, ChunkSchedulerEvent::StatusChanged { .. }))
                .count()
                <= DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET * 2
        );

        poll_scheduler_until_idle(&mut scheduler);
        assert_eq!(scheduler.pending_publication_count(), 0);
        assert_eq!(scheduler.pending_job_count(), 0);
        assert_eq!(scheduler.loaded_chunk_count(), 9);
    }

    #[test]
    fn chunk_interest_updates_player_tickets_and_holder_levels() {
        let mut scheduler = ChunkScheduler::new(12_345);

        let events = scheduler
            .apply_interest(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
            .unwrap();

        assert_eq!(scheduler.ticketed_chunk_count(), 1);
        assert_eq!(scheduler.ticket_count_at(ChunkPos::new(0, 0)), 1);
        assert_eq!(
            scheduler.ticket_level_at(ChunkPos::new(0, 0)),
            PLAYER_TICKET_LEVEL
        );
        let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(holder.ticket_level(), PLAYER_TICKET_LEVEL);
        assert_eq!(holder.full_status(), FullChunkStatus::EntityTicking);
        assert!(holder.full_status().is_or_after(FullChunkStatus::Ticking));
        assert_eq!(events.len(), 9 * 6);

        let moved_events = scheduler
            .apply_interest(ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
            .unwrap();

        assert_eq!(scheduler.ticketed_chunk_count(), 1);
        assert_eq!(scheduler.ticket_count_at(ChunkPos::new(0, 0)), 0);
        assert_eq!(scheduler.ticket_count_at(ChunkPos::new(1, 0)), 1);
        assert_eq!(
            scheduler.ticket_level_at(ChunkPos::new(0, 0)),
            UNLOADED_CHUNK_LEVEL
        );
        assert_eq!(
            scheduler.active_ticket_level_at(ChunkPos::new(0, 0)),
            PLAYER_TICKET_LEVEL + 1
        );
        assert!(scheduler.holder(ChunkPos::new(0, 0)).is_some());
        assert_eq!(
            scheduler
                .holder(ChunkPos::new(1, 0))
                .map(ChunkHolder::ticket_level),
            Some(PLAYER_TICKET_LEVEL)
        );
        assert!(
            moved_events
                .iter()
                .all(|event| !matches!(event, ChunkSchedulerEvent::Unloaded { .. }))
        );
    }

    #[test]
    fn duplicate_interest_while_job_running_does_not_enqueue_second_job() {
        let mut scheduler = ChunkScheduler::new(12_345);
        let interest = ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        };

        let first_events = scheduler.apply_interest(interest.clone()).unwrap();
        assert_eq!(first_events.len(), 9 * 6);
        assert_eq!(scheduler.pending_job_count(), 1);
        assert_eq!(scheduler.job_count(), 1);

        let second_events = scheduler.apply_interest(interest).unwrap();
        assert_eq!(second_events, Vec::new());
        assert_eq!(scheduler.pending_job_count(), 1);
        assert_eq!(scheduler.job_count(), 1);

        assert_eq!(
            without_fluid_tick_events(&poll_scheduler_until_idle(&mut scheduler)).len(),
            19
        );
        assert_eq!(scheduler.loaded_chunk_count(), 9);
        assert_eq!(scheduler.job_count(), 1);
    }

    #[test]
    fn forced_ticket_keeps_chunk_resident_after_interest_moves() {
        let mut scheduler = ChunkScheduler::new(12_345);

        apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );
        assert!(
            scheduler
                .set_chunk_forced(ChunkPos::new(0, 0), true)
                .unwrap()
                .is_empty()
        );
        assert_eq!(scheduler.ticket_count_at(ChunkPos::new(0, 0)), 2);
        assert_eq!(
            scheduler.ticket_level_at(ChunkPos::new(0, 0)),
            FORCED_TICKET_LEVEL
        );

        let events = apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );

        assert!(events.iter().any(
            |event| matches!(event, ChunkSchedulerEvent::Unloaded { pos } if *pos == ChunkPos::new(0, 0))
        ));
        assert!(scheduler.holder(ChunkPos::new(0, 0)).is_some());
        assert!(scheduler.holder(ChunkPos::new(1, 0)).is_some());
        assert_eq!(scheduler.loaded_chunk_count(), 12);

        let events = scheduler
            .set_chunk_forced(ChunkPos::new(0, 0), false)
            .unwrap();

        assert!(events.is_empty());
        assert_eq!(scheduler.ticket_count_at(ChunkPos::new(0, 0)), 0);
        assert_eq!(
            scheduler.active_ticket_level_at(ChunkPos::new(0, 0)),
            PLAYER_TICKET_LEVEL + 1
        );
        assert!(scheduler.holder(ChunkPos::new(0, 0)).is_some());
    }

    #[test]
    fn stale_unknown_ticket_expires_on_scheduler_tick() {
        let mut scheduler = ChunkScheduler::new(12_345);

        let events = scheduler
            .add_region_ticket(ChunkTicketType::Unknown, ChunkPos::new(0, 0), 0)
            .unwrap();
        assert_eq!(events.len(), 6);
        assert_eq!(scheduler.ticketed_chunk_count(), 1);
        assert_eq!(scheduler.ticket_level_at(ChunkPos::new(0, 0)), 33);
        assert_eq!(
            without_fluid_tick_events(&poll_scheduler_until_idle(&mut scheduler)).len(),
            2
        );
        assert_eq!(scheduler.loaded_chunk_count(), 1);
        assert_eq!(scheduler.client_visible_chunk_count(), 0);

        assert!(scheduler.tick().unwrap().is_empty());
        assert_eq!(scheduler.ticket_tick(), 1);
        assert_eq!(scheduler.ticketed_chunk_count(), 1);

        let events = scheduler.tick().unwrap();

        assert_eq!(scheduler.ticket_tick(), 2);
        assert_eq!(scheduler.ticketed_chunk_count(), 0);
        assert!(events.is_empty());
        assert_eq!(
            scheduler.pending_unload_count(),
            active_ticket_square_count(CHUNK_LEVEL_FULL) - DEFAULT_PENDING_UNLOAD_BUDGET
        );

        scheduler.process_pending_unloads(usize::MAX).unwrap();
        assert_eq!(scheduler.pending_unload_count(), 0);
        assert!(scheduler.holder(ChunkPos::new(0, 0)).is_none());
    }

    #[test]
    fn expired_ticket_queues_pending_unload_until_processed() {
        let mut scheduler = ChunkScheduler::new(12_345);
        let pos = ChunkPos::new(0, 0);

        scheduler
            .add_region_ticket(ChunkTicketType::Unknown, pos, 0)
            .unwrap();
        poll_scheduler_until_idle(&mut scheduler);
        let active_count = active_ticket_square_count(CHUNK_LEVEL_FULL);
        assert_eq!(scheduler.holder_count(), active_count);
        assert_eq!(scheduler.pending_unload_count(), 0);
        assert_eq!(scheduler.loaded_chunk_count(), 1);

        scheduler.distance_manager.purge_stale_tickets();
        scheduler.distance_manager.purge_stale_tickets();
        assert!(scheduler.reconcile_ticketed_holders().unwrap().is_empty());

        assert_eq!(scheduler.ticketed_chunk_count(), 0);
        assert_eq!(scheduler.pending_unload_count(), active_count);
        assert_eq!(scheduler.holder_count(), active_count);
        assert!(scheduler.is_pending_unload(pos));
        assert_eq!(
            scheduler.holder(pos).map(ChunkHolder::ticket_level),
            Some(UNLOADED_CHUNK_LEVEL)
        );
        assert_eq!(scheduler.loaded_chunk_count(), 1);

        assert_eq!(scheduler.process_pending_unloads(10).unwrap(), 10);
        assert_eq!(scheduler.pending_unload_count(), active_count - 10);

        assert_eq!(
            scheduler.process_pending_unloads(usize::MAX).unwrap(),
            active_count - 10
        );
        assert_eq!(scheduler.pending_unload_count(), 0);
        assert_eq!(scheduler.holder_count(), 0);
        assert_eq!(scheduler.loaded_chunk_count(), 0);
        assert_eq!(scheduler.dirty_chunk_count(), 0);
        assert!(scheduler.holder(pos).is_none());
    }

    #[test]
    fn pending_unload_holder_is_rescued_when_ticket_returns() {
        let mut scheduler = ChunkScheduler::new(12_345);
        let pos = ChunkPos::new(0, 0);

        scheduler
            .add_region_ticket(ChunkTicketType::Unknown, pos, 0)
            .unwrap();
        poll_scheduler_until_idle(&mut scheduler);

        scheduler.distance_manager.purge_stale_tickets();
        scheduler.distance_manager.purge_stale_tickets();
        scheduler.reconcile_ticketed_holders().unwrap();
        assert!(scheduler.is_pending_unload(pos));
        assert_eq!(
            scheduler.holder(pos).map(ChunkHolder::ticket_level),
            Some(UNLOADED_CHUNK_LEVEL)
        );

        assert!(!scheduler.set_chunk_forced(pos, true).unwrap().is_empty());
        poll_scheduler_until_idle(&mut scheduler);

        assert!(!scheduler.is_pending_unload(pos));
        assert_eq!(scheduler.pending_unload_count(), 0);
        assert_eq!(
            scheduler.holder(pos).map(ChunkHolder::ticket_level),
            Some(FORCED_TICKET_LEVEL)
        );
        assert_eq!(scheduler.loaded_chunk_count(), 9);
        assert_eq!(scheduler.process_pending_unloads(usize::MAX).unwrap(), 0);
        assert!(scheduler.holder(pos).is_some());
    }

    #[test]
    fn chunk_scheduler_records_holder_status_slots_in_order() {
        let mut scheduler = ChunkScheduler::new(12_345);

        let events = apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );

        assert_eq!(
            status_event_count(&events, ChunkStatus::Features, ChunkStatusStep::Scheduled),
            9
        );
        assert_eq!(
            status_event_count(&events, ChunkStatus::Light, ChunkStatusStep::Scheduled),
            9
        );
        assert_eq!(
            status_event_count(&events, ChunkStatus::Features, ChunkStatusStep::Ready),
            9
        );
        assert_eq!(
            status_event_count(&events, ChunkStatus::Light, ChunkStatusStep::Ready),
            9
        );
        assert!(events.iter().any(|event| matches!(
            event,
            ChunkSchedulerEvent::SnapshotReady(snapshot)
                if snapshot.pos == ChunkPos::new(0, 0)
                    && snapshot.status == ChunkStatus::Light
                    && snapshot.light_correct
        )));

        let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(holder.target_status(), Some(ChunkStatus::Light));
        assert_eq!(holder.ready_status_count(), 4);
        assert_eq!(
            holder.status_slot(ChunkStatus::Terrain),
            Some(&ChunkStatusSlot {
                status: ChunkStatus::Terrain,
                step: ChunkStatusStep::Ready,
                revision: None,
                job_id: None,
            })
        );
        assert_eq!(
            holder.status_slot(ChunkStatus::Surface),
            Some(&ChunkStatusSlot {
                status: ChunkStatus::Surface,
                step: ChunkStatusStep::Ready,
                revision: None,
                job_id: None,
            })
        );
        let features_slot = holder.status_slot(ChunkStatus::Features).unwrap();
        assert_eq!(features_slot.status, ChunkStatus::Features);
        assert_eq!(features_slot.step, ChunkStatusStep::Ready);
        assert_eq!(features_slot.job_id, Some(ChunkJobId(1)));
        let feature_revision = features_slot
            .revision
            .expect("features should have revision");

        let light_slot = holder.status_slot(ChunkStatus::Light).unwrap();
        assert_eq!(light_slot.status, ChunkStatus::Light);
        assert_eq!(light_slot.step, ChunkStatusStep::Ready);
        assert_eq!(light_slot.job_id, None);
        assert!(
            light_slot.revision.expect("light should have revision") > feature_revision,
            "light status must be published after the feature snapshot it consumes"
        );
        assert_eq!(
            first_status_event_pos(&events, ChunkStatus::Features, ChunkStatusStep::Ready),
            Some(ChunkPos::new(0, 0))
        );
        assert_eq!(
            first_status_event_pos(&events, ChunkStatus::Light, ChunkStatusStep::Ready),
            Some(ChunkPos::new(0, 0))
        );
        assert_eq!(scheduler.job_count(), 1);
        let job = scheduler.job(ChunkJobId(1)).unwrap();
        assert_eq!(job.id, ChunkJobId(1));
        assert_eq!(job.status, ChunkStatus::Features);
        assert_eq!(job.state, ChunkJobState::Complete);
        assert_eq!(job.target_chunks.first(), Some(&ChunkPos::new(0, 0)));
        assert_eq!(
            job.target_chunks.iter().copied().collect::<BTreeSet<_>>(),
            {
                (-1..=1)
                    .flat_map(|z| (-1..=1).map(move |x| ChunkPos::new(x, z)))
                    .collect()
            }
        );
        assert_eq!(job.feature_centers.first(), Some(&ChunkPos::new(0, 0)));
        assert_eq!(job.feature_centers.len(), 5 * 5);
        assert_eq!(job.dependency_chunks.first(), Some(&ChunkPos::new(0, 0)));
        assert_eq!(job.dependency_chunks.len(), 21 * 21);
        assert_eq!(job.seeded_dependency_chunks, 0);
        assert_eq!(job.dependency_cache_hits, 0);
        assert_eq!(job.dependency_cache_misses, 21 * 21);
        assert_eq!(job.retained_dependency_chunks, 21 * 21);
    }

    #[test]
    fn chunk_scheduler_coalesces_duplicate_status_requests() {
        let mut scheduler = ChunkScheduler::new(12_345);
        let interest = ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        };

        assert_eq!(
            without_fluid_tick_events(&apply_interest_and_poll(&mut scheduler, interest.clone()))
                .len(),
            73
        );
        assert_eq!(
            apply_interest_and_poll(&mut scheduler, interest),
            Vec::new()
        );
        assert_eq!(scheduler.loaded_chunk_count(), 9);
    }

    #[test]
    fn generated_chunks_are_dirty_until_saved() {
        let mut scheduler = ChunkScheduler::new(12_345);
        apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );

        let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(holder.residency(), ChunkResidency::Generated);
        assert!(holder.is_dirty());
        assert_eq!(scheduler.dirty_chunk_count(), 9);

        assert_eq!(scheduler.save_dirty_chunks().unwrap(), 9);

        let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(holder.residency(), ChunkResidency::Saved);
        assert!(!holder.is_dirty());
        assert_eq!(scheduler.dirty_chunk_count(), 0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn integrated_server_saves_and_reloads_resident_chunk() {
        let root = unique_temp_dir("integrated_server_saves_and_reloads_resident_chunk");
        let interest = ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        };
        let first_snapshot = {
            let mut server = IntegratedServer::with_chunk_store(
                12_345,
                Box::new(FilesystemChunkSnapshotStore::new(&root)),
            );
            let updates = try_handle_command_and_poll(
                &mut server,
                ClientCommand::SetChunkView(interest.clone()),
            )
            .unwrap();
            let snapshot = snapshot_update_for(&updates, ChunkPos::new(0, 0))
                .expect("updates should include the resident chunk snapshot");
            assert_eq!(server.scheduler().job_count(), 1);
            assert_eq!(server.scheduler().dirty_chunk_count(), 9);
            assert_eq!(
                server
                    .scheduler()
                    .holder(ChunkPos::new(0, 0))
                    .unwrap()
                    .residency(),
                ChunkResidency::Generated
            );

            assert_eq!(server.save_dirty_chunks().unwrap(), 9);
            assert_eq!(server.scheduler().dirty_chunk_count(), 0);
            assert_eq!(
                server
                    .scheduler()
                    .holder(ChunkPos::new(0, 0))
                    .unwrap()
                    .residency(),
                ChunkResidency::Saved
            );
            snapshot.clone()
        };

        let mut reloaded = IntegratedServer::with_chunk_store(
            12_345,
            Box::new(FilesystemChunkSnapshotStore::new(&root)),
        );
        let updates =
            try_handle_command_and_poll(&mut reloaded, ClientCommand::SetChunkView(interest))
                .unwrap();

        assert!(updates.contains(&ServerUpdate::ChunkSnapshot(first_snapshot)));
        assert_eq!(
            updates
                .iter()
                .filter(|update| matches!(update, ServerUpdate::EntitySnapshot(_)))
                .count(),
            1
        );
        assert_eq!(reloaded.scheduler().job_count(), 0);
        let holder = reloaded.scheduler().holder(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(holder.residency(), ChunkResidency::LoadedFromStore);
        assert!(!holder.is_dirty());
        assert_eq!(reloaded.scheduler().dirty_chunk_count(), 0);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_interest_unloads_chunks_outside_view() {
        let mut server = IntegratedServer::new(12_345);
        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );

        let updates = handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );

        assert!(
            updates
                .iter()
                .any(|update| matches!(update, ServerUpdate::ChunkUnload { pos } if *pos == ChunkPos::new(0, 0)))
        );
        assert!(updates.iter().any(|update| matches!(
            update,
            ServerUpdate::ChunkSnapshot(snapshot)
                if snapshot.pos == ChunkPos::new(1, 0)
                    && snapshot.status == ChunkStatus::Light
                    && snapshot.light_correct
        )));
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("mclone-{name}-{}-{nanos}", std::process::id()));
        path
    }
}
