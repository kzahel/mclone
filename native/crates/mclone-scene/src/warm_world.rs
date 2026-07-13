use std::path::PathBuf;

use mclone_app_runtime::monotonic::MonotonicInstant;
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::scene_session_runtime::SceneSessionRuntime;
use mclone_app_runtime::session::ActiveSessionDescriptor;
#[cfg(not(target_arch = "wasm32"))]
use mclone_core::BlockPos;
use mclone_core::{ChunkPos, Vec3d};
#[cfg(not(target_arch = "wasm32"))]
use mclone_core::{block_to_chunk_coord, block_to_section_coord};
use mclone_mesh::RenderSectionKey;
use mclone_render_session::EngineCameraController;

use crate::McloneSceneHostOptions;

#[cfg(not(target_arch = "wasm32"))]
const PROVISIONAL_GATE_FORWARD_BLOCKS: f64 = 6.0;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const PROVISIONAL_GATE_SEARCH_RADIUS_BLOCKS: i32 = 16;
#[cfg(not(target_arch = "wasm32"))]
const PROVISIONAL_GATE_EXIT_OFFSET_BLOCKS: f64 = 1.25;
#[cfg(not(target_arch = "wasm32"))]
const PROVISIONAL_GATE_HALF_WIDTH_BLOCKS: i32 = 1;
#[cfg(not(target_arch = "wasm32"))]
const PROVISIONAL_GATE_APPROACH_DEPTH_BLOCKS: i32 = 2;
#[cfg(not(target_arch = "wasm32"))]
const PROVISIONAL_GATE_HEIGHT_BLOCKS: i32 = 4;
#[cfg(not(target_arch = "wasm32"))]
const PROVISIONAL_GATE_SURFACE_SEARCH_BLOCKS: i32 = 12;

/// Stable client-side identity for one retained world instance.
///
/// Section and entity keys stay unqualified inside the slot. This identity is
/// for scene-level ownership, diagnostics, and later selection only.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorldInstanceId(u64);

impl WorldInstanceId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Launch-only request for Tactical 174's detached local standby smoke.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WarmWorldStandbyRequest {
    pub seed: i64,
    pub entry_center: ChunkPos,
}

impl WarmWorldStandbyRequest {
    pub const fn new(seed: i64, entry_center: ChunkPos) -> Self {
        Self { seed, entry_center }
    }
}

/// Shared scene command for atomically selecting the retained warm world.
///
/// Platform adapters may request this command between presented frames, but
/// readiness, camera reconciliation, ownership exchange, and diagnostics stay
/// inside `McloneSceneHost`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WarmWorldSelectionCommand {
    SwapWithStandby,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldEntryPose {
    pub feet_position: Vec3d,
    pub yaw_radians: f64,
}

impl WorldEntryPose {
    pub(crate) fn from_camera(camera: &EngineCameraController) -> Self {
        let snapshot = camera.snapshot();
        Self {
            feet_position: camera.feet_position(),
            yaw_radians: snapshot.yaw_radians,
        }
    }
}

/// Provisional runtime-only gate endpoint. Slice 3 records these candidates but
/// does not instantiate, render, or cross a gate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldGateEndpointCandidate {
    pub feet_position: Vec3d,
    pub center: Vec3d,
    pub normal: Vec3d,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WarmWorldStandbyPhase {
    Warming,
    ResolvingEndpoints,
    CpuReady,
    GpuWarming,
    Switchable,
    PlacementFailed,
    Failed,
    Cancelled,
}

impl WarmWorldStandbyPhase {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Warming => "warming",
            Self::ResolvingEndpoints => "resolving-endpoints",
            Self::CpuReady => "cpu-ready",
            Self::GpuWarming => "gpu-warming",
            Self::Switchable => "switchable",
            Self::PlacementFailed => "placement-failed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub const fn terminal(self) -> bool {
        matches!(
            self,
            Self::Switchable | Self::PlacementFailed | Self::Failed | Self::Cancelled
        )
    }
}

