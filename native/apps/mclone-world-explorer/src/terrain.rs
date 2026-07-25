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
    TerrainPreviewSplitLayout, TerrainViewportDetail, TerrainViewportFrameStats,
    TerrainViewportRenderer, TerrainViewportRequest, plan_terrain_viewport,
};
use mclone_worldgen::terrain_preview::{TerrainPreviewContentStage, TerrainPreviewProfile};

use crate::options::{ExplorerAssetProfile, ExplorerOptions};

pub struct ExplorerTerrain {
    renderer: TerrainViewportRenderer,
    options: ExplorerOptions,
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
        let mut terrain = Self {
            renderer,
            options,
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
                    view: self.options.view,
                    layer: TerrainPreviewLayer::Terrain,
                    split_layout: TerrainPreviewSplitLayout::Columns,
                },
                TerrainPreviewCamera::new(
                    self.options.yaw_radians,
                    self.options.pitch_radians,
                    self.options.projection,
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
            self.options.center_x,
            self.options.center_z,
            self.options.blocks_across,
            self.options.view_label(),
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
            center_x: self.options.center_x,
            center_z: self.options.center_z,
            blocks_across: self.options.blocks_across,
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
