use std::path::PathBuf;

use mclone_app_runtime::host_mode::SingleViewHostMode;
use mclone_app_runtime::monotonic::MonotonicInstant;
use mclone_app_runtime::scenario::BuiltInScenarioId;
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::scenario::ScenarioLaunchIntent;
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::scenario_content::NativeManagedScenarioContentOperationService;
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::scene_session_runtime::SceneSessionRuntime;
use mclone_app_runtime::session::ActiveSessionDescriptor;
use mclone_core::{BlockPos, ChunkPos, Vec3d};
#[cfg(not(target_arch = "wasm32"))]
use mclone_core::{block_to_chunk_coord, block_to_section_coord};
use mclone_mesh::RenderSectionKey;
#[cfg(not(target_arch = "wasm32"))]
use mclone_render::chunk::TexturedSectionDrawResources;
use mclone_render::chunk::{PlacedTexturedSectionRenderer, TexturedSectionRenderStats};
#[cfg(not(target_arch = "wasm32"))]
use mclone_render::far_lod::FarTerrainLodRenderer;
#[cfg(not(target_arch = "wasm32"))]
use mclone_render::opaque_world_gate::{OpaqueWorldGate, OpaqueWorldGateRenderer};
use mclone_render::placement::{EmbeddedChunkRegion, WorldPlacement};
use mclone_render_session::EngineCameraController;
use mclone_server::{SimulationCadenceConfig, WorldBehaviorProfile, WorldGenerationProfile};

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
#[cfg(not(target_arch = "wasm32"))]
const WORLD_GATE_WIDTH_BLOCKS: f64 = 3.0;
#[cfg(not(target_arch = "wasm32"))]
const WORLD_GATE_HEIGHT_BLOCKS: f64 = 4.0;
#[cfg(not(target_arch = "wasm32"))]
const WORLD_GATE_ENTER_MARGIN_BLOCKS: f64 = 0.35;
#[cfg(not(target_arch = "wasm32"))]
const WORLD_GATE_EXIT_MARGIN_BLOCKS: f64 = 0.15;
#[cfg(not(target_arch = "wasm32"))]
const WORLD_GATE_OBSERVATION_DEPTH_BLOCKS: f64 = 2.0;
const EMBEDDED_ACTIVATION_MARGIN_BLOCKS: f64 = 0.2;
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) const EMBEDDED_ACTIVATION_MAX_RAY_BLOCKS: f64 = 16.0;
pub(crate) const EMBEDDED_ACTIVATION_CLOSE_SECONDS: f64 = 0.12;
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) const EMBEDDED_ACTIVATION_COVERED_SECONDS: f64 = 0.03;
pub(crate) const EMBEDDED_ACTIVATION_OPEN_SECONDS: f64 = 0.12;

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

/// Prepared local leaf for a detached retained-world startup.
#[derive(Clone, Debug, PartialEq)]
pub struct WarmWorldStandbyRequest {
    pub seed: i64,
    pub entry_center: ChunkPos,
    pub standby_cadence: Option<SimulationCadenceConfig>,
    pub world_dir: Option<PathBuf>,
    pub world_behavior_profile: WorldBehaviorProfile,
    pub world_generation_profile: WorldGenerationProfile,
    pub presentation: WarmWorldPresentationRequest,
}

impl WarmWorldStandbyRequest {
    pub const fn new(seed: i64, entry_center: ChunkPos) -> Self {
        Self {
            seed,
            entry_center,
            standby_cadence: None,
            world_dir: None,
            world_behavior_profile: WorldBehaviorProfile::Mutable,
            world_generation_profile: WorldGenerationProfile::Overworld,
            presentation: WarmWorldPresentationRequest::OpaqueGate,
        }
    }

    pub const fn with_standby_cadence(mut self, cadence: SimulationCadenceConfig) -> Self {
        self.standby_cadence = Some(cadence);
        self
    }

    pub fn with_persistent_world_dir(
        mut self,
        world_dir: impl Into<PathBuf>,
        world_generation_profile: WorldGenerationProfile,
    ) -> Self {
        self.world_dir = Some(world_dir.into());
        self.world_generation_profile = world_generation_profile;
        self
    }

    pub const fn with_world_behavior_profile(mut self, profile: WorldBehaviorProfile) -> Self {
        self.world_behavior_profile = profile;
        self
    }

