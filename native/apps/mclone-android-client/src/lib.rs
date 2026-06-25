#![deny(unsafe_code)]

#[cfg(target_os = "android")]
mod android {
    use std::sync::Arc;

    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::platform::android::EventLoopBuilderExtAndroid;
    use winit::platform::android::activity::AndroidApp;
    use winit::window::{Window, WindowAttributes, WindowId};

    struct AndroidGpuState {
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
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
            })
        }

        fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
            self.config.width = size.width.max(1);
            self.config.height = size.height.max(1);
            self.surface.configure(&self.device, &self.config);
        }

        fn render_clear_frame(&mut self) -> Result<(), wgpu::SurfaceError> {
            let frame = self.surface.get_current_texture()?;
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("mclone_android_clear_encoder"),
                });
            {
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mclone_android_clear_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.03,
                                g: 0.18,
                                b: 0.28,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
            }
            self.queue.submit(std::iter::once(encoder.finish()));
            frame.present();
            Ok(())
        }
    }

    #[derive(Default)]
    struct McloneAndroidApp {
        window: Option<Arc<Window>>,
        gpu: Option<AndroidGpuState>,
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
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: WindowId,
            event: WindowEvent,
        ) {
            let Some(window) = self.window.as_ref() else {
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
                    match gpu.render_clear_frame() {
                        Ok(()) => {}
                        Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(wgpu::SurfaceError::Timeout) => window.request_redraw(),
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            gpu.resize(window.inner_size());
                            window.request_redraw();
                        }
                        Err(wgpu::SurfaceError::Other) => window.request_redraw(),
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
                _ => {}
            }
        }
    }

    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    fn android_main(app: AndroidApp) {
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
