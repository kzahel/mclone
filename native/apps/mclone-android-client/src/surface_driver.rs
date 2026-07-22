use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use glam::Vec2;
use mclone_android_platform::{
    AndroidControllerCollector, AndroidControllerPoll, drain_android_controller_events,
};
use mclone_app_runtime::client_experience::android_flat_native_client_experience_profile;
use mclone_app_runtime::frame_pacing::{
    FramePacingDebugStats, FramePacingMode, FramePacingUiState, FrameTimingStats,
};
use mclone_app_runtime::frame_pipeline_accounting::FramePipelineAccountant;
use mclone_app_runtime::frame_render::FlatSurfacePresentation;
use mclone_app_runtime::host_mode::SingleViewHostOptions;
use mclone_app_runtime::input_preferences::ClientInputPreferences;
use mclone_app_runtime::native_remote_session::{
    NativeRemoteServerSession, connect_native_remote_session_runtime_with_identity,
};
use mclone_app_runtime::native_service_assembly::NativeSessionServices;
use mclone_app_runtime::render_assets::{
    TexturedMeshAssets, load_actor_texture_assets_from_asset_source, load_asset_source,
    load_textured_mesh_assets_from_source,
};
use mclone_app_runtime::session::{
    ActiveSessionDescriptor, GameSessionState, RemoteSessionEndpoint,
};
use mclone_audio::{AudioEngine, AudioSettings};
use mclone_core::{CHUNK_WIDTH, Vec3d};
use mclone_diagnostics::{FrameHostKind, FramePipelineReport};
use mclone_input::{
    FlatInputFrame, InputCapabilities, InputCapabilityState, InputDeviceKind, InputPreferences,
    KeyboardKey, MouseWheelDirection, PointerButton, TouchControlsMode, TouchInputAdapter,
    TouchInputEvent, TouchInputSettings,
};
use mclone_render::chunk::{ChunkDepthTarget, TexturedSectionRenderOptions};
use mclone_render::color_profile::{RenderColorProfile, RenderConfig};
use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
use mclone_scene::{
    HostEffects, McloneSceneHost, McloneSceneHostOptions, MonoInputDisposition,
    MonoInteractiveInputRouter, MonoSceneFrameSummary, MonoUiContext, MonoUiPresentation,
    record_mono_frame_pipeline, xr_frame_pipeline_accounting_config,
};
use mclone_ui::{
    DEFAULT_JOIN_REMOTE_ADDR, EMPTY_HOTBAR_ICONS, GameAuxiliarySplitMode, GameTouchSettings,
    GameUiHost, GuiScale, Point, TouchJoystickOverlay, TouchOverlay, touch_control_at,
    touch_menu_button_rect,
};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, Touch, TouchPhase,
    WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, DeviceEvents};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::AndroidSceneHost;
use crate::startup::{
    AndroidPacingPerfOptions, AndroidStartupOptions, apply_startup_camera_options,
};

const ANDROID_FIXED_FPS_CAP: u32 = 60;
const ANDROID_TARGET_FRAME_MS: f64 = 1_000.0 / ANDROID_FIXED_FPS_CAP as f64;
const TOUCH_MOVEMENT_MAX_FRAME_SECONDS: f64 = 0.05;

pub(crate) enum AndroidRenderError {
    Surface(wgpu::SurfaceError),
    Render(anyhow::Error),
}

#[derive(Clone, Copy, Debug, Default)]
struct AndroidInputOutcome {
    handled: bool,
    exit: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct AndroidHostEffectOutcome {
    touch_controls_mode: Option<TouchControlsMode>,
    quit_to_title: bool,
    exit: bool,
}

#[derive(Default)]
struct AndroidHostEffects {
    outcome: AndroidHostEffectOutcome,
}

impl HostEffects for AndroidHostEffects {
    fn request_mouse_lock(&mut self, _requested: bool) -> Result<()> {
        Ok(())
    }

    fn cycle_frame_pacing(&mut self) -> Result<()> {
        Ok(())
    }

    fn cycle_fps_cap(&mut self) -> Result<()> {
        Ok(())
    }

    fn set_touch_controls_mode(&mut self, mode: TouchControlsMode) -> Result<()> {
        self.outcome.touch_controls_mode = Some(mode);
        Ok(())
    }

    fn quit_to_title(&mut self) -> Result<()> {
        self.outcome.quit_to_title = true;
        Ok(())
    }

    fn exit(&mut self) -> Result<()> {
        self.outcome.exit = true;
        Ok(())
    }
}

pub(crate) struct AndroidSurfaceDriver {
    startup_options: AndroidStartupOptions,
    window: Option<Arc<Window>>,
    gpu: Option<AndroidGpuState>,
    last_cursor: Option<PhysicalPosition<f64>>,
    controller_input: AndroidControllerCollector,
    controller_started_at: Instant,
}

impl AndroidSurfaceDriver {
    pub(crate) fn new(startup_options: AndroidStartupOptions) -> Self {
        Self {
            startup_options,
            window: None,
            gpu: None,
            last_cursor: None,
            controller_input: AndroidControllerCollector::new(),
            controller_started_at: Instant::now(),
        }
    }

