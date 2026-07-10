use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use glam::Vec2;
use mclone_app_runtime::client_experience::android_flat_native_client_experience_profile;
use mclone_app_runtime::frame_pacing::{
    FramePacingDebugStats, FramePacingMode, FramePacingUiState, FrameTimingStats,
};
use mclone_app_runtime::frame_pipeline_accounting::FramePipelineAccountant;
use mclone_app_runtime::host_mode::SingleViewHostOptions;
use mclone_app_runtime::native_remote_session::{
    NativeRemoteServerSession, connect_native_remote_session_runtime,
};
use mclone_app_runtime::native_session_runtime::NativeSessionRuntime;
use mclone_app_runtime::render_assets::{
    TexturedMeshAssets, load_actor_texture_assets_from_asset_source, load_asset_source,
    load_textured_mesh_assets_from_source,
};
use mclone_app_runtime::session::{
    ActiveSessionDescriptor, GameSessionState, RemoteSessionEndpoint,
};
use mclone_audio::{AudioEngine, AudioSettings};
use mclone_diagnostics::FrameHostKind;
use mclone_input::{
    FlatInputAction, FlatInputFrame, InputCapabilities, InputCapabilityState, InputDeviceKind,
    InputPreferences, KeyboardKey, KeyboardMouseInputAdapter, MouseWheelDirection, PointerButton,
    TouchControl, TouchControlsMode, TouchInputAdapter, TouchInputEvent, TouchInputSettings,
};
use mclone_render::chunk::{ChunkDepthTarget, TexturedSectionRenderOptions};
use mclone_render::color_profile::{RenderColorProfile, RenderConfig};
use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
use mclone_scene::{
    HostEffects, McloneSceneHost, McloneSceneHostOptions, MonoSceneFrameSummary, MonoUiContext,
    MonoUiPresentation, MonoWorldActionStatus, record_mono_frame_pipeline,
    xr_frame_pipeline_accounting_config,
};
use mclone_ui::{
    DEFAULT_JOIN_REMOTE_ADDR, EMPTY_HOTBAR_ICONS, GameHelpParent, GameTouchSettings, GameUiAction,
    GameUiHost, GuiKey, GuiScale, Point, TouchJoystickOverlay, TouchOverlay,
    touch_action_button_rects, touch_hotbar_slot_rects, touch_menu_button_rect,
    touch_movement_zone_rect,
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
use crate::startup::{AndroidStartupOptions, apply_startup_camera_options};

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
}

