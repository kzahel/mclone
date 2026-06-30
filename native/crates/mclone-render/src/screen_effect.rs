use std::num::{NonZeroU32, NonZeroU64};
use std::ops::Range;

use anyhow::{Context, Result, anyhow, bail};
use mclone_assets::{AssetPath, AssetSource};

use crate::target::RenderFrameTarget;
use crate::uniform::{PER_VIEW_UNIFORM_SLOT_COUNT, PerViewSlot, SINGLE_VIEW_SLOT};

pub const VANILLA_UNDERWATER_ALPHA: f32 = 0.1;
pub const VANILLA_UNDERWATER_FOV_MULTIPLIER: f32 = 0.85714287;
pub const VANILLA_UNDERWATER_UV_TILE: f32 = 4.0;
const FLOATS_PER_VERTEX: usize = 8;
const VERTEX_SIZE: wgpu::BufferAddress =
    (FLOATS_PER_VERTEX * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const MULTIVIEW_UNIFORM_FLOATS_PER_VIEW: usize = 8;
const MULTIVIEW_UNIFORM_SIZE: wgpu::BufferAddress =
    (2 * MULTIVIEW_UNIFORM_FLOATS_PER_VIEW * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const UNDERWATER_TEXTURE_PATH: &str = "assets/minecraft/textures/misc/underwater.png";
const WATER_VISION_MAX_TICKS: f32 = 600.0;
const WATER_VISION_QUICK_TICKS: f32 = 100.0;
const WATER_VISION_QUICK_PERCENT: f32 = 0.6;
const WATER_VISION_EXIT_TICK_SCALE: f32 = 10.0;
const UNDERWATER_EFFECT_FADE_IN_SECONDS: f32 = 0.35;
const UNDERWATER_EFFECT_FADE_OUT_SECONDS: f32 = 0.15;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnderwaterOverlay {
    pub brightness: f32,
    pub alpha: f32,
    pub uv_offset: [f32; 2],
    pub water_vision: f32,
    pub effect_strength: f32,
}

impl UnderwaterOverlay {
    pub fn vanilla_from_native_camera(yaw_radians: f32, pitch_radians: f32) -> Self {
        Self::vanilla_from_native_camera_with_water_vision(yaw_radians, pitch_radians, 1.0)
    }

    pub fn vanilla_from_native_camera_with_water_vision(
        yaw_radians: f32,
        pitch_radians: f32,
        water_vision: f32,
    ) -> Self {
        Self::new(
            1.0,
            VANILLA_UNDERWATER_ALPHA * water_vision,
            underwater_uv_offset_from_native_radians(yaw_radians, pitch_radians),
            water_vision,
        )
    }

    pub fn new(brightness: f32, alpha: f32, uv_offset: [f32; 2], water_vision: f32) -> Self {
        let effect_strength = if VANILLA_UNDERWATER_ALPHA > 0.0 {
            alpha / VANILLA_UNDERWATER_ALPHA
        } else {
            0.0
        };
        Self {
            brightness: clamp_unit(brightness),
            alpha: clamp_unit(alpha),
            uv_offset,
            water_vision: clamp_unit(water_vision),
            effect_strength: clamp_unit(effect_strength),
        }
    }

    pub fn with_water_vision(self, water_vision: f32) -> Self {
        self.with_effect(water_vision, water_vision)
    }

    pub fn with_effect(self, water_vision: f32, effect_strength: f32) -> Self {
        let water_vision = clamp_unit(water_vision);
        let effect_strength = clamp_unit(effect_strength);
        Self {
            alpha: VANILLA_UNDERWATER_ALPHA * effect_strength,
            water_vision,
            effect_strength,
            ..self
        }
    }

    pub fn fov_multiplier(self) -> f32 {
        lerp(self.effect_strength, 1.0, VANILLA_UNDERWATER_FOV_MULTIPLIER)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UnderwaterEffectState {
    water_vision_ticks: f32,
    effect_strength: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UnderwaterEffect {
    pub water_vision: f32,
    pub effect_strength: f32,
}

impl UnderwaterEffectState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.water_vision_ticks = 0.0;
        self.effect_strength = 0.0;
    }

    pub fn update(&mut self, underwater: bool, dt_seconds: f32) -> UnderwaterEffect {
        let delta_ticks = if dt_seconds.is_finite() {
            (dt_seconds.max(0.0) * 20.0).min(WATER_VISION_MAX_TICKS)
        } else {
            0.0
        };
        let dt_seconds = if dt_seconds.is_finite() {
            dt_seconds.max(0.0)
        } else {
            0.0
        };
        if underwater {
            self.water_vision_ticks =
                (self.water_vision_ticks + delta_ticks).min(WATER_VISION_MAX_TICKS);
            self.effect_strength =
                (self.effect_strength + dt_seconds / UNDERWATER_EFFECT_FADE_IN_SECONDS).min(1.0);
        } else {
            self.water_vision_ticks =
                (self.water_vision_ticks - delta_ticks * WATER_VISION_EXIT_TICK_SCALE).max(0.0);
            self.effect_strength =
                (self.effect_strength - dt_seconds / UNDERWATER_EFFECT_FADE_OUT_SECONDS).max(0.0);
        }
        UnderwaterEffect {
            water_vision: self.water_vision(),
            effect_strength: self.effect_strength,
        }
    }

    pub fn water_vision(self) -> f32 {
        water_vision_from_ticks(self.water_vision_ticks)
    }
}

pub fn water_vision_from_ticks(ticks: f32) -> f32 {
    let ticks = if ticks.is_finite() {
        ticks.clamp(0.0, WATER_VISION_MAX_TICKS)
    } else {
        0.0
    };
    if ticks >= WATER_VISION_MAX_TICKS {
        1.0
    } else {
        let quick = (ticks / WATER_VISION_QUICK_TICKS).clamp(0.0, 1.0);
        let slow = if ticks < WATER_VISION_QUICK_TICKS {
            0.0
        } else {
            ((ticks - WATER_VISION_QUICK_TICKS)
                / (WATER_VISION_MAX_TICKS - WATER_VISION_QUICK_TICKS))
                .clamp(0.0, 1.0)
        };
        quick * WATER_VISION_QUICK_PERCENT + slow * (1.0 - WATER_VISION_QUICK_PERCENT)
    }
}

pub fn underwater_uv_offset_from_java_degrees(y_rot_degrees: f32, x_rot_degrees: f32) -> [f32; 2] {
    [-y_rot_degrees / 64.0, x_rot_degrees / 64.0]
}

pub fn underwater_uv_offset_from_native_radians(yaw_radians: f32, pitch_radians: f32) -> [f32; 2] {
    // Engine camera yaw/pitch are the opposite sign of Java's player
    // yRot/xRot fields used by ScreenEffectRenderer.renderWater.
    underwater_uv_offset_from_java_degrees(-yaw_radians.to_degrees(), -pitch_radians.to_degrees())
}

pub struct ScreenEffectsRenderer {
    pipeline: wgpu::RenderPipeline,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    underwater_texture: GpuScreenEffectTexture,
    color_format: wgpu::TextureFormat,
    multiview: Option<ScreenEffectsMultiviewRenderer>,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_buffer_slot_size: wgpu::BufferAddress,
}

impl ScreenEffectsRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        assets: &impl AssetSource,
    ) -> Result<Self> {
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_screen_effect_texture_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let underwater_texture = create_underwater_texture_bind_group(
            device,
            queue,
            &texture_bind_group_layout,
            assets,
        )?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_screen_effect_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/screen_effect.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_screen_effect_pipeline_layout"),
            bind_group_layouts: &[&texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_screen_effect_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: VERTEX_SIZE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            shader_location: 0,
                            offset: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            shader_location: 1,
                            offset: 8,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            shader_location: 2,
                            offset: 16,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            pipeline,
            texture_bind_group_layout,
            underwater_texture,
            color_format,
            multiview: None,
            vertex_buffer: None,
            vertex_buffer_slot_size: 0,
        })
    }

    pub fn render_underwater(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        overlay: UnderwaterOverlay,
    ) {
        self.render_underwater_in_slot(device, queue, encoder, target, overlay, SINGLE_VIEW_SLOT);
    }

    pub fn render_underwater_in_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        overlay: UnderwaterOverlay,
        view_slot: PerViewSlot,
    ) {
        let vertices = underwater_quad_vertices(overlay);
        let vertex_range = self.upload_vertices(device, queue, view_slot, &vertices);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_underwater_screen_effect_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.underwater_texture.bind_group, &[]);
        pass.set_vertex_buffer(
            0,
            self.vertex_buffer
                .as_ref()
                .expect("screen effect vertex buffer exists")
                .slice(vertex_range),
        );
        pass.draw(0..6, 0..1);
    }

    pub fn render_underwater_multiview(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        overlays: [Option<UnderwaterOverlay>; 2],
    ) -> Result<()> {
        if overlays.iter().all(Option::is_none) {
            return Ok(());
        }
        let vertices = underwater_multiview_quad_vertices();
        let vertex_range = self.upload_vertices(device, queue, SINGLE_VIEW_SLOT, &vertices);
        self.ensure_multiview_renderer(device)?;
        let renderer = self
            .multiview
            .as_ref()
            .expect("screen effect multiview renderer initialized above");
        queue.write_buffer(
            &renderer.uniform_buffer,
            0,
            &underwater_multiview_uniform_bytes(overlays),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_underwater_screen_effect_multiview_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pass.set_pipeline(&renderer.pipeline);
        pass.set_bind_group(0, &self.underwater_texture.bind_group, &[]);
        pass.set_bind_group(1, &renderer.uniform_bind_group, &[]);
        pass.set_vertex_buffer(
            0,
            self.vertex_buffer
                .as_ref()
                .expect("screen effect vertex buffer exists")
                .slice(vertex_range),
        );
        pass.draw(0..6, 0..1);
        Ok(())
    }

    fn ensure_multiview_renderer(&mut self, device: &wgpu::Device) -> Result<()> {
        if !device.features().contains(wgpu::Features::MULTIVIEW) {
            bail!("screen effect multiview render requires wgpu MULTIVIEW");
        }
        if self.multiview.is_none() {
            self.multiview = Some(ScreenEffectsMultiviewRenderer::new(
                device,
                &self.texture_bind_group_layout,
                self.color_format,
            ));
        }
        Ok(())
    }

    fn upload_vertices(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        slot: PerViewSlot,
        vertices: &[f32],
    ) -> Range<wgpu::BufferAddress> {
        let slot_index = slot.index();
        assert!(
            slot_index < PER_VIEW_UNIFORM_SLOT_COUNT,
            "screen effect vertex slot {slot_index} is outside slot count {PER_VIEW_UNIFORM_SLOT_COUNT}"
        );
        let bytes = f32_bytes_vec(vertices);
        let required_slot_size = bytes.len().max(4) as wgpu::BufferAddress;
        let needs_recreate = self
            .vertex_buffer
            .as_ref()
            .is_none_or(|_| self.vertex_buffer_slot_size < required_slot_size);
        if needs_recreate {
            self.vertex_buffer_slot_size = required_slot_size;
            self.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_screen_effect_vertices"),
                size: self.vertex_buffer_slot_size
                    * PER_VIEW_UNIFORM_SLOT_COUNT as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let slot_offset = self.vertex_buffer_slot_size * slot_index as wgpu::BufferAddress;
        if let Some(buffer) = &self.vertex_buffer {
            queue.write_buffer(buffer, slot_offset, &bytes);
        }
        slot_offset..slot_offset + bytes.len() as wgpu::BufferAddress
    }
}

struct ScreenEffectsMultiviewRenderer {
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl ScreenEffectsMultiviewRenderer {
    fn new(
        device: &wgpu::Device,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_screen_effect_multiview_uniforms"),
            size: MULTIVIEW_UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_screen_effect_multiview_uniform_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            NonZeroU64::new(MULTIVIEW_UNIFORM_SIZE)
                                .expect("multiview uniform size is non-zero"),
                        ),
                    },
                    count: None,
                }],
            });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_screen_effect_multiview_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_screen_effect_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/screen_effect_multiview.wgsl").into(),
            ),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_screen_effect_multiview_pipeline_layout"),
            bind_group_layouts: &[texture_bind_group_layout, &uniform_bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_screen_effect_multiview_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: VERTEX_SIZE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            shader_location: 0,
                            offset: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            shader_location: 1,
                            offset: 8,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            shader_location: 2,
                            offset: 16,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview: NonZeroU32::new(2),
            cache: None,
        });
        Self {
            uniform_buffer,
            uniform_bind_group,
            pipeline,
        }
    }
}

