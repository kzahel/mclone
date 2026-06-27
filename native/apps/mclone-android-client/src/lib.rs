#![deny(unsafe_code)]

#[cfg(target_os = "android")]
mod android {
    use std::ffi::{CStr, CString, c_char, c_int};
    use std::sync::Arc;
    use std::sync::Once;
    use std::time::Instant;

    use anyhow::{Context, Result, bail};
    use glam::Vec3;
    use mclone_app_runtime::frame_render::{
        FullFrameGui, RenderStreamStats, record_render_section_update_stats,
        render_full_frame_for_view,
    };
    use mclone_app_runtime::host_mode::{RemoteDedicatedServerSession, SingleViewHostOptions};
    use mclone_app_runtime::local_single_view::{
        LocalSingleViewSceneOptions, NativeSingleViewSessionRuntime,
    };
    use mclone_app_runtime::render_assets::load_asset_source;
    use mclone_app_runtime::session::{
        ActiveSessionDescriptor, RemoteSessionEndpoint, SessionStartRequest,
    };
    use mclone_audio::{AudioEngine, AudioSettings, landing_playback_for_impact};
    use mclone_client::ClientInteractionController;
    use mclone_core::{ChunkPos, Vec3d};
    use mclone_input::{
        FLAT_HOTBAR_SLOT_COUNT, FlatInputAction, FlatInputFrame, FlatInputIntent,
        InputCapabilities, InputCapabilityState, InputDeviceKind, InputPreferences, KeyboardKey,
        KeyboardMouseInputAdapter, MouseWheelDirection, MovementDirection, PointerButton,
    };
    use mclone_mesh::quad_face_count_from_indices;
    use mclone_net::NativeClientSession;
    use mclone_protocol::{ClientCommand, ServerUpdate};
    use mclone_render::chunk::{
        ChunkCamera, ChunkDepthTarget, TexturedSectionDrawResources, TexturedSectionRenderOptions,
        TexturedSectionUploadReport,
    };
    use mclone_render::gui::GuiRenderer;
    use mclone_render::sky_render::SkyRenderer;
    use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
    use mclone_render_session::{
        ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER, ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER,
        ENGINE_CAMERA_MOUSE_SENSITIVITY, EngineCameraController, EngineCameraInput,
        EngineCameraMovementImpulse, EngineCameraMovementMode, EngineRenderCamera,
    };
    use mclone_ui::{
        DEFAULT_JOIN_REMOTE_ADDR, FlatHotbarOverlay, FlatHud, GameFramePacingMode, GameUi,
        GameUiAction, GameUiRenderState, GuiDrawList, GuiKey, GuiScale, Point, StatusOverlay,
        TouchJoystickOverlay, TouchOverlay, render_flat_hud, touch_action_button_rects,
        touch_hotbar_slot_rects, touch_menu_button_rect, touch_movement_zone_rect,
    };
    use winit::application::ApplicationHandler;
    use winit::dpi::PhysicalPosition;
    use winit::event::{
        DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, Touch, TouchPhase,
        WindowEvent,
    };
    use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents, EventLoop};
    use winit::keyboard::{KeyCode, PhysicalKey};
    use winit::platform::android::EventLoopBuilderExtAndroid;
    use winit::platform::android::activity::AndroidApp;
    use winit::window::{Window, WindowAttributes, WindowId};

    const LOG_TAG: &str = "mclone_android";
    const REMOTE_ADDR_PROPERTY: &str = "debug.mclone.remote_addr";
    const REMOTE_ADDR_NONE_SENTINEL: &str = "__mclone_none__";
    const ANDROID_PROPERTY_VALUE_MAX: usize = 92;
    const TOUCH_LOOK_RADIANS_PER_SCREEN: f64 = 2.4;
    const TOUCH_JOYSTICK_RADIUS_GUI: f32 = 50.0;
    const TOUCH_MOVEMENT_MAX_FRAME_SECONDS: f64 = 0.05;
    const ANDROID_MIN_RENDER_DISTANCE: i32 = 1;
    const ANDROID_MAX_RENDER_DISTANCE: i32 = 16;
    const ANDROID_FIXED_FPS_CAP: u32 = 60;

    #[allow(unsafe_code)]
    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_print(
            priority: c_int,
            tag: *const c_char,
            format: *const c_char,
            ...
        ) -> c_int;

        fn __system_property_get(name: *const c_char, value: *mut c_char) -> c_int;
    }

    #[allow(unsafe_code)]
    fn android_log(priority: c_int, message: impl AsRef<str>) {
        let tag = CString::new(LOG_TAG).expect("static log tag contains no nul");
        let format = c"%s";
        let message = CString::new(message.as_ref().replace('\0', "\\0"))
            .expect("nul bytes were escaped before logging");
        unsafe {
            __android_log_print(priority, tag.as_ptr(), format.as_ptr(), message.as_ptr());
        }
    }

    struct AndroidLogger;

    impl log::Log for AndroidLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Info
        }

        fn log(&self, record: &log::Record<'_>) {
            if !self.enabled(record.metadata()) {
                return;
            }
            let priority = match record.level() {
                log::Level::Error => 6,
                log::Level::Warn => 5,
                log::Level::Info => 4,
                log::Level::Debug => 3,
                log::Level::Trace => 2,
            };
            android_log(priority, format!("{}: {}", record.target(), record.args()));
        }

        fn flush(&self) {}
    }

    static LOGGER: AndroidLogger = AndroidLogger;
    static INIT_LOGGER: Once = Once::new();

    fn init_android_logger() {
        INIT_LOGGER.call_once(|| {
            if log::set_logger(&LOGGER).is_ok() {
                log::set_max_level(log::LevelFilter::Info);
            }
        });
    }

    #[allow(unsafe_code)]
    fn configure_android_asset_root(app: &AndroidApp) {
        if let Some(existing) = std::env::var_os("MCLONE_ANDROID_ASSET_ROOT") {
            log::info!(
                "preserving MCLONE_ANDROID_ASSET_ROOT={}",
                std::path::PathBuf::from(existing).display()
            );
            return;
        }
        let Some(path) = app
            .external_data_path()
            .or_else(|| app.internal_data_path())
        else {
            log::warn!("could not resolve Android app data path for MCLONE_ANDROID_ASSET_ROOT");
            return;
        };
        unsafe {
            std::env::set_var("MCLONE_ANDROID_ASSET_ROOT", &path);
        }
        log::info!(
            "configured MCLONE_ANDROID_ASSET_ROOT from app data path: {}",
            path.display()
        );
    }

    struct AndroidGpuState {
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        renderer: AndroidFrameRenderer,
    }

    struct AndroidFrameRenderer {
        depth: ChunkDepthTarget,
        sky: SkyRenderer,
        draw: TexturedSectionDrawResources,
        gui: GuiRenderer,
        scene_options: AndroidSceneOptions,
        scene: AndroidSingleViewSceneRuntime,
        camera: EngineCameraController,
        interaction: ClientInteractionController,
        input_capabilities: InputCapabilityState,
        keyboard_mouse: KeyboardMouseInputAdapter,
        render_options: TexturedSectionRenderOptions,
        ui: GameUi,
        session_status: StatusOverlay,
        touch_menu_touch_id: Option<u64>,
        touch_menu_pressed: bool,
        touch_controls: AndroidTouchControls,
        ui_touch_id: Option<u64>,
        render_stats: RenderStreamStats,
        frame_index: u64,
        last_movement_update: Option<Instant>,
        audio: Option<AudioEngine>,
        seed_reroll_state: u64,
    }

    struct StartedAndroidRenderScene {
        scene: AndroidSingleViewSceneRuntime,
        camera: EngineCameraController,
        draw: TexturedSectionDrawResources,
        render_stats: RenderStreamStats,
    }

    enum AndroidRenderError {
        Surface(wgpu::SurfaceError),
        Render(anyhow::Error),
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct AndroidUiTouchResult {
        handled: bool,
        quit: bool,
    }

    #[derive(Clone, Copy, Debug)]
    struct AndroidMovementTouch {
        id: u64,
        base: Point,
        thumb: Point,
        impulse: EngineCameraMovementImpulse,
    }

    impl AndroidMovementTouch {
        fn new(id: u64, base: Point) -> Self {
            let mut touch = Self {
                id,
                base,
                thumb: base,
                impulse: EngineCameraMovementImpulse::new(0.0, 0.0),
            };
            touch.move_to(base);
            touch
        }

        fn move_to(&mut self, point: Point) {
            let mut dx = point.x - self.base.x;
            let mut dy = point.y - self.base.y;
            if !dx.is_finite() || !dy.is_finite() {
                dx = 0.0;
                dy = 0.0;
            }
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > TOUCH_JOYSTICK_RADIUS_GUI && distance > 0.0 {
                let scale = TOUCH_JOYSTICK_RADIUS_GUI / distance;
                dx *= scale;
                dy *= scale;
            }
            self.thumb = Point {
                x: self.base.x + dx,
                y: self.base.y + dy,
            };
            self.impulse = EngineCameraMovementImpulse::new(
                (-dx / TOUCH_JOYSTICK_RADIUS_GUI).clamp(-1.0, 1.0),
                (-dy / TOUCH_JOYSTICK_RADIUS_GUI).clamp(-1.0, 1.0),
            );
        }
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct AndroidTouchControls {
        movement: Option<AndroidMovementTouch>,
        jump_touch_id: Option<u64>,
        sprint_touch_id: Option<u64>,
        descend_touch_id: Option<u64>,
        attack_touch_id: Option<u64>,
        use_touch_id: Option<u64>,
        hotbar_touch_id: Option<u64>,
        hotbar_pressed_slot: Option<u8>,
    }

    impl AndroidTouchControls {
        fn clear(&mut self) {
            *self = Self::default();
        }

        fn has_continuous_movement_input(self) -> bool {
            self.movement.is_some()
                || self.jump_touch_id.is_some()
                || self.sprint_touch_id.is_some()
                || self.descend_touch_id.is_some()
        }

        fn overlay(self, selected_hotbar_slot: u8) -> TouchOverlay {
            TouchOverlay {
                visible: true,
                menu_pressed: false,
                movement: self
                    .movement
                    .map(|movement| TouchJoystickOverlay {
                        active: true,
                        base: movement.base,
                        thumb: movement.thumb,
                    })
                    .unwrap_or_default(),
                jump_pressed: self.jump_touch_id.is_some(),
                sprint_pressed: self.sprint_touch_id.is_some(),
                descend_pressed: self.descend_touch_id.is_some(),
                interaction_visible: true,
                attack_pressed: self.attack_touch_id.is_some(),
                use_pressed: self.use_touch_id.is_some(),
                hotbar_visible: true,
                selected_hotbar_slot,
                hotbar_pressed_slot: self.hotbar_pressed_slot,
            }
        }

        fn held_frame(self) -> Option<FlatInputFrame> {
            if !self.has_continuous_movement_input() {
                return None;
            }
            let mut frame = FlatInputFrame::default();
            if let Some(movement) = self.movement {
                frame.apply_intent(FlatInputIntent::MoveAnalog {
                    left: movement.impulse.left,
                    forward: movement.impulse.forward,
                });
            }
            if self.jump_touch_id.is_some() {
                frame.apply_intent(FlatInputIntent::Action {
                    action: FlatInputAction::Jump,
                    pressed: true,
                });
            }
            if self.sprint_touch_id.is_some() {
                frame.apply_intent(FlatInputIntent::Action {
                    action: FlatInputAction::Sprint,
                    pressed: true,
                });
            }
            if self.descend_touch_id.is_some() {
                frame.apply_intent(FlatInputIntent::Action {
                    action: FlatInputAction::Descend,
                    pressed: true,
                });
            }
            Some(frame)
        }
    }

    impl AndroidGpuState {
        fn new(window: Arc<Window>) -> Result<Self, String> {
            let size = window.inner_size();
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::VULKAN,
                ..Default::default()
            });
            let surface = instance
                .create_surface(window.clone())
                .map_err(|error| format!("create Android wgpu surface: {error}"))?;
            let (adapter, device, queue) = pollster::block_on(async {
                let adapter = instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        compatible_surface: Some(&surface),
                        power_preference: wgpu::PowerPreference::HighPerformance,
                        force_fallback_adapter: false,
                    })
                    .await
                    .map_err(|error| format!("request Android wgpu adapter: {error}"))?;
                let (device, queue) = adapter
                    .request_device(&wgpu::DeviceDescriptor {
                        label: Some("mclone_android_device"),
                        required_features: wgpu::Features::empty(),
                        required_limits: wgpu::Limits::default(),
                        ..Default::default()
                    })
                    .await
                    .map_err(|error| format!("create Android wgpu device: {error}"))?;
                Ok::<_, String>((adapter, device, queue))
            })?;

            let caps = surface.get_capabilities(&adapter);
            let format = super::preferred_surface_format(&caps);
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
            let renderer = AndroidFrameRenderer::new(
                &device,
                &queue,
                config.format,
                config.width,
                config.height,
            )
            .map_err(|error| format!("initialize Android mclone renderer: {error:#}"))?;
            log::info!(
                "Mclone Android wgpu adapter '{}' backend={:?} format={format:?}",
                adapter.get_info().name,
                adapter.get_info().backend
            );

            Ok(Self {
                surface,
                device,
                queue,
                config,
                renderer,
            })
        }

        fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
            self.config.width = size.width.max(1);
            self.config.height = size.height.max(1);
            self.surface.configure(&self.device, &self.config);
            self.renderer
                .resize(&self.device, self.config.width, self.config.height);
        }

        fn look_camera_by_pixels(&mut self, delta_x: f64, delta_y: f64) -> Result<()> {
            self.renderer.look_camera_by_pixels(
                delta_x,
                delta_y,
                self.config.width,
                self.config.height,
            )
        }

        fn handle_ui_touch(&mut self, touch: &Touch) -> Result<AndroidUiTouchResult> {
            self.renderer.handle_touch(
                touch,
                self.config.width,
                self.config.height,
                &self.device,
                &self.queue,
                self.config.format,
            )
        }

        fn handle_keyboard_input(
            &mut self,
            key_code: KeyCode,
            state: ElementState,
            repeat: bool,
        ) -> Result<AndroidUiTouchResult> {
            self.renderer.handle_keyboard_input(
                key_code,
                state,
                repeat,
                &self.device,
                &self.queue,
                self.config.format,
            )
        }

        fn handle_cursor_move(
            &mut self,
            position: PhysicalPosition<f64>,
        ) -> Result<AndroidUiTouchResult> {
            self.renderer.handle_cursor_move(
                position,
                self.config.width,
                self.config.height,
                &self.device,
                &self.queue,
                self.config.format,
            )
        }

        fn handle_mouse_input(
            &mut self,
            cursor: Option<PhysicalPosition<f64>>,
            state: ElementState,
            button: MouseButton,
        ) -> Result<AndroidUiTouchResult> {
            self.renderer.handle_mouse_input(
                cursor,
                state,
                button,
                self.config.width,
                self.config.height,
                &self.device,
                &self.queue,
                self.config.format,
            )
        }

        fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) -> Result<AndroidUiTouchResult> {
            self.renderer.handle_mouse_wheel(delta)
        }

        fn handle_mouse_motion(&mut self, delta: (f64, f64)) -> Result<bool> {
            self.renderer.handle_mouse_motion(delta)
        }

        fn clear_flat_gameplay_input(&mut self) {
            self.renderer.clear_flat_gameplay_input();
        }

        fn wants_continuous_redraw(&self) -> bool {
            self.renderer.wants_continuous_redraw()
        }

        fn render_mclone_frame(&mut self) -> Result<(), AndroidRenderError> {
            let frame = self
                .surface
                .get_current_texture()
                .map_err(AndroidRenderError::Surface)?;
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("mclone_android_frame_encoder"),
                });
            {
                let target =
                    RenderFrameTarget::color(&view, [self.config.width, self.config.height]);
                let context =
                    RenderFrameContext::new(&self.device, &self.queue, &mut encoder, target);
                self.renderer
                    .render(context)
                    .map_err(AndroidRenderError::Render)?;
            }
            self.queue.submit(std::iter::once(encoder.finish()));
            frame.present();
            Ok(())
        }
    }

    impl AndroidFrameRenderer {
        fn new(
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            format: wgpu::TextureFormat,
            width: u32,
            height: u32,
        ) -> Result<Self> {
            let scene_options = android_scene_options();
            let started = start_android_render_scene(device, queue, format, scene_options.clone())?;
            let depth = ChunkDepthTarget::new(device, width, height);
            let sky = SkyRenderer::new(device, format);
            let audio = match load_asset_source()
                .context("load Android sound assets")
                .and_then(|source| AudioEngine::new(&source, AudioSettings::default()))
            {
                Ok(audio) => Some(audio),
                Err(error) => {
                    log::warn!("Mclone Android audio disabled: {error:#}");
                    None
                }
            };

            Ok(Self {
                depth,
                sky,
                draw: started.draw,
                gui: GuiRenderer::new(device, format),
                scene_options: scene_options.clone(),
                scene: started.scene,
                camera: started.camera,
                interaction: ClientInteractionController::new(),
                input_capabilities: InputCapabilityState::new(InputCapabilities {
                    touch: true,
                    ..InputCapabilities::NONE
                }),
                keyboard_mouse: KeyboardMouseInputAdapter::new(),
                render_options: TexturedSectionRenderOptions::default(),
                ui: android_game_ui_for_scene(&scene_options),
                session_status: StatusOverlay::hidden(),
                touch_menu_touch_id: None,
                touch_menu_pressed: false,
                touch_controls: AndroidTouchControls::default(),
                ui_touch_id: None,
                render_stats: started.render_stats,
                frame_index: 0,
                last_movement_update: None,
                audio,
                seed_reroll_state: initial_android_seed_reroll_state(scene_options.seed),
            })
        }

        fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
            self.depth.resize(device, width, height);
        }

        fn look_camera_by_pixels(
            &mut self,
            delta_x: f64,
            delta_y: f64,
            width: u32,
            height: u32,
        ) -> Result<()> {
            let scale = f64::from(width.min(height).max(1));
            let yaw_delta = -(delta_x / scale) * TOUCH_LOOK_RADIANS_PER_SCREEN;
            let pitch_delta = -(delta_y / scale) * TOUCH_LOOK_RADIANS_PER_SCREEN;
            self.camera.turn_mouse_delta(
                -yaw_delta / ENGINE_CAMERA_MOUSE_SENSITIVITY,
                -pitch_delta / ENGINE_CAMERA_MOUSE_SENSITIVITY,
            );
            self.commit_engine_camera_player_pose()
                .context("sync Android touch-look player pose")?;
            Ok(())
        }

        fn handle_touch(
            &mut self,
            touch: &Touch,
            width: u32,
            height: u32,
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            format: wgpu::TextureFormat,
        ) -> Result<AndroidUiTouchResult> {
            self.input_capabilities
                .note_activity(InputDeviceKind::Touch);
            let gui_scale = GuiScale::from_pixels(width.max(1), height.max(1));
            self.ui.set_scale(gui_scale);
            let point = gui_scale.client_to_gui(touch.location.x, touch.location.y);
            if self.ui.is_active() {
                self.touch_controls.clear();
                self.touch_menu_touch_id = None;
                self.touch_menu_pressed = false;
                return self.handle_active_ui_touch(touch, point, device, queue, format);
            }
            self.handle_closed_gameplay_touch(touch, point, gui_scale)
        }

        fn handle_keyboard_input(
            &mut self,
            key_code: KeyCode,
            state: ElementState,
            repeat: bool,
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            format: wgpu::TextureFormat,
        ) -> Result<AndroidUiTouchResult> {
            self.input_capabilities
                .note_activity(InputDeviceKind::Keyboard);
            if self.ui.is_active() {
                self.keyboard_mouse.clear_held();
                if state == ElementState::Pressed
                    && let Some(gui_key) = gui_key_from_key_code(key_code)
                {
                    let (handled, action) = self.ui.key_pressed(gui_key);
                    if let Some(action) = action {
                        return self.apply_ui_action(action, device, queue, format);
                    }
                    if handled {
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                }
                return Ok(AndroidUiTouchResult::default());
            }
            let Some(key) = android_keyboard_key_from_key_code(key_code) else {
                return Ok(AndroidUiTouchResult::default());
            };
            let event = self
                .keyboard_mouse
                .handle_key(key, state == ElementState::Pressed, repeat);
            if let Some(frame) = event.frame {
                return self.apply_flat_keyboard_mouse_frame(frame);
            }
            Ok(AndroidUiTouchResult {
                handled: event.handled,
                quit: false,
            })
        }

        fn handle_cursor_move(
            &mut self,
            position: PhysicalPosition<f64>,
            width: u32,
            height: u32,
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            format: wgpu::TextureFormat,
        ) -> Result<AndroidUiTouchResult> {
            self.input_capabilities
                .note_activity(InputDeviceKind::Mouse);
            let gui_scale = GuiScale::from_pixels(width.max(1), height.max(1));
            self.ui.set_scale(gui_scale);
            if !self.ui.is_active() {
                return Ok(AndroidUiTouchResult::default());
            }
            let point = gui_scale.client_to_gui(position.x, position.y);
            let ui_state = self.current_ui_render_state();
            let (handled, action) = self.ui.pointer_move(point, ui_state);
            if let Some(action) = action {
                return self.apply_ui_action(action, device, queue, format);
            }
            Ok(AndroidUiTouchResult {
                handled,
                quit: false,
            })
        }

        fn handle_mouse_input(
            &mut self,
            cursor: Option<PhysicalPosition<f64>>,
            state: ElementState,
            button: MouseButton,
            width: u32,
            height: u32,
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            format: wgpu::TextureFormat,
        ) -> Result<AndroidUiTouchResult> {
            self.input_capabilities
                .note_activity(InputDeviceKind::Mouse);
            if self.ui.is_active() {
                self.keyboard_mouse.clear_held();
                if button != MouseButton::Left {
                    return Ok(AndroidUiTouchResult::default());
                }
                let Some(cursor) = cursor else {
                    return Ok(AndroidUiTouchResult::default());
                };
                let gui_scale = GuiScale::from_pixels(width.max(1), height.max(1));
                self.ui.set_scale(gui_scale);
                let point = gui_scale.client_to_gui(cursor.x, cursor.y);
                let ui_state = self.current_ui_render_state();
                match state {
                    ElementState::Pressed => {
                        let handled = self.ui.pointer_down(point, ui_state);
                        Ok(AndroidUiTouchResult {
                            handled,
                            quit: false,
                        })
                    }
                    ElementState::Released => {
                        let (handled, action) = self.ui.pointer_up(point, ui_state);
                        if let Some(action) = action {
                            return self.apply_ui_action(action, device, queue, format);
                        }
                        Ok(AndroidUiTouchResult {
                            handled,
                            quit: false,
                        })
                    }
                }
            } else {
                let Some(button) = android_pointer_button_from_mouse_button(button) else {
                    return Ok(AndroidUiTouchResult::default());
                };
                let event = self
                    .keyboard_mouse
                    .handle_mouse_button(button, state == ElementState::Pressed);
                if let Some(frame) = event.frame {
                    return self.apply_flat_keyboard_mouse_frame(frame);
                }
                Ok(AndroidUiTouchResult {
                    handled: event.handled,
                    quit: false,
                })
            }
        }

        fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) -> Result<AndroidUiTouchResult> {
            self.input_capabilities
                .note_activity(InputDeviceKind::Mouse);
            if self.ui.is_active() {
                return Ok(AndroidUiTouchResult::default());
            }
            let Some(direction) = android_mouse_wheel_direction(delta) else {
                return Ok(AndroidUiTouchResult::default());
            };
            let event = self.keyboard_mouse.handle_mouse_wheel(direction);
            if let Some(frame) = event.frame {
                return self.apply_flat_keyboard_mouse_frame(frame);
            }
            Ok(AndroidUiTouchResult {
                handled: event.handled,
                quit: false,
            })
        }

        fn handle_mouse_motion(&mut self, delta: (f64, f64)) -> Result<bool> {
            self.input_capabilities
                .note_activity(InputDeviceKind::Mouse);
            if self.ui.is_active() {
                return Ok(false);
            }
            let Some(frame) = self
                .keyboard_mouse
                .mouse_motion_frame(delta.0 as f32, delta.1 as f32)
            else {
                return Ok(false);
            };
            self.apply_flat_look_frame(frame)
                .context("apply Android mouse-look frame")
        }

        fn handle_closed_gameplay_touch(
            &mut self,
            touch: &Touch,
            point: Point,
            gui_scale: GuiScale,
        ) -> Result<AndroidUiTouchResult> {
            match touch.phase {
                TouchPhase::Started => {
                    if touch_menu_button_rect().contains(point) {
                        self.touch_menu_touch_id = Some(touch.id);
                        self.touch_menu_pressed = true;
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    let buttons = touch_action_button_rects(gui_scale);
                    if buttons.jump.contains(point) {
                        self.touch_controls.jump_touch_id = Some(touch.id);
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if buttons.sprint.contains(point) {
                        self.touch_controls.sprint_touch_id = Some(touch.id);
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if buttons.descend.contains(point) {
                        self.touch_controls.descend_touch_id = Some(touch.id);
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if buttons.attack.contains(point) {
                        self.touch_controls.attack_touch_id = Some(touch.id);
                        self.apply_flat_touch_frame(flat_action_frame(FlatInputAction::Attack))
                            .context("apply Android touch attack")?;
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if buttons.use_item.contains(point) {
                        self.touch_controls.use_touch_id = Some(touch.id);
                        self.apply_flat_touch_frame(flat_action_frame(FlatInputAction::Use))
                            .context("apply Android touch use")?;
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if let Some((slot, _rect)) = touch_hotbar_slot_rects(gui_scale)
                        .into_iter()
                        .enumerate()
                        .find(|(_slot, rect)| rect.contains(point))
                    {
                        let slot = slot as u8;
                        self.touch_controls.hotbar_touch_id = Some(touch.id);
                        self.touch_controls.hotbar_pressed_slot = Some(slot);
                        let mut frame = FlatInputFrame::default();
                        frame.apply_intent(FlatInputIntent::SelectHotbarSlot(slot));
                        self.apply_flat_touch_frame(frame)
                            .context("apply Android touch hotbar selection")?;
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if self.touch_controls.movement.is_none()
                        && touch_movement_zone_rect(gui_scale).contains(point)
                    {
                        self.touch_controls.movement =
                            Some(AndroidMovementTouch::new(touch.id, point));
                        self.last_movement_update = Some(Instant::now());
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                }
                TouchPhase::Moved => {
                    if self.touch_menu_touch_id == Some(touch.id) {
                        self.touch_menu_pressed = touch_menu_button_rect().contains(point);
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if let Some(movement) = self.touch_controls.movement.as_mut()
                        && movement.id == touch.id
                    {
                        movement.move_to(point);
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if self.touch_controls.jump_touch_id == Some(touch.id)
                        || self.touch_controls.sprint_touch_id == Some(touch.id)
                        || self.touch_controls.descend_touch_id == Some(touch.id)
                        || self.touch_controls.attack_touch_id == Some(touch.id)
                        || self.touch_controls.use_touch_id == Some(touch.id)
                        || self.touch_controls.hotbar_touch_id == Some(touch.id)
                    {
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    if self.touch_menu_touch_id == Some(touch.id) {
                        let should_open =
                            touch.phase == TouchPhase::Ended && self.touch_menu_pressed;
                        self.touch_menu_touch_id = None;
                        self.touch_menu_pressed = false;
                        if should_open {
                            self.ui.open_pause();
                            self.clear_flat_gameplay_input();
                            self.ui.clear_input();
                            log::info!("Mclone Android shared menu opened");
                        }
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if self
                        .touch_controls
                        .movement
                        .is_some_and(|movement| movement.id == touch.id)
                    {
                        self.touch_controls.movement = None;
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if self.touch_controls.jump_touch_id == Some(touch.id) {
                        self.touch_controls.jump_touch_id = None;
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if self.touch_controls.sprint_touch_id == Some(touch.id) {
                        self.touch_controls.sprint_touch_id = None;
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if self.touch_controls.descend_touch_id == Some(touch.id) {
                        self.touch_controls.descend_touch_id = None;
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if self.touch_controls.attack_touch_id == Some(touch.id) {
                        self.touch_controls.attack_touch_id = None;
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if self.touch_controls.use_touch_id == Some(touch.id) {
                        self.touch_controls.use_touch_id = None;
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                    if self.touch_controls.hotbar_touch_id == Some(touch.id) {
                        self.touch_controls.hotbar_touch_id = None;
                        self.touch_controls.hotbar_pressed_slot = None;
                        return Ok(AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        });
                    }
                }
            }
            Ok(AndroidUiTouchResult::default())
        }

        fn handle_active_ui_touch(
            &mut self,
            touch: &Touch,
            point: Point,
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            format: wgpu::TextureFormat,
        ) -> Result<AndroidUiTouchResult> {
            let ui_state = self.current_ui_render_state();
            match touch.phase {
                TouchPhase::Started => {
                    if self.ui_touch_id.is_none() {
                        self.ui_touch_id = Some(touch.id);
                        self.ui.pointer_down(point, ui_state);
                    }
                    Ok(AndroidUiTouchResult {
                        handled: true,
                        quit: false,
                    })
                }
                TouchPhase::Moved => {
                    if self.ui_touch_id == Some(touch.id) {
                        let (_handled, action) = self.ui.pointer_move(point, ui_state);
                        if let Some(action) = action {
                            return self.apply_ui_action(action, device, queue, format);
                        }
                    }
                    Ok(AndroidUiTouchResult {
                        handled: true,
                        quit: false,
                    })
                }
                TouchPhase::Ended => {
                    if self.ui_touch_id == Some(touch.id) {
                        self.ui_touch_id = None;
                        let (_handled, action) = self.ui.pointer_up(point, ui_state);
                        if let Some(action) = action {
                            return self.apply_ui_action(action, device, queue, format);
                        }
                    }
                    Ok(AndroidUiTouchResult {
                        handled: true,
                        quit: false,
                    })
                }
                TouchPhase::Cancelled => {
                    if self.ui_touch_id == Some(touch.id) {
                        self.ui_touch_id = None;
                        self.ui.clear_input();
                    }
                    Ok(AndroidUiTouchResult {
                        handled: true,
                        quit: false,
                    })
                }
            }
        }

        fn next_new_world_seed(&mut self) -> i64 {
            self.seed_reroll_state = self
                .seed_reroll_state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.seed_reroll_state as i64
        }

        fn local_world_options(&self, seed: i64) -> AndroidSceneOptions {
            let mut options = self.scene_options.clone();
            options.seed = seed;
            options.render_distance = self.scene.render_distance();
            options.remote_addr = None;
            options
        }

        fn remote_session_options(&self, remote_addr: String) -> AndroidSceneOptions {
            let mut options = self.scene_options.clone();
            options.render_distance = self.scene.render_distance();
            options.remote_addr = Some(remote_addr);
            options
        }

        fn clear_touch_state(&mut self) {
            self.touch_menu_touch_id = None;
            self.touch_menu_pressed = false;
            self.touch_controls.clear();
            self.ui_touch_id = None;
            self.last_movement_update = None;
        }

        fn clear_keyboard_mouse_state(&mut self) {
            self.keyboard_mouse.clear_held();
            self.camera.clear_keys();
            self.last_movement_update = None;
        }

        fn clear_flat_gameplay_input(&mut self) {
            self.clear_touch_state();
            self.clear_keyboard_mouse_state();
        }

        fn start_replacement_session(
            &mut self,
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            format: wgpu::TextureFormat,
            request: SessionStartRequest,
            options: AndroidSceneOptions,
        ) -> Result<()> {
            self.session_status = StatusOverlay::new(request.starting_message(), true);
            let descriptor = request.active_descriptor().with_context(|| {
                format!(
                    "Android replacement request did not describe an active session: {request:?}"
                )
            })?;
            let started = match start_android_render_scene(device, queue, format, options.clone()) {
                Ok(started) => started,
                Err(error) => {
                    log::error!("failed to start Android session {request:?}: {error:#}");
                    self.session_status =
                        StatusOverlay::new(request.default_failure_message(), false);
                    return Err(error);
                }
            };

            self.scene_options = options;
            self.scene = started.scene;
            self.camera = started.camera;
            self.interaction = ClientInteractionController::new();
            self.draw = started.draw;
            self.render_stats = started.render_stats;
            self.frame_index = 0;
            self.clear_touch_state();
            self.clear_keyboard_mouse_state();
            self.session_status = StatusOverlay::hidden();
            match descriptor {
                ActiveSessionDescriptor::LocalWorld { seed } => {
                    self.ui.set_new_world_seed(seed);
                    log::info!("Mclone Android created local world seed={seed}");
                }
                ActiveSessionDescriptor::Remote { endpoint } => {
                    self.ui.set_join_remote_addr(endpoint.address.clone());
                    log::info!("Mclone Android joined remote session {}", endpoint.address);
                }
            }
            Ok(())
        }

        fn apply_ui_action(
            &mut self,
            action: GameUiAction,
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            format: wgpu::TextureFormat,
        ) -> Result<AndroidUiTouchResult> {
            let mut result = AndroidUiTouchResult {
                handled: true,
                quit: false,
            };
            match action {
                GameUiAction::ToggleSectionOcclusion => {
                    self.render_options.section_occlusion_culling =
                        !self.render_options.section_occlusion_culling;
                    log::info!(
                        "Mclone Android section occlusion culling {}",
                        if self.render_options.section_occlusion_culling {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    );
                }
                GameUiAction::ToggleFullbright => {
                    self.render_options.force_fullbright = !self.render_options.force_fullbright;
                    log::info!(
                        "Mclone Android fullbright {}",
                        if self.render_options.force_fullbright {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    );
                }
                GameUiAction::SetRenderDistance(render_distance) => {
                    let render_distance = render_distance
                        .clamp(ANDROID_MIN_RENDER_DISTANCE, ANDROID_MAX_RENDER_DISTANCE);
                    if self
                        .scene
                        .set_render_distance(render_distance as u32)
                        .with_context(|| {
                            format!("set Android render distance from menu to {render_distance}")
                        })?
                    {
                        log::info!(
                            "Mclone Android render distance set to {} (chunk tracking radius {})",
                            render_distance,
                            self.scene.chunk_tracking_radius()
                        );
                    }
                    self.scene_options.render_distance = render_distance as u32;
                }
                GameUiAction::ToggleFly => {
                    let movement_mode = self.camera.toggle_movement_mode();
                    log::info!(
                        "Mclone Android player movement mode {}",
                        movement_mode.label()
                    );
                }
                GameUiAction::SetFlySpeed(multiplier) => {
                    self.camera.set_fly_speed_multiplier(f64::from(multiplier));
                    log::info!(
                        "Mclone Android fly speed set to {:.1}x ({:.0} blocks/s)",
                        self.camera.fly_speed_multiplier(),
                        self.camera.speed_blocks_per_second()
                    );
                }
                GameUiAction::Quit => {
                    result.quit = true;
                }
                GameUiAction::OpenNewWorld => {
                    let seed = self.next_new_world_seed();
                    self.ui.set_new_world_seed(seed);
                    self.session_status = StatusOverlay::hidden();
                }
                GameUiAction::OpenJoinRemote => {
                    let remote_addr = self.scene_options.remote_addr.clone().unwrap_or_else(|| {
                        normalized_android_remote_addr(self.ui.join_remote_addr())
                    });
                    self.ui.set_join_remote_addr(remote_addr);
                    self.session_status = StatusOverlay::hidden();
                }
                GameUiAction::RerollSeed => {
                    let seed = self.next_new_world_seed();
                    self.ui.set_new_world_seed(seed);
                    self.session_status = StatusOverlay::hidden();
                    log::info!("Mclone Android new-world seed rerolled to {seed}");
                }
                GameUiAction::CreateWorld(seed) => {
                    let request = SessionStartRequest::NewLocalWorld { seed };
                    let options = self.local_world_options(seed);
                    if self
                        .start_replacement_session(device, queue, format, request, options)
                        .is_err()
                    {
                        self.ui.set_new_world_seed(seed);
                        return Ok(result);
                    }
                }
                GameUiAction::JoinRemote => {
                    let remote_addr = normalized_android_remote_addr(self.ui.join_remote_addr());
                    self.ui.set_join_remote_addr(remote_addr.clone());
                    let request = SessionStartRequest::JoinRemote {
                        endpoint: RemoteSessionEndpoint::new(remote_addr.clone()),
                    };
                    let options = self.remote_session_options(remote_addr);
                    if self
                        .start_replacement_session(device, queue, format, request, options)
                        .is_err()
                    {
                        return Ok(result);
                    }
                }
                GameUiAction::CycleFramePacing
                | GameUiAction::CycleFpsCap
                | GameUiAction::SetTouchLookSensitivity(_) => {}
                GameUiAction::BackToTitle | GameUiAction::QuitToTitle => {
                    self.session_status = StatusOverlay::hidden();
                }
                GameUiAction::StartWorld
                | GameUiAction::Resume
                | GameUiAction::OpenOptions(_)
                | GameUiAction::BackToPause => {}
            }
            self.ui.apply_action(action);
            if !self.ui.is_active() {
                self.ui.clear_input();
                self.ui_touch_id = None;
            }
            Ok(result)
        }

        fn current_ui_render_state(&self) -> GameUiRenderState {
            GameUiRenderState {
                render_distance: (self.scene.render_distance() as i32)
                    .clamp(ANDROID_MIN_RENDER_DISTANCE, ANDROID_MAX_RENDER_DISTANCE),
                min_render_distance: ANDROID_MIN_RENDER_DISTANCE,
                max_render_distance: ANDROID_MAX_RENDER_DISTANCE,
                section_occlusion_culling: self.render_options.section_occlusion_culling,
                force_fullbright: self.render_options.force_fullbright,
                fly_enabled: self.camera.movement_mode() == EngineCameraMovementMode::NoClip,
                fly_speed_multiplier: self.camera.fly_speed_multiplier() as f32,
                min_fly_speed_multiplier: ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER as f32,
                max_fly_speed_multiplier: ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER as f32,
                frame_pacing_mode: GameFramePacingMode::Vsync,
                fps_cap: ANDROID_FIXED_FPS_CAP,
                touch_settings: None,
            }
        }

        fn gui_draw_list(&self, gui_scale: GuiScale, ui_state: GameUiRenderState) -> GuiDrawList {
            if self.ui.is_active() {
                let mut draw = self.ui.render_draw_list(ui_state);
                let mut hud = FlatHud::new(self.input_capabilities.resolve(InputPreferences::AUTO));
                hud.world_hud_visible = false;
                hud.crosshair_visible = false;
                hud.status = self.session_status.clone();
                render_flat_hud(gui_scale, &mut draw, &hud);
                return draw;
            }
            let mut draw = GuiDrawList::new();
            let mut touch = self
                .touch_controls
                .overlay(self.interaction.selected_hotbar_slot());
            touch.menu_pressed = self.touch_menu_pressed;
            let mut hud = FlatHud::new(self.input_capabilities.resolve(InputPreferences::AUTO));
            hud.hotbar = FlatHotbarOverlay::selected(self.interaction.selected_hotbar_slot());
            hud.touch = touch;
            hud.status = self.session_status.clone();
            render_flat_hud(gui_scale, &mut draw, &hud);
            draw
        }

        fn apply_flat_touch_frame(&mut self, frame: FlatInputFrame) -> Result<()> {
            if let Some(slot) = frame.selected_hotbar_slot {
                self.select_hotbar_slot(slot);
            }
            if frame.hotbar_step != 0 {
                self.step_hotbar_slot(frame.hotbar_step);
            }
            if frame.attack {
                self.handle_world_flat_action(FlatInputAction::Attack)?;
            }
            if frame.use_item {
                self.handle_world_flat_action(FlatInputAction::Use)?;
            }
            Ok(())
        }

        fn apply_flat_keyboard_mouse_frame(
            &mut self,
            frame: FlatInputFrame,
        ) -> Result<AndroidUiTouchResult> {
            if frame.open_menu {
                self.ui.open_pause();
                self.clear_flat_gameplay_input();
                self.ui.clear_input();
                log::info!("Mclone Android shared menu opened from keyboard");
                return Ok(AndroidUiTouchResult {
                    handled: true,
                    quit: false,
                });
            }
            self.apply_flat_touch_frame(frame)?;
            Ok(AndroidUiTouchResult {
                handled: true,
                quit: false,
            })
        }

        fn apply_flat_look_frame(&mut self, frame: FlatInputFrame) -> Result<bool> {
            if frame.look_delta.x == 0.0 && frame.look_delta.y == 0.0 {
                return Ok(false);
            }
            self.camera
                .turn_mouse_delta(f64::from(frame.look_delta.x), f64::from(frame.look_delta.y));
            self.commit_engine_camera_player_pose()
                .context("sync Android mouse-look player pose")?;
            Ok(true)
        }

        fn select_hotbar_slot(&mut self, slot: u8) -> bool {
            if self.interaction.select_hotbar_slot(slot) {
                log::info!("Mclone Android selected hotbar slot {}", slot + 1);
                true
            } else {
                false
            }
        }

        fn step_hotbar_slot(&mut self, step: i8) -> bool {
            if step == 0 {
                return false;
            }
            let slot_count = i16::from(FLAT_HOTBAR_SLOT_COUNT);
            let slot = (i16::from(self.interaction.selected_hotbar_slot()) + i16::from(step))
                .rem_euclid(slot_count) as u8;
            self.select_hotbar_slot(slot)
        }

        fn sync_carried_item(&mut self) -> Result<bool> {
            let Some(command) = self.interaction.ensure_has_sent_carried_item() else {
                return Ok(false);
            };
            self.scene
                .send_gameplay_command(command)
                .context("failed to sync Android carried item to server")
        }

        fn handle_world_flat_action(&mut self, action: FlatInputAction) -> Result<()> {
            self.commit_engine_camera_player_pose()
                .context("sync Android player pose before interaction")?;
            self.sync_carried_item()?;
            let hit = self
                .camera
                .pick_block(self.scene.client(), &self.interaction);
            let command = match action {
                FlatInputAction::Attack => self.interaction.debug_instant_break_command(hit),
                FlatInputAction::Use => self.interaction.use_item_on_command(hit),
                _ => None,
            };
            let Some(command) = command else {
                return Ok(());
            };
            let changed = self
                .scene
                .send_gameplay_command(command)
                .context("failed to send Android touch interaction command")?;
            log::info!(
                "Mclone Android touch interaction {:?} at ({}, {}, {}) face={:?} changed={}",
                action,
                hit.block_pos.x,
                hit.block_pos.y,
                hit.block_pos.z,
                hit.direction,
                changed
            );
            Ok(())
        }

        fn apply_flat_movement(&mut self) -> Result<()> {
            let now = Instant::now();
            if self.ui.is_active() || !self.has_continuous_flat_movement_input() {
                self.last_movement_update = Some(now);
                return Ok(());
            }
            let dt_seconds = self
                .last_movement_update
                .replace(now)
                .map(|last| now.duration_since(last).as_secs_f64())
                .unwrap_or(0.0)
                .clamp(0.0, TOUCH_MOVEMENT_MAX_FRAME_SECONDS);
            let mut frame = FlatInputFrame::default();
            if let Some(keyboard_mouse_frame) = self.keyboard_mouse.held_frame() {
                merge_flat_input_frame(&mut frame, keyboard_mouse_frame);
            }
            if let Some(touch_frame) = self.touch_controls.held_frame() {
                merge_flat_input_frame(&mut frame, touch_frame);
            }
            let input = engine_camera_input_from_flat_frame(frame, dt_seconds);
            self.camera.apply_movement_input(self.scene.client(), input);
            self.play_landing_events();
            self.commit_engine_camera_player_pose()
                .context("sync Android flat-input player pose")?;
            Ok(())
        }

        fn has_continuous_flat_movement_input(&self) -> bool {
            self.touch_controls.has_continuous_movement_input()
                || self.keyboard_mouse.has_continuous_movement_input()
        }

        fn play_landing_events(&mut self) {
            let events = self.camera.take_landing_events();
            let Some(audio) = &self.audio else {
                return;
            };
            for event in events {
                let (sound, gain) = landing_playback_for_impact(event.impact_speed);
                audio.play(sound, gain);
            }
        }

        fn commit_engine_camera_player_pose(&mut self) -> Result<bool> {
            commit_engine_camera_player_pose(&mut self.scene, &mut self.camera)
        }

        fn apply_pending_engine_camera_position_updates(&mut self) -> Result<bool> {
            apply_pending_engine_camera_position_updates(&mut self.scene, &mut self.camera)
        }

        fn wants_continuous_redraw(&self) -> bool {
            !self.ui.is_active() && self.has_continuous_flat_movement_input()
        }

        fn render(&mut self, frame: RenderFrameContext<'_>) -> Result<()> {
            let _ = self.scene.poll()?;
            self.apply_pending_engine_camera_position_updates()
                .context("apply Android player position corrections")?;
            self.apply_flat_movement()
                .context("apply Android flat movement")?;
            let camera_position = camera_position(&self.camera);
            let section_update = self.scene.sync_render_sections(camera_position)?;
            if section_update.rebuilt_section_count() > 0
                || section_update.removed_section_count() > 0
            {
                let upload_report = self.draw.apply_section_updates(
                    frame.device,
                    &section_update.rebuilt_sections,
                    &section_update.removed_section_keys,
                )?;
                record_render_section_update_stats(
                    &mut self.render_stats,
                    &section_update,
                    upload_report,
                );
                self.render_stats.section_count = self.draw.section_count();
                self.render_stats.index_count = self.draw.index_count();
                self.render_stats.face_count =
                    quad_face_count_from_indices(self.render_stats.index_count);
            }
            self.draw.set_traversal_ready_sections(
                &self
                    .scene
                    .traversal_ready_render_section_keys(camera_position),
            );
            let time_of_day = self.scene.time_of_day();
            let sun_angle = self.scene.sun_angle();
            let render_view =
                chunk_camera_from_engine(self.camera.render_camera(self.scene.render_distance()))
                    .render_view(frame.target.size[0], frame.target.size[1]);
            let gui_scale = GuiScale::from_pixels(frame.target.size[0], frame.target.size[1]);
            self.ui.set_scale(gui_scale);
            let ui_state = self.current_ui_render_state();
            let ui_covers_world = self.ui.covers_world();
            let gui_draw = self.gui_draw_list(gui_scale, ui_state);
            let gui_active = !gui_draw.commands().is_empty();
            let summary = render_full_frame_for_view(
                frame,
                &self.depth,
                &self.sky,
                &mut self.draw,
                None,
                None,
                Some(&mut self.gui),
                render_view,
                &[],
                None,
                self.scene.sky_clear_color(),
                time_of_day,
                sun_angle,
                self.render_options,
                FullFrameGui::new(
                    gui_active,
                    ui_covers_world,
                    [gui_scale.width, gui_scale.height],
                ),
                |_| gui_draw,
                &mut self.render_stats,
            )?;
            if self.frame_index == 0 {
                log::info!(
                    "Mclone Android rendered {} frame: sections={}/{} indices={}/{} gui_commands={}",
                    self.scene.host_label(),
                    summary.drawn_section_count,
                    summary.section_count,
                    summary.drawn_index_count,
                    summary.index_count,
                    summary.gui_command_count
                );
            }
            self.frame_index += 1;
            Ok(())
        }
    }

    fn start_android_render_scene(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        options: AndroidSceneOptions,
    ) -> Result<StartedAndroidRenderScene> {
        let mut scene = android_single_view_scene_runtime(options)?;
        let host_label = scene.host_label();
        let session_label = active_session_label(scene.active_session());
        let mut camera = EngineCameraController::spawn_for_chunk(scene.interest_center());
        let (poll_count, poll_ms) = scene.poll_until_idle()?;
        log::info!(
            "Mclone Android {host_label} runtime idle: session={} polls={} poll_ms={:.1} loaded_chunks={}",
            session_label,
            poll_count,
            poll_ms,
            scene.loaded_chunk_count()
        );
        if commit_engine_camera_player_pose(&mut scene, &mut camera)? {
            let (pose_poll_count, pose_poll_ms) = scene.poll_until_idle()?;
            log::info!(
                "Mclone Android {host_label} player pose synced: polls={} poll_ms={:.1}",
                pose_poll_count,
                pose_poll_ms
            );
            apply_pending_engine_camera_position_updates(&mut scene, &mut camera)?;
        }
        let camera_position = camera_position(&camera);
        let section_update = scene.sync_all_render_sections(camera_position)?;
        let sections = scene.cached_sections();
        if sections.is_empty() {
            bail!(
                "Android {host_label} runtime produced no render sections: loaded_chunks={} rebuilt_sections={} submitted_compile_sections={} completed_compile_sections={} pending_compile_jobs={}",
                scene.loaded_chunk_count(),
                section_update.rebuilt_section_count(),
                section_update.submitted_compile_section_count,
                section_update.completed_compile_section_count,
                section_update.pending_compile_jobs
            );
        }

        let upload_report = TexturedSectionUploadReport {
            uploaded_section_count: section_update.rebuilt_section_count(),
            removed_section_count: section_update.removed_section_count(),
            uploaded_vertex_count: section_update.rebuilt_vertex_count,
            uploaded_index_count: section_update.rebuilt_index_count,
        };
        let mut draw = TexturedSectionDrawResources::new(
            device,
            queue,
            format,
            &sections,
            scene.mesh_assets().atlas.as_upload(),
        )?;
        draw.set_traversal_ready_sections(
            &scene.traversal_ready_render_section_keys(camera_position),
        );

        let index_count = draw.index_count();
        let mut render_stats = RenderStreamStats {
            section_count: draw.section_count(),
            face_count: quad_face_count_from_indices(index_count),
            index_count,
            ..RenderStreamStats::default()
        };
        record_render_section_update_stats(&mut render_stats, &section_update, upload_report);
        log::info!(
            "Mclone Android uploaded {host_label} render sections: sections={} faces={} indices={} rebuilt_sections={} visibility_graph_count={}",
            render_stats.section_count,
            render_stats.face_count,
            render_stats.index_count,
            section_update.rebuilt_section_count(),
            section_update.visibility_graph_stats.build_count
        );

        Ok(StartedAndroidRenderScene {
            scene,
            camera,
            draw,
            render_stats,
        })
    }

    #[derive(Clone, Debug)]
    struct AndroidSceneOptions {
        seed: i64,
        center: ChunkPos,
        render_distance: u32,
        day_time_override: Option<u64>,
        freeze_time: bool,
        remote_addr: Option<String>,
    }

    impl AndroidSceneOptions {
        fn local_options(&self) -> LocalSingleViewSceneOptions {
            LocalSingleViewSceneOptions::new(self.seed, self.center, self.render_distance)
                .with_day_time(self.day_time_override)
                .with_freeze_time(self.freeze_time)
        }

        fn host_options(&self) -> SingleViewHostOptions {
            SingleViewHostOptions::new(self.center, self.render_distance)
        }
    }

    fn android_game_ui_for_scene(options: &AndroidSceneOptions) -> GameUi {
        let mut ui = GameUi::new_ingame();
        ui.set_new_world_seed(options.seed);
        ui.set_join_remote_addr(
            options
                .remote_addr
                .clone()
                .unwrap_or_else(|| DEFAULT_JOIN_REMOTE_ADDR.to_owned()),
        );
        ui
    }

    fn normalized_android_remote_addr(addr: &str) -> String {
        let addr = addr.trim();
        if addr.is_empty() {
            DEFAULT_JOIN_REMOTE_ADDR.to_owned()
        } else {
            addr.to_owned()
        }
    }

    fn initial_android_seed_reroll_state(seed: i64) -> u64 {
        (seed as u64)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0xD1B5_4A32_D192_ED03)
            .max(1)
    }

    fn android_scene_options() -> AndroidSceneOptions {
        let remote_addr = android_remote_addr();
        if let Some(remote_addr) = &remote_addr {
            log::info!(
                "Mclone Android remote dedicated address from {REMOTE_ADDR_PROPERTY}: {remote_addr}"
            );
        } else {
            log::info!(
                "Mclone Android remote dedicated address from {REMOTE_ADDR_PROPERTY}: <none>"
            );
        }
        AndroidSceneOptions {
            seed: 12345,
            center: ChunkPos::new(0, 0),
            render_distance: 2,
            day_time_override: Some(6000),
            freeze_time: true,
            remote_addr,
        }
    }

    fn android_remote_addr() -> Option<String> {
        android_property(REMOTE_ADDR_PROPERTY)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty() && value != REMOTE_ADDR_NONE_SENTINEL)
    }

    #[allow(unsafe_code)]
    fn android_property(name: &str) -> Option<String> {
        let name = CString::new(name).ok()?;
        let mut value = [0 as c_char; ANDROID_PROPERTY_VALUE_MAX];
        let len = unsafe { __system_property_get(name.as_ptr(), value.as_mut_ptr()) };
        if len <= 0 {
            return None;
        }
        Some(
            unsafe { CStr::from_ptr(value.as_ptr()) }
                .to_string_lossy()
                .into_owned(),
        )
    }

    type AndroidSingleViewSceneRuntime = NativeSingleViewSessionRuntime<AndroidRemoteServerSession>;

    fn android_single_view_scene_runtime(
        options: AndroidSceneOptions,
    ) -> Result<AndroidSingleViewSceneRuntime> {
        if let Some(remote_addr) = &options.remote_addr {
            let session = AndroidRemoteServerSession::connect(remote_addr.as_str())?;
            return AndroidSingleViewSceneRuntime::remote_dedicated(
                RemoteSessionEndpoint::new(remote_addr.clone()),
                options.host_options(),
                session,
            )
            .with_context(|| {
                format!("failed to initialize Android remote dedicated runtime from {remote_addr}")
            });
        }
        AndroidSingleViewSceneRuntime::local(options.local_options())
    }

    fn active_session_label(session: Option<&ActiveSessionDescriptor>) -> String {
        match session {
            Some(ActiveSessionDescriptor::LocalWorld { seed }) => format!("local-world:{seed}"),
            Some(ActiveSessionDescriptor::Remote { endpoint }) => {
                format!("remote:{}", endpoint.address)
            }
            None => "none".to_owned(),
        }
    }

    #[derive(Debug)]
    struct AndroidRemoteServerSession {
        addr: String,
        session: NativeClientSession,
    }

    impl AndroidRemoteServerSession {
        fn connect(addr: impl Into<String>) -> Result<Self> {
            let addr = addr.into();
            let session = NativeClientSession::connect(addr.as_str())
                .with_context(|| format!("failed to connect to Android remote server {addr}"))?;
            Ok(Self { addr, session })
        }
    }

    impl RemoteDedicatedServerSession for AndroidRemoteServerSession {
        fn send_command(&mut self, command: ClientCommand) -> Result<Vec<ServerUpdate>> {
            self.session.send_command(&command).with_context(|| {
                format!(
                    "failed to exchange command with Android remote server {}",
                    self.addr
                )
            })
        }

        fn reconnect(&mut self) -> Result<()> {
            self.session = NativeClientSession::connect(self.addr.as_str()).with_context(|| {
                format!("failed to reconnect to Android remote server {}", self.addr)
            })?;
            Ok(())
        }
    }

    #[derive(Default)]
    struct McloneAndroidApp {
        window: Option<Arc<Window>>,
        gpu: Option<AndroidGpuState>,
        active_touch: Option<ActiveTouch>,
        last_cursor: Option<PhysicalPosition<f64>>,
    }

    #[derive(Clone, Copy, Debug)]
    struct ActiveTouch {
        id: u64,
        position: PhysicalPosition<f64>,
    }

    impl ApplicationHandler for McloneAndroidApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_none() {
                let attributes = WindowAttributes::default().with_title("Mclone");
                match event_loop.create_window(attributes) {
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
                    match AndroidGpuState::new(window.clone()) {
                        Ok(gpu) => self.gpu = Some(gpu),
                        Err(error) => {
                            log::error!("{error}");
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
            self.gpu = None;
            self.window = None;
            self.active_touch = None;
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
                        Ok(()) => {
                            if gpu.wants_continuous_redraw() {
                                window.request_redraw();
                            }
                        }
                        Err(AndroidRenderError::Surface(wgpu::SurfaceError::OutOfMemory)) => {
                            event_loop.exit()
                        }
                        Err(AndroidRenderError::Surface(wgpu::SurfaceError::Timeout)) => {
                            window.request_redraw()
                        }
                        Err(AndroidRenderError::Surface(
                            wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated,
                        )) => {
                            gpu.resize(window.inner_size());
                            window.request_redraw();
                        }
                        Err(AndroidRenderError::Surface(wgpu::SurfaceError::Other)) => {
                            window.request_redraw()
                        }
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
                WindowEvent::Touch(touch) => self.handle_touch(event_loop, &window, touch),
                WindowEvent::KeyboardInput { event, .. } => {
                    let PhysicalKey::Code(key_code) = event.physical_key else {
                        return;
                    };
                    let Some(gpu) = self.gpu.as_mut() else {
                        return;
                    };
                    let result = gpu.handle_keyboard_input(key_code, event.state, event.repeat);
                    self.handle_gpu_input_result(event_loop, &window, "keyboard input", result);
                }
                WindowEvent::CursorMoved { position, .. } => {
                    self.last_cursor = Some(position);
                    let Some(gpu) = self.gpu.as_mut() else {
                        return;
                    };
                    let result = gpu.handle_cursor_move(position);
                    self.handle_gpu_input_result(event_loop, &window, "cursor move", result);
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    let Some(gpu) = self.gpu.as_mut() else {
                        return;
                    };
                    let result = gpu.handle_mouse_input(self.last_cursor, state, button);
                    self.handle_gpu_input_result(event_loop, &window, "mouse input", result);
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let Some(gpu) = self.gpu.as_mut() else {
                        return;
                    };
                    let result = gpu.handle_mouse_wheel(delta);
                    self.handle_gpu_input_result(event_loop, &window, "mouse wheel", result);
                }
                WindowEvent::Focused(false) => {
                    self.active_touch = None;
                    self.last_cursor = None;
                    if let Some(gpu) = self.gpu.as_mut() {
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
            let Some(gpu) = self.gpu.as_mut() else {
                return;
            };
            match gpu.handle_mouse_motion(delta) {
                Ok(true) => {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
                Ok(false) => {}
                Err(error) => {
                    log::error!("failed to handle Mclone Android mouse look: {error:#}");
                    event_loop.exit();
                }
            }
        }
    }

    impl McloneAndroidApp {
        fn handle_gpu_input_result(
            &mut self,
            event_loop: &ActiveEventLoop,
            window: &Window,
            context: &str,
            result: Result<AndroidUiTouchResult>,
        ) {
            match result {
                Ok(result) if result.handled => {
                    if result.quit {
                        event_loop.exit();
                    } else {
                        window.request_redraw();
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    log::error!("failed to handle Mclone Android {context}: {error:#}");
                    event_loop.exit();
                }
            }
        }

        fn handle_touch(&mut self, event_loop: &ActiveEventLoop, window: &Window, touch: Touch) {
            if let Some(gpu) = self.gpu.as_mut() {
                match gpu.handle_ui_touch(&touch) {
                    Ok(result) if result.handled => {
                        if result.quit {
                            event_loop.exit();
                            return;
                        }
                        window.request_redraw();
                        return;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        log::error!("failed to handle Mclone Android UI touch: {error:#}");
                        event_loop.exit();
                        return;
                    }
                }
            }
            match touch.phase {
                TouchPhase::Started => {
                    if self.active_touch.is_none() {
                        log::info!(
                            "Mclone Android touch look started: id={} x={:.1} y={:.1}",
                            touch.id,
                            touch.location.x,
                            touch.location.y
                        );
                        self.active_touch = Some(ActiveTouch {
                            id: touch.id,
                            position: touch.location,
                        });
                    }
                }
                TouchPhase::Moved => {
                    let Some(active) = self.active_touch.as_mut() else {
                        return;
                    };
                    if active.id != touch.id {
                        return;
                    }
                    let delta_x = touch.location.x - active.position.x;
                    let delta_y = touch.location.y - active.position.y;
                    active.position = touch.location;
                    if delta_x.abs() < 0.5 && delta_y.abs() < 0.5 {
                        return;
                    }
                    if let Some(gpu) = self.gpu.as_mut() {
                        if let Err(error) = gpu.look_camera_by_pixels(delta_x, delta_y) {
                            log::error!("failed to handle Mclone Android touch look: {error:#}");
                            event_loop.exit();
                            return;
                        }
                        log::info!(
                            "Mclone Android touch look moved: dx={:.1} dy={:.1}",
                            delta_x,
                            delta_y
                        );
                        window.request_redraw();
                    }
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    if self
                        .active_touch
                        .is_some_and(|active| active.id == touch.id)
                    {
                        log::info!("Mclone Android touch look ended: id={}", touch.id);
                        self.active_touch = None;
                    }
                }
            }
        }
    }

    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    fn android_main(app: AndroidApp) {
        init_android_logger();
        configure_android_asset_root(&app);
        log::info!("Mclone Android starting");
        let event_loop = EventLoop::builder()
            .with_android_app(app)
            .build()
            .expect("create Android event loop");
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut state = McloneAndroidApp::default();
        event_loop
            .run_app(&mut state)
            .expect("run Android event loop");
    }

    fn commit_engine_camera_player_pose(
        scene: &mut AndroidSingleViewSceneRuntime,
        camera: &mut EngineCameraController,
    ) -> Result<bool> {
        let server_changed = sync_engine_camera_player_pose(scene, camera)?;
        let interest_changed = update_interest_from_engine_camera(scene, camera)?;
        Ok(server_changed || interest_changed)
    }

    fn sync_engine_camera_player_pose(
        scene: &mut AndroidSingleViewSceneRuntime,
        camera: &mut EngineCameraController,
    ) -> Result<bool> {
        let changed = if let Some(report) = camera.next_pose_sync_command() {
            scene
                .send_gameplay_command(report.command)
                .context("failed to sync Android player pose to server")?
        } else {
            false
        };
        Ok(changed || apply_pending_engine_camera_position_updates(scene, camera)?)
    }

    fn apply_pending_engine_camera_position_updates(
        scene: &mut AndroidSingleViewSceneRuntime,
        camera: &mut EngineCameraController,
    ) -> Result<bool> {
        let mut changed = false;
        for update in scene.drain_player_position_updates() {
            let accepted = camera.accept_position_update(update);
            scene
                .send_gameplay_command(accepted.accept_command)
                .context("failed to acknowledge Android player position correction")?;
            let resync = camera.corrected_pose_sync_command();
            scene
                .send_gameplay_command(resync.command)
                .context("failed to sync corrected Android player pose")?;
            log::warn!(
                "accepted Android player position correction id={} feet=({:.2}, {:.2}, {:.2})",
                accepted.update.teleport_id,
                accepted.feet_position.x,
                accepted.feet_position.y,
                accepted.feet_position.z
            );
            changed = true;
        }
        if changed {
            changed |= update_interest_from_engine_camera(scene, camera)?;
        }
        Ok(changed)
    }

    fn update_interest_from_engine_camera(
        scene: &mut AndroidSingleViewSceneRuntime,
        camera: &EngineCameraController,
    ) -> Result<bool> {
        let snapshot = camera.snapshot();
        let center = snapshot.chunk_pos;
        if scene.set_interest_center(center)? {
            log::info!(
                "Mclone Android chunk interest moved to ({}, {}) at camera position ({:.1}, {:.1}, {:.1})",
                center.x,
                center.z,
                snapshot.eye.x,
                snapshot.eye.y,
                snapshot.eye.z
            );
            return Ok(true);
        }
        Ok(false)
    }

    fn gui_key_from_key_code(key_code: KeyCode) -> Option<GuiKey> {
        match key_code {
            KeyCode::Escape => Some(GuiKey::Escape),
            _ => None,
        }
    }

    fn android_keyboard_key_from_key_code(key_code: KeyCode) -> Option<KeyboardKey> {
        match key_code {
            KeyCode::KeyW => Some(KeyboardKey::KeyW),
            KeyCode::KeyA => Some(KeyboardKey::KeyA),
            KeyCode::KeyS => Some(KeyboardKey::KeyS),
            KeyCode::KeyD => Some(KeyboardKey::KeyD),
            KeyCode::KeyX => Some(KeyboardKey::KeyX),
            KeyCode::Space => Some(KeyboardKey::Space),
            KeyCode::ShiftLeft => Some(KeyboardKey::ShiftLeft),
            KeyCode::ShiftRight => Some(KeyboardKey::ShiftRight),
            KeyCode::ControlLeft => Some(KeyboardKey::ControlLeft),
            KeyCode::ControlRight => Some(KeyboardKey::ControlRight),
            KeyCode::Escape => Some(KeyboardKey::Escape),
            KeyCode::Digit1 => Some(KeyboardKey::Digit1),
            KeyCode::Digit2 => Some(KeyboardKey::Digit2),
            KeyCode::Digit3 => Some(KeyboardKey::Digit3),
            KeyCode::Digit4 => Some(KeyboardKey::Digit4),
            KeyCode::Digit5 => Some(KeyboardKey::Digit5),
            KeyCode::Digit6 => Some(KeyboardKey::Digit6),
            KeyCode::Digit7 => Some(KeyboardKey::Digit7),
            KeyCode::Digit8 => Some(KeyboardKey::Digit8),
            KeyCode::Digit9 => Some(KeyboardKey::Digit9),
            _ => None,
        }
    }

    fn android_pointer_button_from_mouse_button(button: MouseButton) -> Option<PointerButton> {
        match button {
            MouseButton::Left => Some(PointerButton::Primary),
            MouseButton::Right => Some(PointerButton::Secondary),
            MouseButton::Middle => Some(PointerButton::Middle),
            _ => None,
        }
    }

    fn android_mouse_wheel_direction(delta: MouseScrollDelta) -> Option<MouseWheelDirection> {
        let y = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(position) => position.y as f32,
        };
        if !y.is_finite() || y.abs() <= f32::EPSILON {
            return None;
        }
        Some(if y > 0.0 {
            MouseWheelDirection::Up
        } else {
            MouseWheelDirection::Down
        })
    }

    fn flat_action_frame(action: FlatInputAction) -> FlatInputFrame {
        let mut frame = FlatInputFrame::default();
        frame.apply_intent(FlatInputIntent::Action {
            action,
            pressed: true,
        });
        frame
    }

    fn merge_flat_input_frame(target: &mut FlatInputFrame, source: FlatInputFrame) {
        if source.forward {
            target.apply_intent(FlatInputIntent::MoveDirection {
                direction: MovementDirection::Forward,
                pressed: true,
            });
        }
        if source.backward {
            target.apply_intent(FlatInputIntent::MoveDirection {
                direction: MovementDirection::Backward,
                pressed: true,
            });
        }
        if source.left {
            target.apply_intent(FlatInputIntent::MoveDirection {
                direction: MovementDirection::Left,
                pressed: true,
            });
        }
        if source.right {
            target.apply_intent(FlatInputIntent::MoveDirection {
                direction: MovementDirection::Right,
                pressed: true,
            });
        }
        if let Some(movement) = source.analog_movement {
            target.apply_intent(FlatInputIntent::MoveAnalog {
                left: movement.left,
                forward: movement.forward,
            });
        }
        for (pressed, action) in [
            (source.jump, FlatInputAction::Jump),
            (source.sprint, FlatInputAction::Sprint),
            (source.sneak, FlatInputAction::Sneak),
            (source.descend, FlatInputAction::Descend),
            (source.attack, FlatInputAction::Attack),
            (source.use_item, FlatInputAction::Use),
            (source.open_menu, FlatInputAction::OpenMenu),
        ] {
            if pressed {
                target.apply_intent(FlatInputIntent::Action {
                    action,
                    pressed: true,
                });
            }
        }
        if let Some(slot) = source.selected_hotbar_slot {
            target.apply_intent(FlatInputIntent::SelectHotbarSlot(slot));
        }
        if source.hotbar_step != 0 {
            target.apply_intent(FlatInputIntent::StepHotbar(source.hotbar_step));
        }
    }

    fn engine_camera_input_from_flat_frame(
        frame: FlatInputFrame,
        dt_seconds: f64,
    ) -> EngineCameraInput {
        EngineCameraInput {
            dt_seconds,
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
            ..EngineCameraInput::default()
        }
    }

    fn camera_position(camera: &EngineCameraController) -> Vec3 {
        glam_vec3_from_vec3d(camera.snapshot().eye)
    }

    fn glam_vec3_from_vec3d(value: Vec3d) -> Vec3 {
        Vec3::new(value.x as f32, value.y as f32, value.z as f32)
    }

    fn chunk_camera_from_engine(camera: EngineRenderCamera) -> ChunkCamera {
        ChunkCamera {
            eye: camera.eye,
            target: camera.target,
            up: camera.up,
            fov_y_radians: camera.fov_y_radians,
            z_near: camera.z_near,
            z_far: camera.z_far,
        }
    }
}

#[cfg(not(target_os = "android"))]
pub fn host_placeholder() {}

#[cfg(target_os = "android")]
fn preferred_surface_format(caps: &wgpu::SurfaceCapabilities) -> wgpu::TextureFormat {
    caps.formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .unwrap_or(caps.formats[0])
}