impl AndroidSurfaceDriver {
    pub(crate) fn new(startup_options: AndroidStartupOptions) -> Self {
        Self {
            startup_options,
            window: None,
            gpu: None,
            last_cursor: None,
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
                    Ok(gpu) => self.gpu = Some(gpu),
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
                let Some(gpu) = self.gpu.as_mut() else {
                    return;
                };
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
    host: AndroidSceneHost,
    input_capabilities: InputCapabilityState,
    input_preferences: InputPreferences,
    keyboard_mouse: KeyboardMouseInputAdapter,
    touch: TouchInputAdapter,
    ui_touch_id: Option<u64>,
    frame_timing: FrameTimingStats,
    frame_pipeline: FramePipelineAccountant,
    last_frame: Instant,
    frame_index: u64,
    announced_session: Option<ActiveSessionDescriptor>,
}

impl AndroidGpuState {
    fn new(window: Arc<Window>, startup: AndroidStartupOptions) -> Result<Self> {
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

        let asset_source = load_asset_source().context("load Android game assets")?;
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
        host.set_audio_engine(audio);
        host.set_mono_ui_scale(GuiScale::from_pixels(config.width, config.height));

        log::info!(
            "Mclone Android wgpu adapter '{}' backend={:?} format={format:?}",
            adapter.get_info().name,
            adapter.get_info().backend
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
            config,
            host,
            input_capabilities: InputCapabilityState::new(InputCapabilities {
                touch: true,
                ..InputCapabilities::NONE
            }),
            input_preferences: InputPreferences::AUTO,
            keyboard_mouse: KeyboardMouseInputAdapter::new(),
            touch: TouchInputAdapter::new(),
            ui_touch_id: None,
            frame_timing: FrameTimingStats::default(),
            frame_pipeline: FramePipelineAccountant::new(xr_frame_pipeline_accounting_config(
                Some(ANDROID_FIXED_FPS_CAP as f64),
            )),
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
        }
    }

    fn render_mclone_frame(&mut self) -> std::result::Result<(), AndroidRenderError> {
        let frame_start = Instant::now();
        let frame_ms = frame_start.duration_since(self.last_frame).as_secs_f64() * 1_000.0;
        self.last_frame = frame_start;
        self.frame_timing
            .begin_frame(frame_ms, Some(ANDROID_TARGET_FRAME_MS));
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
        let render_view = self
            .host
            .mono_render_view([self.config.width, self.config.height])
            .map_err(AndroidRenderError::Render)?;
        let encode_start = Instant::now();
        let summary = self
            .host
            .render_mono_scene_frame(
                RenderFrameContext::new(
                    &self.device,
                    &self.queue,
                    &mut encoder,
                    RenderFrameTarget::color(&view, [self.config.width, self.config.height]),
                ),
                &self.depth,
                render_view,
                MonoUiPresentation::ScreenSpaceHud,
            )
            .map_err(AndroidRenderError::Render)?;
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
        if self.host.mono_ui_is_active() {
            return Ok(());
        }
        let mut frame = self.keyboard_mouse.held_frame().unwrap_or_default();
        if let Some(touch) = self.touch.held_frame() {
            frame.merge_from(touch);
        }
        if self.host.apply_mono_movement_frame(
            frame,
            dt_seconds.clamp(0.0, TOUCH_MOVEMENT_MAX_FRAME_SECONDS),
        ) {
            self.host.commit_mono_player_pose()?;
        }
        self.host.update_mono_blink_debug();
        Ok(())
    }

    fn apply_flat_frame(&mut self, frame: FlatInputFrame) -> Result<AndroidInputOutcome> {
        let mut outcome = AndroidInputOutcome {
            handled: true,
            exit: false,
        };
        if frame.open_menu {
            self.host.open_mono_pause_menu();
            self.clear_flat_gameplay_input();
            return Ok(outcome);
        }
        if frame.open_block_palette {
            self.host.open_mono_block_palette();
            self.clear_flat_gameplay_input();
            return Ok(outcome);
        }
        if frame.open_help {
            return self.apply_ui_action(GameUiAction::OpenHelp(GameHelpParent::Game), false);
        }
        if frame.toggle_camera_view {
            log::info!(
                "camera view mode {}",
                self.host.toggle_mono_camera_view().label()
            );
        }
        if let Some(slot) = frame.selected_hotbar_slot {
            self.host.select_mono_hotbar_slot(slot);
        }
        if frame.hotbar_step != 0 {
            self.host.step_mono_hotbar_slot(frame.hotbar_step);
        }
        for (pressed, action) in [
            (frame.attack, FlatInputAction::Attack),
            (frame.use_item, FlatInputAction::Use),
        ] {
            if pressed
                && let MonoWorldActionStatus::Sent { target, changed } =
                    self.host.handle_mono_world_action(action)?
            {
                log::info!(
                    "Android gameplay interaction {:?} at ({}, {}, {}) changed={}",
                    action,
                    target.hit.block_pos.x,
                    target.hit.block_pos.y,
                    target.hit.block_pos.z,
                    changed
                );
            }
        }
        if self.host.apply_mono_look_frame(frame) {
            self.host.commit_mono_player_pose()?;
        }
        outcome.handled = frame != FlatInputFrame::default();
        Ok(outcome)
    }

    fn apply_touch_event(&mut self, event: TouchInputEvent) -> Result<AndroidInputOutcome> {
        let mut outcome = AndroidInputOutcome {
            handled: event.handled,
            exit: false,
        };
        if let Some(delta) = event.look_delta {
            if self.host.apply_mono_touch_look(delta) {
                self.host.commit_mono_player_pose()?;
            }
            log::info!(
                "Mclone Android touch look moved: yaw={:.4} pitch={:.4}",
                delta.yaw_radians,
                delta.pitch_radians
            );
        }
        if let Some(frame) = event.frame {
            outcome = self.apply_flat_frame(frame)?;
        }
        Ok(outcome)
    }

    fn apply_ui_action(
        &mut self,
        action: GameUiAction,
        from_pointer_click: bool,
    ) -> Result<AndroidInputOutcome> {
        let mut effects = AndroidHostEffects::default();
        let scene = self.host.apply_mono_ui_action(
            action,
            from_pointer_click,
            &self.device,
            &self.queue,
            &mut effects,
        )?;
        if let GameUiAction::SetTouchLookSensitivity(value) = action {
            self.touch.set_look_sensitivity(value);
        }
        if let Some(mode) = effects.outcome.touch_controls_mode {
            self.input_preferences.touch_controls = mode;
        }
        if scene.clear_gameplay_input || effects.outcome.quit_to_title {
            self.clear_flat_gameplay_input();
        }
        Ok(AndroidInputOutcome {
            handled: true,
            exit: effects.outcome.exit,
        })
    }

    fn handle_keyboard_input(
        &mut self,
        key_code: KeyCode,
        state: ElementState,
        repeat: bool,
    ) -> Result<AndroidInputOutcome> {
        self.input_capabilities
            .note_activity(InputDeviceKind::Keyboard);
        if state == ElementState::Pressed
            && let Some(gui_key) = gui_key_from_key_code(key_code)
        {
            let (handled, action) = self.host.mono_ui_key_pressed(gui_key);
            if let Some(action) = action {
                return self.apply_ui_action(action, false);
            }
            if handled {
                return Ok(AndroidInputOutcome {
                    handled: true,
                    exit: false,
                });
            }
        }
        let Some(key) = KeyboardKey::from_code_name(&format!("{key_code:?}")) else {
            return Ok(AndroidInputOutcome::default());
        };
        let event = self
            .keyboard_mouse
            .handle_key(key, state == ElementState::Pressed, repeat);
        event.frame.map_or_else(
            || {
                Ok(AndroidInputOutcome {
                    handled: event.handled,
                    exit: false,
                })
            },
            |frame| self.apply_flat_frame(frame),
        )
    }

    fn handle_mouse_motion(&mut self, delta: (f64, f64)) -> Result<AndroidInputOutcome> {
        self.input_capabilities
            .note_activity(InputDeviceKind::Mouse);
        if self.host.mono_ui_is_active() {
            return Ok(AndroidInputOutcome::default());
        }
        self.keyboard_mouse
            .mouse_motion_frame(delta.0 as f32, delta.1 as f32)
            .map_or_else(
                || Ok(AndroidInputOutcome::default()),
                |frame| self.apply_flat_frame(frame),
            )
    }

    fn handle_cursor_move(
        &mut self,
        position: PhysicalPosition<f64>,
    ) -> Result<AndroidInputOutcome> {
        self.input_capabilities
            .note_activity(InputDeviceKind::Mouse);
        if !self.host.mono_ui_is_active() {
            return Ok(AndroidInputOutcome::default());
        }
        let (handled, action) = self
            .host
            .mono_ui_pointer_move(self.gui_scale().client_to_gui(position.x, position.y));
        action.map_or_else(
            || {
                Ok(AndroidInputOutcome {
                    handled,
                    exit: false,
                })
            },
            |action| self.apply_ui_action(action, true),
        )
    }

    fn handle_mouse_input(
        &mut self,
        cursor: Option<PhysicalPosition<f64>>,
        state: ElementState,
        button: MouseButton,
    ) -> Result<AndroidInputOutcome> {
        self.input_capabilities
            .note_activity(InputDeviceKind::Mouse);
        if self.host.mono_ui_is_active()
            && button == MouseButton::Left
            && let Some(cursor) = cursor
        {
            let point = self.gui_scale().client_to_gui(cursor.x, cursor.y);
            if state == ElementState::Pressed {
                return Ok(AndroidInputOutcome {
                    handled: self.host.mono_ui_pointer_down(point),
                    exit: false,
                });
            }
            let (handled, action) = self.host.mono_ui_pointer_up(point);
            return action.map_or_else(
                || {
                    Ok(AndroidInputOutcome {
                        handled,
                        exit: false,
                    })
                },
                |action| self.apply_ui_action(action, true),
            );
        }
        let Some(button) = pointer_button(button) else {
            return Ok(AndroidInputOutcome::default());
        };
        let event = self
            .keyboard_mouse
            .handle_mouse_button(button, state == ElementState::Pressed);
        event.frame.map_or_else(
            || {
                Ok(AndroidInputOutcome {
                    handled: event.handled,
                    exit: false,
                })
            },
            |frame| self.apply_flat_frame(frame),
        )
    }

    fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) -> Result<AndroidInputOutcome> {
        self.input_capabilities
            .note_activity(InputDeviceKind::Mouse);
        if self.host.mono_ui_is_active() {
            return Ok(AndroidInputOutcome::default());
        }
        let direction = mouse_wheel_direction(delta);
        let event = self.keyboard_mouse.handle_mouse_wheel(direction);
        event.frame.map_or_else(
            || {
                Ok(AndroidInputOutcome {
                    handled: event.handled,
                    exit: false,
                })
            },
            |frame| self.apply_flat_frame(frame),
        )
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
                TouchPhase::Moved => {
                    let (handled, action) = self.host.mono_ui_pointer_move(point);
                    action.map_or_else(
                        || {
                            Ok(AndroidInputOutcome {
                                handled,
                                exit: false,
                            })
                        },
                        |action| self.apply_ui_action(action, true),
                    )
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    self.ui_touch_id = None;
                    let (handled, action) = self.host.mono_ui_pointer_up(point);
                    action.map_or_else(
                        || {
                            Ok(AndroidInputOutcome {
                                handled,
                                exit: false,
                            })
                        },
                        |action| self.apply_ui_action(action, true),
                    )
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
                Ok(AndroidInputOutcome {
                    handled: self.host.mono_ui_pointer_down(point),
                    exit: false,
                })
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
        self.keyboard_mouse.clear_held();
        self.touch.clear();
        self.ui_touch_id = None;
        self.host.clear_mono_camera_input();
        self.host.clear_mono_blink_debug();
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
    host.set_session_runtime_factory(|endpoint, scene, mesh_assets| {
        android_remote_runtime(&endpoint, &scene, mesh_assets)
    });
    Ok(host)
}

fn android_remote_runtime(
    endpoint: &RemoteSessionEndpoint,
    scene: &McloneSceneHostOptions,
    mesh_assets: TexturedMeshAssets,
) -> Result<NativeSessionRuntime<NativeRemoteServerSession>> {
    connect_native_remote_session_runtime(
        endpoint.clone(),
        SingleViewHostOptions::new(scene.center(), scene.render_distance)
            .with_render_compile_worker_count(scene.render_compile_worker_count)
            .with_render_compile_max_pending_jobs(scene.render_compile_max_pending_jobs)
            .with_render_compile_worker_timing_enabled(scene.render_compile_worker_timing_enabled),
        mesh_assets,
        "Android",
    )
    .with_context(|| {
        format!(
            "failed to initialize Android remote dedicated runtime from {}",
            endpoint.address
        )
    })
}

fn touch_control_at(scale: GuiScale, point: Point) -> TouchControl {
    if touch_menu_button_rect().contains(point) {
        return TouchControl::MenuButton;
    }
    let buttons = touch_action_button_rects(scale);
    for (rect, control) in [
        (buttons.jump, TouchControl::JumpButton),
        (buttons.sprint, TouchControl::SprintButton),
        (buttons.sneak, TouchControl::SneakButton),
        (buttons.descend, TouchControl::DescendButton),
        (buttons.attack, TouchControl::AttackButton),
        (buttons.use_item, TouchControl::UseButton),
    ] {
        if rect.contains(point) {
            return control;
        }
    }
    for (slot, rect) in touch_hotbar_slot_rects(scale).into_iter().enumerate() {
        if rect.contains(point) {
            return TouchControl::HotbarSlot(slot as u8);
        }
    }
    if touch_movement_zone_rect(scale).contains(point) {
        TouchControl::MovementStick
    } else {
        TouchControl::LookDrag
    }
}

fn gui_key_from_key_code(key_code: KeyCode) -> Option<GuiKey> {
    match KeyboardKey::from_code_name(&format!("{key_code:?}")) {
        Some(KeyboardKey::Escape) => Some(GuiKey::Escape),
        Some(KeyboardKey::F1) => Some(GuiKey::F1),
        _ => None,
    }
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
