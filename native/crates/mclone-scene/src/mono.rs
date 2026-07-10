//! Mono (flat, single-view) render topology for the scene host (tactical 168
//! Slice 3).
//!
//! The scene host was born stereo: every public entry took `[XrView; 2]` and
//! rendered two eyes. Slice 3 adds the one-view case — `Mono` — so a flat
//! desktop/offscreen driver can drive the *same* host (session runtime, camera,
//! interest, budgeted section sync/upload, frame-input assembly, effects) and
//! only differ in the view topology it submits. Flat rendering is now the
//! one-view case of the shared host, not a separate orchestrator.
//!
//! The mono path renders through the shared
//! [`render_full_frame_for_view_with_far_lod`] entry (the same one the desktop
//! flat `FlatRenderResources` uses), so terrain, actors, sky, screen effects,
//! and the far-terrain LOD shell all match the flat pipeline. UI is drawn as a
//! conventional screen-space HUD — the counterpart to the stereo world-quad
//! panel — which is a *presentation strategy* on the host, not a second UI
//! stack (see [`MonoUiPresentation`]).
//!
//! Like the shared `render_full_frame_for_view*` entries (and unlike the
//! self-submitting stereo eye path), the mono entries encode into a
//! caller-owned [`RenderFrameContext`] and leave submission to the driver. That
//! keeps them composable with the offscreen capture loop and any surface driver
//! that owns its swapchain frame.

use super::*;

/// How the mono view topology presents UI.
///
/// The stereo path draws UI as a world-space quad (`WorldGuiRenderer`). The mono
/// path draws the same [`GameUiHost`] draw list in screen space through the
/// shared `render_full_frame_for_view*` GUI slot. Both are presentation
/// strategies on one host (tactical 168 Slice 3).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MonoUiPresentation {
    /// No UI overlay — world capture only (e.g. offscreen dual-view review).
    None,
    /// Conventional 2D HUD/menu drawn in screen space at the target resolution.
    ScreenSpaceHud,
}

