use std::time::Duration;

use crate::{
    BoundedRepresentationOwnershipSnapshot, ExactPaintedCoverageSnapshot, McloneTreeOccurrenceId,
    TerrainClipmapConfig, TerrainCompositionSourceIdentity, TerrainExactCoverageMode,
    TerrainHorizonFrameStats, TerrainHorizonPresentation, TerrainHorizonRenderTarget,
    TerrainPreparedExactFrame, TerrainPreviewCamera, TerrainPreviewMaterialAtlas,
    TerrainPreviewProjectionKind, TerrainPreviewView, TerrainVegetationExecutor, TerrainViewEngine,
    TerrainViewEngineConfig, TerrainViewSourceIdentity, terrain_preview_focus_y_for_profile,
};
use mclone_core::HorizontalTopology;
use mclone_render_color::{RenderColorProfile, RenderTargetColorTransform};
use mclone_view_control::{
    ContactEvent, ContactGestureReducer, ViewPoint, ViewportMetrics, WorldViewHeldDirection,
    WorldViewHeldMotion, WorldViewIntent, WorldViewMode, WorldViewProjection, WorldViewReducer,
    WorldViewSignal, WorldViewState,
};
use mclone_worldgen::{
    levelgen::MCLONE_OVERWORLD_SEA_LEVEL,
    terrain_preview::{TerrainPreviewContentStage, TerrainPreviewProfile},
};

const COMPOSED_ORBIT_VIEWER_CLEARANCE_BLOCKS: f32 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainRuntimeConfig {
    pub width: u32,
    pub height: u32,
    pub seed: i64,
    pub initial_view: WorldViewState,
    pub clipmap: TerrainClipmapConfig,
    pub vegetation_enabled: bool,
    pub color_profile: RenderColorProfile,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TerrainRuntimeCompositionMode {
    #[default]
    Horizon,
    Exact,
    Composed,
    Coverage,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TerrainRuntimeExactAnchor {
    #[default]
    Focus,
    ViewerForward,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainRuntimeExactView {
    pub render_view: mclone_render::chunk::ChunkRenderView,
    pub residency_anchor: [i32; 2],
    pub target_y: f32,
}

impl TerrainRuntimeCompositionMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Horizon => "horizon",
            Self::Exact => "exact",
            Self::Composed => "composed",
            Self::Coverage => "coverage",
        }
    }

    pub fn parse_label(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "horizon" | "procedural" => Ok(Self::Horizon),
            "exact" => Ok(Self::Exact),
            "composed" | "composition" => Ok(Self::Composed),
            "coverage" | "mask" => Ok(Self::Coverage),
            other => Err(format!(
                "unsupported runtime terrain composition mode {other:?}; expected horizon, exact, \
                 composed, or coverage"
            )),
        }
    }
}

impl TerrainRuntimeExactAnchor {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::ViewerForward => "viewer-forward",
        }
    }

    pub fn parse_label(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "focus" | "target" | "orbit-target" => Ok(Self::Focus),
            "viewer-forward" | "viewer" | "eye" => Ok(Self::ViewerForward),
            other => Err(format!(
                "unsupported runtime terrain exact anchor {other:?}; expected focus or \
                 viewer-forward"
            )),
        }
    }
}

pub struct TerrainRuntimeSession {
    engine: TerrainViewEngine,
    config: TerrainRuntimeConfig,
    color_format: wgpu::TextureFormat,
    target_color_transform: RenderTargetColorTransform,
    view_state: WorldViewState,
    view_reducer: WorldViewReducer,
    contacts: ContactGestureReducer,
    held_motion: WorldViewHeldMotion,
    residency_anchor: [i32; 2],
    target_y_override: Option<f32>,
    revision: u64,
    coarse_ready_at: Option<Duration>,
    target_ready_at: Option<Duration>,
    process_first_coarse_ready_at: Option<Duration>,
    process_first_target_ready_at: Option<Duration>,
    last_stats: Option<TerrainHorizonFrameStats>,
}

