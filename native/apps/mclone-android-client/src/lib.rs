#![deny(unsafe_code)]

#[cfg(target_os = "android")]
mod android {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use mclone_app_runtime::frame_render::{FullFrameGui, RenderStreamStats, render_full_frame};
    use mclone_mesh::{
        RenderSectionKey, TexturedChunkVertex, TexturedRenderSectionMesh, TexturedVisibleChunkMesh,
        VisibilitySet, quad_face_count_from_indices,
    };
    use mclone_render::chunk::{
        ChunkCamera, ChunkDepthTarget, ChunkTextureAtlas, TexturedSectionDrawResources,
        TexturedSectionRenderOptions,
    };
    use mclone_render::sky_render::SkyRenderer;
    use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
    use mclone_ui::GuiDrawList;
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
        renderer: AndroidFrameRenderer,
    }

    struct AndroidFrameRenderer {
        depth: ChunkDepthTarget,
        sky: SkyRenderer,
        draw: TexturedSectionDrawResources,
        camera: ChunkCamera,
        render_options: TexturedSectionRenderOptions,
        render_stats: RenderStreamStats,
        frame_index: u64,
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
        ) -> anyhow::Result<Self> {
            let depth = ChunkDepthTarget::new(device, width, height);
            let sky = SkyRenderer::new(device, format);
            let sections = static_textured_sections();
            let ready_sections = sections
                .iter()
                .map(|section| section.key)
                .collect::<BTreeSet<_>>();
            let mut draw = TexturedSectionDrawResources::new(
                device,
                queue,
                format,
                &sections,
                static_chunk_atlas(),
            )?;
            draw.set_traversal_ready_sections(&ready_sections);

            let index_count = draw.index_count();
            let render_stats = RenderStreamStats {
                section_count: draw.section_count(),
                face_count: quad_face_count_from_indices(index_count),
                index_count,
                ..RenderStreamStats::default()
            };
            log::info!(
                "Mclone Android uploaded static render section smoke: sections={} faces={} indices={}",
                render_stats.section_count,
                render_stats.face_count,
                render_stats.index_count
            );

            Ok(Self {
                depth,
                sky,
                draw,
                camera: static_camera(),
                render_options: TexturedSectionRenderOptions {
                    section_occlusion_culling: false,
                    force_fullbright: true,
                    ..TexturedSectionRenderOptions::default()
                },
                render_stats,
                frame_index: 0,
            })
        }

        fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
            self.depth.resize(device, width, height);
        }

        fn render(&mut self, frame: RenderFrameContext<'_>) -> anyhow::Result<()> {
            let time_of_day = 0.0;
            let sun_angle = 0.0;
            let summary = render_full_frame(
                frame,
                &self.depth,
                &self.sky,
                &mut self.draw,
                None,
                None,
                None,
                self.camera,
                &[],
                None,
                mclone_render::sky::overworld_clear_color(time_of_day),
                time_of_day,
                sun_angle,
                self.render_options,
                FullFrameGui::new(false, false, [1.0, 1.0]),
                |_| GuiDrawList::new(),
                &mut self.render_stats,
            )?;
            if self.frame_index == 0 {
                log::info!(
                    "Mclone Android rendered static frame: sections={}/{} indices={}/{} gui_commands={}",
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

    const STATIC_ATLAS_RGBA: [u8; 16] = [
        255, 255, 255, 255, 205, 210, 215, 255, 205, 210, 215, 255, 255, 255, 255, 255,
    ];

    fn static_chunk_atlas() -> ChunkTextureAtlas<'static> {
        ChunkTextureAtlas {
            width: 2,
            height: 2,
            rgba: &STATIC_ATLAS_RGBA,
        }
    }

    fn static_camera() -> ChunkCamera {
        ChunkCamera {
            eye: [39.0, 61.0, -42.0],
            target: [8.0, 39.0, 8.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 58.0_f32.to_radians(),
            z_near: 0.1,
            z_far: 180.0,
        }
    }

    fn static_textured_sections() -> Vec<TexturedRenderSectionMesh> {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        push_cuboid(
            &mut vertices,
            &mut indices,
            [0.0, 32.0, 0.0],
            [16.0, 34.0, 16.0],
            [0.42, 0.75, 0.28, 1.0],
        );
        push_cuboid(
            &mut vertices,
            &mut indices,
            [3.0, 34.0, 3.0],
            [13.0, 38.0, 13.0],
            [0.55, 0.42, 0.30, 1.0],
        );
        push_cuboid(
            &mut vertices,
            &mut indices,
            [5.0, 38.0, 5.0],
            [11.0, 45.0, 11.0],
            [0.62, 0.64, 0.68, 1.0],
        );
        push_cuboid(
            &mut vertices,
            &mut indices,
            [7.0, 45.0, 7.0],
            [9.0, 48.0, 9.0],
            [0.95, 0.72, 0.25, 1.0],
        );
        let opaque_index_count = indices.len() as u32;
        vec![TexturedRenderSectionMesh {
            key: RenderSectionKey::new(0, 2, 0),
            mesh: TexturedVisibleChunkMesh {
                vertices,
                indices,
                opaque_index_count,
            },
            visibility: VisibilitySet::all_visible(),
        }]
    }

    fn push_cuboid(
        vertices: &mut Vec<TexturedChunkVertex>,
        indices: &mut Vec<u32>,
        min: [f32; 3],
        max: [f32; 3],
        color: [f32; 4],
    ) {
        let [x0, y0, z0] = min;
        let [x1, y1, z1] = max;
        push_quad(
            vertices,
            indices,
            [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]],
            color,
        );
        push_quad(
            vertices,
            indices,
            [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]],
            color,
        );
        push_quad(
            vertices,
            indices,
            [[x0, y0, z0], [x0, y1, z0], [x0, y1, z1], [x0, y0, z1]],
            color,
        );
        push_quad(
            vertices,
            indices,
            [[x1, y0, z1], [x1, y1, z1], [x1, y1, z0], [x1, y0, z0]],
            color,
        );
        push_quad(
            vertices,
            indices,
            [[x0, y0, z1], [x0, y1, z1], [x1, y1, z1], [x1, y0, z1]],
            color,
        );
        push_quad(
            vertices,
            indices,
            [[x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x0, y0, z0]],
            color,
        );
    }

    fn push_quad(
        vertices: &mut Vec<TexturedChunkVertex>,
        indices: &mut Vec<u32>,
        positions: [[f32; 3]; 4],
        color: [f32; 4],
    ) {
        let base = vertices.len() as u32;
        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        for (position, uv) in positions.into_iter().zip(uvs) {
            vertices.push(TexturedChunkVertex {
                position,
                uv,
                color,
                packed_light: mclone_render::light_texture::FULL_BRIGHT,
            });
        }
        indices.extend_from_slice(&[
            base,
            base + 1,
            base + 2,
            base,
            base + 2,
            base + 3,
            base,
            base + 2,
            base + 1,
            base,
            base + 3,
            base + 2,
        ]);
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
