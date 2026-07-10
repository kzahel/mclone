//! Offscreen driver for the shared scene host's flat (mono) view topology
//! (tactical 168 Slice 3).
//!
//! This is the flat/offscreen counterpart to the OpenXR frame drivers: it owns
//! a headless GPU target and drives an [`XrMcloneTerrainState`] through its
//! `render_mono_frame*` entries. The per-frame orchestration (session runtime,
//! camera, budgeted section sync/upload, sky/time/sun/far-LOD frame-input
//! assembly) lives entirely on the host; this driver only owns the depth
//! target, the drive-to-ready loop, and the flat cameras it wants captured.
//!
//! Before this existed, the headless dual-view path hand-assembled its own
//! sky/time/sun frame inputs and called the shared `render_full_frame_for_view`
//! directly — the "third copy" the tactical deletes. It now consumes the host.

use anyhow::{Context, Result, bail};
use mclone_app_runtime::frame_render::FullFrameRenderSummary;
use mclone_app_runtime::render_assets::{
    load_actor_texture_assets_from_asset_source, load_textured_mesh_assets_from_source,
};
use mclone_render::chunk::{ChunkCamera, ChunkDepthTarget, TexturedSectionRenderOptions};
use mclone_render::headless::HEADLESS_FORMAT;
use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
use mclone_scene::{MonoUiPresentation, XrMcloneTerrainState, XrSceneOptions};

use crate::render_cache::load_asset_source;

/// Upper bound on drive-to-ready warmup frames. Local worldgen for a small
/// review scene streams in well under this; the cap only guards against a world
/// that never produces sections.
const MAX_WARMUP_FRAMES: usize = 4096;
/// Consecutive warmup frames with a non-growing loaded-section count before the
/// world is considered streamed. Approximates the old cold full-sync without
/// blocking the host's step-based streaming model.
const STREAM_STABLE_FRAMES: usize = 6;

/// Offscreen driver over the scene host's mono topology.
pub(crate) struct MonoOffscreenSceneHost {
    host: XrMcloneTerrainState,
    depth: ChunkDepthTarget,
    size: [u32; 2],
    warmup_camera: ChunkCamera,
    captured: Vec<FullFrameRenderSummary>,
}

impl MonoOffscreenSceneHost {
    /// Build the host for a local flat scene and its headless depth target. The
    /// caller supplies the wgpu device/queue (typically the headless capture
    /// loop's) and the target size; `warmup_camera` centers section streaming
    /// during drive-to-ready.
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: [u32; 2],
        scene: XrSceneOptions,
        render_options: TexturedSectionRenderOptions,
        warmup_camera: ChunkCamera,
    ) -> Result<Self> {
        let asset_source =
            load_asset_source().context("load asset source for offscreen scene host")?;
        let actor_assets = load_actor_texture_assets_from_asset_source(&asset_source)
            .context("load actor texture assets for offscreen scene host")?;
        let mesh_assets = load_textured_mesh_assets_from_source(&asset_source)
            .context("load mesh assets for offscreen scene host")?;
        let host = XrMcloneTerrainState::new(
            device,
            queue,
            HEADLESS_FORMAT,
            scene,
            render_options,
            mesh_assets,
            actor_assets.atlas,
            actor_assets.figures,
            &asset_source,
            None,
        )
        .context("initialize offscreen scene host")?;
        let depth = ChunkDepthTarget::new(device, size[0], size[1]);
        Ok(Self {
            host,
            depth,
            size,
            warmup_camera,
            captured: Vec::new(),
        })
    }

    /// Per-camera render summaries, in the order [`Self::render_camera_frozen`]
    /// was called.
    pub(crate) fn captured(&self) -> &[FullFrameRenderSummary] {
        &self.captured
    }

    /// Stream the local world in against a scratch target until the loaded
    /// section set stabilizes. Blocking drive-to-ready loops live in the driver
    /// (not the host) per the tactical's step-based-API rule.
    pub(crate) fn drive_until_streamed(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        let scratch = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_mono_offscreen_warmup"),
            size: wgpu::Extent3d {
                width: self.size[0],
                height: self.size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HEADLESS_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let scratch_view = scratch.create_view(&wgpu::TextureViewDescriptor::default());
        let warmup_view = self.warmup_camera.render_view(self.size[0], self.size[1]);

        let mut stable = 0usize;
        let mut drew_any = false;
        let mut last_pending_stream_work = usize::MAX;
        let mut last_drawn_section_count = 0usize;
        for _ in 0..MAX_WARMUP_FRAMES {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_mono_offscreen_warmup_encoder"),
            });
            let summary = {
                let frame = RenderFrameContext::new(
                    device,
                    queue,
                    &mut encoder,
                    RenderFrameTarget::color(&scratch_view, self.size),
                );
                self.host
                    .render_mono_frame(frame, &self.depth, warmup_view, MonoUiPresentation::None)
                    .context("warmup mono frame")?
            };
            queue.submit(std::iter::once(encoder.finish()));
            device
                .poll(wgpu::PollType::Wait)
                .context("poll device during offscreen warmup")?;

            drew_any |= summary.drawn_section_count > 0;
            last_drawn_section_count = summary.drawn_section_count;
            last_pending_stream_work = self.host.pending_stream_work(warmup_view.camera_position);
            // Fully streamed = no pending server generation / compile / upload
            // work (implies startup completed) and geometry has actually drawn.
            // A short stable window absorbs single-frame lulls between async
            // worldgen bursts.
            if last_pending_stream_work == 0 && summary.drawn_section_count > 0 {
                stable += 1;
                if stable >= STREAM_STABLE_FRAMES {
                    return Ok(());
                }
            } else {
                stable = 0;
            }
        }

        if !drew_any {
            bail!(
                "offscreen scene host streamed no render sections after {MAX_WARMUP_FRAMES} warmup frames"
            );
        }
        bail!(
            "offscreen scene host did not settle after {MAX_WARMUP_FRAMES} warmup frames: pending_stream_work={last_pending_stream_work} last_drawn_sections={last_drawn_section_count}"
        )
    }

    /// Render one flat camera into the caller-owned frame, with the runtime
    /// held frozen (the world was streamed in during [`Self::drive_until_streamed`]).
    pub(crate) fn render_camera_frozen(
        &mut self,
        frame: RenderFrameContext<'_>,
        camera: ChunkCamera,
        ui: MonoUiPresentation,
    ) -> Result<FullFrameRenderSummary> {
        let render_view = camera.render_view(self.size[0], self.size[1]);
        let summary = self
            .host
            .render_mono_frame_frozen_runtime(frame, &self.depth, render_view, ui)
            .context("render frozen mono camera")?;
        self.captured.push(summary);
        Ok(summary)
    }
}