/// Exact admission facts which make a retained destination safe to select.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WarmWorldReadiness {
    pub cpu_ready: bool,
    pub startup_seed_enqueued: bool,
    pub startup_seed_drained: bool,
    pub entry_section: Option<RenderSectionKey>,
    pub entry_section_gpu_resident: bool,
    pub entry_section_traversal_ready: bool,
    pub renderer_topology_ready: bool,
    pub switchable: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WarmWorldStandbySnapshot {
    pub instance_id: WorldInstanceId,
    pub seed: i64,
    pub phase: WarmWorldStandbyPhase,
    pub elapsed_ms: f64,
    pub renderer_shell_create_ms: f64,
    pub renderer_multiview_create_ms: f64,
    pub renderer_multiview_required: bool,
    pub renderer_multiview_materialized: bool,
    pub atlas_size: [u32; 2],
    pub atlas_base_bytes: usize,
    pub asset_epoch: u64,
    pub poll_count: usize,
    pub poll_ms: f64,
    pub last_advance_ms: f64,
    pub worst_advance_ms: f64,
    pub worst_startup_step_ms: f64,
    pub worst_runtime_poll_ms: f64,
    pub endpoint_resolution_ms: f64,
    pub gpu_warm_ms: f64,
    pub last_gpu_advance_ms: f64,
    pub worst_gpu_advance_ms: f64,
    pub gpu_advance_count: usize,
    pub gpu_ready_advance_count: usize,
    pub gpu_skipped_no_slack_count: usize,
    pub camera_reconciled: bool,
    pub loaded_chunks: usize,
    pub startup_seed_sections: usize,
    pub startup_seed_drawable_sections: usize,
    pub startup_seed_owned_bytes: usize,
    pub initial_upload_lifecycle_items: usize,
    pub initial_upload_applied_lifecycle_items: usize,
    pub initial_upload_released_compile_jobs: usize,
    pub queued_upload_sections: usize,
    pub queued_upload_lifecycle_items: usize,
    pub queued_upload_mesh_owned_bytes: usize,
    pub oldest_queued_upload_age_ms: f64,
    pub gpu_section_count: usize,
    pub gpu_vertex_count: u32,
    pub gpu_index_count: u32,
    pub accepted_compile_result_count: usize,
    pub released_compile_job_count: usize,
    pub readiness: WarmWorldReadiness,
    pub source_endpoint: Option<WorldGateEndpointCandidate>,
    pub destination_endpoint: Option<WorldGateEndpointCandidate>,
    pub failure: Option<String>,
}

