#![deny(unsafe_code)]

#[cfg(target_os = "android")]
mod android {
    use std::ffi::{CString, c_char, c_int};
    use std::sync::Arc;
    use std::sync::Once;
    use std::thread;
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result, bail};
    use glam::Vec3;
    use mclone_app_runtime::frame_render::{
        FullFrameGui, RenderStreamStats, record_render_section_update_stats,
        render_full_frame_for_view,
    };
    use mclone_app_runtime::render_assets::{
        RenderSectionCompileWorker, TexturedMeshAssets, load_textured_mesh_assets,
    };
    use mclone_app_runtime::{
        RuntimeUpdateApplyReport, SingleViewRuntime, chunk_tracking_radius_for_render_distance,
        elapsed_ms,
    };
    use mclone_core::ChunkPos;
    use mclone_mesh::quad_face_count_from_indices;
    use mclone_render::chunk::{
        ChunkCamera, ChunkDepthTarget, TexturedSectionDrawResources, TexturedSectionRenderOptions,
        TexturedSectionUploadReport,
    };
    use mclone_render::sky_render::SkyRenderer;
    use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
    use mclone_render_session::RenderSectionCacheUpdate;
    use mclone_server::{
        IntegratedServerRunner, NativeIntegratedServerRunner, NativeIntegratedServerRunnerConfig,
        ServerRunnerDiagnostics,
    };
    use mclone_ui::GuiDrawList;
    use winit::application::ApplicationHandler;
    use winit::dpi::PhysicalPosition;
    use winit::event::{Touch, TouchPhase, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::platform::android::EventLoopBuilderExtAndroid;
    use winit::platform::android::activity::AndroidApp;
    use winit::window::{Window, WindowAttributes, WindowId};

    const LOG_TAG: &str = "mclone_android";
    const TOUCH_ORBIT_RADIANS_PER_SCREEN: f32 = 2.4;

    #[allow(unsafe_code)]
    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_print(
            priority: c_int,
            tag: *const c_char,
            format: *const c_char,
            ...
        ) -> c_int;
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
        scene: AndroidSceneRuntime,
        camera: ChunkCamera,
        render_options: TexturedSectionRenderOptions,
        render_stats: RenderStreamStats,
        frame_index: u64,
    }

    struct AndroidSceneRuntime {
        core: SingleViewRuntime,
        server_runner: NativeIntegratedServerRunner,
        mesh_assets: TexturedMeshAssets,
        render_compile_worker: RenderSectionCompileWorker,
    }

    enum AndroidRenderError {
        Surface(wgpu::SurfaceError),
        Render(anyhow::Error),
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
            let mut scene = AndroidSceneRuntime::new()?;
            let camera = android_smoke_camera(scene.render_distance());
            let camera_position = camera_position(camera);
            let (poll_count, poll_ms) = scene.poll_until_idle()?;
            log::info!(
                "Mclone Android integrated runtime idle: polls={} poll_ms={:.1} loaded_chunks={}",
                poll_count,
                poll_ms,
                scene.loaded_chunk_count()
            );
            let section_update = scene.sync_all_render_sections(camera_position)?;
            let sections = scene.cached_sections();
            if sections.is_empty() {
                bail!(
                    "Android integrated runtime produced no render sections: loaded_chunks={} rebuilt_sections={} submitted_compile_sections={} completed_compile_sections={} pending_compile_jobs={}",
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
                scene.mesh_assets.atlas.as_upload(),
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
                "Mclone Android uploaded integrated render sections: sections={} faces={} indices={} rebuilt_sections={} visibility_graph_count={}",
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
                scene,
                camera,
                render_options: TexturedSectionRenderOptions::default(),
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
            let summary = render_full_frame_for_view(
                frame,
                &self.depth,
                &self.sky,
                &mut self.draw,
                None,
                None,
                None,
                render_view,
                &[],
                None,
                self.scene.sky_clear_color(),
                time_of_day,
                sun_angle,
                self.render_options,
                FullFrameGui::new(false, false, [1.0, 1.0]),
                |_| GuiDrawList::new(),
                &mut self.render_stats,
            )?;
            if self.frame_index == 0 {
                log::info!(
                    "Mclone Android rendered integrated frame: sections={}/{} indices={}/{} gui_commands={}",
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

    impl AndroidSceneRuntime {
        fn new() -> Result<Self> {
            const ANDROID_SMOKE_SEED: i64 = 12345;
            const ANDROID_SMOKE_RENDER_DISTANCE: u32 = 2;
            const ANDROID_SMOKE_DAY_TIME: u64 = 6000;

            let render_distance = ANDROID_SMOKE_RENDER_DISTANCE;
            let chunk_tracking_radius = chunk_tracking_radius_for_render_distance(render_distance);
            let interest_center = ChunkPos::new(0, 0);
            let mesh_assets = load_textured_mesh_assets()?;
            let render_compile_worker =
                RenderSectionCompileWorker::new(mesh_assets.catalog.clone())?;
            let server_runner = NativeIntegratedServerRunner::new(
                NativeIntegratedServerRunnerConfig::new(ANDROID_SMOKE_SEED)
                    .with_day_time(Some(ANDROID_SMOKE_DAY_TIME))
                    .with_day_time_frozen(true),
            )
            .context("failed to start Android integrated server runner")?;
            let mut scene = Self {
                core: SingleViewRuntime::local_integrated(
                    interest_center,
                    render_distance,
                    chunk_tracking_radius,
                ),
                server_runner,
                mesh_assets,
                render_compile_worker,
            };
            scene.core.force_day_time(ANDROID_SMOKE_DAY_TIME);
            scene.set_chunk_view(interest_center, render_distance, chunk_tracking_radius)?;
            Ok(scene)
        }

        fn render_distance(&self) -> u32 {
            self.core.render_distance()
        }

        fn set_chunk_view(
            &mut self,
            center: ChunkPos,
            render_distance: u32,
            chunk_tracking_radius: u32,
        ) -> Result<()> {
            let Some(command) =
                self.core
                    .set_chunk_view_command(center, render_distance, chunk_tracking_radius)
            else {
                return Ok(());
            };
            self.server_runner
                .send_command(command)
                .context("failed to send Android chunk view command")?;
            let _ = self.drain_runner_updates_report()?;
            Ok(())
        }

        fn poll(&mut self) -> Result<bool> {
            let flush_start = Instant::now();
            let apply_report = self.drain_runner_updates_report()?;
            let changed = apply_report.changed;
            let runner_diagnostics = self
                .server_runner
                .poll_diagnostics()
                .context("failed to poll Android integrated server diagnostics")?;
            self.core.finish_poll_diagnostics(
                elapsed_ms(flush_start.elapsed()),
                apply_report,
                Some(&runner_diagnostics),
            );
            Ok(changed)
        }

        fn poll_until_idle(&mut self) -> Result<(usize, f64)> {
            let deadline = Instant::now() + Duration::from_secs(120);
            let mut polls = 0_usize;
            let mut poll_ms = 0.0_f64;
            loop {
                let poll_start = Instant::now();
                self.poll()?;
                poll_ms += elapsed_ms(poll_start.elapsed());
                polls += 1;
                let diagnostics = self.server_runner_diagnostics()?;
                if runner_idle(&diagnostics) {
                    return Ok((polls, poll_ms));
                }
                if Instant::now() >= deadline {
                    bail!("timed out waiting for Android integrated runtime worldgen jobs");
                }
                if diagnostics.update_queue_depth == 0 {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        }

        fn server_runner_diagnostics(&self) -> Result<ServerRunnerDiagnostics> {
            self.server_runner
                .poll_diagnostics()
                .context("failed to poll Android integrated server diagnostics")
        }

        fn drain_runner_updates_report(&mut self) -> Result<RuntimeUpdateApplyReport> {
            let updates = self
                .server_runner
                .drain_updates()
                .context("failed to drain Android integrated server updates")?;
            if updates.is_empty() {
                return Ok(RuntimeUpdateApplyReport::default());
            }
            Ok(self.core.apply_server_updates_report(updates))
        }

        fn sync_render_sections(
            &mut self,
            camera_position: Vec3,
        ) -> Result<RenderSectionCacheUpdate> {
            let render_compile_worker = &mut self.render_compile_worker;
            self.core.sync_render_sections(
                render_compile_worker,
                camera_position,
                |client, _compiler| client.chunk_snapshots().cloned().collect(),
            )
        }

        fn sync_all_render_sections(
            &mut self,
            camera_position: Vec3,
        ) -> Result<RenderSectionCacheUpdate> {
            let deadline = Instant::now() + Duration::from_secs(120);
            let mut combined = RenderSectionCacheUpdate::default();
            loop {
                let update = self.core.sync_render_sections_with_budget(
                    &mut self.render_compile_worker,
                    camera_position,
                    usize::MAX,
                    |client, _compiler| client.chunk_snapshots().cloned().collect(),
                )?;
                let progressed = update.rebuilt_section_count() > 0
                    || update.removed_section_count() > 0
                    || update.submitted_compile_section_count > 0
                    || update.completed_compile_section_count > 0
                    || update.stale_compile_section_count > 0;
                combined.merge(update);
                if self.render_compile_worker.pending_job_count() == 0
                    && !self.core.has_ready_pending_render_work(camera_position)
                {
                    combined.pending_compile_jobs = 0;
                    return Ok(combined);
                }
                if Instant::now() >= deadline {
                    bail!("timed out waiting for Android render section compile queue");
                }
                if !progressed {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        }

        fn cached_sections(&self) -> Vec<mclone_mesh::TexturedRenderSectionMesh> {
            self.core.cached_sections()
        }

        fn loaded_chunk_count(&self) -> usize {
            self.core.client().loaded_chunk_count()
        }

        fn traversal_ready_render_section_keys(
            &self,
            camera_position: Vec3,
        ) -> std::collections::BTreeSet<mclone_mesh::RenderSectionKey> {
            self.core
                .traversal_ready_render_section_keys(camera_position)
        }

        fn sky_clear_color(&self) -> wgpu::Color {
            mclone_render::sky::overworld_clear_color(self.core.time_of_day())
        }

        fn time_of_day(&self) -> f32 {
            self.core.time_of_day()
        }

        fn sun_angle(&self) -> f32 {
            self.core.sun_angle()
        }
    }

    fn runner_idle(diagnostics: &ServerRunnerDiagnostics) -> bool {
        diagnostics.command_queue_depth == 0
            && diagnostics.update_queue_depth == 0
            && !diagnostics.awaiting_tick
            && diagnostics.pending_jobs == 0
            && diagnostics.pending_publications == 0
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
                WindowEvent::Touch(touch) => self.handle_touch(&window, touch),
                _ => {}
            }
        }
    }

    impl McloneAndroidApp {
        fn handle_touch(&mut self, window: &Window, touch: Touch) {
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