    fn handle_input_result(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: &Window,
        context: &str,
        result: Result<AndroidInputOutcome>,
    ) {
        match result {
            Ok(outcome) if outcome.exit => event_loop.exit(),
            Ok(outcome) if outcome.handled => window.request_redraw(),
            Ok(_) => {}
            Err(error) => {
                log::error!("failed to handle Mclone Android {context}: {error:#}");
                event_loop.exit();
            }
        }
    }
}

impl ApplicationHandler for AndroidSurfaceDriver {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.controller_input
            .handle_events(drain_android_controller_events());
        if self.window.is_none() {
            match event_loop.create_window(WindowAttributes::default().with_title("Mclone")) {
                Ok(window) => {
                    log::info!("Mclone Android window created");
                    self.window = Some(Arc::new(window));
                }
                Err(error) => {
                    log::error!("failed to create Mclone Android window: {error}");
                    event_loop.exit();
                    return;
                }
            }
        }
        if let Some(window) = &self.window {
            if self.gpu.is_none() {
                match AndroidGpuState::new(window.clone(), self.startup_options.clone()) {
                    Ok(gpu) => {
                        self.gpu = Some(gpu);
                        self.controller_input.reannounce_connected_sources();
                    }
                    Err(error) => {
                        log::error!("MCLONE_ANDROID_FAILURE: {error:#}");
                        event_loop.exit();
                        return;
                    }
                }
            }
            event_loop.listen_device_events(DeviceEvents::WhenFocused);
            window.request_redraw();
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        log::info!("Mclone Android suspended");
        self.controller_input
            .handle_events(drain_android_controller_events());
        self.controller_input.clear_controls_for_lifecycle();
        if let Some(gpu) = &mut self.gpu
            && let Err(error) = gpu.on_background()
        {
            log::error!("Mclone Android background transition failed: {error:#}");
        }
        self.gpu = None;
        self.window = None;
        self.last_cursor = None;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };
        if window.id() != window_id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                self.controller_input
                    .handle_events(drain_android_controller_events());
                let controller_poll = self
                    .controller_input
                    .poll(self.controller_started_at.elapsed());
                let Some(gpu) = self.gpu.as_mut() else {
                    return;
                };
                match gpu.route_controller_poll(controller_poll) {
                    Ok(outcome) if outcome.exit => {
                        event_loop.exit();
                        return;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        log::error!("failed to route Mclone Android controller input: {error:#}");
                        event_loop.exit();
                        return;
                    }
                }
                match gpu.render_mclone_frame() {
                    Ok(()) => window.request_redraw(),
                    Err(AndroidRenderError::Surface(wgpu::SurfaceError::OutOfMemory)) => {
                        event_loop.exit();
                    }
                    Err(AndroidRenderError::Surface(wgpu::SurfaceError::Lost))
                    | Err(AndroidRenderError::Surface(wgpu::SurfaceError::Outdated)) => {
                        gpu.resize(window.inner_size());
                        window.request_redraw();
                    }
                    Err(AndroidRenderError::Surface(_)) => window.request_redraw(),
                    Err(AndroidRenderError::Render(error)) => {
                        log::error!("failed to render Mclone Android frame: {error:#}");
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size);
                }
                log::info!(
                    "Mclone Android window resized to {}x{}",
                    size.width,
                    size.height
                );
            }
            WindowEvent::Touch(touch) => {
                let result = self
                    .gpu
                    .as_mut()
                    .map_or(Ok(AndroidInputOutcome::default()), |gpu| {
                        gpu.handle_touch(touch)
                    });
                self.handle_input_result(event_loop, &window, "touch input", result);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(key_code) = event.physical_key else {
                    return;
                };
                let result = self
                    .gpu
                    .as_mut()
                    .map_or(Ok(AndroidInputOutcome::default()), |gpu| {
                        gpu.handle_keyboard_input(key_code, event.state, event.repeat)
                    });
                self.handle_input_result(event_loop, &window, "keyboard input", result);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_cursor = Some(position);
                let result = self
                    .gpu
                    .as_mut()
                    .map_or(Ok(AndroidInputOutcome::default()), |gpu| {
                        gpu.handle_cursor_move(position)
                    });
                self.handle_input_result(event_loop, &window, "cursor move", result);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let result = self
                    .gpu
                    .as_mut()
                    .map_or(Ok(AndroidInputOutcome::default()), |gpu| {
                        gpu.handle_mouse_input(self.last_cursor, state, button)
                    });
                self.handle_input_result(event_loop, &window, "mouse input", result);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let result = self
                    .gpu
                    .as_mut()
                    .map_or(Ok(AndroidInputOutcome::default()), |gpu| {
                        gpu.handle_mouse_wheel(delta)
                    });
                self.handle_input_result(event_loop, &window, "mouse wheel", result);
            }
            WindowEvent::Focused(false) => {
                self.last_cursor = None;
                self.controller_input.clear_controls_for_lifecycle();
                if let Some(gpu) = &mut self.gpu {
                    gpu.clear_flat_gameplay_input();
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        let DeviceEvent::MouseMotion { delta } = event else {
            return;
        };
        let result = self
            .gpu
            .as_mut()
            .map_or(Ok(AndroidInputOutcome::default()), |gpu| {
                gpu.handle_mouse_motion(delta)
            });
        if let Some(window) = self.window.as_ref().cloned() {
            self.handle_input_result(event_loop, &window, "mouse motion", result);
        }
    }
}

struct AndroidGpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth: ChunkDepthTarget,
    split_presentation: Option<FlatSurfacePresentation>,
    host: AndroidSceneHost,
    input_capabilities: InputCapabilityState,
    input_preferences: InputPreferences,
    client_input_preferences: ClientInputPreferences,
    input_preference_world_root: Option<std::path::PathBuf>,
    interactive_input: MonoInteractiveInputRouter,
    touch: TouchInputAdapter,
    ui_touch_id: Option<u64>,
    frame_timing: FrameTimingStats,
    frame_pipeline: FramePipelineAccountant,
    pacing_perf: Option<AndroidPacingPerfState>,
    last_frame: Instant,
    frame_index: u64,
    announced_session: Option<ActiveSessionDescriptor>,
}

