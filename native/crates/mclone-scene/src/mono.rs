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

const MONO_BLINK_MAX_DISTANCE: f64 = 8.0;
const MONO_BLINK_ARC_HEIGHT: f64 = 1.25;
const MONO_BLINK_ORIGIN_LEFT_OFFSET: f64 = 0.35;
const MONO_BLINK_ORIGIN_DOWN_OFFSET: f64 = 0.25;
const MONO_BLINK_ORIGIN_FORWARD_OFFSET: f64 = 0.2;
const MONO_BLINK_UPWARD_PITCH_BIAS_RADIANS: f64 = 0.18;
const MONO_BLINK_VALID_ARC_COLOR: [f32; 4] = [0.1, 0.85, 1.0, 1.0];
const MONO_BLINK_INVALID_ARC_COLOR: [f32; 4] = [1.0, 0.25, 0.15, 1.0];
const MONO_BLINK_FEET_COLOR: [f32; 4] = [0.1, 1.0, 0.35, 1.0];
const MONO_BLINK_DOT_COLOR: [f32; 4] = [1.0, 0.95, 0.2, 1.0];
const MONO_BLINK_MARKER_RADIUS: f64 = 0.28;
const MONO_BLINK_DOT_RADIUS: f64 = 0.1;
const MONO_GROUND_PROBE_DISTANCE: f64 = 0.01;

/// Platform facts used to present the shared scene as a conventional flat
/// client. Gameplay, session, HUD, and diagnostics state remain owned by the
/// scene host; the platform supplies only its active input capabilities,
/// cadence/present facts, and render scale.
#[derive(Clone, Debug, PartialEq)]
pub struct MonoUiContext {
    pub resolved_input: ResolvedFlatInput,
    pub frame_pacing: FramePacingUiState,
    pub pacing_debug: FramePacingDebugStats,
    pub frame_timing: FrameTimingStats,
    pub render_scale: f32,
    pub hud_visible: bool,
    pub touch_overlay: TouchOverlay,
    pub touch_controls_mode: Option<TouchControlsMode>,
    pub touch_settings: Option<GameTouchSettings>,
}

