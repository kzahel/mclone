use std::time::Duration;

use mclone_terrain_view::{
    TerrainClipmapConfig, TerrainHorizonFrameStats, TerrainHorizonRenderer, TerrainPreviewCamera,
    TerrainPreviewMaterialAtlas, TerrainPreviewProjectionKind, TerrainPreviewView,
};
use mclone_view_control::{
    ContactEvent, ContactGestureReducer, ViewPoint, ViewportMetrics, WorldViewIntent,
    WorldViewMode, WorldViewProjection, WorldViewReducer, WorldViewSignal, WorldViewState,
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
}

pub struct WorldExplorerSession {
    renderer: TerrainHorizonRenderer,
    config: WorldExplorerConfig,
    view_state: WorldViewState,
    view_reducer: WorldViewReducer,
    contacts: ContactGestureReducer,
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
    ) -> Result<Self, String> {
        let view_reducer = WorldViewReducer::default();
        let view_state = view_reducer.normalize(config.initial_view);
        let renderer = TerrainHorizonRenderer::new(
            device,
            queue,
            color_format,
            config.width,
            config.height,
            material_atlas,
            config.clipmap,
            config.vegetation_enabled,
        )?;
        let mut session = Self {
            renderer,
            config,
            view_state,
            view_reducer,
            contacts: ContactGestureReducer::default(),
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
            self.replan();
        }
    }

    pub const fn view_state(&self) -> WorldViewState {
        self.view_state
    }

    pub const fn revision(&self) -> u64 {
        self.revision
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
        if view_plan_key(self.view_state) != view_plan_key(previous) {
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

    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        elapsed: Duration,
    ) -> Result<TerrainHorizonFrameStats, String> {
        let stats = self.renderer.encode(
            device,
            queue,
            encoder,
            color_view,
            self.config.width,
            self.config.height,
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
        self.note_readiness(stats, elapsed);
        self.last_stats = Some(stats);
        Ok(stats)
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
             coarse_ready_ms={} target_ready_ms={} resident_bytes={} \
             fixed_resident_bytes={} vegetation_bytes={} allocation_slots={} \
             ready_slots={} pending={} vegetation_ready={} vegetation_pending={} \
             tree_instances={} finest_spacing={} \
             refills_total={} rebases_total={}",
            self.config.seed,
            self.view_state.center_x_i32(),
            self.view_state.center_z_i32(),
            self.view_state.blocks_across_u32(),
            view_label(self.view_state.mode),
            self.revision,
            duration_ms(self.coarse_ready_at),
            duration_ms(self.target_ready_at),
            stats.map_or(0, |stats| stats.resident_bytes),
            stats.map_or(0, |stats| stats.fixed_resident_bytes),
            stats.map_or(0, |stats| stats.vegetation_bytes),
            stats.map_or(0, |stats| stats.allocation_slots),
            stats.map_or(0, |stats| stats.ready_slots),
            stats.map_or(0, |stats| stats.pending_refills),
            stats.map_or(0, |stats| stats.vegetation_ready_tiles),
            stats.map_or(0, |stats| stats.pending_vegetation_tiles),
            stats.map_or(0, |stats| stats.tree_instance_count),
            stats.map_or(0, |stats| stats.finest_sample_spacing),
            stats.map_or(0, |stats| stats.residency.total_refills),
            stats.map_or(0, |stats| stats.residency.total_rebases),
        )
    }

    fn replan(&mut self) {
        let view_height_blocks = u32::try_from(
            u64::from(self.view_state.blocks_across_u32())
                .saturating_mul(u64::from(self.config.height))
                .div_ceil(u64::from(self.config.width)),
        )
        .unwrap_or(u32::MAX)
        .max(1);
        self.renderer.set_view(
            self.config.seed,
            self.view_state.center_x_i32(),
            self.view_state.center_z_i32(),
            self.view_state.blocks_across_u32(),
            view_height_blocks,
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

fn view_plan_key(state: WorldViewState) -> (i32, i32, u32) {
    (
        state.center_x_i32(),
        state.center_z_i32(),
        state.blocks_across_u32(),
    )
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
