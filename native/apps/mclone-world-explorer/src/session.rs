use std::time::Duration;

use mclone_render_color::{RenderColorProfile, RenderTargetColorTransform};
use mclone_terrain_view::{
    ExactPaintedCoverageSnapshot, TerrainClipmapConfig, TerrainExactCoverageMode,
    TerrainHorizonFrameStats, TerrainHorizonPresentation, TerrainHorizonRenderTarget,
    TerrainHorizonRenderer, TerrainPreviewCamera, TerrainPreviewMaterialAtlas,
    TerrainPreviewProjectionKind, TerrainPreviewView, TerrainVegetationExecutor,
};
use mclone_view_control::{
    ContactEvent, ContactGestureReducer, ViewPoint, ViewportMetrics, WorldViewHeldDirection,
    WorldViewHeldMotion, WorldViewIntent, WorldViewMode, WorldViewProjection, WorldViewReducer,
    WorldViewSignal, WorldViewState,
};
use mclone_worldgen::terrain_preview::TerrainPreviewContentStage;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldExplorerConfig {
    pub width: u32,
    pub height: u32,
    pub seed: i64,
    pub initial_view: WorldViewState,
    pub clipmap: TerrainClipmapConfig,
    pub vegetation_enabled: bool,
    pub color_profile: RenderColorProfile,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WorldExplorerCompositionMode {
    #[default]
    Horizon,
    Exact,
    Composed,
    Coverage,
}

impl WorldExplorerCompositionMode {
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
                "unsupported World Explorer composition mode {other:?}; expected horizon, exact, \
                 composed, or coverage"
            )),
        }
    }
}

pub struct WorldExplorerSession {
    renderer: TerrainHorizonRenderer,
    config: WorldExplorerConfig,
    color_format: wgpu::TextureFormat,
    target_color_transform: RenderTargetColorTransform,
    view_state: WorldViewState,
    view_reducer: WorldViewReducer,
    contacts: ContactGestureReducer,
    held_motion: WorldViewHeldMotion,
    revision: u64,
    coarse_ready_at: Option<Duration>,
    target_ready_at: Option<Duration>,
    process_first_coarse_ready_at: Option<Duration>,
    process_first_target_ready_at: Option<Duration>,
    last_stats: Option<TerrainHorizonFrameStats>,
}

impl WorldExplorerSession {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        config: WorldExplorerConfig,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
        vegetation_executor: Option<Box<dyn TerrainVegetationExecutor>>,
    ) -> Result<Self, String> {
        if config.vegetation_enabled != vegetation_executor.is_some() {
            return Err(
                "World Explorer vegetation enablement must match executor availability".to_owned(),
            );
        }
        let view_reducer = WorldViewReducer::default();
        let view_state = view_reducer.normalize(config.initial_view);
        let target_color_transform = config.color_profile.target_color_transform(color_format);
        let renderer = TerrainHorizonRenderer::new_with_target_color_transform(
            device,
            queue,
            color_format,
            config.width,
            config.height,
            material_atlas,
            config.clipmap,
            vegetation_executor,
            target_color_transform,
        )?;
        let mut session = Self {
            renderer,
            config,
            color_format,
            target_color_transform,
            view_state,
            view_reducer,
            contacts: ContactGestureReducer::default(),
            held_motion: WorldViewHeldMotion::default(),
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
        self.renderer.resize(device, width, height);
        if self.config.width != width || self.config.height != height {
            self.config.width = width;
            self.config.height = height;
        }
    }

    pub const fn view_state(&self) -> WorldViewState {
        self.view_state
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
        self.encode_horizon(device, queue, encoder, None, color_view, elapsed, None)
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
    ) -> Result<TerrainHorizonFrameStats, String> {
        self.encode_horizon(
            device,
            queue,
            encoder,
            Some(target),
            target.color_view,
            elapsed,
            exact_coverage,
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
        exact_coverage: Option<(&ExactPaintedCoverageSnapshot, TerrainExactCoverageMode)>,
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
        if let Some((snapshot, mode)) = exact_coverage {
            self.renderer
                .set_exact_painted_coverage(queue, snapshot, mode)?;
        } else {
            self.renderer.clear_exact_painted_coverage();
        }
        let stats = match target {
            Some(target) => self.renderer.encode_to_target(
                device,
                queue,
                encoder,
                target,
                self.config.width,
                self.config.height,
                presentation,
            )?,
            None => self.renderer.encode(
                device,
                queue,
                encoder,
                color_view,
                self.config.width,
                self.config.height,
                presentation,
            )?,
        };
        self.note_readiness(stats, elapsed);
        self.last_stats = Some(stats);
        Ok(stats)
    }

    pub fn exact_render_view(&self) -> Result<mclone_render::chunk::ChunkRenderView, String> {
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
        mclone_terrain_view::terrain_horizon_chunk_render_view(
            presentation,
            self.config.width,
            self.config.height,
        )
    }

    pub fn copy_depth_to_buffer(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
        bytes_per_row: u32,
    ) -> Result<(), String> {
        self.renderer
            .copy_depth_to_buffer(encoder, destination, bytes_per_row)
    }

    pub fn set_depth_capture_enabled(&mut self, enabled: bool) {
        self.renderer.set_depth_capture_enabled(enabled);
    }

    pub fn shutdown(&mut self) {
        self.cancel_input();
        self.renderer.shutdown_vegetation();
    }

    pub fn shutdown_complete(&self) -> bool {
        self.renderer.vegetation_shutdown_complete()
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
            "seed={} center=({}, {}) blocks={} view={} revision={} \
             color_profile={} color_format={:?} color_transform={} \
             coarse_ready_ms={} target_ready_ms={} resident_bytes={} \
             fixed_resident_bytes={} halo_bytes={} normal_height_bytes={} \
             vegetation_bytes={} allocation_slots={} \
             ready_slots={} pending={} vegetation_ready={} vegetation_pending={} \
             tree_instances={} vegetation_source={:016x} vegetation_hash={:016x} \
             vegetation_queue={} vegetation_compile_ms={:.2} finest_spacing={} \
             refills_total={} rebases_total={}",
            self.config.seed,
            self.view_state.center_x_i32(),
            self.view_state.center_z_i32(),
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
        self.renderer.set_view(
            self.config.seed,
            floor_i32(self.view_state.focus_x),
            floor_i32(self.view_state.focus_z),
            TerrainPreviewContentStage::Cover,
        );
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
}