impl AndroidGpuState {
    fn new(window: Arc<Window>, startup: AndroidStartupOptions) -> Result<Self> {
        let input_preference_world_root = startup.scene.world_root.clone();
        let client_input_preferences =
            match mclone_app_runtime::input_preferences::load_native_input_preferences(
                input_preference_world_root.as_deref(),
            ) {
                Ok(preferences) => preferences,
                Err(error) => {
                    log::warn!("Android input preferences unavailable: {error:#}");
                    ClientInputPreferences::default()
                }
            };
        let mut input_preferences = InputPreferences::AUTO;
        input_preferences.preferred_scheme = client_input_preferences.controller.preferred_input;
        input_preferences.touch_controls = client_input_preferences.touch_controls_mode;
        let interactive_input = MonoInteractiveInputRouter::with_controller_preferences(
            &client_input_preferences.controller,
        );
        let touch = TouchInputAdapter::with_settings(TouchInputSettings {
            look_sensitivity: client_input_preferences.touch_look_sensitivity,
            ..TouchInputSettings::default()
        });
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let surface = instance
            .create_surface(window)
            .context("create Android wgpu surface")?;
        let (adapter, device, queue) = pollster::block_on(async {
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    force_fallback_adapter: false,
                })
                .await
                .context("request Android wgpu adapter")?;
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("mclone_android_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    ..Default::default()
                })
                .await
                .context("create Android wgpu device")?;
            Ok::<_, anyhow::Error>((adapter, device, queue))
        })?;
        let adapter_info = adapter.get_info();
        let caps = surface.get_capabilities(&adapter);
        let format = RenderConfig::preferred_surface_format_for_profile(
            &caps,
            RenderColorProfile::default(),
        )
        .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Opaque),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let asset_source = mclone_assets::SharedAssetSource::new(
            load_asset_source().context("load Android game assets")?,
        );
        let mesh_assets = load_textured_mesh_assets_from_source(&asset_source)
            .context("load Android textured mesh assets")?;
        let actor_assets = load_actor_texture_assets_from_asset_source(&asset_source)
            .context("load Android actor assets")?;
        let mut host = create_android_scene_host(
            &device,
            &queue,
            format,
            &startup.scene,
            startup.remote_addr.as_deref(),
            startup.render_options,
            mesh_assets,
            actor_assets.atlas,
            actor_assets.figures,
            &asset_source,
        )?;
        if let Some(registry) = mclone_app_runtime::prepared_assets::AssetPackSourceRegistry::discover_native_with_reference(asset_source.clone())? {
            host.configure_asset_pack_sources(
                registry,
                mclone_app_runtime::prepared_assets::reference_asset_pack_selection(),
            )?;
            if let Some(path) = mclone_app_runtime::asset_pack_preferences::native_asset_pack_preference_path(startup.scene.world_root.as_deref()) {
                host.configure_asset_pack_preference_storage(Box::new(
                    mclone_app_runtime::asset_pack_preferences::FileAssetPackPreferenceStorage::new(path),
                ))?;
            }
        }
        apply_startup_camera_options(&mut host, startup.camera);
        let mut ui = GameUiHost::new_ingame();
        ui.set_new_world_seed(startup.scene.seed);
        ui.set_join_remote_addr(
            startup
                .remote_addr
                .clone()
                .unwrap_or_else(|| DEFAULT_JOIN_REMOTE_ADDR.to_owned()),
        );
        host.configure_mono_ui_with_profile(
            ui,
            MonoUiContext::default(),
            android_flat_native_client_experience_profile(),
        );
        host.set_frame_host_kind(FrameHostKind::FlatAndroidWinit);
        host.set_display_refresh_hz(Some(ANDROID_FIXED_FPS_CAP as f32));
        let audio = match AudioEngine::new(&asset_source, AudioSettings::default()) {
            Ok(audio) => Some(audio),
            Err(error) => {
                log::warn!("Mclone Android audio disabled: {error:#}");
                None
            }
        };
        host.set_audio_output(audio.map_or_else(
            || mclone_audio::AudioOutputCapability::Unavailable,
            mclone_audio::AudioOutputCapability::available,
        ));
        host.set_mono_ui_scale(GuiScale::from_pixels(config.width, config.height));

        log::info!(
            "Mclone Android wgpu adapter '{}' backend={:?} format={format:?}",
            adapter_info.name,
            adapter_info.backend
        );
        let announced_session = Some(startup.remote_addr.as_ref().map_or_else(
            || ActiveSessionDescriptor::new_seed_local_world(startup.scene.seed),
            |address| ActiveSessionDescriptor::Remote {
                endpoint: RemoteSessionEndpoint::new(address.clone()),
            },
        ));
        let depth = ChunkDepthTarget::new(&device, config.width, config.height);
        Ok(Self {
            surface,
            device,
            queue,
            depth,
            split_presentation: None,
            config,
            host,
            input_capabilities: InputCapabilityState::new(InputCapabilities {
                touch: true,
                ..InputCapabilities::NONE
            }),
            input_preferences,
            client_input_preferences,
            input_preference_world_root,
            interactive_input,
            touch,
            ui_touch_id: None,
            frame_timing: FrameTimingStats::default(),
            frame_pipeline: FramePipelineAccountant::new(xr_frame_pipeline_accounting_config(
                Some(ANDROID_FIXED_FPS_CAP as f64),
            )),
            pacing_perf: startup.pacing_perf.map(|options| {
                AndroidPacingPerfState::new(
                    options,
                    adapter_info.name,
                    format!("{:?}", adapter_info.backend),
                )
            }),
            last_frame: Instant::now(),
            frame_index: 0,
            announced_session,
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.config.width = size.width.max(1);
        self.config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.depth = ChunkDepthTarget::new(&self.device, self.config.width, self.config.height);
        self.host.set_mono_ui_scale(self.gui_scale());
    }

    fn gui_scale(&self) -> GuiScale {
        GuiScale::from_pixels(self.config.width, self.config.height)
    }

    fn ui_context(&self) -> MonoUiContext {
        let resolved_input = self.input_capabilities.resolve(self.input_preferences);
        let overlay = self.touch.overlay_state();
        MonoUiContext {
            resolved_input,
            controller_layout: self
                .interactive_input
                .latest_controller_actions()
                .active_controller_layout
                .unwrap_or(mclone_input::ControllerLayoutFamily::Unknown),
            frame_pacing: FramePacingUiState {
                mode: FramePacingMode::Vsync,
                fps_cap: ANDROID_FIXED_FPS_CAP,
            },
            pacing_debug: FramePacingDebugStats {
                mode: FramePacingMode::Vsync,
                fps_cap: ANDROID_FIXED_FPS_CAP,
                monitor_refresh_hz: Some(ANDROID_FIXED_FPS_CAP as f32),
                target_frame_ms: Some(ANDROID_TARGET_FRAME_MS),
                active_present_mode_label: "fifo",
            },
            frame_timing: self.frame_timing,
            render_scale: 1.0,
            hud_visible: true,
            touch_overlay: TouchOverlay {
                visible: true,
                menu_pressed: overlay.menu_pressed,
                movement: overlay
                    .movement
                    .map(|movement| TouchJoystickOverlay {
                        active: true,
                        base: point_from_vec2(movement.base),
                        thumb: point_from_vec2(movement.thumb),
                    })
                    .unwrap_or_default(),
                jump_pressed: overlay.jump_pressed,
                sprint_pressed: overlay.sprint_pressed,
                sneak_pressed: overlay.sneak_pressed,
                descend_pressed: overlay.descend_pressed,
                interaction_visible: true,
                attack_pressed: overlay.attack_pressed,
                use_pressed: overlay.use_pressed,
                hotbar_visible: true,
                selected_hotbar_slot: self.host.selected_mono_hotbar_slot(),
                hotbar_pressed_slot: overlay.hotbar_pressed_slot,
                hotbar_icons: EMPTY_HOTBAR_ICONS,
            },
            touch_controls_mode: Some(self.input_preferences.touch_controls),
            touch_settings: Some(GameTouchSettings::new(
                self.touch.settings.look_sensitivity,
                TouchInputSettings::MIN_LOOK_SENSITIVITY,
                TouchInputSettings::MAX_LOOK_SENSITIVITY,
            )),
            auxiliary_split_mode: GameAuxiliarySplitMode::Off,
        }
    }

    fn render_mclone_frame(&mut self) -> std::result::Result<(), AndroidRenderError> {
        let frame_start = Instant::now();
        let frame_ms = frame_start.duration_since(self.last_frame).as_secs_f64() * 1_000.0;
        self.last_frame = frame_start;
        self.frame_timing
            .begin_frame(frame_ms, Some(ANDROID_TARGET_FRAME_MS));
        self.prepare_pacing_perf_frame(frame_start)
            .map_err(AndroidRenderError::Render)?;
        self.drive_held_input(frame_ms / 1_000.0)
            .map_err(AndroidRenderError::Render)?;
        self.host.set_mono_ui_context(self.ui_context());

        let acquire_start = Instant::now();
        let surface_frame = self
            .surface
            .get_current_texture()
            .map_err(AndroidRenderError::Surface)?;
        let acquire_ms = acquire_start.elapsed().as_secs_f64() * 1_000.0;
        let view = surface_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_android_frame_encoder"),
            });
        let encode_start = Instant::now();
        let output_target =
            RenderFrameTarget::color(&view, [self.config.width, self.config.height]);
        let split_layout = self
            .host
            .auxiliary_split_layout(output_target.size)
            .map_err(AndroidRenderError::Render)?;
        let summary = if let Some(layout) = split_layout {
            let primary = layout.panes()[0];
            self.host
                .set_mono_ui_scale(GuiScale::from_pixels(primary.width, primary.height));
            match &mut self.split_presentation {
                Some(presentation) => presentation.resize(&self.device, layout),
                slot @ None => {
                    *slot = Some(FlatSurfacePresentation::new(
                        &self.device,
                        self.config.format,
                        layout,
                    ));
                }
            }
            let presentation = self
                .split_presentation
                .as_ref()
                .expect("split presentation initialized from active layout");
            let summary = self
                .host
                .render_auxiliary_split_surface_frame(
                    &self.device,
                    &self.queue,
                    &mut encoder,
                    presentation,
                )
                .map_err(AndroidRenderError::Render)?;
            presentation.present(&mut encoder, output_target);
            summary
        } else {
            if !self.host.auxiliary_split_mode().is_split() {
                self.split_presentation = None;
            }
            self.host.set_mono_ui_scale(self.gui_scale());
            let render_view = self
                .host
                .mono_render_view(output_target.size)
                .map_err(AndroidRenderError::Render)?;
            self.host
                .render_mono_scene_frame(
                    RenderFrameContext::new(&self.device, &self.queue, &mut encoder, output_target),
                    &self.depth,
                    render_view,
                    MonoUiPresentation::ScreenSpaceHud,
                )
                .map_err(AndroidRenderError::Render)?
        };
        let encode_ms = encode_start.elapsed().as_secs_f64() * 1_000.0;
        let submit_start = Instant::now();
        self.queue.submit(std::iter::once(encoder.finish()));
        let submit_ms = submit_start.elapsed().as_secs_f64() * 1_000.0;
        let present_start = Instant::now();
        surface_frame.present();
        let present_ms = present_start.elapsed().as_secs_f64() * 1_000.0;

        self.record_frame_timing(
            frame_start,
            acquire_ms,
            encode_ms,
            submit_ms,
            present_ms,
            &summary,
        );
        self.finish_pacing_perf_frame(frame_start);
        self.announce_session_change();
        if self.frame_index == 0 {
            log::info!(
                "Mclone Android rendered shared Mono frame: sections={}/{} indices={}/{} gui_commands={} flat_hud_retained_rebuilds={} flat_hud_retained_cache_hits={}",
                summary.render.drawn_section_count,
                summary.render.section_count,
                summary.render.drawn_index_count,
                summary.render.index_count,
                summary.render.gui_command_count,
                summary.render.flat_hud_retained_cache.rebuild_count,
                summary.render.flat_hud_retained_cache.cache_hit_count
            );
        }
        self.frame_index = self.frame_index.saturating_add(1);
        Ok(())
    }

    fn prepare_pacing_perf_frame(&mut self, now: Instant) -> Result<()> {
        let Some(perf) = self.pacing_perf.as_mut() else {
            return Ok(());
        };
        if perf.completed {
            return Ok(());
        }
        let first_frame = *perf.first_frame.get_or_insert(now);
        if perf.active_started.is_none()
            && now.saturating_duration_since(first_frame)
                >= Duration::from_secs(perf.options.warmup_seconds)
        {
            perf.active_started = Some(now);
            self.frame_pipeline = FramePipelineAccountant::new(
                xr_frame_pipeline_accounting_config(Some(ANDROID_FIXED_FPS_CAP as f64)),
            );
            log::info!(
                "MCLONE_ANDROID_PACING_PERF_START label={} workload=chunk-view-churn warmup_seconds={} sample_seconds={} churn_interval_seconds={:.3} churn_offset_chunks={} target_hz={} adapter={} backend={}",
                perf.options.label,
                perf.options.warmup_seconds,
                perf.options.sample_seconds,
                perf.options.churn_interval_seconds,
                perf.options.churn_offset_chunks,
                ANDROID_FIXED_FPS_CAP,
                perf.adapter_name,
                perf.adapter_backend
            );
        }
        let Some(active_started) = perf.active_started else {
            return Ok(());
        };
        let elapsed_seconds = now.saturating_duration_since(active_started).as_secs_f64();
        let step = (elapsed_seconds / perf.options.churn_interval_seconds).floor() as u64;
        let offset = if step % 2 == 0 {
            0
        } else {
            perf.options.churn_offset_chunks
        };
        let center = [
            perf.options.base_chunk_x.saturating_add(offset),
            perf.options.base_chunk_z,
        ];
        if perf.last_center == Some(center) {
            return Ok(());
        }
        perf.last_center = Some(center);

        let camera = self.host.camera_snapshot();
        let eye = Vec3d::new(
            f64::from(
                center[0]
                    .saturating_mul(CHUNK_WIDTH)
                    .saturating_add(CHUNK_WIDTH / 2),
            ),
            camera.eye.y,
            f64::from(
                center[1]
                    .saturating_mul(CHUNK_WIDTH)
                    .saturating_add(CHUNK_WIDTH / 2),
            ),
        );
        self.host.set_mono_capture_camera(
            eye,
            camera.yaw_radians,
            camera.pitch_radians,
            camera.speed_blocks_per_second,
        );
        log::info!(
            "MCLONE_ANDROID_PACING_PERF_CHURN label={} center_x={} center_z={}",
            perf.options.label,
            center[0],
            center[1]
        );
        Ok(())
    }

    fn finish_pacing_perf_frame(&mut self, now: Instant) {
        let Some(perf) = self.pacing_perf.as_mut() else {
            return;
        };
        let Some(active_started) = perf.active_started else {
            return;
        };
        let Some((report, _)) = self.frame_pipeline.latest_report() else {
            return;
        };
        perf.max_queue_depth = perf.max_queue_depth.max(
            report
                .queue_panel
                .queues
                .iter()
                .map(|queue| queue.depth)
                .max()
                .unwrap_or(0),
        );
        if perf.completed
            || now.saturating_duration_since(active_started)
                < Duration::from_secs(perf.options.sample_seconds)
        {
            return;
        }
        log_android_pacing_perf_summary(perf, &report);
        perf.completed = true;
    }

    fn record_frame_timing(
        &mut self,
        frame_start: Instant,
        acquire_ms: f64,
        encode_ms: f64,
        submit_ms: f64,
        present_ms: f64,
        summary: &MonoSceneFrameSummary,
    ) {
        self.frame_timing
            .record_runtime_poll(summary.timing.runtime_poll_ms);
        self.frame_timing.record_remesh_upload(
            summary.timing.runtime_sync_ms,
            summary.timing.runtime_gpu_upload_ms,
        );
        self.frame_timing.record_surface_frame(
            summary.timing.render_views_ms,
            acquire_ms,
            encode_ms,
            submit_ms,
            present_ms,
        );
        let wall_ms = frame_start.elapsed().as_secs_f64() * 1_000.0;
        let budget = self.host.latest_budget_decision_panel();
        let (report, revision) = record_mono_frame_pipeline(
            &mut self.frame_pipeline,
            wall_ms,
            true,
            Some(summary.clone()),
            budget,
        );
        self.host.set_frame_pipeline_report(report, revision);
    }

    fn drive_held_input(&mut self, dt_seconds: f64) -> Result<()> {
        self.interactive_input.advance_held_frame(
            &mut self.host,
            self.touch.held_frame(),
            dt_seconds.clamp(0.0, TOUCH_MOVEMENT_MAX_FRAME_SECONDS),
        )?;
        if !self.host.mono_ui_is_active() {
            self.host.update_mono_blink_debug();
        }
        Ok(())
    }

    fn route_controller_poll(
        &mut self,
        poll: AndroidControllerPoll,
    ) -> Result<AndroidInputOutcome> {
        self.input_capabilities
            .set_present(InputDeviceKind::Gamepad, poll.connected_count() > 0);
        for source_id in poll.disconnected {
            self.interactive_input
                .disconnect_controller_source(source_id)?;
        }
        for (source_id, descriptor) in poll.connected {
            self.interactive_input
                .connect_controller_source(source_id, descriptor);
        }
        let mut effects = AndroidHostEffects::default();
        let disposition = self.interactive_input.route_controller_samples(
            &mut self.host,
            poll.sample_time,
            poll.samples,
            &self.device,
            &self.queue,
            &mut effects,
        )?;
        if disposition.meaningful_controller_activity {
            self.input_capabilities
                .note_activity(InputDeviceKind::Gamepad);
        }
        Ok(self.finish_input_disposition(disposition, effects))
    }

    fn route_flat_frame(&mut self, frame: FlatInputFrame) -> Result<AndroidInputOutcome> {
        let mut effects = AndroidHostEffects::default();
        let disposition = self.interactive_input.route_flat_frame(
            &mut self.host,
            frame,
            &self.device,
            &self.queue,
            &mut effects,
        )?;
        Ok(self.finish_input_disposition(disposition, effects))
    }

    fn apply_touch_event(&mut self, event: TouchInputEvent) -> Result<AndroidInputOutcome> {
        let mut outcome = AndroidInputOutcome {
            handled: event.handled,
            exit: false,
        };
        if let Some(delta) = event.look_delta {
            let disposition = self
                .interactive_input
                .route_touch_look(&mut self.host, delta);
            outcome.handled |= disposition.handled;
            log::info!(
                "Mclone Android touch look moved: yaw={:.4} pitch={:.4}",
                delta.yaw_radians,
                delta.pitch_radians
            );
        }
        if let Some(frame) = event.frame {
            let routed = self.route_flat_frame(frame)?;
            outcome.handled |= routed.handled;
            outcome.exit |= routed.exit;
        }
        Ok(outcome)
    }

    fn finish_input_disposition(
        &mut self,
        disposition: MonoInputDisposition,
        effects: AndroidHostEffects,
    ) -> AndroidInputOutcome {
        if let Some(mode) = effects.outcome.touch_controls_mode {
            self.input_preferences.touch_controls = mode;
        }
        if let Some(settings) = self.host.mono_ui_render_state().touch_settings {
            self.touch
                .set_look_sensitivity(settings.clamped_look_sensitivity());
        }
        self.persist_input_preferences_if_changed();
        if disposition.clear_transient_input || effects.outcome.quit_to_title {
            self.clear_flat_gameplay_input();
        }
        AndroidInputOutcome {
            handled: disposition.handled,
            exit: effects.outcome.exit,
        }
    }

    fn persist_input_preferences_if_changed(&mut self) {
        let mut current = self.client_input_preferences.clone();
        current.touch_look_sensitivity = self.touch.settings.look_sensitivity;
        current.touch_controls_mode = self.input_preferences.touch_controls;
        let current = current.normalized();
        if current == self.client_input_preferences {
            return;
        }
        match mclone_app_runtime::input_preferences::store_native_input_preferences(
            self.input_preference_world_root.as_deref(),
            &current,
        ) {
            Ok(true) => {}
            Ok(false) => {
                log::debug!("Android input preferences have no configured storage path");
            }
            Err(error) => {
                log::warn!("failed to store Android input preferences: {error:#}");
            }
        }
        self.client_input_preferences = current;
    }

    fn handle_keyboard_input(
        &mut self,
        key_code: KeyCode,
        state: ElementState,
        repeat: bool,
    ) -> Result<AndroidInputOutcome> {
        self.input_capabilities
            .note_activity(InputDeviceKind::Keyboard);
        let Some(key) = KeyboardKey::from_code_name(&format!("{key_code:?}")) else {
            return Ok(AndroidInputOutcome::default());
        };
        let mut effects = AndroidHostEffects::default();
        let disposition = self.interactive_input.route_key(
            &mut self.host,
            key,
            state == ElementState::Pressed,
            repeat,
            &self.device,
            &self.queue,
            &mut effects,
        )?;
        Ok(self.finish_input_disposition(disposition, effects))
    }

    fn handle_mouse_motion(&mut self, delta: (f64, f64)) -> Result<AndroidInputOutcome> {
        self.input_capabilities
            .note_activity(InputDeviceKind::Mouse);
        let mut effects = AndroidHostEffects::default();
        let disposition = self.interactive_input.route_mouse_motion(
            &mut self.host,
            delta.0 as f32,
            delta.1 as f32,
            &self.device,
            &self.queue,
            &mut effects,
        )?;
        Ok(self.finish_input_disposition(disposition, effects))
    }

    fn handle_cursor_move(
        &mut self,
        position: PhysicalPosition<f64>,
    ) -> Result<AndroidInputOutcome> {
        self.input_capabilities
            .note_activity(InputDeviceKind::Mouse);
        let point = self.gui_scale().client_to_gui(position.x, position.y);
        let mut effects = AndroidHostEffects::default();
        let disposition = self.interactive_input.route_pointer_move(
            &mut self.host,
            point,
            &self.device,
            &self.queue,
            &mut effects,
        )?;
        Ok(self.finish_input_disposition(disposition, effects))
    }

    fn handle_mouse_input(
        &mut self,
        cursor: Option<PhysicalPosition<f64>>,
        state: ElementState,
        button: MouseButton,
    ) -> Result<AndroidInputOutcome> {
        self.input_capabilities
            .note_activity(InputDeviceKind::Mouse);
        let Some(button) = pointer_button(button) else {
            return Ok(AndroidInputOutcome::default());
        };
        let point = cursor.map(|cursor| self.gui_scale().client_to_gui(cursor.x, cursor.y));
        let mut effects = AndroidHostEffects::default();
        let disposition = self.interactive_input.route_pointer_button(
            &mut self.host,
            button,
            state == ElementState::Pressed,
            point,
            &self.device,
            &self.queue,
            &mut effects,
        )?;
        Ok(self.finish_input_disposition(disposition, effects))
    }

    fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) -> Result<AndroidInputOutcome> {
        self.input_capabilities
            .note_activity(InputDeviceKind::Mouse);
        let direction = mouse_wheel_direction(delta);
        let mut effects = AndroidHostEffects::default();
        let disposition = self.interactive_input.route_wheel(
            &mut self.host,
            direction,
            &self.device,
            &self.queue,
            &mut effects,
        )?;
        Ok(self.finish_input_disposition(disposition, effects))
    }

    fn handle_touch(&mut self, touch: Touch) -> Result<AndroidInputOutcome> {
        self.input_capabilities
            .note_activity(InputDeviceKind::Touch);
        let scale = self.gui_scale();
        let point = scale.client_to_gui(touch.location.x, touch.location.y);
        let position = Vec2::new(point.x, point.y);
        self.touch
            .set_viewport_size(Vec2::new(scale.width, scale.height));
        if self.ui_touch_id == Some(touch.id) {
            return match touch.phase {
                TouchPhase::Moved => self.route_touch_pointer_move(point),
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    self.ui_touch_id = None;
                    self.route_touch_pointer_button(false, point)
                }
                TouchPhase::Started => Ok(AndroidInputOutcome {
                    handled: true,
                    exit: false,
                }),
            };
        }
        match touch.phase {
            TouchPhase::Started if self.host.mono_ui_is_active() && self.ui_touch_id.is_none() => {
                self.ui_touch_id = Some(touch.id);
                self.route_touch_pointer_button(true, point)
            }
            TouchPhase::Started => {
                let control = touch_control_at(scale, point);
                let event = self.touch.begin_contact(touch.id, control, position);
                self.apply_touch_event(event)
            }
            TouchPhase::Moved => {
                let menu_active = touch_menu_button_rect().contains(point);
                let event = self.touch.move_contact(touch.id, position, menu_active);
                self.apply_touch_event(event)
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                let event = self
                    .touch
                    .end_contact(touch.id, touch.phase == TouchPhase::Cancelled);
                self.apply_touch_event(event)
            }
        }
    }

    fn clear_flat_gameplay_input(&mut self) {
        self.interactive_input.clear_transient_input();
        self.touch.clear();
        self.ui_touch_id = None;
        self.host.clear_mono_camera_input();
        self.host.clear_mono_blink_debug();
    }

    fn route_touch_pointer_move(&mut self, point: Point) -> Result<AndroidInputOutcome> {
        let mut effects = AndroidHostEffects::default();
        let disposition = self.interactive_input.route_pointer_move(
            &mut self.host,
            point,
            &self.device,
            &self.queue,
            &mut effects,
        )?;
        Ok(self.finish_input_disposition(disposition, effects))
    }

    fn route_touch_pointer_button(
        &mut self,
        pressed: bool,
        point: Point,
    ) -> Result<AndroidInputOutcome> {
        let mut effects = AndroidHostEffects::default();
        let disposition = self.interactive_input.route_pointer_button(
            &mut self.host,
            PointerButton::Primary,
            pressed,
            Some(point),
            &self.device,
            &self.queue,
            &mut effects,
        )?;
        Ok(self.finish_input_disposition(disposition, effects))
    }

    fn on_background(&mut self) -> Result<()> {
        self.clear_flat_gameplay_input();
        self.host.on_background()?;
        Ok(())
    }

    fn announce_session_change(&mut self) {
        let GameSessionState::Active { session } = self.host.session_state() else {
            return;
        };
        if self.announced_session.as_ref() == Some(session) {
            return;
        }
        match session {
            ActiveSessionDescriptor::LocalWorld { seed, .. } => {
                log::info!("Mclone Android created local world seed={seed}");
            }
            ActiveSessionDescriptor::Remote { endpoint } => {
                log::info!("Mclone Android joined remote world at {}", endpoint.address);
            }
        }
        self.announced_session = Some(session.clone());
    }
}