struct GpuScreenEffectTexture {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
}

fn create_underwater_texture_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    assets: &impl AssetSource,
) -> Result<GpuScreenEffectTexture> {
    let path = AssetPath::new(UNDERWATER_TEXTURE_PATH);
    let bytes = assets
        .read(&path)?
        .ok_or_else(|| anyhow!("missing underwater screen effect texture {}", path.as_str()))?;
    let image = image::load_from_memory(&bytes)
        .with_context(|| format!("failed to decode {}", path.as_str()))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        bail!(
            "underwater screen effect texture {} is empty",
            path.as_str()
        );
    }
    let rgba = image.into_raw();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mclone_underwater_screen_effect_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: Default::default(),
            aspect: Default::default(),
        },
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("mclone_underwater_screen_effect_sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("mclone_underwater_screen_effect_bind_group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });
    Ok(GpuScreenEffectTexture {
        _texture: texture,
        _view: view,
        _sampler: sampler,
        bind_group,
    })
}

fn underwater_quad_vertices(overlay: UnderwaterOverlay) -> Vec<f32> {
    let [u, v] = overlay.uv_offset;
    let color = [
        overlay.brightness,
        overlay.brightness,
        overlay.brightness,
        overlay.alpha,
    ];
    let mut vertices = Vec::with_capacity(6 * FLOATS_PER_VERTEX);
    push_vertex(
        &mut vertices,
        [-1.0, -1.0],
        [
            VANILLA_UNDERWATER_UV_TILE + u,
            VANILLA_UNDERWATER_UV_TILE + v,
        ],
        color,
    );
    push_vertex(
        &mut vertices,
        [1.0, -1.0],
        [u, VANILLA_UNDERWATER_UV_TILE + v],
        color,
    );
    push_vertex(&mut vertices, [1.0, 1.0], [u, v], color);
    push_vertex(
        &mut vertices,
        [-1.0, -1.0],
        [
            VANILLA_UNDERWATER_UV_TILE + u,
            VANILLA_UNDERWATER_UV_TILE + v,
        ],
        color,
    );
    push_vertex(&mut vertices, [1.0, 1.0], [u, v], color);
    push_vertex(
        &mut vertices,
        [-1.0, 1.0],
        [VANILLA_UNDERWATER_UV_TILE + u, v],
        color,
    );
    vertices
}

