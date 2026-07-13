use std::path::PathBuf;

use mclone_app_runtime::monotonic::MonotonicInstant;
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::scene_session_runtime::SceneSessionRuntime;
use mclone_app_runtime::session::ActiveSessionDescriptor;
#[cfg(not(target_arch = "wasm32"))]
use mclone_core::BlockPos;
use mclone_core::{ChunkPos, Vec3d};
use mclone_render_session::EngineCameraController;

use crate::McloneSceneHostOptions;

#[cfg(not(target_arch = "wasm32"))]
const PROVISIONAL_GATE_FORWARD_BLOCKS: f64 = 6.0;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const PROVISIONAL_GATE_SEARCH_RADIUS_BLOCKS: i32 = 16;
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
            Self::PlacementFailed => "placement-failed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub const fn terminal(self) -> bool {
        matches!(
            self,
            Self::CpuReady | Self::PlacementFailed | Self::Failed | Self::Cancelled
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WarmWorldStandbySnapshot {
    pub instance_id: WorldInstanceId,
    pub seed: i64,
    pub phase: WarmWorldStandbyPhase,
    pub elapsed_ms: f64,
    pub renderer_shell_create_ms: f64,
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
    pub camera_reconciled: bool,
    pub loaded_chunks: usize,
    pub startup_seed_sections: usize,
    pub startup_seed_drawable_sections: usize,
    pub startup_seed_owned_bytes: usize,
    pub source_endpoint: Option<WorldGateEndpointCandidate>,
    pub destination_endpoint: Option<WorldGateEndpointCandidate>,
    pub failure: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorldSlotLifecycle {
    Starting,
    ActiveReady,
    #[cfg(not(target_arch = "wasm32"))]
    StandbyCpuReady,
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
    pub camera_reconciled: bool,
    pub loaded_chunks: usize,
    pub startup_seed_sections: usize,
    pub startup_seed_drawable_sections: usize,
    pub startup_seed_owned_bytes: usize,
    pub source_endpoint: Option<WorldGateEndpointCandidate>,
    pub destination_endpoint: Option<WorldGateEndpointCandidate>,
    pub failure: Option<String>,
}

impl WarmWorldStandbyState {
    pub(crate) fn snapshot(&self, elapsed_ms: f64) -> WarmWorldStandbySnapshot {
        WarmWorldStandbySnapshot {
            instance_id: self.instance_id,
            seed: self.seed,
            phase: self.phase,
            elapsed_ms,
            renderer_shell_create_ms: self.renderer_shell_create_ms,
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
            camera_reconciled: self.camera_reconciled,
            loaded_chunks: self.loaded_chunks,
            startup_seed_sections: self.startup_seed_sections,
            startup_seed_drawable_sections: self.startup_seed_drawable_sections,
            startup_seed_owned_bytes: self.startup_seed_owned_bytes,
            source_endpoint: self.source_endpoint,
            destination_endpoint: self.destination_endpoint,
            failure: self.failure.clone(),
        }
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
}
