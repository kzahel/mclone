use super::*;
use crate::client_clock::ClientClockCadence;
use crate::pose_sync::PlayerPoseSyncCadence;

#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::DEFAULT_STARTUP_READINESS_TIMEOUT;
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::session::SessionStorageIntent;

const STANDBY_RUNTIME_POLL_BUDGET: Duration = Duration::from_micros(500);
const STANDBY_UPLOAD_BUDGET: usize = 1;
const STANDBY_ACCEPT_BUDGET: usize = 1;
const STANDBY_COMPILE_REQUEST_BUDGET: usize = 1;
const STANDBY_PREPARATION_BUDGET: Duration = Duration::from_micros(750);
const LOBBY_RETURN_PREVIEW_DISPLAY_OFFSET: Vec3d = Vec3d::new(0.0, 2.0, 6.0);
const WARM_WORLD_ENTRY_GROUND_PROBE_DISTANCE: f64 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SceneCameraConfig {
    movement_speed_multiplier: f64,
    first_person_player_visible: bool,
    movement_mode: EngineCameraMovementMode,
    collision_mode: EngineCameraCollisionMode,
}

impl SceneCameraConfig {
    pub(crate) fn from_scene(scene: &McloneSceneHostOptions) -> Self {
        let mut movement = ClientExperienceSettingsState::default();
        movement.apply_movement_experience_change(
            ClientExperienceMovementSettingChange::MovementMode(scene.movement_mode),
        );
        Self {
            movement_speed_multiplier: f64::from(scene.movement_speed_multiplier),
            first_person_player_visible: scene.first_person_player_visible,
            movement_mode: engine_movement_mode(movement.movement_mode),
            collision_mode: engine_collision_mode(movement.collision_mode),
        }
    }

    fn spawn_for_chunk(self, center: ChunkPos) -> EngineCameraController {
        self.spawn_for_chunk_with_speed(center, self.movement_speed_multiplier)
    }

    fn spawn_for_chunk_with_speed(
        self,
        center: ChunkPos,
        movement_speed_multiplier: f64,
    ) -> EngineCameraController {
        let mut camera = EngineCameraController::spawn_for_chunk(center);
        self.apply_with_speed(&mut camera, movement_speed_multiplier);
        camera
    }

    pub(crate) fn from_eye_pose(
        self,
        eye: Vec3d,
        yaw_radians: f64,
        pitch_radians: f64,
        speed_blocks_per_second: f64,
        collision_mode: EngineCameraCollisionMode,
    ) -> EngineCameraController {
        let mut camera = EngineCameraController::from_eye_pose(
            eye,
            yaw_radians,
            pitch_radians,
            speed_blocks_per_second,
        );
        self.apply_with_speed(&mut camera, self.movement_speed_multiplier);
        camera.set_collision_mode(collision_mode);
        camera
    }

