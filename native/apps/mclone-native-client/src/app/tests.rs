use super::*;
use crate::DEFAULT_SEED;
use crate::flat_client_driver::{
    effective_render_options_for_camera, engine_camera_input_from_flat_frame,
};
use mclone_app_runtime::session::{
    ActiveSessionDescriptor, GameSessionState, RemoteSessionEndpoint, SessionStartRequest,
};
use mclone_client::{ActorInterpolationConfig, ActorInterpolationState};
use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockPos, BlockStateId, ChunkSnapshot, Direction, block_to_section_coord,
    chunk_section_index, local_block_coord, local_section_block_coord,
};
use mclone_protocol::{ClientCommand, MovePlayerCommand, PlayerActionCommand, PlayerActionKind};
use mclone_render_session::{EngineCameraMovementMode, actor_instances_from_presentations};
use mclone_ui::GameScreen;

fn test_app_with_runtime(scene: SceneOptions) -> ChunkApp {
    let assets = WindowSceneAssets::load().unwrap();
    let runtime = WindowSceneRuntime::with_assets(&scene, &assets).unwrap();
    let mut app = ChunkApp::new(
        scene,
        assets,
        TexturedSectionRenderOptions::default(),
        WindowStartIntent::InWorld,
        StartupWaitPolicy::DESKTOP_DEFAULT,
    );
    app.driver.runtime = Some(runtime);
    app
}

fn center_chunk_signature(runtime: &WindowSceneRuntime, center: mclone_core::ChunkPos) -> u64 {
    let snapshot = runtime
        .client()
        .chunk_snapshot(center)
        .expect("center chunk must be loaded");
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash = hash_signature_value(hash, snapshot.sections.len() as u64);
    for section in &snapshot.sections {
        hash = hash_signature_value(hash, section.section_y as u64);
        for state_id in section.unpack_block_state_ids() {
            hash = hash_signature_value(hash, u64::from(state_id.0));
        }
    }
    hash
}

fn hash_signature_value(hash: u64, value: u64) -> u64 {
    (hash ^ value).wrapping_mul(0x0000_0100_0000_01b3)
}

fn ui_action_context() -> FlatClientUiActionContext<'static> {
    FlatClientUiActionContext {
        session_starting: false,
        from_pointer_click: true,
        fallback_remote_addr: None,
    }
}

fn unique_temp_world_root(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("mclone-{name}-{}-{nanos}", std::process::id()))
}

fn start_planned_session_immediately(
    app: &mut ChunkApp,
    mut result: crate::flat_client_driver::FlatClientUiActionResult,
) {
    let plan = result
        .session_start
        .take()
        .expect("session start should be planned");
    let descriptor = plan.session.payload.descriptor;
    app.start_world_from_scene(plan.session.payload.options)
        .unwrap();
    app.driver.session.complete_start(descriptor.clone());
    app.driver.apply_started_session_ui(&descriptor);
}

fn wait_for_client_block_state(app: &mut ChunkApp, pos: BlockPos, expected: BlockStateId) {
    let deadline = Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let observed = app
            .driver
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.client().block_state_at_block_pos(pos));
        if observed == Some(expected) {
            return;
        }
        app.driver.poll_runtime().unwrap();
        assert!(
            Instant::now() < deadline,
            "timed out waiting for block {pos:?} to become {expected:?}; observed {observed:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn first_non_air_block(snapshot: &ChunkSnapshot) -> Option<BlockPos> {
    for section in snapshot.sections.iter().rev() {
        let section_blocks = section.unpack_block_state_ids();
        for local_y in (0..16).rev() {
            for local_z in 0..16 {
                for local_x in 0..16 {
                    if section_blocks[chunk_section_index(local_x, local_y, local_z)]
                        != AIR_BLOCK_STATE_ID
                    {
                        return Some(BlockPos::new(
                            snapshot.pos.min_block_x() + local_x,
                            section.section_y * 16 + local_y,
                            snapshot.pos.min_block_z() + local_z,
                        ));
                    }
                }
            }
        }
    }
    None
}

fn snapshot_block_state(snapshot: &ChunkSnapshot, pos: BlockPos) -> BlockStateId {
    let Some(section) = snapshot
        .sections
        .iter()
        .find(|section| section.section_y == block_to_section_coord(pos.y))
    else {
        return AIR_BLOCK_STATE_ID;
    };
    let blocks = section.unpack_block_state_ids();
    blocks[chunk_section_index(
        local_block_coord(pos.x),
        local_section_block_coord(pos.y),
        local_block_coord(pos.z),
    )]
}

mod actor_camera;
mod input_adapter;
mod render_surface;
mod sessions_catalog;
