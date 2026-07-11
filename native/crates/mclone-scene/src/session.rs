use super::*;

#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::DEFAULT_STARTUP_READINESS_TIMEOUT;
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::session::SessionStorageIntent;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) trait SceneSessionRuntimeFactory {
    fn start(
        &mut self,
        endpoint: RemoteSessionEndpoint,
        scene: McloneSceneHostOptions,
        mesh_assets: TexturedMeshAssets,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        movement_speed_multiplier: f32,
        clock: &MonotonicClockHandle,
    ) -> Result<StartedSceneRuntime>;
}

#[cfg(not(target_arch = "wasm32"))]
struct NativeSceneSessionRuntimeFactory<F, S> {
    factory: F,
    session: std::marker::PhantomData<fn() -> S>,
}

#[cfg(not(target_arch = "wasm32"))]
impl<F, S> SceneSessionRuntimeFactory for NativeSceneSessionRuntimeFactory<F, S>
where
    F: FnMut(
        RemoteSessionEndpoint,
        McloneSceneHostOptions,
        TexturedMeshAssets,
    ) -> Result<NativeSessionServices<S>>,
    S: RemoteDedicatedServerSession + 'static,
{
    fn start(
        &mut self,
        endpoint: RemoteSessionEndpoint,
        scene: McloneSceneHostOptions,
        mesh_assets: TexturedMeshAssets,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        movement_speed_multiplier: f32,
        clock: &MonotonicClockHandle,
    ) -> Result<StartedSceneRuntime> {
        let runtime = (self.factory)(endpoint, scene, mesh_assets)?;
        start_scene_runtime(
            device,
            queue,
            color_format,
            runtime,
            movement_speed_multiplier,
            None,
            clock,
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct StartedSceneRuntime {
    runtime: SceneSessionRuntime,
    camera: EngineCameraController,
    draw: TexturedSectionDrawResources,
    render_stats: RenderStreamStats,
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct SceneLocalStartup {
    pub(super) request: SessionStartRequest,
    pub(super) descriptor: Option<ActiveSessionDescriptor>,
    pub(super) scene: McloneSceneHostOptions,
    pub(super) pump: LocalIntegratedStartupPump,
    pub(super) camera: EngineCameraController,
    pub(super) startup_view_pose: Option<XrStartupViewPose>,
}

#[cfg(target_arch = "wasm32")]
pub(crate) struct SceneLocalStartup;

#[cfg(not(target_arch = "wasm32"))]
impl SceneLocalStartup {
    pub(crate) fn replace_camera(&mut self, camera: EngineCameraController) {
        self.camera = camera;
    }

    pub(crate) fn render_distance(&self, _fallback: u32) -> u32 {
        self.scene.render_distance
    }

    pub(crate) fn mesh_catalog(&self) -> Option<&mclone_mesh::TexturedMeshCatalog> {
        Some(&self.pump.runtime().mesh_assets().catalog)
    }

    pub(crate) fn progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        self.pump.progress_overlay()
    }
}

#[cfg(target_arch = "wasm32")]
impl SceneLocalStartup {
    pub(crate) fn replace_camera(&mut self, _camera: EngineCameraController) {}

    pub(crate) fn render_distance(&self, fallback: u32) -> u32 {
        fallback
    }

    pub(crate) fn mesh_catalog(&self) -> Option<&mclone_mesh::TexturedMeshCatalog> {
        None
    }

    pub(crate) fn progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        None
    }
}

pub(crate) type ScenePendingSessionStart = SessionStartPayload<McloneSceneHostOptions>;

/// Platform-neutral pending session start handed to an asynchronous driver.
///
/// Native hosts normally consume this payload through their synchronous
/// runtime factory. Browser hosts take it after shared UI/catalog policy has
/// classified the request, construct the Worker or WebSocket runtime, and
/// complete it back into the same scene host.
#[derive(Clone, Debug, PartialEq)]
pub struct ExternalSceneSessionStart {
    pub request: SessionStartRequest,
    pub runtime_kind: SessionRuntimeKind,
    pub scene: McloneSceneHostOptions,
    pub descriptor: ActiveSessionDescriptor,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SceneSessionStartOutcome {
    LocalStartupQueued,
    Started(ActiveSessionDescriptor),
}

impl McloneSceneHost {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        scene: McloneSceneHostOptions,
        render_options: TexturedSectionRenderOptions,
        mesh_assets: TexturedMeshAssets,
        actor_atlas: ActorTextureImage,
        actor_figures: ActorFigureSet,
        asset_source: &impl AssetSource,
        startup_view_pose: Option<XrStartupViewPose>,
    ) -> Result<Self> {
        let scene = scene.validated()?;
        Self::start_local_async(
            device,
            queue,
            color_format,
            system_monotonic_clock(),
            scene,
            render_options,
            mesh_assets,
            actor_atlas,
            actor_figures,
            asset_source,
            startup_view_pose,
        )
    }
}

impl McloneSceneHost {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn start_local_async(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        clock: MonotonicClockHandle,
        scene: McloneSceneHostOptions,
        render_options: TexturedSectionRenderOptions,
        mesh_assets: TexturedMeshAssets,
        actor_atlas: ActorTextureImage,
        actor_figures: ActorFigureSet,
        asset_source: &impl AssetSource,
        startup_view_pose: Option<XrStartupViewPose>,
    ) -> Result<Self> {
        let scene = scene.validated()?;
        let active_assets = PreparedSceneAssets::startup(
            0,
            mesh_assets.clone(),
            ActorTextureAssets {
                atlas: actor_atlas.clone(),
                figures: actor_figures.clone(),
            },
            load_screen_effect_texture_assets(asset_source)
                .context("prepare initial scene screen-effect assets")?,
            PreparedAudioAssets::load(asset_source)
                .context("prepare initial scene audio assets")?,
        );
        let request = SessionStartRequest::new_seed_local_world(scene.seed);
        let descriptor = request.active_descriptor();
        let catalog_operations = scene
            .world_root
            .clone()
            .map(native_world_catalog_operations);
        let mut camera = EngineCameraController::spawn_for_chunk(scene.center());
        camera.set_movement_speed_multiplier(f64::from(scene.movement_speed_multiplier));
        camera.set_first_person_player_visible(scene.first_person_player_visible);
        if let Some(view_pose) = startup_view_pose {
            apply_xr_startup_view_pose(&mut camera, view_pose.position, view_pose.yaw_degrees)
                .context("apply initial XR local startup view pose")?;
        }
        let draw = TexturedSectionDrawResources::new(
            device,
            queue,
            color_format,
            &[],
            mesh_assets.atlas.as_upload(),
        )
        .context("initialize empty XR terrain draw resources")?;
        let mut world_gui_renderer = WorldGuiRenderer::new(device, color_format);
        world_gui_renderer
            .upload_texture_atlas(device, queue, mesh_assets.atlas.as_upload())
            .context("upload initial XR GUI atlas")?;
        let pump = LocalIntegratedStartupPump::with_mesh_assets(
            local_integrated_scene_options(&scene),
            mesh_assets.clone(),
        )
        .context("create initial XR local world startup pump")?;
        let ui = xr_game_ui_for_session(None, scene.seed);
        let mut session = GameSessionCoordinator::new();
        session.begin_start(request.clone());
        let mut state = Self {
            scene: scene.clone(),
            services: SceneHostServices {
                clock,
                catalog_operations,
                teleport_preview: TeleportPreviewCapability::Unavailable,
                audio: AudioOutputCapability::Unavailable,
            },
            color_format,
            mesh_assets,
            active_assets,
            asset_replacement: None,
            asset_replacement_status: AssetReplacementStatus::Active { epoch: 0 },
            last_asset_replacement_commit: None,
            asset_replacement_started_at: None,
            asset_replacement_assets_ready_at: None,
            asset_pack_sources: None,
            asset_pack_preference: AssetPackPreference::default(),
            asset_pack_preference_storage: None,
            asset_pack_preference_error: None,
            pending_restored_asset_pack_selection: None,
            external_asset_pack_preparation: false,
            pending_external_asset_pack_selection: None,
            runtime: None,
            local_startup: Some(SceneLocalStartup {
                request: request.clone(),
                descriptor,
                scene: scene.clone(),
                pump,
                camera: camera.clone(),
                startup_view_pose,
            }),
            session,
            session_runtime_factory: None,
            client_experience: ClientExperienceController::new(xr_client_experience_profile()),
            camera,
            interaction: ClientInteractionController::new(),
            initial_alignment_mode: if startup_view_pose.is_some() {
                XrViewAlignmentMode::ViewPose
            } else {
                XrViewAlignmentMode::PlayerSpawn
            },
            render_options,
            player_collision_box_visible: false,
            crosshair_visible: true,
            travel_assist_mode: GameTravelAssistMode::Off,
            player_model: GamePlayerModel::default(),
            draw,
            traversal_ready_sections: TraversalReadySectionCache::default(),
            section_uploads: RenderSectionUploadCoordinator::default(),
            actors: ActorDrawResources::new(
                device,
                queue,
                color_format,
                actor_atlas.as_upload(),
                Some(&actor_figures),
            )
            .context("initialize XR terrain actor draw resources")?,
            far_lod: FarTerrainLodRenderer::new(device, color_format),
            selection_outline: SelectionOutlineRenderer::new(device, color_format),
            world_gui_renderer,
            world_gui_overlay_renderer: WorldGuiRenderer::new(device, color_format),
            mono_gui: None,
            mono_ui_context: None,
            diagnostic_panel: XrDiagnosticPanel::new(device, color_format),
            ui,
            menu_overlay_cache: XrMenuPanelOverlayCache::default(),
            status_overlay: StatusOverlay::hidden(),
            sky: SkyRenderer::new_with_color_profile(
                device,
                color_format,
                render_options.color_profile,
            ),
            screen_effects: ScreenEffectsRenderer::new(device, queue, color_format, asset_source)
                .context("initialize XR screen effects renderer")?,
            underwater_effects: XrUnderwaterEffectStates::default(),
            last_underwater_update: None,
            head_comfort: XrHeadComfortState::default(),
            render_stats: RenderStreamStats::default(),
            tracking_origin: None,
            locomotion_mode: XrLocomotionMode::default(),
            turn_policy: XrTurnPolicy::default(),
            snap_turn_state: XrSnapTurnState::default(),
            blink_teleport: XrBlinkTeleportState::default(),
            mono_blink_debug: MonoBlinkDebugState::default(),
            display_refresh_hz: None,
            render_admission_policy: RenderAdmissionPolicy::new(
                FrameHostKind::HeadlessOffscreenPerf,
                WorkWindow::BeforeRender,
            ),
            render_split_timing_enabled: false,
            defer_eye_waits_enabled: false,
            overlap_runtime_prefetch_enabled: false,
            prefetched_live_upload: None,
            render_section_upload_budget: None,
            render_section_accept_budget: None,
            render_completed_result_accept_budget: None,
            per_view_uniform_frame: 0,
            last_locomotion_update: None,
            menu_toggle_down: false,
            game_ui_toggle_down: false,
            menu_pointer_down: false,
            gameplay_interaction_buttons: XrGameplayInteractionButtons::default(),
            menu_panel_pose: None,
            menu_panel_anchor: XrUiPanelAnchor::Head,
            menu_panel_recenter_pending: true,
            latest_controllers: Vec::new(),
            first_eye_summary: None,
            last_ui_panel_stats: WorldGuiPanelRenderStats::default(),
            last_ui_draw_cache_stats: UiDrawCacheStats::default(),
            rendered_frames: 0,
            seed_reroll: NewWorldSeedReroll::new(scene.seed),
        };
        state.refresh_world_catalog_ui(WorldCatalogUiStatus::hidden());
        Ok(state)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_runtime<S>(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        clock: MonotonicClockHandle,
        scene: McloneSceneHostOptions,
        runtime: NativeSessionServices<S>,
        render_options: TexturedSectionRenderOptions,
        actor_atlas: ActorTextureImage,
        actor_figures: ActorFigureSet,
        asset_source: &impl AssetSource,
        startup_view_pose: Option<XrStartupViewPose>,
    ) -> Result<Self>
    where
        S: RemoteDedicatedServerSession + 'static,
    {
        let scene = scene.validated()?;
        let catalog_operations = scene
            .world_root
            .clone()
            .map(native_world_catalog_operations);
        let started = start_scene_runtime(
            device,
            queue,
            color_format,
            runtime,
            scene.movement_speed_multiplier,
            startup_view_pose,
            &clock,
        )?;
        let active_session = started
            .runtime
            .active_session()
            .cloned()
            .context("XR runtime did not expose an active session")?;
        let ui = xr_game_ui_for_session(Some(&active_session), scene.seed);
        let mut session = GameSessionCoordinator::new();
        session.complete_start(active_session);
        let mesh_assets = started.runtime.mesh_assets().clone();
        let active_assets = PreparedSceneAssets::startup(
            0,
            mesh_assets.clone(),
            ActorTextureAssets {
                atlas: actor_atlas.clone(),
                figures: actor_figures.clone(),
            },
            load_screen_effect_texture_assets(asset_source)
                .context("prepare initial scene screen-effect assets")?,
            PreparedAudioAssets::load(asset_source)
                .context("prepare initial scene audio assets")?,
        );
        let mut world_gui_renderer = WorldGuiRenderer::new(device, color_format);
        world_gui_renderer
            .upload_texture_atlas(device, queue, mesh_assets.atlas.as_upload())
            .context("upload XR GUI atlas")?;
        let mut state = Self {
            scene: scene.clone(),
            services: SceneHostServices {
                clock,
                catalog_operations,
                teleport_preview: TeleportPreviewCapability::Unavailable,
                audio: AudioOutputCapability::Unavailable,
            },
            color_format,
            mesh_assets,
            active_assets,
            asset_replacement: None,
            asset_replacement_status: AssetReplacementStatus::Active { epoch: 0 },
            last_asset_replacement_commit: None,
            asset_replacement_started_at: None,
            asset_replacement_assets_ready_at: None,
            asset_pack_sources: None,
            asset_pack_preference: AssetPackPreference::default(),
            asset_pack_preference_storage: None,
            asset_pack_preference_error: None,
            pending_restored_asset_pack_selection: None,
            external_asset_pack_preparation: false,
            pending_external_asset_pack_selection: None,
            runtime: Some(started.runtime),
            local_startup: None,
            session,
            session_runtime_factory: None,
            client_experience: ClientExperienceController::new(xr_client_experience_profile()),
            camera: started.camera,
            interaction: ClientInteractionController::new(),
            initial_alignment_mode: if startup_view_pose.is_some() {
                XrViewAlignmentMode::ViewPose
            } else {
                XrViewAlignmentMode::PlayerSpawn
            },
            render_options,
            player_collision_box_visible: false,
            crosshair_visible: true,
            travel_assist_mode: GameTravelAssistMode::Off,
            player_model: GamePlayerModel::default(),
            draw: started.draw,
            traversal_ready_sections: TraversalReadySectionCache::default(),
            section_uploads: RenderSectionUploadCoordinator::default(),
            actors: ActorDrawResources::new(
                device,
                queue,
                color_format,
                actor_atlas.as_upload(),
                Some(&actor_figures),
            )
            .context("initialize XR terrain actor draw resources")?,
            far_lod: FarTerrainLodRenderer::new(device, color_format),
            selection_outline: SelectionOutlineRenderer::new(device, color_format),
            world_gui_renderer,
            world_gui_overlay_renderer: WorldGuiRenderer::new(device, color_format),
            mono_gui: None,
            mono_ui_context: None,
            diagnostic_panel: XrDiagnosticPanel::new(device, color_format),
            ui,
            menu_overlay_cache: XrMenuPanelOverlayCache::default(),
            status_overlay: StatusOverlay::hidden(),
            sky: SkyRenderer::new_with_color_profile(
                device,
                color_format,
                render_options.color_profile,
            ),
            screen_effects: ScreenEffectsRenderer::new(device, queue, color_format, asset_source)
                .context("initialize XR screen effects renderer")?,
            underwater_effects: XrUnderwaterEffectStates::default(),
            last_underwater_update: None,
            head_comfort: XrHeadComfortState::default(),
            render_stats: started.render_stats,
            tracking_origin: None,
            locomotion_mode: XrLocomotionMode::default(),
            turn_policy: XrTurnPolicy::default(),
            snap_turn_state: XrSnapTurnState::default(),
            blink_teleport: XrBlinkTeleportState::default(),
            mono_blink_debug: MonoBlinkDebugState::default(),
            display_refresh_hz: None,
            render_admission_policy: RenderAdmissionPolicy::new(
                FrameHostKind::HeadlessOffscreenPerf,
                WorkWindow::BeforeRender,
            ),
            render_split_timing_enabled: false,
            defer_eye_waits_enabled: false,
            overlap_runtime_prefetch_enabled: false,
            prefetched_live_upload: None,
            render_section_upload_budget: None,
            render_section_accept_budget: None,
            render_completed_result_accept_budget: None,
            per_view_uniform_frame: 0,
            last_locomotion_update: None,
            menu_toggle_down: false,
            game_ui_toggle_down: false,
            menu_pointer_down: false,
            gameplay_interaction_buttons: XrGameplayInteractionButtons::default(),
            menu_panel_pose: None,
            menu_panel_anchor: XrUiPanelAnchor::Head,
            menu_panel_recenter_pending: false,
            latest_controllers: Vec::new(),
            first_eye_summary: None,
            last_ui_panel_stats: WorldGuiPanelRenderStats::default(),
            last_ui_draw_cache_stats: UiDrawCacheStats::default(),
            rendered_frames: 0,
            seed_reroll: NewWorldSeedReroll::new(scene.seed),
        };
        state
            .camera
            .set_first_person_player_visible(scene.first_person_player_visible);
        state.refresh_world_catalog_ui(WorldCatalogUiStatus::hidden());
        state.apply_debug_ui_screen();
        Ok(state)
    }

    /// Construct the shared scene around an already-started, platform-neutral
    /// runtime. Browser assembly uses this seam after its worker/socket promise
    /// has completed; native callers may continue to use the startup pumps above.
    /// The driver still owns the surface/canvas and presentation cadence.
    #[allow(clippy::too_many_arguments)]
    pub fn with_scene_runtime(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        clock: MonotonicClockHandle,
        scene: McloneSceneHostOptions,
        runtime: SceneSessionRuntime,
        render_options: TexturedSectionRenderOptions,
        active_assets: PreparedSceneAssets,
        client_experience_profile: ClientExperienceProfile,
        catalog_operations: Option<WorldCatalogOperationService>,
        startup_view_pose: Option<XrStartupViewPose>,
    ) -> Result<Self> {
        let scene = scene.validated()?;
        let active_session = runtime
            .active_session()
            .cloned()
            .context("scene runtime did not expose an active session")?;
        if active_assets.mesh.catalog.len() != runtime.mesh_assets().catalog.len() {
            bail!(
                "prepared scene/runtime terrain catalogs disagree: prepared={} runtime={}",
                active_assets.mesh.catalog.len(),
                runtime.mesh_assets().catalog.len()
            );
        }
        let mesh_assets = active_assets.mesh.clone();
        let mut camera = EngineCameraController::spawn_for_chunk(runtime.interest_center());
        camera.set_movement_speed_multiplier(f64::from(scene.movement_speed_multiplier));
        camera.set_first_person_player_visible(scene.first_person_player_visible);
        if let Some(view_pose) = startup_view_pose {
            apply_xr_startup_view_pose(&mut camera, view_pose.position, view_pose.yaw_degrees)
                .context("apply initial scene startup view pose")?;
        }

        let draw = TexturedSectionDrawResources::new(
            device,
            queue,
            color_format,
            &[],
            mesh_assets.atlas.as_upload(),
        )
        .context("initialize empty scene terrain draw resources")?;
        let mut world_gui_renderer = WorldGuiRenderer::new(device, color_format);
        world_gui_renderer
            .upload_texture_atlas(device, queue, mesh_assets.atlas.as_upload())
            .context("upload initial scene GUI atlas")?;
        let actors = ActorDrawResources::new(
            device,
            queue,
            color_format,
            active_assets.actors.atlas.as_upload(),
            Some(&active_assets.actors.figures),
        )
        .context("initialize scene actor draw resources")?;
        let screen_effects = ScreenEffectsRenderer::new_with_assets(
            device,
            queue,
            color_format,
            &active_assets.screen_effects,
        )
        .context("initialize scene screen effects")?;
        let ui = xr_game_ui_for_session(Some(&active_session), scene.seed);
        let mut session = GameSessionCoordinator::new();
        session.complete_start(active_session);
        let mut state = Self {
            scene: scene.clone(),
            services: SceneHostServices {
                clock,
                catalog_operations,
                teleport_preview: TeleportPreviewCapability::Unavailable,
                audio: AudioOutputCapability::Unavailable,
            },
            color_format,
            mesh_assets,
            asset_replacement_status: AssetReplacementStatus::Active {
                epoch: active_assets.epoch,
            },
            active_assets,
            asset_replacement: None,
            last_asset_replacement_commit: None,
            asset_replacement_started_at: None,
            asset_replacement_assets_ready_at: None,
            asset_pack_sources: None,
            asset_pack_preference: AssetPackPreference::default(),
            asset_pack_preference_storage: None,
            asset_pack_preference_error: None,
            pending_restored_asset_pack_selection: None,
            external_asset_pack_preparation: false,
            pending_external_asset_pack_selection: None,
            runtime: Some(runtime),
            local_startup: None,
            session,
            #[cfg(not(target_arch = "wasm32"))]
            session_runtime_factory: None,
            client_experience: ClientExperienceController::new(client_experience_profile),
            camera,
            interaction: ClientInteractionController::new(),
            initial_alignment_mode: if startup_view_pose.is_some() {
                XrViewAlignmentMode::ViewPose
            } else {
                XrViewAlignmentMode::PlayerSpawn
            },
            render_options,
            player_collision_box_visible: false,
            crosshair_visible: true,
            travel_assist_mode: GameTravelAssistMode::Off,
            player_model: GamePlayerModel::default(),
            draw,
            traversal_ready_sections: TraversalReadySectionCache::default(),
            section_uploads: RenderSectionUploadCoordinator::default(),
            actors,
            far_lod: FarTerrainLodRenderer::new(device, color_format),
            selection_outline: SelectionOutlineRenderer::new(device, color_format),
            world_gui_renderer,
            world_gui_overlay_renderer: WorldGuiRenderer::new(device, color_format),
            mono_gui: None,
            mono_ui_context: None,
            diagnostic_panel: XrDiagnosticPanel::new(device, color_format),
            ui,
            menu_overlay_cache: XrMenuPanelOverlayCache::default(),
            status_overlay: StatusOverlay::hidden(),
            sky: SkyRenderer::new_with_color_profile(
                device,
                color_format,
                render_options.color_profile,
            ),
            screen_effects,
            underwater_effects: XrUnderwaterEffectStates::default(),
            last_underwater_update: None,
            head_comfort: XrHeadComfortState::default(),
            render_stats: RenderStreamStats::default(),
            tracking_origin: None,
            locomotion_mode: XrLocomotionMode::default(),
            turn_policy: XrTurnPolicy::default(),
            snap_turn_state: XrSnapTurnState::default(),
            blink_teleport: XrBlinkTeleportState::default(),
            mono_blink_debug: MonoBlinkDebugState::default(),
            display_refresh_hz: None,
            render_admission_policy: RenderAdmissionPolicy::new(
                FrameHostKind::HeadlessOffscreenPerf,
                WorkWindow::BeforeRender,
            ),
            render_split_timing_enabled: false,
            defer_eye_waits_enabled: false,
            overlap_runtime_prefetch_enabled: false,
            prefetched_live_upload: None,
            render_section_upload_budget: None,
            render_section_accept_budget: None,
            render_completed_result_accept_budget: None,
            per_view_uniform_frame: 0,
            last_locomotion_update: None,
            menu_toggle_down: false,
            game_ui_toggle_down: false,
            menu_pointer_down: false,
            gameplay_interaction_buttons: XrGameplayInteractionButtons::default(),
            menu_panel_pose: None,
            menu_panel_anchor: XrUiPanelAnchor::Head,
            menu_panel_recenter_pending: false,
            latest_controllers: Vec::new(),
            first_eye_summary: None,
            last_ui_panel_stats: WorldGuiPanelRenderStats::default(),
            last_ui_draw_cache_stats: UiDrawCacheStats::default(),
            rendered_frames: 0,
            seed_reroll: NewWorldSeedReroll::new(scene.seed),
        };
        state.refresh_world_catalog_ui(WorldCatalogUiStatus::hidden());
        state.apply_debug_ui_screen();
        Ok(state)
    }

    pub fn set_audio_output(&mut self, audio: AudioOutputCapability) {
        self.services.audio = audio;
    }

    pub fn set_teleport_preview_capability(&mut self, capability: TeleportPreviewCapability) {
        self.services.teleport_preview = capability;
    }

    pub fn set_frame_pipeline_report(&mut self, report: Arc<FramePipelineReport>, revision: u64) {
        self.render_admission_policy
            .set_frame_pipeline_report(report.clone());
        self.diagnostic_panel
            .set_frame_pipeline_report(report, revision);
    }

    pub fn clear_frame_pipeline_report(&mut self) {
        self.render_admission_policy.clear_frame_pipeline_report();
        self.diagnostic_panel.clear_frame_pipeline_report();
    }

    /// Whether the frame-pipeline (perf) overlay is currently toggled on. Hosts
    /// use this to feed the overlay only while it is visible, instead of gating
    /// the report at compile time behind `perf-diagnostics`.
    pub fn frame_metrics_visible(&self) -> bool {
        self.diagnostic_panel.frame_metrics_visible()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_session_runtime_factory<F, S>(&mut self, factory: F)
    where
        F: FnMut(
                RemoteSessionEndpoint,
                McloneSceneHostOptions,
                TexturedMeshAssets,
            ) -> Result<NativeSessionServices<S>>
            + 'static,
        S: RemoteDedicatedServerSession + 'static,
    {
        self.session_runtime_factory = Some(Box::new(NativeSceneSessionRuntimeFactory {
            factory,
            session: std::marker::PhantomData,
        }));
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn start_session_for_request(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        request: SessionStartRequest,
    ) -> Result<bool> {
        self.request_session_start(request)?;
        self.start_pending_session(device, queue)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn start_session_for_request(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        request: SessionStartRequest,
    ) -> Result<bool> {
        self.request_external_session_start(request)?;
        Ok(false)
    }

    /// Queue a session request for an asynchronous platform runtime builder.
    /// Request classification remains shared; the driver receives only the
    /// resolved runtime kind, scene options, and active descriptor.
    pub fn request_external_session_start(&mut self, request: SessionStartRequest) -> Result<()> {
        let plan = plan_session_start(
            request,
            |options| {
                let mut scene = self.scene.clone();
                scene.seed = options.seed;
                scene.world_dir = None;
                Ok::<_, anyhow::Error>(scene)
            },
            |id| {
                let summary = self
                    .client_experience
                    .catalog()
                    .world_summary(id)
                    .cloned()
                    .context("local world is not present in the catalog view")?;
                let mut scene = self.scene.clone();
                scene.seed = summary.seed;
                scene.world_dir = None;
                Ok((
                    scene,
                    ActiveSessionDescriptor::from_local_world_summary(&summary),
                ))
            },
            |endpoint| {
                let mut scene = self.scene.clone();
                scene.world_dir = None;
                let _ = endpoint;
                Ok(scene)
            },
            || anyhow!("unsupported unknown scene session start"),
        )?;
        self.status_overlay = StatusOverlay::hidden();
        self.session.request_start(plan.request, plan.payload);
        Ok(())
    }

    /// Take the next shared-policy session request for asynchronous platform
    /// construction. A taken request remains in the coordinator's Starting
    /// state until completion or failure is submitted.
    pub fn take_external_session_start(&mut self) -> Option<ExternalSceneSessionStart> {
        let pending = self.session.take_pending_start()?;
        Some(ExternalSceneSessionStart {
            request: pending.request,
            runtime_kind: pending.payload.runtime_kind,
            scene: pending.payload.options,
            descriptor: pending.payload.descriptor,
        })
    }

    pub fn external_session_start_snapshot(&self) -> Option<ExternalSceneSessionStart> {
        let pending = self.session.pending_start()?;
        Some(ExternalSceneSessionStart {
            request: pending.request.clone(),
            runtime_kind: pending.payload.runtime_kind,
            scene: pending.payload.options.clone(),
            descriptor: pending.payload.descriptor.clone(),
        })
    }

    /// Install an asynchronously constructed neutral runtime without moving
    /// session, render, UI, or camera policy into the platform adapter.
    pub fn complete_external_session_start(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pending: ExternalSceneSessionStart,
        runtime: SceneSessionRuntime,
    ) -> Result<()> {
        let active = runtime
            .active_session()
            .cloned()
            .context("external scene runtime has no active descriptor")?;
        if active != pending.descriptor {
            bail!(
                "external scene runtime descriptor mismatch: expected {:?}, got {:?}",
                pending.descriptor,
                active
            );
        }
        if runtime.mesh_assets().catalog.len() != self.mesh_assets.catalog.len() {
            bail!(
                "external scene runtime catalog mismatch: active={} replacement={}",
                self.mesh_assets.catalog.len(),
                runtime.mesh_assets().catalog.len()
            );
        }

        let movement_speed_multiplier = self.camera.movement_speed_multiplier();
        let mut camera = EngineCameraController::spawn_for_chunk(runtime.interest_center());
        camera.set_movement_speed_multiplier(movement_speed_multiplier);
        camera.set_first_person_player_visible(pending.scene.first_person_player_visible);
        self.draw = TexturedSectionDrawResources::new(
            device,
            queue,
            self.color_format,
            &[],
            self.mesh_assets.atlas.as_upload(),
        )
        .context("reset scene terrain for external session start")?;
        self.scene = pending.scene;
        self.runtime = Some(runtime);
        self.camera = camera;
        self.render_stats = RenderStreamStats::default();
        self.clear_transient_world_state();
        self.clear_menu_input_state();
        self.session.complete_start(active.clone());
        self.status_overlay = StatusOverlay::hidden();
        self.apply_started_session_ui(&active);
        self.sync_player_appearance()
            .context("sync external session player appearance")?;
        Ok(())
    }

    pub fn fail_external_session_start(
        &mut self,
        pending: ExternalSceneSessionStart,
        error: impl Into<String>,
    ) {
        let message = error.into();
        log::error!(
            "failed to start external scene session {:?}: {}",
            pending.request,
            message
        );
        self.session.fail_start(SessionFailure::new(
            pending.request.default_failure_message(),
        ));
        self.apply_xr_session_ui_effects(client_session_failed_start_ui_effects(
            &pending.request,
            false,
        ));
        self.status_overlay = StatusOverlay::new(message, false);
    }

    pub(crate) fn next_new_world_seed(&mut self) -> i64 {
        self.seed_reroll.next_seed()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn local_world_options(&self, seed: i64) -> McloneSceneHostOptions {
        self.scene_for_storage_intent(SessionStorageIntent::transient_local_world(seed))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn remote_session_options(&self, remote_addr: String) -> McloneSceneHostOptions {
        self.scene_for_storage_intent(SessionStorageIntent::remote_session(remote_addr))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn catalog_world_scene(
        &self,
        summary: &LocalWorldSummary,
        world_dir: PathBuf,
    ) -> McloneSceneHostOptions {
        self.scene_for_storage_intent(SessionStorageIntent::catalog_world(summary, world_dir))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn scene_for_storage_intent(&self, intent: SessionStorageIntent) -> McloneSceneHostOptions {
        let mut scene = self.scene.clone();
        if let Some(seed) = intent.seed() {
            scene.seed = seed;
        }
        scene.world_dir = intent.world_dir().map(PathBuf::from);
        if intent.suppress_adaptive_chunk_publication_budget() {
            scene.adaptive_chunk_publication_budget = false;
        }
        if let Some(runtime) = &self.runtime {
            scene.render_distance = runtime.render_distance();
        }
        scene
    }

    pub(crate) fn refresh_world_catalog_ui(&mut self, status: WorldCatalogUiStatus) {
        let Some(mut operations) = self.services.catalog_operations.take() else {
            self.client_experience.catalog_mut().set_worlds(
                WorldCatalogCapabilities::default(),
                Vec::new(),
                status,
            );
            return;
        };
        let active_world = self.active_local_world_id().cloned();
        let catalog = self.client_experience.catalog_mut();
        catalog.set_worlds(
            WorldCatalogCapabilities::persistent_local(),
            Vec::new(),
            status,
        );
        let requests = catalog.request_world_list();
        for request in requests.catalog_requests {
            operations.submit(request, active_world.clone());
        }
        let effects = operations.poll(catalog);
        debug_assert!(effects.session_starts.is_empty());
        debug_assert!(effects.catalog_requests.is_empty());
        self.services.catalog_operations = Some(operations);
    }

    pub(crate) fn active_local_world_id(&self) -> Option<&LocalWorldId> {
        let GameSessionState::Active { session } = self.session.state() else {
            return None;
        };
        session.local_world_id()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn request_session_start(&mut self, request: SessionStartRequest) -> Result<()> {
        let plan = plan_session_start(
            request,
            |options| Ok(self.local_world_options(options.seed)),
            |id| {
                let summary = self
                    .client_experience
                    .catalog()
                    .world_summary(id)
                    .cloned()
                    .context("local world is not present in the catalog view")?;
                let world_root = self
                    .scene
                    .world_root
                    .as_ref()
                    .context("Persistent worlds unavailable")?;
                Ok((
                    self.catalog_world_scene(&summary, world_root.join(summary.id.as_str())),
                    ActiveSessionDescriptor::from_local_world_summary(&summary),
                ))
            },
            |endpoint| Ok(self.remote_session_options(endpoint.address.clone())),
            || anyhow!("unsupported unknown XR session start"),
        )?;
        self.status_overlay = StatusOverlay::hidden();
        self.session.request_start(plan.request, plan.payload);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn start_pending_session(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        let Some(pending) = self.session.take_pending_start() else {
            return Ok(false);
        };
        let request = pending.request.clone();
        match self.start_pending_session_payload(device, queue, pending) {
            Ok(SceneSessionStartOutcome::LocalStartupQueued) => Ok(false),
            Ok(SceneSessionStartOutcome::Started(descriptor)) => {
                self.session.complete_start(descriptor.clone());
                self.status_overlay = StatusOverlay::hidden();
                self.apply_started_session_ui(&descriptor);
                Ok(true)
            }
            Err(error) => {
                log::error!("failed to start XR session {request:?}: {error:#}");
                self.session
                    .fail_start(SessionFailure::new(request.default_failure_message()));
                self.apply_xr_session_ui_effects(client_session_failed_start_ui_effects(
                    &request, false,
                ));
                self.menu_panel_anchor = XrUiPanelAnchor::Head;
                self.menu_panel_recenter_pending = true;
                Err(error)
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn start_pending_session(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<bool> {
        Ok(false)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn start_pending_session_payload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pending: mclone_app_runtime::session::PendingSessionStart<ScenePendingSessionStart>,
    ) -> Result<SceneSessionStartOutcome> {
        let request = pending.request;
        let SessionStartPayload {
            runtime_kind,
            options: scene,
            descriptor,
        } = pending.payload;
        match runtime_kind {
            SessionRuntimeKind::Local => {
                self.begin_local_session_start(request, Some(descriptor), scene)?;
                Ok(SceneSessionStartOutcome::LocalStartupQueued)
            }
            SessionRuntimeKind::Remote => {
                let descriptor =
                    self.start_replacement_session(device, queue, request, descriptor, scene)?;
                Ok(SceneSessionStartOutcome::Started(descriptor))
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn begin_local_session_start(
        &mut self,
        request: SessionStartRequest,
        descriptor: Option<ActiveSessionDescriptor>,
        scene: McloneSceneHostOptions,
    ) -> Result<()> {
        let scene = scene.validated()?;
        let mesh_assets = self.mesh_assets.clone();
        let pump = LocalIntegratedStartupPump::with_mesh_assets(
            local_integrated_scene_options(&scene),
            mesh_assets,
        )
        .context("create XR local world startup pump")?;
        let mut camera = EngineCameraController::spawn_for_chunk(scene.center());
        camera.set_movement_speed_multiplier(f64::from(scene.movement_speed_multiplier));
        camera.set_first_person_player_visible(scene.first_person_player_visible);
        self.local_startup = Some(SceneLocalStartup {
            request: request.clone(),
            descriptor,
            scene: scene.clone(),
            pump,
            camera,
            startup_view_pose: None,
        });
        self.status_overlay = StatusOverlay::hidden();
        self.ui.clear_input();
        self.menu_pointer_down = false;
        self.menu_panel_anchor = XrUiPanelAnchor::Head;
        self.menu_panel_recenter_pending = true;
        log::info!(
            "XR local world startup queued seed={} center=({}, {}) render_distance={}",
            scene.seed,
            scene.chunk_x,
            scene.chunk_z,
            scene.render_distance
        );
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn advance_local_startup(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        if self.local_startup.is_none() {
            return Ok(false);
        }

        let step = {
            let startup = self
                .local_startup
                .as_mut()
                .expect("startup presence checked");
            let camera_position = glam_vec3_from_vec3d(startup.camera.snapshot().eye);
            startup
                .pump
                .step(camera_position)
                .context("advance XR local world startup pump")
        };
        let step = match step {
            Ok(step) => step,
            Err(error) => {
                let startup = self
                    .local_startup
                    .take()
                    .expect("startup must exist after failed pump step");
                self.fail_local_startup(startup, error);
                return Ok(false);
            }
        };

        if !step.playable_ready {
            return Ok(false);
        }

        let startup = self
            .local_startup
            .take()
            .expect("startup must exist after playable step");
        let failed_request = startup.request.clone();
        match self.complete_local_startup(device, queue, startup, step) {
            Ok(()) => Ok(true),
            Err(error) => {
                log::error!("failed to complete XR local world startup: {error:#}");
                self.session.fail_start(SessionFailure::new(
                    failed_request.default_failure_message(),
                ));
                self.apply_xr_session_ui_effects(client_session_failed_start_ui_effects(
                    &failed_request,
                    false,
                ));
                self.menu_panel_anchor = XrUiPanelAnchor::Head;
                self.menu_panel_recenter_pending = true;
                Ok(false)
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn advance_local_startup(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<bool> {
        Ok(false)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn complete_local_startup(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        startup: SceneLocalStartup,
        step: LocalIntegratedStartupStep,
    ) -> Result<()> {
        let SceneLocalStartup {
            request,
            descriptor,
            scene,
            pump,
            mut camera,
            startup_view_pose,
        } = startup;
        let descriptor = descriptor
            .or_else(|| request.active_descriptor())
            .context("XR local startup request did not describe an active session")?;
        // docs/tactical/167 Slice 4: reconcile the final startup pose against the
        // pump-owned runtime, then drain the render seed. The pump reaches playable
        // at the spawn camera; if the accepted correction or the XR startup view
        // pose moves chunk interest, the reconciled drive re-pumps at the new camera
        // so the drained seed covers the final camera rather than the spawn camera.
        let startup_camera = glam_vec3_from_vec3d(camera.snapshot().eye);
        let clock = self.services.clock.clone();
        let (local_runtime, startup_sections) = pump
            .drive_to_ready_reconciled(
                startup_camera,
                DEFAULT_STARTUP_READINESS_TIMEOUT,
                |runtime| {
                    reconcile_xr_startup_pose(runtime, &mut camera, startup_view_pose, &clock)?;
                    Ok(glam_vec3_from_vec3d(camera.snapshot().eye))
                },
            )
            .context("reconcile XR local startup pose")?;
        let runtime: SceneSessionRuntime = NativeSessionServices::<
            mclone_app_runtime::LocalOnlySession,
        >::from_active_runtime_with_descriptor(
            descriptor.clone(),
            NativeSceneServices::Local(local_runtime),
        )
        .into();

        let camera_position = glam_vec3_from_vec3d(camera.snapshot().eye);
        // docs/tactical/167: seed the initial draw resources from the startup
        // pump's render seed rather than recompiling the resident cache; the seed
        // and the traversal-ready publication below both use the final reconciled
        // startup camera position.
        let sections = startup_sections;
        if sections.is_empty() {
            bail!(
                "XR local startup seed={} center=({}, {}) render_distance={} reached playable threshold without render sections",
                scene.seed,
                scene.chunk_x,
                scene.chunk_z,
                scene.render_distance
            );
        }
        let mut draw = TexturedSectionDrawResources::new(
            device,
            queue,
            self.color_format,
            &sections,
            runtime.mesh_assets().atlas.as_upload(),
        )
        .context("upload XR local startup render sections")?;
        draw.set_traversal_ready_sections(
            &runtime.traversal_ready_render_section_keys(camera_position),
        );

        let section_count = draw.section_count();
        let index_count = draw.index_count();
        let face_count = quad_face_count_from_indices(index_count);
        self.scene = scene;
        self.mesh_assets = runtime.mesh_assets().clone();
        self.runtime = Some(runtime);
        self.camera = camera;
        self.draw = draw;
        self.traversal_ready_sections.clear();
        self.render_stats = RenderStreamStats {
            section_count,
            index_count,
            face_count,
            ..RenderStreamStats::default()
        };
        self.sync_player_appearance()
            .context("sync XR local startup player appearance")?;
        self.clear_transient_world_state();
        self.session.complete_start(descriptor.clone());
        self.status_overlay = StatusOverlay::hidden();
        self.apply_started_session_ui(&descriptor);
        match descriptor {
            ActiveSessionDescriptor::LocalWorld { seed, .. } => {
                log::info!(
                    "XR local world playable seed={} polls={} poll_ms={:.3} sections={} target_ready={}/{}",
                    seed,
                    step.poll_count,
                    step.poll_ms,
                    section_count,
                    step.progress
                        .as_ref()
                        .map_or(0, |progress| progress.target_ready_chunks),
                    step.progress
                        .as_ref()
                        .map_or(0, |progress| progress.target_chunk_count)
                );
            }
            ActiveSessionDescriptor::Remote { .. } => {}
        }
        self.apply_debug_ui_screen();
        if !self.ui.is_active() {
            self.clear_menu_input_state();
        }
        Ok(())
    }

    pub(crate) fn apply_debug_ui_screen(&mut self) {
        let Some(screen) = self.scene.debug_ui_screen else {
            return;
        };
        let desired_screen = match screen {
            XrDebugUiScreen::Pause => GameScreen::Pause,
            XrDebugUiScreen::Controls => GameScreen::Help {
                parent: mclone_ui::GameHelpParent::OptionsPause,
            },
        };
        if self.ui.screen() != Some(desired_screen) {
            self.ui.set_screen(Some(desired_screen));
            self.ui.clear_input();
            self.menu_pointer_down = false;
            self.menu_panel_anchor = XrUiPanelAnchor::Head;
            self.menu_panel_recenter_pending = true;
        }
        if self.menu_panel_pose.is_none() {
            self.menu_panel_anchor = XrUiPanelAnchor::Head;
            self.menu_panel_recenter_pending = true;
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn fail_local_startup(&mut self, startup: SceneLocalStartup, error: anyhow::Error) {
        log::error!(
            "failed to start XR local world {:?}: {error:#}",
            startup.request
        );
        self.session.fail_start(SessionFailure::new(
            startup.request.default_failure_message(),
        ));
        self.apply_xr_session_ui_effects(client_session_failed_start_ui_effects(
            &startup.request,
            false,
        ));
        self.menu_panel_anchor = XrUiPanelAnchor::Head;
        self.menu_panel_recenter_pending = true;
    }

    pub(crate) fn clear_menu_input_state(&mut self) {
        self.ui.clear_input();
        self.menu_pointer_down = false;
        self.menu_panel_pose = None;
        self.menu_panel_anchor = XrUiPanelAnchor::Head;
        self.menu_panel_recenter_pending = false;
    }

    pub(crate) fn clear_transient_world_state(&mut self) {
        self.tracking_origin = None;
        self.last_locomotion_update = None;
        self.underwater_effects = XrUnderwaterEffectStates::default();
        self.last_underwater_update = None;
        self.head_comfort.reset();
        self.clear_xr_blink_teleport();
        self.clear_mono_blink_debug();
        self.latest_controllers.clear();
        self.first_eye_summary = None;
        self.last_ui_panel_stats = WorldGuiPanelRenderStats::default();
        self.last_ui_draw_cache_stats = UiDrawCacheStats::default();
        self.rendered_frames = 0;
        self.prefetched_live_upload = None;
        self.traversal_ready_sections.clear();
        self.section_uploads.clear();
        self.render_admission_policy.reset();
    }

    pub(crate) fn teardown_world(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        if let Some(operations) = self.services.catalog_operations.as_mut() {
            for request in operations.begin_epoch() {
                let _ = self.client_experience.catalog_mut().apply_catalog_error(
                    request.id,
                    WorldCatalogError::unsupported("World catalog operation superseded"),
                );
            }
        }
        self.local_startup = None;
        if self.runtime.take().is_some() {
            self.draw = TexturedSectionDrawResources::new(
                device,
                queue,
                self.color_format,
                &[],
                self.mesh_assets.atlas.as_upload(),
            )
            .context("reset XR terrain draw resources during session teardown")?;
        }
        self.render_stats = RenderStreamStats::default();
        self.clear_transient_world_state();
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn start_replacement_session(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        request: SessionStartRequest,
        descriptor: ActiveSessionDescriptor,
        scene: McloneSceneHostOptions,
    ) -> Result<ActiveSessionDescriptor> {
        let ActiveSessionDescriptor::Remote { endpoint } = &descriptor else {
            bail!("XR replacement runtime requires a remote descriptor: {descriptor:?}");
        };
        let mesh_assets = self.mesh_assets.clone();
        let Some(factory) = self.session_runtime_factory.as_mut() else {
            let error = anyhow!("XR session runtime factory is not installed");
            log::error!("{error:#}");
            return Err(error);
        };
        let scene = scene.validated()?;
        let movement_speed_multiplier = scene.movement_speed_multiplier;
        let started = match factory.start(
            endpoint.clone(),
            scene.clone(),
            mesh_assets,
            device,
            queue,
            self.color_format,
            movement_speed_multiplier,
            &self.services.clock,
        ) {
            Ok(started) => started,
            Err(error) => {
                log::error!("failed to warm XR session {request:?}: {error:#}");
                return Err(error);
            }
        };

        self.scene = scene;
        self.mesh_assets = started.runtime.mesh_assets().clone();
        self.runtime = Some(started.runtime);
        self.camera = started.camera;
        self.camera
            .set_first_person_player_visible(self.scene.first_person_player_visible);
        self.draw = started.draw;
        self.render_stats = started.render_stats;
        self.sync_player_appearance()
            .context("sync XR session start player appearance")?;
        self.clear_transient_world_state();
        self.clear_menu_input_state();
        match &descriptor {
            ActiveSessionDescriptor::LocalWorld { seed, .. } => {
                log::info!("XR created local world seed={seed}");
            }
            ActiveSessionDescriptor::Remote { endpoint } => {
                log::info!("XR joined remote session {}", endpoint.address);
            }
        }
        Ok(descriptor)
    }

    pub(crate) fn apply_xr_ui_action(
        &mut self,
        action: GameUiAction,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        if self.local_startup.is_some() {
            return Ok(false);
        }
        let settings_state = self.client_experience_settings_state();
        self.client_experience.set_settings_state(settings_state);

        let active_remote_addr = self.active_remote_addr();
        let current_join_remote_addr = normalized_xr_remote_addr(self.ui.join_remote_addr());
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
        let apply_ui_action =
            client_experience_should_apply_ui_projection(action) && effects.projection.is_empty();
        let scene_replaced = self.apply_client_experience_effects(effects, device, queue)?;
        if apply_ui_action {
            self.apply_xr_projection_action(action);
        }
        if !self.ui.is_active() {
            self.clear_menu_input_state();
        }
        Ok(scene_replaced)
    }

    pub(crate) fn apply_client_experience_effects(
        &mut self,
        effects: ClientExperienceEffects,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        let catalog_scene_replaced =
            self.apply_xr_catalog_effects(effects.catalog, device, queue)?;
        let session_scene_replaced =
            self.apply_xr_session_effects(effects.session, device, queue)?;
        self.apply_asset_pack_effects(effects.asset_packs);
        if !self.apply_xr_settings_effects(effects.settings)? {
            return Ok(catalog_scene_replaced || session_scene_replaced);
        }
        for effect in effects.gameplay {
            match effect {
                ClientExperienceGameplayEffect::AssignHotbarBlock { slot, block_state } => {
                    let changed = self.assign_debug_hotbar_slot(slot, BlockStateId(block_state))?;
                    log::info!(
                        "XR debug hotbar slot {} assigned block_state={} changed={}",
                        slot + 1,
                        block_state,
                        changed
                    );
                }
            }
        }
        for effect in effects.projection {
            match effect {
                ClientExperienceProjectionEffect::ApplyUiAction(action) => {
                    self.apply_xr_projection_action(action);
                }
            }
        }
        Ok(catalog_scene_replaced || session_scene_replaced)
    }

    pub(crate) fn apply_xr_catalog_effects(
        &mut self,
        effects: ClientCatalogEffects,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        let mut scene_replaced = false;
        for start in effects.session_starts {
            #[cfg(not(target_arch = "wasm32"))]
            let scene = {
                let Some(world_root) = self.scene.world_root.clone() else {
                    let error = WorldCatalogError::unsupported("Persistent worlds unavailable");
                    log::warn!("XR catalog session start failed: {error}");
                    continue;
                };
                self.catalog_world_scene(&start.summary, world_root.join(start.summary.id.as_str()))
            };
            #[cfg(target_arch = "wasm32")]
            let scene = {
                let mut scene = self.scene.clone();
                scene.seed = start.summary.seed;
                scene.world_dir = None;
                scene
            };
            self.status_overlay = StatusOverlay::hidden();
            self.session.request_start(
                start.request,
                ScenePendingSessionStart {
                    runtime_kind: SessionRuntimeKind::Local,
                    options: scene,
                    descriptor: start.descriptor,
                },
            );
            self.ui.clear_input();
            self.menu_pointer_down = false;
            scene_replaced |= self.start_pending_session(device, queue)?;
        }
        for request in effects.catalog_requests {
            scene_replaced |= self.execute_xr_catalog_request(request, device, queue)?;
        }
        Ok(scene_replaced)
    }

    pub(crate) fn execute_xr_catalog_request(
        &mut self,
        request: ClientCatalogRequest,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        let Some(mut operations) = self.services.catalog_operations.take() else {
            let error = WorldCatalogError::unsupported("Persistent worlds unavailable");
            let effects = self
                .client_experience
                .catalog_mut()
                .apply_catalog_error(request.id, error);
            return self.apply_xr_catalog_effects(effects, device, queue);
        };
        let active_world = self.active_local_world_id().cloned();
        operations.submit(request, active_world);
        let effects = operations.poll(self.client_experience.catalog_mut());
        self.services.catalog_operations = Some(operations);
        self.apply_xr_catalog_effects(effects, device, queue)
    }

    /// Fold deferred platform catalog completions back through the shared
    /// controller. Browser drivers call this after submitting an IndexedDB
    /// completion; any resulting session start remains queued for
    /// `take_external_session_start`.
    pub fn poll_external_catalog_operations(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        let Some(mut operations) = self.services.catalog_operations.take() else {
            return Ok(false);
        };
        let effects = operations.poll(self.client_experience.catalog_mut());
        self.services.catalog_operations = Some(operations);
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.apply_xr_catalog_effects(effects, device, queue)
        }
        #[cfg(target_arch = "wasm32")]
        {
            let mut scene_replaced = false;
            for start in effects.session_starts {
                let mut scene = self.scene.clone();
                scene.seed = start.summary.seed;
                scene.world_dir = None;
                self.status_overlay = StatusOverlay::hidden();
                self.session.request_start(
                    start.request,
                    ScenePendingSessionStart {
                        runtime_kind: SessionRuntimeKind::Local,
                        options: scene,
                        descriptor: start.descriptor,
                    },
                );
                self.ui.clear_input();
                self.menu_pointer_down = false;
            }
            for request in effects.catalog_requests {
                scene_replaced |= self.execute_xr_catalog_request(request, device, queue)?;
            }
            let render_state = self.current_mono_ui_render_state();
            self.ui.commit_render_state(render_state);
            Ok(scene_replaced)
        }
    }

    pub(crate) fn apply_xr_session_effects(
        &mut self,
        effects: ClientSessionEffects,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        if let Some(seed) = effects.new_world_seed {
            self.ui.set_new_world_seed(seed);
        }
        if let Some(addr) = effects.join_remote_addr {
            self.ui
                .set_join_remote_addr(normalized_xr_remote_addr(&addr));
        }
        if effects.clear_inactive_session_status {
            self.clear_inactive_session_status();
        }

        let mut scene_replaced = false;
        if let Some(request) = effects.session_start {
            scene_replaced = self.start_session_for_request(device, queue, request)?;
        }
        if let Some(host_action) = effects.host_action {
            let mut host = XrSessionHostEffects {
                scene: self,
                device,
                queue,
            };
            apply_client_session_host_action(host_action, &mut host)?;
        }
        Ok(scene_replaced)
    }

    pub(crate) fn apply_xr_session_transition_effects(
        &mut self,
        effects: ClientSessionTransitionEffects,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        if effects.teardown_world {
            self.teardown_world(device, queue)?;
        }
        if effects.clear_session {
            self.session.clear();
        }
        self.status_overlay = StatusOverlay::hidden();
        self.apply_xr_session_ui_effects(effects.ui);
        Ok(())
    }

    pub(crate) fn apply_xr_settings_effects(
        &mut self,
        effects: ClientExperienceSettingsEffects,
    ) -> Result<bool> {
        let mut host = XrSettingsHostEffects;
        apply_client_experience_settings_effects(self, &mut host, effects)
    }

    pub(crate) fn apply_xr_session_ui_effects(&mut self, effects: ClientSessionUiEffects) {
        if let Some(seed) = effects.new_world_seed {
            self.ui.set_new_world_seed(seed);
        }
        if let Some(addr) = effects.join_remote_addr {
            self.ui
                .set_join_remote_addr(normalized_xr_remote_addr(&addr));
        }
        if let Some(screen) = effects.screen {
            self.ui.set_screen(Some(screen));
        }
    }

    pub(crate) fn apply_started_session_ui(&mut self, descriptor: &ActiveSessionDescriptor) {
        match descriptor {
            ActiveSessionDescriptor::LocalWorld { seed, id, .. } => {
                self.ui.set_new_world_seed(*seed);
                if id.is_some() {
                    self.ui.close();
                } else {
                    self.ui.apply_action(GameUiAction::CreateWorld(*seed));
                }
            }
            ActiveSessionDescriptor::Remote { endpoint } => {
                self.ui.set_join_remote_addr(endpoint.address.clone());
                self.ui.apply_action(GameUiAction::JoinRemote);
            }
        }
    }

    pub(crate) fn apply_xr_projection_action(&mut self, action: GameUiAction) {
        if matches!(action, GameUiAction::OpenBlockPalette) {
            self.menu_panel_anchor = XrUiPanelAnchor::LeftHand;
            self.menu_panel_pose = None;
            self.menu_panel_recenter_pending = true;
        }
        self.ui.apply_action(action);
    }
}

struct XrSettingsHostEffects;

impl HostEffects for XrSettingsHostEffects {
    fn request_mouse_lock(&mut self, _requested: bool) -> Result<()> {
        Ok(())
    }

    fn cycle_frame_pacing(&mut self) -> Result<()> {
        log::info!("XR frame pacing cycle ignored by scene host");
        Ok(())
    }

    fn cycle_fps_cap(&mut self) -> Result<()> {
        log::info!("XR FPS cap cycle ignored by scene host");
        Ok(())
    }

    fn set_touch_controls_mode(&mut self, _mode: TouchControlsMode) -> Result<()> {
        Ok(())
    }

    fn quit_to_title(&mut self) -> Result<()> {
        Ok(())
    }

    fn exit(&mut self) -> Result<()> {
        log::info!("XR menu quit action ignored by shared scene");
        Ok(())
    }
}

struct XrSessionHostEffects<'a, 'device> {
    scene: &'a mut McloneSceneHost,
    device: &'device wgpu::Device,
    queue: &'device wgpu::Queue,
}

impl HostEffects for XrSessionHostEffects<'_, '_> {
    fn request_mouse_lock(&mut self, _requested: bool) -> Result<()> {
        Ok(())
    }

    fn cycle_frame_pacing(&mut self) -> Result<()> {
        log::info!("XR frame pacing cycle ignored by scene host");
        Ok(())
    }

    fn cycle_fps_cap(&mut self) -> Result<()> {
        log::info!("XR FPS cap cycle ignored by scene host");
        Ok(())
    }

    fn set_touch_controls_mode(&mut self, _mode: TouchControlsMode) -> Result<()> {
        Ok(())
    }

    fn quit_to_title(&mut self) -> Result<()> {
        let transition = client_session_quit_to_title_transition(self.scene.session.state());
        self.scene
            .apply_xr_session_transition_effects(transition, self.device, self.queue)
    }

    fn exit(&mut self) -> Result<()> {
        log::info!("XR menu quit action ignored by shared scene");
        Ok(())
    }
}

impl ClientExperienceSettingsHost for McloneSceneHost {
    fn set_section_occlusion_culling(&mut self, enabled: bool) -> Result<()> {
        self.render_options.section_occlusion_culling = enabled;
        log::info!(
            "XR section occlusion culling {}",
            if enabled { "enabled" } else { "disabled" }
        );
        Ok(())
    }

    fn set_fullbright(&mut self, enabled: bool) -> Result<()> {
        self.render_options.force_fullbright = enabled;
        log::info!(
            "XR fullbright {}",
            if enabled { "enabled" } else { "disabled" }
        );
        Ok(())
    }

    fn set_far_lod(&mut self, enabled: bool, extra_radius_chunks: u32) -> Result<()> {
        self.scene.far_lod = self
            .scene
            .far_lod
            .with_extra_radius_chunks(extra_radius_chunks);
        self.scene.far_lod.enabled = enabled;
        log::info!(
            "XR far LOD {}",
            if enabled { "enabled" } else { "disabled" }
        );
        log::info!(
            "XR far LOD range set to {} chunks beyond render distance",
            self.scene.far_lod.extra_radius_chunks
        );
        Ok(())
    }

    fn clear_far_lod(&mut self) -> Result<()> {
        if let Some(runtime) = &mut self.runtime {
            runtime.clear_far_lod();
        }
        Ok(())
    }

    fn set_player_collision_box_visible(&mut self, visible: bool) -> Result<()> {
        self.player_collision_box_visible = visible;
        log::info!(
            "XR player collision box debug {}",
            if visible { "visible" } else { "hidden" }
        );
        Ok(())
    }

    fn set_first_person_player_visible(&mut self, visible: bool) -> Result<()> {
        self.camera.set_first_person_player_visible(visible);
        log::info!(
            "XR first-person player body {}",
            if visible { "visible" } else { "hidden" }
        );
        Ok(())
    }

    fn set_crosshair_visible(&mut self, visible: bool) -> Result<()> {
        self.crosshair_visible = visible;
        Ok(())
    }

    fn set_frame_pipeline_overlay_visible(&mut self, visible: bool) -> Result<()> {
        self.diagnostic_panel.set_frame_metrics_visible(visible);
        log::info!(
            "XR frame pipeline overlay {}",
            if visible { "visible" } else { "hidden" }
        );
        Ok(())
    }

    fn set_debug_diagnostics_visible(&mut self, visible: bool) -> Result<()> {
        self.diagnostic_panel.set_debug_diagnostics_visible(visible);
        log::info!(
            "XR debug diagnostics {}",
            if visible { "visible" } else { "hidden" }
        );
        Ok(())
    }

    fn set_player_model(&mut self, model: GamePlayerModel) -> Result<()> {
        self.player_model = model;
        log::info!("XR player model set to {}", model.label());
        Ok(())
    }

    fn sync_player_appearance(&mut self) -> Result<()> {
        if let Err(error) = McloneSceneHost::sync_player_appearance(self) {
            log::warn!("failed to sync XR player appearance: {error:#}");
        }
        Ok(())
    }

    fn set_movement_mode(&mut self, mode: GameMovementMode) -> Result<()> {
        self.camera.set_movement_mode(engine_movement_mode(mode));
        log::info!(
            "XR player movement mode {}",
            self.camera.movement_mode().label()
        );
        Ok(())
    }

    fn set_collision_mode(&mut self, mode: GameCollisionMode) -> Result<()> {
        self.camera.set_collision_mode(engine_collision_mode(mode));
        log::info!(
            "XR player collision mode {}",
            self.camera.collision_mode().label()
        );
        Ok(())
    }

    fn set_travel_assist_mode(&mut self, mode: GameTravelAssistMode) -> Result<()> {
        self.travel_assist_mode = mode;
        if self.travel_assist_mode != GameTravelAssistMode::Blink {
            self.clear_xr_blink_teleport();
            self.clear_mono_blink_debug();
        }
        log::info!("XR travel assist {}", mode.label());
        Ok(())
    }

    fn set_turn_mode(&mut self, mode: GameTurnMode) -> Result<()> {
        let mode = GameXrTurnMode::from(mode);
        self.set_turn_policy(XrTurnPolicy::from_game_mode(mode));
        log::info!("XR turn mode {}", mode.label());
        Ok(())
    }

    fn set_xr_turn_mode(&mut self, mode: GameXrTurnMode) -> Result<()> {
        self.set_turn_policy(XrTurnPolicy::from_game_mode(mode));
        log::info!("XR turn mode {}", mode.label());
        Ok(())
    }

    fn set_render_distance(&mut self, render_distance: u32) -> Result<()> {
        if let Some(runtime) = &mut self.runtime
            && runtime
                .set_render_distance(render_distance)
                .context("set XR render distance from menu")?
        {
            log::info!(
                "XR render distance set to {} (chunk tracking radius {})",
                render_distance,
                runtime.chunk_tracking_radius()
            );
        }
        self.scene.render_distance = render_distance;
        Ok(())
    }

    fn set_fly_speed_multiplier(&mut self, multiplier: f32) -> Result<()> {
        self.camera.set_fly_speed_multiplier(f64::from(multiplier));
        log::info!(
            "XR fly speed set to {:.1}x ({:.0} blocks/s)",
            self.camera.fly_speed_multiplier(),
            self.camera.speed_blocks_per_second()
        );
        Ok(())
    }

    fn set_movement_speed_multiplier(&mut self, multiplier: f32) -> Result<()> {
        self.camera
            .set_movement_speed_multiplier(f64::from(multiplier));
        self.scene.movement_speed_multiplier = self.camera.movement_speed_multiplier() as f32;
        log::info!(
            "XR movement speed multiplier set to {:.1}x",
            self.camera.movement_speed_multiplier()
        );
        Ok(())
    }

    fn set_touch_look_sensitivity(&mut self, _sensitivity: f32) -> Result<()> {
        Ok(())
    }

    fn set_server_simulation_cadence(&mut self, cadence: GameSimulationCadence) -> Result<()> {
        let cadence = SimulationCadenceConfig::new(
            cadence.host_rate_hz,
            cadence.gameplay_rate_hz,
            cadence.physics_rate_hz,
        );
        if let Some(runtime) = &mut self.runtime {
            runtime.set_simulation_cadence(cadence)?;
        }
        self.scene.simulation_cadence = cadence;
        Ok(())
    }

    fn set_status_overlay(&mut self, status: StatusOverlay) {
        self.status_overlay = status;
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn start_scene_runtime<S>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color_format: wgpu::TextureFormat,
    runtime: NativeSessionServices<S>,
    movement_speed_multiplier: f32,
    startup_view_pose: Option<XrStartupViewPose>,
    clock: &MonotonicClockHandle,
) -> Result<StartedSceneRuntime>
where
    S: RemoteDedicatedServerSession + 'static,
{
    let center = runtime.interest_center();
    let render_distance = runtime.render_distance();
    let host_label = runtime.host_label();
    let session_label = active_session_label(runtime.active_session());
    let mut camera = EngineCameraController::spawn_for_chunk(center);
    camera.set_movement_speed_multiplier(f64::from(movement_speed_multiplier));
    // docs/tactical/167 Slice 3/4: drive the shared startup pump synchronously to a
    // drawable active view instead of `poll_until_idle` + `sync_all_render_sections`,
    // reconciling the final startup pose *inside* the drive. If the accepted server
    // correction or the XR startup view pose moves chunk interest (a far view pose
    // can move it several chunks), the pump re-pumps at the new camera so the seed
    // covers the final camera rather than the initial spawn camera.
    let initial_poll_start = clock.now();
    let startup_camera_position = glam_vec3_from_vec3d(camera.snapshot().eye);
    let completion = NativeSessionStartupPump::from_runtime(runtime)
        .drive_to_ready_reconciled(
            startup_camera_position,
            DEFAULT_STARTUP_READINESS_TIMEOUT,
            |runtime| {
                reconcile_xr_startup_pose(runtime, &mut camera, startup_view_pose, clock)?;
                Ok(glam_vec3_from_vec3d(camera.snapshot().eye))
            },
        )
        .context("wait for initial XR terrain chunks")?;
    let NativeSessionStartupCompletion {
        runtime,
        startup_sections,
        final_step,
    } = completion;

    let camera_position = glam_vec3_from_vec3d(camera.snapshot().eye);
    // docs/tactical/167: seed the initial draw resources from the startup pump's
    // render seed rather than recompiling the resident cache; the seed and
    // traversal-ready publication both use the final reconciled startup camera.
    let sections = startup_sections;
    if sections.is_empty() {
        bail!(
            "XR terrain host={} center=({}, {}) render_distance={} produced no render sections",
            host_label,
            center.x,
            center.z,
            render_distance
        );
    }
    let mut draw = TexturedSectionDrawResources::new(
        device,
        queue,
        color_format,
        &sections,
        runtime.mesh_assets().atlas.as_upload(),
    )
    .context("upload initial XR terrain render sections")?;
    draw.set_traversal_ready_sections(
        &runtime.traversal_ready_render_section_keys(camera_position),
    );

    let render_stats = RenderStreamStats {
        section_count: draw.section_count(),
        index_count: draw.index_count(),
        face_count: quad_face_count_from_indices(draw.index_count()),
        ..RenderStreamStats::default()
    };
    log::info!(
        "mclone XR terrain runtime: host={} session={} center=({}, {}) render_distance={} chunks={} sections={} faces={} indices={} initial_polls={} poll_ms={:.3} elapsed_ms={:.3}",
        host_label,
        session_label,
        center.x,
        center.z,
        render_distance,
        runtime.loaded_chunk_count(),
        render_stats.section_count,
        render_stats.face_count,
        render_stats.index_count,
        final_step.poll_count,
        final_step.poll_ms,
        elapsed_ms(clock.elapsed_since(initial_poll_start))
    );
    Ok(StartedSceneRuntime {
        runtime: runtime.into(),
        camera,
        draw,
        render_stats,
    })
}

/// Shared camera/interest reconciliation lane for XR startup (docs/tactical/167
/// Slice 4): send-only pose sync, `"XR terrain"` log lane. Per-frame callers
/// request optional shared instrumentation for XR camera-commit profiling.
pub(crate) const XR_CAMERA_COMMIT_CONTEXT: EngineCameraCommitContext =
    EngineCameraCommitContext::send_only("XR terrain");

/// Apply the XR startup physical pose against the pump-owned runtime during
/// reconciliation (docs/tactical/167 Slice 4): accept any pending server position
/// correction, apply the optional startup view pose, then commit the camera pose
/// and follow chunk interest. When this moves chunk interest the shared pump
/// re-pumps at the new camera so the seed covers the final startup camera — a far
/// startup view pose (validated on-device several chunks out) otherwise leaves the
/// ready frame with nothing drawn near the camera.
#[cfg(not(target_arch = "wasm32"))]
fn reconcile_xr_startup_pose<S>(
    runtime: &mut NativeSceneServices<S>,
    camera: &mut EngineCameraController,
    startup_view_pose: Option<XrStartupViewPose>,
    clock: &MonotonicClockHandle,
) -> Result<bool>
where
    S: RemoteDedicatedServerSession + 'static,
{
    let mut changed = mclone_app_runtime::apply_pending_engine_camera_position_updates(
        runtime,
        camera,
        XR_CAMERA_COMMIT_CONTEXT,
    )?;
    if let Some(view_pose) = startup_view_pose {
        apply_xr_startup_view_pose(camera, view_pose.position, view_pose.yaw_degrees)
            .context("apply XR startup view pose")?;
    }
    changed |= mclone_app_runtime::commit_engine_camera_player_pose(
        runtime,
        camera,
        XR_CAMERA_COMMIT_CONTEXT,
        clock,
        None,
    )?;
    Ok(changed)
}

pub(crate) fn xr_game_ui_for_session(
    session: Option<&ActiveSessionDescriptor>,
    seed: i64,
) -> GameUiHost {
    let mut ui = GameUiHost::new();
    ui.set_new_world_seed(match session {
        Some(ActiveSessionDescriptor::LocalWorld { seed, .. }) => *seed,
        Some(ActiveSessionDescriptor::Remote { .. }) | None => seed,
    });
    ui.set_join_remote_addr(match session {
        Some(ActiveSessionDescriptor::Remote { endpoint }) => endpoint.address.clone(),
        Some(ActiveSessionDescriptor::LocalWorld { .. }) | None => {
            DEFAULT_JOIN_REMOTE_ADDR.to_owned()
        }
    });
    ui
}

pub(crate) fn normalized_xr_remote_addr(addr: &str) -> String {
    let addr = addr.trim();
    if addr.is_empty() {
        DEFAULT_JOIN_REMOTE_ADDR.to_owned()
    } else {
        addr.to_owned()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn xr_client_experience_profile() -> ClientExperienceProfile {
    // Shared native profile owner; the XR crate keeps this thin crate-local alias
    // so its several call sites stay stable.
    xr_native_client_experience_profile()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn local_integrated_scene_options(
    scene: &McloneSceneHostOptions,
) -> LocalIntegratedSceneOptions {
    let storage = IntegratedWorldSessionStorage::from_world_dir(scene.world_dir.as_deref())
        .with_adaptive_chunk_publication_budget(scene.adaptive_chunk_publication_budget);
    let mut options =
        LocalIntegratedSceneOptions::new(scene.seed, scene.center(), scene.render_distance);
    if scene.use_initial_spawn_center {
        options = options.with_initial_spawn_center();
    }
    options
        .with_freeze_scheduled_fluid_ticks(scene.freeze_scheduled_fluid_ticks)
        .with_day_time(scene.day_time_override)
        .with_freeze_time(scene.freeze_time)
        .with_cadence(scene.simulation_cadence)
        .with_debug_passive_showcase(scene.debug_passive_showcase)
        .with_lighting_enabled(scene.lighting_enabled)
        .with_light_status_batch_size(scene.light_status_batch_size)
        .with_render_compile_worker_count(scene.render_compile_worker_count)
        .with_render_compile_max_pending_jobs(scene.render_compile_max_pending_jobs)
        .with_render_compile_worker_timing_enabled(scene.render_compile_worker_timing_enabled)
        .with_startup_lod_prewarm(
            mclone_app_runtime::far_lod::StartupLodPrewarmConfig::for_far_lod(
                scene.far_lod,
                scene.startup_lod_prewarm,
            ),
        )
        .with_integrated_world_session_storage(storage)
}

pub fn single_view_host_options(scene: &McloneSceneHostOptions) -> SingleViewHostOptions {
    SingleViewHostOptions::new(scene.center(), scene.render_distance)
        .with_render_compile_worker_count(scene.render_compile_worker_count)
        .with_render_compile_max_pending_jobs(scene.render_compile_max_pending_jobs)
        .with_render_compile_worker_timing_enabled(scene.render_compile_worker_timing_enabled)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn active_session_label(session: Option<&ActiveSessionDescriptor>) -> String {
    match session {
        Some(ActiveSessionDescriptor::LocalWorld { seed, .. }) => {
            format!("local-world:{seed}")
        }
        Some(ActiveSessionDescriptor::Remote { endpoint }) => {
            format!("remote:{}", endpoint.address)
        }
        None => "none".to_owned(),
    }
}