#[derive(Clone, Debug)]
struct AndroidPacingPerfState {
    options: AndroidPacingPerfOptions,
    adapter_name: String,
    adapter_backend: String,
    first_frame: Option<Instant>,
    active_started: Option<Instant>,
    last_center: Option<[i32; 2]>,
    max_queue_depth: u64,
    completed: bool,
}

impl AndroidPacingPerfState {
    fn new(
        options: AndroidPacingPerfOptions,
        adapter_name: String,
        adapter_backend: String,
    ) -> Self {
        Self {
            options,
            adapter_name,
            adapter_backend,
            first_frame: None,
            active_started: None,
            last_center: None,
            max_queue_depth: 0,
            completed: false,
        }
    }
}

fn log_android_pacing_perf_summary(perf: &AndroidPacingPerfState, report: &FramePipelineReport) {
    let frames = &report.frame_summary;
    let queue_conservation_violations = report
        .queue_panel
        .queues
        .iter()
        .map(|queue| queue.conservation_violations)
        .sum::<u64>();
    let max_queue_age_ms = report
        .queue_panel
        .queues
        .iter()
        .map(|queue| queue.max_oldest_age_ms)
        .max_by(f64::total_cmp)
        .unwrap_or(0.0);
    let summary = serde_json::json!({
        "schemaVersion": report.schema_version,
        "label": perf.options.label,
        "workload": "chunk-view-churn",
        "adapter": perf.adapter_name,
        "backend": perf.adapter_backend,
        "targetHz": frames.target_hz,
        "warmupSeconds": perf.options.warmup_seconds,
        "sampleSeconds": perf.options.sample_seconds,
        "churnIntervalSeconds": perf.options.churn_interval_seconds,
        "churnOffsetChunks": perf.options.churn_offset_chunks,
        "frames": frames.frames,
        "renderedFrames": frames.rendered_frames,
        "appWorkAverageMs": frames.app_work.average_ms,
        "appWorkP50Ms": frames.app_work.p50_ms,
        "appWorkP95Ms": frames.app_work.p95_ms,
        "appWorkP99Ms": frames.app_work.p99_ms,
        "appWorkMaxMs": frames.app_work.max_ms,
        "headroomAverageMs": frames.headroom.average_ms,
        "headroomP05Ms": frames.headroom.p05_ms,
        "headroomMinMs": frames.headroom.min_ms,
        "overBudget": frames.over_budget,
        "appOverPeriodFrames": frames.app_over_period_frames,
        "appOverPeriodPct": frames.app_over_period_pct,
        "frameConservationViolations": frames.conservation_violations.total(),
        "queueConservationViolations": queue_conservation_violations,
        "maxQueueDepth": perf.max_queue_depth,
        "maxQueueAgeMs": max_queue_age_ms,
    });
    log::info!("MCLONE_ANDROID_PACING_PERF_SUMMARY {summary}");
}

