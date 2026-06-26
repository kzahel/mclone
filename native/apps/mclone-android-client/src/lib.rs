#![deny(unsafe_code)]

#[cfg(target_os = "android")]
mod android {
    use std::ffi::{CStr, CString, c_char, c_int};
    use std::sync::Arc;
    use std::sync::Once;

    use anyhow::{Context, Result, bail};
    use glam::Vec3;
    use mclone_app_runtime::frame_render::{
        FullFrameGui, RenderStreamStats, record_render_section_update_stats,
        render_full_frame_for_view,
    };
    use mclone_app_runtime::host_mode::{RemoteDedicatedServerSession, SingleViewHostOptions};
    use mclone_app_runtime::local_single_view::{
        LocalSingleViewSceneOptions, NativeSingleViewSceneRuntime,
    };
    use mclone_core::ChunkPos;
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
    use mclone_ui::{
        GameFramePacingMode, GameUi, GameUiAction, GameUiRenderState, GuiDrawList, GuiScale, Point,
        TouchOverlay, render_touch_overlay, touch_menu_button_rect,
    };
    use winit::application::ApplicationHandler;
    use winit::dpi::PhysicalPosition;
    use winit::event::{Touch, TouchPhase, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::platform::android::EventLoopBuilderExtAndroid;
    use winit::platform::android::activity::AndroidApp;
    use winit::window::{Window, WindowAttributes, WindowId};

    const LOG_TAG: &str = "mclone_android";
    const REMOTE_ADDR_PROPERTY: &str = "debug.mclone.remote_addr";
    const ANDROID_PROPERTY_VALUE_MAX: usize = 92;
    const TOUCH_ORBIT_RADIANS_PER_SCREEN: f32 = 2.4;
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
        scene: AndroidSingleViewSceneRuntime,
        camera: ChunkCamera,
        render_options: TexturedSectionRenderOptions,
        ui: GameUi,
        touch_menu_touch_id: Option<u64>,
        touch_menu_pressed: bool,
        ui_touch_id: Option<u64>,
        render_stats: RenderStreamStats,
        frame_index: u64,
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

        fn orbit_camera_by_pixels(&mut self, delta_x: f64, delta_y: f64) {
            self.renderer.orbit_camera_by_pixels(
                delta_x,
                delta_y,
                self.config.width,
                self.config.height,
            );
        }

        fn handle_ui_touch(&mut self, touch: &Touch) -> Result<AndroidUiTouchResult> {
            self.renderer
                .handle_touch(touch, self.config.width, self.config.height)
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
            let mut scene = android_single_view_scene_runtime(scene_options)?;
            let host_label = scene.host_label();
            let camera = android_smoke_camera(scene.render_distance());
            let camera_position = camera_position(camera);
            let (poll_count, poll_ms) = scene.poll_until_idle()?;
            log::info!(
                "Mclone Android {host_label} runtime idle: polls={} poll_ms={:.1} loaded_chunks={}",
                poll_count,
                poll_ms,
                scene.loaded_chunk_count()
            );
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

            let depth = ChunkDepthTarget::new(device, width, height);
            let sky = SkyRenderer::new(device, format);
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

            Ok(Self {
                depth,
                sky,
                draw,
                gui: GuiRenderer::new(device, format),
                scene,
                camera,
                render_options: TexturedSectionRenderOptions::default(),
                ui: GameUi::new_ingame(),
                touch_menu_touch_id: None,
                touch_menu_pressed: false,
                ui_touch_id: None,
                render_stats,
                frame_index: 0,
            })
        }

        fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
            self.depth.resize(device, width, height);
        }

        fn orbit_camera_by_pixels(&mut self, delta_x: f64, delta_y: f64, width: u32, height: u32) {
            let scale = width.min(height).max(1) as f32;
            let yaw_delta = -(delta_x as f32 / scale) * TOUCH_ORBIT_RADIANS_PER_SCREEN;
            let pitch_delta = -(delta_y as f32 / scale) * TOUCH_ORBIT_RADIANS_PER_SCREEN;
            self.camera.orbit(yaw_delta, pitch_delta);
        }

        fn handle_touch(
            &mut self,
            touch: &Touch,
            width: u32,
            height: u32,
        ) -> Result<AndroidUiTouchResult> {
            let gui_scale = GuiScale::from_pixels(width.max(1), height.max(1));
            self.ui.set_scale(gui_scale);
            let point = gui_scale.client_to_gui(touch.location.x, touch.location.y);
            if self.ui.is_active() {
                return self.handle_active_ui_touch(touch, point);
            }
            Ok(self.handle_menu_button_touch(touch, point))
        }