impl<S> XrMcloneTerrainState<S>
where
    S: RemoteDedicatedServerSession,
{
    /// Render one flat (mono) view — the one-view case of the host's
    /// views-as-data topology. Advances the local startup pump and streams
    /// sections live, then renders `render_view` through the shared
    /// [`render_full_frame_for_view_with_far_lod`] entry with the chosen UI
    /// presentation strategy. Encodes into the caller-owned `frame`; the caller
    /// submits. Returns the shared [`FullFrameRenderSummary`] so flat drivers
    /// get the same section/actor/GUI accounting the windowed desktop path
    /// reports.
    pub fn render_mono_frame(
        &mut self,
        frame: RenderFrameContext<'_>,
        depth: &ChunkDepthTarget,
        render_view: ChunkRenderView,
        ui: MonoUiPresentation,
    ) -> Result<FullFrameRenderSummary> {
        self.render_mono_frame_inner(
            frame,
            depth,
            render_view,
            ui,
            XrTerrainRuntimeUpdateMode::Live,
        )
    }

    /// Mono render that does not stream new sections this frame (the runtime is
    /// held frozen). Matches the frozen stereo variants; used by offscreen
    /// capture paths that stream the world in once and then render fixed views.
    pub fn render_mono_frame_frozen_runtime(
        &mut self,
        frame: RenderFrameContext<'_>,
        depth: &ChunkDepthTarget,
        render_view: ChunkRenderView,
        ui: MonoUiPresentation,
    ) -> Result<FullFrameRenderSummary> {
        self.render_mono_frame_inner(
            frame,
            depth,
            render_view,
            ui,
            XrTerrainRuntimeUpdateMode::Frozen,
        )
    }

    /// Whether the local-world startup pump has finished promoting into a live
    /// runtime. Non-blocking; drivers own the drive-to-ready loop (Web posture
    /// rule: blocking convenience loops live in native drivers, not the host).
    pub fn local_startup_complete(&self) -> bool {
        self.local_startup.is_none()
    }

    /// In-flight render work relevant to the requested camera: target render
    /// chunks/inflight sections plus queued client uploads. Tracking-halo dirt
    /// outside the drawable render distance intentionally does not keep an
    /// offscreen capture alive forever; target readiness is the same distinction
    /// used by the startup-streaming probes. Returns [`usize::MAX`] while local
    /// startup is still promoting.
    pub fn pending_stream_work(&self, camera_position: Vec3) -> usize {
        if self.local_startup.is_some() {
            return usize::MAX;
        }
        let Some(runtime) = self.runtime.as_ref() else {
            return usize::MAX;
        };
        let target = runtime.target_render_work_stats(camera_position);
        let upload = self.section_uploads.stats();
        target.pending_render_chunks
            + target.inflight_render_sections
            + usize::from(target.ready_render_work_pending)
            + upload.queued_upload_sections
            + upload.queued_lifecycle_items
    }

    fn render_mono_frame_inner(
        &mut self,
        frame: RenderFrameContext<'_>,
        depth: &ChunkDepthTarget,
        render_view: ChunkRenderView,
        ui: MonoUiPresentation,
        runtime_mode: XrTerrainRuntimeUpdateMode,
    ) -> Result<FullFrameRenderSummary> {
        let device = frame.device;
        let queue = frame.queue;
        if matches!(runtime_mode, XrTerrainRuntimeUpdateMode::Live) {
            self.advance_local_startup(device, queue)?;
        }

        let center_position = render_view.camera_position;
        let frame_deadline = self.render_compile_frame_deadline();
        let mut timing = XrTerrainFrameTiming::default();
        // Poll/sync/upload/compile-release under the shared budgeted admission
        // path (or the frozen no-op summary), exactly as the stereo frame does.
        // These use their own command buffers, independent of `frame`.
        let _upload = self.live_upload_for_frame(
            device,
            center_position,
            runtime_mode,
            frame_deadline,
            &mut timing,
        )?;

        // Frame-input assembly reuses the shared accessors. The shared
        // `render_full_frame_for_view*` entry applies sky-darken and underwater
        // fog internally, so pass the base effective options plus the overlay.
        let render_options = self.effective_render_options(center_position);
        let sky_clear_color = self.sky_clear_color();
        let time_of_day = self.time_of_day();
        let sun_angle = self.sun_angle();
        let underwater_overlay = self.mono_underwater_overlay(render_view);
        let actor_instances = self.current_actor_instances();

        let gui_scale = GuiScale::from_pixels(frame.target.size[0], frame.target.size[1]);
        let (full_frame_gui, gui_draw) = self.mono_gui_frame(gui_scale, ui);
        if matches!(ui, MonoUiPresentation::ScreenSpaceHud) {
            self.ensure_mono_gui(device, queue)?;
        }

        let far_lod_config = self.scene.far_lod;
        let far_lod_seed = self.scene.seed;
        let far_lod_center = self.camera.snapshot().chunk_pos;

        let mut render_stats = self.render_stats;

        // `runtime`, `far_lod`, `mono_gui`, `sky`, `draw`, `actors`, and
        // `screen_effects` are disjoint fields, so these borrows coexist.
        let far_lod_mesh = self.runtime.as_mut().and_then(|runtime| {
            runtime.prepare_far_lod_mesh(
                far_lod_config,
                far_lod_seed,
                far_lod_center,
                render_view.camera_position,
            )
        });
        let far_lod = far_lod_mesh.map(|_| &mut self.far_lod);
        let gui_renderer = self.mono_gui.as_mut();

        let summary = render_full_frame_for_view_with_far_lod(
            frame,
            depth,
            &self.sky,
            &mut self.draw,
            far_lod,
            far_lod_mesh,
            Some(&mut self.actors),
            Some(&mut self.screen_effects),
            gui_renderer,
            render_view,
            &actor_instances,
            underwater_overlay,
            sky_clear_color,
            time_of_day,
            sun_angle,
            render_options,
            full_frame_gui,
            |_| gui_draw,
            &mut render_stats,
        )
        .context("render mono scene frame")?;

        self.render_stats = render_stats;
        self.rendered_frames = self.rendered_frames.wrapping_add(1);
        Ok(summary)
    }

    /// Build the screen-space GUI draw list + `FullFrameGui` flags for a mono
    /// frame. Reuses the same [`GameUiHost`] draw list the stereo world-quad
    /// path renders, just laid out at the flat target resolution.
    fn mono_gui_frame(
        &mut self,
        gui_scale: GuiScale,
        ui: MonoUiPresentation,
    ) -> (FullFrameGui, GuiDrawList) {
        match ui {
            MonoUiPresentation::None => (
                FullFrameGui::new(false, false, [gui_scale.width, gui_scale.height]),
                GuiDrawList::new(),
            ),
            MonoUiPresentation::ScreenSpaceHud => {
                let ui_state = self.current_ui_render_state();
                let session_projection = self.session_projection();
                let panel = prepare_xr_menu_panel_draw(
                    &mut self.ui,
                    &mut self.menu_overlay_cache,
                    gui_scale,
                    ui_state,
                    session_projection.loading_progress_overlay.as_ref(),
                    &session_projection.status_overlay,
                );
                let mut draw = panel.panel_draw;
                draw.append(&panel.overlay_draw);
                // A full-screen menu covers the world; a bare HUD does not.
                let covers_world = self.ui.is_active();
                if let Some(hud) = self.mono_flat_hud(covers_world) {
                    let hud_draw = self.ui.render_flat_hud_draw_list(gui_scale, &hud);
                    draw.append(&hud_draw.draw);
                }
                (
                    FullFrameGui::new(true, covers_world, [gui_scale.width, gui_scale.height]),
                    draw,
                )
            }
        }
    }

    /// Capture-grade flat HUD assembled by the shared host. Slice 7 expands the
    /// input/capability and diagnostic fields for the interactive desktop lane;
    /// this baseline deliberately renders the shared crosshair + selected
    /// hotbar/icons so the mono GUI renderer has a real pixel canary now.
    fn mono_flat_hud(&self, menu_active: bool) -> Option<FlatHud> {
        let runtime = self.runtime.as_ref()?;
        let mut hud = FlatHud::new(ResolvedFlatInput {
            preferred_prompt: Some(InputPromptKind::KeyboardMouse),
            touch_controls_visible: false,
            accepts_keyboard_mouse: true,
            accepts_touch: false,
            accepts_gamepad: false,
            accepts_xr_controller: false,
        });
        hud.world_hud_visible = !menu_active;
        hud.crosshair_visible = !menu_active;
        hud.hotbar = FlatHotbarOverlay::selected_with_icons(
            self.interaction.selected_hotbar_slot(),
            debug_hotbar_icons(
                self.interaction.hotbar_items(),
                &runtime.mesh_assets().catalog,
            ),
        );
        Some(hud)
    }

    fn ensure_mono_gui(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<()> {
        if self.mono_gui.is_none() {
            let mut gui = GuiRenderer::new(device, self.color_format);
            gui.upload_texture_atlas(device, queue, self.mesh_assets.atlas.as_upload())
                .context("initialize mono screen-space HUD GUI atlas")?;
            self.mono_gui = Some(gui);
        }
        Ok(())
    }
}