impl TerrainRuntimeSession {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        config: TerrainRuntimeConfig,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
        vegetation_executor: Option<Box<dyn TerrainVegetationExecutor>>,
    ) -> Result<Self, String> {
        if config.vegetation_enabled != vegetation_executor.is_some() {
            return Err(
                "runtime terrain vegetation enablement must match executor availability".to_owned(),
            );
        }
        let view_reducer = WorldViewReducer::default();
        let view_state = view_reducer.normalize(config.initial_view);
        let residency_anchor = [floor_i32(view_state.focus_x), floor_i32(view_state.focus_z)];
        let target_color_transform = config.color_profile.target_color_transform(color_format);
        let source = TerrainViewSourceIdentity::detached(
            TerrainCompositionSourceIdentity::new(
                TerrainPreviewProfile::McloneOverworldV1,
                config.seed,
            ),
            HorizontalTopology::UNBOUNDED,
            1,
            1,
        )?;
        let engine = TerrainViewEngine::new(
            device,
            queue,
            color_format,
            TerrainViewEngineConfig {
                width: config.width,
                height: config.height,
                source,
                clipmap: config.clipmap,
                render_cell_stride: 1,
                vegetation_max_sample_spacing:
                    mclone_worldgen::terrain_preview::TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING,
                vegetation_enabled: config.vegetation_enabled,
                color_profile: config.color_profile,
            },
            material_atlas,
            vegetation_executor,
        )?;
        let mut session = Self {
            engine,
            config,
            color_format,
            target_color_transform,
            view_state,
            view_reducer,
            contacts: ContactGestureReducer::default(),
            held_motion: WorldViewHeldMotion::default(),
            residency_anchor,
            target_y_override: None,
            revision: 0,
            coarse_ready_at: None,
            target_ready_at: None,
            process_first_coarse_ready_at: None,
            process_first_target_ready_at: None,
            last_stats: None,
        };
        session.replan();
        Ok(session)
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        self.engine.resize(device, width, height);
        if self.config.width != width || self.config.height != height {
            self.config.width = width;
            self.config.height = height;
        }
    }

    pub const fn view_state(&self) -> WorldViewState {
        self.view_state
    }

    pub fn set_view_state(&mut self, state: WorldViewState) -> bool {
        let state = self.view_reducer.normalize(state);
        let previous = self.view_state;
        if state == previous {
            return false;
        }
        self.view_state = state;
        if residency_plan_key(state, self.config.clipmap)
            != residency_plan_key(previous, self.config.clipmap)
        {
            self.replan();
        }
        true
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub const fn color_profile(&self) -> RenderColorProfile {
        self.config.color_profile
    }

    pub const fn color_format(&self) -> wgpu::TextureFormat {
        self.color_format
    }

    pub const fn target_color_transform(&self) -> RenderTargetColorTransform {
        self.target_color_transform
    }

    pub fn apply_intent(&mut self, intent: WorldViewIntent) -> bool {
        let previous = self.view_state;
        let reduction = self.view_reducer.reduce(previous, intent);
        self.view_state = reduction.state;
        if let Some(WorldViewSignal::DoubleTap { position }) = reduction.signal {
            self.apply_double_tap(position);
        }
        if self.view_state == previous {
            return false;
        }
        if residency_plan_key(self.view_state, self.config.clipmap)
            != residency_plan_key(previous, self.config.clipmap)
        {
            self.replan();
        }
        true
    }

    pub fn contact(&mut self, event: ContactEvent) -> bool {
        let mut changed = false;
        for intent in self.contacts.handle(self.view_state, event) {
            changed |= self.apply_intent(intent);
        }
        changed
    }

    pub fn set_held_motion(&mut self, direction: WorldViewHeldDirection, pressed: bool) -> bool {
        self.held_motion.set_direction(direction, pressed)
    }

    pub const fn has_held_motion(&self) -> bool {
        self.held_motion.is_active()
    }

    pub fn cancel_input(&mut self) -> bool {
        let contacts_changed = self.contact(ContactEvent::CancelAll);
        self.held_motion.clear() || contacts_changed
    }

    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        elapsed: Duration,
    ) -> Result<TerrainHorizonFrameStats, String> {
        self.encode_horizon(
            device, queue, encoder, None, color_view, elapsed, None, None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode_to_target(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: TerrainHorizonRenderTarget<'_>,
        elapsed: Duration,
        exact_coverage: Option<(&ExactPaintedCoverageSnapshot, TerrainExactCoverageMode)>,
        tree_ownership: Option<&BoundedRepresentationOwnershipSnapshot<McloneTreeOccurrenceId>>,
    ) -> Result<TerrainHorizonFrameStats, String> {
        let prepared_exact = exact_coverage
            .map(|(snapshot, mode)| {
                TerrainPreparedExactFrame::new(self.engine.source(), snapshot.clone())
                    .map(|frame| (frame, mode))
            })
            .transpose()?;
        self.encode_prepared_to_target(
            device,
            queue,
            encoder,
            target,
            elapsed,
            prepared_exact.as_ref().map(|(frame, mode)| (frame, *mode)),
            tree_ownership,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode_prepared_to_target(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: TerrainHorizonRenderTarget<'_>,
        elapsed: Duration,
        exact: Option<(&TerrainPreparedExactFrame, TerrainExactCoverageMode)>,
        tree_ownership: Option<&BoundedRepresentationOwnershipSnapshot<McloneTreeOccurrenceId>>,
    ) -> Result<TerrainHorizonFrameStats, String> {
        self.encode_horizon(
            device,
            queue,
            encoder,
            Some(target),
            target.color_view,
            elapsed,
            exact,
            tree_ownership,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_horizon(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: Option<TerrainHorizonRenderTarget<'_>>,
        color_view: &wgpu::TextureView,
        elapsed: Duration,
        prepared_exact: Option<(&TerrainPreparedExactFrame, TerrainExactCoverageMode)>,
        tree_ownership: Option<&BoundedRepresentationOwnershipSnapshot<McloneTreeOccurrenceId>>,
    ) -> Result<TerrainHorizonFrameStats, String> {
        let motion_state = self.view_state;
        if let Some(intent) = self.held_motion.advance(motion_state, elapsed) {
            self.apply_intent(intent);
        }
        let view_height_blocks = self.view_state.blocks_across * f64::from(self.config.height)
            / f64::from(self.config.width);
        let presentation = TerrainHorizonPresentation::new(
            self.view_state.focus_x,
            self.view_state.focus_z,
            self.view_state.blocks_across,
            view_height_blocks,
            match self.view_state.mode {
                WorldViewMode::Map => TerrainPreviewView::Map,
                WorldViewMode::Orbit => TerrainPreviewView::ThreeDimensional,
            },
            TerrainPreviewCamera::new(
                self.view_state.yaw_radians as f32,
                self.view_state.pitch_radians as f32,
                match self.view_state.projection {
                    WorldViewProjection::Orthographic => TerrainPreviewProjectionKind::Orthographic,
                    WorldViewProjection::Perspective => TerrainPreviewProjectionKind::Perspective,
                },
            )?,
        )?;
        let presentation = match self.target_y_override {
            Some(target_y) => presentation.with_target_y(target_y)?,
            None => presentation,
        };
        let stats = match target {
            Some(target) => self.engine.encode_to_target(
                device,
                queue,
                encoder,
                target,
                presentation,
                prepared_exact,
                tree_ownership,
            )?,
            None => self.engine.encode(
                device,
                queue,
                encoder,
                color_view,
                presentation,
                prepared_exact,
                tree_ownership,
            )?,
        };
        self.note_readiness(stats, elapsed);
        self.last_stats = Some(stats);
        Ok(stats)
    }

    pub fn exact_composition_view(
        &self,
        radius_chunks: u32,
        anchor: TerrainRuntimeExactAnchor,
    ) -> Result<TerrainRuntimeExactView, String> {
        terrain_runtime_exact_view(
            self.config.seed,
            self.view_state,
            self.config.width,
            self.config.height,
            radius_chunks,
            anchor,
        )
    }

    pub fn apply_exact_composition_view(&mut self, view: TerrainRuntimeExactView) {
        self.target_y_override = Some(view.target_y);
        if self.residency_anchor != view.residency_anchor {
            self.residency_anchor = view.residency_anchor;
            self.replan_at(view.residency_anchor);
        }
    }

    pub fn restore_horizon_view(&mut self) {
        self.target_y_override = None;
        let anchor = [
            floor_i32(self.view_state.focus_x),
            floor_i32(self.view_state.focus_z),
        ];
        if self.residency_anchor != anchor {
            self.residency_anchor = anchor;
            self.replan_at(anchor);
        }
    }

    pub fn copy_depth_to_buffer(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
        bytes_per_row: u32,
    ) -> Result<(), String> {
        self.engine
            .copy_depth_to_buffer(encoder, destination, bytes_per_row)
    }

    pub fn set_depth_capture_enabled(&mut self, enabled: bool) {
        self.engine.set_depth_capture_enabled(enabled);
    }

    pub fn shutdown(&mut self) {
        self.cancel_input();
        self.engine.shutdown();
    }

    pub fn shutdown_complete(&self) -> bool {
        self.engine.shutdown_complete()
    }

    pub const fn last_stats(&self) -> Option<TerrainHorizonFrameStats> {
        self.last_stats
    }

    pub const fn process_first_coarse_ready_at(&self) -> Option<Duration> {
        self.process_first_coarse_ready_at
    }

    pub const fn process_first_target_ready_at(&self) -> Option<Duration> {
        self.process_first_target_ready_at
    }

    pub fn diagnostics(&self) -> String {
        let stats = self.last_stats;
        format!(
            "seed={} center=({}, {}) residency_anchor=({}, {}) target_y={} \
             blocks={} view={} revision={} \
             color_profile={} color_format={:?} color_transform={} \
             coarse_ready_ms={} target_ready_ms={} resident_bytes={} \
             fixed_resident_bytes={} halo_bytes={} normal_height_bytes={} \
             vegetation_bytes={} allocation_slots={} \
             ready_slots={} pending={} vegetation_ready={} vegetation_pending={} \
             tree_instances={} tree_suppressed={}:{} tree_missing={}:{} tree_ownership={}:{} \
             tree_exact={} tree_proxy={} vegetation_source={:016x} vegetation_hash={:016x} \
             vegetation_queue={} vegetation_compile_ms={:.2} finest_spacing={} \
             refills_total={} rebases_total={}",
            self.config.seed,
            self.view_state.center_x_i32(),
            self.view_state.center_z_i32(),
            self.residency_anchor[0],
            self.residency_anchor[1],
            self.target_y_override.map_or_else(
                || "sea-level".to_owned(),
                |target_y| format!("{target_y:.2}"),
            ),
            self.view_state.blocks_across_u32(),
            view_label(self.view_state.mode),
            self.revision,
            self.config.color_profile.as_str(),
            self.color_format,
            self.target_color_transform.as_str(),
            duration_ms(self.coarse_ready_at),
            duration_ms(self.target_ready_at),
            stats.map_or(0, |stats| stats.resident_bytes),
            stats.map_or(0, |stats| stats.fixed_resident_bytes),
            stats.map_or(0, |stats| stats.normal_halo_fixed_bytes),
            stats.map_or(0, |stats| stats.normal_height_fixed_bytes),
            stats.map_or(0, |stats| stats.vegetation_bytes),
            stats.map_or(0, |stats| stats.allocation_slots),
            stats.map_or(0, |stats| stats.ready_slots),
            stats.map_or(0, |stats| stats.pending_refills),
            stats.map_or(0, |stats| stats.vegetation_ready_tiles),
            stats.map_or(0, |stats| stats.pending_vegetation_tiles),
            stats.map_or(0, |stats| stats.tree_instance_count),
            stats.map_or(0, |stats| stats.tree_proxy_suppressed_instances),
            stats.map_or(0, |stats| stats.tree_proxy_suppressed_records),
            stats.map_or(0, |stats| stats.tree_proxy_missing_exact_records),
            stats.map_or(0, |stats| stats.tree_proxy_missing_proxy_records),
            stats.map_or(0, |stats| stats.tree_ownership_generation),
            stats.map_or(0, |stats| stats.tree_ownership_units),
            stats.map_or(0, |stats| stats.exact_owned_tree_records),
            stats.map_or(0, |stats| stats.proxy_owned_tree_records),
            stats.map_or(0, |stats| { stats.vegetation_service.source_fingerprint }),
            stats.map_or(0, |stats| stats.vegetation_service.record_hash),
            stats.map_or(0, |stats| stats.vegetation_service.queued_tiles),
            stats.map_or(0.0, |stats| {
                stats.vegetation_service.compile_micros as f64 / 1_000.0
            }),
            stats.map_or(0, |stats| stats.finest_sample_spacing),
            stats.map_or(0, |stats| stats.residency.total_refills),
            stats.map_or(0, |stats| stats.residency.total_rebases),
        )
    }

    fn replan(&mut self) {
        let anchor = [
            floor_i32(self.view_state.focus_x),
            floor_i32(self.view_state.focus_z),
        ];
        self.residency_anchor = anchor;
        self.replan_at(anchor);
    }

    fn replan_at(&mut self, anchor: [i32; 2]) {
        self.engine
            .set_residency(anchor[0], anchor[1], TerrainPreviewContentStage::Cover);
        self.revision = self.revision.saturating_add(1);
        self.coarse_ready_at = None;
        self.target_ready_at = None;
    }

    fn apply_double_tap(&mut self, position: ViewPoint) {
        if self.view_state.mode == WorldViewMode::Map {
            let viewport =
                ViewportMetrics::new(f64::from(self.config.width), f64::from(self.config.height));
            let anchor = viewport.normalized_anchor(position);
            let world_x = self.view_state.focus_x + anchor.x * self.view_state.blocks_across;
            let world_z = self.view_state.focus_z
                + anchor.y * self.view_state.blocks_across / viewport.aspect();
            self.view_state = self
                .view_reducer
                .reduce(
                    self.view_state,
                    WorldViewIntent::FocusAt { world_x, world_z },
                )
                .state;
        }
        self.view_state = self
            .view_reducer
            .reduce(
                self.view_state,
                WorldViewIntent::AnchoredZoom {
                    log_delta: 0.5_f64.ln(),
                    normalized_anchor: ViewPoint::default(),
                    viewport: ViewportMetrics::new(
                        f64::from(self.config.width),
                        f64::from(self.config.height),
                    ),
                },
            )
            .state;
    }

    fn note_readiness(&mut self, stats: TerrainHorizonFrameStats, elapsed: Duration) {
        if stats.coarse_ready && self.coarse_ready_at.is_none() {
            self.coarse_ready_at = Some(elapsed);
            if self.process_first_coarse_ready_at.is_none() {
                self.process_first_coarse_ready_at = Some(elapsed);
            }
        }
        if stats.target_ready && self.target_ready_at.is_none() {
            self.target_ready_at = Some(elapsed);
            if self.process_first_target_ready_at.is_none() {
                self.process_first_target_ready_at = Some(elapsed);
            }
        }
    }
}

fn residency_plan_key(state: WorldViewState, config: TerrainClipmapConfig) -> (i32, i32) {
    let footprint = f64::from(config.finest_tile_footprint_blocks());
    (
        (state.focus_x / footprint).floor() as i32,
        (state.focus_z / footprint).floor() as i32,
    )
}

fn floor_i32(value: f64) -> i32 {
    value
        .floor()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

fn terrain_runtime_exact_view(
    seed: i64,
    state: WorldViewState,
    width: u32,
    height: u32,
    radius_chunks: u32,
    exact_anchor: TerrainRuntimeExactAnchor,
) -> Result<TerrainRuntimeExactView, String> {
    let view_height_blocks = state.blocks_across * f64::from(height) / f64::from(width);
    let presentation = TerrainHorizonPresentation::new(
        state.focus_x,
        state.focus_z,
        state.blocks_across,
        view_height_blocks,
        match state.mode {
            WorldViewMode::Map => TerrainPreviewView::Map,
            WorldViewMode::Orbit => TerrainPreviewView::ThreeDimensional,
        },
        TerrainPreviewCamera::new(
            state.yaw_radians as f32,
            state.pitch_radians as f32,
            match state.projection {
                WorldViewProjection::Orthographic => TerrainPreviewProjectionKind::Orthographic,
                WorldViewProjection::Perspective => TerrainPreviewProjectionKind::Perspective,
            },
        )?,
    )?;
    let base_render_view = crate::terrain_horizon_chunk_render_view(presentation, width, height)?;
    let target_y = if state.mode == WorldViewMode::Orbit {
        let viewer_surface_y = terrain_preview_focus_y_for_profile(
            TerrainPreviewProfile::McloneOverworldV1,
            seed,
            floor_i32(f64::from(base_render_view.camera_position.x)),
            floor_i32(f64::from(base_render_view.camera_position.z)),
        );
        let eye_offset_y = base_render_view.camera_position.y - presentation.target_y;
        presentation
            .target_y
            .max(viewer_surface_y + COMPOSED_ORBIT_VIEWER_CLEARANCE_BLOCKS - eye_offset_y)
    } else {
        MCLONE_OVERWORLD_SEA_LEVEL as f32
    };
    let presentation = presentation.with_target_y(target_y)?;
    let render_view = crate::terrain_horizon_chunk_render_view(presentation, width, height)?;
    let mut anchor_x = state.focus_x as f32;
    let mut anchor_z = state.focus_z as f32;
    if state.mode == WorldViewMode::Orbit
        && exact_anchor == TerrainRuntimeExactAnchor::ViewerForward
    {
        anchor_x = render_view.camera_position.x;
        anchor_z = render_view.camera_position.z;
        let forward_length = render_view
            .camera_forward
            .x
            .hypot(render_view.camera_forward.z);
        if forward_length > f32::EPSILON {
            // Put the viewer one half-chunk behind the odd-sized chunk square so
            // the bounded exact proof occupies visible ground ahead of an orbit
            // camera and its tree crowns remain inside the horizon's vegetation
            // record domain.
            let forward_blocks = radius_chunks.saturating_mul(16).saturating_add(16) as f32;
            anchor_x += render_view.camera_forward.x / forward_length * forward_blocks;
            anchor_z += render_view.camera_forward.z / forward_length * forward_blocks;
        }
    }
    Ok(TerrainRuntimeExactView {
        residency_anchor: [
            floor_i32(f64::from(anchor_x)),
            floor_i32(f64::from(anchor_z)),
        ],
        render_view,
        target_y,
    })
}

fn view_label(mode: WorldViewMode) -> &'static str {
    match mode {
        WorldViewMode::Map => "map",
        WorldViewMode::Orbit => "3d",
    }
}

fn duration_ms(duration: Option<Duration>) -> String {
    duration.map_or_else(
        || "pending".to_owned(),
        |duration| format!("{:.2}", duration.as_secs_f64() * 1_000.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_mode_labels_round_trip() {
        for mode in [
            TerrainRuntimeCompositionMode::Horizon,
            TerrainRuntimeCompositionMode::Exact,
            TerrainRuntimeCompositionMode::Composed,
            TerrainRuntimeCompositionMode::Coverage,
        ] {
            assert_eq!(
                TerrainRuntimeCompositionMode::parse_label(mode.label()).unwrap(),
                mode
            );
        }
    }

    #[test]
    fn exact_anchor_labels_round_trip() {
        for anchor in [
            TerrainRuntimeExactAnchor::Focus,
            TerrainRuntimeExactAnchor::ViewerForward,
        ] {
            assert_eq!(
                TerrainRuntimeExactAnchor::parse_label(anchor.label()).unwrap(),
                anchor
            );
        }
        assert_eq!(
            TerrainRuntimeExactAnchor::parse_label("eye").unwrap(),
            TerrainRuntimeExactAnchor::ViewerForward
        );
        assert!(TerrainRuntimeExactAnchor::parse_label("camera-target").is_err());
    }

    #[test]
    fn residency_key_only_changes_at_finest_tile_boundaries() {
        let config = TerrainClipmapConfig::default();
        let state = WorldViewState {
            focus_x: 31.25,
            focus_z: 47.75,
            ..WorldViewState::default()
        };
        let key = residency_plan_key(state, config);

        for changed in [
            WorldViewState {
                focus_x: 63.999,
                ..state
            },
            WorldViewState {
                focus_z: 0.001,
                ..state
            },
            WorldViewState {
                blocks_across: 256.5,
                ..state
            },
            WorldViewState {
                yaw_radians: 1.25,
                pitch_radians: 0.25,
                ..state
            },
        ] {
            assert_eq!(residency_plan_key(changed, config), key);
        }

        assert_ne!(
            residency_plan_key(
                WorldViewState {
                    focus_x: 64.0,
                    ..state
                },
                config,
            ),
            key
        );
    }

    #[test]
    fn residency_key_uses_floor_for_negative_coordinates() {
        let config = TerrainClipmapConfig::default();
        let state = WorldViewState {
            focus_x: -0.001,
            focus_z: -64.0,
            ..WorldViewState::default()
        };

        assert_eq!(residency_plan_key(state, config), (-1, -1));
        assert_eq!(floor_i32(-0.001), -1);
        assert_eq!(floor_i32(0.001), 0);
    }

    #[test]
    fn orbit_exact_near_field_uses_the_focus_by_default() {
        let state = WorldViewState {
            focus_x: -0.25,
            focus_z: 17.75,
            blocks_across: 96.0,
            yaw_radians: 0.0,
            pitch_radians: 0.12,
            mode: WorldViewMode::Orbit,
            projection: WorldViewProjection::Perspective,
        };

        for yaw_radians in [
            0.0,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::PI,
            std::f64::consts::PI * 1.5,
        ] {
            let view = terrain_runtime_exact_view(
                12_345,
                WorldViewState {
                    yaw_radians,
                    ..state
                },
                1280,
                720,
                2,
                TerrainRuntimeExactAnchor::Focus,
            )
            .unwrap();
            assert_eq!(view.residency_anchor, [-1, 17]);
        }
    }

    #[test]
    fn orbit_exact_near_field_can_use_the_viewer_forward_anchor() {
        let state = WorldViewState {
            focus_x: 0.0,
            focus_z: 0.0,
            blocks_across: 96.0,
            yaw_radians: 0.0,
            pitch_radians: 0.12,
            mode: WorldViewMode::Orbit,
            projection: WorldViewProjection::Perspective,
        };

        let east = terrain_runtime_exact_view(
            12_345,
            state,
            1280,
            720,
            2,
            TerrainRuntimeExactAnchor::ViewerForward,
        )
        .unwrap();
        let west = terrain_runtime_exact_view(
            12_345,
            WorldViewState {
                yaw_radians: std::f64::consts::PI,
                ..state
            },
            1280,
            720,
            2,
            TerrainRuntimeExactAnchor::ViewerForward,
        )
        .unwrap();

        assert!(east.residency_anchor[0] > 100);
        assert_eq!(east.residency_anchor[1], 0);
        assert!(west.residency_anchor[0] < -100);
        assert!(
            east.residency_anchor[0] < floor_i32(f64::from(east.render_view.camera_position.x))
        );
        assert!(
            west.residency_anchor[0] > floor_i32(f64::from(west.render_view.camera_position.x))
        );
        for yaw_radians in [
            0.0,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::PI,
            std::f64::consts::PI * 1.5,
        ] {
            let view = terrain_runtime_exact_view(
                12_345,
                WorldViewState {
                    yaw_radians,
                    ..state
                },
                1280,
                720,
                2,
                TerrainRuntimeExactAnchor::ViewerForward,
            )
            .unwrap();
            let surface_y = terrain_preview_focus_y_for_profile(
                TerrainPreviewProfile::McloneOverworldV1,
                12_345,
                floor_i32(f64::from(view.render_view.camera_position.x)),
                floor_i32(f64::from(view.render_view.camera_position.z)),
            );
            assert!(
                view.render_view.camera_position.y
                    >= surface_y + COMPOSED_ORBIT_VIEWER_CLEARANCE_BLOCKS
            );
        }
    }

    #[test]
    fn map_exact_near_field_remains_under_the_focus() {
        let view = terrain_runtime_exact_view(
            12_345,
            WorldViewState {
                focus_x: -0.25,
                focus_z: 17.75,
                mode: WorldViewMode::Map,
                ..WorldViewState::default()
            },
            1280,
            720,
            2,
            TerrainRuntimeExactAnchor::Focus,
        )
        .unwrap();

        assert_eq!(view.residency_anchor, [-1, 17]);
    }
}