        fn handle_menu_button_touch(
            &mut self,
            touch: &Touch,
            point: Point,
        ) -> AndroidUiTouchResult {
            match touch.phase {
                TouchPhase::Started => {
                    if touch_menu_button_rect().contains(point) {
                        self.touch_menu_touch_id = Some(touch.id);
                        self.touch_menu_pressed = true;
                        return AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        };
                    }
                }
                TouchPhase::Moved => {
                    if self.touch_menu_touch_id == Some(touch.id) {
                        self.touch_menu_pressed = touch_menu_button_rect().contains(point);
                        return AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        };
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
                            self.ui.clear_input();
                            log::info!("Mclone Android shared menu opened");
                        }
                        return AndroidUiTouchResult {
                            handled: true,
                            quit: false,
                        };
                    }
                }
            }
            AndroidUiTouchResult::default()
        }

        fn handle_active_ui_touch(
            &mut self,
            touch: &Touch,
            point: Point,
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
                            return self.apply_ui_action(action);
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
                            return self.apply_ui_action(action);
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

        fn apply_ui_action(&mut self, action: GameUiAction) -> Result<AndroidUiTouchResult> {
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
                }
                GameUiAction::Quit => {
                    result.quit = true;
                }
                GameUiAction::CycleFramePacing
                | GameUiAction::CycleFpsCap
                | GameUiAction::SetTouchLookSensitivity(_) => {}
                GameUiAction::StartWorld
                | GameUiAction::Resume
                | GameUiAction::OpenOptions(_)
                | GameUiAction::BackToTitle
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
                frame_pacing_mode: GameFramePacingMode::Vsync,
                fps_cap: ANDROID_FIXED_FPS_CAP,
                touch_settings: None,
            }
        }

        fn gui_draw_list(&self, gui_scale: GuiScale, ui_state: GameUiRenderState) -> GuiDrawList {
            if self.ui.is_active() {
                return self.ui.render_draw_list(ui_state);
            }
            let mut draw = GuiDrawList::new();
            render_touch_overlay(
                gui_scale,
                &mut draw,
                &TouchOverlay {
                    visible: true,
                    menu_pressed: self.touch_menu_pressed,
                    ..TouchOverlay::hidden()
                },
            );
            draw
        }

        fn render(&mut self, frame: RenderFrameContext<'_>) -> Result<()> {
            let camera_position = camera_position(self.camera);
            let _ = self.scene.poll()?;
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
            let render_view = self
                .camera
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
            .filter(|value| !value.is_empty())
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

    type AndroidSingleViewSceneRuntime = NativeSingleViewSceneRuntime<AndroidRemoteServerSession>;

    fn android_single_view_scene_runtime(
        options: AndroidSceneOptions,
    ) -> Result<AndroidSingleViewSceneRuntime> {
        if let Some(remote_addr) = &options.remote_addr {
            let session = AndroidRemoteServerSession::connect(remote_addr.as_str())?;
            return AndroidSingleViewSceneRuntime::remote_dedicated(
                options.host_options(),
                session,
            )
            .with_context(|| {
                format!("failed to initialize Android remote dedicated runtime from {remote_addr}")
            });
        }
        AndroidSingleViewSceneRuntime::local(options.local_options())
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
                window.request_redraw();
            }
        }

        fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
            log::info!("Mclone Android suspended");
            self.gpu = None;
            self.window = None;
            self.active_touch = None;
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
                        Ok(()) => {}
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
                _ => {}
            }
        }
    }

    impl McloneAndroidApp {
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
                            "Mclone Android touch orbit started: id={} x={:.1} y={:.1}",
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
                        gpu.orbit_camera_by_pixels(delta_x, delta_y);
                        log::info!(
                            "Mclone Android touch orbit moved: dx={:.1} dy={:.1}",
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
                        log::info!("Mclone Android touch orbit ended: id={}", touch.id);
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

    fn android_smoke_camera(render_distance: u32) -> ChunkCamera {
        ChunkCamera::overview_for_chunk_area(0, 0, render_distance as i32)
    }

    fn camera_position(camera: ChunkCamera) -> Vec3 {
        Vec3::from_array(camera.eye)
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
