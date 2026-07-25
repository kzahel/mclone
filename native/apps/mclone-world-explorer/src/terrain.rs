use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use mclone_assets::{
    AUTHORED_FIRST_PARTY_PACK_ID, AssetPackId, AssetPackOrigin, AssetSourceChain,
    DIAGNOSTIC_MISSING_PACK_ID, PROVISIONAL_FIRST_PARTY_PACK_ID, PackedAssetSource,
    TexturePresentation,
};
use mclone_core::BlockStateId;
use mclone_mesh::{
    TexturedTerrainAssets, load_first_party_textured_terrain_assets_with_presentation,
};
use mclone_terrain_view::{
    TERRAIN_PREVIEW_MATERIAL_UV_COUNT, TerrainPreviewCamera, TerrainPreviewDrawOptions,
    TerrainPreviewLayer, TerrainPreviewMaterialAtlas, TerrainPreviewSource,
    TerrainPreviewSplitLayout, TerrainPreviewView, TerrainViewportDetail,
    TerrainViewportFrameStats, TerrainViewportRenderer, TerrainViewportRequest,
    plan_terrain_viewport,
};
use mclone_view_control::{
    ViewPoint, ViewportMetrics, WorldViewIntent, WorldViewMode, WorldViewProjection,
    WorldViewReducer, WorldViewSignal, WorldViewState,
};
use mclone_worldgen::terrain_preview::{TerrainPreviewContentStage, TerrainPreviewProfile};

use crate::options::{ExplorerAssetProfile, ExplorerOptions};

pub struct ExplorerTerrain {
    renderer: TerrainViewportRenderer,
    options: ExplorerOptions,
    view_state: WorldViewState,
    view_reducer: WorldViewReducer,
    revision: u64,
    started: Instant,
    coarse_ready_at: Option<Duration>,
    target_ready_at: Option<Duration>,
    last_stats: Option<TerrainViewportFrameStats>,
}

