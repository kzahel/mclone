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
    TERRAIN_PREVIEW_MATERIAL_UV_COUNT, TerrainClipmapConfig, TerrainHorizonFrameStats,
    TerrainPreviewMaterialAtlas,
};
use mclone_view_control::{WorldViewHeldDirection, WorldViewIntent, WorldViewState};
use mclone_world_explorer::{WorldExplorerConfig, WorldExplorerSession};

use crate::options::{ExplorerAssetProfile, ExplorerOptions};

pub struct ExplorerTerrain {
    session: WorldExplorerSession,
    options: ExplorerOptions,
    started: Instant,
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
        let session = WorldExplorerSession::new(
            device,
            queue,
            color_format,
            WorldExplorerConfig {
                width: options.width,
                height: options.height,
                seed: options.seed,
                initial_view: options.initial_view_state(),
                clipmap: TerrainClipmapConfig::default(),
                vegetation_enabled: true,
            },
            TerrainPreviewMaterialAtlas {
                width: assets.atlas.width,
                height: assets.atlas.height,
                rgba: assets.atlas.rgba(),
                material_uvs: &material_uvs,
            },
        )
        .map_err(anyhow::Error::msg)?;
        Ok(Self {
            session,
            options,
            started,
        })
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) -> Result<()> {
        self.options.width = width.max(1);
        self.options.height = height.max(1);
        self.session
            .resize(device, self.options.width, self.options.height);
        Ok(())
    }

    pub const fn view_state(&self) -> WorldViewState {
        self.session.view_state()
    }

    pub fn title(&self) -> String {
        let state = self.session.view_state();
        format!(
            "Mclone World Explorer — seed {} — ({}, {}) — {} blocks — {}",
            self.options.seed,
            state.center_x_i32(),
            state.center_z_i32(),
            state.blocks_across_u32(),
            match state.mode {
                mclone_view_control::WorldViewMode::Map => "map",
                mclone_view_control::WorldViewMode::Orbit => "3d",
            },
        )
    }

    pub fn apply_intent(&mut self, intent: WorldViewIntent) -> Result<bool> {
        Ok(self.session.apply_intent(intent))
    }

    pub fn set_held_motion(&mut self, direction: WorldViewHeldDirection, pressed: bool) -> bool {
        self.session.set_held_motion(direction, pressed)
    }

    pub const fn has_held_motion(&self) -> bool {
        self.session.has_held_motion()
    }

    pub fn cancel_input(&mut self) -> bool {
        self.session.cancel_input()
    }

    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
    ) -> Result<TerrainHorizonFrameStats> {
        self.session
            .encode(device, queue, encoder, color_view, self.started.elapsed())
            .map_err(anyhow::Error::msg)
    }

    pub fn poll_completed(&mut self, _device: &wgpu::Device) -> Result<()> {
        Ok(())
    }

    pub fn copy_depth_to_buffer(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
        bytes_per_row: u32,
    ) -> Result<()> {
        self.session
            .copy_depth_to_buffer(encoder, destination, bytes_per_row)
            .map_err(anyhow::Error::msg)
    }

    pub fn set_depth_capture_enabled(&mut self, enabled: bool) {
        self.session.set_depth_capture_enabled(enabled);
    }

    pub const fn last_stats(&self) -> Option<TerrainHorizonFrameStats> {
        self.session.last_stats()
    }

    pub const fn process_first_coarse_ready_at(&self) -> Option<Duration> {
        self.session.process_first_coarse_ready_at()
    }

    pub const fn process_first_target_ready_at(&self) -> Option<Duration> {
        self.session.process_first_target_ready_at()
    }

    pub fn diagnostics(&self) -> String {
        self.session.diagnostics()
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