    pub const fn with_embedded_preview(
        mut self,
        region: EmbeddedChunkRegion,
        placement: WorldPlacement,
        return_placement: WorldPlacement,
    ) -> Self {
        self.presentation = WarmWorldPresentationRequest::Diorama {
            region,
            placement,
            return_placement,
        };
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WarmWorldPresentationRequest {
    OpaqueGate,
    Diorama {
        region: EmbeddedChunkRegion,
        placement: WorldPlacement,
        return_placement: WorldPlacement,
    },
}

/// Storage-resolved scene request for one built-in embedded-world scenario.
///
/// The shared product intent remains path-free. A native content executor may
/// attach a persistent path while resolving this prepared scene boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedEmbeddedWorldScenario {
    pub id: BuiltInScenarioId,
    pub destination: WarmWorldStandbyRequest,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ManagedScenarioLaunchPhase {
    ResolvingContent,
    StartingPrimary,
    PrimaryPlayable,
    WarmingDestination,
    PreviewReady,
    DestinationFailed,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub(crate) struct ManagedScenarioLaunchState {
    pub(crate) intent: ScenarioLaunchIntent,
    pub(crate) phase: ManagedScenarioLaunchPhase,
    pub(crate) primary_operations: Option<NativeManagedScenarioContentOperationService>,
    pub(crate) destination_operations: Option<NativeManagedScenarioContentOperationService>,
    pub(crate) destination: Option<PreparedEmbeddedWorldScenario>,
    pub(crate) destination_failure: Option<String>,
}

impl PreparedEmbeddedWorldScenario {
    pub const fn new(id: BuiltInScenarioId, destination: WarmWorldStandbyRequest) -> Self {
        Self { id, destination }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct PreparedWarmWorldRendererShell {
    pub presentation: WarmWorldPresentationRequest,
    pub draw: TexturedSectionDrawResources,
    pub far_lod: FarTerrainLodRenderer,
    pub gate_renderer: Option<OpaqueWorldGateRenderer>,
    pub placed_renderer: Option<PlacedTexturedSectionRenderer>,
    pub renderer_shell_create_ms: f64,
    pub renderer_multiview_create_ms: f64,
    pub renderer_multiview_required: bool,
    pub renderer_multiview_materialized: bool,
    pub placed_renderer_topology_ready: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EmbeddedWorldPreviewPhase {
    #[default]
    Warming,
    Visible,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EmbeddedWorldActivationPhase {
    #[default]
    Idle,
    Closing,
    Covered,
    Opening,
    Failed,
}

impl EmbeddedWorldActivationPhase {
    pub const fn active(self) -> bool {
        matches!(self, Self::Closing | Self::Covered | Self::Opening)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmbeddedWorldActivationVolume {
    pub min: Vec3d,
    pub max: Vec3d,
}

impl EmbeddedWorldActivationVolume {
    pub(crate) fn from_region(region: EmbeddedChunkRegion, placement: WorldPlacement) -> Self {
        let radius = i64::from(region.horizontal_radius());
        let min_chunk_x = i64::from(region.center().x) - radius;
        let min_chunk_z = i64::from(region.center().z) - radius;
        let max_chunk_x = i64::from(region.center().x) + radius + 1;
        let max_chunk_z = i64::from(region.center().z) + radius + 1;
        let source_min = Vec3d::new(
            (min_chunk_x * 16) as f64,
            f64::from(region.min_section_y()) * 16.0,
            (min_chunk_z * 16) as f64,
        );
        let source_max = Vec3d::new(
            (max_chunk_x * 16) as f64,
            (f64::from(region.max_section_y()) + 1.0) * 16.0,
            (max_chunk_z * 16) as f64,
        );
        let mapped_min = placement.source_to_composition(source_min);
        let mapped_max = placement.source_to_composition(source_max);
        let margin = Vec3d::new(
            EMBEDDED_ACTIVATION_MARGIN_BLOCKS,
            EMBEDDED_ACTIVATION_MARGIN_BLOCKS,
            EMBEDDED_ACTIVATION_MARGIN_BLOCKS,
        );
        Self {
            min: mapped_min.subtract(margin),
            max: mapped_max.add(margin),
        }
    }

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) fn ray_distance(self, origin: Vec3d, direction: Vec3d) -> Option<f64> {
        if !origin.is_finite() || !direction.is_finite() {
            return None;
        }
        let length_squared =
            direction.x * direction.x + direction.y * direction.y + direction.z * direction.z;
        if length_squared <= f64::EPSILON {
            return None;
        }
        let inv_length = length_squared.sqrt().recip();
        let direction = direction.scale(inv_length);
        let mut near: f64 = 0.0;
        let mut far = EMBEDDED_ACTIVATION_MAX_RAY_BLOCKS;
        for (origin, direction, min, max) in [
            (origin.x, direction.x, self.min.x, self.max.x),
            (origin.y, direction.y, self.min.y, self.max.y),
            (origin.z, direction.z, self.min.z, self.max.z),
        ] {
            if direction.abs() <= f64::EPSILON {
                if origin < min || origin > max {
                    return None;
                }
                continue;
            }
            let inverse = direction.recip();
            let first = (min - origin) * inverse;
            let second = (max - origin) * inverse;
            near = near.max(first.min(second));
            far = far.min(first.max(second));
            if near > far {
                return None;
            }
        }
        (far >= 0.0 && near <= EMBEDDED_ACTIVATION_MAX_RAY_BLOCKS).then_some(near.max(0.0))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddedWorldActivationReport {
    pub sequence: u64,
    pub source_world: WorldInstanceId,
    pub destination_world: WorldInstanceId,
    pub requested_after_rendered_frame: u32,
    pub close_seconds: f64,
    pub covered_seconds: f64,
    pub open_seconds: f64,
    pub switch_elapsed_ms: Option<f64>,
    pub switched_activation_frame: Option<u32>,
    pub covered_rendered_frames: u32,
    pub first_uncovered_activation_frame: Option<u32>,
    pub first_uncovered_world: Option<WorldInstanceId>,
    pub first_uncovered_drawn_section_count: usize,
    pub first_uncovered_uploaded_section_count: usize,
    pub first_uncovered_submitted_compile_section_count: usize,
    pub first_uncovered_accepted_compile_result_count: usize,
    pub first_uncovered_queue_lifecycle_items: usize,
    pub first_uncovered_pending_compile_jobs: usize,
    pub first_uncovered_eye_count: usize,
    pub completed_activation_frame: Option<u32>,
    pub failure: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddedWorldActivationSnapshot {
    pub phase: EmbeddedWorldActivationPhase,
    pub alpha: f32,
    pub activation_ready: bool,
    pub volume: Option<EmbeddedWorldActivationVolume>,
    pub last_report: Option<EmbeddedWorldActivationReport>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct EmbeddedWorldActivationState {
    pub phase: EmbeddedWorldActivationPhase,
    pub phase_elapsed_seconds: f64,
    pub activation_frame: u32,
    pub volume: Option<EmbeddedWorldActivationVolume>,
    pub report: Option<EmbeddedWorldActivationReport>,
}

impl EmbeddedWorldActivationState {
    pub(crate) fn alpha(&self) -> f32 {
        match self.phase {
            EmbeddedWorldActivationPhase::Idle | EmbeddedWorldActivationPhase::Failed => 0.0,
            EmbeddedWorldActivationPhase::Closing => (self.phase_elapsed_seconds
                / EMBEDDED_ACTIVATION_CLOSE_SECONDS)
                .clamp(0.0, 1.0) as f32,
            EmbeddedWorldActivationPhase::Covered => 1.0,
            EmbeddedWorldActivationPhase::Opening => (1.0
                - self.phase_elapsed_seconds / EMBEDDED_ACTIVATION_OPEN_SECONDS)
                .clamp(0.0, 1.0) as f32,
        }
    }

    pub(crate) fn snapshot(&self, activation_ready: bool) -> EmbeddedWorldActivationSnapshot {
        EmbeddedWorldActivationSnapshot {
            phase: self.phase,
            alpha: self.alpha(),
            activation_ready,
            volume: self.volume,
            last_report: self.report.clone(),
        }
    }

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) fn begin(
        &mut self,
        sequence: u64,
        source_world: WorldInstanceId,
        destination_world: WorldInstanceId,
        rendered_frame: u32,
        volume: EmbeddedWorldActivationVolume,
    ) {
        self.phase = EmbeddedWorldActivationPhase::Closing;
        self.phase_elapsed_seconds = 0.0;
        self.activation_frame = 0;
        self.volume = Some(volume);
        self.report = Some(EmbeddedWorldActivationReport {
            sequence,
            source_world,
            destination_world,
            requested_after_rendered_frame: rendered_frame,
            close_seconds: EMBEDDED_ACTIVATION_CLOSE_SECONDS,
            covered_seconds: EMBEDDED_ACTIVATION_COVERED_SECONDS,
            open_seconds: EMBEDDED_ACTIVATION_OPEN_SECONDS,
            switch_elapsed_ms: None,
            switched_activation_frame: None,
            covered_rendered_frames: 0,
            first_uncovered_activation_frame: None,
            first_uncovered_world: None,
            first_uncovered_drawn_section_count: 0,
            first_uncovered_uploaded_section_count: 0,
            first_uncovered_submitted_compile_section_count: 0,
            first_uncovered_accepted_compile_result_count: 0,
            first_uncovered_queue_lifecycle_items: 0,
            first_uncovered_pending_compile_jobs: 0,
            first_uncovered_eye_count: 0,
            completed_activation_frame: None,
            failure: None,
        });
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddedWorldPreviewSnapshot {
    pub source_world: WorldInstanceId,
    pub region: EmbeddedChunkRegion,
    pub placement: WorldPlacement,
    pub asset_epoch: u64,
    pub phase: EmbeddedWorldPreviewPhase,
    pub renderer_topology_ready: bool,
    pub source_anchor_gpu_resident: bool,
    pub source_anchor_traversal_ready: bool,
    pub bounded_section_count: usize,
    pub last_drawn_section_count: usize,
    pub last_drawn_index_count: u32,
    pub source_host_mode: Option<SingleViewHostMode>,
    pub fixed_interest_center: ChunkPos,
    pub preparation: EmbeddedWorldPreviewPreparationSnapshot,
    pub render: EmbeddedWorldPreviewRenderSnapshot,
    pub last_mutation: Option<EmbeddedWorldPreviewMutationSnapshot>,
    pub boundary_warning: Option<String>,
    pub failure: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EmbeddedWorldPreviewPreparationSnapshot {
    pub frame_count: usize,
    pub last_runtime_poll_ms: f64,
    pub total_runtime_poll_ms: f64,
    pub last_compile_sync_ms: f64,
    pub total_compile_sync_ms: f64,
    pub last_gpu_upload_ms: f64,
    pub total_gpu_upload_ms: f64,
    pub last_submitted_compile_section_count: usize,
    pub submitted_compile_section_count: usize,
    pub last_accepted_compile_result_count: usize,
    pub accepted_compile_result_count: usize,
    pub last_uploaded_section_count: usize,
    pub uploaded_section_count: usize,
    pub pending_compile_jobs: usize,
    pub max_pending_compile_jobs: usize,
    pub queued_upload_lifecycle_items: usize,
    pub max_queued_upload_lifecycle_items: usize,
    pub queued_upload_mesh_owned_bytes: usize,
    pub max_queued_upload_mesh_owned_bytes: usize,
    pub source_priority_position: Vec3d,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EmbeddedWorldPreviewRenderSnapshot {
    pub frame_count: usize,
    pub last_cull_ms: f64,
    pub total_cull_ms: f64,
    pub last_draw_ms: f64,
    pub total_draw_ms: f64,
    pub last_bounded_section_count: usize,
    pub last_drawn_section_count: usize,
    pub last_drawn_index_count: u32,
    pub out_of_region_submission_count: usize,
    pub last_translucent_order: EmbeddedWorldPreviewTranslucentOrderSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedWorldPreviewTranslucentSubmissionSnapshot {
    pub world: WorldInstanceId,
    pub section: RenderSectionKey,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EmbeddedWorldPreviewTranslucentOrderSnapshot {
    pub section_count: usize,
    pub active_section_count: usize,
    pub preview_section_count: usize,
    pub source_switch_count: usize,
    pub first: Option<EmbeddedWorldPreviewTranslucentSubmissionSnapshot>,
    pub last: Option<EmbeddedWorldPreviewTranslucentSubmissionSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddedWorldPreviewMutationPhase {
    CommandSent,
    ClientApplied,
    GpuApplied,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddedWorldPreviewMutationSnapshot {
    pub sequence: u64,
    pub block: BlockPos,
    pub phase: EmbeddedWorldPreviewMutationPhase,
    pub command_changed: bool,
    pub command_update_count: usize,
    pub command_section_block_update_count: usize,
    pub requested_after_rendered_frame: u32,
    pub completed_after_rendered_frame: Option<u32>,
    pub submitted_compile_section_count: usize,
    pub accepted_compile_result_count: usize,
    pub uploaded_section_count: usize,
    pub failure: Option<String>,
}

pub(crate) struct EmbeddedWorldPreviewMutationState {
    pub snapshot: EmbeddedWorldPreviewMutationSnapshot,
    #[cfg(not(target_arch = "wasm32"))]
    pub submitted_compile_baseline: usize,
    #[cfg(not(target_arch = "wasm32"))]
    pub accepted_compile_baseline: usize,
    #[cfg(not(target_arch = "wasm32"))]
    pub uploaded_section_baseline: usize,
}

pub(crate) struct EmbeddedWorldPreview {
    pub source_world: WorldInstanceId,
    pub region: EmbeddedChunkRegion,
    pub placement: WorldPlacement,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub return_placement: WorldPlacement,
    pub asset_epoch: u64,
    pub phase: EmbeddedWorldPreviewPhase,
    pub renderer: PlacedTexturedSectionRenderer,
    pub renderer_topology_ready: bool,
    pub source_anchor_gpu_resident: bool,
    pub source_anchor_traversal_ready: bool,
    pub bounded_section_count: usize,
    pub last_draw: TexturedSectionRenderStats,
    pub source_host_mode: Option<SingleViewHostMode>,
    pub fixed_interest_center: ChunkPos,
    pub preparation: EmbeddedWorldPreviewPreparationSnapshot,
    pub render: EmbeddedWorldPreviewRenderSnapshot,
    #[cfg(not(target_arch = "wasm32"))]
    pub mutation_sequence: u64,
    pub last_mutation: Option<EmbeddedWorldPreviewMutationState>,
    pub boundary_warning: Option<String>,
    pub failure: Option<String>,
}

impl EmbeddedWorldPreview {
    pub(crate) fn snapshot(&self) -> EmbeddedWorldPreviewSnapshot {
        EmbeddedWorldPreviewSnapshot {
            source_world: self.source_world,
            region: self.region,
            placement: self.placement,
            asset_epoch: self.asset_epoch,
            phase: self.phase,
            renderer_topology_ready: self.renderer_topology_ready,
            source_anchor_gpu_resident: self.source_anchor_gpu_resident,
            source_anchor_traversal_ready: self.source_anchor_traversal_ready,
            bounded_section_count: self.bounded_section_count,
            last_drawn_section_count: self.last_draw.drawn_section_count,
            last_drawn_index_count: self.last_draw.drawn_index_count,
            source_host_mode: self.source_host_mode,
            fixed_interest_center: self.fixed_interest_center,
            preparation: self.preparation,
            render: self.render,
            last_mutation: self
                .last_mutation
                .as_ref()
                .map(|mutation| mutation.snapshot.clone()),
            boundary_warning: self.boundary_warning.clone(),
            failure: self.failure.clone(),
        }
    }

    pub(crate) fn record_render(
        &mut self,
        bounded_section_count: usize,
        out_of_region_submission_count: usize,
        cull_ms: f64,
        draw_ms: f64,
        stats: TexturedSectionRenderStats,
        translucent_order: EmbeddedWorldPreviewTranslucentOrderSnapshot,
    ) {
        self.last_draw = stats;
        self.render.frame_count = self.render.frame_count.saturating_add(1);
        self.render.last_cull_ms = cull_ms;
        self.render.total_cull_ms += cull_ms;
        self.render.last_draw_ms = draw_ms;
        self.render.total_draw_ms += draw_ms;
        self.render.last_bounded_section_count = bounded_section_count;
        self.render.last_drawn_section_count = stats.drawn_section_count;
        self.render.last_drawn_index_count = stats.drawn_index_count;
        self.render.last_translucent_order = translucent_order;
        self.render.out_of_region_submission_count = self
            .render
            .out_of_region_submission_count
            .saturating_add(out_of_region_submission_count);
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

/// Provisional runtime-only gate endpoint resolved from a slot's authoritative
/// accepted spawn pose and consumed by the Slice 6 paired-gate fixture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldGateEndpointCandidate {
    pub feet_position: Vec3d,
    pub center: Vec3d,
    pub normal: Vec3d,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldGateAvailability {
    Warming,
    Switchable,
    Failed,
}

impl WorldGateAvailability {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Warming => "warming",
            Self::Switchable => "switchable",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldGateSwitchDirection {
    PositiveToNegative,
    NegativeToPositive,
}

#[cfg(not(target_arch = "wasm32"))]
impl WorldGateSwitchDirection {
    const fn approach_sign(self) -> f64 {
        match self {
            Self::PositiveToNegative => 1.0,
            Self::NegativeToPositive => -1.0,
        }
    }

    const fn destination_side_sign(self) -> f64 {
        -self.approach_sign()
    }

    const fn reversed(self) -> Self {
        match self {
            Self::PositiveToNegative => Self::NegativeToPositive,
            Self::NegativeToPositive => Self::PositiveToNegative,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldGateObservation {
    None,
    Armed,
    Blocked,
    Crossed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldGateSnapshot {
    pub active_world_id: WorldInstanceId,
    pub destination_world_id: WorldInstanceId,
    pub active_endpoint: WorldGateEndpointCandidate,
    pub destination_endpoint: WorldGateEndpointCandidate,
    pub availability: WorldGateAvailability,
    pub direction: WorldGateSwitchDirection,
    pub armed: bool,
    pub last_oriented_distance: Option<f64>,
    pub crossing_count: u64,
}

/// Runtime-only paired gate and its shared visual-midpoint hysteresis.
///
/// Endpoint placement remains tied to each world's authoritative safe-surface
/// spawn. The model owns transition policy but no GPU resources or saved blocks.
#[derive(Clone, Debug, PartialEq)]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct WorldGate {
    active_world_id: WorldInstanceId,
    destination_world_id: WorldInstanceId,
    active_endpoint: WorldGateEndpointCandidate,
    destination_endpoint: WorldGateEndpointCandidate,
    availability: WorldGateAvailability,
    direction: WorldGateSwitchDirection,
    armed: bool,
    last_oriented_distance: Option<f64>,
    crossing_count: u64,
}

#[cfg(not(target_arch = "wasm32"))]
impl WorldGate {
    pub(crate) fn new(
        active_world_id: WorldInstanceId,
        destination_world_id: WorldInstanceId,
        active_endpoint: WorldGateEndpointCandidate,
        destination_endpoint: WorldGateEndpointCandidate,
    ) -> Self {
        Self {
            active_world_id,
            destination_world_id,
            active_endpoint,
            destination_endpoint,
            availability: WorldGateAvailability::Warming,
            direction: WorldGateSwitchDirection::PositiveToNegative,
            armed: false,
            last_oriented_distance: None,
            crossing_count: 0,
        }
    }

    pub(crate) fn snapshot(&self) -> WorldGateSnapshot {
        WorldGateSnapshot {
            active_world_id: self.active_world_id,
            destination_world_id: self.destination_world_id,
            active_endpoint: self.active_endpoint,
            destination_endpoint: self.destination_endpoint,
            availability: self.availability,
            direction: self.direction,
            armed: self.armed,
            last_oriented_distance: self.last_oriented_distance,
            crossing_count: self.crossing_count,
        }
    }

    pub(crate) fn set_availability(&mut self, availability: WorldGateAvailability) {
        self.availability = availability;
    }

    pub(crate) fn synchronize(
        &mut self,
        active_world_id: WorldInstanceId,
        destination_world_id: WorldInstanceId,
        active_endpoint: WorldGateEndpointCandidate,
        destination_endpoint: WorldGateEndpointCandidate,
        availability: WorldGateAvailability,
    ) {
        let placement_changed = self.active_world_id != active_world_id
            || self.destination_world_id != destination_world_id
            || self.active_endpoint != active_endpoint
            || self.destination_endpoint != destination_endpoint;
        self.active_world_id = active_world_id;
        self.destination_world_id = destination_world_id;
        self.active_endpoint = active_endpoint;
        self.destination_endpoint = destination_endpoint;
        self.availability = availability;
        if placement_changed {
            self.armed = false;
            self.last_oriented_distance = None;
        }
    }

    pub(crate) fn render_gate(&self) -> OpaqueWorldGate {
        let color = match self.availability {
            WorldGateAvailability::Warming => [0.95, 0.34, 0.06, 1.0],
            WorldGateAvailability::Switchable => [0.18, 0.24, 0.95, 1.0],
            WorldGateAvailability::Failed => [0.75, 0.04, 0.08, 1.0],
        };
        OpaqueWorldGate::new(
            glam_vec3_from_vec3d(self.active_endpoint.center),
            glam_vec3_from_vec3d(self.active_endpoint.normal),
            WORLD_GATE_WIDTH_BLOCKS as f32,
            WORLD_GATE_HEIGHT_BLOCKS as f32,
            color,
        )
    }

    pub(crate) fn observe_visual_midpoint(&mut self, point: Vec3d) -> WorldGateObservation {
        if !point.is_finite() {
            self.armed = false;
            self.last_oriented_distance = None;
            return WorldGateObservation::None;
        }
        let delta = point.add(self.active_endpoint.center.scale(-1.0));
        let signed_distance = dot(delta, self.active_endpoint.normal);
        let tangent = Vec3d::new(
            -self.active_endpoint.normal.z,
            0.0,
            self.active_endpoint.normal.x,
        );
        let lateral_distance = dot(delta, tangent).abs();
        let inside_aperture = lateral_distance <= WORLD_GATE_WIDTH_BLOCKS * 0.5
            && point.y >= self.active_endpoint.feet_position.y
            && point.y <= self.active_endpoint.feet_position.y + WORLD_GATE_HEIGHT_BLOCKS;
        let inside_observation_depth = signed_distance.abs() <= WORLD_GATE_OBSERVATION_DEPTH_BLOCKS;
        if !inside_aperture || !inside_observation_depth {
            self.armed = false;
            self.last_oriented_distance = None;
            return WorldGateObservation::None;
        }

        let oriented_distance = signed_distance * self.direction.approach_sign();
        let previous = self.last_oriented_distance.replace(oriented_distance);
        if oriented_distance >= WORLD_GATE_ENTER_MARGIN_BLOCKS {
            self.armed = true;
            return WorldGateObservation::Armed;
        }
        let crossed = self.armed
            && previous.is_some_and(|previous| previous > -WORLD_GATE_EXIT_MARGIN_BLOCKS)
            && oriented_distance <= -WORLD_GATE_EXIT_MARGIN_BLOCKS;
        if !crossed {
            return WorldGateObservation::None;
        }
        self.armed = false;
        match self.availability {
            WorldGateAvailability::Switchable => WorldGateObservation::Crossed,
            WorldGateAvailability::Warming | WorldGateAvailability::Failed => {
                // Keep a closed gate armed at its approach margin so a
                // controller that cannot immediately resolve its physical
                // body (notably room-scale XR) is rejected again next frame.
                self.armed = true;
                self.last_oriented_distance = Some(WORLD_GATE_ENTER_MARGIN_BLOCKS);
                WorldGateObservation::Blocked
            }
        }
    }

    pub(crate) fn blocked_feet_position(&self, current: Vec3d) -> Vec3d {
        if !current.is_finite() {
            return self.active_endpoint.feet_position.add(
                self.active_endpoint
                    .normal
                    .scale(self.direction.approach_sign() * WORLD_GATE_ENTER_MARGIN_BLOCKS),
            );
        }
        let signed_distance = dot(
            current.add(self.active_endpoint.center.scale(-1.0)),
            self.active_endpoint.normal,
        );
        let target_signed_distance =
            self.direction.approach_sign() * WORLD_GATE_ENTER_MARGIN_BLOCKS;
        current.add(
            self.active_endpoint
                .normal
                .scale(target_signed_distance - signed_distance),
        )
    }

    pub(crate) fn destination_entry_pose(&self) -> WorldEntryPose {
        world_gate_entry_pose_on_side(
            self.destination_endpoint,
            self.direction.destination_side_sign(),
        )
    }

    pub(crate) fn return_entry_pose_after_switch(&self) -> WorldEntryPose {
        let return_direction = self.direction.reversed();
        world_gate_entry_pose_on_side(
            self.active_endpoint,
            return_direction.destination_side_sign(),
        )
    }

    pub(crate) fn complete_switch(&mut self) {
        std::mem::swap(&mut self.active_world_id, &mut self.destination_world_id);
        std::mem::swap(&mut self.active_endpoint, &mut self.destination_endpoint);
        self.direction = self.direction.reversed();
        self.armed = false;
        self.last_oriented_distance = None;
        self.crossing_count = self.crossing_count.saturating_add(1);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn dot(left: Vec3d, right: Vec3d) -> f64 {
    left.x * right.x + left.y * right.y + left.z * right.z
}

#[cfg(not(target_arch = "wasm32"))]
fn glam_vec3_from_vec3d(value: Vec3d) -> glam::Vec3 {
    glam::Vec3::new(value.x as f32, value.y as f32, value.z as f32)
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
    /// Bytes in the one shared base atlas; not attributable to the standby.
    pub atlas_base_bytes: usize,
    pub duplicated_atlas_base_bytes: usize,
    pub shared_terrain_resource_owner_count: usize,
    pub asset_epoch: u64,
    pub standby_cadence: SimulationCadenceConfig,
    pub standby_cadence_applied: bool,
    pub poll_count: usize,
    pub poll_ms: f64,
    pub startup_advance_count: usize,
    pub startup_advance_total_ms: f64,
    pub last_advance_ms: f64,
    pub worst_advance_ms: f64,
    pub worst_startup_step_ms: f64,
    pub worst_runtime_poll_ms: f64,
    pub endpoint_resolution_ms: f64,
    pub gpu_warm_ms: f64,
    pub gpu_advance_total_ms: f64,
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
    pub estimated_gpu_terrain_bytes: usize,
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
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub presentation: WarmWorldPresentationRequest,
    pub phase: WarmWorldStandbyPhase,
    pub started_at: MonotonicInstant,
    pub renderer_shell_create_ms: f64,
    pub renderer_multiview_create_ms: f64,
    pub renderer_multiview_required: bool,
    pub renderer_multiview_materialized: bool,
    pub atlas_size: [u32; 2],
    pub atlas_base_bytes: usize,
    pub duplicated_atlas_base_bytes: usize,
    pub shared_terrain_resource_owner_count: usize,
    pub asset_epoch: u64,
    pub standby_cadence: SimulationCadenceConfig,
    pub standby_cadence_applied: bool,
    pub poll_count: usize,
    pub poll_ms: f64,
    pub startup_advance_count: usize,
    pub startup_advance_total_ms: f64,
    pub last_advance_ms: f64,
    pub worst_advance_ms: f64,
    pub worst_startup_step_ms: f64,
    pub worst_runtime_poll_ms: f64,
    pub endpoint_resolution_ms: f64,
    pub gpu_warm_started_at: Option<MonotonicInstant>,
    pub gpu_ready_at: Option<MonotonicInstant>,
    pub upload_queue_nonempty_since: Option<MonotonicInstant>,
    pub last_gpu_advance_ms: f64,
    pub gpu_advance_total_ms: f64,
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
            duplicated_atlas_base_bytes: self.duplicated_atlas_base_bytes,
            shared_terrain_resource_owner_count: self.shared_terrain_resource_owner_count,
            asset_epoch: self.asset_epoch,
            standby_cadence: self.standby_cadence,
            standby_cadence_applied: self.standby_cadence_applied,
            poll_count: self.poll_count,
            poll_ms: self.poll_ms,
            startup_advance_count: self.startup_advance_count,
            startup_advance_total_ms: self.startup_advance_total_ms,
            last_advance_ms: self.last_advance_ms,
            worst_advance_ms: self.worst_advance_ms,
            worst_startup_step_ms: self.worst_startup_step_ms,
            worst_runtime_poll_ms: self.worst_runtime_poll_ms,
            endpoint_resolution_ms: self.endpoint_resolution_ms,
            gpu_warm_ms: self.gpu_warm_started_at.map_or(0.0, |started_at| {
                elapsed_between_ms(started_at, self.gpu_ready_at.unwrap_or(now))
            }),
            gpu_advance_total_ms: self.gpu_advance_total_ms,
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
            estimated_gpu_terrain_bytes: estimated_gpu_terrain_bytes(
                self.gpu_vertex_count,
                self.gpu_index_count,
            ),
            accepted_compile_result_count: self.accepted_compile_result_count,
            released_compile_job_count: self.released_compile_job_count,
            readiness: self.readiness,
            source_endpoint: self.source_endpoint,
            destination_endpoint: self.destination_endpoint,
            failure: self.failure.clone(),
        }
    }
}

fn estimated_gpu_terrain_bytes(vertex_count: u32, index_count: u32) -> usize {
    (vertex_count as usize)
        .saturating_mul(std::mem::size_of::<mclone_mesh::TexturedChunkVertex>())
        .saturating_add((index_count as usize).saturating_mul(std::mem::size_of::<u32>()))
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

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn anchor_render_section(anchor: Vec3d) -> Option<RenderSectionKey> {
    if !anchor.is_finite() {
        return None;
    }
    let x = anchor.x.floor() as i32;
    let y = anchor.y.floor() as i32;
    let z = anchor.z.floor() as i32;
    Some(RenderSectionKey::new(
        block_to_chunk_coord(x),
        block_to_section_coord(y),
        block_to_chunk_coord(z),
    ))
}

/// Map a crossing to the clear approach on the far side of an endpoint.
/// Endpoint normals point back toward their authored approach, so arrival
/// proceeds opposite the normal and faces away from the gate.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn world_gate_destination_entry_pose(
    endpoint: WorldGateEndpointCandidate,
) -> WorldEntryPose {
    world_gate_entry_pose_on_side(endpoint, -1.0)
}

#[cfg(not(target_arch = "wasm32"))]
fn world_gate_entry_pose_on_side(
    endpoint: WorldGateEndpointCandidate,
    side_sign: f64,
) -> WorldEntryPose {
    let forward = endpoint.normal.scale(side_sign);
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

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn bounded_preview_source_priority(
    region: EmbeddedChunkRegion,
    placement: WorldPlacement,
    composition_camera: Vec3d,
) -> Vec3d {
    let source = placement.composition_to_source(composition_camera);
    let radius = i64::from(region.horizontal_radius());
    let min_chunk_x = i64::from(region.center().x) - radius;
    let max_chunk_x = i64::from(region.center().x) + radius;
    let min_chunk_z = i64::from(region.center().z) - radius;
    let max_chunk_z = i64::from(region.center().z) + radius;
    let min_x = (min_chunk_x * 16) as f64;
    let max_x = ((max_chunk_x + 1) * 16) as f64 - 0.5;
    let min_z = (min_chunk_z * 16) as f64;
    let max_z = ((max_chunk_z + 1) * 16) as f64 - 0.5;
    let min_y = f64::from(region.min_section_y()) * 16.0;
    let max_y = f64::from(region.max_section_y() + 1) * 16.0 - 0.5;
    Vec3d::new(
        source.x.clamp(min_x, max_x),
        source.y.clamp(min_y, max_y),
        source.z.clamp(min_z, max_z),
    )
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
    fn preview_compile_priority_inverse_maps_and_clamps_to_its_region() {
        let region = EmbeddedChunkRegion::new(ChunkPos::new(2, -3), 1, 3, 5).unwrap();
        let placement = WorldPlacement::new(
            Vec3d::new(40.0, 65.0, -40.0),
            Vec3d::new(8.0, 65.0, 8.0),
            0.125,
        )
        .unwrap();

        assert_eq!(
            bounded_preview_source_priority(region, placement, Vec3d::new(8.5, 65.5, 7.5)),
            Vec3d::new(44.0, 69.0, -44.0)
        );
        assert_eq!(
            bounded_preview_source_priority(region, placement, Vec3d::new(100.0, 200.0, -100.0)),
            Vec3d::new(63.5, 95.5, -64.0)
        );
    }

    #[test]
    fn embedded_activation_volume_maps_region_and_bounds_ray_distance() {
        let region = EmbeddedChunkRegion::new(ChunkPos::new(0, 0), 0, 3, 5).unwrap();
        let placement = WorldPlacement::new(
            Vec3d::new(8.0, 64.0, 8.0),
            Vec3d::new(8.0, 65.0, 8.0),
            0.125,
        )
        .unwrap();
        let volume = EmbeddedWorldActivationVolume::from_region(region, placement);

        assert!(volume.min.x < 7.0 && volume.max.x > 9.0);
        assert!(volume.min.z < 7.0 && volume.max.z > 9.0);
        assert!(
            volume
                .ray_distance(Vec3d::new(8.0, 66.0, -4.0), Vec3d::new(0.0, 0.0, 1.0))
                .is_some()
        );
        assert!(
            volume
                .ray_distance(Vec3d::new(8.0, 66.0, -20.0), Vec3d::new(0.0, 0.0, 1.0))
                .is_none()
        );
        assert!(
            volume
                .ray_distance(Vec3d::new(20.0, 66.0, -4.0), Vec3d::new(0.0, 0.0, 1.0))
                .is_none()
        );
    }

    #[test]
    fn embedded_activation_alpha_closes_and_opens_without_overshoot() {
        let mut state = EmbeddedWorldActivationState::default();
        state.begin(
            1,
            WorldInstanceId::new(1),
            WorldInstanceId::new(2),
            7,
            EmbeddedWorldActivationVolume {
                min: Vec3d::ZERO,
                max: Vec3d::new(1.0, 1.0, 1.0),
            },
        );
        state.phase_elapsed_seconds = EMBEDDED_ACTIVATION_CLOSE_SECONDS * 0.5;
        assert!((state.alpha() - 0.5).abs() < 1.0e-6);
        state.phase = EmbeddedWorldActivationPhase::Covered;
        assert_eq!(state.alpha(), 1.0);
        state.phase = EmbeddedWorldActivationPhase::Opening;
        state.phase_elapsed_seconds = EMBEDDED_ACTIVATION_OPEN_SECONDS * 0.5;
        assert!((state.alpha() - 0.5).abs() < 1.0e-6);
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

    fn paired_gate() -> WorldGate {
        WorldGate::new(
            WorldInstanceId::new(1),
            WorldInstanceId::new(2),
            WorldGateEndpointCandidate {
                feet_position: Vec3d::new(0.5, 64.0, 6.5),
                center: Vec3d::new(0.5, 66.0, 6.5),
                normal: Vec3d::new(0.0, 0.0, -1.0),
            },
            WorldGateEndpointCandidate {
                feet_position: Vec3d::new(10.5, 72.0, 20.5),
                center: Vec3d::new(10.5, 74.0, 20.5),
                normal: Vec3d::new(1.0, 0.0, 0.0),
            },
        )
    }

    #[test]
    fn gate_hysteresis_requires_approach_then_exit_margin() {
        let mut gate = paired_gate();
        gate.set_availability(WorldGateAvailability::Switchable);

        assert_eq!(
            gate.observe_visual_midpoint(Vec3d::new(0.5, 66.0, 6.0)),
            WorldGateObservation::Armed
        );
        assert_eq!(
            gate.observe_visual_midpoint(Vec3d::new(0.5, 66.0, 6.45)),
            WorldGateObservation::None
        );
        assert_eq!(
            gate.observe_visual_midpoint(Vec3d::new(0.5, 66.0, 6.55)),
            WorldGateObservation::None
        );
        assert_eq!(
            gate.observe_visual_midpoint(Vec3d::new(0.5, 66.0, 6.7)),
            WorldGateObservation::Crossed
        );
    }

    #[test]
    fn closed_gate_reports_blocked_instead_of_crossed() {
        let mut gate = paired_gate();

        assert_eq!(
            gate.observe_visual_midpoint(Vec3d::new(0.5, 66.0, 6.0)),
            WorldGateObservation::Armed
        );
        assert_eq!(
            gate.observe_visual_midpoint(Vec3d::new(0.5, 66.0, 6.7)),
            WorldGateObservation::Blocked
        );
        assert!(gate.snapshot().armed);
        assert_eq!(
            gate.observe_visual_midpoint(Vec3d::new(0.5, 66.0, 6.8)),
            WorldGateObservation::Blocked
        );
        assert_eq!(
            gate.blocked_feet_position(Vec3d::new(0.5, 64.0, 6.8)),
            Vec3d::new(0.5, 64.0, 6.15),
        );
        assert_eq!(gate.snapshot().crossing_count, 0);
    }

    #[test]
    fn paired_gate_reverses_direction_and_arrival_side_each_switch() {
        let mut gate = paired_gate();
        gate.set_availability(WorldGateAvailability::Switchable);

        assert_eq!(
            gate.destination_entry_pose().feet_position,
            Vec3d::new(9.25, 72.0, 20.5)
        );
        assert_eq!(
            gate.return_entry_pose_after_switch().feet_position,
            Vec3d::new(0.5, 64.0, 5.25)
        );

        gate.complete_switch();

        let snapshot = gate.snapshot();
        assert_eq!(snapshot.active_world_id, WorldInstanceId::new(2));
        assert_eq!(snapshot.destination_world_id, WorldInstanceId::new(1));
        assert_eq!(
            snapshot.direction,
            WorldGateSwitchDirection::NegativeToPositive
        );
        assert_eq!(snapshot.crossing_count, 1);
        assert_eq!(
            gate.destination_entry_pose().feet_position,
            Vec3d::new(0.5, 64.0, 5.25)
        );
    }
}