impl Default for MonoUiContext {
    fn default() -> Self {
        Self {
            resolved_input: ResolvedFlatInput {
                preferred_prompt: Some(InputPromptKind::KeyboardMouse),
                touch_controls_visible: false,
                accepts_keyboard_mouse: true,
                accepts_touch: false,
                accepts_gamepad: false,
                accepts_xr_controller: false,
            },
            frame_pacing: FramePacingUiState::default(),
            pacing_debug: FramePacingDebugStats::default(),
            frame_timing: FrameTimingStats::default(),
            render_scale: 1.0,
            hud_visible: true,
            touch_overlay: TouchOverlay::hidden(),
            touch_controls_mode: None,
            touch_settings: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MonoSceneFrameSummary {
    pub render: FullFrameRenderSummary,
    pub timing: XrTerrainFrameTiming,
    pub upload: XrTerrainUploadSummary,
}

/// One independently posed flat presentation view. The target and depth
/// attachment remain platform-owned; camera, culling, effects, and UI policy
/// are view-local scene inputs.
#[derive(Clone, Copy)]
pub struct FlatPresentationView<'a> {
    pub target: RenderFrameTarget<'a>,
    pub depth: &'a ChunkDepthTarget,
    pub render_view: ChunkRenderView,
    pub ui: MonoUiPresentation,
}

impl<'a> FlatPresentationView<'a> {
    pub const fn new(
        target: RenderFrameTarget<'a>,
        depth: &'a ChunkDepthTarget,
        render_view: ChunkRenderView,
        ui: MonoUiPresentation,
    ) -> Self {
        Self {
            target,
            depth,
            render_view,
            ui,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FlatPresentationViewSummary {
    pub view: PresentationViewIndex,
    pub render: FullFrameRenderSummary,
}

/// Receipt for one flat frame that may contain one through four presentation
/// views. Runtime/update/upload preparation is reported once for the frame;
/// render summaries remain separate per view.
#[derive(Clone, Debug)]
pub struct FlatPresentationFrameSummary {
    pub shared_preparation_count: u32,
    pub rendered_view_count: u32,
    pub views: Vec<FlatPresentationViewSummary>,
    pub timing: XrTerrainFrameTiming,
    pub upload: XrTerrainUploadSummary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FlatPresentationAdmission {
    shared_preparation_count: u32,
    rendered_view_count: u32,
}

fn admit_flat_presentation_views(view_count: usize) -> Result<FlatPresentationAdmission> {
    if view_count == 0 {
        bail!("flat presentation frame requires at least one view");
    }
    if view_count > MAX_PRESENTATION_VIEW_COUNT as usize {
        bail!(
            "flat presentation frame requested {view_count} views; maximum is {MAX_PRESENTATION_VIEW_COUNT}"
        );
    }
    Ok(FlatPresentationAdmission {
        shared_preparation_count: 1,
        rendered_view_count: view_count as u32,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub enum MonoWorldActionStatus {
    NoRuntime,
    NoTarget,
    NoCommand,
    DeniedByWorldBehavior,
    EmbeddedWorldActivationRequested,
    Submitted { target: BlockInteractionTarget },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct MonoBlinkDebugState {
    active: bool,
    preview: Option<TeleportPreview>,
    first_request_id: Option<TeleportPreviewRequestId>,
    latest_request_id: Option<TeleportPreviewRequestId>,
    preview_request_id: Option<TeleportPreviewRequestId>,
    last_submitted_intent: Option<TeleportIntent>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MonoBlinkCommitStatus {
    Inactive,
    NoRuntime,
    NoValidPreview { validity: TeleportValidityReason },
    Committed { target_feet: Vec3d, changed: bool },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MonoUiActionOutcome {
    pub scene_replaced: bool,
    pub session_start_requested: bool,
    pub clear_gameplay_input: bool,
}

/// Shared result of applying one platform-neutral mono input/cadence frame.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MonoInputFrameOutcome {
    pub camera_changed: bool,
    pub pose_sync_changed: bool,
    pub activation_changed: bool,
}

impl MonoInputFrameOutcome {
    pub const fn changed(self) -> bool {
        self.camera_changed || self.pose_sync_changed || self.activation_changed
    }
}

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

impl McloneSceneHost {
    /// Select the conventional flat-client capability/UI profile. This is a
    /// topology choice on the shared host, not a platform-owned gameplay path.
    pub fn configure_mono_ui(&mut self, ui: GameUiHost, context: MonoUiContext) {
        self.configure_mono_ui_with_profile(
            ui,
            context,
            desktop_native_client_experience_profile(),
        );
    }

    /// Configure flat UI using a platform capability profile while retaining
    /// the shared Mono gameplay/session/render implementation.
    pub fn configure_mono_ui_with_profile(
        &mut self,
        ui: GameUiHost,
        context: MonoUiContext,
        profile: ClientExperienceProfile,
    ) {
        self.ui = ui;
        self.mono_ui_context = Some(context);
        self.client_experience = ClientExperienceController::new(profile);
        self.refresh_world_catalog_ui(WorldCatalogUiStatus::hidden());
    }

    pub fn set_mono_ui_context(&mut self, context: MonoUiContext) {
        self.mono_ui_context = Some(context);
    }

    pub fn enter_mono_title(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<()> {
        self.teardown_world(device, queue)?;
        self.session.clear();
        self.status_overlay = StatusOverlay::hidden();
        self.ui = GameUiHost::new();
        self.refresh_world_catalog_ui(WorldCatalogUiStatus::hidden());
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn rebuild_mono_render_resources(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        actor_atlas: ActorTextureImage,
        actor_figures: &ActorFigureSet,
        asset_source: &impl AssetSource,
    ) -> Result<()> {
        let screen_effects = load_screen_effect_texture_assets(asset_source)
            .context("reload Mono screen-effect assets")?;
        self.rebuild_mono_render_resources_with_assets(
            device,
            queue,
            actor_atlas,
            actor_figures,
            &screen_effects,
        )
    }

    /// Rebuild against already-prepared host-neutral assets. Browser adapters
    /// use this path after a WebGPU resource-generation reset; native adapters
    /// retain the source-loading convenience wrapper above.
    pub fn rebuild_mono_render_resources_with_assets(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        actor_atlas: ActorTextureImage,
        actor_figures: &ActorFigureSet,
        screen_effects: &ScreenEffectTextureAssets,
    ) -> Result<()> {
        // A retained slot owns resources created by the previous device. Until
        // multi-slot device migration is implemented, cancellation is the
        // explicit safe policy: drop its runtime/compiler/GPU ownership before
        // constructing any replacement resources for the active slot.
        self.cancel_warm_world_standby("render resource rebuild");
        let frame_metrics_visible = self.diagnostic_panel.frame_metrics_visible();
        let debug_diagnostics_visible = self.diagnostic_panel.debug_diagnostics_visible();
        self.active_world.draw = TexturedSectionDrawResources::new(
            device,
            queue,
            self.color_format,
            &[],
            self.mesh_assets.atlas.as_upload(),
        )
        .context("rebuild Mono terrain draw resources")?;
        self.active_world.actors = Some(
            ActorDrawResources::new(
                device,
                queue,
                self.color_format,
                actor_atlas.as_upload(),
                Some(actor_figures),
            )
            .context("rebuild Mono actor draw resources")?,
        );
        self.active_world.far_lod = FarTerrainLodRenderer::new(device, self.color_format);
        self.selection_outline = SelectionOutlineRenderer::new(device, self.color_format);
        self.world_gui_renderer = WorldGuiRenderer::new(device, self.color_format);
        self.world_gui_overlay_renderer = WorldGuiRenderer::new(device, self.color_format);
        #[cfg(not(target_arch = "wasm32"))]
        if self.opaque_world_gate_renderer.is_some() {
            let renderer = OpaqueWorldGateRenderer::new(device, self.color_format);
            if device.features().contains(wgpu::Features::MULTIVIEW) {
                renderer
                    .materialize_multiview_renderer(device)
                    .context("rebuild opaque world gate multiview pipeline")?;
            }
            self.opaque_world_gate_renderer = Some(renderer);
        }
        self.mono_gui = None;
        self.diagnostic_panel = XrDiagnosticPanel::new(device, self.color_format);
        self.diagnostic_panel
            .set_frame_metrics_visible(frame_metrics_visible);
        self.diagnostic_panel
            .set_debug_diagnostics_visible(debug_diagnostics_visible);
        self.sky = SkyRenderer::new_with_color_profile(
            device,
            self.color_format,
            self.render_options.color_profile,
        );
        self.screen_effects = ScreenEffectsRenderer::new_with_assets(
            device,
            queue,
            self.color_format,
            screen_effects,
        )
        .context("rebuild Mono screen effects")?;
        self.active_world.traversal_ready_sections.clear();
        self.active_world.section_uploads.clear();
        self.active_world.render_stats = RenderStreamStats::default();
        if let Some(runtime) = &mut self.active_world.runtime {
            runtime.mark_all_render_sections_dirty_for_resource_rebuild();
        }
        Ok(())
    }

    pub fn has_runtime(&self) -> bool {
        self.active_world.runtime.is_some()
    }

    pub fn session_state(&self) -> &GameSessionState {
        self.session.state()
    }

    pub fn scene_options(&self) -> &McloneSceneHostOptions {
        &self.active_world.scene
    }

    pub fn render_stats(&self) -> RenderStreamStats {
        self.active_world.render_stats
    }

    pub fn runtime_stats(&self) -> Option<SingleViewRuntimeStats> {
        self.active_world
            .runtime
            .as_ref()
            .map(|runtime| runtime.stats())
    }

    pub fn far_lod_stats(&self) -> FarTerrainLodProducerStats {
        self.active_world
            .runtime
            .as_ref()
            .map(|runtime| runtime.far_lod_stats())
            .unwrap_or_default()
    }

    /// Pull the exact far-LOD lifecycle and real-terrain paint sets for a mono
    /// view. The renderer reuses its cached culling records and only materializes
    /// the section-key set for this explicit diagnostic call.
    pub fn mono_far_lod_settle_snapshot(
        &self,
        render_view: ChunkRenderView,
    ) -> Option<FarLodSettleSnapshot> {
        let runtime = self.active_world.runtime.as_ref()?;
        let render_options = self.effective_render_options(render_view.camera_position);
        let view_sets = self
            .active_world
            .draw
            .section_view_set_snapshot(render_view, render_options);
        Some(FarLodSettleSnapshot::new(
            runtime.far_lod_settle_snapshot(render_view.camera_position),
            view_sets.paintable_frustum_keys,
            view_sets.drawn_keys,
        ))
    }

    pub fn lod_coverage_counters(
        &self,
    ) -> mclone_app_runtime::lod_coverage::LodReplacementCounters {
        self.active_world
            .runtime
            .as_ref()
            .map(|runtime| runtime.lod_coverage_counters())
            .unwrap_or_default()
    }

    pub fn runtime_poll_diagnostics(&self) -> Option<RuntimePollDiagnostics> {
        self.active_world
            .runtime
            .as_ref()
            .map(|runtime| runtime.last_poll_diagnostics())
    }

    pub fn mono_client(&self) -> Option<&mclone_client::ClientRuntime> {
        self.active_world
            .runtime
            .as_ref()
            .map(|runtime| runtime.client())
    }

    pub fn mono_highest_non_air_block_y_at_world(&self, world_x: i32, world_z: i32) -> Option<i32> {
        self.active_world
            .runtime
            .as_ref()?
            .highest_non_air_block_y_at_world(world_x, world_z)
    }

    pub fn mono_view_readiness_overlay(&self) -> Option<LoadingProgressOverlay> {
        self.active_world
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.view_readiness_overlay())
    }

    pub fn mono_target_render_work_stats(
        &self,
        camera_position: Vec3,
    ) -> mclone_app_runtime::TargetRenderWorkStats {
        self.active_world.runtime.as_ref().map_or_else(
            mclone_app_runtime::TargetRenderWorkStats::default,
            |runtime| runtime.target_render_work_stats(camera_position),
        )
    }

    pub fn mono_render_vertex_count(&self) -> u32 {
        self.active_world.draw.vertex_count()
    }

    pub fn force_mono_day_time(&mut self, day_time: u64) {
        if let Some(runtime) = &mut self.active_world.runtime {
            runtime.force_day_time(day_time);
        }
    }

    pub fn set_mono_capture_camera(
        &mut self,
        eye: Vec3d,
        yaw_radians: f64,
        pitch_radians: f64,
        speed_blocks_per_second: f64,
    ) {
        self.replace_mono_camera(
            eye,
            yaw_radians,
            pitch_radians,
            speed_blocks_per_second,
            EngineCameraCollisionMode::NoClip,
        );
    }

    pub fn set_mono_player_camera(
        &mut self,
        eye: Vec3d,
        yaw_radians: f64,
        pitch_radians: f64,
        speed_blocks_per_second: f64,
    ) {
        self.replace_mono_camera(
            eye,
            yaw_radians,
            pitch_radians,
            speed_blocks_per_second,
            EngineCameraCollisionMode::Normal,
        );
    }

    fn replace_mono_camera(
        &mut self,
        eye: Vec3d,
        yaw_radians: f64,
        pitch_radians: f64,
        speed_blocks_per_second: f64,
        collision_mode: EngineCameraCollisionMode,
    ) {
        let camera = SceneCameraConfig::from_scene(&self.active_world.scene).from_eye_pose(
            eye,
            yaw_radians,
            pitch_radians,
            speed_blocks_per_second,
            collision_mode,
        );
        if let Some(startup) = &mut self.active_world.local_startup {
            startup.replace_camera(camera.clone());
        }
        self.active_world.camera = camera;
    }

    pub fn set_mono_camera_view(&mut self, view_mode: EngineCameraViewMode) {
        self.active_world.camera.set_view_mode(view_mode);
    }

    pub fn set_mono_ui_screen(&mut self, screen: Option<GameScreen>) {
        self.ui.set_screen(screen);
    }

    /// Inject an already-authoritative life update for deterministic visual
    /// diagnostics. Interactive clients receive this only through their
    /// ordered server stream.
    pub fn apply_mono_player_life_for_diagnostics(
        &mut self,
        life: mclone_protocol::PlayerLifeState,
    ) -> bool {
        let Some(runtime) = self.active_world.runtime.as_mut() else {
            return false;
        };
        let applied = runtime
            .core_mut()
            .apply_server_updates(vec![mclone_protocol::ServerUpdate::PlayerLife(life)]);
        if applied {
            self.sync_player_lifecycle_ui();
        }
        applied
    }

    pub fn set_mono_new_world_seed(&mut self, seed: i64) {
        self.ui.set_new_world_seed(seed);
    }

    pub fn set_mono_player_collision_box_visible(&mut self, visible: bool) {
        self.player_collision_box_visible = visible;
    }

    pub fn set_mono_frame_pipeline_overlay_visible(&mut self, visible: bool) {
        self.diagnostic_panel.set_frame_metrics_visible(visible);
    }

    pub fn set_mono_debug_diagnostics_visible(&mut self, visible: bool) {
        self.diagnostic_panel.set_debug_diagnostics_visible(visible);
    }

    pub fn mono_debug_diagnostics_visible(&self) -> bool {
        self.diagnostic_panel.debug_diagnostics_visible()
    }

    pub fn set_mono_travel_assist_mode(&mut self, mode: GameTravelAssistMode) {
        self.travel_assist_mode = mode;
        if mode != GameTravelAssistMode::Blink {
            self.clear_mono_blink_debug();
        }
    }

    pub fn mono_blink_preview_ready(&self) -> bool {
        self.mono_blink_debug.preview.is_some()
    }

    pub fn mono_loading_progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        self.session_projection().loading_progress_overlay
    }

    pub fn mono_underwater(&self) -> bool {
        let snapshot = self.active_world.camera.snapshot();
        self.active_world
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.camera_inside_water(glam_vec3_from_vec3d(snapshot.eye)))
    }

    pub fn current_render_distance(&self) -> u32 {
        self.active_world
            .runtime
            .as_ref()
            .map_or(self.active_world.scene.render_distance, |runtime| {
                runtime.render_distance()
            })
    }

    pub fn camera_frame_state(&self) -> EngineCameraFrameState {
        self.active_world
            .camera
            .frame_state(&self.active_world.interaction)
    }

    pub fn mono_render_view(&self, size: [u32; 2]) -> Result<ChunkRenderView> {
        render_pose_from_snapshot_with_view_mode(
            self.active_world.camera.snapshot(),
            self.active_world.camera.view_mode(),
            self.current_render_distance(),
        )
        .render_view(size[0].max(1), size[1].max(1))
    }

    /// Apply flat input and advance local-player publication from one shared
    /// scene boundary. Platform adapters call this every presentation frame,
    /// including idle and menu frames; the scene owns gameplay gating and the
    /// vanilla-rate pose publication deadline.
    pub fn advance_mono_input_frame(
        &mut self,
        frame: FlatInputFrame,
        dt_seconds: f64,
    ) -> Result<MonoInputFrameOutcome> {
        if !self.gameplay_startup_complete() {
            return Ok(MonoInputFrameOutcome::default());
        }
        let activation_was_active = self.embedded_world_activation.phase.active();
        let activation_changed = self.advance_embedded_world_activation(dt_seconds);
        let mut camera_changed = false;
        if !self.mono_ui_is_active() && !activation_was_active {
            camera_changed |= self.apply_mono_movement_frame(frame, dt_seconds);
        }
        let pose_sync_changed = if self.embedded_world_activation.phase.active() {
            false
        } else {
            self.publish_mono_player_pose_if_due()?
        };
        Ok(MonoInputFrameOutcome {
            camera_changed,
            pose_sync_changed,
            activation_changed,
        })
    }

    fn apply_mono_movement_frame(&mut self, frame: FlatInputFrame, dt_seconds: f64) -> bool {
        if !self.gameplay_startup_complete() {
            return false;
        }
        let runtime = self
            .active_world
            .runtime
            .as_ref()
            .expect("startup completion requires runtime");
        let input = engine_camera_input_from_flat_frame(frame, dt_seconds);
        let before = self.active_world.camera.snapshot();
        let after = self
            .active_world
            .camera
            .apply_movement_input(runtime.client(), input);
        self.play_landing_events();
        after != before
    }

    pub fn apply_mono_look_frame(&mut self, frame: FlatInputFrame) -> bool {
        if frame.look_delta.x == 0.0 && frame.look_delta.y == 0.0 {
            return false;
        }
        self.active_world
            .camera
            .turn_mouse_delta(f64::from(frame.look_delta.x), f64::from(frame.look_delta.y));
        true
    }

    pub fn apply_mono_touch_look(&mut self, delta: TouchLookDelta) -> bool {
        if delta.yaw_radians == 0.0 && delta.pitch_radians == 0.0 {
            return false;
        }
        self.active_world.camera.turn_mouse_delta(
            -f64::from(delta.yaw_radians) / ENGINE_CAMERA_MOUSE_SENSITIVITY,
            -f64::from(delta.pitch_radians) / ENGINE_CAMERA_MOUSE_SENSITIVITY,
        );
        true
    }

    pub fn clear_mono_camera_input(&mut self) {
        self.active_world.camera.clear_keys();
    }

    pub fn begin_mono_blink_debug(&mut self) -> bool {
        if self.active_world.runtime.is_none()
            || self.travel_assist_mode != GameTravelAssistMode::Blink
        {
            self.clear_mono_blink_debug();
            return false;
        }
        if !self.ensure_mono_blink_worker() {
            self.clear_mono_blink_debug();
            return false;
        }
        self.mono_blink_debug = MonoBlinkDebugState {
            active: true,
            ..MonoBlinkDebugState::default()
        };
        self.update_mono_blink_debug()
    }

    pub fn clear_mono_blink_debug(&mut self) {
        self.mono_blink_debug = MonoBlinkDebugState::default();
    }

    pub fn update_mono_blink_debug(&mut self) -> bool {
        if !self.mono_blink_debug.active {
            return false;
        }
        if self.travel_assist_mode != GameTravelAssistMode::Blink {
            self.clear_mono_blink_debug();
            return true;
        }
        let mut changed = self.submit_mono_blink_request();
        changed |= self.poll_mono_blink_worker();
        changed
    }

    pub fn commit_mono_blink_debug(&mut self) -> Result<MonoBlinkCommitStatus> {
        if !self.mono_blink_debug.active {
            return Ok(MonoBlinkCommitStatus::Inactive);
        }
        if self.travel_assist_mode != GameTravelAssistMode::Blink {
            self.clear_mono_blink_debug();
            return Ok(MonoBlinkCommitStatus::Inactive);
        }
        if self.active_world.runtime.is_none() {
            self.clear_mono_blink_debug();
            return Ok(MonoBlinkCommitStatus::NoRuntime);
        }
        self.poll_mono_blink_worker();
        self.mono_blink_debug.active = false;
        let Some(preview) = self.mono_blink_debug.preview.take() else {
            return Ok(MonoBlinkCommitStatus::NoValidPreview {
                validity: TeleportValidityReason::NoCandidate,
            });
        };
        let Some(target_feet) = preview.target_feet.filter(|_| preview.is_valid()) else {
            return Ok(MonoBlinkCommitStatus::NoValidPreview {
                validity: preview.validity,
            });
        };
        let pitch_radians = self.active_world.camera.snapshot().pitch_radians;
        self.active_world.camera.set_player_feet_pose(
            target_feet,
            -preview.target_yaw_degrees.to_radians(),
            pitch_radians,
        );
        if let Some(runtime) = self.active_world.runtime.as_ref() {
            self.active_world
                .camera
                .probe_ground(runtime.client(), MONO_GROUND_PROBE_DISTANCE);
        }
        let changed = self.commit_mono_player_pose_now()?;
        Ok(MonoBlinkCommitStatus::Committed {
            target_feet,
            changed,
        })
    }

    fn ensure_mono_blink_worker(&mut self) -> bool {
        match self.services.teleport_preview.ensure_started() {
            Ok(available) => available,
            Err(error) => {
                log::warn!("failed to start Mono Blink preview service: {error}");
                false
            }
        }
    }

    fn submit_mono_blink_request(&mut self) -> bool {
        let intent = mono_blink_intent(&self.active_world.camera);
        if self.mono_blink_debug.last_submitted_intent == Some(intent) {
            return false;
        }
        let Some(runtime) = self.active_world.runtime.as_ref() else {
            return false;
        };
        let config = mono_blink_config();
        let collision = TeleportCollisionSnapshot::capture(runtime.client(), intent, config);
        match self
            .services
            .teleport_preview
            .submit_snapshot(intent, config, collision)
        {
            Ok(Some(request_id)) => {
                self.mono_blink_debug
                    .first_request_id
                    .get_or_insert(request_id);
                self.mono_blink_debug.latest_request_id = Some(request_id);
                self.mono_blink_debug.last_submitted_intent = Some(intent);
                true
            }
            Ok(None) => false,
            Err(error) => {
                log::warn!("Mono Blink preview submit failed: {error}");
                self.services.teleport_preview.reset_service();
                false
            }
        }
    }

    fn poll_mono_blink_worker(&mut self) -> bool {
        match self.services.teleport_preview.try_recv_latest() {
            Ok(Some(result)) => self.accept_mono_blink_result(result),
            Ok(None) => false,
            Err(error) => {
                log::warn!("Mono Blink preview service failed: {error}");
                self.services.teleport_preview.reset_service();
                false
            }
        }
    }

    fn accept_mono_blink_result(&mut self, result: TeleportPreviewResult) -> bool {
        if !self.mono_blink_debug.active
            || self
                .mono_blink_debug
                .first_request_id
                .is_some_and(|first| result.id < first)
            || self
                .mono_blink_debug
                .preview_request_id
                .is_some_and(|preview| result.id <= preview)
        {
            return false;
        }
        self.mono_blink_debug.preview = Some(result.preview);
        self.mono_blink_debug.preview_request_id = Some(result.id);
        true
    }

    fn publish_mono_player_pose_if_due(&mut self) -> Result<bool> {
        if !self.gameplay_startup_complete() {
            return Ok(false);
        }
        let changed = self
            .commit_engine_camera_player_pose_if_due_timed()?
            .is_some_and(|(changed, _)| changed);
        self.reconcile_mono_world_gate(changed)
    }

    fn commit_mono_player_pose_now(&mut self) -> Result<bool> {
        if !self.gameplay_startup_complete() {
            return Ok(false);
        }
        let (changed, _) = self.commit_engine_camera_player_pose_timed()?;
        self.reconcile_mono_world_gate(changed)
    }

    /// Immediate pose reconciliation for deterministic offscreen diagnostics.
    /// Interactive platform adapters must use [`Self::advance_mono_input_frame`]
    /// so their command cadence cannot diverge.
    pub fn force_mono_player_pose_reconcile_for_diagnostics(&mut self) -> Result<bool> {
        self.commit_mono_player_pose_now()
    }

    fn reconcile_mono_world_gate(&mut self, changed: bool) -> Result<bool> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.world_gate.is_none() {
                return Ok(changed);
            }
            let midpoint = self.active_world.camera.snapshot().eye;
            // Crossing observation is an independent side effect. Do not put
            // it behind `changed || ...`: ordinary locomotion changes the
            // camera every frame and would otherwise short-circuit the gate.
            let crossed = self.apply_world_gate_visual_midpoint(midpoint)?;
            return Ok(changed || crossed);
        }
        #[cfg(target_arch = "wasm32")]
        Ok(changed)
    }

    pub fn toggle_mono_camera_view(&mut self) -> EngineCameraViewMode {
        self.active_world.camera.toggle_view_mode()
    }

    pub fn toggle_mono_movement_mode(&mut self) -> EngineCameraMovementMode {
        self.active_world.camera.toggle_movement_mode()
    }

    pub fn adjust_mono_camera_speed(&mut self, amount: f64) {
        self.active_world.camera.adjust_speed(amount);
    }

    pub fn mono_camera_speed_blocks_per_second(&self) -> f64 {
        self.active_world.camera.speed_blocks_per_second()
    }

    pub fn select_mono_hotbar_slot(&mut self, slot: u8) -> bool {
        self.active_world.interaction.select_hotbar_slot(slot)
    }

    pub fn selected_mono_hotbar_slot(&self) -> u8 {
        self.active_world.interaction.selected_hotbar_slot()
    }

    pub fn selected_mono_hotbar_block_state(&self) -> Option<BlockStateId> {
        match self.active_world.interaction.hotbar_items()
            [usize::from(self.active_world.interaction.selected_hotbar_slot())]
        {
            Some(mclone_protocol::DebugHotbarItem::Block(block_state)) => Some(block_state),
            Some(mclone_protocol::DebugHotbarItem::SpawnActor(_)) | None => None,
        }
    }

    pub fn mono_local_world_id_for_ui_id(
        &self,
        id: mclone_ui::WorldCatalogUiWorldId,
    ) -> Option<&LocalWorldId> {
        self.client_experience
            .catalog()
            .local_world_id_for_ui_id(id)
    }

    pub fn step_mono_hotbar_slot(&mut self, step: i8) -> bool {
        if step == 0 {
            return false;
        }
        let selected = i16::from(self.active_world.interaction.selected_hotbar_slot());
        let next = (selected + i16::from(step)).rem_euclid(i16::from(FLAT_HOTBAR_SLOT_COUNT)) as u8;
        self.active_world.interaction.select_hotbar_slot(next)
    }

    pub fn open_mono_pause_menu(&mut self) {
        self.ui.open_pause();
    }

    pub fn open_mono_block_palette(&mut self) {
        self.ui.apply_action(GameUiAction::OpenBlockPalette);
    }

    pub fn mono_ui_is_active(&self) -> bool {
        self.ui.is_active()
    }

    pub fn mono_ui_covers_world(&self) -> bool {
        self.ui.covers_world()
    }

    pub fn mono_ui_screen(&self) -> Option<GameScreen> {
        self.ui.screen()
    }

    pub fn mono_ui_render_state(&self) -> GameUiRenderState {
        self.current_mono_ui_render_state()
    }

    pub fn mono_status_overlay(&self) -> StatusOverlay {
        self.session_projection().status_overlay
    }

    pub fn set_mono_ui_scale(&mut self, scale: GuiScale) {
        self.ui.set_scale(scale);
    }

    pub fn clear_mono_ui_input(&mut self) {
        self.ui.clear_input();
    }

    pub fn mono_ui_key_pressed(&mut self, key: GuiKey) -> (bool, Option<GameUiAction>) {
        self.ui.key_pressed(key)
    }

    pub fn mono_ui_navigate(
        &mut self,
        navigation: mclone_ui::GuiNavigation,
    ) -> (bool, Option<GameUiAction>) {
        self.ui.navigate(navigation)
    }

    pub fn mono_ui_pointer_down(&mut self, point: Point) -> bool {
        self.ui.pointer_down(point)
    }

    pub fn mono_ui_pointer_up(&mut self, point: Point) -> (bool, Option<GameUiAction>) {
        self.ui.pointer_up(point)
    }

    pub fn mono_ui_pointer_move(&mut self, point: Point) -> (bool, Option<GameUiAction>) {
        self.ui.pointer_move(point)
    }

    pub fn set_mono_ui_debug_overlay(&mut self, enabled: bool) {
        self.ui.set_v2_debug_overlay(enabled);
    }

    pub fn mono_ui_v2_is_active(&self) -> bool {
        self.ui.v2_is_active()
    }

    pub fn mono_ui_debug_snapshot(&mut self) -> Option<UiDebugSnapshot> {
        self.ui.v2_debug_snapshot()
    }

    pub fn apply_mono_ui_action<H>(
        &mut self,
        action: GameUiAction,
        from_pointer_click: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        host: &mut H,
    ) -> Result<MonoUiActionOutcome>
    where
        H: HostEffects,
    {
        if matches!(action, GameUiAction::BackToTitle)
            && self.ui.screen() == Some(GameScreen::PreparingLobby)
        {
            self.cancel_warm_world_standby("lobby title cancellation");
        }
        if self.active_world.local_startup.is_some() && !matches!(action, GameUiAction::Quit) {
            return Ok(MonoUiActionOutcome::default());
        }
        if matches!(self.ui.screen(), Some(GameScreen::Death { .. }))
            && !matches!(
                action,
                GameUiAction::Respawn | GameUiAction::QuitToTitle | GameUiAction::Quit
            )
        {
            return Ok(MonoUiActionOutcome::default());
        }

        if from_pointer_click
            && matches!(
                action,
                GameUiAction::StartWorld
                    | GameUiAction::Resume
                    | GameUiAction::JoinRemote
                    | GameUiAction::AssignHotbarBlock { .. }
                    | GameUiAction::AssignHotbarActor { .. }
            )
        {
            host.request_mouse_lock(true)?;
        }

        let settings_state =
            ClientExperienceSettingsState::from(self.current_mono_ui_render_state());
        self.client_experience.set_settings_state(settings_state);
        let active_remote_addr = self.active_remote_addr();
        let current_join_remote_addr = self.ui.join_remote_addr().trim().to_owned();
        let new_world_seed = if matches!(action, GameUiAction::OpenWorldCreate) {
            self.next_new_world_seed()
        } else {
            self.ui.new_world_seed()
        };
        let next_new_world_seed = matches!(
            action,
            GameUiAction::OpenNewWorld | GameUiAction::RerollSeed
        )
        .then(|| self.next_new_world_seed());
        let effects = self.client_experience.apply_ui_action(
            action,
            ClientExperienceActionContext {
                new_world_seed,
                next_new_world_seed,
                current_join_remote_addr: &current_join_remote_addr,
                fallback_remote_addr: active_remote_addr.as_deref(),
            },
        );
        let starts_session = effects.session.session_start.is_some()
            || !effects.catalog.session_starts.is_empty()
            || !effects.scenario.is_empty()
            || matches!(
                action,
                GameUiAction::OpenWorld(_) | GameUiAction::CreateCatalogWorld
            );
        if starts_session {
            host.request_mouse_lock(false)?;
        }
        let apply_ui_action =
            client_experience_should_apply_ui_projection(action) && effects.projection.is_empty();
        let scene_replaced =
            self.apply_mono_client_experience_effects(effects, device, queue, host)?;
        if apply_ui_action {
            self.ui.apply_action(action);
        }
        if !self.ui.is_active() {
            self.clear_menu_input_state();
        }
        let render_state = self.current_mono_ui_render_state();
        self.ui.commit_render_state(render_state);
        Ok(MonoUiActionOutcome {
            scene_replaced,
            session_start_requested: starts_session,
            clear_gameplay_input: true,
        })
    }

    fn apply_mono_client_experience_effects<H>(
        &mut self,
        effects: ClientExperienceEffects,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        host: &mut H,
    ) -> Result<bool>
    where
        H: HostEffects,
    {
        let catalog_scene_replaced =
            self.apply_xr_catalog_effects(effects.catalog, device, queue)?;
        let session_scene_replaced =
            self.apply_mono_session_effects(effects.session, device, queue, host)?;
        let mut scenario_scene_replaced = false;
        for effect in effects.scenario {
            scenario_scene_replaced |= self.apply_lobby_effect(effect, device, queue)?;
        }
        self.apply_asset_pack_effects(effects.asset_packs);
        for effect in effects.local_data {
            self.apply_local_data_effect(effect);
        }
        if !apply_client_experience_settings_effects(self, host, effects.settings)? {
            return Ok(catalog_scene_replaced || session_scene_replaced || scenario_scene_replaced);
        }
        for effect in effects.gameplay {
            match effect {
                ClientExperienceGameplayEffect::AssignHotbarBlock { slot, block_state } => {
                    self.assign_debug_hotbar_slot(slot, BlockStateId(block_state))?;
                }
                ClientExperienceGameplayEffect::AssignHotbarActor { slot, actor } => {
                    self.assign_debug_hotbar_actor(slot, actor)?;
                }
                ClientExperienceGameplayEffect::Respawn => {
                    if let Some(runtime) = self.active_world.runtime.as_mut() {
                        runtime.send_gameplay_command(mclone_protocol::ClientCommand::Respawn)?;
                    }
                }
            }
        }
        for effect in effects.projection {
            match effect {
                ClientExperienceProjectionEffect::ApplyUiAction(action) => {
                    self.ui.apply_action(action);
                }
                ClientExperienceProjectionEffect::SuppressUiAction => {}
            }
        }
        Ok(catalog_scene_replaced || session_scene_replaced || scenario_scene_replaced)
    }

    fn apply_mono_session_effects<H>(
        &mut self,
        effects: ClientSessionEffects,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        host: &mut H,
    ) -> Result<bool>
    where
        H: HostEffects,
    {
        if let Some(seed) = effects.new_world_seed {
            self.ui.set_new_world_seed(seed);
        }
        if let Some(addr) = effects.join_remote_addr {
            self.ui.set_join_remote_addr(addr);
        }
        if effects.clear_inactive_session_status {
            self.clear_inactive_session_status();
        }
        let mut scene_replaced = false;
        if let Some(request) = effects.session_start {
            scene_replaced = self.start_session_for_request(device, queue, request)?;
        }
        if let Some(host_action) = effects.host_action {
            if matches!(host_action, ClientSessionHostAction::QuitToTitle) {
                let transition = client_session_quit_to_title_transition(self.session.state());
                self.apply_xr_session_transition_effects(transition, device, queue)?;
            }
            apply_client_session_host_action(host_action, host)?;
        }
        Ok(scene_replaced)
    }

    pub fn shoot_mono_debug_physics_cube(&mut self) -> Result<bool> {
        if self.active_world.runtime.is_none() {
            return Ok(false);
        }
        self.commit_mono_player_pose_now()?;
        self.active_world
            .runtime
            .as_mut()
            .expect("runtime presence checked")
            .send_gameplay_command(mclone_protocol::ClientCommand::ShootDebugPhysicsCube)
            .map(|_| true)
            .map_err(Into::into)
    }

    pub fn handle_mono_world_action(
        &mut self,
        action: FlatInputAction,
    ) -> Result<MonoWorldActionStatus> {
        if self.active_world.runtime.is_none() {
            return Ok(MonoWorldActionStatus::NoRuntime);
        }
        if self.embedded_world_activation.phase.active() {
            return Ok(MonoWorldActionStatus::NoCommand);
        }
        if action == FlatInputAction::Use {
            let camera = self.active_world.camera.snapshot();
            let direction = mclone_client::view_vector(camera.yaw_radians, camera.pitch_radians);
            if self.request_embedded_world_activation(camera.eye, direction) {
                return Ok(MonoWorldActionStatus::EmbeddedWorldActivationRequested);
            }
        }
        let behavior = self.active_world.scene.world_behavior_profile;
        if (action == FlatInputAction::Attack && !behavior.allows_player_break())
            || (action == FlatInputAction::Use && !behavior.allows_player_place())
        {
            return Ok(MonoWorldActionStatus::DeniedByWorldBehavior);
        }
        self.commit_mono_player_pose_now()?;
        if let Some(command) = self.active_world.interaction.ensure_has_sent_carried_item() {
            self.active_world
                .runtime
                .as_mut()
                .expect("runtime presence checked")
                .send_gameplay_command(command)?;
        }
        let Some(target) = self.current_mono_block_target() else {
            return Ok(MonoWorldActionStatus::NoTarget);
        };
        let command = match action {
            FlatInputAction::Attack => self
                .active_world
                .interaction
                .debug_instant_break_command(target.hit),
            FlatInputAction::Use => self
                .active_world
                .interaction
                .use_item_on_command(target.hit),
            _ => None,
        };
        let Some(command) = command else {
            return Ok(MonoWorldActionStatus::NoCommand);
        };
        self.active_world
            .runtime
            .as_mut()
            .expect("runtime presence checked")
            .send_gameplay_command(command)?;
        Ok(MonoWorldActionStatus::Submitted { target })
    }

    pub fn mono_block_target(&self) -> Option<BlockInteractionTarget> {
        self.current_mono_block_target()
    }

    pub const fn active_world_behavior_profile(&self) -> mclone_server::WorldBehaviorProfile {
        self.active_world.scene.world_behavior_profile
    }

    pub fn mono_time_of_day(&self) -> f32 {
        self.time_of_day()
    }

    pub fn mono_sun_angle(&self) -> f32 {
        self.sun_angle()
    }

    fn current_mono_block_target(&self) -> Option<BlockInteractionTarget> {
        let runtime = self.active_world.runtime.as_ref()?;
        self.active_world
            .camera
            .target_block(runtime.client(), &self.active_world.interaction)
    }
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
        Ok(self
            .render_mono_frame_inner(
                frame,
                depth,
                render_view,
                ui,
                XrTerrainRuntimeUpdateMode::Live,
            )?
            .render)
    }

    /// Rich mono-frame outcome for live surface drivers. It retains the
    /// shared runtime/upload timing and admission summary needed by frame
    /// accounting, while the compatibility entry above returns only pixels.
    pub fn render_mono_scene_frame(
        &mut self,
        frame: RenderFrameContext<'_>,
        depth: &ChunkDepthTarget,
        render_view: ChunkRenderView,
        ui: MonoUiPresentation,
    ) -> Result<MonoSceneFrameSummary> {
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
        Ok(self
            .render_mono_frame_inner(
                frame,
                depth,
                render_view,
                ui,
                XrTerrainRuntimeUpdateMode::Frozen,
            )?
            .render)
    }

    pub fn render_mono_scene_frame_frozen(
        &mut self,
        frame: RenderFrameContext<'_>,
        depth: &ChunkDepthTarget,
        render_view: ChunkRenderView,
        ui: MonoUiPresentation,
    ) -> Result<MonoSceneFrameSummary> {
        self.render_mono_frame_inner(
            frame,
            depth,
            render_view,
            ui,
            XrTerrainRuntimeUpdateMode::Frozen,
        )
    }

    /// Render one through four independently posed flat views while advancing
    /// shared scene/runtime work exactly once. Cardinality one delegates to the
    /// existing mono fast path without materializing multi-view preparation.
    pub fn render_flat_presentation_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        views: &[FlatPresentationView<'_>],
    ) -> Result<FlatPresentationFrameSummary> {
        self.render_flat_presentation_frame_inner(
            device,
            queue,
            encoder,
            views,
            XrTerrainRuntimeUpdateMode::Live,
        )
    }

    /// Frozen-runtime counterpart used by deterministic auxiliary-view
    /// captures after the primary view has finished streaming.
    pub fn render_flat_presentation_frame_frozen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        views: &[FlatPresentationView<'_>],
    ) -> Result<FlatPresentationFrameSummary> {
        self.render_flat_presentation_frame_inner(
            device,
            queue,
            encoder,
            views,
            XrTerrainRuntimeUpdateMode::Frozen,
        )
    }

    fn render_flat_presentation_frame_inner(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        views: &[FlatPresentationView<'_>],
        runtime_mode: XrTerrainRuntimeUpdateMode,
    ) -> Result<FlatPresentationFrameSummary> {
        let admission = admit_flat_presentation_views(views.len())?;
        if let [view] = views {
            let mono = self.render_mono_frame_inner(
                RenderFrameContext::new(device, queue, encoder, view.target),
                view.depth,
                view.render_view,
                view.ui,
                runtime_mode,
            )?;
            return Ok(FlatPresentationFrameSummary {
                shared_preparation_count: admission.shared_preparation_count,
                rendered_view_count: admission.rendered_view_count,
                views: vec![FlatPresentationViewSummary {
                    view: PresentationViewIndex::PRIMARY,
                    render: mono.render,
                }],
                timing: mono.timing,
                upload: mono.upload,
            });
        }

        if self
            .embedded_world_preview
            .as_ref()
            .is_some_and(|preview| preview.phase == EmbeddedWorldPreviewPhase::Visible)
        {
            bail!("multi-view flat presentation does not yet compose an embedded-world preview");
        }

        self.poll_asset_replacement(device, queue)?;
        if matches!(runtime_mode, XrTerrainRuntimeUpdateMode::Live) {
            self.advance_local_startup(device, queue)?;
        }

        // The primary view owns ordinary player interest. Auxiliary views may
        // inspect already resident facts but do not manufacture authority or a
        // second interest source.
        let center_position = views[0].render_view.camera_position;
        let frame_deadline = self.render_compile_frame_deadline();
        let mut timing = XrTerrainFrameTiming::default();
        let upload = self.live_upload_for_frame(
            device,
            center_position,
            runtime_mode,
            frame_deadline,
            &mut timing,
        )?;
        self.sync_player_lifecycle_ui();

        let sky_clear_color = self.sky_clear_color();
        let time_of_day = self.time_of_day();
        let sun_angle = self.sun_angle();
        let actor_instances = self.current_actor_instances();
        let prepared_records = self.active_world.draw.prepare_render_records();
        let far_lod_config = self.active_world.scene.far_lod;
        let far_lod_seed = self.active_world.scene.seed;
        let far_lod_center = self.active_world.camera.snapshot().chunk_pos;
        let lod_grant = self.active_world.render_admission_policy.lod_grant();
        let far_lod_mesh = if let Some(runtime) = self.active_world.runtime.as_mut() {
            runtime
                .prepare_far_lod_frame(
                    far_lod_config,
                    far_lod_seed,
                    far_lod_center,
                    center_position,
                    lod_grant.build_tiles,
                    lod_grant.upload_tiles,
                )?
                .cloned()
        } else {
            None
        };
        let uniform_frame = self.next_per_view_uniform_frame();
        let render_start = self.services.clock.now();
        let mut view_summaries = Vec::with_capacity(views.len());
        let mut primary_render_stats = None;
        for (index, view) in views.iter().enumerate() {
            let view_index = PresentationViewIndex::new(index as u32);
            let view_slot = PerViewSlot::for_view(view_index).in_uniform_frame(uniform_frame);
            let render_options = self.effective_render_options(view.render_view.camera_position);
            let underwater_overlay = self.mono_underwater_overlay(view.render_view);
            let gui_scale = GuiScale::from_pixels(view.target.size[0], view.target.size[1]);
            let (full_frame_gui, gui_draw, hud_cache) = self.mono_gui_frame(gui_scale, view.ui);
            if matches!(view.ui, MonoUiPresentation::ScreenSpaceHud) {
                self.ensure_mono_gui(device, queue)?;
            }
            let world_gui =
                FullFrameGui::new(false, full_frame_gui.covers_world, full_frame_gui.scale);
            let mut render_stats = self.active_world.render_stats;
            let actor_preparation = if index == 0 {
                FrameActorPreparation::Refresh
            } else {
                FrameActorPreparation::ReusePrepared
            };
            let far_lod = far_lod_mesh
                .as_ref()
                .map(|_| &mut self.active_world.far_lod);
            #[cfg(not(target_arch = "wasm32"))]
            let opaque_world_gate = self
                .opaque_world_gate_renderer
                .as_ref()
                .zip(self.world_gate.as_ref().map(WorldGate::render_gate));
            #[cfg(target_arch = "wasm32")]
            let opaque_world_gate = None;
            let mut summary = render_full_frame_for_view_with_far_lod_and_prepared_records_in_slot(
                RenderFrameContext::new(device, queue, encoder, view.target),
                view.depth,
                &self.sky,
                &mut self.active_world.draw,
                &prepared_records,
                far_lod,
                far_lod_mesh.as_ref(),
                opaque_world_gate,
                Some(
                    self.active_world
                        .actors
                        .as_mut()
                        .expect("active world owns actor draw state"),
                ),
                Some(&mut self.screen_effects),
                None,
                view.render_view,
                &actor_instances,
                underwater_overlay,
                sky_clear_color,
                time_of_day,
                sun_angle,
                render_options,
                world_gui,
                |_| GuiDrawList::new(),
                &mut render_stats,
                view_slot,
                actor_preparation,
            )
            .with_context(|| format!("render flat presentation view {index}"))?;

            if !full_frame_gui.covers_world {
                let selection_view =
                    render_view_with_underwater_effect(view.render_view, underwater_overlay);
                let selection = self
                    .current_mono_block_target()
                    .map(|target| SelectionOutline::new(target.outline_boxes));
                self.selection_outline.render_in_slot(
                    device,
                    queue,
                    encoder,
                    view.target,
                    view.depth,
                    selection_view,
                    selection.as_ref(),
                    view_slot,
                );
                let mut world_lines = engine_debug_world_lines(
                    &self.active_world.camera,
                    EngineDebugVisualOptions::new(self.player_collision_box_visible),
                );
                if self.diagnostic_panel.debug_diagnostics_visible() {
                    world_lines.extend(topology_debug_world_lines(
                        self.active_world
                            .runtime
                            .as_ref()
                            .map_or(HorizontalTopology::UNBOUNDED, |runtime| {
                                runtime.client().topology()
                            }),
                        selection_view.camera_position,
                    ));
                }
                if let Some(preview) = self.mono_blink_debug.preview.as_ref() {
                    world_lines.extend(mono_blink_lines(preview));
                }
                if !world_lines.is_empty() {
                    self.world_gui_renderer
                        .render_lines_in_slot(
                            device,
                            queue,
                            encoder,
                            view.target,
                            selection_view,
                            &world_lines,
                            view_slot,
                        )
                        .with_context(|| {
                            format!("render flat presentation debug lines for view {index}")
                        })?;
                }
            }

            summary.gui_command_count = gui_draw.commands().len();
            summary.flat_hud_retained_cache = hud_cache;
            if full_frame_gui.active {
                self.mono_gui
                    .as_mut()
                    .expect("Mono GUI renderer initialized for active UI")
                    .render(
                        device,
                        queue,
                        encoder,
                        view.target,
                        full_frame_gui.scale,
                        &gui_draw,
                        if full_frame_gui.covers_world {
                            GuiRenderOptions::clear(mclone_render::default_clear_color())
                        } else {
                            GuiRenderOptions::overlay()
                        },
                    )
                    .with_context(|| format!("render flat presentation UI for view {index}"))?;
            }
            if !matches!(view.ui, MonoUiPresentation::None)
                && let Some(overlay) = self.embedded_world_activation_fade_overlay()
            {
                self.screen_effects.render_fade_in_slot(
                    device,
                    queue,
                    encoder,
                    view.target,
                    overlay,
                    view_slot,
                );
            }

            if index == 0 {
                primary_render_stats = Some(render_stats);
            }
            view_summaries.push(FlatPresentationViewSummary {
                view: view_index,
                render: summary,
            });
        }
        timing.render_views_ms = elapsed_ms(self.services.clock.elapsed_since(render_start));

        if let Some(stats) = primary_render_stats {
            self.active_world.render_stats = stats;
        }
        self.rendered_frames = self.rendered_frames.wrapping_add(1);
        let primary_drawn_sections = view_summaries[0].render.drawn_section_count;
        self.record_warm_world_first_destination_frame(primary_drawn_sections, upload);
        self.record_embedded_world_activation_frame(primary_drawn_sections, upload, views.len());
        Ok(FlatPresentationFrameSummary {
            shared_preparation_count: admission.shared_preparation_count,
            rendered_view_count: admission.rendered_view_count,
            views: view_summaries,
            timing,
            upload,
        })
    }

    /// Whether the local-world startup pump has finished promoting into a live
    /// runtime. Non-blocking; drivers own the drive-to-ready loop (Web posture
    /// rule: blocking convenience loops live in native drivers, not the host).
    pub fn local_startup_complete(&self) -> bool {
        self.active_world.local_startup.is_none()
    }

    /// Whether authoritative startup and drawable coverage admit gameplay.
    /// Platform drivers may expose this fact, but movement/pose methods enforce
    /// it internally so a slow host cannot accidentally run gravity early.
    pub fn gameplay_startup_complete(&self) -> bool {
        self.active_world.local_startup.is_none()
            && !self.active_world.external_runtime_startup_pending
            && self.active_world.runtime.is_some()
    }

    /// Ready or in-flight render work relevant to the requested camera plus
    /// queued client uploads. Dirty chunks waiting on neighbor readiness do not
    /// keep deterministic capture loops alive: they cannot make progress until
    /// another runtime update arrives, and the next frame will observe that
    /// update normally. Returns [`usize::MAX`] while local startup is promoting.
    pub fn pending_stream_work(&self, camera_position: Vec3) -> usize {
        if !self.gameplay_startup_complete() {
            return usize::MAX;
        }
        let Some(runtime) = self.active_world.runtime.as_ref() else {
            return usize::MAX;
        };
        let target = runtime.target_render_work_stats(camera_position);
        let upload = self.active_world.section_uploads.stats();
        let far_lod = runtime.far_lod_stats();
        target.inflight_render_sections
            + usize::from(target.ready_render_work_pending)
            + upload.queued_upload_sections
            + upload.queued_lifecycle_items
            + far_lod.pending_builds
            + far_lod.inflight_builds
            + far_lod.queued_uploads
    }

    fn render_mono_frame_inner(
        &mut self,
        frame: RenderFrameContext<'_>,
        depth: &ChunkDepthTarget,
        render_view: ChunkRenderView,
        ui: MonoUiPresentation,
        runtime_mode: XrTerrainRuntimeUpdateMode,
    ) -> Result<MonoSceneFrameSummary> {
        let RenderFrameContext {
            device,
            queue,
            encoder,
            target,
        } = frame;
        self.poll_asset_replacement(device, queue)?;
        if matches!(runtime_mode, XrTerrainRuntimeUpdateMode::Live) {
            self.advance_local_startup(device, queue)?;
        }

        let center_position = render_view.camera_position;
        let frame_deadline = self.render_compile_frame_deadline();
        let mut timing = XrTerrainFrameTiming::default();
        // Poll/sync/upload/compile-release under the shared budgeted admission
        // path (or the frozen no-op summary), exactly as the stereo frame does.
        // These use their own command buffers, independent of `frame`.
        let upload = self.live_upload_for_frame(
            device,
            center_position,
            runtime_mode,
            frame_deadline,
            &mut timing,
        )?;
        self.sync_player_lifecycle_ui();

        // Frame-input assembly reuses the shared accessors. The shared
        // `render_full_frame_for_view*` entry applies sky-darken and underwater
        // fog internally, so pass the base effective options plus the overlay.
        let render_options = self.effective_render_options(center_position);
        let sky_clear_color = self.sky_clear_color();
        let time_of_day = self.time_of_day();
        let sun_angle = self.sun_angle();
        let underwater_overlay = self.mono_underwater_overlay(render_view);
        let actor_instances = self.current_actor_instances();
        let preview_actor_instances = self.current_preview_actor_instances();

        let gui_scale = GuiScale::from_pixels(target.size[0], target.size[1]);
        let (full_frame_gui, gui_draw, hud_cache) = self.mono_gui_frame(gui_scale, ui);
        if matches!(ui, MonoUiPresentation::ScreenSpaceHud) {
            self.ensure_mono_gui(device, queue)?;
        }

        let render_start = self.services.clock.now();
        let far_lod_config = self.active_world.scene.far_lod;
        let far_lod_seed = self.active_world.scene.seed;
        let far_lod_center = self.active_world.camera.snapshot().chunk_pos;

        let mut render_stats = self.active_world.render_stats;

        // `runtime`, `far_lod`, `mono_gui`, `sky`, `draw`, `actors`, and
        // `screen_effects` are disjoint fields, so these borrows coexist.
        let lod_grant = self.active_world.render_admission_policy.lod_grant();
        let far_lod_mesh = if let Some(runtime) = self.active_world.runtime.as_mut() {
            runtime.prepare_far_lod_frame(
                far_lod_config,
                far_lod_seed,
                far_lod_center,
                render_view.camera_position,
                lod_grant.build_tiles,
                lod_grant.upload_tiles,
            )?
        } else {
            None
        };
        let far_lod = far_lod_mesh.map(|_| &mut self.active_world.far_lod);
        #[cfg(not(target_arch = "wasm32"))]
        let opaque_world_gate = self
            .opaque_world_gate_renderer
            .as_ref()
            .zip(self.world_gate.as_ref().map(WorldGate::render_gate));
        #[cfg(target_arch = "wasm32")]
        let opaque_world_gate = None;
        let preview_records = self
            .embedded_world_preview
            .as_ref()
            .filter(|preview| preview.phase == EmbeddedWorldPreviewPhase::Visible)
            .and_then(|preview| {
                self.standby_world
                    .as_ref()
                    .filter(|slot| slot.id == preview.source_world)
                    .map(|slot| {
                        slot.draw
                            .prepare_render_records_for_context(preview.context)
                    })
            });
        let world_gui = FullFrameGui::new(false, full_frame_gui.covers_world, full_frame_gui.scale);
        let (mut summary, preview_render_timing, preview_translucent_order) =
            if let Some(preview_records) = preview_records.as_ref() {
                let preview = self
                    .embedded_world_preview
                    .as_ref()
                    .expect("visible preview record preparation retains preview state");
                let standby = self
                    .standby_world
                    .as_mut()
                    .expect("visible preview retains its source slot");
                let preview_time = standby
                    .runtime
                    .as_ref()
                    .map_or(0.0, |runtime| runtime.time_of_day());
                let preview_options = self
                    .render_options
                    .with_sky_darken(mclone_render::light_texture::sky_darken(preview_time));
                let terrain_view =
                    render_view_with_underwater_effect(render_view, underwater_overlay);
                let active_records = self.active_world.draw.prepare_render_records();
                let active_translucent = self.active_world.draw.prepare_placed_translucent_records(
                    &active_records,
                    terrain_view,
                    render_options,
                    mclone_render::placement::WorldCompositionContext::unbounded(
                        mclone_render::placement::WorldPlacement::identity(),
                        None,
                    ),
                );
                let placed_translucent = standby.draw.prepare_placed_translucent_records(
                    preview_records,
                    terrain_view,
                    preview_options,
                    preview.context,
                );
                let translucent_order = compose_translucent_terrain_order(
                    self.active_world.id,
                    active_translucent,
                    preview.source_world,
                    placed_translucent,
                    std::slice::from_ref(&terrain_view),
                );
                let translucent_order_snapshot = embedded_translucent_order_snapshot(
                    self.active_world.id,
                    preview.source_world,
                    &translucent_order,
                );
                let placed_actors = preview_actor_instances
                    .as_ref()
                    .filter(|actors| actors.source_world == preview.source_world)
                    .and_then(|actors| {
                        standby.actors.as_mut().map(|draw| PlacedActorFrame {
                            draw,
                            instances: &actors.instances,
                            context: preview.context,
                            render_options: preview_options,
                        })
                    });
                let rendered = render_full_frame_for_view_with_far_lod_and_placed_terrain_timed(
                    RenderFrameContext::new(device, queue, encoder, target),
                    depth,
                    &self.sky,
                    &mut self.active_world.draw,
                    far_lod,
                    far_lod_mesh,
                    TerrainCompositionFrame {
                        placed: PlacedTerrainFrame {
                            draw: &standby.draw,
                            renderer: &preview.renderer,
                            prepared: PlacedTerrainPrepared::Mono(preview_records),
                            context: preview.context,
                            render_options: preview_options,
                        },
                        actors: placed_actors,
                        translucent_order: &translucent_order,
                    },
                    Some(
                        self.active_world
                            .actors
                            .as_mut()
                            .expect("active world owns actor draw state"),
                    ),
                    Some(&mut self.screen_effects),
                    None,
                    render_view,
                    &actor_instances,
                    underwater_overlay,
                    sky_clear_color,
                    time_of_day,
                    sun_angle,
                    render_options,
                    world_gui,
                    |_| GuiDrawList::new(),
                    &self.services.clock,
                    &mut render_stats,
                );
                rendered
                    .map(|(summary, timing)| (summary, Some(timing), translucent_order_snapshot))
            } else {
                render_full_frame_for_view_with_far_lod_and_opaque_gate(
                    RenderFrameContext::new(device, queue, encoder, target),
                    depth,
                    &self.sky,
                    &mut self.active_world.draw,
                    far_lod,
                    far_lod_mesh,
                    opaque_world_gate,
                    Some(
                        self.active_world
                            .actors
                            .as_mut()
                            .expect("active world owns actor draw state"),
                    ),
                    Some(&mut self.screen_effects),
                    None,
                    render_view,
                    &actor_instances,
                    underwater_overlay,
                    sky_clear_color,
                    time_of_day,
                    sun_angle,
                    render_options,
                    world_gui,
                    |_| GuiDrawList::new(),
                    &mut render_stats,
                )
                .map(|summary| {
                    (
                        summary,
                        None,
                        EmbeddedWorldPreviewTranslucentOrderSnapshot::default(),
                    )
                })
            }
            .context("render mono scene frame")?;
        let preview_actor_receipt = preview_actor_instances.as_ref().and_then(|actors| {
            self.standby_world
                .as_ref()
                .filter(|slot| slot.id == actors.source_world)
                .and_then(|slot| slot.actors.as_ref())
                .map(|resources| {
                    (
                        actors.entity_count,
                        actors.remote_player_count,
                        actors.source_local_player_count,
                        resources.resource_snapshot(),
                    )
                })
        });
        let actor_rendered_at = self.services.clock.now();
        if let Some(preview) = self.embedded_world_preview.as_mut() {
            let stats = TexturedSectionRenderStats {
                drawn_section_count: summary.placed_drawn_section_count,
                drawn_index_count: summary.placed_drawn_index_count,
                ..TexturedSectionRenderStats::default()
            };
            let bounded_section_count = preview_records
                .as_ref()
                .map_or(0, |records| records.section_keys().len());
            let out_of_region_submission_count = preview_records.as_ref().map_or(0, |records| {
                records
                    .section_keys()
                    .filter(|key| !preview.region.contains(*key))
                    .count()
            });
            let timing = preview_render_timing.unwrap_or_default();
            preview.record_render(
                bounded_section_count,
                out_of_region_submission_count,
                timing.placed_cull_ms,
                timing.placed_draw_ms,
                stats,
                preview_translucent_order,
            );
            if let Some((entities, remote_players, source_local_players, resources)) =
                preview_actor_receipt
            {
                preview.record_actor_render(
                    entities,
                    remote_players,
                    source_local_players,
                    timing.placed_actor_ms,
                    summary.placed_actor_stats,
                    resources,
                    &preview_actor_instances
                        .as_ref()
                        .expect("preview actor receipt requires collected actors")
                        .entity_observations,
                    &preview_actor_instances
                        .as_ref()
                        .expect("preview actor receipt requires collected actors")
                        .remote_player_observations,
                    actor_rendered_at,
                );
            }
        }

        if !full_frame_gui.covers_world {
            let selection_view =
                render_view_with_underwater_effect(render_view, underwater_overlay);
            let selection = self
                .current_mono_block_target()
                .map(|target| SelectionOutline::new(target.outline_boxes));
            self.selection_outline.render_in_slot(
                device,
                queue,
                encoder,
                target,
                depth,
                selection_view,
                selection.as_ref(),
                SINGLE_VIEW_SLOT,
            );
            let mut world_lines = engine_debug_world_lines(
                &self.active_world.camera,
                EngineDebugVisualOptions::new(self.player_collision_box_visible),
            );
            if self.diagnostic_panel.debug_diagnostics_visible() {
                world_lines.extend(topology_debug_world_lines(
                    self.active_world
                        .runtime
                        .as_ref()
                        .map_or(HorizontalTopology::UNBOUNDED, |runtime| {
                            runtime.client().topology()
                        }),
                    selection_view.camera_position,
                ));
            }
            if let Some(preview) = self.mono_blink_debug.preview.as_ref() {
                world_lines.extend(mono_blink_lines(preview));
            }
            if !world_lines.is_empty() {
                self.world_gui_renderer
                    .render_lines_in_slot(
                        device,
                        queue,
                        encoder,
                        target,
                        selection_view,
                        &world_lines,
                        SINGLE_VIEW_SLOT,
                    )
                    .context("render Mono world debug lines")?;
            }
        }

        summary.gui_command_count = gui_draw.commands().len();
        summary.flat_hud_retained_cache = hud_cache;
        if full_frame_gui.active {
            self.mono_gui
                .as_mut()
                .expect("Mono GUI renderer initialized for active UI")
                .render(
                    device,
                    queue,
                    encoder,
                    target,
                    full_frame_gui.scale,
                    &gui_draw,
                    if full_frame_gui.covers_world {
                        GuiRenderOptions::clear(mclone_render::default_clear_color())
                    } else {
                        GuiRenderOptions::overlay()
                    },
                )
                .context("render Mono screen-space UI")?;
        }
        if !matches!(ui, MonoUiPresentation::None)
            && let Some(overlay) = self.embedded_world_activation_fade_overlay()
        {
            self.screen_effects.render_fade_in_slot(
                device,
                queue,
                encoder,
                target,
                overlay,
                SINGLE_VIEW_SLOT,
            );
        }
        timing.render_views_ms = elapsed_ms(self.services.clock.elapsed_since(render_start));

        self.active_world.render_stats = render_stats;
        self.rendered_frames = self.rendered_frames.wrapping_add(1);
        self.record_warm_world_first_destination_frame(summary.drawn_section_count, upload);
        self.record_embedded_world_activation_frame(summary.drawn_section_count, upload, 1);
        Ok(MonoSceneFrameSummary {
            render: summary,
            timing,
            upload,
        })
    }

    /// Compose only the flat screen-space UI into an already-rendered target.
    /// Surface drivers use this after scaled world presentation so HUD/menu
    /// pixels stay at the native output resolution.
    pub fn render_mono_screen_space_ui(
        &mut self,
        frame: RenderFrameContext<'_>,
    ) -> Result<(usize, UiDrawCacheStats)> {
        let RenderFrameContext {
            device,
            queue,
            encoder,
            target,
        } = frame;
        let gui_scale = GuiScale::from_pixels(target.size[0], target.size[1]);
        let (gui, draw, hud_cache) =
            self.mono_gui_frame(gui_scale, MonoUiPresentation::ScreenSpaceHud);
        self.ensure_mono_gui(device, queue)?;
        if gui.active {
            self.mono_gui
                .as_mut()
                .expect("Mono GUI renderer initialized")
                .render(
                    device,
                    queue,
                    encoder,
                    target,
                    gui.scale,
                    &draw,
                    if gui.covers_world {
                        GuiRenderOptions::clear(mclone_render::default_clear_color())
                    } else {
                        GuiRenderOptions::overlay()
                    },
                )
                .context("render native-resolution Mono screen-space UI")?;
        }
        if let Some(overlay) = self.embedded_world_activation_fade_overlay() {
            self.screen_effects.render_fade_in_slot(
                device,
                queue,
                encoder,
                target,
                overlay,
                SINGLE_VIEW_SLOT,
            );
        }
        Ok((draw.commands().len(), hud_cache))
    }

    /// Build the screen-space GUI draw list + `FullFrameGui` flags for a mono
    /// frame. Reuses the same [`GameUiHost`] draw list the stereo world-quad
    /// path renders, just laid out at the flat target resolution.
    fn mono_gui_frame(
        &mut self,
        gui_scale: GuiScale,
        ui: MonoUiPresentation,
    ) -> (FullFrameGui, GuiDrawList, UiDrawCacheStats) {
        match ui {
            MonoUiPresentation::None => (
                FullFrameGui::new(false, false, [gui_scale.width, gui_scale.height]),
                GuiDrawList::new(),
                UiDrawCacheStats::default(),
            ),
            MonoUiPresentation::ScreenSpaceHud => {
                let ui_state = self.current_mono_ui_render_state();
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
                let mut hud_cache = UiDrawCacheStats::default();
                // A full-screen menu covers the world; a bare HUD does not.
                let menu_active = self.ui.is_active();
                let covers_world = self.ui.covers_world();
                if let Some(hud) = self.mono_flat_hud(menu_active) {
                    let hud_draw = self.ui.render_flat_hud_draw_list(gui_scale, &hud);
                    draw.append(&hud_draw.draw);
                    hud_cache = hud_draw.retained_cache;
                }
                if self.diagnostic_panel.debug_diagnostics_visible()
                    && !menu_active
                    && let Some(progress) = self
                        .active_world
                        .runtime
                        .as_ref()
                        .and_then(|runtime| runtime.view_readiness_overlay())
                {
                    self.ui.append_loading_progress_draw(
                        gui_scale,
                        &mut draw,
                        &progress,
                        mclone_ui::LoadingProgressOverlayLayer::panel(
                            Point {
                                x: (gui_scale.width - 132.0).max(4.0),
                                y: 4.0,
                            },
                            "VIEW",
                        ),
                    );
                }
                (
                    FullFrameGui::new(true, covers_world, [gui_scale.width, gui_scale.height]),
                    draw,
                    hud_cache,
                )
            }
        }
    }

    fn mono_flat_hud(&self, menu_active: bool) -> Option<FlatHud> {
        let runtime = self.active_world.runtime.as_ref()?;
        let context = self.mono_ui_context.clone().unwrap_or_default();
        let palette_active = self.ui.screen() == Some(GameScreen::BlockPalette);
        let mut hud = FlatHud::new(context.resolved_input);
        hud.world_hud_visible = context.hud_visible && (!menu_active || palette_active);
        hud.crosshair_visible = context.hud_visible && self.crosshair_visible && !menu_active;
        hud.hotbar = FlatHotbarOverlay::selected_with_icons(
            self.active_world.interaction.selected_hotbar_slot(),
            debug_hotbar_icons(
                self.active_world.interaction.hotbar_items(),
                &runtime.mesh_assets().catalog,
            ),
        );
        hud.status = self.session_projection().status_overlay;
        let vitals = runtime.client().player_vitals();
        hud.player_health = Some(mclone_ui::PlayerHealthHud {
            health: vitals.health(),
            max_health: vitals.max_health(),
        });
        let statistics = runtime.client().player_statistics();
        hud.player_statistics = Some(mclone_ui::PlayerStatisticsHud {
            jumps: statistics.jump_count(),
            successful_block_placements: statistics.successful_block_placement_count(),
        });
        hud.touch = context.touch_overlay;
        hud.frame_pipeline = self
            .diagnostic_panel
            .frame_metrics_visible()
            .then(|| self.diagnostic_panel.frame_metrics_overlay())
            .flatten();
        if self.diagnostic_panel.debug_diagnostics_visible() && !menu_active {
            let camera = self
                .active_world
                .camera
                .frame_state(&self.active_world.interaction);
            let snapshot = self.active_world.camera.snapshot();
            let render_options = self.effective_render_options(glam_vec3_from_vec3d(snapshot.eye));
            let topology = runtime.client().topology();
            let topology_actor = (!topology.is_unbounded())
                .then(|| {
                    runtime
                        .client()
                        .actor_presentations()
                        .into_iter()
                        .find_map(|presentation| {
                            let mclone_client::ActorPresentationId::Entity(entity_id) =
                                presentation.id
                            else {
                                return None;
                            };
                            let lifted = topology
                                .nearest_position_lift(presentation.feet_position, snapshot.eye);
                            let delta = topology.shortest_position_displacement(
                                snapshot.eye,
                                presentation.feet_position,
                            );
                            Some(format!(
                                "ACT E{} C{:.2} L{:.2} D{:+.2} X{}",
                                entity_id.0,
                                presentation.feet_position.x,
                                lifted.x,
                                delta.x,
                                runtime.client().entity_seam_crossing_count(entity_id),
                            ))
                        })
                })
                .flatten();
            let debug = DebugPaneStats {
                position: glam_vec3_from_vec3d(snapshot.eye),
                speed: camera.camera.speed_blocks_per_second as f32,
                movement_mode: format!(
                    "{}/{}",
                    camera.movement_mode_label(),
                    camera.collision_mode_label()
                ),
                on_ground: camera.on_ground,
                seed: self.active_world.scene.seed,
                generation_profile: self.active_world.scene.world_generation_profile.label(),
                runtime: runtime.stats(),
                render: self.active_world.render_stats,
                frame: context.frame_timing,
                pacing: context.pacing_debug,
                section_occlusion: render_options.section_occlusion_culling,
                force_fullbright: render_options.force_fullbright,
                color_profile: render_options.color_profile.label(),
                render_scale: context.render_scale,
                topology,
                topology_actor,
            }
            .overlay();
            let mut debug_overlay = debug.to_debug_overlay();
            if let Some(standby) = self.warm_world_standby_snapshot() {
                let mut warm_lines = vec![
                    format!(
                        "WARM {} S{} C{} M{}/{}",
                        standby.phase.label().to_ascii_uppercase(),
                        standby.seed,
                        standby.loaded_chunks,
                        standby.startup_seed_drawable_sections,
                        standby.startup_seed_sections,
                    ),
                    format!(
                        "GPU S{} I{} Q{} {}/{}",
                        standby.gpu_section_count,
                        standby.gpu_index_count,
                        standby.queued_upload_lifecycle_items,
                        standby.initial_upload_applied_lifecycle_items,
                        standby.initial_upload_lifecycle_items,
                    ),
                    format!(
                        "WARM {:.0}MS GPU {:.0} W{:.2}",
                        standby.elapsed_ms, standby.gpu_warm_ms, standby.worst_gpu_advance_ms,
                    ),
                ];
                if let Some(failure) = standby.failure {
                    warm_lines.push(format!("WARM FAIL {failure}"));
                }
                debug_overlay.lines.splice(0..0, warm_lines);
            }
            hud.debug = Some(FlatHudDebugOverlay::new(debug_overlay));
        }
        Some(hud)
    }

    pub(crate) fn current_mono_ui_render_state(&self) -> GameUiRenderState {
        let mut state = self.current_ui_render_state();
        let context = self.mono_ui_context.clone().unwrap_or_default();
        state.crosshair_visible = Some(self.crosshair_visible);
        state.frame_pacing_mode = match context.frame_pacing.mode {
            mclone_app_runtime::frame_pacing::FramePacingMode::Vsync => GameFramePacingMode::Vsync,
            mclone_app_runtime::frame_pacing::FramePacingMode::Capped => {
                GameFramePacingMode::Capped
            }
            mclone_app_runtime::frame_pacing::FramePacingMode::Uncapped => {
                GameFramePacingMode::Uncapped
            }
        };
        state.fps_cap = context.frame_pacing.fps_cap;
        state.touch_controls_mode = context.touch_controls_mode;
        state.touch_settings = context.touch_settings;
        state.server_cadence = self.active_world.runtime.as_ref().and_then(|runtime| {
            runtime
                .simulation_cadence()
                .map(|cadence| GameSimulationCadence {
                    host_rate_hz: cadence.host_rate_hz,
                    gameplay_rate_hz: cadence.gameplay_rate_hz,
                    physics_rate_hz: cadence.physics_rate_hz,
                })
        });
        state.turn_mode = None;
        state.xr_turn_mode = None;
        state
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

fn engine_camera_input_from_flat_frame(
    frame: FlatInputFrame,
    dt_seconds: f64,
) -> EngineCameraInput {
    EngineCameraInput {
        dt_seconds,
        mouse_delta_x: f64::from(frame.look_delta.x)
            + keyboard_turn_mouse_delta(frame.keyboard_turn, dt_seconds),
        mouse_delta_y: f64::from(frame.look_delta.y),
        forward: frame.forward,
        backward: frame.backward,
        left: frame.left,
        right: frame.right,
        jump: frame.jump,
        descend: frame.descend,
        shift: frame.sneak,
        sprint: frame.sprint,
        movement_impulse: frame
            .analog_movement
            .map(|movement| EngineCameraMovementImpulse::new(movement.left, movement.forward)),
        movement_yaw_radians: None,
        hand_push_emulation: true,
        thruster_emulation: true,
        ..EngineCameraInput::default()
    }
}

pub(crate) fn actor_figure_id_for_player_model(model: GamePlayerModel) -> ActorFigureId {
    match model {
        GamePlayerModel::Player => mclone_assets::DEFAULT_PLAYER_FIGURE_ID,
        GamePlayerModel::UprightBear => mclone_assets::UPRIGHT_BEAR_FIGURE_ID,
    }
}

fn mono_blink_config() -> TeleportConfig {
    TeleportConfig {
        max_distance: MONO_BLINK_MAX_DISTANCE,
        arc_height: MONO_BLINK_ARC_HEIGHT,
        ..TeleportConfig::default()
    }
}

fn mono_blink_intent(camera: &EngineCameraController) -> TeleportIntent {
    let pose = camera.player().pose();
    let snapshot = camera.snapshot();
    let forward = Vec3d::new(snapshot.yaw_radians.sin(), 0.0, snapshot.yaw_radians.cos());
    let right = Vec3d::new(snapshot.yaw_radians.cos(), 0.0, -snapshot.yaw_radians.sin());
    let aim_origin = pose
        .eye_position()
        .add(right.scale(-MONO_BLINK_ORIGIN_LEFT_OFFSET))
        .add(Vec3d::new(0.0, -MONO_BLINK_ORIGIN_DOWN_OFFSET, 0.0))
        .add(forward.scale(MONO_BLINK_ORIGIN_FORWARD_OFFSET));
    let pitch = (snapshot.pitch_radians + MONO_BLINK_UPWARD_PITCH_BIAS_RADIANS)
        .clamp(-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
    let aim_direction = Vec3d::new(
        snapshot.yaw_radians.sin() * pitch.cos(),
        pitch.sin(),
        snapshot.yaw_radians.cos() * pitch.cos(),
    );
    TeleportIntent::new(pose.position, aim_origin, aim_direction, pose.y_rot_degrees)
}

fn mono_blink_lines(preview: &TeleportPreview) -> Vec<WorldGuiLine> {
    let mut lines = Vec::new();
    let arc_color = if preview.is_valid() {
        MONO_BLINK_VALID_ARC_COLOR
    } else {
        MONO_BLINK_INVALID_ARC_COLOR
    };
    for points in preview.arc_points.windows(2) {
        lines.push(WorldGuiLine::new(
            glam_vec3_from_vec3d(points[0]),
            glam_vec3_from_vec3d(points[1]),
            arc_color,
        ));
    }
    if let Some(feet) = preview.target_feet {
        push_mono_blink_cross(
            &mut lines,
            feet,
            MONO_BLINK_MARKER_RADIUS,
            MONO_BLINK_FEET_COLOR,
        );
    }
    if let (Some(feet), Some(dot)) = (preview.target_feet, preview.marker_dot) {
        lines.push(WorldGuiLine::new(
            glam_vec3_from_vec3d(feet),
            glam_vec3_from_vec3d(dot),
            MONO_BLINK_DOT_COLOR,
        ));
        push_mono_blink_cross(&mut lines, dot, MONO_BLINK_DOT_RADIUS, MONO_BLINK_DOT_COLOR);
    }
    lines
}

fn push_mono_blink_cross(
    lines: &mut Vec<WorldGuiLine>,
    center: Vec3d,
    radius: f64,
    color: [f32; 4],
) {
    lines.push(WorldGuiLine::new(
        glam_vec3_from_vec3d(center.add(Vec3d::new(-radius, 0.0, 0.0))),
        glam_vec3_from_vec3d(center.add(Vec3d::new(radius, 0.0, 0.0))),
        color,
    ));
    lines.push(WorldGuiLine::new(
        glam_vec3_from_vec3d(center.add(Vec3d::new(0.0, 0.0, -radius))),
        glam_vec3_from_vec3d(center.add(Vec3d::new(0.0, 0.0, radius))),
        color,
    ));
}

#[cfg(test)]
mod tests {
    use mclone_input::{LookDelta, MovementImpulse};

    use super::*;

    #[test]
    fn flat_input_maps_to_shared_engine_camera_input() {
        let frame = FlatInputFrame {
            forward: true,
            right: true,
            jump: true,
            sprint: true,
            sneak: true,
            descend: true,
            keyboard_turn: 0.5,
            look_delta: LookDelta { x: 3.0, y: -2.0 },
            analog_movement: Some(MovementImpulse {
                left: -0.25,
                forward: 0.75,
            }),
            ..FlatInputFrame::default()
        };

        let input = engine_camera_input_from_flat_frame(frame, 0.25);

        assert_eq!(input.dt_seconds, 0.25);
        assert_eq!(
            input.mouse_delta_x,
            3.0 + keyboard_turn_mouse_delta(0.5, 0.25)
        );
        assert_eq!(input.mouse_delta_y, -2.0);
        assert!(input.forward);
        assert!(input.right);
        assert!(input.jump);
        assert!(input.sprint);
        assert!(input.shift);
        assert!(input.descend);
        assert_eq!(
            input.movement_impulse,
            Some(EngineCameraMovementImpulse::new(-0.25, 0.75))
        );
        assert!(input.hand_push_emulation);
        assert!(input.thruster_emulation);
    }

    #[test]
    fn mono_ui_context_defaults_to_keyboard_mouse_without_touch_controls() {
        let context = MonoUiContext::default();

        assert_eq!(
            context.resolved_input.preferred_prompt,
            Some(InputPromptKind::KeyboardMouse)
        );
        assert!(context.resolved_input.accepts_keyboard_mouse);
        assert!(!context.resolved_input.touch_controls_visible);
        assert!(!context.resolved_input.accepts_touch);
        assert_eq!(context.render_scale, 1.0);
    }

    #[test]
    fn flat_presentation_admits_one_two_and_four_with_one_shared_preparation() {
        for count in [1, 2, 4] {
            assert_eq!(
                admit_flat_presentation_views(count).unwrap(),
                FlatPresentationAdmission {
                    shared_preparation_count: 1,
                    rendered_view_count: count as u32,
                }
            );
        }
    }

    #[test]
    fn flat_presentation_rejects_zero_and_more_than_four_views() {
        assert!(
            admit_flat_presentation_views(0)
                .unwrap_err()
                .to_string()
                .contains("at least one")
        );
        assert!(
            admit_flat_presentation_views(5)
                .unwrap_err()
                .to_string()
                .contains("maximum is 4")
        );
    }
}