fn underwater_multiview_quad_vertices() -> Vec<f32> {
    underwater_quad_vertices(UnderwaterOverlay::new(1.0, 1.0, [0.0, 0.0], 1.0))
}

fn underwater_multiview_uniform_bytes(overlays: [Option<UnderwaterOverlay>; 2]) -> [u8; 64] {
    let mut bytes = [0_u8; MULTIVIEW_UNIFORM_SIZE as usize];
    let mut offset = 0;
    for overlay in overlays {
        let (uv_offset, color) = overlay.map_or(([0.0, 0.0], [0.0, 0.0, 0.0, 0.0]), |overlay| {
            (
                overlay.uv_offset,
                [
                    overlay.brightness,
                    overlay.brightness,
                    overlay.brightness,
                    overlay.alpha,
                ],
            )
        });
        let values = [
            uv_offset[0],
            uv_offset[1],
            0.0,
            0.0,
            color[0],
            color[1],
            color[2],
            color[3],
        ];
        for value in values {
            bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
            offset += 4;
        }
    }
    bytes
}

fn push_vertex(vertices: &mut Vec<f32>, position: [f32; 2], uv: [f32; 2], color: [f32; 4]) {
    vertices.extend_from_slice(&[
        position[0],
        position[1],
        uv[0],
        uv[1],
        color[0],
        color[1],
        color[2],
        color[3],
    ]);
}