#[allow(clippy::too_many_arguments)]
fn create_android_scene_host(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color_format: wgpu::TextureFormat,
    scene: &McloneSceneHostOptions,
    remote_addr: Option<&str>,
    render_options: TexturedSectionRenderOptions,
    mesh_assets: TexturedMeshAssets,
    actor_atlas: mclone_render::actor_assets::ActorTextureImage,
    actor_figures: mclone_render::entity::ActorFigureSet,
    asset_source: &impl mclone_assets::AssetSource,
) -> Result<AndroidSceneHost> {
    let mut host: AndroidSceneHost = if let Some(remote_addr) = remote_addr {
        let endpoint = RemoteSessionEndpoint::new(remote_addr);
        let runtime = android_remote_runtime(&endpoint, scene, mesh_assets)?;
        McloneSceneHost::with_runtime(
            device,
            queue,
            color_format,
            mclone_app_runtime::monotonic::system_monotonic_clock(),
            scene.clone(),
            runtime,
            render_options,
            actor_atlas,
            actor_figures,
            asset_source,
            None,
        )
    } else {
        McloneSceneHost::start_local_async(
            device,
            queue,
            color_format,
            mclone_app_runtime::monotonic::system_monotonic_clock(),
            scene.clone(),
            render_options,
            mesh_assets,
            actor_atlas,
            actor_figures,
            asset_source,
            None,
        )
    }
    .context("initialize Android shared Mono scene host")?;
    host.set_teleport_preview_capability(mclone_client::native_teleport_preview_capability());
    host.set_session_runtime_factory(|endpoint, scene, mesh_assets| {
        android_remote_runtime(&endpoint, &scene, mesh_assets)
    });
    Ok(host)
}