/// Evidence produced by one complete active/standby ownership exchange.
///
/// The switch itself does no renderer preparation. First-frame fields are
/// filled by the next scene render so the scripted smoke can distinguish the
/// atomic selection cost from destination runtime/terrain work.
#[derive(Clone, Debug, PartialEq)]
pub struct WarmWorldSwitchReport {
    pub sequence: u64,
    pub source_instance_id: WorldInstanceId,
    pub source_seed: i64,
    pub destination_instance_id: WorldInstanceId,
    pub destination_seed: i64,
    pub command_after_source_frame: u32,
    pub switch_elapsed_ms: f64,
    pub source_camera_commit_changed: bool,
    pub destination_camera_commit_changed: bool,
    pub source_camera_position_changed: bool,
    pub destination_camera_position_changed: bool,
    pub camera_commit_ms: f64,
    pub source_cadence_changed: bool,
    pub destination_cadence_changed: bool,
    pub source_queue_lifecycle_items_before: usize,
    pub source_queue_mesh_owned_bytes_before: usize,
    pub destination_queue_lifecycle_items_before: usize,
    pub destination_queue_mesh_owned_bytes_before: usize,
    pub source_pending_compile_jobs_before: usize,
    pub destination_pending_compile_jobs_before: usize,
    pub source_runtime_command_count_before: usize,
    pub source_runtime_update_count_before: usize,
    pub destination_runtime_command_count_before: usize,
    pub destination_runtime_update_count_before: usize,
    pub switch_uploaded_section_count: usize,
    pub switch_submitted_compile_section_count: usize,
    pub switch_accepted_compile_result_count: usize,
    pub switch_materialized_renderer: bool,
    pub first_drawable_destination_frame: Option<u32>,
    pub first_drawn_section_count: usize,
    pub first_frame_uploaded_section_count: usize,
    pub first_frame_submitted_compile_section_count: usize,
    pub first_frame_accepted_compile_result_count: usize,
    pub first_frame_queue_lifecycle_items: usize,
    pub first_frame_queue_mesh_owned_bytes: usize,
    pub first_frame_pending_compile_jobs_after: usize,
    pub destination_runtime_command_count_after_first_frame: usize,
    pub destination_runtime_update_count_after_first_frame: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorldSlotLifecycle {
    Starting,
    ActiveReady,
    #[cfg(not(target_arch = "wasm32"))]
    StandbyCpuReady,
    #[cfg(not(target_arch = "wasm32"))]
    StandbyGpuWarming,
    #[cfg(not(target_arch = "wasm32"))]
    StandbySwitchable,
    Empty,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorldSlotStorageIntent {
    TransientLocal,
    PersistentLocal,
    Remote,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorldSlotStorage {
    pub intent: WorldSlotStorageIntent,
    pub world_root: Option<PathBuf>,
    pub world_dir: Option<PathBuf>,
}

impl WorldSlotStorage {
    pub(crate) fn from_scene(
        scene: &McloneSceneHostOptions,
        descriptor: Option<&ActiveSessionDescriptor>,
    ) -> Self {
        let intent = match descriptor {
            Some(ActiveSessionDescriptor::Remote { .. }) => WorldSlotStorageIntent::Remote,
            Some(ActiveSessionDescriptor::LocalWorld { .. }) | None
                if scene.world_dir.is_some() =>
            {
                WorldSlotStorageIntent::PersistentLocal
            }
            Some(ActiveSessionDescriptor::LocalWorld { .. }) | None => {
                WorldSlotStorageIntent::TransientLocal
            }
        };
        Self {
            intent,
            world_root: scene.world_root.clone(),
            world_dir: scene.world_dir.clone(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct WarmWorldStandbyState {
    pub instance_id: WorldInstanceId,
    pub seed: i64,
    pub phase: WarmWorldStandbyPhase,
    pub started_at: MonotonicInstant,
    pub renderer_shell_create_ms: f64,
    pub renderer_multiview_create_ms: f64,
    pub renderer_multiview_required: bool,
    pub renderer_multiview_materialized: bool,
    pub atlas_size: [u32; 2],
    pub atlas_base_bytes: usize,
    pub asset_epoch: u64,
    pub poll_count: usize,
    pub poll_ms: f64,
    pub last_advance_ms: f64,
    pub worst_advance_ms: f64,
    pub worst_startup_step_ms: f64,
    pub worst_runtime_poll_ms: f64,
    pub endpoint_resolution_ms: f64,
    pub gpu_warm_started_at: Option<MonotonicInstant>,
    pub gpu_ready_at: Option<MonotonicInstant>,
    pub upload_queue_nonempty_since: Option<MonotonicInstant>,
    pub last_gpu_advance_ms: f64,
    pub worst_gpu_advance_ms: f64,
    pub gpu_advance_count: usize,
    pub gpu_ready_advance_count: usize,
    pub gpu_skipped_no_slack_count: usize,
    #[cfg(not(target_arch = "wasm32"))]
    pub last_gpu_advance_frame: Option<u32>,
    pub camera_reconciled: bool,
    pub loaded_chunks: usize,
    pub startup_seed_sections: usize,
    pub startup_seed_drawable_sections: usize,
    pub startup_seed_owned_bytes: usize,
    pub initial_upload_lifecycle_items: usize,
    pub initial_upload_applied_lifecycle_items: usize,
    pub initial_upload_released_compile_jobs: usize,
    pub queued_upload_sections: usize,
    pub queued_upload_lifecycle_items: usize,
    pub queued_upload_mesh_owned_bytes: usize,
    pub gpu_section_count: usize,
    pub gpu_vertex_count: u32,
    pub gpu_index_count: u32,
    pub accepted_compile_result_count: usize,
    pub released_compile_job_count: usize,
    pub readiness: WarmWorldReadiness,
    pub source_endpoint: Option<WorldGateEndpointCandidate>,
    pub destination_endpoint: Option<WorldGateEndpointCandidate>,
    pub failure: Option<String>,
}

impl WarmWorldStandbyState {
    pub(crate) fn snapshot(&self, now: MonotonicInstant) -> WarmWorldStandbySnapshot {
        WarmWorldStandbySnapshot {
            instance_id: self.instance_id,
            seed: self.seed,
            phase: self.phase,
            elapsed_ms: elapsed_between_ms(self.started_at, now),
            renderer_shell_create_ms: self.renderer_shell_create_ms,
            renderer_multiview_create_ms: self.renderer_multiview_create_ms,
            renderer_multiview_required: self.renderer_multiview_required,
            renderer_multiview_materialized: self.renderer_multiview_materialized,
            atlas_size: self.atlas_size,
            atlas_base_bytes: self.atlas_base_bytes,
            asset_epoch: self.asset_epoch,
            poll_count: self.poll_count,
            poll_ms: self.poll_ms,
            last_advance_ms: self.last_advance_ms,
            worst_advance_ms: self.worst_advance_ms,
            worst_startup_step_ms: self.worst_startup_step_ms,
            worst_runtime_poll_ms: self.worst_runtime_poll_ms,
            endpoint_resolution_ms: self.endpoint_resolution_ms,
            gpu_warm_ms: self.gpu_warm_started_at.map_or(0.0, |started_at| {
                elapsed_between_ms(started_at, self.gpu_ready_at.unwrap_or(now))
            }),
            last_gpu_advance_ms: self.last_gpu_advance_ms,
            worst_gpu_advance_ms: self.worst_gpu_advance_ms,
            gpu_advance_count: self.gpu_advance_count,
            gpu_ready_advance_count: self.gpu_ready_advance_count,
            gpu_skipped_no_slack_count: self.gpu_skipped_no_slack_count,
            camera_reconciled: self.camera_reconciled,
            loaded_chunks: self.loaded_chunks,
            startup_seed_sections: self.startup_seed_sections,
            startup_seed_drawable_sections: self.startup_seed_drawable_sections,
            startup_seed_owned_bytes: self.startup_seed_owned_bytes,
            initial_upload_lifecycle_items: self.initial_upload_lifecycle_items,
            initial_upload_applied_lifecycle_items: self.initial_upload_applied_lifecycle_items,
            initial_upload_released_compile_jobs: self.initial_upload_released_compile_jobs,
            queued_upload_sections: self.queued_upload_sections,
            queued_upload_lifecycle_items: self.queued_upload_lifecycle_items,
            queued_upload_mesh_owned_bytes: self.queued_upload_mesh_owned_bytes,
            oldest_queued_upload_age_ms: if self.queued_upload_lifecycle_items == 0 {
                0.0
            } else {
                self.upload_queue_nonempty_since
                    .map_or(0.0, |started_at| elapsed_between_ms(started_at, now))
            },
            gpu_section_count: self.gpu_section_count,
            gpu_vertex_count: self.gpu_vertex_count,
            gpu_index_count: self.gpu_index_count,
            accepted_compile_result_count: self.accepted_compile_result_count,
            released_compile_job_count: self.released_compile_job_count,
            readiness: self.readiness,
            source_endpoint: self.source_endpoint,
            destination_endpoint: self.destination_endpoint,
            failure: self.failure.clone(),
        }
    }
}

fn elapsed_between_ms(start: MonotonicInstant, end: MonotonicInstant) -> f64 {
    end.saturating_duration_since(start).as_secs_f64() * 1_000.0
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn entry_support_render_section(pose: WorldEntryPose) -> Option<RenderSectionKey> {
    if !pose.feet_position.is_finite() {
        return None;
    }
    let support_x = pose.feet_position.x.floor() as i32;
    let support_y = (pose.feet_position.y - 0.01).floor() as i32;
    let support_z = pose.feet_position.z.floor() as i32;
    Some(RenderSectionKey::new(
        block_to_chunk_coord(support_x),
        block_to_section_coord(support_y),
        block_to_chunk_coord(support_z),
    ))
}

/// Map a crossing to the clear approach on the far side of an endpoint.
/// Endpoint normals point back toward their authored approach, so arrival
/// proceeds opposite the normal and faces away from the gate.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn world_gate_destination_entry_pose(
    endpoint: WorldGateEndpointCandidate,
) -> WorldEntryPose {
    let forward = endpoint.normal.scale(-1.0);
    WorldEntryPose {
        feet_position: endpoint
            .feet_position
            .add(forward.scale(PROVISIONAL_GATE_EXIT_OFFSET_BLOCKS)),
        yaw_radians: forward.x.atan2(forward.z),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn resolve_world_gate_endpoint(
    runtime: &SceneSessionRuntime,
    pose: WorldEntryPose,
) -> Option<WorldGateEndpointCandidate> {
    let client = runtime.client();
    resolve_world_gate_endpoint_with(
        pose,
        |pos| client.block_state_at_block_pos(pos).is_some(),
        |point| client.is_point_inside_solid_block(point),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_world_gate_endpoint_with(
    pose: WorldEntryPose,
    mut block_loaded: impl FnMut(BlockPos) -> bool,
    mut point_solid: impl FnMut(Vec3d) -> bool,
) -> Option<WorldGateEndpointCandidate> {
    if !pose.feet_position.is_finite() || !pose.yaw_radians.is_finite() {
        return None;
    }
    let (forward_x, forward_z) = cardinal_forward(pose.yaw_radians);
    let normal_x = -forward_x;
    let normal_z = -forward_z;
    let tangent_x = -normal_z;
    let tangent_z = normal_x;
    let preferred_x = (pose.feet_position.x
        + f64::from(forward_x) * PROVISIONAL_GATE_FORWARD_BLOCKS)
        .floor() as i32;
    let preferred_z = (pose.feet_position.z
        + f64::from(forward_z) * PROVISIONAL_GATE_FORWARD_BLOCKS)
        .floor() as i32;
    let preferred_feet_y = pose.feet_position.y.round() as i32;

    for radius in 0..=PROVISIONAL_GATE_SEARCH_RADIUS_BLOCKS {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if radius > 0 && dx.abs() != radius && dz.abs() != radius {
                    continue;
                }
                let x = preferred_x + dx;
                let z = preferred_z + dz;
                for surface_delta in nearest_surface_deltas() {
                    let feet_y = preferred_feet_y + surface_delta;
                    if gate_footprint_clear(
                        x,
                        feet_y,
                        z,
                        normal_x,
                        normal_z,
                        tangent_x,
                        tangent_z,
                        &mut block_loaded,
                        &mut point_solid,
                    ) {
                        let feet_position =
                            Vec3d::new(f64::from(x) + 0.5, f64::from(feet_y), f64::from(z) + 0.5);
                        return Some(WorldGateEndpointCandidate {
                            feet_position,
                            center: feet_position.add(Vec3d::new(0.0, 2.0, 0.0)),
                            normal: Vec3d::new(f64::from(normal_x), 0.0, f64::from(normal_z)),
                        });
                    }
                }
            }
        }
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn nearest_surface_deltas() -> impl Iterator<Item = i32> {
    std::iter::once(0).chain(
        (1..=PROVISIONAL_GATE_SURFACE_SEARCH_BLOCKS).flat_map(|distance| [distance, -distance]),
    )
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::too_many_arguments)]
fn gate_footprint_clear(
    center_x: i32,
    feet_y: i32,
    center_z: i32,
    normal_x: i32,
    normal_z: i32,
    tangent_x: i32,
    tangent_z: i32,
    block_loaded: &mut impl FnMut(BlockPos) -> bool,
    point_solid: &mut impl FnMut(Vec3d) -> bool,
) -> bool {
    for lateral in -PROVISIONAL_GATE_HALF_WIDTH_BLOCKS..=PROVISIONAL_GATE_HALF_WIDTH_BLOCKS {
        for depth in
            -PROVISIONAL_GATE_APPROACH_DEPTH_BLOCKS..=PROVISIONAL_GATE_APPROACH_DEPTH_BLOCKS
        {
            let x = center_x + tangent_x * lateral + normal_x * depth;
            let z = center_z + tangent_z * lateral + normal_z * depth;
            let block_center = |y: f64| Vec3d::new(f64::from(x) + 0.5, y, f64::from(z) + 0.5);
            let support = BlockPos::new(x, feet_y - 1, z);
            if !block_loaded(support) || !point_solid(block_center(f64::from(feet_y) - 0.05)) {
                return false;
            }
            for height in 0..PROVISIONAL_GATE_HEIGHT_BLOCKS {
                let pos = BlockPos::new(x, feet_y + height, z);
                if !block_loaded(pos) || point_solid(block_center(f64::from(feet_y + height) + 0.5))
                {
                    return false;
                }
            }
        }
    }
    true
}

#[cfg(not(target_arch = "wasm32"))]
fn cardinal_forward(yaw_radians: f64) -> (i32, i32) {
    let x = -yaw_radians.sin();
    let z = yaw_radians.cos();
    if x.abs() > z.abs() {
        (if x >= 0.0 { 1 } else { -1 }, 0)
    } else {
        (0, if z >= 0.0 { 1 } else { -1 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_pose() -> WorldEntryPose {
        WorldEntryPose {
            feet_position: Vec3d::new(0.5, 64.0, 0.5),
            yaw_radians: 0.0,
        }
    }

    #[test]
    fn provisional_endpoint_prefers_six_blocks_forward_on_flat_ground() {
        let endpoint =
            resolve_world_gate_endpoint_with(flat_pose(), |_| true, |point| point.y < 64.0)
                .expect("flat loaded ground accepts a gate endpoint");

        assert_eq!(endpoint.feet_position, Vec3d::new(0.5, 64.0, 6.5));
        assert_eq!(endpoint.center, Vec3d::new(0.5, 66.0, 6.5));
        assert_eq!(endpoint.normal, Vec3d::new(0.0, 0.0, -1.0));
    }

    #[test]
    fn provisional_endpoint_searches_deterministically_around_obstruction() {
        let endpoint = resolve_world_gate_endpoint_with(
            flat_pose(),
            |_| true,
            |point| {
                point.y < 64.0
                    || ((point.x - 0.5).abs() < 0.25
                        && (point.z - 6.5).abs() < 0.25
                        && point.y < 68.0)
            },
        )
        .expect("ring search finds a clear alternate");

        assert_ne!(endpoint.feet_position, Vec3d::new(0.5, 64.0, 6.5));
    }

    #[test]
    fn provisional_endpoint_fails_without_loaded_clearance() {
        assert!(resolve_world_gate_endpoint_with(flat_pose(), |_| false, |_| false).is_none());
    }

    #[test]
    fn switchable_is_the_only_success_terminal_phase() {
        assert!(!WarmWorldStandbyPhase::CpuReady.terminal());
        assert!(!WarmWorldStandbyPhase::GpuWarming.terminal());
        assert!(WarmWorldStandbyPhase::Switchable.terminal());
    }

    #[test]
    fn entry_coverage_uses_the_support_section_below_the_feet() {
        assert_eq!(
            entry_support_render_section(flat_pose()),
            Some(RenderSectionKey::new(0, 3, 0))
        );
    }

    #[test]
    fn destination_entry_maps_just_beyond_and_faces_away_from_gate() {
        let endpoint = WorldGateEndpointCandidate {
            feet_position: Vec3d::new(0.5, 64.0, 6.5),
            center: Vec3d::new(0.5, 66.0, 6.5),
            normal: Vec3d::new(0.0, 0.0, -1.0),
        };

        let pose = world_gate_destination_entry_pose(endpoint);

        assert_eq!(pose.feet_position, Vec3d::new(0.5, 64.0, 7.75));
        assert_eq!(pose.yaw_radians, 0.0);
    }
}
