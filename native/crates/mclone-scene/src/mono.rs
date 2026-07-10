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
        }
    }
}

#[derive(Clone, Debug)]
pub struct MonoSceneFrameSummary {
    pub render: FullFrameRenderSummary,
    pub timing: XrTerrainFrameTiming,
    pub upload: XrTerrainUploadSummary,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MonoWorldActionStatus {
    NoRuntime,
    NoTarget,
    NoCommand,
    Sent {
        target: BlockInteractionTarget,
        changed: bool,
    },
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
    pub preserve_pointer_state: bool,
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

impl<S> XrMcloneTerrainState<S>
where
    S: RemoteDedicatedServerSession,
{
    /// Select the conventional flat-client capability/UI profile. This is a
    /// topology choice on the shared host, not a platform-owned gameplay path.
    pub fn configure_mono_ui(&mut self, ui: GameUiHost, context: MonoUiContext) {
        self.ui = ui;
        self.mono_ui_context = Some(context);
        self.client_experience =
            ClientExperienceController::new(desktop_native_client_experience_profile());
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

    pub fn rebuild_mono_render_resources(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        actor_atlas: ActorTextureImage,
        actor_figures: &ActorFigureSet,
        asset_source: &impl AssetSource,
    ) -> Result<()> {
        let frame_metrics_visible = self.diagnostic_panel.frame_metrics_visible();
        let debug_diagnostics_visible = self.diagnostic_panel.debug_diagnostics_visible();
        self.draw = TexturedSectionDrawResources::new(
            device,
            queue,
            self.color_format,
            &[],
            self.mesh_assets.atlas.as_upload(),
        )
        .context("rebuild Mono terrain draw resources")?;
        self.actors = ActorDrawResources::new(
            device,
            queue,
            self.color_format,
            actor_atlas.as_upload(),
            Some(actor_figures),
        )
        .context("rebuild Mono actor draw resources")?;
        self.far_lod = FarTerrainLodRenderer::new(device, self.color_format);
        self.selection_outline = SelectionOutlineRenderer::new(device, self.color_format);
        self.world_gui_renderer = WorldGuiRenderer::new(device, self.color_format);
        self.world_gui_overlay_renderer = WorldGuiRenderer::new(device, self.color_format);
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
        self.screen_effects =
            ScreenEffectsRenderer::new(device, queue, self.color_format, asset_source)
                .context("rebuild Mono screen effects")?;
        self.traversal_ready_sections.clear();
        self.section_uploads.clear();
        self.render_stats = RenderStreamStats::default();
        if let Some(runtime) = &mut self.runtime {
            runtime.mark_all_render_sections_dirty_for_resource_rebuild();
        }
        Ok(())
    }

    pub fn has_runtime(&self) -> bool {
        self.runtime.is_some()
    }

    pub fn session_state(&self) -> &GameSessionState {
        self.session.state()
    }

    pub fn scene_options(&self) -> &XrSceneOptions {
        &self.scene
    }

    pub fn render_stats(&self) -> RenderStreamStats {
        self.render_stats
    }

    pub fn runtime_stats(&self) -> Option<SingleViewRuntimeStats> {
        self.runtime.as_ref().map(|runtime| runtime.stats())
    }

    pub fn runtime_poll_diagnostics(&self) -> Option<RuntimePollDiagnostics> {
        self.runtime
            .as_ref()
            .map(|runtime| runtime.last_poll_diagnostics())
    }

    pub fn current_render_distance(&self) -> u32 {
        self.runtime
            .as_ref()
            .map_or(self.scene.render_distance, |runtime| {
                runtime.render_distance()
            })
    }

    pub fn camera_frame_state(&self) -> EngineCameraFrameState {
        self.camera.frame_state(&self.interaction)
    }

    pub fn mono_render_view(&self, size: [u32; 2]) -> Result<ChunkRenderView> {
        render_pose_from_snapshot_with_view_mode(
            self.camera.snapshot(),
            self.camera.view_mode(),
            self.current_render_distance(),
        )
        .render_view(size[0].max(1), size[1].max(1))
    }

    pub fn apply_mono_movement_frame(&mut self, frame: FlatInputFrame, dt_seconds: f64) -> bool {
        let Some(runtime) = self.runtime.as_ref() else {
            return false;
        };
        let input = engine_camera_input_from_flat_frame(frame, dt_seconds);
        let before = self.camera.snapshot();
        let after = self.camera.apply_movement_input(runtime.client(), input);
        self.play_landing_events();
        after != before
    }

    pub fn apply_mono_look_frame(&mut self, frame: FlatInputFrame) -> bool {
        if frame.look_delta.x == 0.0 && frame.look_delta.y == 0.0 {
            return false;
        }
        self.camera
            .turn_mouse_delta(f64::from(frame.look_delta.x), f64::from(frame.look_delta.y));
        true
    }

    pub fn clear_mono_camera_input(&mut self) {
        self.camera.clear_keys();
    }

    pub fn begin_mono_blink_debug(&mut self) -> bool {
        if self.runtime.is_none() || self.travel_assist_mode != GameTravelAssistMode::Blink {
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
        if self.runtime.is_none() {
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
        let pitch_radians = self.camera.snapshot().pitch_radians;
        self.camera.set_player_feet_pose(
            target_feet,
            -preview.target_yaw_degrees.to_radians(),
            pitch_radians,
        );
        if let Some(runtime) = self.runtime.as_ref() {
            self.camera
                .probe_ground(runtime.client(), MONO_GROUND_PROBE_DISTANCE);
        }
        let changed = self.commit_mono_player_pose()?;
        Ok(MonoBlinkCommitStatus::Committed {
            target_feet,
            changed,
        })
    }

    fn ensure_mono_blink_worker(&mut self) -> bool {
        if self.blink_teleport_worker.is_some() {
            return true;
        }
        match NativeTeleportPreviewWorker::new() {
            Ok(worker) => {
                self.blink_teleport_worker = Some(worker);
                true
            }
            Err(error) => {
                log::warn!("failed to start Mono Blink preview worker: {error}");
                false
            }
        }
    }

    fn submit_mono_blink_request(&mut self) -> bool {
        let intent = mono_blink_intent(&self.camera);
        if self.mono_blink_debug.last_submitted_intent == Some(intent) {
            return false;
        }
        let (Some(runtime), Some(worker)) =
            (self.runtime.as_ref(), self.blink_teleport_worker.as_mut())
        else {
            return false;
        };
        match worker.submit_from_world(runtime.client(), intent, mono_blink_config()) {
            Ok(request_id) => {
                self.mono_blink_debug
                    .first_request_id
                    .get_or_insert(request_id);
                self.mono_blink_debug.latest_request_id = Some(request_id);
                self.mono_blink_debug.last_submitted_intent = Some(intent);
                true
            }
            Err(error) => {
                log::warn!("Mono Blink preview submit failed: {error}");
                self.blink_teleport_worker = None;
                false
            }
        }
    }

    fn poll_mono_blink_worker(&mut self) -> bool {
        let Some(worker) = self.blink_teleport_worker.as_mut() else {
            return false;
        };
        match worker.try_recv_latest() {
            Ok(Some(result)) => self.accept_mono_blink_result(result),
            Ok(None) => false,
            Err(error) => {
                log::warn!("Mono Blink preview worker failed: {error}");
                self.blink_teleport_worker = None;
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

    pub fn commit_mono_player_pose(&mut self) -> Result<bool> {
        if self.runtime.is_none() {
            return Ok(false);
        }
        self.commit_engine_camera_player_pose_timed()
            .map(|(changed, _)| changed)
    }

    pub fn toggle_mono_camera_view(&mut self) -> EngineCameraViewMode {
        self.camera.toggle_view_mode()
    }

    pub fn toggle_mono_movement_mode(&mut self) -> EngineCameraMovementMode {
        self.camera.toggle_movement_mode()
    }

    pub fn adjust_mono_camera_speed(&mut self, amount: f64) {
        self.camera.adjust_speed(amount);
    }

    pub fn mono_camera_speed_blocks_per_second(&self) -> f64 {
        self.camera.speed_blocks_per_second()
    }

    pub fn select_mono_hotbar_slot(&mut self, slot: u8) -> bool {
        self.interaction.select_hotbar_slot(slot)
    }

    pub fn step_mono_hotbar_slot(&mut self, step: i8) -> bool {
        if step == 0 {
            return false;
        }
        let selected = i16::from(self.interaction.selected_hotbar_slot());
        let next = (selected + i16::from(step)).rem_euclid(i16::from(FLAT_HOTBAR_SLOT_COUNT)) as u8;
        self.interaction.select_hotbar_slot(next)
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

    pub fn mono_ui_screen(&self) -> Option<GameScreen> {
        self.ui.screen()
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
        if self.local_startup.is_some() && !matches!(action, GameUiAction::Quit) {
            return Ok(MonoUiActionOutcome {
                preserve_pointer_state: true,
                ..MonoUiActionOutcome::default()
            });
        }

        if from_pointer_click
            && matches!(
                action,
                GameUiAction::StartWorld
                    | GameUiAction::Resume
                    | GameUiAction::JoinRemote
                    | GameUiAction::AssignHotbarBlock { .. }
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
        Ok(MonoUiActionOutcome {
            scene_replaced,
            session_start_requested: starts_session,
            clear_gameplay_input: true,
            preserve_pointer_state: matches!(
                action,
                GameUiAction::SetRenderDistance(_)
                    | GameUiAction::SetFlySpeed(_)
                    | GameUiAction::SetMovementSpeed(_)
                    | GameUiAction::SetCollisionMode(_)
                    | GameUiAction::SetTravelAssistMode(_)
                    | GameUiAction::SetTurnMode(_)
                    | GameUiAction::SetXrTurnMode(_)
                    | GameUiAction::SetTouchLookSensitivity(_)
                    | GameUiAction::SetTouchControlsMode(_)
                    | GameUiAction::SetServerSimulationCadence(_)
            ),
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
        if !apply_client_experience_settings_effects(self, host, effects.settings)? {
            return Ok(catalog_scene_replaced || session_scene_replaced);
        }
        for effect in effects.gameplay {
            match effect {
                ClientExperienceGameplayEffect::AssignHotbarBlock { slot, block_state } => {
                    self.assign_debug_hotbar_slot(slot, BlockStateId(block_state))?;
                }
            }
        }
        for effect in effects.projection {
            match effect {
                ClientExperienceProjectionEffect::ApplyUiAction(action) => {
                    self.ui.apply_action(action);
                }
            }
        }
        Ok(catalog_scene_replaced || session_scene_replaced)
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
        if self.runtime.is_none() {
            return Ok(false);
        }
        self.commit_mono_player_pose()?;
        self.runtime
            .as_mut()
            .expect("runtime presence checked")
            .send_gameplay_command(mclone_protocol::ClientCommand::ShootDebugPhysicsCube)
            .map_err(Into::into)
    }

    pub fn handle_mono_world_action(
        &mut self,
        action: FlatInputAction,
    ) -> Result<MonoWorldActionStatus> {
        if self.runtime.is_none() {
            return Ok(MonoWorldActionStatus::NoRuntime);
        }
        self.commit_mono_player_pose()?;
        if let Some(command) = self.interaction.ensure_has_sent_carried_item() {
            self.runtime
                .as_mut()
                .expect("runtime presence checked")
                .send_gameplay_command(command)?;
        }
        let Some(target) = self.current_mono_block_target() else {
            return Ok(MonoWorldActionStatus::NoTarget);
        };
        let command = match action {
            FlatInputAction::Attack => self.interaction.debug_instant_break_command(target.hit),
            FlatInputAction::Use => self.interaction.use_item_on_command(target.hit),
            _ => None,
        };
        let Some(command) = command else {
            return Ok(MonoWorldActionStatus::NoCommand);
        };
        let changed = self
            .runtime
            .as_mut()
            .expect("runtime presence checked")
            .send_gameplay_command(command)?;
        Ok(MonoWorldActionStatus::Sent { target, changed })
    }

    fn current_mono_block_target(&self) -> Option<BlockInteractionTarget> {
        let runtime = self.runtime.as_ref()?;
        self.camera
            .target_block(runtime.client(), &self.interaction)
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
    ) -> Result<MonoSceneFrameSummary> {
        let RenderFrameContext {
            device,
            queue,
            encoder,
            target,
        } = frame;
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

        // Frame-input assembly reuses the shared accessors. The shared
        // `render_full_frame_for_view*` entry applies sky-darken and underwater
        // fog internally, so pass the base effective options plus the overlay.
        let render_options = self.effective_render_options(center_position);
        let sky_clear_color = self.sky_clear_color();
        let time_of_day = self.time_of_day();
        let sun_angle = self.sun_angle();
        let underwater_overlay = self.mono_underwater_overlay(render_view);
        let actor_instances = self.current_actor_instances();

        let gui_scale = GuiScale::from_pixels(target.size[0], target.size[1]);
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
        let world_gui = FullFrameGui::new(false, full_frame_gui.covers_world, full_frame_gui.scale);
        let mut summary = render_full_frame_for_view_with_far_lod(
            RenderFrameContext::new(device, queue, encoder, target),
            depth,
            &self.sky,
            &mut self.draw,
            far_lod,
            far_lod_mesh,
            Some(&mut self.actors),
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
        .context("render mono scene frame")?;

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
                &self.camera,
                EngineDebugVisualOptions::new(self.player_collision_box_visible),
            );
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

        self.render_stats = render_stats;
        self.rendered_frames = self.rendered_frames.wrapping_add(1);
        Ok(MonoSceneFrameSummary {
            render: summary,
            timing,
            upload,
        })
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

    fn mono_flat_hud(&self, menu_active: bool) -> Option<FlatHud> {
        let runtime = self.runtime.as_ref()?;
        let context = self.mono_ui_context.clone().unwrap_or_default();
        let palette_active = self.ui.screen() == Some(GameScreen::BlockPalette);
        let mut hud = FlatHud::new(context.resolved_input);
        hud.world_hud_visible = !menu_active || palette_active;
        hud.crosshair_visible = self.crosshair_visible && !menu_active;
        hud.hotbar = FlatHotbarOverlay::selected_with_icons(
            self.interaction.selected_hotbar_slot(),
            debug_hotbar_icons(
                self.interaction.hotbar_items(),
                &runtime.mesh_assets().catalog,
            ),
        );
        hud.status = self.session_projection().status_overlay;
        hud.frame_pipeline = self.diagnostic_panel.frame_metrics_overlay();
        if self.diagnostic_panel.debug_diagnostics_visible() && !menu_active {
            let camera = self.camera.frame_state(&self.interaction);
            let snapshot = self.camera.snapshot();
            let render_options = self.effective_render_options(glam_vec3_from_vec3d(snapshot.eye));
            hud.debug = Some(
                DebugPaneStats {
                    position: glam_vec3_from_vec3d(snapshot.eye),
                    speed: camera.camera.speed_blocks_per_second as f32,
                    movement_mode: format!(
                        "{}/{}",
                        camera.movement_mode_label(),
                        camera.collision_mode_label()
                    ),
                    on_ground: camera.on_ground,
                    seed: self.scene.seed,
                    runtime: runtime.stats(),
                    render: self.render_stats,
                    frame: context.frame_timing,
                    pacing: context.pacing_debug,
                    section_occlusion: render_options.section_occlusion_culling,
                    force_fullbright: render_options.force_fullbright,
                    color_profile: render_options.color_profile.label(),
                    render_scale: context.render_scale,
                }
                .hud_debug_overlay(),
            );
        }
        Some(hud)
    }

    fn current_mono_ui_render_state(&self) -> GameUiRenderState {
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
        state.server_cadence = self.runtime.as_ref().and_then(|runtime| {
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
}