fn f32_bytes_vec(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(values));
    for value in values {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn clamp_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn lerp(t: f32, from: f32, to: f32) -> f32 {
    from + (to - from) * clamp_unit(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn underwater_uv_offset_matches_java_formula() {
        assert_eq!(
            underwater_uv_offset_from_java_degrees(64.0, -32.0),
            [-1.0, -0.5]
        );
    }

    #[test]
    fn native_camera_uv_offset_accounts_for_engine_rotation_sign() {
        let offset =
            underwater_uv_offset_from_native_radians(90.0_f32.to_radians(), -45.0_f32.to_radians());

        assert!((offset[0] - 90.0 / 64.0).abs() < 1.0e-6);
        assert!((offset[1] - 45.0 / 64.0).abs() < 1.0e-6);
    }

    #[test]
    fn underwater_vertices_tile_fullscreen_quad_with_color_alpha() {
        let vertices =
            underwater_quad_vertices(UnderwaterOverlay::new(0.75, 0.25, [0.5, 0.25], 1.0));

        assert_eq!(vertices.len(), 6 * FLOATS_PER_VERTEX);
        assert_eq!(
            &vertices[0..8],
            &[-1.0, -1.0, 4.5, 4.25, 0.75, 0.75, 0.75, 0.25]
        );
        assert_eq!(
            &vertices[16..24],
            &[1.0, 1.0, 0.5, 0.25, 0.75, 0.75, 0.75, 0.25]
        );
    }

    #[test]
    fn underwater_multiview_uniform_serializes_distinct_eye_overlays() {
        let left = UnderwaterOverlay::new(0.75, 0.2, [0.5, 0.25], 1.0);
        let bytes = underwater_multiview_uniform_bytes([
            Some(left),
            Some(UnderwaterOverlay::new(0.5, 0.1, [-0.25, 0.75], 0.5)),
        ]);
        let floats = bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_ne_bytes(chunk.try_into().unwrap()))
            .collect::<Vec<_>>();

        assert_eq!(
            &floats[0..MULTIVIEW_UNIFORM_FLOATS_PER_VIEW],
            &[0.5, 0.25, 0.0, 0.0, 0.75, 0.75, 0.75, 0.2]
        );
        assert_eq!(
            &floats[MULTIVIEW_UNIFORM_FLOATS_PER_VIEW..MULTIVIEW_UNIFORM_FLOATS_PER_VIEW * 2],
            &[-0.25, 0.75, 0.0, 0.0, 0.5, 0.5, 0.5, 0.1]
        );
    }

    #[test]
    fn underwater_multiview_uniform_serializes_missing_eye_as_transparent() {
        let bytes = underwater_multiview_uniform_bytes([None, None]);
        assert!(bytes.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn water_vision_matches_java_piecewise_curve() {
        assert_eq!(water_vision_from_ticks(0.0), 0.0);
        assert!((water_vision_from_ticks(50.0) - 0.3).abs() < 1.0e-6);
        assert!((water_vision_from_ticks(100.0) - 0.6).abs() < 1.0e-6);
        assert_eq!(water_vision_from_ticks(600.0), 1.0);
    }

    #[test]
    fn underwater_effect_state_enters_and_exits_at_java_rates() {
        let mut state = UnderwaterEffectState::new();

        let entered = state.update(true, 5.0);
        assert!((entered.water_vision - 0.6).abs() < 1.0e-6);
        assert_eq!(entered.effect_strength, 1.0);

        let exited = state.update(false, 0.5);
        assert_eq!(exited.water_vision, 0.0);
        assert_eq!(exited.effect_strength, 0.0);
    }

    #[test]
    fn underwater_overlay_scales_alpha_and_fov_with_effect_strength() {
        let overlay =
            UnderwaterOverlay::vanilla_from_native_camera_with_water_vision(0.0, 0.0, 0.5)
                .with_effect(0.5, 0.25);

        assert_eq!(overlay.water_vision, 0.5);
        assert_eq!(overlay.alpha, VANILLA_UNDERWATER_ALPHA * 0.25);
        assert!((overlay.fov_multiplier() - 0.96428573).abs() < 1.0e-6);
    }
}
