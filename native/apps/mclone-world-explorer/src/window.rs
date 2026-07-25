use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::capture::{TextureReadback, save_validated_png};
use crate::input::NativeViewInput;
use crate::options::ExplorerOptions;
use crate::terrain::ExplorerTerrain;

pub fn run_window(options: ExplorerOptions, started: Instant) -> Result<()> {
    let event_loop = EventLoop::new().context("failed to create World Explorer event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ExplorerApp::new(options, started);
    event_loop
        .run_app(&mut app)
        .context("World Explorer event loop failed")?;
    if let Some(error) = app.fatal_error {
        return Err(error);
    }
    Ok(())
}

struct ExplorerApp {
    options: ExplorerOptions,
    started: Instant,
    window: Option<Arc<Window>>,
    gpu: Option<WindowGpu>,
    needs_redraw: bool,
    fatal_error: Option<anyhow::Error>,
}

impl ExplorerApp {
    fn new(options: ExplorerOptions, started: Instant) -> Self {
        Self {
            options,
            started,
            window: None,
            gpu: None,
            needs_redraw: true,
            fatal_error: None,
        }
    }

    fn initialize(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        if self.window.is_some() {
            return Ok(());
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title(self.options.title())
                        .with_inner_size(PhysicalSize::new(
                            self.options.width,
                            self.options.height,
                        )),
                )
                .context("failed to create World Explorer window")?,
        );
        let size = window.inner_size();
        let mut options = self.options.clone();
        options.width = size.width.max(1);
        options.height = size.height.max(1);
        let gpu = WindowGpu::new(window.clone(), options, self.started)?;
        self.window = Some(window.clone());
        self.gpu = Some(gpu);
        self.needs_redraw = true;
        window.request_redraw();
        Ok(())
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: anyhow::Error) {
        log::error!("World Explorer stopped: {error:#}");
        self.fatal_error = Some(error);
        event_loop.exit();
    }
}