impl ExplorerTerrain {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        options: ExplorerOptions,
        started: Instant,
    ) -> Result<Self> {
        let assets = load_assets(&options.asset_root, options.asset_profile)?;
        let mut material_uvs = [[0.0_f32, 0.0, 1.0, 1.0]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT];
        for (raw_id, target) in material_uvs.iter_mut().enumerate() {
            if let Some(sprite) = assets.catalog.gui_icon_uv(BlockStateId(raw_id as u32)) {
                *target = [sprite.u0, sprite.v0, sprite.u1, sprite.v1];
            }
        }
        let renderer = TerrainViewportRenderer::new(
            device,
            queue,
            color_format,
            options.width,
            options.height,
            TerrainPreviewMaterialAtlas {
                width: assets.atlas.width,
                height: assets.atlas.height,
                rgba: assets.atlas.rgba(),
                material_uvs: &material_uvs,
            },
        )
        .map_err(anyhow::Error::msg)?;
        let view_reducer = WorldViewReducer::default();
        let view_state = view_reducer.normalize(options.initial_view_state());
        let mut terrain = Self {
            renderer,
            options,
            view_state,
            view_reducer,
            revision: 0,
            started,
            coarse_ready_at: None,
            target_ready_at: None,
            last_stats: None,
        };
        terrain.replan()?;
        Ok(terrain)
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) -> Result<()> {
        let width = width.max(1);
        let height = height.max(1);
        self.renderer.resize(device, width, height);
        if self.options.width != width || self.options.height != height {
            self.options.width = width;
            self.options.height = height;
            self.replan()?;
        }
        Ok(())
    }

    pub const fn view_state(&self) -> WorldViewState {
        self.view_state
    }

    pub fn title(&self) -> String {
        format!(
            "Mclone World Explorer — seed {} — ({}, {}) — {} blocks — {}",
            self.options.seed,
            self.view_state.center_x_i32(),
            self.view_state.center_z_i32(),
            self.view_state.blocks_across_u32(),
            view_label(self.view_state.mode),
        )
    }

    pub fn apply_intent(&mut self, intent: WorldViewIntent) -> Result<bool> {
        let previous = self.view_state;
        let reduction = self.view_reducer.reduce(previous, intent);
        self.view_state = reduction.state;
        if let Some(WorldViewSignal::DoubleTap { position }) = reduction.signal {
            self.apply_double_tap(position);
        }
        if self.view_state == previous {
            return Ok(false);
        }
        if view_plan_key(self.view_state) != view_plan_key(previous) {
            self.replan()?;
        }
        Ok(true)
    }

    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
    ) -> Result<TerrainViewportFrameStats> {
        let now = Instant::now();
        let stats = self
            .renderer
            .encode(
                device,
                queue,
                encoder,
                color_view,
                self.options.width,
                self.options.height,
                TerrainPreviewDrawOptions {
                    source: TerrainPreviewSource::Gpu,
                    view: match self.view_state.mode {
                        WorldViewMode::Map => TerrainPreviewView::Map,
                        WorldViewMode::Orbit => TerrainPreviewView::ThreeDimensional,
                    },
                    layer: TerrainPreviewLayer::Terrain,
                    split_layout: TerrainPreviewSplitLayout::Columns,
                },
                TerrainPreviewCamera::new(
                    self.view_state.yaw_radians as f32,
                    self.view_state.pitch_radians as f32,
                    match self.view_state.projection {
                        WorldViewProjection::Orthographic => {
                            mclone_terrain_view::TerrainPreviewProjectionKind::Orthographic
                        }
                        WorldViewProjection::Perspective => {
                            mclone_terrain_view::TerrainPreviewProjectionKind::Perspective
                        }
                    },
                )
                .map_err(anyhow::Error::msg)?,
                || now.elapsed().as_secs_f64() * 1_000.0,
            )
            .map_err(anyhow::Error::msg)?;
        self.note_readiness(stats);
        self.last_stats = Some(stats);
        Ok(stats)
    }

    pub fn poll_completed(&mut self, device: &wgpu::Device) -> Result<()> {
        for result in self.renderer.poll_completed(device) {
            result.map_err(anyhow::Error::msg)?;
        }
        Ok(())
    }

    pub fn diagnostics(&self) -> String {
        let stats = self.last_stats;
        format!(
            "seed={} center=({}, {}) blocks={} view={} revision={} \
             coarse_ready_ms={} target_ready_ms={} resident_bytes={} \
             resident_tiles={} pending={} published_spacing={}",
            self.options.seed,
            self.view_state.center_x_i32(),
            self.view_state.center_z_i32(),
            self.view_state.blocks_across_u32(),
            view_label(self.view_state.mode),
            self.revision,
            duration_ms(self.coarse_ready_at),
            duration_ms(self.target_ready_at),
            stats.map_or(0, |stats| stats.resident_bytes),
            stats.map_or(0, |stats| stats.resident_tile_count),
            stats.map_or(0, |stats| {
                stats.queued_tile_count + stats.pending_readback_count
            }),
            stats.map_or(0, |stats| stats.published_spacing),
        )
    }

    fn replan(&mut self) -> Result<()> {
        let plan = plan_terrain_viewport(TerrainViewportRequest {
            profile: TerrainPreviewProfile::McloneOverworldV1,
            seed: self.options.seed,
            center_x: self.view_state.center_x_i32(),
            center_z: self.view_state.center_z_i32(),
            blocks_across: self.view_state.blocks_across_u32(),
            panel_width_css: self.options.width,
            panel_height_css: self.options.height,
            detail: TerrainViewportDetail::Auto,
            max_visible_tiles_per_axis: 8,
            content_stage: TerrainPreviewContentStage::Cover,
        })
        .map_err(anyhow::Error::msg)?;
        self.revision = self.revision.saturating_add(1);
        self.renderer.set_viewport(self.revision, plan);
        self.coarse_ready_at = None;
        self.target_ready_at = None;
        Ok(())
    }

    fn apply_double_tap(&mut self, position: ViewPoint) {
        if self.view_state.mode == WorldViewMode::Map {
            let viewport = ViewportMetrics::new(
                f64::from(self.options.width),
                f64::from(self.options.height),
            );
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
                        f64::from(self.options.width),
                        f64::from(self.options.height),
                    ),
                },
            )
            .state;
    }

    fn note_readiness(&mut self, stats: TerrainViewportFrameStats) {
        let elapsed = self.started.elapsed();
        if stats.coarse_ready && self.coarse_ready_at.is_none() {
            self.coarse_ready_at = Some(elapsed);
            log::info!(
                "World Explorer first coarse frame ready in {:.2} ms",
                elapsed.as_secs_f64() * 1_000.0
            );
        }
        if stats.target_ready && self.target_ready_at.is_none() {
            self.target_ready_at = Some(elapsed);
            log::info!(
                "World Explorer target frame ready in {:.2} ms",
                elapsed.as_secs_f64() * 1_000.0
            );
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

fn load_assets(root: &Path, profile: ExplorerAssetProfile) -> Result<TexturedTerrainAssets> {
    let mut source = AssetSourceChain::new();
    if profile == ExplorerAssetProfile::Original {
        source.push_named(
            AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID),
            AssetPackOrigin::FirstParty,
            load_pack(root, "mclone-authored.pbp")?,
        );
    }
    source.push_named(
        AssetPackId::new(PROVISIONAL_FIRST_PARTY_PACK_ID),
        AssetPackOrigin::FirstPartyProvisional,
        load_pack(root, "mclone-generated-fallback.pbp")?,
    );
    let diagnostic_path = root.join("mclone-diagnostic-missing.pbp");
    if diagnostic_path.is_file() {
        source.push_named(
            AssetPackId::new(DIAGNOSTIC_MISSING_PACK_ID),
            AssetPackOrigin::Diagnostic,
            PackedAssetSource::from_file(&diagnostic_path).with_context(|| {
                format!(
                    "failed to open diagnostic asset pack {}",
                    diagnostic_path.display()
                )
            })?,
        );
    }
    load_first_party_textured_terrain_assets_with_presentation(
        &source,
        TexturePresentation::Textured,
    )
    .map_err(anyhow::Error::new)
    .with_context(|| {
        format!(
            "failed to prepare World Explorer {} materials from {}",
            profile.label(),
            root.display()
        )
    })
}

fn load_pack(root: &Path, file_name: &str) -> Result<PackedAssetSource> {
    let path = root.join(file_name);
    PackedAssetSource::from_file(&path).with_context(|| {
        format!(
            "failed to open World Explorer asset pack {}",
            path.display()
        )
    })
}

fn duration_ms(duration: Option<Duration>) -> String {
    duration.map_or_else(
        || "pending".to_owned(),
        |duration| format!("{:.2}", duration.as_secs_f64() * 1_000.0),
    )
}