    fn apply_with_speed(self, camera: &mut EngineCameraController, movement_speed_multiplier: f64) {
        camera.set_movement_speed_multiplier(movement_speed_multiplier);
        camera.set_first_person_player_visible(self.first_person_player_visible);
        camera.set_movement_mode(self.movement_mode);
        camera.set_collision_mode(self.collision_mode);
    }
}

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
        clock: &MonotonicClockHandle,
    ) -> Result<StartedSceneRuntime> {
        let camera_config = SceneCameraConfig::from_scene(&scene);
        let runtime = (self.factory)(endpoint, scene, mesh_assets)?;
        start_scene_runtime(
            device,
            queue,
            color_format,
            runtime,
            camera_config,
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
    /// Interest center whose drawable seed must arrive before the next pose
    /// reconciliation pass. Keeping this state beside the pump lets interactive
    /// hosts return to their event loop between startup steps.
    pub(super) reconciled_interest_center: Option<ChunkPos>,
    pub(super) reconciliation_deadline: Option<MonotonicDeadline>,
    pub(super) reconciliation_passes: usize,
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
pub enum ExternalSceneStartTarget {
    ActiveSession {
        token: mclone_app_runtime::platform_operation::PlatformOperationToken,
    },
    Lobby {
        token: mclone_app_runtime::platform_operation::PlatformOperationToken,
        role: mclone_app_runtime::scenario_content::LobbyWorldRole,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalSceneSessionStart {
    pub target: ExternalSceneStartTarget,
    pub instance_id: WorldInstanceId,
    pub storage_source: Option<mclone_app_runtime::scenario_content::LobbyWorldSource>,
    pub request: SessionStartRequest,
    pub runtime_kind: SessionRuntimeKind,
    pub scene: McloneSceneHostOptions,
    pub local_launch_plan: Option<mclone_app_runtime::local_session_launch::LocalSessionLaunchPlan>,
    pub descriptor: ActiveSessionDescriptor,
    pub destination: Option<PreparedEmbeddedWorldScenario>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SceneSessionStartOutcome {
    LocalStartupQueued,
    Started(ActiveSessionDescriptor),
}

impl McloneSceneHost {
    pub const fn debug_lobby_auxiliary_player_script_enabled(&self) -> bool {
        self.debug_lobby_auxiliary_player_script
    }

    /// Replace only the platform capability profile while preserving catalog,
    /// asset-pack, and settings controller state.
    pub fn set_client_experience_profile(&mut self, profile: ClientExperienceProfile) {
        self.client_experience.set_profile(profile);
    }

    fn allocate_world_instance_id(&mut self) -> WorldInstanceId {
        let id = WorldInstanceId::new(self.next_world_instance_id);
        self.next_world_instance_id = self
            .next_world_instance_id
            .checked_add(1)
            .expect("world instance ID space exhausted");
        id
    }

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
    /// Build the retained native scene shell used by a session-free title.
    ///
    /// Asset preparation and catalog services are available to the menu, but
    /// no integrated server, client runtime, or world worker is started.
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::too_many_arguments)]
    pub fn start_native_without_session(
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
        client_experience_profile: ClientExperienceProfile,
        startup_view_pose: Option<XrStartupViewPose>,
    ) -> Result<Self> {
        let scene = scene.validated()?;
        let active_assets = PreparedSceneAssets::startup(
            0,
            mesh_assets,
            ActorTextureAssets {
                atlas: actor_atlas,
                figures: actor_figures,
            },
            load_screen_effect_texture_assets(asset_source)
                .context("prepare title scene screen-effect assets")?,
            PreparedAudioAssets::load(asset_source).context("prepare title scene audio assets")?,
        )?;
        let catalog_operations = scene
            .world_root
            .clone()
            .map(native_world_catalog_operations);
        Self::without_session(
            device,
            queue,
            color_format,
            clock,
            scene,
            render_options,
            active_assets,
            client_experience_profile,
            catalog_operations,
            startup_view_pose,
        )
    }

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
        let mut scene = scene.validated()?;
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
        )?;
        let request = transient_local_session_start_request(&scene);
        let descriptor = request.active_descriptor();
        let catalog_operations = scene
            .world_root
            .clone()
            .map(native_world_catalog_operations);
        let local_options = local_integrated_scene_options(&scene);
        scene.chunk_x = local_options.center.x;
        scene.chunk_z = local_options.center.z;
        let mut camera =
            SceneCameraConfig::from_scene(&scene).spawn_for_chunk(local_options.center);
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
        let pump = LocalIntegratedStartupPump::with_mesh_assets(local_options, mesh_assets.clone())
            .context("create initial XR local world startup pump")?;
        let ui = xr_game_ui_for_session(None, scene.seed);
        let mut session = GameSessionCoordinator::new();
        session.begin_start(request.clone());
        let active_world = DrawableWorldSlot::new(
            DrawableWorldSlotInstall {
                id: WorldInstanceId::new(1),
                descriptor: descriptor.clone(),
                lifecycle: WorldSlotLifecycle::Starting,
                asset_epoch: 0,
                scene: scene.clone(),
                runtime: None,
                local_startup: Some(SceneLocalStartup {
                    request: request.clone(),
                    descriptor,
                    scene: scene.clone(),
                    pump,
                    camera: camera.clone(),
                    startup_view_pose,
                    reconciled_interest_center: None,
                    reconciliation_deadline: None,
                    reconciliation_passes: 0,
                }),
                external_runtime_startup_pending: false,
                camera,
                draw,
                actors: Some(
                    ActorDrawResources::new(
                        device,
                        queue,
                        color_format,
                        actor_atlas.as_upload(),
                        Some(&actor_figures),
                    )
                    .context("initialize XR terrain actor draw resources")?,
                ),
                render_stats: RenderStreamStats::default(),
                accepted_entry_pose: None,
                pending_startup_sections: Vec::new(),
            },
            RenderAdmissionPolicy::new(
                FrameHostKind::HeadlessOffscreenPerf,
                WorkWindow::BeforeRender,
            ),
        );
        let mut state = Self {
            active_world,
            standby_world: None,
            next_world_instance_id: 2,
            warm_world_standby: None,
            prepared_warm_world_shell: None,
            lobby_launch: None,
            debug_lobby_auxiliary_player_script: scene.debug_auxiliary_player_script,
            embedded_world_preview: None,
            embedded_world_activation: EmbeddedWorldActivationState::default(),
            embedded_world_activation_sequence: 0,
            world_gate: None,
            opaque_world_gate_renderer: None,
            warm_world_switch_sequence: 0,
            last_warm_world_switch: None,
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
            graphics_preference_storage: None,
            graphics_preference_error: None,
            terrain_lod_preset_preference: game_terrain_lod_preset(
                scene.startup.terrain_lod_preset,
            ),
            terrain_lod_applied_preset: TerrainLodPreset::Off,
            terrain_lod_persisted_preference: None,
            terrain_lod_pending_persistence: None,
            terrain_lod_apply_error: None,
            fog_settings: GameFogSettings::default(),
            pending_leaf_detail: None,
            pending_restored_asset_pack_selection: None,
            external_asset_pack_preparation: false,
            pending_external_asset_pack_selection: None,
            external_asset_pack_operations: PlatformOperationLedger::new(),
            session,
            active_session_start_operations: PlatformOperationLedger::new(),
            session_runtime_factory: None,
            client_experience: ClientExperienceController::new(
                xr_native_client_experience_profile(),
            ),
            storage_profile_ui: StorageProfileUiState::default(),
            initial_alignment_mode: if startup_view_pose.is_some() {
                XrViewAlignmentMode::ViewPose
            } else {
                XrViewAlignmentMode::PlayerSpawn
            },
            render_options,
            season_preview: SeasonPreviewSettings::default(),
            celestial_debug: CelestialDebugSettings::default(),
            player_collision_box_visible: false,
            crosshair_visible: true,
            travel_assist_mode: GameTravelAssistMode::Off,
            selection_outline: SelectionOutlineRenderer::new(device, color_format),
            worldgen_lens: Default::default(),
            worldgen_lens_renderer: WorldColorMeshRenderer::new(device, color_format),
            world_gui_renderer,
            world_gui_overlay_renderer: WorldGuiRenderer::new(device, color_format),
            mono_gui: None,
            mono_ui_context: None,
            diagnostic_panel: XrDiagnosticPanel::new(device, color_format),
            ui,
            menu_overlay_cache: XrMenuPanelOverlayCache::default(),
            status_overlay: StatusOverlay::hidden(),
            xr_render_path_state: None,
            pending_xr_render_mode_request: None,
            sky: SkyRenderer::new_with_color_profile_and_source(
                device,
                queue,
                color_format,
                render_options.color_profile,
                asset_source,
            )
            .context("initialize XR sun texture assets")?,
            screen_effects: ScreenEffectsRenderer::new(device, queue, color_format, asset_source)
                .context("initialize XR screen effects renderer")?,
            terrain_view: None,
            terrain_horizon_diagnostic: Default::default(),
            terrain_vegetation_executor_factory:
                terrain_view::default_scene_terrain_vegetation_executor_factory(),
            underwater_effects: XrUnderwaterEffectStates::default(),
            last_underwater_update: None,
            head_comfort: XrHeadComfortState::default(),
            tracking_origin: None,
            locomotion_mode: XrLocomotionMode::default(),
            turn_policy: XrTurnPolicy::default(),
            snap_turn_state: XrSnapTurnState::default(),
            blink_teleport: XrBlinkTeleportState::default(),
            mono_blink_debug: MonoBlinkDebugState::default(),
            display_refresh_hz: None,
            render_split_timing_enabled: false,
            defer_eye_waits_enabled: false,
            overlap_runtime_prefetch_enabled: false,
            prefetched_live_upload: None,
            render_section_upload_budget: None,
            render_section_accept_budget: None,
            render_completed_result_accept_budget: None,
            per_view_uniform_frame: 0,
            last_locomotion_update: None,
            client_clock: ClientClockCadence::default(),
            player_pose_sync: PlayerPoseSyncCadence::default(),
            menu_toggle_down: false,
            game_ui_toggle_down: false,
            menu_pointer_down: false,
            menu_panel_pose: None,
            menu_panel_anchor: XrUiPanelAnchor::Head,
            menu_panel_recenter_pending: true,
            latest_xr_input: XrInputFrame::default(),
            latest_xr_head_gaze_stage: None,
            first_eye_summary: None,
            last_ui_panel_stats: WorldGuiPanelRenderStats::default(),
            last_ui_draw_cache_stats: UiDrawCacheStats::default(),
            rendered_frames: 0,
            seed_reroll: NewWorldSeedReroll::new(scene.seed),
        };
        state.refresh_world_catalog_ui(WorldCatalogUiStatus::hidden());
        state.refresh_storage_profile_ui();
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
            SceneCameraConfig::from_scene(&scene),
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
        session.complete_start(active_session.clone());
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
        )?;
        let mut world_gui_renderer = WorldGuiRenderer::new(device, color_format);
        world_gui_renderer
            .upload_texture_atlas(device, queue, mesh_assets.atlas.as_upload())
            .context("upload XR GUI atlas")?;
        let accepted_entry_pose = Some(WorldEntryPose::from_camera(&started.camera));
        let active_world = DrawableWorldSlot::new(
            DrawableWorldSlotInstall {
                id: WorldInstanceId::new(1),
                descriptor: Some(active_session),
                lifecycle: WorldSlotLifecycle::ActiveReady,
                asset_epoch: 0,
                scene: scene.clone(),
                runtime: Some(started.runtime),
                local_startup: None,
                external_runtime_startup_pending: false,
                camera: started.camera,
                draw: started.draw,
                actors: Some(
                    ActorDrawResources::new(
                        device,
                        queue,
                        color_format,
                        actor_atlas.as_upload(),
                        Some(&actor_figures),
                    )
                    .context("initialize XR terrain actor draw resources")?,
                ),
                render_stats: started.render_stats,
                accepted_entry_pose,
                pending_startup_sections: Vec::new(),
            },
            RenderAdmissionPolicy::new(
                FrameHostKind::HeadlessOffscreenPerf,
                WorkWindow::BeforeRender,
            ),
        );
        let mut state = Self {
            active_world,
            standby_world: None,
            next_world_instance_id: 2,
            warm_world_standby: None,
            prepared_warm_world_shell: None,
            lobby_launch: None,
            debug_lobby_auxiliary_player_script: scene.debug_auxiliary_player_script,
            embedded_world_preview: None,
            embedded_world_activation: EmbeddedWorldActivationState::default(),
            embedded_world_activation_sequence: 0,
            world_gate: None,
            opaque_world_gate_renderer: None,
            warm_world_switch_sequence: 0,
            last_warm_world_switch: None,
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
            graphics_preference_storage: None,
            graphics_preference_error: None,
            terrain_lod_preset_preference: game_terrain_lod_preset(
                scene.startup.terrain_lod_preset,
            ),
            terrain_lod_applied_preset: TerrainLodPreset::Off,
            terrain_lod_persisted_preference: None,
            terrain_lod_pending_persistence: None,
            terrain_lod_apply_error: None,
            fog_settings: GameFogSettings::default(),
            pending_leaf_detail: None,
            pending_restored_asset_pack_selection: None,
            external_asset_pack_preparation: false,
            pending_external_asset_pack_selection: None,
            external_asset_pack_operations: PlatformOperationLedger::new(),
            session,
            active_session_start_operations: PlatformOperationLedger::new(),
            session_runtime_factory: None,
            client_experience: ClientExperienceController::new(
                xr_native_client_experience_profile(),
            ),
            storage_profile_ui: StorageProfileUiState::default(),
            initial_alignment_mode: if startup_view_pose.is_some() {
                XrViewAlignmentMode::ViewPose
            } else {
                XrViewAlignmentMode::PlayerSpawn
            },
            render_options,
            season_preview: SeasonPreviewSettings::default(),
            celestial_debug: CelestialDebugSettings::default(),
            player_collision_box_visible: false,
            crosshair_visible: true,
            travel_assist_mode: GameTravelAssistMode::Off,
            selection_outline: SelectionOutlineRenderer::new(device, color_format),
            worldgen_lens: Default::default(),
            worldgen_lens_renderer: WorldColorMeshRenderer::new(device, color_format),
            world_gui_renderer,
            world_gui_overlay_renderer: WorldGuiRenderer::new(device, color_format),
            mono_gui: None,
            mono_ui_context: None,
            diagnostic_panel: XrDiagnosticPanel::new(device, color_format),
            ui,
            menu_overlay_cache: XrMenuPanelOverlayCache::default(),
            status_overlay: StatusOverlay::hidden(),
            xr_render_path_state: None,
            pending_xr_render_mode_request: None,
            sky: SkyRenderer::new_with_color_profile_and_source(
                device,
                queue,
                color_format,
                render_options.color_profile,
                asset_source,
            )
            .context("initialize XR sun texture assets")?,
            screen_effects: ScreenEffectsRenderer::new(device, queue, color_format, asset_source)
                .context("initialize XR screen effects renderer")?,
            terrain_view: None,
            terrain_horizon_diagnostic: Default::default(),
            terrain_vegetation_executor_factory:
                terrain_view::default_scene_terrain_vegetation_executor_factory(),
            underwater_effects: XrUnderwaterEffectStates::default(),
            last_underwater_update: None,
            head_comfort: XrHeadComfortState::default(),
            tracking_origin: None,
            locomotion_mode: XrLocomotionMode::default(),
            turn_policy: XrTurnPolicy::default(),
            snap_turn_state: XrSnapTurnState::default(),
            blink_teleport: XrBlinkTeleportState::default(),
            mono_blink_debug: MonoBlinkDebugState::default(),
            display_refresh_hz: None,
            render_split_timing_enabled: false,
            defer_eye_waits_enabled: false,
            overlap_runtime_prefetch_enabled: false,
            prefetched_live_upload: None,
            render_section_upload_budget: None,
            render_section_accept_budget: None,
            render_completed_result_accept_budget: None,
            per_view_uniform_frame: 0,
            last_locomotion_update: None,
            client_clock: ClientClockCadence::default(),
            player_pose_sync: PlayerPoseSyncCadence::default(),
            menu_toggle_down: false,
            game_ui_toggle_down: false,
            menu_pointer_down: false,
            menu_panel_pose: None,
            menu_panel_anchor: XrUiPanelAnchor::Head,
            menu_panel_recenter_pending: false,
            latest_xr_input: XrInputFrame::default(),
            latest_xr_head_gaze_stage: None,
            first_eye_summary: None,
            last_ui_panel_stats: WorldGuiPanelRenderStats::default(),
            last_ui_draw_cache_stats: UiDrawCacheStats::default(),
            rendered_frames: 0,
            seed_reroll: NewWorldSeedReroll::new(scene.seed),
        };
        state.refresh_world_catalog_ui(WorldCatalogUiStatus::hidden());
        state.refresh_storage_profile_ui();
        state.apply_debug_ui_screen();
        Ok(state)
    }

    /// Construct a session-free shared scene for an ordinary title-menu entry.
    ///
    /// This creates only retained render/UI resources. It does not construct a
    /// local runtime, integrated server, remote connection, or world worker.
    #[allow(clippy::too_many_arguments)]
    pub fn without_session(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        clock: MonotonicClockHandle,
        scene: McloneSceneHostOptions,
        render_options: TexturedSectionRenderOptions,
        active_assets: PreparedSceneAssets,
        client_experience_profile: ClientExperienceProfile,
        catalog_operations: Option<WorldCatalogOperationService>,
        startup_view_pose: Option<XrStartupViewPose>,
    ) -> Result<Self> {
        Self::with_optional_scene_runtime(
            device,
            queue,
            color_format,
            clock,
            scene,
            None,
            render_options,
            active_assets,
            client_experience_profile,
            catalog_operations,
            startup_view_pose,
        )
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
        Self::with_optional_scene_runtime(
            device,
            queue,
            color_format,
            clock,
            scene,
            Some(runtime),
            render_options,
            active_assets,
            client_experience_profile,
            catalog_operations,
            startup_view_pose,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn with_optional_scene_runtime(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        clock: MonotonicClockHandle,
        scene: McloneSceneHostOptions,
        runtime: Option<SceneSessionRuntime>,
        render_options: TexturedSectionRenderOptions,
        active_assets: PreparedSceneAssets,
        client_experience_profile: ClientExperienceProfile,
        catalog_operations: Option<WorldCatalogOperationService>,
        startup_view_pose: Option<XrStartupViewPose>,
    ) -> Result<Self> {
        let scene = scene.validated()?;
        let active_session = runtime
            .as_ref()
            .map(|runtime| {
                runtime
                    .active_session()
                    .cloned()
                    .context("scene runtime did not expose an active session")
            })
            .transpose()?;
        if let Some(runtime) = runtime.as_ref()
            && active_assets.mesh.catalog.len() != runtime.mesh_assets().catalog.len()
        {
            bail!(
                "prepared scene/runtime terrain catalogs disagree: prepared={} runtime={}",
                active_assets.mesh.catalog.len(),
                runtime.mesh_assets().catalog.len()
            );
        }
        let mesh_assets = active_assets.mesh.clone();
        let interest_center = runtime
            .as_ref()
            .map_or_else(|| scene.center(), SceneSessionRuntime::interest_center);
        let mut camera = SceneCameraConfig::from_scene(&scene).spawn_for_chunk(interest_center);
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
        let ui = xr_game_ui_for_session(active_session.as_ref(), scene.seed);
        let mut session = GameSessionCoordinator::new();
        if let Some(active_session) = active_session.as_ref() {
            session.complete_start(active_session.clone());
        }
        let runtime_present = runtime.is_some();
        let active_world = DrawableWorldSlot::new(
            DrawableWorldSlotInstall {
                id: WorldInstanceId::new(1),
                descriptor: active_session,
                lifecycle: if runtime_present {
                    WorldSlotLifecycle::Starting
                } else {
                    WorldSlotLifecycle::Empty
                },
                asset_epoch: active_assets.epoch,
                scene: scene.clone(),
                runtime,
                local_startup: None,
                external_runtime_startup_pending: runtime_present,
                camera,
                draw,
                actors: Some(actors),
                render_stats: RenderStreamStats::default(),
                accepted_entry_pose: None,
                pending_startup_sections: Vec::new(),
            },
            RenderAdmissionPolicy::new(
                FrameHostKind::HeadlessOffscreenPerf,
                WorkWindow::BeforeRender,
            ),
        );
        let mut state = Self {
            active_world,
            standby_world: None,
            next_world_instance_id: 2,
            warm_world_standby: None,
            prepared_warm_world_shell: None,
            lobby_launch: None,
            debug_lobby_auxiliary_player_script: scene.debug_auxiliary_player_script,
            embedded_world_preview: None,
            embedded_world_activation: EmbeddedWorldActivationState::default(),
            embedded_world_activation_sequence: 0,
            world_gate: None,
            opaque_world_gate_renderer: None,
            warm_world_switch_sequence: 0,
            last_warm_world_switch: None,
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
            graphics_preference_storage: None,
            graphics_preference_error: None,
            terrain_lod_preset_preference: game_terrain_lod_preset(
                scene.startup.terrain_lod_preset,
            ),
            terrain_lod_applied_preset: TerrainLodPreset::Off,
            terrain_lod_persisted_preference: None,
            terrain_lod_pending_persistence: None,
            terrain_lod_apply_error: None,
            fog_settings: GameFogSettings::default(),
            pending_leaf_detail: None,
            pending_restored_asset_pack_selection: None,
            external_asset_pack_preparation: false,
            pending_external_asset_pack_selection: None,
            external_asset_pack_operations: PlatformOperationLedger::new(),
            session,
            active_session_start_operations: PlatformOperationLedger::new(),
            #[cfg(not(target_arch = "wasm32"))]
            session_runtime_factory: None,
            client_experience: ClientExperienceController::new(client_experience_profile),
            storage_profile_ui: StorageProfileUiState::default(),
            initial_alignment_mode: if startup_view_pose.is_some() {
                XrViewAlignmentMode::ViewPose
            } else {
                XrViewAlignmentMode::PlayerSpawn
            },
            render_options,
            season_preview: SeasonPreviewSettings::default(),
            celestial_debug: CelestialDebugSettings::default(),
            player_collision_box_visible: false,
            crosshair_visible: true,
            travel_assist_mode: GameTravelAssistMode::Off,
            selection_outline: SelectionOutlineRenderer::new(device, color_format),
            worldgen_lens: Default::default(),
            worldgen_lens_renderer: WorldColorMeshRenderer::new(device, color_format),
            world_gui_renderer,
            world_gui_overlay_renderer: WorldGuiRenderer::new(device, color_format),
            mono_gui: None,
            mono_ui_context: None,
            diagnostic_panel: XrDiagnosticPanel::new(device, color_format),
            ui,
            menu_overlay_cache: XrMenuPanelOverlayCache::default(),
            status_overlay: StatusOverlay::hidden(),
            xr_render_path_state: None,
            pending_xr_render_mode_request: None,
            sky: SkyRenderer::new_with_color_profile(
                device,
                queue,
                color_format,
                render_options.color_profile,
            ),
            screen_effects,
            terrain_view: None,
            terrain_horizon_diagnostic: Default::default(),
            terrain_vegetation_executor_factory:
                terrain_view::default_scene_terrain_vegetation_executor_factory(),
            underwater_effects: XrUnderwaterEffectStates::default(),
            last_underwater_update: None,
            head_comfort: XrHeadComfortState::default(),
            tracking_origin: None,
            locomotion_mode: XrLocomotionMode::default(),
            turn_policy: XrTurnPolicy::default(),
            snap_turn_state: XrSnapTurnState::default(),
            blink_teleport: XrBlinkTeleportState::default(),
            mono_blink_debug: MonoBlinkDebugState::default(),
            display_refresh_hz: None,
            render_split_timing_enabled: false,
            defer_eye_waits_enabled: false,
            overlap_runtime_prefetch_enabled: false,
            prefetched_live_upload: None,
            render_section_upload_budget: None,
            render_section_accept_budget: None,
            render_completed_result_accept_budget: None,
            per_view_uniform_frame: 0,
            last_locomotion_update: None,
            client_clock: ClientClockCadence::default(),
            player_pose_sync: PlayerPoseSyncCadence::default(),
            menu_toggle_down: false,
            game_ui_toggle_down: false,
            menu_pointer_down: false,
            menu_panel_pose: None,
            menu_panel_anchor: XrUiPanelAnchor::Head,
            menu_panel_recenter_pending: false,
            latest_xr_input: XrInputFrame::default(),
            latest_xr_head_gaze_stage: None,
            first_eye_summary: None,
            last_ui_panel_stats: WorldGuiPanelRenderStats::default(),
            last_ui_draw_cache_stats: UiDrawCacheStats::default(),
            rendered_frames: 0,
            seed_reroll: NewWorldSeedReroll::new(scene.seed),
        };
        state.refresh_world_catalog_ui(WorldCatalogUiStatus::hidden());
        state.refresh_storage_profile_ui();
        state.apply_debug_ui_screen();
        Ok(state)
    }

    pub fn set_audio_output(&mut self, audio: AudioOutputCapability) {
        self.services.audio = audio;
    }

    pub fn set_teleport_preview_capability(&mut self, capability: TeleportPreviewCapability) {
        self.services.teleport_preview = capability;
    }

    pub fn set_client_entry_status(
        &mut self,
        status: Option<mclone_app_runtime::client_entry::ClientEntryStatus>,
    ) {
        self.status_overlay = status.map_or_else(StatusOverlay::hidden, |status| {
            StatusOverlay::new(status.message, status.ok)
        });
    }

    pub fn set_frame_pipeline_report(&mut self, report: Arc<FramePipelineReport>, revision: u64) {
        self.active_world
            .render_admission_policy
            .set_frame_pipeline_report(report.clone());
        self.diagnostic_panel
            .set_frame_pipeline_report(report, revision);
    }

    pub fn set_frame_pipeline_budget_signal(
        &mut self,
        signal: mclone_app_runtime::frame_pipeline_accounting::FramePipelineBudgetSignal,
    ) {
        self.active_world
            .render_admission_policy
            .set_frame_pipeline_budget_signal(signal);
    }

    pub fn clear_frame_pipeline_report(&mut self) {
        self.active_world
            .render_admission_policy
            .clear_frame_pipeline_report();
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

    /// Start an explicit native local-world destination at its adapter-owned
    /// storage path.
    ///
    /// The path stays out of `SessionStartRequest`: entry destination and
    /// platform storage configuration are separate launch axes. Callers must
    /// use the ordinary method when no direct native path was supplied.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn start_session_for_request_with_native_world_dir(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        request: SessionStartRequest,
        world_dir: &std::path::Path,
    ) -> Result<bool> {
        self.request_session_start_with_native_world_dir(request, Some(world_dir))?;
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
                let mut scene = self.active_world.scene.clone();
                project_ordinary_local_world_identity(
                    &mut scene,
                    options.seed,
                    options.world_generation_profile,
                    options.starter_content,
                );
                scene.world_dir = None;
                scene.project_terrain_presentation_for_source(true);
                Ok::<_, anyhow::Error>(scene)
            },
            |id| {
                let summary = self
                    .client_experience
                    .catalog()
                    .world_summary(id)
                    .cloned()
                    .context("local world is not present in the catalog view")?;
                let mut scene = self.active_world.scene.clone();
                project_ordinary_local_world_identity(
                    &mut scene,
                    summary.seed,
                    summary.world_generation_profile,
                    summary.starter_content,
                );
                scene.debug_passive_showcase = false;
                scene.startup.local_entry_intent = mclone_app_runtime::local_session_launch::LocalSessionEntryIntent::PersistedPlayerOrProfilePreferred;
                scene.world_dir = None;
                scene.project_terrain_presentation_for_source(true);
                Ok((
                    scene,
                    ActiveSessionDescriptor::from_local_world_summary(&summary),
                ))
            },
            |endpoint| {
                let mut scene = self.active_world.scene.clone();
                scene.world_dir = None;
                scene.project_terrain_presentation_for_source(false);
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
        let instance_id = self.allocate_world_instance_id();
        if !self.active_session_start_operations.is_empty() {
            let cancelled = self.active_session_start_operations.teardown();
            log::info!(
                "superseded {} active-session platform operation(s)",
                cancelled.len()
            );
        }
        let operation = self
            .active_session_start_operations
            .issue(instance_id, pending.request.clone());
        Some(ExternalSceneSessionStart {
            target: ExternalSceneStartTarget::ActiveSession {
                token: operation.token,
            },
            instance_id,
            storage_source: None,
            request: pending.request,
            runtime_kind: pending.payload.runtime_kind,
            local_launch_plan: (pending.payload.runtime_kind == SessionRuntimeKind::Local)
                .then(|| local_session_launch_plan(&pending.payload.options, false)),
            scene: pending.payload.options,
            descriptor: pending.payload.descriptor,
            destination: None,
        })
    }

    pub fn external_session_start_pending(&self) -> bool {
        self.session.pending_start().is_some()
    }

    pub fn take_external_runtime_start(&mut self) -> Option<ExternalSceneSessionStart> {
        self.take_external_session_start()
            .or_else(|| self.take_lobby_world_start())
    }

    pub const fn lobby_launch_active(&self) -> bool {
        self.lobby_launch.is_some()
    }

    pub fn lobby_destination_failure(&self) -> Option<&str> {
        self.lobby_launch
            .as_ref()
            .and_then(|launch| launch.destination_failure.as_deref())
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
        if !self.accept_external_scene_start(&pending) {
            log::info!(
                "dropping non-current external scene start instance={} target={:?}",
                pending.instance_id.get(),
                pending.target,
            );
            return Ok(());
        }
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

        let completion_target = pending.target.clone();
        match completion_target {
            ExternalSceneStartTarget::ActiveSession { .. }
            | ExternalSceneStartTarget::Lobby {
                role: mclone_app_runtime::scenario_content::LobbyWorldRole::Primary,
                ..
            } => {
                let movement_speed_multiplier =
                    self.active_world.camera.movement_speed_multiplier();
                let camera = SceneCameraConfig::from_scene(&pending.scene)
                    .spawn_for_chunk_with_speed(
                        runtime.interest_center(),
                        movement_speed_multiplier,
                    );
                let draw = if matches!(completion_target, ExternalSceneStartTarget::Lobby { .. }) {
                    match self.prepared_warm_world_shell.as_ref() {
                        Some(shell) => TexturedSectionDrawResources::new_with_shared_resources(
                            device,
                            queue,
                            &[],
                            shell.draw.shared_resources(),
                        ),
                        None => TexturedSectionDrawResources::new(
                            device,
                            queue,
                            self.color_format,
                            &[],
                            self.mesh_assets.atlas.as_upload(),
                        ),
                    }
                } else {
                    TexturedSectionDrawResources::new(
                        device,
                        queue,
                        self.color_format,
                        &[],
                        self.mesh_assets.atlas.as_upload(),
                    )
                }
                .context("reset scene terrain for external session start")?;
                let actors = ActorDrawResources::new_with_shared_resources(
                    device,
                    self.active_world
                        .actors
                        .as_ref()
                        .expect("active world owns actor draw state")
                        .shared_resources(),
                );
                self.active_world.install(DrawableWorldSlotInstall {
                    id: pending.instance_id,
                    descriptor: Some(active.clone()),
                    lifecycle: WorldSlotLifecycle::Starting,
                    asset_epoch: self.active_assets.epoch,
                    scene: pending.scene.clone(),
                    runtime: Some(runtime),
                    local_startup: None,
                    external_runtime_startup_pending: true,
                    camera,
                    draw,
                    actors: Some(actors),
                    render_stats: RenderStreamStats::default(),
                    accepted_entry_pose: None,
                    pending_startup_sections: Vec::new(),
                });
                self.clear_transient_world_state();
                self.clear_menu_input_state();
                self.session.complete_start(active.clone());
                self.status_overlay = StatusOverlay::hidden();
                self.apply_started_session_ui(&active);
                self.sync_player_appearance()
                    .context("sync external session player appearance")?;
            }
            ExternalSceneStartTarget::Lobby {
                role: mclone_app_runtime::scenario_content::LobbyWorldRole::Destination,
                ..
            } => {
                let mut destination = pending
                    .destination
                    .clone()
                    .context("lobby destination completion omitted its presentation")?;
                destination.destination.instance_id = Some(pending.instance_id);
                let camera = SceneCameraConfig::from_scene(&pending.scene)
                    .spawn_for_chunk(runtime.interest_center());
                self.install_prepared_warm_world_slot(
                    destination.destination,
                    pending.scene.clone(),
                    active,
                    Some(runtime),
                    None,
                    true,
                    camera,
                )?;
            }
        }
        if let ExternalSceneStartTarget::Lobby { role, .. } = completion_target {
            self.finish_lobby_world_start(role, pending.instance_id, device, queue)?;
        }
        Ok(())
    }

    fn accept_external_scene_start(&mut self, pending: &ExternalSceneSessionStart) -> bool {
        let (token, role) = match pending.target {
            ExternalSceneStartTarget::ActiveSession { token } => {
                return matches!(
                    self.active_session_start_operations.complete(
                        mclone_app_runtime::platform_operation::PlatformOperationCompletion {
                            token,
                            result: Ok::<(), String>(()),
                        },
                    ),
                    mclone_app_runtime::platform_operation::PlatformOperationResolution::Applied {
                        kind,
                        ..
                    } if kind == pending.instance_id
                );
            }
            ExternalSceneStartTarget::Lobby { token, role } => (token, role),
        };
        let Some(launch) = self.lobby_launch.as_mut() else {
            return false;
        };
        let resolution = launch.complete_start(
            mclone_app_runtime::platform_operation::PlatformOperationCompletion {
                token,
                result: Ok(()),
            },
        );
        matches!(
            resolution,
            mclone_app_runtime::platform_operation::PlatformOperationResolution::Applied {
                kind,
                ..
            } if kind.role == role && kind.instance_id == pending.instance_id
        )
    }

    fn finish_lobby_world_start(
        &mut self,
        role: mclone_app_runtime::scenario_content::LobbyWorldRole,
        instance_id: WorldInstanceId,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        let Some(mut launch) = self.lobby_launch.take() else {
            bail!("lobby world completion has no active launch");
        };
        match role {
            mclone_app_runtime::scenario_content::LobbyWorldRole::Primary => {
                launch.primary_start_token = None;
                launch.phase = LobbyLaunchPhase::PrimaryPlayable;
                self.try_issue_lobby_destination_start(&mut launch, device, queue)?;
            }
            mclone_app_runtime::scenario_content::LobbyWorldRole::Destination => {
                launch.destination_start_token = None;
            }
        }
        debug_assert!(
            match role {
                mclone_app_runtime::scenario_content::LobbyWorldRole::Primary => {
                    self.active_world.id == instance_id
                }
                mclone_app_runtime::scenario_content::LobbyWorldRole::Destination => self
                    .standby_world
                    .as_ref()
                    .is_some_and(|world| world.id == instance_id),
            },
            "accepted lobby world was not installed in its target slot"
        );
        self.lobby_launch = Some(launch);
        Ok(())
    }

    pub fn fail_external_session_start(
        &mut self,
        pending: ExternalSceneSessionStart,
        error: impl Into<String>,
    ) {
        let message = error.into();
        match pending.target {
            ExternalSceneStartTarget::Lobby { token, .. } => {
                self.fail_lobby_world_start(token, message);
                return;
            }
            ExternalSceneStartTarget::ActiveSession { token } => {
                if !matches!(
                    self.active_session_start_operations.complete(
                        mclone_app_runtime::platform_operation::PlatformOperationCompletion {
                            token,
                            result: Err::<(), _>(message.clone()),
                        },
                    ),
                    mclone_app_runtime::platform_operation::PlatformOperationResolution::Failed { .. }
                ) {
                    log::info!(
                        "ignored non-current active-session failure instance={}",
                        pending.instance_id.get()
                    );
                    return;
                }
            }
        }
        log::error!(
            "failed to start external scene session {:?}: {}",
            pending.request,
            message
        );
        self.session.fail_start(SessionFailure::for_request(
            &pending.request,
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
    pub(crate) fn local_world_options(
        &self,
        options: &LocalWorldCreateOptions,
    ) -> McloneSceneHostOptions {
        self.scene_for_storage_intent(
            SessionStorageIntent::transient_local_world_with_generation_profile_and_starter_content(
                options.seed,
                options.world_generation_profile,
                options.starter_content,
            ),
        )
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
        let mut scene =
            self.scene_for_storage_intent(SessionStorageIntent::catalog_world(summary, world_dir));
        scene.debug_passive_showcase = false;
        scene
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn scene_for_storage_intent(&self, intent: SessionStorageIntent) -> McloneSceneHostOptions {
        let mut scene = self.active_world.scene.clone();
        if let Some(seed) = intent.seed() {
            project_ordinary_local_world_identity(
                &mut scene,
                seed,
                intent.world_generation_profile(),
                intent.starter_content(),
            );
            scene.startup.local_entry_intent = intent.local_entry_intent();
        } else {
            scene.world_generation_profile = intent.world_generation_profile();
            scene.starter_content = intent.starter_content();
        }
        scene.world_dir = intent.world_dir().map(PathBuf::from);
        scene.project_terrain_presentation_for_source(intent.remote_addr().is_none());
        if intent.suppress_adaptive_chunk_publication_budget() {
            scene.adaptive_chunk_publication_budget = false;
        }
        if let Some(runtime) = &self.active_world.runtime {
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

    pub(crate) fn refresh_storage_profile_ui(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        let profile = mclone_app_runtime::local_profile::load_or_create_native_local_player_profile(
            self.active_world.scene.world_root.as_deref(),
        );
        #[cfg(target_arch = "wasm32")]
        let profile = mclone_app_runtime::local_profile::load_or_create_web_local_player_profile();

        self.storage_profile_ui = match profile {
            Ok(profile) => StorageProfileUiState::available(
                profile.id.as_bytes(),
                &profile.display_name,
                if cfg!(target_arch = "wasm32") {
                    StorageProfileBackend::BrowserLocalStorage
                } else {
                    StorageProfileBackend::NativePreferences
                },
            ),
            Err(error) => StorageProfileUiState {
                status: WorldCatalogUiStatus::new(
                    &format!("Player profile unavailable: {error:#}"),
                    false,
                ),
                ..StorageProfileUiState::default()
            },
        };
    }

    pub(crate) fn apply_local_data_effect(&mut self, effect: ClientExperienceLocalDataEffect) {
        let factory_reset = effect == ClientExperienceLocalDataEffect::FactoryReset;
        let result = match effect {
            ClientExperienceLocalDataEffect::ClearRebuildableCache => {
                self.status_overlay =
                    StatusOverlay::new("No registered rebuildable caches are available", false);
                return;
            }
            ClientExperienceLocalDataEffect::ResetPlayerIdentity => {
                #[cfg(not(target_arch = "wasm32"))]
                let result = mclone_app_runtime::local_profile::reset_native_local_player_profile(
                    self.active_world.scene.world_root.as_deref(),
                );
                #[cfg(target_arch = "wasm32")]
                let result = mclone_app_runtime::local_profile::reset_web_local_player_profile();
                result.map(|profile| {
                    (
                        profile,
                        "Player identity reset; existing world and server records remain",
                    )
                })
            }
            ClientExperienceLocalDataEffect::FactoryReset => {
                #[cfg(not(target_arch = "wasm32"))]
                let result =
                    mclone_app_runtime::local_profile::factory_reset_native_local_preferences(
                        self.active_world.scene.world_root.as_deref(),
                    );
                #[cfg(target_arch = "wasm32")]
                let result =
                    mclone_app_runtime::local_profile::factory_reset_web_local_preferences();
                result.map(|profile| {
                    (
                        profile,
                        "Factory reset complete; remote server player records remain",
                    )
                })
            }
        };

        match result {
            Ok((profile, message)) => {
                if factory_reset {
                    self.reset_terrain_lod_preference_to_platform_default();
                }
                self.storage_profile_ui = StorageProfileUiState::available(
                    profile.id.as_bytes(),
                    &profile.display_name,
                    if cfg!(target_arch = "wasm32") {
                        StorageProfileBackend::BrowserLocalStorage
                    } else {
                        StorageProfileBackend::NativePreferences
                    },
                );
                self.storage_profile_ui.status = WorldCatalogUiStatus::new(message, true);
                self.status_overlay = StatusOverlay::new(message, true);
            }
            Err(error) => {
                let message = format!("Local data operation failed: {error:#}");
                self.storage_profile_ui.status = WorldCatalogUiStatus::new(&message, false);
                self.status_overlay = StatusOverlay::new(message, false);
            }
        }
    }

    pub(crate) fn active_local_world_id(&self) -> Option<&LocalWorldId> {
        let GameSessionState::Active { session } = self.session.state() else {
            return None;
        };
        session.local_world_id()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn request_session_start(&mut self, request: SessionStartRequest) -> Result<()> {
        self.request_session_start_with_native_world_dir(request, None)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn request_session_start_with_native_world_dir(
        &mut self,
        request: SessionStartRequest,
        native_world_dir: Option<&std::path::Path>,
    ) -> Result<()> {
        let plan = plan_session_start(
            request,
            |options| {
                Ok(bind_native_entry_world_dir(
                    self.local_world_options(options),
                    native_world_dir,
                ))
            },
            |id| {
                let summary = self
                    .client_experience
                    .catalog()
                    .world_summary(id)
                    .cloned()
                    .context("local world is not present in the catalog view")?;
                let world_root = self
                    .active_world
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
        self.start_pending_session_with_world_storage(device, queue, None)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn start_pending_session_with_world_storage(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world_storage: Option<mclone_server::NativeIntegratedServerWorldStorage>,
    ) -> Result<bool> {
        let Some(pending) = self.session.take_pending_start() else {
            return Ok(false);
        };
        let request = pending.request.clone();
        match self.start_pending_session_payload_with_world_storage(
            device,
            queue,
            pending,
            world_storage,
        ) {
            Ok(SceneSessionStartOutcome::LocalStartupQueued) => Ok(false),
            Ok(SceneSessionStartOutcome::Started(descriptor)) => {
                self.session.complete_start(descriptor.clone());
                self.status_overlay = StatusOverlay::hidden();
                self.apply_started_session_ui(&descriptor);
                Ok(true)
            }
            Err(error) => {
                log::error!("failed to start XR session {request:?}: {error:#}");
                self.session.fail_start(SessionFailure::for_request(
                    &request,
                    request.default_failure_message(),
                ));
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
    fn start_pending_session_payload_with_world_storage(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pending: mclone_app_runtime::session::PendingSessionStart<ScenePendingSessionStart>,
        world_storage: Option<mclone_server::NativeIntegratedServerWorldStorage>,
    ) -> Result<SceneSessionStartOutcome> {
        let request = pending.request;
        let SessionStartPayload {
            runtime_kind,
            options: scene,
            descriptor,
        } = pending.payload;
        match runtime_kind {
            SessionRuntimeKind::Local => {
                self.begin_local_session_start_with_world_storage(
                    request,
                    Some(descriptor),
                    scene,
                    world_storage,
                )?;
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
    fn begin_local_session_start_with_world_storage(
        &mut self,
        request: SessionStartRequest,
        descriptor: Option<ActiveSessionDescriptor>,
        mut scene: McloneSceneHostOptions,
        world_storage: Option<mclone_server::NativeIntegratedServerWorldStorage>,
    ) -> Result<()> {
        if matches!(request, SessionStartRequest::OpenLocalWorld { .. }) {
            scene.debug_passive_showcase = false;
        }
        let mut scene = scene.validated()?;
        let mesh_assets = self.mesh_assets.clone();
        let mut options = local_integrated_scene_options(&scene);
        if let Some(world_storage) = world_storage {
            options = options.with_world_storage(world_storage);
        }
        scene.chunk_x = options.center.x;
        scene.chunk_z = options.center.z;
        let initial_center = options.center;
        let pump = LocalIntegratedStartupPump::with_mesh_assets(options, mesh_assets)
            .context("create XR local world startup pump")?;
        let camera = SceneCameraConfig::from_scene(&scene).spawn_for_chunk(initial_center);
        self.active_world.local_startup = Some(SceneLocalStartup {
            request: request.clone(),
            descriptor,
            scene: scene.clone(),
            pump,
            camera,
            startup_view_pose: None,
            reconciled_interest_center: None,
            reconciliation_deadline: None,
            reconciliation_passes: 0,
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

    pub(crate) fn apply_lobby_effect(
        &mut self,
        effect: ClientExperienceScenarioEffect,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<bool> {
        match effect {
            ClientExperienceScenarioEffect::Launch(intent) => self.begin_lobby_launch(intent),
        }
    }

    pub fn begin_lobby_launch(
        &mut self,
        intent: mclone_app_runtime::scenario::ScenarioLaunchIntent,
    ) -> Result<bool> {
        if self.lobby_launch.is_some() {
            log::info!("ignoring repeated lobby launch");
            return Ok(false);
        }
        if self.prepared_warm_world_shell.is_none()
            && self.standby_world.is_none()
            && self.warm_world_standby.as_ref().is_some_and(|state| {
                matches!(
                    state.phase,
                    WarmWorldStandbyPhase::Failed | WarmWorldStandbyPhase::Cancelled
                )
            })
        {
            self.warm_world_standby = None;
            self.world_gate = None;
            log::info!("cleared terminal retained-world diagnostics for scenario relaunch");
        }
        if self.prepared_warm_world_shell.is_some()
            || self.warm_world_standby.is_some()
            || self.standby_world.is_some()
        {
            bail!("a retained-world scenario is already active");
        }
        let mut launch = LobbyLaunchState::new(intent);
        let fixture = launch.content.primary.kind;
        let primary = self.prepare_lobby_primary(&launch.content)?;
        let descriptor = ActiveSessionDescriptor::new_seed_local_world(primary.seed);
        let start = LobbyWorldStart {
            instance_id: self.allocate_world_instance_id(),
            role: mclone_app_runtime::scenario_content::LobbyWorldRole::Primary,
            storage_source:
                mclone_app_runtime::scenario_content::LobbyWorldSource::TransientAuthored(fixture),
            scene: primary,
            descriptor,
            destination: None,
        };
        let token = launch.issue_start(start);
        launch.primary_start_token = Some(token);
        launch.phase = LobbyLaunchPhase::StartingPrimary;
        self.ui.close();
        self.try_resolve_lobby_destination(&mut launch)?;
        self.lobby_launch = Some(launch);
        self.status_overlay = StatusOverlay::new("Preparing Lobby", true);
        log::info!("lobby requested with transient authored primary");
        Ok(false)
    }

    pub fn take_lobby_world_start(&mut self) -> Option<ExternalSceneSessionStart> {
        let operation = self
            .lobby_launch
            .as_mut()
            .and_then(LobbyLaunchState::take_start_request)?;
        let start = operation.kind;
        let request = start
            .descriptor
            .local_world_id()
            .cloned()
            .map(SessionStartRequest::open_local_world)
            .unwrap_or_else(|| {
                SessionStartRequest::new_seed_local_world_with_generation_profile_and_starter_content(
                    start.scene.seed,
                    start.scene.world_generation_profile,
                    start.scene.starter_content,
                )
            });
        Some(ExternalSceneSessionStart {
            target: ExternalSceneStartTarget::Lobby {
                token: operation.token,
                role: start.role,
            },
            instance_id: start.instance_id,
            storage_source: Some(start.storage_source),
            request,
            runtime_kind: SessionRuntimeKind::Local,
            local_launch_plan: Some(local_session_launch_plan(
                &start.scene,
                matches!(
                    start.role,
                    mclone_app_runtime::scenario_content::LobbyWorldRole::Destination
                ),
            )),
            scene: start.scene,
            descriptor: start.descriptor,
            destination: start.destination,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_lobby_launch(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<()> {
        if self.lobby_launch.is_none() {
            return Ok(());
        }
        if let Some(mut launch) = self.lobby_launch.take() {
            self.try_resolve_lobby_destination(&mut launch)?;
            self.lobby_launch = Some(launch);
        }
        self.start_native_lobby_world_requests(device, queue)?;
        Ok(())
    }

    fn prepare_lobby_primary(
        &self,
        content: &mclone_app_runtime::scenario_content::LobbyScenarioContent,
    ) -> Result<McloneSceneHostOptions> {
        let primary_fixture = &content.primary;

        let mut primary = self.active_world.scene.clone();
        primary.seed = primary_fixture.seed;
        primary.chunk_x = primary_fixture.center_chunk[0];
        primary.chunk_z = primary_fixture.center_chunk[1];
        primary.remote_addr = None;
        primary.world_dir = None;
        primary.world_generation_profile = primary_fixture.world_generation_profile;
        primary.world_behavior_profile = mclone_server::WorldBehaviorProfile::ProtectedLobby;
        primary.startup.local_entry_intent =
            mclone_app_runtime::local_session_launch::LocalSessionEntryIntent::AuthoredCoordinate;
        primary.freeze_scheduled_fluid_ticks = true;
        primary.debug_passive_showcase = false;
        primary.debug_auxiliary_player_script = false;
        primary.project_terrain_presentation_for_source(true);
        primary.validated()
    }

    fn prepare_lobby_fallback_destination(
        &self,
        content: &mclone_app_runtime::scenario_content::LobbyScenarioContent,
        private_world_key: mclone_app_runtime::scenario_content::AppPrivateWorldKey,
        preview_bounds: mclone_app_runtime::scenario::ScenarioPreviewBounds,
    ) -> Result<PreparedEmbeddedWorldScenario> {
        let primary_fixture = &content.primary;
        let fallback = &content.fallback;
        let center_chunk = fallback.center_chunk;
        let center = ChunkPos::new(center_chunk[0], center_chunk[1]);
        let preview_anchor = vec3d_from_array(fallback.preview_anchor);
        let anchor_section_y = mclone_core::block_to_section_coord(
            preview_anchor
                .y
                .floor()
                .clamp(i32::MIN as f64, i32::MAX as f64) as i32,
        );
        let region = preview_bounds.resolve(center, anchor_section_y)?;
        let primary_anchor = vec3d_from_array(primary_fixture.preview_anchor);
        let primary_anchor_section_y = mclone_core::block_to_section_coord(
            primary_anchor
                .y
                .floor()
                .clamp(i32::MIN as f64, i32::MAX as f64) as i32,
        );
        let return_region = preview_bounds.resolve(
            ChunkPos::new(
                primary_fixture.center_chunk[0],
                primary_fixture.center_chunk[1],
            ),
            primary_anchor_section_y,
        )?;
        let placement = mclone_render::placement::WorldPlacement::new(
            preview_anchor,
            primary_anchor,
            1.0 / 8.0,
        )?;
        let return_placement = mclone_render::placement::WorldPlacement::new(
            primary_anchor,
            vec3d_from_array(fallback.preview_display_anchor),
            1.0 / 8.0,
        )?;
        let descriptor = ActiveSessionDescriptor::new_seed_local_world(fallback.seed);
        let destination = WarmWorldStandbyRequest::new(fallback.seed, center)
            .with_storage_source(
                mclone_app_runtime::scenario_content::LobbyWorldSource::AppPrivate(
                    private_world_key,
                ),
                fallback.world_generation_profile,
            )
            .with_descriptor(descriptor)
            .with_world_behavior_profile(fallback.behavior_profile)
            .with_embedded_preview_regions(region, return_region, placement, return_placement)
            .with_entry_relative_embedded_preview(
                preview_bounds,
                primary_anchor,
                LOBBY_RETURN_PREVIEW_DISPLAY_OFFSET,
            )?;
        Ok(PreparedEmbeddedWorldScenario::new(
            content.scenario_id,
            destination,
        ))
    }

    fn prepare_catalog_lobby_destination(
        content: &mclone_app_runtime::scenario_content::LobbyScenarioContent,
        summary: &LocalWorldSummary,
        preview_bounds: mclone_app_runtime::scenario::ScenarioPreviewBounds,
    ) -> Result<PreparedEmbeddedWorldScenario> {
        let primary_fixture = &content.primary;
        let center = mclone_server::initial_spawn_center_for_profile(
            summary.seed,
            summary.world_generation_profile,
        );
        let preview_anchor = Vec3d::new(
            f64::from(center.x * 16 + 8),
            96.0,
            f64::from(center.z * 16 + 8),
        );
        let anchor_section_y = mclone_core::block_to_section_coord(96);
        let region = preview_bounds.resolve(center, anchor_section_y)?;
        let primary_anchor = vec3d_from_array(primary_fixture.preview_anchor);
        let primary_anchor_section_y = mclone_core::block_to_section_coord(
            primary_anchor
                .y
                .floor()
                .clamp(i32::MIN as f64, i32::MAX as f64) as i32,
        );
        let return_region = preview_bounds.resolve(
            ChunkPos::new(
                primary_fixture.center_chunk[0],
                primary_fixture.center_chunk[1],
            ),
            primary_anchor_section_y,
        )?;
        let placement = mclone_render::placement::WorldPlacement::new(
            preview_anchor,
            primary_anchor,
            1.0 / 8.0,
        )?;
        let return_placement = mclone_render::placement::WorldPlacement::new(
            primary_anchor,
            preview_anchor,
            1.0 / 8.0,
        )?;
        let destination = WarmWorldStandbyRequest::new(summary.seed, center)
            .with_storage_source(
                mclone_app_runtime::scenario_content::LobbyWorldSource::Catalog(summary.id.clone()),
                summary.world_generation_profile,
            )
            .with_descriptor(ActiveSessionDescriptor::from_local_world_summary(summary))
            .with_starter_content(summary.starter_content)
            .with_world_behavior_profile(mclone_server::WorldBehaviorProfile::Mutable)
            .with_embedded_preview_regions(region, return_region, placement, return_placement)
            .with_entry_relative_embedded_preview(
                preview_bounds,
                primary_anchor,
                LOBBY_RETURN_PREVIEW_DISPLAY_OFFSET,
            )?;
        Ok(PreparedEmbeddedWorldScenario::new(
            content.scenario_id,
            destination,
        ))
    }

    fn try_resolve_lobby_destination(&mut self, launch: &mut LobbyLaunchState) -> Result<()> {
        if launch.destination.is_some() || launch.destination_failure.is_some() {
            return Ok(());
        }
        let catalog = self.client_experience.catalog();
        if catalog.world_list_pending() {
            return Ok(());
        }
        if let Some(summary) = catalog.most_recent_compatible_world().cloned() {
            let destination = Self::prepare_catalog_lobby_destination(
                &launch.content,
                &summary,
                launch.intent.preview_bounds,
            )?;
            log::info!(
                "selected recent lobby destination source=catalog id={} seed={}",
                summary.id,
                summary.seed
            );
            launch.destination = Some(destination);
        } else {
            let destination = self.prepare_lobby_fallback_destination(
                &launch.content,
                mclone_app_runtime::scenario_content::AppPrivateWorldKey::LobbyFallback,
                launch.intent.preview_bounds,
            )?;
            launch.destination = Some(destination);
            log::info!(
                "selected lobby destination source=app-private seed={}",
                mclone_app_runtime::scenario_content::LOBBY_PREVIEW_FALLBACK_SEED
            );
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn start_native_lobby_world_requests(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        while let Some(start) = self.take_lobby_world_start() {
            let ExternalSceneStartTarget::Lobby { token, role } = start.target else {
                unreachable!("lobby queue produced an ordinary session start");
            };
            match role {
                mclone_app_runtime::scenario_content::LobbyWorldRole::Primary => {
                    let fixture = start
                        .storage_source
                        .as_ref()
                        .and_then(|source| source.authored_fixture())
                        .context("native lobby primary omitted its authored fixture")?;
                    self.active_world.id = start.instance_id;
                    self.session.request_start(
                        start.request.clone(),
                        ScenePendingSessionStart {
                            runtime_kind: SessionRuntimeKind::Local,
                            options: start.scene,
                            descriptor: start.descriptor,
                        },
                    );
                    if let Err(error) = self.start_pending_session_with_world_storage(
                        device,
                        queue,
                        Some(
                            mclone_server::NativeIntegratedServerWorldStorage::TransientAuthored {
                                fixture,
                            },
                        ),
                    ) {
                        self.fail_lobby_world_start(token, error.to_string());
                    }
                }
                mclone_app_runtime::scenario_content::LobbyWorldRole::Destination => {
                    let destination = start
                        .destination
                        .context("lobby destination start omitted its presentation")?;
                    if let Err(error) = self.begin_prepared_embedded_world_scenario(destination) {
                        self.fail_lobby_world_start(token, error.to_string());
                    }
                }
            }
        }
        Ok(())
    }

    fn fail_lobby_world_start(
        &mut self,
        token: mclone_app_runtime::platform_operation::PlatformOperationToken,
        error: String,
    ) {
        let Some(mut launch) = self.lobby_launch.take() else {
            return;
        };
        let resolution = launch.complete_start(
            mclone_app_runtime::platform_operation::PlatformOperationCompletion {
                token,
                result: Err(error.clone()),
            },
        );
        if let mclone_app_runtime::platform_operation::PlatformOperationResolution::Failed {
            kind,
            ..
        } = resolution
        {
            match kind.role {
                mclone_app_runtime::scenario_content::LobbyWorldRole::Primary => {
                    let _ = launch.cancel();
                    self.prepared_warm_world_shell = None;
                    self.ui.set_screen(Some(GameScreen::Title));
                    self.status_overlay =
                        StatusOverlay::new(format!("Lobby startup failed: {error}"), false);
                    return;
                }
                mclone_app_runtime::scenario_content::LobbyWorldRole::Destination => {
                    log::warn!("lobby destination start rejected: {error}");
                    launch.destination_failure = Some(error);
                    launch.phase = LobbyLaunchPhase::DestinationFailed;
                }
            }
        }
        self.lobby_launch = Some(launch);
    }

    fn advance_lobby_after_primary(
        &mut self,
        active_completed: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        let Some(mut launch) = self.lobby_launch.take() else {
            return;
        };
        if launch.phase == LobbyLaunchPhase::StartingPrimary {
            if active_completed {
                if let Some(token) = launch.primary_start_token.take() {
                    match launch.complete_start(
                        mclone_app_runtime::platform_operation::PlatformOperationCompletion {
                            token,
                            result: Ok(()),
                        },
                    ) {
                        mclone_app_runtime::platform_operation::PlatformOperationResolution::Applied {
                            kind,
                            ..
                        } if kind.instance_id == self.active_world.id => {
                            launch.phase = LobbyLaunchPhase::PrimaryPlayable;
                        }
                        _ => {
                            launch.destination_failure =
                                Some("primary start completion was stale or mismatched".to_owned());
                        }
                    }
                }
            } else if matches!(self.session.state(), GameSessionState::Failed { .. }) {
                let _ = launch.cancel();
                self.prepared_warm_world_shell = None;
                self.ui.set_screen(Some(GameScreen::Title));
                return;
            }
        }
        if let Err(error) = self.try_issue_lobby_destination_start(&mut launch, device, queue) {
            launch.destination_failure = Some(format!("prepare destination start: {error:#}"));
        }
        self.lobby_launch = Some(launch);
    }

    fn try_issue_lobby_destination_start(
        &mut self,
        launch: &mut LobbyLaunchState,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        if launch.phase != LobbyLaunchPhase::PrimaryPlayable {
            return Ok(());
        }
        if let Some(error) = launch.destination_failure.take() {
            log::warn!("lobby destination startup failed: {error}");
            self.prepared_warm_world_shell = None;
            launch.phase = LobbyLaunchPhase::DestinationFailed;
            self.status_overlay =
                StatusOverlay::new(format!("Lobby preview unavailable: {error}"), false);
            return Ok(());
        }
        let Some(destination) = launch.destination.take() else {
            return Ok(());
        };
        if let Err(error) = self.prepare_embedded_world_scenario_shell(device, queue, &destination)
        {
            let error =
                format!("prepare destination renderer shell after lobby startup: {error:#}");
            log::warn!("lobby destination startup failed: {error}");
            self.prepared_warm_world_shell = None;
            launch.destination = Some(destination);
            launch.phase = LobbyLaunchPhase::DestinationFailed;
            self.status_overlay =
                StatusOverlay::new(format!("Lobby preview unavailable: {error}"), false);
            return Ok(());
        }
        let storage_source = destination
            .destination
            .storage_source
            .clone()
            .context("scenario destination start omitted its storage source")?;
        let mut scene = self.active_world.scene.clone();
        scene.seed = destination.destination.seed;
        scene.chunk_x = destination.destination.entry_center.x;
        scene.chunk_z = destination.destination.entry_center.z;
        scene.remote_addr = None;
        scene.world_dir = None;
        scene.world_behavior_profile = destination.destination.world_behavior_profile;
        scene.world_generation_profile = destination.destination.world_generation_profile;
        scene.starter_content = destination.destination.starter_content;
        scene.startup.local_entry_intent =
            mclone_app_runtime::local_session_launch::LocalSessionEntryIntent::AuthoredCoordinate;
        scene.debug_passive_showcase = self.debug_lobby_auxiliary_player_script
            && storage_source.allows_runtime_actor_authoring();
        scene.debug_auxiliary_player_script = self.debug_lobby_auxiliary_player_script;
        scene.project_terrain_presentation_for_source(true);
        let scene = scene.validated()?;
        let instance_id = self.allocate_world_instance_id();
        let mut destination = destination;
        destination.destination.instance_id = Some(instance_id);
        let descriptor = destination
            .destination
            .descriptor
            .clone()
            .unwrap_or_else(|| ActiveSessionDescriptor::new_seed_local_world(scene.seed));
        let start = LobbyWorldStart {
            instance_id,
            role: mclone_app_runtime::scenario_content::LobbyWorldRole::Destination,
            storage_source,
            descriptor,
            scene,
            destination: Some(destination),
        };
        let token = launch.issue_start(start);
        launch.destination_start_token = Some(token);
        launch.phase = LobbyLaunchPhase::WarmingDestination;
        Ok(())
    }

    fn update_lobby_destination_status(&mut self) {
        let Some(launch) = self.lobby_launch.as_mut() else {
            return;
        };
        if launch.phase != LobbyLaunchPhase::WarmingDestination {
            return;
        }
        let Some(standby) = self.warm_world_standby.as_ref() else {
            return;
        };
        if self
            .standby_world
            .as_ref()
            .is_some_and(|slot| slot.runtime.is_some())
            && let Some(token) = launch.destination_start_token.take()
        {
            let _ = launch.complete_start(
                mclone_app_runtime::platform_operation::PlatformOperationCompletion {
                    token,
                    result: Ok(()),
                },
            );
        }
        match standby.phase {
            WarmWorldStandbyPhase::Switchable => {
                launch.phase = LobbyLaunchPhase::PreviewReady;
            }
            WarmWorldStandbyPhase::Failed | WarmWorldStandbyPhase::Cancelled => {
                launch.phase = LobbyLaunchPhase::DestinationFailed;
                self.status_overlay = StatusOverlay::new(
                    format!(
                        "Lobby preview unavailable: {}",
                        standby
                            .failure
                            .as_deref()
                            .unwrap_or("destination startup failed")
                    ),
                    false,
                );
            }
            _ => {}
        }
    }

    /// Prepare the per-world mutable renderer shell while a loading cover still
    /// owns presentation. Compatible atlas and direct-terrain pipelines remain
    /// shared with the active slot. This performs no destination storage or
    /// runtime work.
    pub fn prepare_warm_world_standby_shell(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        presentation: WarmWorldPresentationRequest,
    ) -> Result<()> {
        if self.prepared_warm_world_shell.is_some()
            || self.warm_world_standby.is_some()
            || self.standby_world.is_some()
        {
            bail!("a warm-world standby request or prepared shell already exists");
        }
        if matches!(
            self.active_world.descriptor,
            Some(ActiveSessionDescriptor::Remote { .. })
        ) {
            bail!("warm-world standby currently requires an active local world");
        }

        let shell_started_at = self.services.clock.now();
        let draw = TexturedSectionDrawResources::new_with_shared_resources(
            device,
            queue,
            &[],
            self.active_world.draw.shared_resources(),
        )
        .context("initialize detached standby terrain renderer shell")?;
        let renderer_shell_create_ms =
            elapsed_ms(self.services.clock.elapsed_since(shell_started_at));
        let renderer_multiview_required = device.features().contains(wgpu::Features::MULTIVIEW);
        let gate_renderer = matches!(presentation, WarmWorldPresentationRequest::OpaqueGate)
            .then(|| OpaqueWorldGateRenderer::new(device, self.color_format));
        let placed_renderer = matches!(presentation, WarmWorldPresentationRequest::Diorama { .. })
            .then(|| draw.create_placed_renderer(device));
        let multiview_started_at = self.services.clock.now();
        if renderer_multiview_required {
            self.active_world
                .draw
                .materialize_multiview_renderer(device)
                .context("materialize active multiview terrain pipelines for return standby")?;
            draw.materialize_multiview_renderer(device)
                .context("materialize detached standby multiview terrain pipelines")?;
            if let Some(gate_renderer) = gate_renderer.as_ref() {
                gate_renderer
                    .materialize_multiview_renderer(device)
                    .context("materialize opaque world gate multiview pipeline")?;
            }
            if let Some(placed_renderer) = placed_renderer.as_ref() {
                draw.materialize_placed_multiview_renderer(device, placed_renderer)
                    .context("materialize embedded-world placed terrain multiview pipelines")?;
            }
        }
        let renderer_multiview_create_ms = renderer_multiview_required
            .then(|| elapsed_ms(self.services.clock.elapsed_since(multiview_started_at)))
            .unwrap_or(0.0);
        let renderer_multiview_materialized = draw.multiview_renderer_materialized()
            && gate_renderer
                .as_ref()
                .is_none_or(OpaqueWorldGateRenderer::multiview_renderer_materialized)
            && placed_renderer
                .as_ref()
                .is_none_or(|renderer| renderer.multiview_renderer_materialized());
        let placed_renderer_topology_ready = placed_renderer.as_ref().is_some_and(|renderer| {
            !renderer_multiview_required || renderer.multiview_renderer_materialized()
        });
        self.prepared_warm_world_shell = Some(PreparedWarmWorldRendererShell {
            presentation,
            draw,
            gate_renderer,
            placed_renderer,
            renderer_shell_create_ms,
            renderer_multiview_create_ms,
            renderer_multiview_required,
            renderer_multiview_materialized,
            placed_renderer_topology_ready,
        });
        Ok(())
    }

    /// Compatibility convenience for callers that already have a prepared
    /// local leaf and want shell preparation plus runtime startup together.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn begin_warm_world_standby(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        request: WarmWorldStandbyRequest,
    ) -> Result<()> {
        self.prepare_warm_world_standby_shell(device, queue, request.presentation)?;
        if let Err(error) = self.begin_prepared_warm_world_standby(request) {
            self.prepared_warm_world_shell = None;
            return Err(error);
        }
        Ok(())
    }

    /// Execute the one scene-owned embedded-world scenario seam.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn begin_embedded_world_scenario(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scenario: PreparedEmbeddedWorldScenario,
    ) -> Result<()> {
        self.prepare_embedded_world_scenario_shell(device, queue, &scenario)?;
        if let Err(error) = self.begin_prepared_embedded_world_scenario(scenario) {
            self.prepared_warm_world_shell = None;
            return Err(error);
        }
        Ok(())
    }

    /// Native diagnostic adapter for an explicitly authored persistent world.
    /// The shared scenario request remains path-free; only native assembly
    /// resolves its storage handle here.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn begin_native_embedded_world_scenario(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scenario: PreparedEmbeddedWorldScenario,
        world_dir: &std::path::Path,
    ) -> Result<()> {
        self.prepare_embedded_world_scenario_shell(device, queue, &scenario)?;
        if let Err(error) = self.begin_prepared_warm_world_standby_with_native_world_dir(
            scenario.destination,
            Some(world_dir),
        ) {
            self.prepared_warm_world_shell = None;
            return Err(error);
        }
        Ok(())
    }

    pub fn prepare_embedded_world_scenario_shell(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scenario: &PreparedEmbeddedWorldScenario,
    ) -> Result<()> {
        match scenario.id {
            mclone_app_runtime::scenario::BuiltInScenarioId::LobbyPreview => {}
        }
        if !matches!(
            scenario.destination.presentation,
            WarmWorldPresentationRequest::Diorama { .. }
        ) {
            bail!("embedded-world scenario requires a diorama presentation");
        }
        self.prepare_warm_world_standby_shell(device, queue, scenario.destination.presentation)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn begin_prepared_embedded_world_scenario(
        &mut self,
        scenario: PreparedEmbeddedWorldScenario,
    ) -> Result<()> {
        match scenario.id {
            mclone_app_runtime::scenario::BuiltInScenarioId::LobbyPreview => {}
        }
        if !matches!(
            scenario.destination.presentation,
            WarmWorldPresentationRequest::Diorama { .. }
        ) {
            bail!("embedded-world scenario requires a diorama presentation");
        }
        self.begin_prepared_warm_world_standby(scenario.destination)
    }

    /// Attach destination startup to an already-created renderer shell.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn begin_prepared_warm_world_standby(
        &mut self,
        request: WarmWorldStandbyRequest,
    ) -> Result<()> {
        self.begin_prepared_warm_world_standby_with_native_world_dir(request, None)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn begin_prepared_warm_world_standby_with_native_world_dir(
        &mut self,
        request: WarmWorldStandbyRequest,
        native_world_dir: Option<&std::path::Path>,
    ) -> Result<()> {
        if self.warm_world_standby.is_some() || self.standby_world.is_some() {
            bail!("a warm-world standby request already exists");
        }
        let prepared_presentation = self
            .prepared_warm_world_shell
            .as_ref()
            .map(|shell| shell.presentation)
            .ok_or_else(|| anyhow!("warm-world renderer shell was not prepared"))?;
        if prepared_presentation != request.presentation {
            bail!("prepared warm-world renderer presentation does not match request");
        }
        if matches!(
            self.active_world.descriptor,
            Some(ActiveSessionDescriptor::Remote { .. })
        ) {
            bail!("warm-world standby currently requires an active local world");
        }
        if self.active_world.scene.seed == request.seed {
            bail!("warm-world standby seed must differ from the active seed");
        }
        let mut scene = self.active_world.scene.clone();
        let catalog_world_root = scene.world_root.clone();
        scene.seed = request.seed;
        scene.chunk_x = request.entry_center.x;
        scene.chunk_z = request.entry_center.z;
        scene.remote_addr = None;
        scene.world_root = None;
        scene.world_dir = match (native_world_dir, request.storage_source.as_ref()) {
            (Some(world_dir), _) => Some(world_dir.to_owned()),
            (
                None,
                Some(mclone_app_runtime::scenario_content::LobbyWorldSource::AppPrivate(key)),
            ) => Some(native_app_private_world_dir(
                catalog_world_root
                    .as_deref()
                    .context("app-private destination requires a native world root")?,
                *key,
            )),
            (None, Some(mclone_app_runtime::scenario_content::LobbyWorldSource::Catalog(id))) => {
                Some(
                    catalog_world_root
                        .as_deref()
                        .context("catalog destination requires a native world root")?
                        .join(id.as_str()),
                )
            }
            (
                None,
                Some(mclone_app_runtime::scenario_content::LobbyWorldSource::TransientAuthored(_)),
            ) => None,
            (None, None) => None,
        };
        scene.world_behavior_profile = request.world_behavior_profile;
        scene.world_generation_profile = request.world_generation_profile;
        scene.starter_content = request.starter_content;
        scene.startup.local_entry_intent =
            mclone_app_runtime::local_session_launch::LocalSessionEntryIntent::AuthoredCoordinate;
        scene.debug_passive_showcase = self.debug_lobby_auxiliary_player_script
            && native_world_dir.is_none()
            && request
                .storage_source
                .as_ref()
                .is_none_or(|source| source.allows_runtime_actor_authoring());
        scene.debug_auxiliary_player_script = self.debug_lobby_auxiliary_player_script;
        scene.project_terrain_presentation_for_source(true);
        let scene = scene.validated()?;
        let descriptor = request
            .descriptor
            .clone()
            .unwrap_or_else(|| ActiveSessionDescriptor::new_seed_local_world(request.seed));
        let startup_request = descriptor
            .local_world_id()
            .cloned()
            .map(SessionStartRequest::open_local_world)
            .unwrap_or_else(|| {
                SessionStartRequest::new_seed_local_world_with_generation_profile_and_starter_content(
                    request.seed,
                    request.world_generation_profile,
                    request.starter_content,
                )
            });
        let camera = SceneCameraConfig::from_scene(&scene).spawn_for_chunk(request.entry_center);

        let observer_only = matches!(
            request.presentation,
            WarmWorldPresentationRequest::Diorama { .. }
        );
        let pump = LocalIntegratedStartupPump::with_mesh_assets(
            local_integrated_scene_options(&scene).with_observer_only(observer_only),
            self.mesh_assets.clone(),
        )
        .context("create detached standby local startup pump")?;
        let local_startup = SceneLocalStartup {
            request: startup_request,
            descriptor: Some(descriptor.clone()),
            scene: scene.clone(),
            pump,
            camera: camera.clone(),
            startup_view_pose: None,
            reconciled_interest_center: None,
            reconciliation_deadline: None,
            reconciliation_passes: 0,
        };
        self.install_prepared_warm_world_slot(
            request,
            scene,
            descriptor,
            None,
            Some(local_startup),
            false,
            camera,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn install_prepared_warm_world_slot(
        &mut self,
        request: WarmWorldStandbyRequest,
        scene: McloneSceneHostOptions,
        descriptor: ActiveSessionDescriptor,
        runtime: Option<SceneSessionRuntime>,
        local_startup: Option<SceneLocalStartup>,
        external_runtime_startup_pending: bool,
        camera: EngineCameraController,
    ) -> Result<()> {
        if self.warm_world_standby.is_some() || self.standby_world.is_some() {
            bail!("a warm-world standby request already exists");
        }
        let prepared_presentation = self
            .prepared_warm_world_shell
            .as_ref()
            .map(|shell| shell.presentation)
            .ok_or_else(|| anyhow!("warm-world renderer shell was not prepared"))?;
        if prepared_presentation != request.presentation {
            bail!("prepared warm-world renderer presentation does not match request");
        }
        if matches!(
            self.active_world.descriptor,
            Some(ActiveSessionDescriptor::Remote { .. })
        ) {
            bail!("warm-world standby currently requires an active local world");
        }
        if self.active_world.scene.seed == request.seed {
            bail!("warm-world standby seed must differ from the active seed");
        }
        let standby_cadence = request.standby_cadence.unwrap_or(scene.simulation_cadence);
        if !standby_cadence.is_valid() {
            bail!("warm-world standby cadence must be valid");
        }
        let presentation = request.presentation;
        let preview_boundary_warning = matches!(presentation, WarmWorldPresentationRequest::Diorama { .. })
            .then(|| request.world_generation_profile.authored_missing_chunk().is_none())
            .filter(|warn| *warn)
            .map(|_| {
                "non-authored preview uses a hard region edge; canonical neighbor-culled faces may be exposed"
                    .to_owned()
            });
        let started_at = self.services.clock.now();
        let PreparedWarmWorldRendererShell {
            presentation: _,
            draw,
            gate_renderer,
            placed_renderer,
            renderer_shell_create_ms,
            renderer_multiview_create_ms,
            renderer_multiview_required,
            renderer_multiview_materialized,
            placed_renderer_topology_ready,
        } = self
            .prepared_warm_world_shell
            .take()
            .expect("prepared shell presence checked before fallible startup work");
        let shared_terrain_resource_owner_count = draw.shared_resource_owner_count();
        let instance_id = request
            .instance_id
            .unwrap_or_else(|| self.allocate_world_instance_id());
        let asset_epoch = self.active_assets.epoch;

        self.standby_world = Some(DrawableWorldSlot::new(
            DrawableWorldSlotInstall {
                id: instance_id,
                descriptor: Some(descriptor.clone()),
                lifecycle: WorldSlotLifecycle::Starting,
                asset_epoch,
                scene: scene.clone(),
                runtime,
                local_startup,
                external_runtime_startup_pending,
                camera,
                draw,
                actors: None,
                render_stats: RenderStreamStats::default(),
                accepted_entry_pose: None,
                pending_startup_sections: Vec::new(),
            },
            RenderAdmissionPolicy::new(
                FrameHostKind::HeadlessOffscreenPerf,
                WorkWindow::BeforeRender,
            ),
        ));
        self.warm_world_standby = Some(WarmWorldStandbyState {
            instance_id,
            seed: request.seed,
            presentation,
            phase: WarmWorldStandbyPhase::Warming,
            started_at,
            renderer_shell_create_ms,
            renderer_multiview_create_ms,
            renderer_multiview_required,
            renderer_multiview_materialized,
            atlas_size: [self.mesh_assets.atlas.width, self.mesh_assets.atlas.height],
            atlas_base_bytes: self.mesh_assets.atlas.byte_len(),
            duplicated_atlas_base_bytes: 0,
            shared_terrain_resource_owner_count,
            actor_state_materialized: false,
            shared_actor_resource_owner_count: 1,
            shared_actor_known_retained_bytes: self
                .active_world
                .actors
                .as_ref()
                .expect("active world owns actor draw state")
                .resource_snapshot()
                .shared_known_retained_bytes,
            standby_actor_state_allocated_bytes: 0,
            asset_epoch,
            standby_cadence,
            standby_cadence_applied: false,
            poll_count: 0,
            poll_ms: 0.0,
            startup_advance_count: 0,
            startup_advance_total_ms: 0.0,
            last_advance_ms: 0.0,
            worst_advance_ms: 0.0,
            worst_startup_step_ms: 0.0,
            worst_runtime_poll_ms: 0.0,
            endpoint_resolution_ms: 0.0,
            gpu_warm_started_at: None,
            gpu_ready_at: None,
            upload_queue_nonempty_since: None,
            last_gpu_advance_ms: 0.0,
            gpu_advance_total_ms: 0.0,
            worst_gpu_advance_ms: 0.0,
            gpu_advance_count: 0,
            gpu_ready_advance_count: 0,
            gpu_skipped_no_slack_count: 0,
            last_gpu_advance_frame: None,
            camera_reconciled: false,
            loaded_chunks: 0,
            startup_seed_sections: 0,
            startup_seed_drawable_sections: 0,
            startup_seed_owned_bytes: 0,
            initial_upload_lifecycle_items: 0,
            initial_upload_applied_lifecycle_items: 0,
            initial_upload_released_compile_jobs: 0,
            queued_upload_sections: 0,
            queued_upload_lifecycle_items: 0,
            queued_upload_mesh_owned_bytes: 0,
            gpu_section_count: 0,
            gpu_vertex_count: 0,
            gpu_index_count: 0,
            accepted_compile_result_count: 0,
            released_compile_job_count: 0,
            readiness: WarmWorldReadiness {
                renderer_topology_ready: !renderer_multiview_required
                    || renderer_multiview_materialized,
                ..WarmWorldReadiness::default()
            },
            accepted_entry_pose: None,
            source_endpoint: None,
            destination_endpoint: None,
            failure: None,
        });
        self.opaque_world_gate_renderer = gate_renderer;
        self.embedded_world_preview = match (presentation, placed_renderer) {
            (
                WarmWorldPresentationRequest::Diorama {
                    region,
                    return_region,
                    placement,
                    return_placement,
                    ..
                },
                Some(renderer),
            ) => Some(EmbeddedWorldPreview {
                source_world: instance_id,
                region,
                return_region,
                context: mclone_render::placement::WorldCompositionContext::unbounded(
                    placement,
                    Some(region.source_bounds()),
                ),
                return_context: mclone_render::placement::WorldCompositionContext::unbounded(
                    return_placement,
                    Some(return_region.source_bounds()),
                ),
                asset_epoch,
                phase: EmbeddedWorldPreviewPhase::Warming,
                renderer,
                renderer_topology_ready: placed_renderer_topology_ready,
                source_anchor_gpu_resident: false,
                source_anchor_traversal_ready: false,
                bounded_section_count: 0,
                last_draw: TexturedSectionRenderStats::default(),
                source_host_mode: None,
                // Even-sized preview bounds do not have a unique geometric
                // center chunk. Keep streaming interest pinned to the
                // scenario entry anchor instead of silently biasing it toward
                // the lower midpoint selected by `region.center()`.
                fixed_interest_center: request.entry_center,
                return_interest_center: return_region.center(),
                preparation: EmbeddedWorldPreviewPreparationSnapshot::default(),
                render: EmbeddedWorldPreviewRenderSnapshot::default(),
                pending_actor_update: None,
                pending_remote_player_update: None,
                mutation_sequence: 0,
                last_mutation: None,
                boundary_warning: preview_boundary_warning,
                failure: None,
            }),
            (WarmWorldPresentationRequest::OpaqueGate, None) => None,
            _ => unreachable!("presentation renderer construction stays paired"),
        };
        log::info!(
            "warm-world standby queued id={} seed={} center=({}, {}) renderer_shell_ms={:.3} multiview_required={} multiview_ms={:.3} atlas={}x{} base_bytes={}",
            instance_id.get(),
            request.seed,
            request.entry_center.x,
            request.entry_center.z,
            renderer_shell_create_ms,
            renderer_multiview_required,
            renderer_multiview_create_ms,
            self.mesh_assets.atlas.width,
            self.mesh_assets.atlas.height,
            self.mesh_assets.atlas.byte_len(),
        );
        Ok(())
    }

    pub fn warm_world_standby_snapshot(&self) -> Option<WarmWorldStandbySnapshot> {
        self.warm_world_standby
            .as_ref()
            .map(|state| state.snapshot(self.services.clock.now()))
    }

    pub fn embedded_world_preview_snapshot(&self) -> Option<EmbeddedWorldPreviewSnapshot> {
        self.embedded_world_preview
            .as_ref()
            .map(EmbeddedWorldPreview::snapshot)
    }

    pub fn active_persistent_passive_actor_identity_summary(
        &self,
    ) -> PersistentPassiveActorIdentitySummary {
        self.active_world.runtime.as_ref().map_or_else(
            PersistentPassiveActorIdentitySummary::default,
            |runtime| {
                persistent_passive_actor_identity_summary(
                    runtime
                        .client()
                        .entity_snapshots()
                        .map(|snapshot| (snapshot.persistent_id, snapshot.kind)),
                )
            },
        )
    }

    pub fn embedded_world_activation_snapshot(&self) -> EmbeddedWorldActivationSnapshot {
        let mut snapshot = self
            .embedded_world_activation
            .snapshot(self.embedded_world_activation_ready());
        if snapshot.volume.is_none() {
            snapshot.volume = self
                .embedded_world_preview
                .as_ref()
                .map(|preview| EmbeddedWorldActivationVolume::from_context(preview.context));
        }
        snapshot
    }

    fn embedded_world_activation_ready(&self) -> bool {
        let Some(preview) = self.embedded_world_preview.as_ref() else {
            return false;
        };
        let Some(standby) = self.standby_world.as_ref() else {
            return false;
        };
        let Some(state) = self.warm_world_standby.as_ref() else {
            return false;
        };
        preview.phase == EmbeddedWorldPreviewPhase::Visible
            && preview.source_world == standby.id
            && preview.renderer_topology_ready
            && preview.source_anchor_gpu_resident
            && preview.source_anchor_traversal_ready
            && preview.bounded_section_count > 0
            && state.phase == WarmWorldStandbyPhase::Switchable
            && state.readiness.switchable
    }

    /// Request the scene-owned diorama action through a neutral world-space
    /// ray. Flat and XR adapters both enter here; block interaction is consumed
    /// only when the ray actually hits a fully switchable preview volume.
    pub fn request_embedded_world_activation(
        &mut self,
        ray_origin: Vec3d,
        ray_direction: Vec3d,
    ) -> bool {
        if self.embedded_world_activation.phase != EmbeddedWorldActivationPhase::Idle
            || !self.embedded_world_activation_ready()
        {
            return false;
        }
        let Some(preview) = self.embedded_world_preview.as_ref() else {
            return false;
        };
        let volume = EmbeddedWorldActivationVolume::from_context(preview.context);
        if volume.ray_distance(ray_origin, ray_direction).is_none() {
            return false;
        }
        let authority_handoff = (|| -> Result<()> {
            self.standby_world
                .as_mut()
                .and_then(|standby| standby.runtime.as_mut())
                .context("embedded-world destination has no runtime")?
                .promote_observer_to_player()
                .context("promote embedded-world destination observer")?;
            self.active_world
                .runtime
                .as_mut()
                .context("embedded-world source has no runtime")?
                .demote_player_to_observer()
                .context("demote embedded-world source player")
        })();
        if let Err(error) = authority_handoff {
            log::error!("embedded-world authority handoff failed: {error:#}");
            return false;
        }
        let Some(standby) = self.standby_world.as_ref() else {
            return false;
        };
        let accepted_destination_entry_pose = standby
            .accepted_entry_pose
            .unwrap_or_else(|| WorldEntryPose::from_camera(&standby.camera));
        self.embedded_world_activation_sequence =
            self.embedded_world_activation_sequence.saturating_add(1);
        self.embedded_world_activation.begin(
            self.embedded_world_activation_sequence,
            self.active_world.id,
            standby.id,
            self.rendered_frames,
            volume,
        );
        self.embedded_world_activation.authority_handoff_pending = true;
        if let Some(report) = self.embedded_world_activation.report.as_mut() {
            report.accepted_destination_entry_pose = Some(accepted_destination_entry_pose);
        }
        true
    }

    pub fn advance_embedded_world_activation(&mut self, dt_seconds: f64) -> bool {
        if !self.embedded_world_activation.phase.active() {
            return false;
        }
        let dt_seconds = if dt_seconds.is_finite() {
            dt_seconds.clamp(0.0, 0.25)
        } else {
            0.0
        };
        match self.embedded_world_activation.phase {
            EmbeddedWorldActivationPhase::Closing => {
                self.embedded_world_activation.phase_elapsed_seconds += dt_seconds;
                let authority_ready = match self.advance_embedded_world_authority_handoff() {
                    Ok(ready) => ready,
                    Err(error) => {
                        let failure = format!("embedded-world authority handoff failed: {error:#}");
                        log::error!("{failure}");
                        self.embedded_world_activation.phase = EmbeddedWorldActivationPhase::Failed;
                        self.embedded_world_activation.phase_elapsed_seconds = 0.0;
                        if let Some(report) = self.embedded_world_activation.report.as_mut() {
                            report.failure = Some(failure);
                        }
                        return true;
                    }
                };
                if self.embedded_world_activation.phase_elapsed_seconds
                    < EMBEDDED_ACTIVATION_CLOSE_SECONDS
                    || !authority_ready
                    || !self.embedded_world_activation_ready()
                {
                    return dt_seconds > 0.0;
                }
                if let Some(accepted_entry_pose) = self
                    .standby_world
                    .as_ref()
                    .and_then(|standby| standby.accepted_entry_pose)
                    && let Some(report) = self.embedded_world_activation.report.as_mut()
                {
                    report.accepted_destination_entry_pose = Some(accepted_entry_pose);
                }
                let switch = self.swap_through_embedded_world_activation();
                match switch {
                    Ok(switch) => {
                        let post_swap_entry =
                            Self::world_slot_entry_support_sample(&self.active_world);
                        if let Some(report) = self.embedded_world_activation.report.as_mut() {
                            report.switch_elapsed_ms = Some(switch.switch_elapsed_ms);
                            report.switched_activation_frame = Some(
                                self.embedded_world_activation
                                    .activation_frame
                                    .saturating_add(1),
                            );
                            match post_swap_entry {
                                Ok(sample) if sample.support.supported() => {
                                    report.post_swap_entry = Some(sample);
                                }
                                Ok(sample) => {
                                    report.post_swap_entry = Some(sample);
                                    report.failure = Some(format!(
                                        "post-swap entry is not supported: {:?}",
                                        sample.support
                                    ));
                                }
                                Err(error) => {
                                    report.failure =
                                        Some(format!("sample post-swap entry support: {error:#}"));
                                }
                            }
                        }
                        if self
                            .embedded_world_activation
                            .report
                            .as_ref()
                            .is_some_and(|report| report.failure.is_some())
                        {
                            self.embedded_world_activation.phase =
                                EmbeddedWorldActivationPhase::Failed;
                            self.embedded_world_activation.phase_elapsed_seconds = 0.0;
                            return true;
                        }
                        if let Err(error) = self.retarget_embedded_world_preview_after_switch() {
                            let failure = format!(
                                "embedded-world activation preview retarget failed: {error:#}"
                            );
                            log::error!("{failure}");
                            if let Some(preview) = self.embedded_world_preview.as_mut() {
                                preview.phase = EmbeddedWorldPreviewPhase::Failed;
                                preview.failure = Some(failure.clone());
                            }
                            // The complete-slot exchange already succeeded and
                            // the selected destination is drawable. Reveal it
                            // normally while keeping the invalid return preview
                            // closed and the diagnostic failure explicit.
                            self.embedded_world_activation.phase =
                                EmbeddedWorldActivationPhase::Opening;
                            self.embedded_world_activation.phase_elapsed_seconds = 0.0;
                            if let Some(report) = self.embedded_world_activation.report.as_mut() {
                                report.failure = Some(failure);
                            }
                            return true;
                        }
                        self.embedded_world_activation.phase =
                            EmbeddedWorldActivationPhase::Covered;
                        self.embedded_world_activation.phase_elapsed_seconds = 0.0;
                    }
                    Err(error) => {
                        let failure = format!("embedded-world activation switch failed: {error:#}");
                        log::error!("{failure}");
                        self.embedded_world_activation.phase = EmbeddedWorldActivationPhase::Failed;
                        self.embedded_world_activation.phase_elapsed_seconds = 0.0;
                        if let Some(report) = self.embedded_world_activation.report.as_mut() {
                            report.failure = Some(failure);
                        }
                    }
                }
                true
            }
            EmbeddedWorldActivationPhase::Covered => {
                self.embedded_world_activation.phase_elapsed_seconds += dt_seconds;
                if self.embedded_world_activation.phase_elapsed_seconds
                    >= EMBEDDED_ACTIVATION_COVERED_SECONDS
                    && self.embedded_world_activation_ready()
                {
                    self.embedded_world_activation.phase = EmbeddedWorldActivationPhase::Opening;
                    self.embedded_world_activation.phase_elapsed_seconds = 0.0;
                }
                dt_seconds > 0.0
            }
            EmbeddedWorldActivationPhase::Opening => {
                self.embedded_world_activation.phase_elapsed_seconds += dt_seconds;
                if self.embedded_world_activation.phase_elapsed_seconds
                    >= EMBEDDED_ACTIVATION_OPEN_SECONDS
                    && self
                        .embedded_world_activation
                        .report
                        .as_ref()
                        .is_some_and(|report| report.stability_entry.is_some())
                {
                    if let Some(report) = self.embedded_world_activation.report.as_mut() {
                        report.completed_activation_frame = Some(
                            self.embedded_world_activation
                                .activation_frame
                                .saturating_add(1),
                        );
                    }
                    self.embedded_world_activation.phase = EmbeddedWorldActivationPhase::Idle;
                    self.embedded_world_activation.phase_elapsed_seconds = 0.0;
                }
                dt_seconds > 0.0
            }
            EmbeddedWorldActivationPhase::Idle | EmbeddedWorldActivationPhase::Failed => false,
        }
    }

    fn advance_embedded_world_authority_handoff(&mut self) -> Result<bool> {
        if !self.embedded_world_activation.authority_handoff_pending {
            return Ok(true);
        }
        let standby = self
            .standby_world
            .as_mut()
            .context("embedded-world authority handoff lost its destination")?;
        let runtime = standby
            .runtime
            .as_mut()
            .context("embedded-world authority handoff destination has no runtime")?;
        runtime
            .poll_with_update_budget(RuntimeUpdatePumpBudget::unlimited())
            .context("poll promoted embedded-world destination")?;
        let before = standby.local_participant.camera.snapshot();
        let before_feet = standby.local_participant.camera.feet_position();
        let accepted = mclone_app_runtime::apply_pending_engine_camera_position_updates(
            runtime,
            &mut standby.local_participant.camera,
            XR_CAMERA_COMMIT_CONTEXT,
        )?;
        if !accepted {
            return Ok(false);
        }
        self.embedded_world_activation.authority_handoff_pending = false;
        let after = standby.camera.snapshot();
        let position_changed = before.eye != after.eye
            || before.yaw_radians != after.yaw_radians
            || before.pitch_radians != after.pitch_radians
            || before_feet != standby.camera.feet_position();
        if position_changed {
            standby.local_participant.reset_movement();
            self.invalidate_warm_world_destination_after_correction();
        }
        Ok(true)
    }

    fn retarget_embedded_world_preview_after_switch(&mut self) -> Result<()> {
        let Some(standby) = self.standby_world.as_mut() else {
            bail!("standby slot disappeared after its ownership exchange");
        };
        let Some(preview) = self.embedded_world_preview.as_mut() else {
            bail!("embedded preview disappeared after its ownership exchange");
        };
        let fixed_interest_center = preview.return_interest_center;
        if let Some(runtime) = standby.runtime.as_mut() {
            runtime
                .set_interest_center(fixed_interest_center)
                .context("retarget retained preview interest after ownership exchange")?;
        }
        std::mem::swap(&mut preview.context, &mut preview.return_context);
        std::mem::swap(&mut preview.region, &mut preview.return_region);
        std::mem::swap(
            &mut preview.fixed_interest_center,
            &mut preview.return_interest_center,
        );
        preview.source_world = standby.id;
        preview.phase = EmbeddedWorldPreviewPhase::Warming;
        preview.source_anchor_gpu_resident = false;
        preview.source_anchor_traversal_ready = false;
        preview.bounded_section_count = 0;
        preview.last_draw = TexturedSectionRenderStats::default();
        preview.source_host_mode = standby.runtime.as_ref().map(|runtime| runtime.host_mode());
        preview.fixed_interest_center = fixed_interest_center;
        preview.preparation = EmbeddedWorldPreviewPreparationSnapshot::default();
        preview.render = EmbeddedWorldPreviewRenderSnapshot::default();
        preview.last_mutation = None;
        preview.failure = None;
        if let Some(state) = self.warm_world_standby.as_mut()
            && let WarmWorldPresentationRequest::Diorama {
                region,
                return_region,
                placement,
                return_placement,
                entry_relative,
            } = &mut state.presentation
        {
            std::mem::swap(region, return_region);
            std::mem::swap(placement, return_placement);
            if let Some(layout) = entry_relative {
                std::mem::swap(
                    &mut layout.standby_entry_pose,
                    &mut layout.active_entry_pose,
                );
                layout.standby_uses_primary_anchor = !layout.standby_uses_primary_anchor;
            }
        }
        self.embedded_world_activation.volume =
            Some(EmbeddedWorldActivationVolume::from_context(preview.context));
        Ok(())
    }

    pub(crate) fn embedded_world_activation_fade_overlay(&self) -> Option<ScreenFadeOverlay> {
        let alpha = self.embedded_world_activation.alpha();
        (alpha > 0.0).then_some(ScreenFadeOverlay::new([0.0, 0.0, 0.0], alpha))
    }

    pub(crate) fn defer_embedded_world_destination_preparation(&self) -> bool {
        self.embedded_world_activation.phase.active()
            && self
                .embedded_world_activation
                .report
                .as_ref()
                .is_some_and(|report| {
                    report.switch_elapsed_ms.is_some()
                        && report.first_uncovered_activation_frame.is_none()
                })
    }

    pub(crate) fn record_embedded_world_activation_frame(
        &mut self,
        drawn_section_count: usize,
        upload: XrTerrainUploadSummary,
        eye_count: usize,
    ) {
        if !self.embedded_world_activation.phase.active() {
            return;
        }
        self.embedded_world_activation.activation_frame = self
            .embedded_world_activation
            .activation_frame
            .saturating_add(1);
        let activation_frame = self.embedded_world_activation.activation_frame;
        let alpha = self.embedded_world_activation.alpha();
        let entry_sample = self
            .embedded_world_activation
            .report
            .as_ref()
            .is_some_and(|report| report.switch_elapsed_ms.is_some())
            .then(|| Self::world_slot_entry_support_sample(&self.active_world));
        let mut support_failure = None;
        let Some(report) = self.embedded_world_activation.report.as_mut() else {
            return;
        };
        if alpha >= 1.0 {
            report.covered_rendered_frames = report.covered_rendered_frames.saturating_add(1);
        }
        if report.switch_elapsed_ms.is_some()
            && alpha < 1.0
            && report.first_uncovered_activation_frame.is_none()
        {
            report.first_uncovered_activation_frame = Some(activation_frame);
            report.first_uncovered_world = Some(self.active_world.id);
            report.first_uncovered_drawn_section_count = drawn_section_count;
            report.first_uncovered_uploaded_section_count = upload.uploaded_section_count;
            report.first_uncovered_submitted_compile_section_count =
                upload.submitted_compile_section_count;
            report.first_uncovered_accepted_compile_result_count =
                upload.accepted_compile_result_count;
            report.first_uncovered_queue_lifecycle_items =
                upload.queued_upload_lifecycle_item_count;
            report.first_uncovered_pending_compile_jobs = upload.pending_compile_jobs_after;
            report.first_uncovered_eye_count = eye_count;
            match entry_sample.as_ref() {
                Some(Ok(sample)) => {
                    report.first_uncovered_entry = Some(*sample);
                    if !sample.support.supported() {
                        support_failure = Some(format!(
                            "first uncovered entry is not supported: {:?}",
                            sample.support
                        ));
                    }
                }
                Some(Err(error)) => {
                    support_failure =
                        Some(format!("sample first-uncovered entry support: {error:#}"));
                }
                None => {}
            }
        }
        if report.stability_entry.is_none()
            && report
                .first_uncovered_activation_frame
                .is_some_and(|first| {
                    activation_frame >= first.saturating_add(EMBEDDED_ACTIVATION_STABILITY_FRAMES)
                })
        {
            report.stability_activation_frame = Some(activation_frame);
            match entry_sample.as_ref() {
                Some(Ok(sample)) => {
                    report.stability_entry = Some(*sample);
                    if !sample.support.supported() {
                        support_failure = Some(format!(
                            "delayed entry stability is not supported: {:?}",
                            sample.support
                        ));
                    }
                }
                Some(Err(error)) => {
                    support_failure = Some(format!("sample delayed entry stability: {error:#}"));
                }
                None => {}
            }
        }
        if let Some(failure) = support_failure {
            report.failure = Some(failure);
            self.embedded_world_activation.phase = EmbeddedWorldActivationPhase::Failed;
            self.embedded_world_activation.phase_elapsed_seconds = 0.0;
        }
    }

    fn world_slot_entry_support_sample(
        slot: &DrawableWorldSlot,
    ) -> Result<WorldEntrySupportSample> {
        let runtime = slot
            .runtime
            .as_ref()
            .context("world entry support sample has no runtime")?;
        let pose = WorldEntryPose::from_camera(&slot.camera);
        Ok(WorldEntrySupportSample {
            pose,
            on_ground: slot.camera.on_ground(),
            support: mclone_client::standing_pose_facts(runtime.client(), pose.feet_position),
        })
    }

    /// Launch-smoke diagnostic for proving that the retained preview is a live
    /// authoritative world. This uses a host-only mutation control on B's
    /// observer session; it does not grant gameplay authority to the observer
    /// or route the interaction through A's player.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn debug_break_embedded_world_preview_block(
        &mut self,
        block: mclone_core::BlockPos,
    ) -> Result<EmbeddedWorldPreviewMutationSnapshot> {
        let (source_world, region, phase, in_flight) = self
            .embedded_world_preview
            .as_ref()
            .map(|preview| {
                (
                    preview.source_world,
                    preview.region,
                    preview.phase,
                    preview.last_mutation.as_ref().is_some_and(|mutation| {
                        matches!(
                            mutation.snapshot.phase,
                            EmbeddedWorldPreviewMutationPhase::CommandSent
                                | EmbeddedWorldPreviewMutationPhase::ClientApplied
                        )
                    }),
                )
            })
            .context("embedded preview mutation requested without a preview")?;
        if phase != EmbeddedWorldPreviewPhase::Visible {
            bail!("embedded preview mutation requires a visible preview");
        }
        if in_flight {
            bail!("embedded preview already has a mutation awaiting GPU application");
        }
        let chunk = block.chunk_pos();
        let section = mclone_mesh::RenderSectionKey::new(
            chunk.x,
            mclone_core::block_to_section_coord(block.y),
            chunk.z,
        );
        if !region.contains(section) {
            bail!(
                "embedded preview mutation block ({}, {}, {}) lies outside its bounded region",
                block.x,
                block.y,
                block.z,
            );
        }

        let command_changed = {
            let slot = self
                .standby_world
                .as_mut()
                .filter(|slot| slot.id == source_world)
                .context("embedded preview mutation lost its retained source slot")?;
            let runtime = slot
                .runtime
                .as_mut()
                .context("embedded preview mutation source has no runtime")?;
            if runtime.client().block_state_at_block_pos(block)
                == Some(mclone_core::AIR_BLOCK_STATE_ID)
            {
                bail!("embedded preview mutation target is already air");
            }
            runtime.debug_break_observed_block(block)?
        };
        let client_applied = self
            .standby_world
            .as_ref()
            .and_then(|slot| slot.runtime.as_ref())
            .and_then(|runtime| runtime.client().block_state_at_block_pos(block))
            == Some(mclone_core::AIR_BLOCK_STATE_ID);

        let preview = self
            .embedded_world_preview
            .as_mut()
            .expect("preview presence checked above");
        preview.mutation_sequence = preview.mutation_sequence.saturating_add(1);
        let snapshot = EmbeddedWorldPreviewMutationSnapshot {
            sequence: preview.mutation_sequence,
            block,
            phase: if client_applied {
                EmbeddedWorldPreviewMutationPhase::ClientApplied
            } else {
                EmbeddedWorldPreviewMutationPhase::CommandSent
            },
            command_changed,
            command_update_count: 0,
            command_section_block_update_count: 0,
            requested_after_rendered_frame: self.rendered_frames,
            completed_after_rendered_frame: None,
            submitted_compile_section_count: 0,
            accepted_compile_result_count: 0,
            uploaded_section_count: 0,
            failure: None,
        };
        preview.last_mutation = Some(EmbeddedWorldPreviewMutationState {
            snapshot: snapshot.clone(),
            submitted_compile_baseline: preview.preparation.submitted_compile_section_count,
            accepted_compile_baseline: preview.preparation.accepted_compile_result_count,
            uploaded_section_baseline: preview.preparation.uploaded_section_count,
        });
        Ok(snapshot)
    }

    pub fn world_gate_snapshot(&self) -> Option<WorldGateSnapshot> {
        self.world_gate.as_ref().map(WorldGate::snapshot)
    }

    pub(crate) fn synchronize_world_gate_state(&mut self) {
        let Some(state) = self.warm_world_standby.as_ref() else {
            self.world_gate = None;
            return;
        };
        if matches!(
            state.presentation,
            WarmWorldPresentationRequest::Diorama { .. }
        ) {
            self.world_gate = None;
            return;
        }
        let availability = match state.phase {
            WarmWorldStandbyPhase::Switchable if state.readiness.switchable => {
                WorldGateAvailability::Switchable
            }
            WarmWorldStandbyPhase::PlacementFailed
            | WarmWorldStandbyPhase::Failed
            | WarmWorldStandbyPhase::Cancelled => WorldGateAvailability::Failed,
            _ => WorldGateAvailability::Warming,
        };
        let (Some(active_endpoint), Some(destination_endpoint), Some(standby)) = (
            state.source_endpoint,
            state.destination_endpoint,
            self.standby_world.as_ref(),
        ) else {
            if let Some(gate) = self.world_gate.as_mut() {
                gate.set_availability(availability);
            }
            return;
        };
        if let Some(gate) = self.world_gate.as_mut() {
            gate.synchronize(
                self.active_world.id,
                standby.id,
                active_endpoint,
                destination_endpoint,
                availability,
            );
        } else {
            let mut gate = WorldGate::new(
                self.active_world.id,
                standby.id,
                active_endpoint,
                destination_endpoint,
            );
            gate.set_availability(availability);
            self.world_gate = Some(gate);
        }
    }

    pub fn active_world_instance_id(&self) -> WorldInstanceId {
        self.active_world.id
    }

    pub fn active_world_seed(&self) -> i64 {
        self.active_world.scene.seed
    }

    pub fn last_warm_world_switch_report(&self) -> Option<WarmWorldSwitchReport> {
        self.last_warm_world_switch.clone()
    }

    /// Apply one shared warm-world selection command between presented frames.
    /// The complete slot moves atomically; no runtime, draw store, renderer, or
    /// upload work is constructed here.
    pub fn apply_warm_world_selection_command(
        &mut self,
        command: WarmWorldSelectionCommand,
    ) -> Result<WarmWorldSwitchReport> {
        match command {
            WarmWorldSelectionCommand::SwapWithStandby if self.world_gate.is_some() => {
                self.swap_through_world_gate()
            }
            WarmWorldSelectionCommand::SwapWithStandby => {
                self.swap_with_switchable_warm_world_using_poses(None)
            }
        }
    }

    fn swap_through_embedded_world_activation(&mut self) -> Result<WarmWorldSwitchReport> {
        let destination = self
            .standby_world
            .as_ref()
            .context("embedded-world activation has no retained destination")?;
        let destination_entry_pose = destination
            .accepted_entry_pose
            .unwrap_or_else(|| WorldEntryPose::from_camera(&destination.camera));
        let return_entry_pose = self
            .active_world
            .accepted_entry_pose
            .unwrap_or_else(|| WorldEntryPose::from_camera(&self.active_world.camera));
        self.swap_with_switchable_warm_world_using_poses(Some((
            destination_entry_pose,
            return_entry_pose,
        )))
    }

    fn swap_through_world_gate(&mut self) -> Result<WarmWorldSwitchReport> {
        let (destination_entry_pose, return_entry_pose) = {
            let gate = self
                .world_gate
                .as_ref()
                .context("world-gate selection requested without a gate model")?;
            (
                gate.destination_entry_pose(),
                gate.return_entry_pose_after_switch(),
            )
        };
        let report = self.swap_with_switchable_warm_world_using_poses(Some((
            destination_entry_pose,
            return_entry_pose,
        )))?;
        self.world_gate
            .as_mut()
            .expect("successful world-gate selection retains its gate model")
            .complete_switch();
        Ok(report)
    }

    pub(crate) fn apply_world_gate_visual_midpoint(&mut self, point: Vec3d) -> Result<bool> {
        let Some(observation) = self
            .world_gate
            .as_mut()
            .map(|gate| gate.observe_visual_midpoint(point))
        else {
            return Ok(false);
        };
        match observation {
            WorldGateObservation::Crossed => {
                self.swap_through_world_gate()?;
                Ok(true)
            }
            WorldGateObservation::Blocked => {
                log::debug!("closed world gate rejected visual midpoint crossing");
                let snapshot = self.active_world.camera.snapshot();
                let feet = self.active_world.camera.feet_position();
                let clamped_feet = self
                    .world_gate
                    .as_ref()
                    .expect("blocked observation retains its gate")
                    .blocked_feet_position(feet);
                self.active_world.camera.set_player_feet_pose(
                    clamped_feet,
                    snapshot.yaw_radians,
                    snapshot.pitch_radians,
                );
                self.active_world.local_participant.reset_movement();
                let (changed, _) = self.commit_engine_camera_player_pose_timed()?;
                Ok(changed)
            }
            WorldGateObservation::None | WorldGateObservation::Armed => Ok(false),
        }
    }

    fn swap_with_switchable_warm_world_using_poses(
        &mut self,
        selection_entry_poses: Option<(WorldEntryPose, WorldEntryPose)>,
    ) -> Result<WarmWorldSwitchReport> {
        let state = self
            .warm_world_standby
            .as_ref()
            .context("warm-world selection requested without standby state")?;
        if state.phase != WarmWorldStandbyPhase::Switchable || !state.readiness.switchable {
            bail!(
                "warm-world standby is not switchable: phase={} readiness={}",
                state.phase.label(),
                state.readiness.switchable,
            );
        }
        let (selected_source_endpoint, selected_destination_endpoint) =
            if self.world_gate.is_none() && selection_entry_poses.is_some() {
                (state.source_endpoint, state.destination_endpoint)
            } else {
                (
                    Some(
                        state
                            .source_endpoint
                            .context("switchable warm-world state has no source endpoint")?,
                    ),
                    Some(
                        state
                            .destination_endpoint
                            .context("switchable warm-world state has no destination endpoint")?,
                    ),
                )
            };
        let standby = self
            .standby_world
            .as_ref()
            .context("switchable warm-world state has no retained standby slot")?;
        if standby.lifecycle != WorldSlotLifecycle::StandbySwitchable {
            bail!(
                "warm-world standby slot has lifecycle {:?}, expected switchable",
                standby.lifecycle,
            );
        }
        if standby.asset_epoch != self.active_world.asset_epoch
            || standby.asset_epoch != self.active_assets.epoch
        {
            bail!(
                "warm-world selection rejected mixed asset epochs: active={} standby={} assets={}",
                self.active_world.asset_epoch,
                standby.asset_epoch,
                self.active_assets.epoch,
            );
        }

        let renderer_multiview_required = state.renderer_multiview_required;
        let standby_cadence = state.standby_cadence;
        let source_renderer_ready = !renderer_multiview_required
            || self.active_world.draw.multiview_renderer_materialized();
        let destination_renderer_ready =
            !renderer_multiview_required || standby.draw.multiview_renderer_materialized();
        if !source_renderer_ready || !destination_renderer_ready {
            bail!(
                "warm-world selection would leave lazy terrain topology: source={} destination={}",
                source_renderer_ready,
                destination_renderer_ready,
            );
        }
        let destination_entry_pose = selection_entry_poses
            .map(|(destination, _)| destination)
            .unwrap_or_else(|| {
                world_gate_destination_entry_pose(
                    selected_destination_endpoint.expect("gate endpoint checked above"),
                )
            });
        let destination_entry_section = entry_support_render_section(destination_entry_pose)
            .context("mapped warm-world destination pose has no entry section")?;
        if !standby.draw.contains_section(destination_entry_section)
            || !standby
                .draw
                .traversal_ready_contains_section(destination_entry_section)
        {
            bail!(
                "mapped warm-world destination is not GPU/traversal ready at section {:?}",
                destination_entry_section,
            );
        }
        let destination_support = mclone_client::standing_pose_facts(
            standby
                .runtime
                .as_ref()
                .context("switchable warm-world destination has no runtime")?
                .client(),
            destination_entry_pose.feet_position,
        );
        if !destination_support.supported() {
            bail!(
                "mapped warm-world destination lacks loaded clear support: {destination_support:?}"
            );
        }
        let return_entry_pose = selection_entry_poses
            .map(|(_, return_pose)| return_pose)
            .unwrap_or_else(|| {
                world_gate_destination_entry_pose(
                    selected_source_endpoint.expect("gate endpoint checked above"),
                )
            });
        let return_entry_section = entry_support_render_section(return_entry_pose)
            .context("mapped warm-world return pose has no entry section")?;
        if !self
            .active_world
            .draw
            .contains_section(return_entry_section)
            || !self
                .active_world
                .draw
                .traversal_ready_contains_section(return_entry_section)
        {
            bail!(
                "mapped warm-world return destination is not GPU/traversal ready at section {:?}",
                return_entry_section,
            );
        }
        let return_support = mclone_client::standing_pose_facts(
            self.active_world
                .runtime
                .as_ref()
                .context("warm-world source has no runtime")?
                .client(),
            return_entry_pose.feet_position,
        );
        if !return_support.supported() {
            bail!(
                "mapped warm-world return destination lacks loaded clear support: {return_support:?}"
            );
        }

        let source_id = self.active_world.id;
        let source_seed = self.active_world.scene.seed;
        let destination_id = standby.id;
        let destination_seed = standby.scene.seed;
        let command_after_source_frame = self.rendered_frames;
        let source_queue = self.active_world.section_uploads.stats();
        let destination_queue = standby.section_uploads.stats();
        let source_stats = self
            .active_world
            .runtime
            .as_ref()
            .context("active warm-world source has no runtime")?
            .stats();
        let destination_stats = standby
            .runtime
            .as_ref()
            .context("switchable warm-world destination has no runtime")?
            .stats();

        let switch_started_at = self.services.clock.now();
        let camera_started_at = self.services.clock.now();
        let (source_camera_commit_changed, source_camera_position_changed) =
            Self::commit_world_slot_camera(&mut self.active_world, &self.services.clock)
                .context("reconcile warm-world source camera before selection")?;
        let (mut destination_camera_commit_changed, destination_camera_position_changed) = {
            let standby = self
                .standby_world
                .as_mut()
                .expect("standby presence checked before camera reconcile");
            Self::commit_world_slot_camera(standby, &self.services.clock)
                .context("reconcile warm-world destination camera before selection")?
        };
        if destination_camera_position_changed {
            self.invalidate_warm_world_destination_after_correction();
            bail!(
                "warm-world destination received a late camera correction; readiness must settle again"
            );
        }

        let destination_mapped_position_changed = {
            let standby = self
                .standby_world
                .as_mut()
                .expect("standby presence checked before endpoint mapping");
            let pitch_radians = standby.camera.snapshot().pitch_radians;
            standby.camera.set_player_feet_pose(
                destination_entry_pose.feet_position,
                destination_entry_pose.yaw_radians,
                pitch_radians,
            );
            standby.local_participant.reset_movement();
            let (changed, position_changed) =
                Self::commit_world_slot_camera(standby, &self.services.clock)
                    .context("commit mapped warm-world destination camera and interest")?;
            destination_camera_commit_changed |= changed;
            position_changed
        };
        if destination_mapped_position_changed {
            self.invalidate_warm_world_destination_after_correction();
            bail!(
                "warm-world destination corrected the mapped gate-entry pose; readiness must settle again"
            );
        }
        {
            let standby = self
                .standby_world
                .as_mut()
                .expect("standby presence checked before entry ground probe");
            let runtime = standby
                .runtime
                .as_ref()
                .expect("mapped warm-world destination retains its runtime");
            standby
                .local_participant
                .camera
                .probe_ground(runtime.client(), WARM_WORLD_ENTRY_GROUND_PROBE_DISTANCE);
        }
        let camera_commit_ms = elapsed_ms(self.services.clock.elapsed_since(camera_started_at));

        // The live camera may have moved to observe or activate the tabletop.
        // Retain the separately admitted return pose; the next covered switch
        // reconciles that slot's camera to this safe entry before reveal.
        let source_entry_pose = return_entry_pose;
        let source_entry_section = entry_support_render_section(source_entry_pose)
            .context("active camera cannot produce return entry coverage")?;
        if !self
            .active_world
            .draw
            .contains_section(source_entry_section)
            || !self
                .active_world
                .draw
                .traversal_ready_contains_section(source_entry_section)
        {
            bail!(
                "active world cannot become an immediate return standby at section {:?}",
                source_entry_section,
            );
        }

        // Demote the old active runtime to the launch-scoped standby cadence;
        // restore the selected destination to its authored active cadence.
        // This is the only simulation-rate distinction between the retained
        // slots and is applied before their ownership exchange.
        let source_cadence = standby_cadence;
        let source_cadence_changed = self
            .active_world
            .runtime
            .as_mut()
            .expect("active runtime presence checked")
            .set_simulation_cadence(source_cadence)
            .context("apply standby simulation cadence before source demotion")?;
        let destination_cadence_changed = {
            let standby = self
                .standby_world
                .as_mut()
                .expect("standby presence checked before cadence restore");
            let cadence = standby.scene.simulation_cadence;
            standby
                .runtime
                .as_mut()
                .expect("standby runtime presence checked")
                .set_simulation_cadence(cadence)
                .context("restore destination world simulation cadence before selection")?
        };

        self.active_world.accepted_entry_pose = Some(source_entry_pose);
        self.active_world.camera.clear_keys();
        self.standby_world
            .as_mut()
            .expect("standby presence checked before ownership exchange")
            .camera
            .clear_keys();
        std::mem::swap(
            &mut self.active_world,
            self.standby_world
                .as_mut()
                .expect("standby presence checked before ownership exchange"),
        );
        // A deferred per-eye preparation summary belongs to the pre-exchange
        // active slot. Never attribute or apply it after world ownership moves.
        self.prefetched_live_upload = None;
        self.active_world.lifecycle = WorldSlotLifecycle::ActiveReady;
        self.standby_world
            .as_mut()
            .expect("ownership exchange retains old active slot")
            .lifecycle = WorldSlotLifecycle::StandbySwitchable;
        self.active_world
            .runtime
            .as_mut()
            .expect("selected active slot retains its runtime")
            .set_render_priority(
                mclone_app_runtime::scene_session_runtime::RuntimeRenderPriority::Active,
            );
        self.standby_world
            .as_mut()
            .expect("ownership exchange retains old active slot")
            .runtime
            .as_mut()
            .expect("return standby retains its runtime")
            .set_render_priority(
                mclone_app_runtime::scene_session_runtime::RuntimeRenderPriority::Standby,
            );

        let destination_descriptor = self
            .active_world
            .descriptor
            .clone()
            .context("selected warm world has no active-session descriptor")?;
        let activated_catalog_world = destination_descriptor.local_world_id().cloned();
        self.session.complete_start(destination_descriptor);
        self.clear_world_selection_presentation_state();

        self.retarget_warm_world_state_after_switch(
            renderer_multiview_required,
            selected_destination_endpoint,
            selected_source_endpoint,
            return_entry_pose,
        )?;
        if let Some(id) = activated_catalog_world {
            self.request_catalog_activation_play_record(id);
        }

        self.warm_world_switch_sequence = self.warm_world_switch_sequence.saturating_add(1);
        let report = WarmWorldSwitchReport {
            sequence: self.warm_world_switch_sequence,
            source_instance_id: source_id,
            source_seed,
            destination_instance_id: destination_id,
            destination_seed,
            command_after_source_frame,
            switch_elapsed_ms: elapsed_ms(self.services.clock.elapsed_since(switch_started_at)),
            source_camera_commit_changed,
            destination_camera_commit_changed,
            source_camera_position_changed,
            destination_camera_position_changed,
            camera_commit_ms,
            source_cadence_changed,
            destination_cadence_changed,
            source_queue_lifecycle_items_before: source_queue.queued_lifecycle_items,
            source_queue_mesh_owned_bytes_before: source_queue.queued_upload_mesh_owned_bytes,
            destination_queue_lifecycle_items_before: destination_queue.queued_lifecycle_items,
            destination_queue_mesh_owned_bytes_before: destination_queue
                .queued_upload_mesh_owned_bytes,
            source_pending_compile_jobs_before: source_stats.pending_render_compile_jobs,
            destination_pending_compile_jobs_before: destination_stats.pending_render_compile_jobs,
            source_runtime_command_count_before: source_stats.command_count,
            source_runtime_update_count_before: source_stats.update_count,
            destination_runtime_command_count_before: destination_stats.command_count,
            destination_runtime_update_count_before: destination_stats.update_count,
            switch_uploaded_section_count: 0,
            switch_submitted_compile_section_count: 0,
            switch_accepted_compile_result_count: 0,
            switch_materialized_renderer: false,
            first_drawable_destination_frame: None,
            first_drawn_section_count: 0,
            first_frame_uploaded_section_count: 0,
            first_frame_submitted_compile_section_count: 0,
            first_frame_accepted_compile_result_count: 0,
            first_frame_queue_lifecycle_items: 0,
            first_frame_queue_mesh_owned_bytes: 0,
            first_frame_pending_compile_jobs_after: 0,
            destination_runtime_command_count_after_first_frame: 0,
            destination_runtime_update_count_after_first_frame: 0,
        };
        log::info!(
            "warm-world switch sequence={} source={} seed={} destination={} seed={} source_frame={} switch_ms={:.3} camera_ms={:.3} source_queue={} destination_queue={}",
            report.sequence,
            report.source_instance_id.get(),
            report.source_seed,
            report.destination_instance_id.get(),
            report.destination_seed,
            report.command_after_source_frame,
            report.switch_elapsed_ms,
            report.camera_commit_ms,
            report.source_queue_lifecycle_items_before,
            report.destination_queue_lifecycle_items_before,
        );
        self.last_warm_world_switch = Some(report.clone());
        Ok(report)
    }

    fn request_catalog_activation_play_record(&mut self, id: LocalWorldId) {
        let Some(mut operations) = self.services.catalog_operations.take() else {
            log::warn!(
                "catalog activation for `{id}` could not record recency: no catalog service"
            );
            return;
        };
        let effects = self
            .client_experience
            .catalog_mut()
            .request_record_world_played(id.clone());
        for request in effects.catalog_requests {
            operations.submit(request, Some(id.clone()));
        }
        let completed = operations.poll(self.client_experience.catalog_mut());
        debug_assert!(completed.session_starts.is_empty());
        debug_assert!(completed.catalog_requests.is_empty());
        self.services.catalog_operations = Some(operations);
    }

    fn commit_world_slot_camera(
        slot: &mut DrawableWorldSlot,
        clock: &MonotonicClockHandle,
    ) -> Result<(bool, bool)> {
        let before = slot.camera.snapshot();
        let before_feet = slot.camera.feet_position();
        let runtime = slot
            .runtime
            .as_mut()
            .context("warm-world camera reconcile requires a runtime")?;
        let changed = mclone_app_runtime::commit_engine_camera_player_pose(
            runtime,
            &mut slot.local_participant.camera,
            XR_CAMERA_COMMIT_CONTEXT,
            clock,
            None,
        )?;
        let after = slot.camera.snapshot();
        let position_changed = before.eye != after.eye
            || before.yaw_radians != after.yaw_radians
            || before.pitch_radians != after.pitch_radians
            || before_feet != slot.camera.feet_position();
        if position_changed {
            slot.local_participant.reset_movement();
        }
        Ok((changed, position_changed))
    }

    fn invalidate_warm_world_destination_after_correction(&mut self) {
        let standby = self
            .standby_world
            .as_mut()
            .expect("warm-world correction invalidation requires a standby");
        standby.accepted_entry_pose = Some(WorldEntryPose::from_camera(&standby.camera));
        let state = self
            .warm_world_standby
            .as_mut()
            .expect("warm-world correction invalidation requires standby state");
        state.phase = WarmWorldStandbyPhase::ResolvingEndpoints;
        state.destination_endpoint = None;
        state.readiness.cpu_ready = false;
        state.readiness.entry_section = None;
        state.readiness.entry_section_gpu_resident = false;
        state.readiness.entry_section_traversal_ready = false;
        state.readiness.entry_body_loaded = false;
        state.readiness.entry_body_clear = false;
        state.readiness.entry_support_loaded = false;
        state.readiness.entry_solid_support = false;
        state.readiness.switchable = false;
        state.accepted_entry_pose = None;
    }

    fn retarget_warm_world_state_after_switch(
        &mut self,
        renderer_multiview_required: bool,
        source_endpoint: Option<WorldGateEndpointCandidate>,
        destination_endpoint: Option<WorldGateEndpointCandidate>,
        entry_pose: WorldEntryPose,
    ) -> Result<()> {
        let standby = self
            .standby_world
            .as_ref()
            .context("ownership exchange lost its return standby")?;
        let entry_section = entry_support_render_section(entry_pose)
            .context("return standby departure pose has no entry section")?;
        let entry_section_gpu_resident = standby.draw.contains_section(entry_section);
        let entry_section_traversal_ready =
            standby.draw.traversal_ready_contains_section(entry_section);
        let renderer_multiview_materialized = standby.draw.multiview_renderer_materialized();
        let renderer_topology_ready =
            !renderer_multiview_required || renderer_multiview_materialized;
        let entry_support = mclone_client::standing_pose_facts(
            standby
                .runtime
                .as_ref()
                .context("return standby lost its runtime")?
                .client(),
            entry_pose.feet_position,
        );
        if !entry_section_gpu_resident
            || !entry_section_traversal_ready
            || !renderer_topology_ready
            || !entry_support.supported()
        {
            bail!(
                "ownership exchange produced a non-switchable return slot: gpu={} traversal={} topology={} support={entry_support:?}",
                entry_section_gpu_resident,
                entry_section_traversal_ready,
                renderer_topology_ready,
            );
        }
        let runtime = standby
            .runtime
            .as_ref()
            .context("return standby lost its runtime")?;
        let queue = standby.section_uploads.stats();
        let now = self.services.clock.now();
        let state = self
            .warm_world_standby
            .as_mut()
            .context("ownership exchange lost warm-world state")?;
        state.instance_id = standby.id;
        state.seed = standby.scene.seed;
        state.phase = WarmWorldStandbyPhase::Switchable;
        state.started_at = now;
        state.renderer_shell_create_ms = 0.0;
        state.renderer_multiview_create_ms = 0.0;
        state.renderer_multiview_required = renderer_multiview_required;
        state.renderer_multiview_materialized = renderer_multiview_materialized;
        state.asset_epoch = standby.asset_epoch;
        state.standby_cadence_applied = true;
        state.poll_count = 0;
        state.poll_ms = 0.0;
        state.startup_advance_count = 0;
        state.startup_advance_total_ms = 0.0;
        state.last_advance_ms = 0.0;
        state.worst_advance_ms = 0.0;
        state.worst_startup_step_ms = 0.0;
        state.worst_runtime_poll_ms = 0.0;
        state.endpoint_resolution_ms = 0.0;
        state.gpu_warm_started_at = Some(now);
        state.gpu_ready_at = Some(now);
        state.upload_queue_nonempty_since = (queue.queued_lifecycle_items > 0).then_some(now);
        state.last_gpu_advance_ms = 0.0;
        state.gpu_advance_total_ms = 0.0;
        state.worst_gpu_advance_ms = 0.0;
        state.gpu_advance_count = 0;
        state.gpu_ready_advance_count = 0;
        state.gpu_skipped_no_slack_count = 0;
        state.last_gpu_advance_frame = None;
        state.camera_reconciled = true;
        state.loaded_chunks = runtime.client().loaded_chunk_count();
        state.startup_seed_sections = 0;
        state.startup_seed_drawable_sections = 0;
        state.startup_seed_owned_bytes = 0;
        state.initial_upload_lifecycle_items = 0;
        state.initial_upload_applied_lifecycle_items = 0;
        state.initial_upload_released_compile_jobs = 0;
        state.queued_upload_sections = queue.queued_upload_sections;
        state.queued_upload_lifecycle_items = queue.queued_lifecycle_items;
        state.queued_upload_mesh_owned_bytes = queue.queued_upload_mesh_owned_bytes;
        state.gpu_section_count = standby.draw.section_count();
        state.gpu_vertex_count = standby.draw.vertex_count();
        state.gpu_index_count = standby.draw.index_count();
        state.accepted_compile_result_count = 0;
        state.released_compile_job_count = 0;
        state.readiness = WarmWorldReadiness {
            cpu_ready: true,
            startup_seed_enqueued: true,
            startup_seed_drained: true,
            entry_section: Some(entry_section),
            entry_section_gpu_resident,
            entry_section_traversal_ready,
            entry_body_loaded: entry_support.body_loaded,
            entry_body_clear: entry_support.body_clear,
            entry_support_loaded: entry_support.support_loaded,
            entry_solid_support: entry_support.solid_support,
            renderer_topology_ready,
            switchable: true,
        };
        state.accepted_entry_pose = Some(entry_pose);
        state.source_endpoint = source_endpoint;
        state.destination_endpoint = destination_endpoint;
        state.failure = None;
        Ok(())
    }

    pub(crate) fn record_warm_world_first_destination_frame(
        &mut self,
        drawn_section_count: usize,
        upload: XrTerrainUploadSummary,
    ) {
        let Some(report) = self.last_warm_world_switch.as_mut() else {
            return;
        };
        if report.first_drawable_destination_frame.is_some()
            || report.destination_instance_id != self.active_world.id
        {
            return;
        }
        report.first_drawable_destination_frame = Some(self.rendered_frames);
        report.first_drawn_section_count = drawn_section_count;
        report.first_frame_uploaded_section_count = upload.uploaded_section_count;
        report.first_frame_submitted_compile_section_count = upload.submitted_compile_section_count;
        report.first_frame_accepted_compile_result_count = upload.accepted_compile_result_count;
        report.first_frame_queue_lifecycle_items = upload.queued_upload_lifecycle_item_count;
        report.first_frame_queue_mesh_owned_bytes = upload.queued_upload_mesh_owned_bytes;
        report.first_frame_pending_compile_jobs_after = upload.pending_compile_jobs_after;
        if let Some(runtime) = self.active_world.runtime.as_ref() {
            let stats = runtime.stats();
            report.destination_runtime_command_count_after_first_frame = stats.command_count;
            report.destination_runtime_update_count_after_first_frame = stats.update_count;
        }
        log::info!(
            "warm-world first destination frame sequence={} frame={} drawn={} uploads={} submitted={} accepted={} queue={}",
            report.sequence,
            self.rendered_frames,
            report.first_drawn_section_count,
            report.first_frame_uploaded_section_count,
            report.first_frame_submitted_compile_section_count,
            report.first_frame_accepted_compile_result_count,
            report.first_frame_queue_lifecycle_items,
        );
    }

    pub(crate) fn cancel_warm_world_standby(&mut self, reason: &str) {
        if let Some(mut launch) = self.lobby_launch.take() {
            let cancelled = launch.cancel();
            log::info!(
                "cancelled lobby operations count={} reason={reason}",
                cancelled
            );
        }
        #[cfg(not(target_arch = "wasm32"))]
        {}
        self.embedded_world_preview = None;
        self.embedded_world_activation = EmbeddedWorldActivationState::default();
        self.prepared_warm_world_shell = None;
        // Drop the concrete slot unconditionally. State normally accompanies
        // it, but teardown and resource-rebuild safety must not depend on that
        // diagnostic invariant: taking the slot joins its runtime/compiler
        // owners and releases its GPU resources before the caller continues.
        let dropped_slot = self.standby_world.take().is_some();
        let Some(state) = self.warm_world_standby.as_mut() else {
            if dropped_slot {
                log::warn!(
                    "warm-world standby slot cancelled without diagnostic state reason={reason}"
                );
            }
            return;
        };
        state.phase = WarmWorldStandbyPhase::Cancelled;
        state.failure = Some(reason.to_owned());
        state.readiness = WarmWorldReadiness::default();
        state.upload_queue_nonempty_since = None;
        state.queued_upload_sections = 0;
        state.queued_upload_lifecycle_items = 0;
        state.queued_upload_mesh_owned_bytes = 0;
        state.gpu_section_count = 0;
        state.gpu_vertex_count = 0;
        state.gpu_index_count = 0;
        state.loaded_chunks = 0;
        state.renderer_multiview_materialized = false;
        if let Some(gate) = self.world_gate.as_mut() {
            gate.set_availability(WorldGateAvailability::Failed);
        }
        log::info!(
            "warm-world standby cancelled id={} seed={} reason={reason}",
            state.instance_id.get(),
            state.seed,
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn advance_active_local_startup(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        if self.active_world.local_startup.is_none() {
            return Ok(false);
        }

        let step = {
            let startup = self
                .active_world
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
                    .active_world
                    .local_startup
                    .take()
                    .expect("startup must exist after failed pump step");
                self.fail_local_startup(startup, error);
                return Ok(false);
            }
        };

        // Never submit the provisional scene camera before the server's initial
        // position arrives. A returning player's persisted view can become
        // server-ready one frame before its position update reaches the client;
        // committing here based only on spawn diagnostics would overwrite the
        // restored pose and redirect chunk interest to the provisional camera.
        let authoritative_pose_ready = {
            let startup = self
                .active_world
                .local_startup
                .as_mut()
                .expect("startup must exist after successful pump step");
            if startup.reconciliation_passes == 0 {
                mclone_app_runtime::apply_pending_engine_camera_position_updates(
                    &mut **startup.pump.runtime_services_mut(),
                    &mut startup.camera,
                    XR_CAMERA_COMMIT_CONTEXT,
                )
                .context("accept authoritative local-world startup pose")?
            } else {
                false
            }
        };
        if authoritative_pose_ready {
            let startup = self
                .active_world
                .local_startup
                .as_mut()
                .expect("startup must exist after accepted startup pose");
            reconcile_local_startup_pose_pass(startup, &self.services.clock)
                .context("reconcile authoritative local-world startup pose")?;
            // Force the next budgeted step to use the authoritative camera. A
            // seed produced by this step may still have been prioritized at the
            // provisional scene center.
            return Ok(false);
        }

        if self
            .active_world
            .local_startup
            .as_ref()
            .is_some_and(|startup| startup.reconciliation_passes == 0)
        {
            // Playable chunk evidence is not permission to synthesize an
            // initial player pose. Keep polling until the server position is
            // present; only then may the incremental reconciliation below send
            // camera movement or retarget interest.
            return Ok(false);
        }

        if !step.playable_ready {
            if self
                .active_world
                .local_startup
                .as_ref()
                .and_then(|startup| startup.reconciliation_deadline.as_ref())
                .is_some_and(MonotonicDeadline::is_reached)
            {
                let startup = self
                    .active_world
                    .local_startup
                    .take()
                    .expect("timed-out startup must still own its pump");
                self.fail_local_startup(
                    startup,
                    anyhow!(
                        "timed out after {:.3}s waiting for reconciled local-world startup readiness: cached_sections={} render_seed_drawable_sections={} pending_compile_jobs={}",
                        DEFAULT_STARTUP_READINESS_TIMEOUT.as_secs_f64(),
                        step.cached_section_count,
                        step.render_seed_drawable_section_count,
                        step.pending_compile_jobs,
                    ),
                );
            }
            return Ok(false);
        }

        // A saved player pose or explicit XR startup pose can move interest far
        // away from the provisional scene center. Keep that re-pump incremental:
        // this method runs from the presentation callback, so the blocking
        // `drive_to_ready_reconciled` helper would stop the native event loop and
        // make a large persisted world appear hung. Each call performs one
        // budgeted pump step, then returns until the corrected view has drawable
        // seed coverage and is ready for another correction pass.
        let reconciliation_ready = {
            let startup = self
                .active_world
                .local_startup
                .as_mut()
                .expect("startup must exist after playable step");
            let coverage = startup
                .reconciled_interest_center
                .map(|center| startup.pump.render_seed_drawable_section_count_near(center));
            let deadline_reached = startup
                .reconciliation_deadline
                .as_ref()
                .is_some_and(MonotonicDeadline::is_reached);
            if coverage.is_some_and(|count| count == 0) && !deadline_reached {
                false
            } else {
                if coverage == Some(0) {
                    log::warn!(
                        "local-world startup reconciliation timed out after {:.3}s waiting for render-seed coverage near {:?}; completing with best-effort seed (render_seed_drawable_sections={})",
                        DEFAULT_STARTUP_READINESS_TIMEOUT.as_secs_f64(),
                        startup.reconciled_interest_center,
                        step.render_seed_drawable_section_count,
                    );
                }

                if startup.reconciliation_passes >= MAX_STARTUP_RECONCILE_PASSES {
                    true
                } else {
                    !reconcile_local_startup_pose_pass(startup, &self.services.clock)
                        .context("reconcile incremental local-world startup pose")?
                }
            }
        };
        if !reconciliation_ready {
            return Ok(false);
        }

        let startup = self
            .active_world
            .local_startup
            .take()
            .expect("startup must exist after playable step");
        let failed_request = startup.request.clone();
        match self.complete_local_startup(device, queue, startup, step) {
            Ok(()) => Ok(true),
            Err(error) => {
                log::error!("failed to complete XR local world startup: {error:#}");
                self.session.fail_start(SessionFailure::for_request(
                    &failed_request,
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

    fn resolve_entry_relative_preview_layout(
        &mut self,
        state: &mut WarmWorldStandbyState,
        slot: &mut DrawableWorldSlot,
    ) -> Result<()> {
        let active_entry_pose = self
            .active_world
            .accepted_entry_pose
            .unwrap_or_else(|| WorldEntryPose::from_camera(&self.active_world.camera));
        let standby_entry_pose = slot
            .accepted_entry_pose
            .unwrap_or_else(|| WorldEntryPose::from_camera(&slot.camera));
        let WarmWorldPresentationRequest::Diorama {
            region,
            return_region,
            placement,
            return_placement,
            entry_relative: Some(layout),
        } = &mut state.presentation
        else {
            return Ok(());
        };
        if layout.standby_entry_pose == Some(standby_entry_pose)
            && layout.active_entry_pose == Some(active_entry_pose)
        {
            return Ok(());
        }

        let standby_entry_section = entry_support_render_section(standby_entry_pose)
            .context("accepted standby entry has no support section")?;
        let active_entry_section = entry_support_render_section(active_entry_pose)
            .context("accepted active entry has no support section")?;
        let standby_entry_chunk =
            ChunkPos::new(standby_entry_section.chunk_x, standby_entry_section.chunk_z);
        let active_entry_chunk =
            ChunkPos::new(active_entry_section.chunk_x, active_entry_section.chunk_z);
        let resolved_standby = layout
            .bounds
            .resolve(standby_entry_chunk, standby_entry_section.section_y)?;
        let resolved_active = layout
            .bounds
            .resolve(active_entry_chunk, active_entry_section.section_y)?;
        let standby_source_anchor =
            preview_crop_source_anchor(resolved_standby, standby_entry_pose.feet_position.y);
        let active_source_anchor =
            preview_crop_source_anchor(resolved_active, active_entry_pose.feet_position.y);
        let relative_anchor = entry_relative_composition_anchor(
            if layout.standby_uses_primary_anchor {
                standby_entry_pose
            } else {
                active_entry_pose
            },
            layout.return_display_offset,
        );
        let scale = placement.uniform_scale();
        let (standby_composition_anchor, active_composition_anchor) =
            if layout.standby_uses_primary_anchor {
                (layout.primary_composition_anchor, relative_anchor)
            } else {
                (relative_anchor, layout.primary_composition_anchor)
            };
        let resolved_placement = mclone_render::placement::WorldPlacement::new(
            standby_source_anchor,
            standby_composition_anchor,
            scale,
        )?;
        let resolved_return_placement = mclone_render::placement::WorldPlacement::new(
            active_source_anchor,
            active_composition_anchor,
            scale,
        )?;

        *region = resolved_standby;
        *return_region = resolved_active;
        *placement = resolved_placement;
        *return_placement = resolved_return_placement;
        layout.standby_entry_pose = Some(standby_entry_pose);
        layout.active_entry_pose = Some(active_entry_pose);
        state.accepted_entry_pose = Some(standby_entry_pose);
        slot.runtime
            .as_mut()
            .context("accepted standby preview has no runtime")?
            .set_interest_center(standby_entry_chunk)
            .context("pin accepted-entry preview interest")?;

        if let Some(preview) = self
            .embedded_world_preview
            .as_mut()
            .filter(|preview| preview.source_world == slot.id)
        {
            preview.region = resolved_standby;
            preview.return_region = resolved_active;
            preview.context = mclone_render::placement::WorldCompositionContext::unbounded(
                resolved_placement,
                Some(resolved_standby.source_bounds()),
            );
            preview.return_context = mclone_render::placement::WorldCompositionContext::unbounded(
                resolved_return_placement,
                Some(resolved_active.source_bounds()),
            );
            preview.fixed_interest_center = standby_entry_chunk;
            preview.return_interest_center = active_entry_chunk;
            preview.phase = EmbeddedWorldPreviewPhase::Warming;
            preview.source_anchor_gpu_resident = false;
            preview.source_anchor_traversal_ready = false;
            preview.bounded_section_count = 0;
            preview.last_draw = TexturedSectionRenderStats::default();
            preview.preparation = EmbeddedWorldPreviewPreparationSnapshot::default();
            preview.render = EmbeddedWorldPreviewRenderSnapshot::default();
            preview.failure = None;
        }
        log::info!(
            "resolved accepted-entry preview id={} entry=({:.3}, {:.3}, {:.3}) chunks=({}, {})..=({}, {}) sections={}..={} return_chunks=({}, {})..=({}, {})",
            slot.id.get(),
            standby_entry_pose.feet_position.x,
            standby_entry_pose.feet_position.y,
            standby_entry_pose.feet_position.z,
            resolved_standby.min_chunk().x,
            resolved_standby.min_chunk().z,
            resolved_standby.max_chunk().x,
            resolved_standby.max_chunk().z,
            resolved_standby.min_section_y(),
            resolved_standby.max_section_y(),
            resolved_active.min_chunk().x,
            resolved_active.min_chunk().z,
            resolved_active.max_chunk().x,
            resolved_active.max_chunk().z,
        );
        Ok(())
    }

    fn advance_warm_world_standby(&mut self) {
        let Some(mut state) = self.warm_world_standby.take() else {
            return;
        };
        let Some(mut slot) = self.standby_world.take() else {
            // Failed/cancelled diagnostics intentionally outlive the dropped
            // runtime so the harness and debug surfaces can report why it is
            // not switchable.
            self.warm_world_standby = Some(state);
            return;
        };
        if matches!(
            state.phase,
            WarmWorldStandbyPhase::Failed | WarmWorldStandbyPhase::Cancelled
        ) {
            self.warm_world_standby = Some(state);
            return;
        }

        let advance_started_at = self.services.clock.now();
        let mut retain_slot = true;

        #[cfg(not(target_arch = "wasm32"))]
        if state.phase == WarmWorldStandbyPhase::Warming && slot.local_startup.is_some() {
            let startup_step_started_at = self.services.clock.now();
            let step_result = {
                let startup = slot
                    .local_startup
                    .as_mut()
                    .expect("warming standby owns its startup pump");
                let camera_position = glam_vec3_from_vec3d(startup.camera.snapshot().eye);
                startup.pump.step(camera_position).map(|step| {
                    state.poll_count = step.poll_count;
                    state.poll_ms = step.poll_ms;
                    state.loaded_chunks = startup.pump.runtime().loaded_chunk_count();
                    state.startup_seed_sections = startup.pump.render_seed_section_count();
                    state.startup_seed_drawable_sections =
                        startup.pump.render_seed_drawable_section_count();
                    state.startup_seed_owned_bytes =
                        startup.pump.render_seed_estimated_owned_bytes();
                    step
                })
            };
            state.worst_startup_step_ms = state.worst_startup_step_ms.max(elapsed_ms(
                self.services.clock.elapsed_since(startup_step_started_at),
            ));
            match step_result {
                Err(error) => {
                    state.phase = WarmWorldStandbyPhase::Failed;
                    state.failure = Some(format!("advance startup pump: {error:#}"));
                    retain_slot = false;
                }
                Ok(step) if step.playable_ready => {
                    let reconciliation = if matches!(
                        state.presentation,
                        WarmWorldPresentationRequest::Diorama { .. }
                    ) {
                        Ok(false)
                    } else {
                        let startup = slot
                            .local_startup
                            .as_mut()
                            .expect("playable standby owns its startup pump");
                        reconcile_xr_startup_pose(
                            startup.pump.runtime_services_mut(),
                            &mut startup.camera,
                            None,
                            &self.services.clock,
                        )
                    };
                    match reconciliation {
                        Err(error) => {
                            state.phase = WarmWorldStandbyPhase::Failed;
                            state.failure = Some(format!(
                                "acknowledge initial standby camera correction: {error:#}"
                            ));
                            retain_slot = false;
                        }
                        Ok(true) => {
                            // Always observe one stable post-ack step. If the
                            // correction moved interest, the pump must establish
                            // readiness around the corrected camera before its
                            // render seed is detached.
                            state.camera_reconciled = true;
                        }
                        Ok(false) if !state.camera_reconciled => {
                            // Even when no correction was queued, require one
                            // subsequent stable pump step before detaching.
                            state.camera_reconciled = true;
                        }
                        Ok(false) => {
                            let mut startup = slot
                                .local_startup
                                .take()
                                .expect("stable playable standby owns its startup pump");
                            if matches!(
                                state.presentation,
                                WarmWorldPresentationRequest::Diorama { .. }
                            ) && !reconcile_observer_preview_entry(
                                &startup.scene,
                                startup.pump.runtime().client(),
                                &mut startup.camera,
                            ) {
                                slot.local_startup = Some(startup);
                                self.standby_world = Some(slot);
                                self.warm_world_standby = Some(state);
                                return;
                            }
                            let SceneLocalStartup {
                                descriptor,
                                pump,
                                camera,
                                ..
                            } = startup;
                            match descriptor {
                                None => {
                                    state.phase = WarmWorldStandbyPhase::Failed;
                                    state.failure = Some(
                                        "standby startup has no session descriptor".to_owned(),
                                    );
                                    retain_slot = false;
                                }
                                Some(descriptor) => {
                                    state.startup_seed_sections = pump.render_seed_section_count();
                                    state.startup_seed_drawable_sections =
                                        pump.render_seed_drawable_section_count();
                                    state.startup_seed_owned_bytes =
                                        pump.render_seed_estimated_owned_bytes();
                                    let (runtime, startup_sections) =
                                        pump.into_runtime_with_startup_sections();
                                    let runtime: SceneSessionRuntime = NativeSessionServices::<
                                        mclone_app_runtime::LocalOnlySession,
                                    >::from_active_runtime_with_descriptor(
                                        descriptor.clone(),
                                        NativeSceneServices::Local(runtime),
                                    )
                                    .into();
                                    state.loaded_chunks = runtime.client().loaded_chunk_count();
                                    slot.complete_detached_local_startup(
                                        descriptor,
                                        runtime,
                                        camera,
                                        startup_sections,
                                    );
                                    state.phase = WarmWorldStandbyPhase::ResolvingEndpoints;
                                }
                            }
                        }
                    }
                }
                Ok(_) => {}
            }
        }

        if retain_slot
            && state.phase == WarmWorldStandbyPhase::Warming
            && slot.local_startup.is_none()
            && slot.runtime.is_some()
        {
            state.phase = WarmWorldStandbyPhase::ResolvingEndpoints;
        }

        if retain_slot
            && state.phase == WarmWorldStandbyPhase::ResolvingEndpoints
            && slot.external_runtime_startup_pending
        {
            let camera_position = glam_vec3_from_vec3d(slot.camera.snapshot().eye);
            let runtime_ready = slot.runtime.as_ref().is_some_and(|runtime| {
                runtime.startup_host_ready(
                    mclone_app_runtime::StartupReadinessPolicy::Playable,
                    camera_position,
                )
            });
            let observer_entry_ready = !matches!(
                state.presentation,
                WarmWorldPresentationRequest::Diorama { .. }
            ) || slot.runtime.as_ref().is_some_and(|runtime| {
                reconcile_observer_preview_entry(
                    &slot.scene,
                    runtime.client(),
                    &mut slot.local_participant.camera,
                )
            });
            if runtime_ready && observer_entry_ready {
                slot.external_runtime_startup_pending = false;
                slot.lifecycle = WorldSlotLifecycle::StandbyCpuReady;
                slot.accepted_entry_pose = Some(WorldEntryPose::from_camera(&slot.camera));
                state.camera_reconciled = true;
            }
        }

        if retain_slot
            && state.phase == WarmWorldStandbyPhase::ResolvingEndpoints
            && !slot.external_runtime_startup_pending
            && matches!(
                state.presentation,
                WarmWorldPresentationRequest::Diorama { .. }
            )
        {
            match self.resolve_entry_relative_preview_layout(&mut state, &mut slot) {
                Err(error) => {
                    state.phase = WarmWorldStandbyPhase::Failed;
                    state.failure =
                        Some(format!("resolve accepted-entry preview layout: {error:#}"));
                    retain_slot = false;
                }
                Ok(()) => {
                    let accepted_entry_pose = slot
                        .accepted_entry_pose
                        .unwrap_or_else(|| WorldEntryPose::from_camera(&slot.camera));
                    state.accepted_entry_pose = Some(accepted_entry_pose);
                    state.phase = WarmWorldStandbyPhase::CpuReady;
                    state.readiness.cpu_ready = true;
                    state.readiness.entry_section =
                        entry_support_render_section(accepted_entry_pose);
                    if let Some(runtime) = slot.runtime.as_ref() {
                        let support = mclone_client::standing_pose_facts(
                            runtime.client(),
                            accepted_entry_pose.feet_position,
                        );
                        state.readiness.entry_body_loaded = support.body_loaded;
                        state.readiness.entry_body_clear = support.body_clear;
                        state.readiness.entry_support_loaded = support.support_loaded;
                        state.readiness.entry_solid_support = support.solid_support;
                    }
                    log::info!(
                        "warm-world diorama CPU-ready id={} seed={} elapsed_ms={:.3} loaded_chunks={} seed_sections={} drawable_sections={} seed_bytes={}",
                        state.instance_id.get(),
                        state.seed,
                        elapsed_ms(self.services.clock.elapsed_since(state.started_at)),
                        state.loaded_chunks,
                        state.startup_seed_sections,
                        state.startup_seed_drawable_sections,
                        state.startup_seed_owned_bytes,
                    );
                }
            }
        }

        if retain_slot
            && state.phase == WarmWorldStandbyPhase::ResolvingEndpoints
            && !slot.external_runtime_startup_pending
            && self.active_world.lifecycle == WorldSlotLifecycle::ActiveReady
            && matches!(state.presentation, WarmWorldPresentationRequest::OpaqueGate)
        {
            let endpoint_started_at = self.services.clock.now();
            let source_endpoint = self
                .active_world
                .runtime
                .as_ref()
                .zip(self.active_world.accepted_entry_pose)
                .and_then(|(runtime, pose)| resolve_world_gate_endpoint(runtime, pose));
            let destination_endpoint = slot
                .runtime
                .as_ref()
                .zip(slot.accepted_entry_pose)
                .and_then(|(runtime, pose)| resolve_world_gate_endpoint(runtime, pose));
            state.endpoint_resolution_ms =
                elapsed_ms(self.services.clock.elapsed_since(endpoint_started_at));
            state.source_endpoint = source_endpoint;
            state.destination_endpoint = destination_endpoint;
            if source_endpoint.is_some() && destination_endpoint.is_some() {
                let destination_entry_pose = destination_endpoint
                    .map(world_gate_destination_entry_pose)
                    .expect("resolved destination endpoint has an entry pose");
                state.phase = WarmWorldStandbyPhase::CpuReady;
                state.readiness.cpu_ready = true;
                state.accepted_entry_pose = Some(destination_entry_pose);
                state.readiness.entry_section =
                    entry_support_render_section(destination_entry_pose);
                if let Some(runtime) = slot.runtime.as_ref() {
                    let support = mclone_client::standing_pose_facts(
                        runtime.client(),
                        destination_entry_pose.feet_position,
                    );
                    state.readiness.entry_body_loaded = support.body_loaded;
                    state.readiness.entry_body_clear = support.body_clear;
                    state.readiness.entry_support_loaded = support.support_loaded;
                    state.readiness.entry_solid_support = support.solid_support;
                }
                log::info!(
                    "warm-world standby CPU-ready id={} seed={} elapsed_ms={:.3} polls={} poll_ms={:.3} loaded_chunks={} seed_sections={} drawable_sections={} seed_bytes={} worst_startup_step_ms={:.3} worst_runtime_poll_ms={:.3} endpoint_ms={:.3}",
                    state.instance_id.get(),
                    state.seed,
                    elapsed_ms(self.services.clock.elapsed_since(state.started_at)),
                    state.poll_count,
                    state.poll_ms,
                    state.loaded_chunks,
                    state.startup_seed_sections,
                    state.startup_seed_drawable_sections,
                    state.startup_seed_owned_bytes,
                    state.worst_startup_step_ms,
                    state.worst_runtime_poll_ms,
                    state.endpoint_resolution_ms,
                );
            } else {
                state.phase = WarmWorldStandbyPhase::PlacementFailed;
                state.failure = Some(format!(
                    "no clear loaded provisional gate endpoint within {} blocks (source={}, destination={})",
                    PROVISIONAL_GATE_SEARCH_RADIUS_BLOCKS,
                    source_endpoint.is_some(),
                    destination_endpoint.is_some(),
                ));
                log::warn!(
                    "warm-world standby endpoint placement failed id={} seed={} reason={}",
                    state.instance_id.get(),
                    state.seed,
                    state.failure.as_deref().unwrap_or("unknown"),
                );
            }
        }

        let advance_ms = elapsed_ms(self.services.clock.elapsed_since(advance_started_at));
        state.startup_advance_count = state.startup_advance_count.saturating_add(1);
        state.startup_advance_total_ms += advance_ms;
        state.last_advance_ms = advance_ms;
        state.worst_advance_ms = state.worst_advance_ms.max(advance_ms);
        if !retain_slot {
            log::error!(
                "warm-world standby failed id={} seed={} reason={}",
                state.instance_id.get(),
                state.seed,
                state.failure.as_deref().unwrap_or("unknown"),
            );
        } else {
            self.standby_world = Some(slot);
        }
        self.warm_world_standby = Some(state);
    }

    /// Advance at most one budgeted standby GPU-preparation pass after the
    /// active slot has consumed its render-thread work for this frame.
    pub(crate) fn advance_warm_world_gpu(
        &mut self,
        device: &wgpu::Device,
        active_camera_position: Vec3,
        active_frame_deadline: Option<MonotonicDeadline>,
    ) -> Result<()> {
        let Some(mut state) = self.warm_world_standby.take() else {
            return Ok(());
        };
        let Some(mut slot) = self.standby_world.take() else {
            self.warm_world_standby = Some(state);
            return Ok(());
        };
        if matches!(
            state.phase,
            WarmWorldStandbyPhase::Warming
                | WarmWorldStandbyPhase::PlacementFailed
                | WarmWorldStandbyPhase::Failed
                | WarmWorldStandbyPhase::Cancelled
        ) {
            self.standby_world = Some(slot);
            self.warm_world_standby = Some(state);
            return Ok(());
        }
        if state.phase == WarmWorldStandbyPhase::ResolvingEndpoints
            && !slot.external_runtime_startup_pending
        {
            self.standby_world = Some(slot);
            self.warm_world_standby = Some(state);
            return Ok(());
        }
        if state.last_gpu_advance_frame == Some(self.rendered_frames) {
            self.standby_world = Some(slot);
            self.warm_world_standby = Some(state);
            return Ok(());
        }
        state.last_gpu_advance_frame = Some(self.rendered_frames);
        if active_frame_deadline
            .as_ref()
            .is_some_and(MonotonicDeadline::is_reached)
        {
            state.gpu_skipped_no_slack_count = state.gpu_skipped_no_slack_count.saturating_add(1);
            self.standby_world = Some(slot);
            self.warm_world_standby = Some(state);
            return Ok(());
        }

        let advance_started_at = self.services.clock.now();
        if !state.readiness.startup_seed_enqueued {
            let startup_sections = std::mem::take(&mut slot.pending_startup_sections);
            if startup_sections.is_empty() {
                if slot.local_startup.is_none() && slot.runtime.is_some() {
                    state.readiness.startup_seed_enqueued = true;
                    state.readiness.startup_seed_drained = true;
                    state
                        .gpu_warm_started_at
                        .get_or_insert_with(|| self.services.clock.now());
                } else {
                    state.phase = WarmWorldStandbyPhase::Failed;
                    state.failure = Some("CPU-ready standby has no startup seed meshes".to_owned());
                    self.warm_world_standby = Some(state);
                    return Ok(());
                }
            } else {
                let expected_lifecycle_items = startup_sections.len();
                let update = RenderSectionCacheUpdate::from_startup_seed(startup_sections);
                let enqueue = slot.section_uploads.enqueue_cache_update(update);
                if enqueue.queued_lifecycle_items != expected_lifecycle_items
                    || enqueue.superseded_lifecycle_items != 0
                    || enqueue.released_compile_jobs != 0
                {
                    state.phase = WarmWorldStandbyPhase::Failed;
                    state.failure = Some(format!(
                        "startup seed enqueue violated lifecycle conservation: expected={} queued={} superseded={} released={}",
                        expected_lifecycle_items,
                        enqueue.queued_lifecycle_items,
                        enqueue.superseded_lifecycle_items,
                        enqueue.released_compile_jobs,
                    ));
                    self.warm_world_standby = Some(state);
                    return Ok(());
                }
                state.gpu_warm_started_at = Some(self.services.clock.now());
                state.upload_queue_nonempty_since = state.gpu_warm_started_at;
                state.initial_upload_lifecycle_items = expected_lifecycle_items;
                state.readiness.startup_seed_enqueued = true;
                slot.lifecycle = WorldSlotLifecycle::StandbyGpuWarming;
                state.phase = WarmWorldStandbyPhase::GpuWarming;
            }
        } else if state.phase == WarmWorldStandbyPhase::CpuReady {
            state.phase = WarmWorldStandbyPhase::GpuWarming;
        }

        let initial_seed_was_draining = !state.readiness.startup_seed_drained;
        let policy = WorldPreparationPolicy {
            clock: self.services.clock.clone(),
            target_period_ms: self.render_admission_target_period_ms(),
            poll_budget: RuntimeUpdatePumpBudget::MaxElapsed(STANDBY_RUNTIME_POLL_BUDGET),
            upload_budget: Some(STANDBY_UPLOAD_BUDGET),
            accept_budget: Some(STANDBY_ACCEPT_BUDGET),
            completed_result_accept_budget: Some(STANDBY_ACCEPT_BUDGET),
            max_compile_requests: Some(STANDBY_COMPILE_REQUEST_BUDGET),
            work_elapsed_budget: Some(STANDBY_PREPARATION_BUDGET),
            defer_sync_after_pre_drain: true,
        };
        let priority_position = match state.presentation {
            WarmWorldPresentationRequest::Diorama { .. }
                if !state.readiness.entry_section_gpu_resident =>
            {
                slot.camera.snapshot().eye
            }
            WarmWorldPresentationRequest::Diorama {
                region, placement, ..
            } => bounded_preview_source_priority(
                region,
                placement,
                Vec3d::new(
                    f64::from(active_camera_position.x),
                    f64::from(active_camera_position.y),
                    f64::from(active_camera_position.z),
                ),
            ),
            WarmWorldPresentationRequest::OpaqueGate => slot.camera.snapshot().eye,
        };
        let camera_position = glam_vec3_from_vec3d(priority_position);
        let preview_entities_before = slot
            .runtime
            .as_ref()
            .map(|runtime| {
                runtime
                    .client()
                    .entity_snapshots()
                    .copied()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let preview_remote_players_before = slot
            .runtime
            .as_ref()
            .map(|runtime| {
                runtime
                    .client()
                    .remote_player_snapshots()
                    .copied()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut timing = XrTerrainFrameTiming::default();
        let upload = match Self::prepare_world_slot(
            &mut slot,
            device,
            camera_position,
            active_frame_deadline,
            &policy,
            &mut timing,
        ) {
            Ok(upload) => upload,
            Err(error) => {
                state.phase = WarmWorldStandbyPhase::Failed;
                state.failure = Some(format!("prepare standby GPU terrain: {error:#}"));
                self.warm_world_standby = Some(state);
                return Ok(());
            }
        };
        let preview_entities_after = slot
            .runtime
            .as_ref()
            .map(|runtime| {
                runtime
                    .client()
                    .entity_snapshots()
                    .copied()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let preview_remote_players_after = slot
            .runtime
            .as_ref()
            .map(|runtime| {
                runtime
                    .client()
                    .remote_player_snapshots()
                    .copied()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some((from, to)) =
            changed_entity_motion(&preview_entities_before, &preview_entities_after)
            && let Some(preview) = self
                .embedded_world_preview
                .as_mut()
                .filter(|preview| preview.source_world == slot.id)
        {
            let source_packed_light = slot
                .runtime
                .as_ref()
                .and_then(|runtime| {
                    let client = runtime.client();
                    client
                        .actor_presentations()
                        .into_iter()
                        .find(|presentation| {
                            presentation.id == mclone_client::ActorPresentationId::Entity(to.id)
                        })
                        .map(|presentation| {
                            client.packed_light_at_world_or_fullbright(
                                mclone_render_session::actor_light_probe_block_pos(&presentation),
                            )
                        })
                })
                .unwrap_or(mclone_render::light_texture::FULL_BRIGHT);
            preview.record_actor_update(from, to, source_packed_light, self.services.clock.now());
        }
        if let Some((from, to)) = changed_remote_player_motion(
            &preview_remote_players_before,
            &preview_remote_players_after,
        ) && let Some(preview) = self
            .embedded_world_preview
            .as_mut()
            .filter(|preview| preview.source_world == slot.id)
        {
            let (walk_animation_distance, source_packed_light) = slot
                .runtime
                .as_ref()
                .and_then(|runtime| {
                    let client = runtime.client();
                    client
                        .actor_presentations()
                        .into_iter()
                        .find(|presentation| {
                            presentation.id
                                == mclone_client::ActorPresentationId::RemotePlayer(to.id)
                        })
                        .map(|presentation| {
                            (
                                presentation.walk_animation_distance,
                                client.packed_light_at_world_or_fullbright(
                                    mclone_render_session::actor_light_probe_block_pos(
                                        &presentation,
                                    ),
                                ),
                            )
                        })
                })
                .unwrap_or((0.0, mclone_render::light_texture::FULL_BRIGHT));
            preview.record_remote_player_update(
                from,
                to,
                walk_animation_distance,
                source_packed_light,
                self.services.clock.now(),
            );
        }
        state.poll_count = state.poll_count.saturating_add(1);
        state.worst_runtime_poll_ms = state.worst_runtime_poll_ms.max(timing.runtime_poll_ms);
        state.accepted_compile_result_count = state
            .accepted_compile_result_count
            .saturating_add(upload.accepted_compile_result_count);
        state.released_compile_job_count = state
            .released_compile_job_count
            .saturating_add(upload.upload_released_compile_job_count);
        if initial_seed_was_draining {
            state.initial_upload_released_compile_jobs = state
                .initial_upload_released_compile_jobs
                .saturating_add(upload.upload_released_compile_job_count);
        }

        let camera_changed = {
            let runtime = slot
                .runtime
                .as_mut()
                .expect("GPU-warming standby owns a runtime");
            match mclone_app_runtime::apply_pending_engine_camera_position_updates(
                runtime,
                &mut slot.local_participant.camera,
                XR_CAMERA_COMMIT_CONTEXT,
            ) {
                Ok(changed) => changed,
                Err(error) => {
                    state.phase = WarmWorldStandbyPhase::Failed;
                    state.failure = Some(format!(
                        "apply post-startup standby camera correction: {error:#}"
                    ));
                    self.warm_world_standby = Some(state);
                    return Ok(());
                }
            }
        };
        if camera_changed {
            slot.local_participant.reset_movement();
            slot.accepted_entry_pose = Some(WorldEntryPose::from_camera(&slot.camera));
            state.accepted_entry_pose = None;
            state.destination_endpoint = None;
            state.readiness.cpu_ready = false;
            state.readiness.entry_section = None;
            state.readiness.entry_section_gpu_resident = false;
            state.readiness.entry_section_traversal_ready = false;
            state.readiness.entry_body_loaded = false;
            state.readiness.entry_body_clear = false;
            state.readiness.entry_support_loaded = false;
            state.readiness.entry_solid_support = false;
            state.readiness.switchable = false;
            state.phase = WarmWorldStandbyPhase::ResolvingEndpoints;
        }

        let queue = slot.section_uploads.stats();
        state.loaded_chunks = slot
            .runtime
            .as_ref()
            .map_or(0, |runtime| runtime.client().loaded_chunk_count());
        state.queued_upload_sections = queue.queued_upload_sections;
        state.queued_upload_lifecycle_items = queue.queued_lifecycle_items;
        state.queued_upload_mesh_owned_bytes = queue.queued_upload_mesh_owned_bytes;
        if queue.queued_lifecycle_items == 0 {
            state.upload_queue_nonempty_since = None;
        } else if state.upload_queue_nonempty_since.is_none() {
            state.upload_queue_nonempty_since = Some(self.services.clock.now());
        }
        if !state.readiness.startup_seed_drained {
            state.initial_upload_applied_lifecycle_items = state
                .initial_upload_lifecycle_items
                .saturating_sub(queue.queued_lifecycle_items)
                .min(state.initial_upload_lifecycle_items);
            state.readiness.startup_seed_drained = state.initial_upload_lifecycle_items > 0
                && state.initial_upload_applied_lifecycle_items
                    == state.initial_upload_lifecycle_items;
        }
        state.gpu_section_count = slot.draw.section_count();
        state.gpu_vertex_count = slot.draw.vertex_count();
        state.gpu_index_count = slot.draw.index_count();
        state.renderer_multiview_materialized = slot.draw.multiview_renderer_materialized();
        state.readiness.renderer_topology_ready =
            !state.renderer_multiview_required || state.renderer_multiview_materialized;
        state.readiness.entry_section_gpu_resident = state
            .readiness
            .entry_section
            .is_some_and(|key| slot.draw.contains_section(key));
        state.readiness.entry_section_traversal_ready = state
            .readiness
            .entry_section
            .is_some_and(|key| slot.draw.traversal_ready_contains_section(key));
        let entry_support = state
            .accepted_entry_pose
            .zip(slot.runtime.as_ref())
            .map(|(pose, runtime)| {
                mclone_client::standing_pose_facts(runtime.client(), pose.feet_position)
            })
            .unwrap_or_default();
        state.readiness.entry_body_loaded = entry_support.body_loaded;
        state.readiness.entry_body_clear = entry_support.body_clear;
        state.readiness.entry_support_loaded = entry_support.support_loaded;
        state.readiness.entry_solid_support = entry_support.solid_support;
        state.readiness.switchable = state.readiness.cpu_ready
            && state.readiness.startup_seed_enqueued
            && state.readiness.startup_seed_drained
            && state.readiness.entry_section_gpu_resident
            && state.readiness.entry_section_traversal_ready
            && state.readiness.entry_body_loaded
            && state.readiness.entry_body_clear
            && state.readiness.entry_support_loaded
            && state.readiness.entry_solid_support
            && state.readiness.renderer_topology_ready;

        if let Some(preview) = self
            .embedded_world_preview
            .as_mut()
            .filter(|preview| preview.source_world == slot.id)
        {
            let runtime = slot
                .runtime
                .as_ref()
                .expect("GPU-warming preview owns a runtime");
            preview.source_host_mode = Some(runtime.host_mode());
            if runtime.interest_center() != preview.fixed_interest_center {
                preview.phase = EmbeddedWorldPreviewPhase::Failed;
                preview.failure = Some(format!(
                    "embedded preview interest moved from fixed center ({}, {}) to ({}, {})",
                    preview.fixed_interest_center.x,
                    preview.fixed_interest_center.z,
                    runtime.interest_center().x,
                    runtime.interest_center().z,
                ));
            }
            let preparation = &mut preview.preparation;
            preparation.frame_count = preparation.frame_count.saturating_add(1);
            preparation.last_runtime_poll_ms = timing.runtime_poll_ms;
            preparation.total_runtime_poll_ms += timing.runtime_poll_ms;
            preparation.last_compile_sync_ms = timing.runtime_sync_ms;
            preparation.total_compile_sync_ms += timing.runtime_sync_ms;
            preparation.last_gpu_upload_ms = timing.runtime_gpu_upload_ms;
            preparation.total_gpu_upload_ms += timing.runtime_gpu_upload_ms;
            preparation.last_submitted_compile_section_count =
                upload.submitted_compile_section_count;
            preparation.submitted_compile_section_count = preparation
                .submitted_compile_section_count
                .saturating_add(upload.submitted_compile_section_count);
            preparation.last_accepted_compile_result_count = upload.accepted_compile_result_count;
            preparation.accepted_compile_result_count = preparation
                .accepted_compile_result_count
                .saturating_add(upload.accepted_compile_result_count);
            preparation.last_uploaded_section_count = upload.uploaded_section_count;
            preparation.uploaded_section_count = preparation
                .uploaded_section_count
                .saturating_add(upload.uploaded_section_count);
            preparation.pending_compile_jobs = upload.pending_compile_jobs_after;
            preparation.max_pending_compile_jobs = preparation
                .max_pending_compile_jobs
                .max(upload.pending_compile_jobs_after);
            preparation.queued_upload_lifecycle_items = upload.queued_upload_lifecycle_item_count;
            preparation.max_queued_upload_lifecycle_items = preparation
                .max_queued_upload_lifecycle_items
                .max(upload.queued_upload_lifecycle_item_count);
            preparation.queued_upload_mesh_owned_bytes = upload.queued_upload_mesh_owned_bytes;
            preparation.max_queued_upload_mesh_owned_bytes = preparation
                .max_queued_upload_mesh_owned_bytes
                .max(upload.queued_upload_mesh_owned_bytes);
            preparation.source_priority_position = priority_position;
            let runtime_stats = runtime.stats();
            preparation.tracked_players = runtime_stats.tracked_players;
            preparation.client_remote_player_count = runtime.client().remote_player_count();
            preparation.debug_auxiliary_player_script = slot.scene.debug_auxiliary_player_script;

            if let Some(mutation) = preview.last_mutation.as_mut() {
                if matches!(
                    mutation.snapshot.phase,
                    EmbeddedWorldPreviewMutationPhase::CommandSent
                        | EmbeddedWorldPreviewMutationPhase::ClientApplied
                ) {
                    mutation.snapshot.command_update_count = mutation
                        .snapshot
                        .command_update_count
                        .saturating_add(upload.poll_updates);
                    mutation.snapshot.command_section_block_update_count = mutation
                        .snapshot
                        .command_section_block_update_count
                        .saturating_add(upload.poll_section_block_updates);
                }
                mutation.snapshot.submitted_compile_section_count = preparation
                    .submitted_compile_section_count
                    .saturating_sub(mutation.submitted_compile_baseline);
                mutation.snapshot.accepted_compile_result_count = preparation
                    .accepted_compile_result_count
                    .saturating_sub(mutation.accepted_compile_baseline);
                mutation.snapshot.uploaded_section_count = preparation
                    .uploaded_section_count
                    .saturating_sub(mutation.uploaded_section_baseline);
                let client_applied = runtime
                    .client()
                    .block_state_at_block_pos(mutation.snapshot.block)
                    == Some(mclone_core::AIR_BLOCK_STATE_ID);
                if mutation.snapshot.phase == EmbeddedWorldPreviewMutationPhase::CommandSent
                    && client_applied
                {
                    mutation.snapshot.phase = EmbeddedWorldPreviewMutationPhase::ClientApplied;
                }
                if mutation.snapshot.phase == EmbeddedWorldPreviewMutationPhase::ClientApplied
                    && client_applied
                    && mutation.snapshot.accepted_compile_result_count > 0
                    && mutation.snapshot.uploaded_section_count > 0
                {
                    mutation.snapshot.phase = EmbeddedWorldPreviewMutationPhase::GpuApplied;
                    mutation.snapshot.completed_after_rendered_frame = Some(self.rendered_frames);
                }
            }
        }

        let advance_ms = elapsed_ms(self.services.clock.elapsed_since(advance_started_at));
        state.last_gpu_advance_ms = advance_ms;
        state.gpu_advance_total_ms += advance_ms;
        state.worst_gpu_advance_ms = state.worst_gpu_advance_ms.max(advance_ms);
        state.gpu_advance_count = state.gpu_advance_count.saturating_add(1);
        if state.readiness.switchable && state.phase != WarmWorldStandbyPhase::ResolvingEndpoints {
            if !state.standby_cadence_applied {
                let cadence_result = slot
                    .runtime
                    .as_mut()
                    .expect("switchable standby owns a runtime")
                    .set_simulation_cadence(state.standby_cadence);
                if let Err(error) = cadence_result {
                    state.phase = WarmWorldStandbyPhase::Failed;
                    state.readiness.switchable = false;
                    state.failure = Some(format!("apply standby simulation cadence: {error:#}"));
                    self.standby_world = Some(slot);
                    self.warm_world_standby = Some(state);
                    return Ok(());
                }
                state.standby_cadence_applied = true;
            }
            if state.gpu_ready_at.is_none() {
                state.gpu_ready_at = Some(self.services.clock.now());
                state.gpu_ready_advance_count = state.gpu_advance_count;
                log::info!(
                    "warm-world standby switchable id={} seed={} gpu_ms={:.3} advances={} sections={} vertices={} indices={} worst_gpu_ms={:.3}",
                    state.instance_id.get(),
                    state.seed,
                    state.gpu_warm_started_at.map_or(0.0, |started| elapsed_ms(
                        self.services.clock.elapsed_since(started)
                    )),
                    state.gpu_advance_count,
                    state.gpu_section_count,
                    state.gpu_vertex_count,
                    state.gpu_index_count,
                    state.worst_gpu_advance_ms,
                );
            }
            if slot.actors.is_none() {
                let shared = self
                    .active_world
                    .actors
                    .as_ref()
                    .expect("active world owns actor draw state")
                    .shared_resources();
                let mut actors = ActorDrawResources::new_with_shared_resources(device, shared);
                actors.ensure_composed_topology(device);
                slot.actors = Some(actors);
            }
            self.active_world
                .actors
                .as_mut()
                .expect("active world owns actor draw state")
                .ensure_composed_topology(device);
            slot.actors
                .as_mut()
                .expect("switchable standby owns actor draw state")
                .ensure_composed_topology(device);
            let actor_snapshot = slot
                .actors
                .as_ref()
                .expect("switchable standby owns actor draw state")
                .resource_snapshot();
            state.actor_state_materialized = true;
            state.shared_actor_resource_owner_count = actor_snapshot.shared_strong_owner_count;
            state.shared_actor_known_retained_bytes = actor_snapshot.shared_known_retained_bytes;
            state.standby_actor_state_allocated_bytes =
                actor_snapshot.mutable_state_allocated_bytes;
            slot.lifecycle = WorldSlotLifecycle::StandbySwitchable;
            state.phase = WarmWorldStandbyPhase::Switchable;
        }

        if let Some(preview) = self.embedded_world_preview.as_mut() {
            let source_anchor_section =
                anchor_render_section(preview.context.placement().source_anchor());
            let bounded_records = slot
                .draw
                .prepare_render_records_for_context(preview.context);
            preview.bounded_section_count = bounded_records.section_keys().len();
            preview.source_anchor_gpu_resident =
                source_anchor_section.is_some_and(|key| slot.draw.contains_section(key));
            preview.source_anchor_traversal_ready = source_anchor_section
                .is_some_and(|key| slot.draw.traversal_ready_contains_section(key));
            let failure = if preview.phase == EmbeddedWorldPreviewPhase::Failed {
                preview.failure.clone()
            } else if preview.source_world != slot.id {
                Some("embedded preview source no longer names the retained slot".to_owned())
            } else if preview.asset_epoch != slot.asset_epoch
                || preview.asset_epoch != self.active_world.asset_epoch
            {
                Some("embedded preview asset epoch does not match both slots".to_owned())
            } else if source_anchor_section.is_none_or(|key| !preview.region.contains(key)) {
                Some("embedded preview source anchor lies outside its bounded region".to_owned())
            } else {
                None
            };
            if let Some(failure) = failure {
                preview.phase = EmbeddedWorldPreviewPhase::Failed;
                preview.failure = Some(failure);
            } else if state.phase == WarmWorldStandbyPhase::Switchable
                && preview.renderer_topology_ready
                && preview.source_anchor_gpu_resident
                && preview.source_anchor_traversal_ready
                && preview.bounded_section_count > 0
            {
                preview.phase = EmbeddedWorldPreviewPhase::Visible;
                preview.failure = None;
            }
        }

        self.standby_world = Some(slot);
        self.warm_world_standby = Some(state);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn advance_local_startup(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        self.poll_lobby_launch(device, queue)?;
        let active_completed = self.advance_active_local_startup(device, queue)?;
        self.advance_lobby_after_primary(active_completed, device, queue);
        self.advance_warm_world_standby();
        self.update_lobby_destination_status();
        Ok(active_completed)
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn advance_local_startup(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<bool> {
        self.advance_warm_world_standby();
        self.update_lobby_destination_status();
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
            camera,
            ..
        } = startup;
        let descriptor = descriptor
            .or_else(|| request.active_descriptor())
            .context("XR local startup request did not describe an active session")?;
        // Camera/interest reconciliation has already advanced frame by frame in
        // `advance_active_local_startup`; completion only consumes the ready pump
        // and its seed, so no interactive presentation callback spins here.
        let (local_runtime, startup_sections) = pump.into_runtime_with_startup_sections();
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
        let prepared_shared_resources = self
            .prepared_warm_world_shell
            .as_ref()
            .map(|shell| shell.draw.shared_resources());
        let mut draw = match prepared_shared_resources {
            Some(shared) => TexturedSectionDrawResources::new_with_shared_resources(
                device, queue, &sections, shared,
            ),
            None => TexturedSectionDrawResources::new(
                device,
                queue,
                self.color_format,
                &sections,
                runtime.mesh_assets().atlas.as_upload(),
            ),
        }
        .context("upload XR local startup render sections")?;
        draw.set_traversal_ready_sections(
            &runtime.traversal_ready_render_section_keys(camera_position),
        );

        let section_count = draw.section_count();
        let index_count = draw.index_count();
        let face_count = quad_face_count_from_indices(index_count);
        let mesh_assets = runtime.mesh_assets().clone();
        let accepted_entry_pose = Some(WorldEntryPose::from_camera(&camera));
        let actors = self.active_world.actors.take();
        self.active_world.install(DrawableWorldSlotInstall {
            id: self.active_world.id,
            descriptor: Some(descriptor.clone()),
            lifecycle: WorldSlotLifecycle::ActiveReady,
            asset_epoch: self.active_assets.epoch,
            scene,
            runtime: Some(runtime),
            local_startup: None,
            external_runtime_startup_pending: false,
            camera,
            draw,
            actors,
            render_stats: RenderStreamStats {
                section_count,
                index_count,
                face_count,
                ..RenderStreamStats::default()
            },
            accepted_entry_pose,
            pending_startup_sections: Vec::new(),
        });
        self.mesh_assets = mesh_assets;
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
        let Some(screen) = self.active_world.scene.debug_ui_screen else {
            return;
        };
        let desired_screen = match screen {
            XrDebugUiScreen::Pause => GameScreen::Pause,
            XrDebugUiScreen::Controls => GameScreen::Help {
                parent: mclone_ui::GameHelpParent::OptionsPause,
            },
            XrDebugUiScreen::Graphics => GameScreen::OptionsCategory {
                parent: mclone_ui::GameOptionsParent::Pause,
                category: mclone_ui::GameOptionsCategory::Graphics,
            },
            XrDebugUiScreen::SeasonalDebug => GameScreen::OptionsCategory {
                parent: mclone_ui::GameOptionsParent::Pause,
                category: mclone_ui::GameOptionsCategory::SeasonalDebug,
            },
            XrDebugUiScreen::CelestialDebug => GameScreen::OptionsCategory {
                parent: mclone_ui::GameOptionsParent::Pause,
                category: mclone_ui::GameOptionsCategory::CelestialDebug,
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
        self.session.fail_start(SessionFailure::for_request(
            &startup.request,
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

    fn clear_physical_presentation_state(&mut self) {
        self.tracking_origin = None;
        self.last_locomotion_update = None;
        self.active_world.local_participant.reset_movement();
        self.active_world.footsteps.reset();
        self.active_world.pending_interaction_sounds.clear();
        self.client_clock.reset();
        self.player_pose_sync.reset();
        self.head_comfort.reset();
        self.clear_xr_blink_teleport();
        self.clear_mono_blink_debug();
        self.latest_xr_input = XrInputFrame::default();
        self.latest_xr_head_gaze_stage = None;
        self.first_eye_summary = None;
        self.last_ui_panel_stats = WorldGuiPanelRenderStats::default();
        self.last_ui_draw_cache_stats = UiDrawCacheStats::default();
        self.rendered_frames = 0;
    }

    fn clear_active_world_transient_state(&mut self) {
        self.underwater_effects = XrUnderwaterEffectStates::default();
        self.last_underwater_update = None;
        self.prefetched_live_upload = None;
        self.active_world.clear_stream_state();
    }

    /// Clear only host presentation caches whose meaning changes when another
    /// complete world slot becomes active. Slot-owned traversal, uploads, draw
    /// resources, runtime state, and camera state intentionally survive.
    fn clear_world_selection_presentation_state(&mut self) {
        self.underwater_effects = XrUnderwaterEffectStates::default();
        self.last_underwater_update = None;
        self.tracking_origin = None;
        self.prefetched_live_upload = None;
        self.last_locomotion_update = None;
        self.active_world.local_participant.reset_movement();
        self.client_clock.reset();
        self.player_pose_sync.reset();
        self.first_eye_summary = None;
        self.last_ui_panel_stats = WorldGuiPanelRenderStats::default();
        self.last_ui_draw_cache_stats = UiDrawCacheStats::default();
        self.rendered_frames = 0;
    }

    pub(crate) fn clear_transient_world_state(&mut self) {
        self.clear_physical_presentation_state();
        self.clear_active_world_transient_state();
    }

    pub(crate) fn teardown_world(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        self.reset_terrain_view();
        let cancelled_session_starts = self.active_session_start_operations.teardown().len();
        let cancelled_asset_preparations = self.external_asset_pack_operations.teardown().len();
        self.pending_external_asset_pack_selection = None;
        if cancelled_session_starts != 0 || cancelled_asset_preparations != 0 {
            log::info!(
                "cancelled platform operations during world teardown session_starts={} asset_preparations={}",
                cancelled_session_starts,
                cancelled_asset_preparations,
            );
        }
        self.cancel_warm_world_standby("active world teardown");
        if let Some(operations) = self.services.catalog_operations.as_mut() {
            for request in operations.begin_epoch() {
                let _ = self.client_experience.catalog_mut().apply_catalog_error(
                    request.id,
                    WorldCatalogError::unsupported("World catalog operation superseded"),
                );
            }
        }
        self.active_world.local_startup = None;
        self.active_world.external_runtime_startup_pending = false;
        self.active_world.lifecycle = WorldSlotLifecycle::Empty;
        self.active_world.descriptor = None;
        self.active_world.accepted_entry_pose = None;
        self.active_world.pending_startup_sections.clear();
        if self.active_world.runtime.take().is_some() {
            self.active_world.draw = TexturedSectionDrawResources::new(
                device,
                queue,
                self.color_format,
                &[],
                self.mesh_assets.atlas.as_upload(),
            )
            .context("reset XR terrain draw resources during session teardown")?;
        }
        self.active_world.render_stats = RenderStreamStats::default();
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
        let started = match factory.start(
            endpoint.clone(),
            scene.clone(),
            mesh_assets,
            device,
            queue,
            self.color_format,
            &self.services.clock,
        ) {
            Ok(started) => started,
            Err(error) => {
                log::error!("failed to warm XR session {request:?}: {error:#}");
                return Err(error);
            }
        };

        let mesh_assets = started.runtime.mesh_assets().clone();
        let accepted_entry_pose = Some(WorldEntryPose::from_camera(&started.camera));
        let actors = ActorDrawResources::new_with_shared_resources(
            device,
            self.active_world
                .actors
                .as_ref()
                .expect("active world owns actor draw state")
                .shared_resources(),
        );
        self.active_world.install(DrawableWorldSlotInstall {
            id: self.active_world.id,
            descriptor: Some(descriptor.clone()),
            lifecycle: WorldSlotLifecycle::ActiveReady,
            asset_epoch: self.active_assets.epoch,
            scene,
            runtime: Some(started.runtime),
            local_startup: None,
            external_runtime_startup_pending: false,
            camera: started.camera,
            draw: started.draw,
            actors: Some(actors),
            render_stats: started.render_stats,
            accepted_entry_pose,
            pending_startup_sections: Vec::new(),
        });
        self.mesh_assets = mesh_assets;
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

    pub fn apply_xr_ui_action(
        &mut self,
        action: GameUiAction,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        if matches!(action, GameUiAction::BackToTitle)
            && self.ui.screen() == Some(GameScreen::PreparingLobby)
        {
            self.cancel_warm_world_standby("lobby title cancellation");
        }
        if self.active_world.local_startup.is_some() {
            self.play_ui_error_sound();
            return Ok(false);
        }
        if matches!(self.ui.screen(), Some(GameScreen::Death { .. }))
            && !matches!(
                action,
                GameUiAction::Respawn | GameUiAction::QuitToTitle | GameUiAction::Quit
            )
        {
            self.play_ui_error_sound();
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
        self.play_ui_action_sound(action);
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
        let mut scenario_scene_replaced = false;
        for effect in effects.scenario {
            scenario_scene_replaced |= self.apply_lobby_effect(effect, device, queue)?;
        }
        self.apply_asset_pack_effects(effects.asset_packs);
        for effect in effects.local_data {
            self.apply_local_data_effect(effect);
        }
        if !self.apply_xr_settings_effects(effects.settings)? {
            return Ok(catalog_scene_replaced || session_scene_replaced || scenario_scene_replaced);
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
                ClientExperienceGameplayEffect::AssignHotbarActor { slot, actor } => {
                    let changed = self.assign_debug_hotbar_actor(slot, actor)?;
                    log::info!(
                        "XR debug hotbar slot {} assigned actor={actor:?} changed={changed}",
                        slot + 1,
                    );
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
                    self.apply_xr_projection_action(action);
                }
                ClientExperienceProjectionEffect::SuppressUiAction => {}
            }
        }
        Ok(catalog_scene_replaced || session_scene_replaced || scenario_scene_replaced)
    }

    pub(crate) fn apply_xr_catalog_effects(
        &mut self,
        effects: ClientCatalogEffects,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        let mut scene_replaced = false;
        for start in effects.session_starts {
            let scene = match self.catalog_session_scene_options(&start.summary) {
                Ok(scene) => scene,
                Err(error) => {
                    log::warn!("catalog session start failed: {error}");
                    continue;
                }
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

    fn catalog_session_scene_options(
        &self,
        summary: &LocalWorldSummary,
    ) -> Result<McloneSceneHostOptions, WorldCatalogError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(world_root) = self.active_world.scene.world_root.clone() else {
                return Err(WorldCatalogError::unsupported(
                    "Persistent worlds unavailable",
                ));
            };
            Ok(self.catalog_world_scene(summary, world_root.join(summary.id.as_str())))
        }
        #[cfg(target_arch = "wasm32")]
        {
            let mut scene = self.active_world.scene.clone();
            project_ordinary_local_world_identity(
                &mut scene,
                summary.seed,
                summary.world_generation_profile,
                summary.starter_content,
            );
            scene.startup.local_entry_intent = mclone_app_runtime::local_session_launch::LocalSessionEntryIntent::PersistedPlayerOrProfilePreferred;
            scene.debug_passive_showcase = false;
            scene.world_dir = None;
            scene.project_terrain_presentation_for_source(true);
            Ok(scene)
        }
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
        if let Some(mut launch) = self.lobby_launch.take() {
            self.try_resolve_lobby_destination(&mut launch)?;
            self.try_issue_lobby_destination_start(&mut launch, device, queue)?;
            self.lobby_launch = Some(launch);
        }
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
        if let Some(mut launch) = self.lobby_launch.take() {
            self.try_resolve_lobby_destination(&mut launch)?;
            self.try_issue_lobby_destination_start(&mut launch, device, queue)?;
            self.lobby_launch = Some(launch);
        }
        let scene_replaced = self.apply_xr_catalog_effects(effects, device, queue)?;
        // Deferred completions arrive between frames. Preserve the browser's
        // immediate input/render projection refresh and give any future native
        // deferred driver the same behavior.
        let render_state = self.current_mono_ui_render_state();
        self.ui.commit_render_state(render_state);
        Ok(scene_replaced)
    }

    pub fn pending_external_catalog_operation_count(&self) -> usize {
        self.services
            .catalog_operations
            .as_ref()
            .map_or(0, WorldCatalogOperationService::pending_len)
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

    fn set_world_render_scale_mode(&mut self, _mode: GameWorldRenderScaleMode) -> Result<()> {
        log::info!("flat world render scale ignored by XR scene host");
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

    fn set_world_render_scale_mode(&mut self, _mode: GameWorldRenderScaleMode) -> Result<()> {
        log::info!("flat world render scale ignored by XR scene host");
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

    fn set_leaf_detail(&mut self, detail: GameLeafDetail) -> Result<()> {
        self.request_leaf_detail(engine_leaf_detail(detail))
    }

    fn set_grass_detail(&mut self, detail: GameGrassDetail) -> Result<()> {
        self.request_grass_detail(engine_grass_detail(detail));
        Ok(())
    }

    fn set_terrain_lod_preset(&mut self, presentation: TerrainLodPreset) -> Result<()> {
        self.request_terrain_lod_preset(presentation)
    }

    fn set_fog_settings(&mut self, settings: GameFogSettings) -> Result<()> {
        self.fog_settings = settings.normalized();
        self.persist_graphics_preferences();
        log::info!("fog settings set to {:?}", self.fog_settings);
        Ok(())
    }

    fn set_season_preview(&mut self, settings: SeasonPreviewSettings) -> Result<()> {
        self.season_preview = settings;
        Ok(())
    }

    fn set_celestial_debug(&mut self, settings: CelestialDebugSettings) -> Result<()> {
        self.celestial_debug = settings;
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

    fn set_player_collision_box_visible(&mut self, visible: bool) -> Result<()> {
        self.player_collision_box_visible = visible;
        log::info!(
            "XR player collision box debug {}",
            if visible { "visible" } else { "hidden" }
        );
        Ok(())
    }

    fn set_first_person_player_visible(&mut self, visible: bool) -> Result<()> {
        self.active_world
            .camera
            .set_first_person_player_visible(visible);
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
        self.active_world.player_model = model;
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
        self.active_world
            .camera
            .set_movement_mode(engine_movement_mode(mode));
        self.active_world.local_participant.reset_movement();
        log::info!(
            "XR player movement mode {}",
            self.active_world.camera.movement_mode().label()
        );
        Ok(())
    }

    fn set_collision_mode(&mut self, mode: GameCollisionMode) -> Result<()> {
        self.active_world
            .camera
            .set_collision_mode(engine_collision_mode(mode));
        self.active_world.local_participant.reset_movement();
        log::info!(
            "XR player collision mode {}",
            self.active_world.camera.collision_mode().label()
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

    fn request_xr_render_mode(&mut self, mode: GameXrRenderMode) -> Result<()> {
        let Some(mut state) = self.xr_render_path_state else {
            self.status_overlay =
                StatusOverlay::new("XR render path is unavailable for this host", false);
            return Ok(());
        };
        if !state.supported_modes.contains(mode) {
            self.status_overlay =
                StatusOverlay::new("The selected XR render path is unsupported", false);
            return Ok(());
        }
        state.requested_mode = mode;
        state.pending_mode = (mode != state.active_mode).then_some(mode);
        state.transition_state = if state.pending_mode.is_some() {
            GameXrRenderTransitionState::Pending
        } else {
            GameXrRenderTransitionState::Idle
        };
        self.xr_render_path_state = Some(state);
        self.pending_xr_render_mode_request = Some(mode);
        log::info!("XR render-path request queued: {}", mode.label());
        Ok(())
    }

    fn set_render_distance(&mut self, render_distance: u32) -> Result<()> {
        if let Some(runtime) = &mut self.active_world.runtime
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
        self.active_world.scene.render_distance = render_distance;
        Ok(())
    }

    fn set_fly_speed_multiplier(&mut self, multiplier: f32) -> Result<()> {
        self.active_world
            .camera
            .set_fly_speed_multiplier(f64::from(multiplier));
        log::info!(
            "XR fly speed set to {:.1}x ({:.0} blocks/s)",
            self.active_world.camera.fly_speed_multiplier(),
            self.active_world.camera.speed_blocks_per_second()
        );
        Ok(())
    }

    fn set_movement_speed_multiplier(&mut self, multiplier: f32) -> Result<()> {
        self.active_world
            .camera
            .set_movement_speed_multiplier(f64::from(multiplier));
        self.active_world.scene.movement_speed_multiplier =
            self.active_world.camera.movement_speed_multiplier() as f32;
        log::info!(
            "XR movement speed multiplier set to {:.1}x",
            self.active_world.camera.movement_speed_multiplier()
        );
        Ok(())
    }

    fn set_touch_look_sensitivity(&mut self, sensitivity: f32) -> Result<()> {
        if let Some(settings) = self
            .mono_ui_context
            .as_mut()
            .and_then(|context| context.touch_settings.as_mut())
        {
            settings.look_sensitivity = GameTouchSettings::new(
                sensitivity,
                settings.min_look_sensitivity,
                settings.max_look_sensitivity,
            )
            .clamped_look_sensitivity();
        }
        Ok(())
    }

    fn set_server_simulation_cadence(&mut self, cadence: GameSimulationCadence) -> Result<()> {
        let cadence = SimulationCadenceConfig::new(
            cadence.host_rate_hz,
            cadence.gameplay_rate_hz,
            cadence.physics_rate_hz,
        );
        if let Some(runtime) = &mut self.active_world.runtime {
            runtime.set_simulation_cadence(cadence)?;
        }
        self.active_world.scene.simulation_cadence = cadence;
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
    camera_config: SceneCameraConfig,
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
    let mut camera = camera_config.spawn_for_chunk(center);
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

#[cfg(not(target_arch = "wasm32"))]
fn reconcile_local_startup_pose_pass(
    startup: &mut SceneLocalStartup,
    clock: &MonotonicClockHandle,
) -> Result<bool> {
    let interest_before = startup.pump.interest_center();
    reconcile_xr_startup_pose(
        startup.pump.runtime_services_mut(),
        &mut startup.camera,
        startup.startup_view_pose,
        clock,
    )?;
    let interest_after = startup.pump.interest_center();
    startup.reconciliation_passes += 1;
    if interest_after == interest_before {
        return Ok(false);
    }

    startup.reconciled_interest_center = Some(interest_after);
    startup
        .reconciliation_deadline
        .get_or_insert_with(|| clock.deadline_after(DEFAULT_STARTUP_READINESS_TIMEOUT));
    Ok(true)
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
fn bind_native_entry_world_dir(
    mut scene: McloneSceneHostOptions,
    world_dir: Option<&std::path::Path>,
) -> McloneSceneHostOptions {
    if let Some(world_dir) = world_dir {
        scene.world_dir = Some(world_dir.to_owned());
    }
    scene
}

fn project_ordinary_local_world_identity(
    scene: &mut McloneSceneHostOptions,
    seed: i64,
    world_generation_profile: mclone_server::WorldGenerationProfile,
    starter_content: mclone_server::StarterContentDescriptor,
) {
    scene.seed = seed;
    scene.world_generation_profile = world_generation_profile;
    scene.starter_content = starter_content;
    scene.world_behavior_profile = mclone_server::WorldBehaviorProfile::Mutable;
    scene.startup.local_entry_intent =
        if world_generation_profile.authored_missing_chunk().is_none() {
            mclone_app_runtime::local_session_launch::LocalSessionEntryIntent::ProfilePreferred
        } else {
            mclone_app_runtime::local_session_launch::LocalSessionEntryIntent::AuthoredCoordinate
        };
    scene.remote_addr = None;
}

#[cfg(not(target_arch = "wasm32"))]
fn transient_local_session_start_request(scene: &McloneSceneHostOptions) -> SessionStartRequest {
    SessionStartRequest::new_seed_local_world_with_generation_profile_and_starter_content(
        scene.seed,
        scene.world_generation_profile,
        scene.starter_content,
    )
}

pub fn local_session_launch_plan(
    scene: &McloneSceneHostOptions,
    observer_only: bool,
) -> mclone_app_runtime::local_session_launch::LocalSessionLaunchPlan {
    let mut authority = mclone_server::LocalAuthorityStartConfig::new(scene.seed);
    authority.world_generation_profile = scene.world_generation_profile;
    authority.starter_content = scene.starter_content;
    authority.world_topology = scene.world_topology;
    authority.world_behavior_profile = scene.world_behavior_profile;
    authority.lighting_enabled = scene.lighting_enabled;
    authority.light_status_batch_size = scene.light_status_batch_size;
    authority.debug_light_admission_delay_ticks = scene.debug_light_admission_delay_ticks;
    authority.day_time = scene.day_time_override;
    authority.day_time_frozen = scene.freeze_time;
    authority.scheduled_fluid_ticks_frozen = scene.freeze_scheduled_fluid_ticks;
    authority.debug_passive_showcase = scene.debug_passive_showcase;
    authority.debug_auxiliary_player_script = scene.debug_auxiliary_player_script;
    authority.cadence = scene.simulation_cadence;
    authority.adaptive_chunk_publication_budget = scene.adaptive_chunk_publication_budget;
    authority.observer_only = observer_only;
    #[cfg(not(target_arch = "wasm32"))]
    match mclone_app_runtime::local_profile::load_or_create_native_local_player_profile(
        scene.world_root.as_deref(),
    ) {
        Ok(profile) => authority.local_player_identity = Some(profile.client_identity()),
        Err(error) => {
            log::warn!("local player profile is unavailable for this scene: {error:#}");
        }
    }
    #[cfg(target_arch = "wasm32")]
    match mclone_app_runtime::local_profile::load_or_create_web_local_player_profile() {
        Ok(profile) => authority.local_player_identity = Some(profile.client_identity()),
        Err(error) => {
            log::warn!("browser local player profile is unavailable for this scene: {error:#}");
        }
    }
    mclone_app_runtime::local_session_launch::resolve_local_session_launch_plan(
        authority,
        scene.center(),
        scene.render_distance,
        scene.startup.local_entry_intent,
    )
    .expect("validated scene options must resolve one local-session launch plan")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn local_integrated_scene_options(
    scene: &McloneSceneHostOptions,
) -> LocalIntegratedSceneOptions {
    let plan = local_session_launch_plan(scene, false);
    let storage = IntegratedWorldSessionStorage::from_world_dir(scene.world_dir.as_deref())
        .with_adaptive_chunk_publication_budget(scene.adaptive_chunk_publication_budget);
    LocalIntegratedSceneOptions::new(
        plan.authority.seed,
        plan.initial_view.center,
        plan.initial_view.render_distance,
    )
    .with_authority(plan.authority)
    .with_render_compile_worker_count(scene.render_compile_worker_count)
    .with_render_compile_max_pending_jobs(scene.render_compile_max_pending_jobs)
    .with_render_compile_worker_timing_enabled(scene.render_compile_worker_timing_enabled)
    .with_integrated_world_session_storage(storage)
}

#[cfg(not(target_arch = "wasm32"))]
fn native_app_private_world_dir(
    world_root: &std::path::Path,
    key: mclone_app_runtime::scenario_content::AppPrivateWorldKey,
) -> PathBuf {
    let app_root = world_root.parent().unwrap_or(world_root);
    match key {
        mclone_app_runtime::scenario_content::AppPrivateWorldKey::LobbyFallback => app_root
            .join("scenarios")
            .join(mclone_app_runtime::scenario_content::LOBBY_FALLBACK_LEGACY_PARENT_DIRECTORY)
            .join(mclone_app_runtime::scenario_content::LOBBY_FALLBACK_DIRECTORY),
    }
}

fn reconcile_observer_preview_entry(
    scene: &McloneSceneHostOptions,
    client: &mclone_client::ClientRuntime,
    camera: &mut EngineCameraController,
) -> bool {
    let snapshot = camera.snapshot();
    let Some(feet_position) = mclone_server::find_safe_surface_spawn_for_loaded_descriptor(
        scene.seed,
        scene.world_generation_profile,
        scene.world_topology,
        snapshot.chunk_pos,
        |pos| {
            client
                .block_state_at_block_pos(pos)
                .and_then(|state| u16::try_from(state.0).ok())
        },
        |chunk| client.chunk_snapshot(chunk).is_some(),
    ) else {
        return false;
    };
    camera.set_player_feet_pose(feet_position, snapshot.yaw_radians, snapshot.pitch_radians);
    true
}

fn preview_crop_source_anchor(
    region: mclone_render::placement::EmbeddedChunkRegion,
    entry_y: f64,
) -> Vec3d {
    let bounds = region.source_bounds();
    Vec3d::new(
        (bounds.min().x + bounds.max().x) * 0.5,
        entry_y,
        (bounds.min().z + bounds.max().z) * 0.5,
    )
}

fn entry_relative_composition_anchor(pose: WorldEntryPose, offset: Vec3d) -> Vec3d {
    let forward = Vec3d::new(pose.yaw_radians.sin(), 0.0, pose.yaw_radians.cos());
    let right = Vec3d::new(pose.yaw_radians.cos(), 0.0, -pose.yaw_radians.sin());
    pose.feet_position
        .add(right.scale(offset.x))
        .add(Vec3d::new(0.0, offset.y, 0.0))
        .add(forward.scale(offset.z))
}

const fn vec3d_from_array([x, y, z]: [f64; 3]) -> Vec3d {
    Vec3d::new(x, y, z)
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

#[cfg(test)]
mod camera_config_tests {
    use super::*;
    use mclone_server::WorldGenerationProfile;

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn direct_native_entry_binds_the_requested_world_directory() {
        let scene = bind_native_entry_world_dir(
            McloneSceneHostOptions::default(),
            Some(std::path::Path::new("/worlds/direct-save")),
        );

        assert_eq!(scene.world_dir, Some(PathBuf::from("/worlds/direct-save")));
    }

    #[test]
    fn private_lobby_fallback_reuses_the_legacy_storage_location() {
        assert_eq!(
            native_app_private_world_dir(
                std::path::Path::new("/app/worlds"),
                mclone_app_runtime::scenario_content::AppPrivateWorldKey::LobbyFallback,
            ),
            PathBuf::from("/app/scenarios/lobby-preview-v3/fallback-overworld")
        );
    }

    #[test]
    fn configured_camera_applies_launch_defaults_once() {
        let mut scene = McloneSceneHostOptions::default();
        scene.movement_speed_multiplier = 2.25;
        scene.first_person_player_visible = true;

        let camera = SceneCameraConfig::from_scene(&scene).spawn_for_chunk(scene.center());

        assert_eq!(camera.movement_speed_multiplier(), 2.25);
        assert!(camera.first_person_player_visible());
    }

    #[test]
    fn replacement_camera_preserves_runtime_speed_and_applies_scene_visibility() {
        let mut scene = McloneSceneHostOptions::default();
        scene.movement_speed_multiplier = 1.25;
        scene.first_person_player_visible = true;

        let camera = SceneCameraConfig::from_scene(&scene)
            .spawn_for_chunk_with_speed(ChunkPos::new(4, -3), 3.5);

        assert_eq!(camera.movement_speed_multiplier(), 3.5);
        assert!(camera.first_person_player_visible());
    }

    #[test]
    fn configured_camera_applies_movement_through_shared_reducer() {
        for (mode, engine_mode) in [
            (GameMovementMode::Walk, EngineCameraMovementMode::Walking),
            (GameMovementMode::Fly, EngineCameraMovementMode::Fly),
            (
                GameMovementMode::HandPush,
                EngineCameraMovementMode::HandPush,
            ),
            (
                GameMovementMode::Thruster,
                EngineCameraMovementMode::Thruster,
            ),
        ] {
            let mut scene = McloneSceneHostOptions::default();
            scene.movement_mode = mode;

            let camera = SceneCameraConfig::from_scene(&scene).spawn_for_chunk(scene.center());

            assert_eq!(camera.movement_mode(), engine_mode);
            assert_eq!(camera.collision_mode(), EngineCameraCollisionMode::Normal);
        }
    }

    #[test]
    fn authored_local_startup_keeps_the_configured_entry_center() {
        let mut scene = McloneSceneHostOptions::default();
        scene.seed = 17_501;
        scene.chunk_x = 3;
        scene.chunk_z = -2;
        scene.startup.local_entry_intent =
            mclone_app_runtime::local_session_launch::LocalSessionEntryIntent::AuthoredCoordinate;
        scene.world_generation_profile = WorldGenerationProfile::authored_only();

        let options = local_integrated_scene_options(&scene);

        assert_eq!(options.center, ChunkPos::new(3, -2));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn transient_local_startup_request_keeps_the_selected_generation_profile() {
        let mut scene = McloneSceneHostOptions::default();
        scene.seed = 17_501;
        scene.world_generation_profile = WorldGenerationProfile::TopologyProbeV1;

        assert_eq!(
            transient_local_session_start_request(&scene),
            SessionStartRequest::new_seed_local_world_with_generation_profile(
                scene.seed,
                WorldGenerationProfile::TopologyProbeV1,
            )
        );
    }

    #[test]
    fn local_startup_carries_world_behavior_without_global_state() {
        let mut lobby = McloneSceneHostOptions::default();
        lobby.world_behavior_profile = mclone_server::WorldBehaviorProfile::ProtectedLobby;
        let island = McloneSceneHostOptions::default();

        assert_eq!(
            local_integrated_scene_options(&lobby)
                .authority
                .world_behavior_profile,
            mclone_server::WorldBehaviorProfile::ProtectedLobby
        );
        assert_eq!(
            local_integrated_scene_options(&island)
                .authority
                .world_behavior_profile,
            mclone_server::WorldBehaviorProfile::Mutable
        );
    }

    #[test]
    fn ordinary_local_world_projection_drops_lobby_behavior_and_keeps_starter_identity() {
        let mut scene = McloneSceneHostOptions::default();
        scene.world_behavior_profile = mclone_server::WorldBehaviorProfile::ProtectedLobby;
        project_ordinary_local_world_identity(
            &mut scene,
            0,
            WorldGenerationProfile::Overworld,
            mclone_server::StarterContentDescriptor::IntroHomesteadV1,
        );

        assert_eq!(
            scene.world_behavior_profile,
            mclone_server::WorldBehaviorProfile::Mutable
        );
        assert_eq!(
            scene.starter_content,
            mclone_server::StarterContentDescriptor::IntroHomesteadV1
        );
    }

    #[test]
    fn default_mclone_wild_menu_projection_resolves_the_reproduced_inland_center() {
        let mut scene = McloneSceneHostOptions::default();
        scene.chunk_x = 0;
        scene.chunk_z = 0;
        project_ordinary_local_world_identity(
            &mut scene,
            553_534_047_293_117_028,
            WorldGenerationProfile::McloneOverworldV1,
            mclone_server::StarterContentDescriptor::Wild,
        );

        let plan = local_session_launch_plan(&scene, false);
        assert_eq!(
            plan.receipt().entry.intent,
            mclone_app_runtime::local_session_launch::LocalSessionEntryIntent::ProfilePreferred
        );
        assert_eq!(plan.receipt().entry.requested_center, ChunkPos::new(0, 0));
        assert_eq!(plan.initial_view.center, ChunkPos::new(-48, 20));
        assert_eq!(plan.receipt().authority, plan.authority);
    }

    #[test]
    fn catalog_lobby_destination_keeps_homestead_identity_and_mutable_behavior() {
        let content = mclone_app_runtime::scenario_content::LobbyScenarioContent::for_intent(
            mclone_app_runtime::scenario::ScenarioLaunchIntent::lobby_preview(),
        );
        let mut summary =
            LocalWorldSummary::new(LocalWorldId::new("new-world").unwrap(), "Homestead", 0, 1)
                .unwrap();
        summary.starter_content = mclone_server::StarterContentDescriptor::IntroHomesteadV1;

        let destination = McloneSceneHost::prepare_catalog_lobby_destination(
            &content,
            &summary,
            mclone_app_runtime::scenario::ScenarioPreviewBounds::square(2).unwrap(),
        )
        .unwrap();

        assert_eq!(
            destination.destination.starter_content,
            mclone_server::StarterContentDescriptor::IntroHomesteadV1
        );
        assert_eq!(
            destination.destination.world_behavior_profile,
            mclone_server::WorldBehaviorProfile::Mutable
        );
    }

    #[test]
    fn preview_crop_anchor_centers_even_two_and_four_chunk_bounds() {
        for (span, expected_xz) in [(2, (112.0, -48.0)), (4, (112.0, -48.0))] {
            let region = mclone_app_runtime::scenario::ScenarioPreviewBounds::square(span)
                .unwrap()
                .resolve(ChunkPos::new(7, -3), 4)
                .unwrap();
            let anchor = preview_crop_source_anchor(region, 65.0);

            assert_eq!(anchor, Vec3d::new(expected_xz.0, 65.0, expected_xz.1));
        }
    }

    #[test]
    fn return_preview_offset_rotates_with_accepted_entry_yaw() {
        let pose = WorldEntryPose {
            feet_position: Vec3d::new(10.0, 64.0, 20.0),
            yaw_radians: std::f64::consts::FRAC_PI_2,
        };

        assert_eq!(
            entry_relative_composition_anchor(pose, Vec3d::new(2.0, 3.0, 6.0)),
            Vec3d::new(16.0, 67.0, 18.0),
        );
    }
}