impl ApplicationHandler for ExplorerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.initialize(event_loop) {
            self.fail(event_loop, error);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        log::trace!("World Explorer window event: {event:?}");
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if window.id() != window_id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed
                    && event.physical_key == PhysicalKey::Code(KeyCode::Escape) =>
            {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(gpu) = self.gpu.as_mut() {
                    match gpu.keyboard(&event) {
                        Ok(redraw) if redraw => {
                            self.needs_redraw = true;
                            event_loop.set_control_flow(ControlFlow::Poll);
                            window.set_title(&gpu.title());
                            window.request_redraw();
                        }
                        Ok(_) => {}
                        Err(error) => self.fail(event_loop, error),
                    }
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.set_modifiers(modifiers.state());
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(gpu) = self.gpu.as_mut() {
                    match gpu.cursor_moved(position.x, position.y) {
                        Ok(true) => {
                            self.needs_redraw = true;
                            event_loop.set_control_flow(ControlFlow::Poll);
                            window.set_title(&gpu.title());
                            window.request_redraw();
                        }
                        Ok(false) => {}
                        Err(error) => self.fail(event_loop, error),
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(gpu) = self.gpu.as_mut() {
                    match gpu.mouse_button(button, state) {
                        Ok(true) => {
                            self.needs_redraw = true;
                            event_loop.set_control_flow(ControlFlow::Poll);
                            window.set_title(&gpu.title());
                            window.request_redraw();
                        }
                        Ok(false) => {}
                        Err(error) => self.fail(event_loop, error),
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(gpu) = self.gpu.as_mut() {
                    match gpu.mouse_wheel(delta) {
                        Ok(true) => {
                            self.needs_redraw = true;
                            event_loop.set_control_flow(ControlFlow::Poll);
                            window.set_title(&gpu.title());
                            window.request_redraw();
                        }
                        Ok(false) => {}
                        Err(error) => self.fail(event_loop, error),
                    }
                }
            }
            WindowEvent::Touch(touch) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    match gpu.touch(touch) {
                        Ok(true) => {
                            self.needs_redraw = true;
                            event_loop.set_control_flow(ControlFlow::Poll);
                            window.set_title(&gpu.title());
                            window.request_redraw();
                        }
                        Ok(false) => {}
                        Err(error) => self.fail(event_loop, error),
                    }
                }
            }
            WindowEvent::Focused(false) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    match gpu.cancel_input() {
                        Ok(_) => {
                            self.needs_redraw = true;
                            window.request_redraw();
                        }
                        Err(error) => self.fail(event_loop, error),
                    }
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = self.gpu.as_mut()
                    && let Err(error) = gpu.resize(size)
                {
                    self.fail(event_loop, error);
                    return;
                }
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let Some(gpu) = self.gpu.as_mut() else {
                    return;
                };
                match gpu.render() {
                    Ok(outcome) => {
                        self.needs_redraw = outcome.needs_redraw;
                        event_loop.set_control_flow(if outcome.needs_redraw {
                            ControlFlow::Poll
                        } else {
                            ControlFlow::Wait
                        });
                        if outcome.capture_completed {
                            event_loop.exit();
                        } else if outcome.needs_redraw {
                            window.request_redraw();
                        }
                    }
                    Err(WindowRenderError::Surface(wgpu::SurfaceError::Lost))
                    | Err(WindowRenderError::Surface(wgpu::SurfaceError::Outdated)) => {
                        if let Err(error) = gpu.reconfigure() {
                            self.fail(event_loop, error);
                        } else {
                            window.request_redraw();
                        }
                    }
                    Err(WindowRenderError::Surface(wgpu::SurfaceError::Timeout)) => {
                        log::warn!("World Explorer surface acquisition timed out");
                        window.request_redraw();
                    }
                    Err(WindowRenderError::Surface(wgpu::SurfaceError::OutOfMemory)) => {
                        self.fail(
                            event_loop,
                            anyhow::anyhow!("World Explorer WGPU surface ran out of memory"),
                        );
                    }
                    Err(WindowRenderError::Surface(wgpu::SurfaceError::Other)) => {
                        window.request_redraw();
                    }
                    Err(WindowRenderError::Terrain(error)) => self.fail(event_loop, error),
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        log::trace!(
            "World Explorer about to wait: needs_redraw={}",
            self.needs_redraw
        );
        if self.needs_redraw {
            event_loop.set_control_flow(ControlFlow::Poll);
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

enum WindowRenderError {
    Surface(wgpu::SurfaceError),
    Terrain(anyhow::Error),
}

struct WindowGpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    terrain: ExplorerTerrain,
    input: NativeViewInput,
    capture_path: Option<std::path::PathBuf>,
}

impl WindowGpu {
    fn new(window: Arc<Window>, options: ExplorerOptions, started: Instant) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let surface = instance
            .create_surface(window)
            .context("failed to create World Explorer WGPU surface")?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .context("no suitable World Explorer window WGPU adapter")?;
        let adapter_info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("mclone_world_explorer_window_device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .context("failed to create World Explorer window WGPU device")?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .context("World Explorer surface reported no color formats")?;
        let present_mode = capabilities
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::AutoNoVsync)
            .or_else(|| {
                capabilities
                    .present_modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::Immediate)
            })
            .or_else(|| {
                capabilities
                    .present_modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::Mailbox)
            })
            .or_else(|| {
                capabilities
                    .present_modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::Fifo)
            })
            .unwrap_or(wgpu::PresentMode::Fifo);
        let capture_path = options.window_capture.clone();
        let capture_usage = if capture_path.is_some() {
            if !capabilities.usages.contains(wgpu::TextureUsages::COPY_SRC) {
                anyhow::bail!("World Explorer surface does not support capture readback");
            }
            wgpu::TextureUsages::COPY_SRC
        } else {
            wgpu::TextureUsages::empty()
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | capture_usage,
            format,
            width: options.width,
            height: options.height,
            present_mode,
            alpha_mode: capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 3,
        };
        surface.configure(&device, &config);
        let terrain = ExplorerTerrain::new(&device, &queue, format, options.clone(), started)?;
        log::info!(
            "World Explorer window adapter={:?} backend={:?} device_type={:?} \
             driver={:?} format={format:?} present_mode={present_mode:?} \
             present_modes={:?} asset_profile={} asset_bytes={} {}",
            adapter_info.name,
            adapter_info.backend,
            adapter_info.device_type,
            adapter_info.driver,
            capabilities.present_modes,
            options.asset_profile.label(),
            options.asset_bytes()?,
            terrain.diagnostics(),
        );
        Ok(Self {
            surface,
            device,
            queue,
            config,
            terrain,
            input: NativeViewInput::new(started),
            capture_path,
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) -> Result<()> {
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.terrain.resize(&self.device, size.width, size.height)
    }

    fn reconfigure(&mut self) -> Result<()> {
        self.surface.configure(&self.device, &self.config);
        self.terrain
            .resize(&self.device, self.config.width, self.config.height)
    }

    fn title(&self) -> String {
        self.terrain.title()
    }

    fn set_modifiers(&mut self, modifiers: winit::keyboard::ModifiersState) {
        self.input.set_modifiers(modifiers);
    }

    fn cursor_moved(&mut self, x: f64, y: f64) -> Result<bool> {
        let intents = self
            .input
            .cursor_moved(x, y, self.terrain.view_state(), self.viewport());
        self.apply_intents(intents)
    }

    fn mouse_button(
        &mut self,
        button: winit::event::MouseButton,
        state: ElementState,
    ) -> Result<bool> {
        let intents =
            self.input
                .mouse_button(button, state, self.terrain.view_state(), self.viewport());
        self.apply_intents(intents)
    }

    fn mouse_wheel(&mut self, delta: winit::event::MouseScrollDelta) -> Result<bool> {
        let intent = self
            .input
            .mouse_wheel(delta, self.terrain.view_state(), self.viewport());
        self.terrain.apply_intent(intent)
    }

    fn touch(&mut self, touch: winit::event::Touch) -> Result<bool> {
        let intents = self
            .input
            .touch(touch, self.terrain.view_state(), self.viewport());
        self.apply_intents(intents)
    }

    fn keyboard(&mut self, event: &winit::event::KeyEvent) -> Result<bool> {
        let intents = self
            .input
            .keyboard(event, self.terrain.view_state(), self.viewport());
        let changed = self.apply_intents(intents)?;
        Ok(changed || self.input.has_continuous_input())
    }

    fn cancel_input(&mut self) -> Result<bool> {
        let intents = self.input.cancel(self.terrain.view_state());
        self.apply_intents(intents)
    }

    fn apply_intents(
        &mut self,
        intents: impl IntoIterator<Item = mclone_view_control::WorldViewIntent>,
    ) -> Result<bool> {
        let mut changed = false;
        for intent in intents {
            changed |= self.terrain.apply_intent(intent)?;
        }
        Ok(changed)
    }

    fn viewport(&self) -> mclone_view_control::ViewportMetrics {
        mclone_view_control::ViewportMetrics::new(
            f64::from(self.config.width),
            f64::from(self.config.height),
        )
    }

    fn render(&mut self) -> std::result::Result<WindowRenderOutcome, WindowRenderError> {
        if let Some(intent) = self
            .input
            .continuous_intent(Instant::now(), self.terrain.view_state())
        {
            self.terrain
                .apply_intent(intent)
                .map_err(WindowRenderError::Terrain)?;
        }
        let continuous_input = self.input.has_continuous_input();
        log::trace!("World Explorer acquiring surface frame");
        let frame = self
            .surface
            .get_current_texture()
            .map_err(WindowRenderError::Surface)?;
        log::trace!("World Explorer acquired surface frame");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_world_explorer_window_frame"),
            });
        let stats = self
            .terrain
            .encode(&self.device, &self.queue, &mut encoder, &view)
            .map_err(WindowRenderError::Terrain)?;
        log::trace!("World Explorer encoded surface frame");
        let capture = if stats.target_ready && !stats.needs_redraw && self.capture_path.is_some() {
            Some(
                TextureReadback::encode(
                    &self.device,
                    &mut encoder,
                    &frame.texture,
                    self.config.width,
                    self.config.height,
                    self.config.format,
                )
                .map_err(WindowRenderError::Terrain)?,
            )
        } else {
            None
        };
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        log::trace!("World Explorer presented surface frame");
        self.terrain
            .poll_completed(&self.device)
            .map_err(WindowRenderError::Terrain)?;
        log::trace!("World Explorer polled surface frame");
        let capture_completed = if let Some(capture) = capture {
            let path = self
                .capture_path
                .take()
                .expect("capture path exists when its readback was encoded");
            let pixels = capture
                .finish(&self.device)
                .map_err(WindowRenderError::Terrain)?;
            save_validated_png(&path, &pixels, self.config.width, self.config.height)
                .map_err(WindowRenderError::Terrain)?;
            log::info!(
                "World Explorer native surface capture={} {}",
                path.display(),
                self.terrain.diagnostics()
            );
            true
        } else {
            false
        };
        Ok(WindowRenderOutcome {
            needs_redraw: continuous_input || stats.needs_redraw || !stats.target_ready,
            capture_completed,
        })
    }
}

struct WindowRenderOutcome {
    needs_redraw: bool,
    capture_completed: bool,
}