fn android_remote_runtime(
    endpoint: &RemoteSessionEndpoint,
    scene: &McloneSceneHostOptions,
    mesh_assets: TexturedMeshAssets,
) -> Result<NativeSessionServices<NativeRemoteServerSession>> {
    let profile = mclone_app_runtime::local_profile::load_or_create_native_local_player_profile(
        scene.world_root.as_deref(),
    )?;
    connect_native_remote_session_runtime_with_identity(
        endpoint.clone(),
        SingleViewHostOptions::new(scene.center(), scene.render_distance)
            .with_render_compile_worker_count(scene.render_compile_worker_count)
            .with_render_compile_max_pending_jobs(scene.render_compile_max_pending_jobs)
            .with_render_compile_worker_timing_enabled(scene.render_compile_worker_timing_enabled),
        mesh_assets,
        "Android",
        profile.client_identity(),
    )
    .with_context(|| {
        format!(
            "failed to initialize Android remote dedicated runtime from {}",
            endpoint.address
        )
    })
}

fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        _ => None,
    }
}

fn mouse_wheel_direction(delta: MouseScrollDelta) -> MouseWheelDirection {
    let y = match delta {
        MouseScrollDelta::LineDelta(_, y) => y,
        MouseScrollDelta::PixelDelta(position) => position.y as f32,
    };
    if y > 0.0 {
        MouseWheelDirection::Up
    } else {
        MouseWheelDirection::Down
    }
}

fn point_from_vec2(point: Vec2) -> Point {
    Point {
        x: point.x,
        y: point.y,
    }
}
