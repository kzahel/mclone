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
use mclone_render::chunk::ChunkTextureAtlas;
use mclone_render_color::RenderColorProfile;
use mclone_terrain_view::{
    TERRAIN_PREVIEW_MATERIAL_UV_COUNT, TerrainClipmapConfig, TerrainExactCoverageMode,
    TerrainHorizonFrameStats, TerrainHorizonRenderTarget, TerrainPreviewMaterialAtlas,
};
use mclone_view_control::{WorldViewHeldDirection, WorldViewIntent, WorldViewState};
use mclone_world_explorer::{
    ExplorerExactStats, ExplorerExactTerrain, NativeTerrainVegetationExecutor,
    WorldExplorerCompositionMode, WorldExplorerConfig, WorldExplorerSession,
};

use crate::options::{ExplorerAssetProfile, ExplorerOptions};

pub struct ExplorerTerrain {
    session: WorldExplorerSession,
    options: ExplorerOptions,
    started: Instant,
    exact: ExplorerExactTerrain,
    composition: WorldExplorerCompositionMode,
    last_exact_stats: ExplorerExactStats,
    last_exact_anchor: [i32; 2],
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
        let target_color_transform =
            RenderColorProfile::Vanilla.target_color_transform(color_format);
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
                color_profile: RenderColorProfile::Vanilla,
            },
            TerrainPreviewMaterialAtlas {
                width: assets.atlas.width,
                height: assets.atlas.height,
                rgba: assets.atlas.rgba(),
                material_uvs: &material_uvs,
            },
            Some(Box::new(NativeTerrainVegetationExecutor::new())),
        )
        .map_err(anyhow::Error::msg)?;
        let exact = ExplorerExactTerrain::new(
            device,
            queue,
            color_format,
            options.width,
            options.height,
            options.seed,
            options.exact_radius,
            Duration::from_millis(options.exact_delay_ms),
            assets.catalog.clone(),
            ChunkTextureAtlas {
                width: assets.atlas.width,
                height: assets.atlas.height,
                rgba: assets.atlas.rgba(),
            },
            options.source_colors,
            target_color_transform,
        )?;
        let composition = options.composition;
        let initial_exact_anchor = [options.center_x, options.center_z];
        Ok(Self {
            session,
            options,
            started,
            exact,
            composition,
            last_exact_stats: ExplorerExactStats::default(),
            last_exact_anchor: initial_exact_anchor,
        })
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) -> Result<()> {
        self.options.width = width.max(1);
        self.options.height = height.max(1);
        self.session
            .resize(device, self.options.width, self.options.height);
        self.exact
            .resize(device, self.options.width, self.options.height);
        Ok(())
    }

    pub const fn view_state(&self) -> WorldViewState {
        self.session.view_state()
    }

    pub fn title(&self) -> String {
        let state = self.session.view_state();
        format!(
            "Mclone World Explorer — {} [1–4] — seed {} — ({}, {}) — {} blocks — {}",
            self.composition.label(),
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

    pub fn set_composition_mode(&mut self, mode: WorldExplorerCompositionMode) -> bool {
        if self.composition == mode {
            return false;
        }
        self.composition = mode;
        self.options.composition = mode;
        true
    }

    pub const fn composition_mode(&self) -> WorldExplorerCompositionMode {
        self.composition
    }

    pub const fn exact_stats(&self) -> ExplorerExactStats {
        self.last_exact_stats
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
        let exact_view = (self.composition != WorldExplorerCompositionMode::Horizon)
            .then(|| {
                self.session
                    .exact_composition_view(self.options.exact_radius)
            })
            .transpose()
            .map_err(anyhow::Error::msg)?;
        if let Some(exact_view) = exact_view {
            self.session.apply_exact_composition_view(exact_view);
        } else {
            self.session.restore_horizon_view();
        }
        if self.composition != WorldExplorerCompositionMode::Horizon {
            let exact_view = *exact_view
                .as_ref()
                .expect("non-horizon composition has an exact render view");
            self.last_exact_anchor = exact_view.residency_anchor;
            self.exact.update_and_pump(
                device,
                exact_view.residency_anchor[0],
                exact_view.residency_anchor[1],
            )?;
        }
        let coverage = self.exact.coverage_snapshot().map_err(anyhow::Error::msg)?;
        let coverage_mode = match self.composition {
            WorldExplorerCompositionMode::Composed => {
                Some((&coverage, TerrainExactCoverageMode::DiscardPainted))
            }
            WorldExplorerCompositionMode::Coverage => {
                Some((&coverage, TerrainExactCoverageMode::VisualizePainted))
            }
            WorldExplorerCompositionMode::Horizon | WorldExplorerCompositionMode::Exact => None,
        };
        let tree_ownership = matches!(
            self.composition,
            WorldExplorerCompositionMode::Composed | WorldExplorerCompositionMode::Coverage
        )
        .then_some(self.exact.tree_ownership());
        let depth_view = &self.exact.depth().view;
        let clear_color = self.exact.clear_color();
        let mut stats = self
            .session
            .encode_to_target(
                device,
                queue,
                encoder,
                TerrainHorizonRenderTarget {
                    color_view,
                    depth_view,
                    color_load: wgpu::LoadOp::Clear(clear_color),
                    color_store: wgpu::StoreOp::Store,
                    depth_load: wgpu::LoadOp::Clear(0.0),
                    depth_store: wgpu::StoreOp::Store,
                },
                self.started.elapsed(),
                coverage_mode,
                tree_ownership,
            )
            .map_err(anyhow::Error::msg)?;
        if matches!(
            self.composition,
            WorldExplorerCompositionMode::Exact | WorldExplorerCompositionMode::Composed
        ) {
            let render_view = exact_view
                .expect("drawn exact composition has an exact render view")
                .render_view;
            self.exact.render(
                queue,
                encoder,
                color_view,
                render_view,
                [self.options.width, self.options.height],
                self.composition == WorldExplorerCompositionMode::Composed,
            )?;
        }
        self.last_exact_stats = self.exact.stats();
        if self.composition != WorldExplorerCompositionMode::Horizon {
            stats.target_ready &= self.last_exact_stats.complete;
            stats.needs_redraw |= !self.last_exact_stats.complete;
        }
        Ok(stats)
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
        let width = self.exact.depth().width;
        let height = self.exact.depth().height;
        let unpadded_row_bytes = width
            .checked_mul(std::mem::size_of::<f32>() as u32)
            .context("World Explorer exact depth row byte length overflow")?;
        if bytes_per_row < unpadded_row_bytes
            || !bytes_per_row.is_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        {
            anyhow::bail!(
                "World Explorer depth copy row length must be at least {unpadded_row_bytes} bytes \
                 and {}-byte aligned, got {bytes_per_row}",
                wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
            );
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: self.exact.depth().texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::DepthOnly,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: destination,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        Ok(())
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
        let exact = self.last_exact_stats;
        format!(
            "{} composition={} exact={}/{} exact_queue={} exact_pending={} exact_inflight={} \
             exact_generation={} exact_admitted={} exact_stale={} exact_sections={} \
             exact_vertices={} exact_indices={}/{} exact_bytes={} exact_trees={}:{}:{} \
             exact_tree_draw={}:{} exact_compile_ms={:.2} \
             exact_present_ms={:.2} exact_mesh_ms={:.2} exact_pack_ms={:.2} \
             exact_anchor=viewer-forward({}, {}) frontier=procedural-collar-1.5-blocks",
            self.session.diagnostics(),
            self.composition.label(),
            exact.painted_chunks,
            exact.desired_chunks,
            exact.queued_chunks,
            exact.pending_admissions,
            exact.in_flight,
            exact.coverage_generation,
            exact.admitted_chunks_total,
            exact.stale_chunks_total,
            exact.drawn_sections,
            exact.vertex_count,
            exact.drawn_indices,
            exact.index_count,
            exact.resident_mesh_bytes,
            exact.natural_tree_records,
            exact.exact_owned_tree_records,
            exact.proxy_owned_tree_records,
            exact.exact_tree_sections,
            exact.exact_tree_indices,
            exact.generation_ms,
            exact.presentation_ms,
            exact.mesh_ms,
            exact.pack_ms,
            self.last_exact_anchor[0],
            self.last_exact_anchor[1],
        )
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
