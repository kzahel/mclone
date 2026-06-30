use std::cell::RefCell;
use std::num::{NonZeroU32, NonZeroU64};

use anyhow::{Result, bail};
use glam::{Mat4, Vec3};
use mclone_ui::{ClipRect, Color, GuiDrawCommand, GuiDrawList, Rect};
use wgpu::util::DeviceExt;

use crate::chunk::{ChunkRenderView, ChunkTextureAtlas};
use crate::target::RenderFrameTarget;
use crate::uniform::{
    PER_VIEW_UNIFORM_SLOT_COUNT, PerViewSlot, PerViewUniformBuffer, SINGLE_VIEW_SLOT,
};

const FLOATS_PER_VERTEX: usize = 8;
const VERTEX_SIZE: wgpu::BufferAddress =
    (FLOATS_PER_VERTEX * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const GUI_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 3] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 8,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 16,
        shader_location: 2,
    },
];
const QUAD_VERTEX_COUNT: u32 = 6;
const WORLD_PANEL_FLOATS_PER_VERTEX: usize = 5;
const WORLD_PANEL_VERTEX_SIZE: wgpu::BufferAddress =
    (WORLD_PANEL_FLOATS_PER_VERTEX * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const WORLD_LINE_FLOATS_PER_VERTEX: usize = 7;
const WORLD_LINE_VERTEX_SIZE: wgpu::BufferAddress =
    (WORLD_LINE_FLOATS_PER_VERTEX * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const WORLD_GUI_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

const GUI_SOLID_WGSL: &str = r#"
struct VertexOut {
  @builtin(position) position: vec4f,
  @location(0) color: vec4f,
};

@vertex
fn vertex_main(
  @location(0) position: vec2f,
  @location(1) uv: vec2f,
  @location(2) color: vec4f,
) -> VertexOut {
  var out: VertexOut;
  out.position = vec4f(position, 0.0, 1.0);
  out.color = color;
  return out;
}

@fragment
fn fragment_main(input: VertexOut) -> @location(0) vec4f {
  return input.color;
}
"#;

const GUI_TEXTURED_WGSL: &str = r#"
@group(0) @binding(0) var gui_texture: texture_2d<f32>;
@group(0) @binding(1) var gui_sampler: sampler;

struct VertexOut {
  @builtin(position) position: vec4f,
  @location(0) uv: vec2f,
  @location(1) color: vec4f,
};

@vertex
fn vertex_main(
  @location(0) position: vec2f,
  @location(1) uv: vec2f,
  @location(2) color: vec4f,
) -> VertexOut {
  var out: VertexOut;
  out.position = vec4f(position, 0.0, 1.0);
  out.uv = uv;
  out.color = color;
  return out;
}

@fragment
fn fragment_main(input: VertexOut) -> @location(0) vec4f {
  return textureSample(gui_texture, gui_sampler, input.uv) * input.color;
}
"#;

const WORLD_GUI_WGSL: &str = r#"
struct PanelUniform {
  view_projection: mat4x4<f32>,
};

@group(0) @binding(0) var panel_texture: texture_2d<f32>;
@group(0) @binding(1) var panel_sampler: sampler;
@group(1) @binding(0) var<uniform> panel_uniform: PanelUniform;

struct VertexOut {
  @builtin(position) position: vec4f,
  @location(0) uv: vec2f,
};

@vertex
fn vertex_main(
  @location(0) world_position: vec3f,
  @location(1) uv: vec2f,
) -> VertexOut {
  var out: VertexOut;
  out.position = panel_uniform.view_projection * vec4f(world_position, 1.0);
  out.uv = uv;
  return out;
}

@fragment
fn fragment_main(input: VertexOut) -> @location(0) vec4f {
  return textureSample(panel_texture, panel_sampler, input.uv);
}
"#;

const WORLD_LINE_WGSL: &str = r#"
struct LineUniform {
  view_projection: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> line_uniform: LineUniform;

struct VertexOut {
  @builtin(position) position: vec4f,
  @location(0) color: vec4f,
};

@vertex
fn vertex_main(
  @location(0) world_position: vec3f,
  @location(1) color: vec4f,
) -> VertexOut {
  var out: VertexOut;
  out.position = line_uniform.view_projection * vec4f(world_position, 1.0);
  out.color = color;
  return out;
}

@fragment
fn fragment_main(input: VertexOut) -> @location(0) vec4f {
  return input.color;
}
"#;

const WORLD_GUI_MULTIVIEW_WGSL: &str = r#"
struct ViewUniform {
  view_projection: mat4x4<f32>,
};

struct StereoUniforms {
  views: array<ViewUniform, 2>,
};

@group(0) @binding(0) var panel_texture: texture_2d<f32>;
@group(0) @binding(1) var panel_sampler: sampler;
@group(1) @binding(0) var<uniform> stereo_uniforms: StereoUniforms;

struct VertexOut {
  @builtin(position) position: vec4f,
  @location(0) uv: vec2f,
};

@vertex
fn vertex_main(
  @location(0) world_position: vec3f,
  @location(1) uv: vec2f,
  @builtin(view_index) view_index: i32,
) -> VertexOut {
  var out: VertexOut;
  out.position = stereo_uniforms.views[u32(view_index)].view_projection * vec4f(world_position, 1.0);
  out.uv = uv;
  return out;
}

@fragment
fn fragment_main(input: VertexOut) -> @location(0) vec4f {
  return textureSample(panel_texture, panel_sampler, input.uv);
}
"#;

const WORLD_LINE_MULTIVIEW_WGSL: &str = r#"
struct ViewUniform {
  view_projection: mat4x4<f32>,
};

struct StereoUniforms {
  views: array<ViewUniform, 2>,
};

@group(0) @binding(0) var<uniform> stereo_uniforms: StereoUniforms;

struct VertexOut {
  @builtin(position) position: vec4f,
  @location(0) color: vec4f,
};

@vertex
fn vertex_main(
  @location(0) world_position: vec3f,
  @location(1) color: vec4f,
  @builtin(view_index) view_index: i32,
) -> VertexOut {
  var out: VertexOut;
  out.position = stereo_uniforms.views[u32(view_index)].view_projection * vec4f(world_position, 1.0);
  out.color = color;
  return out;
}

@fragment
fn fragment_main(input: VertexOut) -> @location(0) vec4f {
  return input.color;
}
"#;

#[derive(Clone, Copy, Debug)]
pub struct GuiRenderOptions {
    pub clear_color: Option<wgpu::Color>,
}

impl GuiRenderOptions {
    pub fn overlay() -> Self {
        Self { clear_color: None }
    }

    pub fn clear(color: wgpu::Color) -> Self {
        Self {
            clear_color: Some(color),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PreparedDrawKind {
    Solid,
    Textured,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PreparedDraw {
    kind: PreparedDrawKind,
    first_vertex: u32,
    vertex_count: u32,
    clip: Option<ClipRect>,
}

pub struct GuiRenderer {
    solid_pipeline: wgpu::RenderPipeline,
    textured_pipeline: wgpu::RenderPipeline,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    texture_atlas: Option<GuiTextureAtlas>,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_buffer_size: wgpu::BufferAddress,
}

impl GuiRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_gui_texture_bind_group_layout"),
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
        let solid_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_gui_solid_shader"),
            source: wgpu::ShaderSource::Wgsl(GUI_SOLID_WGSL.into()),
        });
        let solid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_gui_solid_pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &solid_module,
                entry_point: Some("vertex_main"),
                buffers: &[gui_vertex_buffer_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &solid_module,
                entry_point: Some("fragment_main"),
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
        let textured_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_gui_textured_shader"),
            source: wgpu::ShaderSource::Wgsl(GUI_TEXTURED_WGSL.into()),
        });
        let textured_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mclone_gui_textured_pipeline_layout"),
                bind_group_layouts: &[&texture_bind_group_layout],
                push_constant_ranges: &[],
            });
        let textured_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_gui_textured_pipeline"),
            layout: Some(&textured_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &textured_module,
                entry_point: Some("vertex_main"),
                buffers: &[gui_vertex_buffer_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &textured_module,
                entry_point: Some("fragment_main"),
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
        Self {
            solid_pipeline,
            textured_pipeline,
            texture_bind_group_layout,
            texture_atlas: None,
            vertex_buffer: None,
            vertex_buffer_size: 0,
        }
    }

    pub fn upload_texture_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: ChunkTextureAtlas<'_>,
    ) -> Result<()> {
        self.texture_atlas = Some(GuiTextureAtlas::new(
            device,
            queue,
            &self.texture_bind_group_layout,
            atlas,
        )?);
        Ok(())
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        gui_size: [f32; 2],
        gui: &GuiDrawList,
        options: GuiRenderOptions,
    ) -> Result<()> {
        let prepared = prepare_draws(gui.commands(), gui_size);
        let load = match options.clear_color {
            Some(color) => wgpu::LoadOp::Clear(color),
            None => wgpu::LoadOp::Load,
        };
        if prepared.draws.is_empty() {
            if options.clear_color.is_some() {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mclone_gui_clear_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target.color_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
            }
            return Ok(());
        }

        self.upload_vertices(device, queue, &prepared.vertices);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_gui_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        let vertex_buffer = self
            .vertex_buffer
            .as_ref()
            .expect("GUI vertex buffer exists");
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        for draw in prepared.draws {
            apply_scissor(&mut pass, target.size, gui_size, draw.clip);
            match draw.kind {
                PreparedDrawKind::Solid => {
                    pass.set_pipeline(&self.solid_pipeline);
                }
                PreparedDrawKind::Textured => {
                    let Some(atlas) = &self.texture_atlas else {
                        continue;
                    };
                    pass.set_pipeline(&self.textured_pipeline);
                    pass.set_bind_group(0, &atlas.bind_group, &[]);
                }
            }
            pass.draw(
                draw.first_vertex..draw.first_vertex + draw.vertex_count,
                0..1,
            );
        }
        Ok(())
    }

    fn upload_vertices(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, vertices: &[f32]) {
        let bytes = f32_bytes_vec(vertices);
        let required_size = bytes.len().max(4) as wgpu::BufferAddress;
        let needs_recreate = self
            .vertex_buffer
            .as_ref()
            .is_none_or(|_| self.vertex_buffer_size < required_size);
        if needs_recreate {
            self.vertex_buffer = Some(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("mclone_gui_vertices"),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                },
            ));
            self.vertex_buffer_size = required_size;
        } else if let Some(buffer) = &self.vertex_buffer {
            queue.write_buffer(buffer, 0, &bytes);
        }
    }
}

struct GuiTextureAtlas {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
}

impl GuiTextureAtlas {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        atlas: ChunkTextureAtlas<'_>,
    ) -> Result<Self> {
        let width = atlas.width.max(1);
        let height = atlas.height.max(1);
        let expected_len = width as usize * height as usize * 4;
        if atlas.rgba.len() != expected_len {
            bail!(
                "GUI texture atlas has {} bytes; expected {expected_len} for {}x{} RGBA",
                atlas.rgba.len(),
                width,
                height
            );
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_gui_texture_atlas"),
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
            atlas.rgba,
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
            label: Some("mclone_gui_texture_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_gui_texture_bind_group"),
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
        Ok(Self {
            _texture: texture,
            _view: view,
            _sampler: sampler,
            bind_group,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldGuiPanel {
    pub center: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub width: f32,
    pub height: f32,
}

impl WorldGuiPanel {
    pub fn new(center: Vec3, right: Vec3, up: Vec3, width: f32, height: f32) -> Self {
        Self {
            center,
            right: right.normalize_or_zero(),
            up: up.normalize_or_zero(),
            width: width.max(0.0),
            height: height.max(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldGuiLine {
    pub start: Vec3,
    pub end: Vec3,
    pub color: [f32; 4],
}

impl WorldGuiLine {
    pub fn new(start: Vec3, end: Vec3, color: [f32; 4]) -> Self {
        Self { start, end, color }
    }
}

pub struct WorldGuiRenderer {
    gui_renderer: GuiRenderer,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    _uniform_bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    uniforms: PerViewUniformBuffer,
    uniform_bind_group: wgpu::BindGroup,
    color_format: wgpu::TextureFormat,
    multiview: RefCell<Option<WorldGuiMultiviewRenderer>>,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_buffer_size: wgpu::BufferAddress,
    line_vertex_buffer: Option<wgpu::Buffer>,
    line_vertex_buffer_size: wgpu::BufferAddress,
    panel_texture: Option<WorldGuiTexture>,
}

impl WorldGuiRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_world_gui_texture_bind_group_layout"),
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
        let uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_world_gui_uniforms",
            64,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_world_gui_uniform_bind_group_layout"),
                entries: &[uniforms.layout_entry(0, wgpu::ShaderStages::VERTEX)],
            });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_world_gui_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[uniforms.bind_group_entry(0)],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_world_gui_shader"),
            source: wgpu::ShaderSource::Wgsl(WORLD_GUI_WGSL.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_world_gui_pipeline_layout"),
            bind_group_layouts: &[&texture_bind_group_layout, &uniform_bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_world_gui_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: WORLD_PANEL_VERTEX_SIZE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            shader_location: 0,
                            offset: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            shader_location: 1,
                            offset: 12,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
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
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_world_gui_line_shader"),
            source: wgpu::ShaderSource::Wgsl(WORLD_LINE_WGSL.into()),
        });
        let line_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_world_gui_line_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });
        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_world_gui_line_pipeline"),
            layout: Some(&line_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &line_shader,
                entry_point: Some("vertex_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: WORLD_LINE_VERTEX_SIZE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            shader_location: 0,
                            offset: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            shader_location: 1,
                            offset: 12,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &line_shader,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        Self {
            gui_renderer: GuiRenderer::new(device, WORLD_GUI_TEXTURE_FORMAT),
            texture_bind_group_layout,
            _uniform_bind_group_layout: uniform_bind_group_layout,
            pipeline,
            line_pipeline,
            uniforms,
            uniform_bind_group,
            color_format,
            multiview: RefCell::new(None),
            vertex_buffer: None,
            vertex_buffer_size: 0,
            line_vertex_buffer: None,
            line_vertex_buffer_size: 0,
            panel_texture: None,
        }
    }

    pub fn upload_texture_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: ChunkTextureAtlas<'_>,
    ) -> Result<()> {
        self.gui_renderer.upload_texture_atlas(device, queue, atlas)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_panel(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_view: ChunkRenderView,
        panel_pixels: [u32; 2],
        gui_size: [f32; 2],
        gui: &GuiDrawList,
        panel: WorldGuiPanel,
        lines: &[WorldGuiLine],
    ) -> Result<()> {
        self.render_panel_in_slot(
            device,
            queue,
            encoder,
            target,
            render_view,
            panel_pixels,
            gui_size,
            gui,
            panel,
            lines,
            SINGLE_VIEW_SLOT,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_panel_in_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_view: ChunkRenderView,
        panel_pixels: [u32; 2],
        gui_size: [f32; 2],
        gui: &GuiDrawList,
        panel: WorldGuiPanel,
        lines: &[WorldGuiLine],
        view_slot: PerViewSlot,
    ) -> Result<()> {
        if gui.commands().is_empty() || panel.width <= 0.0 || panel.height <= 0.0 {
            return Ok(());
        }
        let panel_pixels = [panel_pixels[0].max(1), panel_pixels[1].max(1)];
        self.ensure_panel_texture(device, panel_pixels);
        {
            let texture = self
                .panel_texture
                .as_ref()
                .expect("world GUI panel texture exists");
            self.gui_renderer.render(
                device,
                queue,
                encoder,
                RenderFrameTarget::color(&texture.view, panel_pixels),
                gui_size,
                gui,
                GuiRenderOptions::clear(wgpu::Color::TRANSPARENT),
            )?;
        }

        let vertices = world_gui_panel_vertices(panel);
        self.upload_world_vertices(device, queue, &vertices);
        let line_vertices = world_gui_line_vertices(lines);
        if !line_vertices.is_empty() {
            self.upload_world_line_vertices(device, queue, &line_vertices);
        }
        let uniform_offset =
            self.uniforms
                .write_slot(queue, view_slot, &matrix_bytes(render_view.view_projection));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_world_gui_panel_pass"),
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
        let texture = self
            .panel_texture
            .as_ref()
            .expect("world GUI panel texture exists");
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &texture.bind_group, &[]);
        pass.set_bind_group(1, &self.uniform_bind_group, &[uniform_offset]);
        pass.set_vertex_buffer(
            0,
            self.vertex_buffer
                .as_ref()
                .expect("world GUI vertex buffer exists")
                .slice(..),
        );
        pass.draw(0..QUAD_VERTEX_COUNT, 0..1);
        if !line_vertices.is_empty() {
            pass.set_pipeline(&self.line_pipeline);
            pass.set_bind_group(0, &self.uniform_bind_group, &[uniform_offset]);
            pass.set_vertex_buffer(
                0,
                self.line_vertex_buffer
                    .as_ref()
                    .expect("world GUI line vertex buffer exists")
                    .slice(..),
            );
            pass.draw(
                0..(line_vertices.len() / WORLD_LINE_FLOATS_PER_VERTEX) as u32,
                0..1,
            );
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_panel_multiview(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_views: [ChunkRenderView; 2],
        panel_pixels: [u32; 2],
        gui_size: [f32; 2],
        gui: &GuiDrawList,
        panel: WorldGuiPanel,
        lines: &[WorldGuiLine],
    ) -> Result<()> {
        if gui.commands().is_empty() || panel.width <= 0.0 || panel.height <= 0.0 {
            return Ok(());
        }
        let panel_pixels = [panel_pixels[0].max(1), panel_pixels[1].max(1)];
        self.ensure_panel_texture(device, panel_pixels);
        {
            let texture = self
                .panel_texture
                .as_ref()
                .expect("world GUI panel texture exists");
            self.gui_renderer.render(
                device,
                queue,
                encoder,
                RenderFrameTarget::color(&texture.view, panel_pixels),
                gui_size,
                gui,
                GuiRenderOptions::clear(wgpu::Color::TRANSPARENT),
            )?;
        }

        let vertices = world_gui_panel_vertices(panel);
        self.upload_world_vertices(device, queue, &vertices);
        let line_vertices = world_gui_line_vertices(lines);
        if !line_vertices.is_empty() {
            self.upload_world_line_vertices(device, queue, &line_vertices);
        }
        let renderer = self.multiview_renderer(device)?;
        renderer.write_uniforms(queue, render_views);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_world_gui_panel_multiview_pass"),
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
        let texture = self
            .panel_texture
            .as_ref()
            .expect("world GUI panel texture exists");
        pass.set_pipeline(&renderer.pipeline);
        pass.set_bind_group(0, &texture.bind_group, &[]);
        pass.set_bind_group(1, &renderer.uniform_bind_group, &[]);
        pass.set_vertex_buffer(
            0,
            self.vertex_buffer
                .as_ref()
                .expect("world GUI vertex buffer exists")
                .slice(..),
        );
        pass.draw(0..QUAD_VERTEX_COUNT, 0..1);
        if !line_vertices.is_empty() {
            pass.set_pipeline(&renderer.line_pipeline);
            pass.set_bind_group(0, &renderer.uniform_bind_group, &[]);
            pass.set_vertex_buffer(
                0,
                self.line_vertex_buffer
                    .as_ref()
                    .expect("world GUI line vertex buffer exists")
                    .slice(..),
            );
            pass.draw(
                0..(line_vertices.len() / WORLD_LINE_FLOATS_PER_VERTEX) as u32,
                0..1,
            );
        }
        Ok(())
    }

    pub fn render_lines_in_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_view: ChunkRenderView,
        lines: &[WorldGuiLine],
        view_slot: PerViewSlot,
    ) -> Result<()> {
        let line_vertices = world_gui_line_vertices(lines);
        if line_vertices.is_empty() {
            return Ok(());
        }
        self.upload_world_line_vertices(device, queue, &line_vertices);
        let uniform_offset =
            self.uniforms
                .write_slot(queue, view_slot, &matrix_bytes(render_view.view_projection));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_world_gui_lines_pass"),
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
        pass.set_pipeline(&self.line_pipeline);
        pass.set_bind_group(0, &self.uniform_bind_group, &[uniform_offset]);
        pass.set_vertex_buffer(
            0,
            self.line_vertex_buffer
                .as_ref()
                .expect("world GUI line vertex buffer exists")
                .slice(..),
        );
        pass.draw(
            0..(line_vertices.len() / WORLD_LINE_FLOATS_PER_VERTEX) as u32,
            0..1,
        );
        Ok(())
    }

    pub fn render_lines_multiview(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_views: [ChunkRenderView; 2],
        lines: &[WorldGuiLine],
    ) -> Result<()> {
        let line_vertices = world_gui_line_vertices(lines);
        if line_vertices.is_empty() {
            return Ok(());
        }
        self.upload_world_line_vertices(device, queue, &line_vertices);
        let renderer = self.multiview_renderer(device)?;
        renderer.write_uniforms(queue, render_views);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_world_gui_lines_multiview_pass"),
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
        pass.set_pipeline(&renderer.line_pipeline);
        pass.set_bind_group(0, &renderer.uniform_bind_group, &[]);
        pass.set_vertex_buffer(
            0,
            self.line_vertex_buffer
                .as_ref()
                .expect("world GUI line vertex buffer exists")
                .slice(..),
        );
        pass.draw(
            0..(line_vertices.len() / WORLD_LINE_FLOATS_PER_VERTEX) as u32,
            0..1,
        );
        Ok(())
    }

    fn multiview_renderer(
        &self,
        device: &wgpu::Device,
    ) -> Result<std::cell::Ref<'_, WorldGuiMultiviewRenderer>> {
        if !device.features().contains(wgpu::Features::MULTIVIEW) {
            bail!("world GUI multiview render requires wgpu MULTIVIEW");
        }
        if self.multiview.borrow().is_none() {
            let renderer = WorldGuiMultiviewRenderer::new(
                device,
                self.color_format,
                &self.texture_bind_group_layout,
            );
            *self.multiview.borrow_mut() = Some(renderer);
        }
        Ok(std::cell::Ref::map(self.multiview.borrow(), |renderer| {
            renderer
                .as_ref()
                .expect("world GUI multiview renderer initialized above")
        }))
    }

    fn ensure_panel_texture(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self
            .panel_texture
            .as_ref()
            .is_some_and(|texture| texture.size == size)
        {
            return;
        }
        self.panel_texture = Some(WorldGuiTexture::new(
            device,
            &self.texture_bind_group_layout,
            size,
        ));
    }

    fn upload_world_vertices(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: &[f32],
    ) {
        let bytes = f32_bytes_vec(vertices);
        let required_size = bytes.len().max(4) as wgpu::BufferAddress;
        let needs_recreate = self
            .vertex_buffer
            .as_ref()
            .is_none_or(|_| self.vertex_buffer_size < required_size);
        if needs_recreate {
            self.vertex_buffer = Some(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("mclone_world_gui_vertices"),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                },
            ));
            self.vertex_buffer_size = required_size;
        } else if let Some(buffer) = &self.vertex_buffer {
            queue.write_buffer(buffer, 0, &bytes);
        }
    }

    fn upload_world_line_vertices(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: &[f32],
    ) {
        let bytes = f32_bytes_vec(vertices);
        let required_size = bytes.len().max(4) as wgpu::BufferAddress;
        let needs_recreate = self
            .line_vertex_buffer
            .as_ref()
            .is_none_or(|_| self.line_vertex_buffer_size < required_size);
        if needs_recreate {
            self.line_vertex_buffer = Some(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("mclone_world_gui_line_vertices"),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                },
            ));
            self.line_vertex_buffer_size = required_size;
        } else if let Some(buffer) = &self.line_vertex_buffer {
            queue.write_buffer(buffer, 0, &bytes);
        }
    }
}

struct WorldGuiMultiviewRenderer {
    pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
}

impl WorldGuiMultiviewRenderer {
    fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_world_gui_multiview_uniforms"),
            size: 128,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_world_gui_multiview_uniform_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(128),
                    },
                    count: None,
                }],
            });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_world_gui_multiview_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_world_gui_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(WORLD_GUI_MULTIVIEW_WGSL.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_world_gui_multiview_pipeline_layout"),
            bind_group_layouts: &[texture_bind_group_layout, &uniform_bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = create_world_gui_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_world_gui_multiview_pipeline",
            NonZeroU32::new(2),
        );
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_world_gui_line_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(WORLD_LINE_MULTIVIEW_WGSL.into()),
        });
        let line_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_world_gui_line_multiview_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });
        let line_pipeline = create_world_gui_line_pipeline(
            device,
            &line_pipeline_layout,
            &line_shader,
            color_format,
            "mclone_world_gui_line_multiview_pipeline",
            NonZeroU32::new(2),
        );
        Self {
            pipeline,
            line_pipeline,
            uniform_buffer,
            uniform_bind_group,
        }
    }

    fn write_uniforms(&self, queue: &wgpu::Queue, render_views: [ChunkRenderView; 2]) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            &multiview_matrix_bytes([
                render_views[0].view_projection,
                render_views[1].view_projection,
            ]),
        );
    }
}

fn create_world_gui_pipeline(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    label: &'static str,
    multiview: Option<NonZeroU32>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vertex_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: WORLD_PANEL_VERTEX_SIZE,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        shader_location: 0,
                        offset: 0,
                        format: wgpu::VertexFormat::Float32x3,
                    },
                    wgpu::VertexAttribute {
                        shader_location: 1,
                        offset: 12,
                        format: wgpu::VertexFormat::Float32x2,
                    },
                ],
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fragment_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
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
        multiview,
        cache: None,
    })
}

fn create_world_gui_line_pipeline(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    label: &'static str,
    multiview: Option<NonZeroU32>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vertex_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: WORLD_LINE_VERTEX_SIZE,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        shader_location: 0,
                        offset: 0,
                        format: wgpu::VertexFormat::Float32x3,
                    },
                    wgpu::VertexAttribute {
                        shader_location: 1,
                        offset: 12,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                ],
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fragment_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: Default::default(),
        multiview,
        cache: None,
    })
}

struct WorldGuiTexture {
    size: [u32; 2],
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
}

impl WorldGuiTexture {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, size: [u32; 2]) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_world_gui_panel_texture"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: WORLD_GUI_TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mclone_world_gui_panel_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_world_gui_panel_bind_group"),
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
        Self {
            size,
            _texture: texture,
            view,
            _sampler: sampler,
            bind_group,
        }
    }
}

struct PreparedDraws {
    vertices: Vec<f32>,
    draws: Vec<PreparedDraw>,
}

fn prepare_draws(commands: &[GuiDrawCommand], gui_size: [f32; 2]) -> PreparedDraws {
    let mut vertices = Vec::new();
    let mut draws = Vec::new();
    for command in commands {
        match *command {
            GuiDrawCommand::SolidRect { rect, color, clip } => {
                let first_vertex = (vertices.len() / FLOATS_PER_VERTEX) as u32;
                push_solid_quad(&mut vertices, gui_size, rect, color, color);
                draws.push(PreparedDraw {
                    kind: PreparedDrawKind::Solid,
                    first_vertex,
                    vertex_count: QUAD_VERTEX_COUNT,
                    clip,
                });
            }
            GuiDrawCommand::GradientRect {
                rect,
                top,
                bottom,
                clip,
            } => {
                let first_vertex = (vertices.len() / FLOATS_PER_VERTEX) as u32;
                push_solid_quad(&mut vertices, gui_size, rect, top, bottom);
                draws.push(PreparedDraw {
                    kind: PreparedDrawKind::Solid,
                    first_vertex,
                    vertex_count: QUAD_VERTEX_COUNT,
                    clip,
                });
            }
            GuiDrawCommand::TextureRect {
                rect,
                uv,
                color,
                clip,
            } => {
                let first_vertex = (vertices.len() / FLOATS_PER_VERTEX) as u32;
                push_textured_quad(&mut vertices, gui_size, rect, uv, color);
                draws.push(PreparedDraw {
                    kind: PreparedDrawKind::Textured,
                    first_vertex,
                    vertex_count: QUAD_VERTEX_COUNT,
                    clip,
                });
            }
        }
    }
    PreparedDraws { vertices, draws }
}

fn gui_vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: VERTEX_SIZE,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &GUI_VERTEX_ATTRIBUTES,
    }
}

fn push_solid_quad(
    vertices: &mut Vec<f32>,
    gui_size: [f32; 2],
    rect: Rect,
    top: Color,
    bottom: Color,
) {
    push_quad(vertices, gui_size, rect, [0.0, 0.0, 0.0, 0.0], top, bottom);
}

fn push_textured_quad(
    vertices: &mut Vec<f32>,
    gui_size: [f32; 2],
    rect: Rect,
    uv: mclone_ui::GuiTextureUv,
    color: Color,
) {
    push_quad(
        vertices,
        gui_size,
        rect,
        [uv.u0, uv.v0, uv.u1, uv.v1],
        color,
        color,
    );
}

fn push_quad(
    vertices: &mut Vec<f32>,
    gui_size: [f32; 2],
    rect: Rect,
    uv: [f32; 4],
    top: Color,
    bottom: Color,
) {
    let left = clip_x(rect.x, gui_size[0]);
    let right = clip_x(rect.right(), gui_size[0]);
    let top_y = clip_y(rect.y, gui_size[1]);
    let bottom_y = clip_y(rect.bottom(), gui_size[1]);
    let top = top.to_linear_f32();
    let bottom = bottom.to_linear_f32();
    push_vertex(vertices, left, top_y, [uv[0], uv[1]], top);
    push_vertex(vertices, right, top_y, [uv[2], uv[1]], top);
    push_vertex(vertices, right, bottom_y, [uv[2], uv[3]], bottom);
    push_vertex(vertices, left, top_y, [uv[0], uv[1]], top);
    push_vertex(vertices, right, bottom_y, [uv[2], uv[3]], bottom);
    push_vertex(vertices, left, bottom_y, [uv[0], uv[3]], bottom);
}

fn push_vertex(vertices: &mut Vec<f32>, x: f32, y: f32, uv: [f32; 2], color: [f32; 4]) {
    vertices.extend_from_slice(&[x, y, uv[0], uv[1], color[0], color[1], color[2], color[3]]);
}

fn clip_x(x: f32, width: f32) -> f32 {
    (x / width.max(1.0)) * 2.0 - 1.0
}

fn clip_y(y: f32, height: f32) -> f32 {
    1.0 - (y / height.max(1.0)) * 2.0
}

fn apply_scissor(
    pass: &mut wgpu::RenderPass<'_>,
    target_size: [u32; 2],
    gui_size: [f32; 2],
    clip: Option<ClipRect>,
) {
    let Some(clip) = clip else {
        pass.set_scissor_rect(0, 0, target_size[0].max(1), target_size[1].max(1));
        return;
    };
    let scale_x = target_size[0].max(1) as f32 / gui_size[0].max(1.0);
    let scale_y = target_size[1].max(1) as f32 / gui_size[1].max(1.0);
    let x = (clip.x.max(0.0) * scale_x).floor() as u32;
    let y = (clip.y.max(0.0) * scale_y).floor() as u32;
    let right = ((clip.x + clip.width) * scale_x)
        .ceil()
        .clamp(0.0, target_size[0] as f32) as u32;
    let bottom = ((clip.y + clip.height) * scale_y)
        .ceil()
        .clamp(0.0, target_size[1] as f32) as u32;
    pass.set_scissor_rect(x, y, right.saturating_sub(x), bottom.saturating_sub(y));
}

fn f32_bytes_vec(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(values));
    for value in values {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn matrix_bytes(matrix: Mat4) -> [u8; 64] {
    let mut bytes = [0; 64];
    for (index, value) in matrix.to_cols_array().into_iter().enumerate() {
        let start = index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn multiview_matrix_bytes(matrices: [Mat4; 2]) -> [u8; 128] {
    let mut bytes = [0; 128];
    bytes[..64].copy_from_slice(&matrix_bytes(matrices[0]));
    bytes[64..].copy_from_slice(&matrix_bytes(matrices[1]));
    bytes
}

fn world_gui_panel_vertices(panel: WorldGuiPanel) -> Vec<f32> {
    let half_right = panel.right * (panel.width * 0.5);
    let half_up = panel.up * (panel.height * 0.5);
    let top_left = panel.center - half_right + half_up;
    let top_right = panel.center + half_right + half_up;
    let bottom_right = panel.center + half_right - half_up;
    let bottom_left = panel.center - half_right - half_up;
    let mut vertices =
        Vec::with_capacity(QUAD_VERTEX_COUNT as usize * WORLD_PANEL_FLOATS_PER_VERTEX);
    push_world_gui_vertex(&mut vertices, top_left, [0.0, 0.0]);
    push_world_gui_vertex(&mut vertices, top_right, [1.0, 0.0]);
    push_world_gui_vertex(&mut vertices, bottom_right, [1.0, 1.0]);
    push_world_gui_vertex(&mut vertices, top_left, [0.0, 0.0]);
    push_world_gui_vertex(&mut vertices, bottom_right, [1.0, 1.0]);
    push_world_gui_vertex(&mut vertices, bottom_left, [0.0, 1.0]);
    vertices
}

fn push_world_gui_vertex(vertices: &mut Vec<f32>, position: Vec3, uv: [f32; 2]) {
    vertices.extend_from_slice(&[position.x, position.y, position.z, uv[0], uv[1]]);
}

fn world_gui_line_vertices(lines: &[WorldGuiLine]) -> Vec<f32> {
    let mut vertices = Vec::with_capacity(lines.len() * 2 * WORLD_LINE_FLOATS_PER_VERTEX);
    for line in lines {
        push_world_gui_line_vertex(&mut vertices, line.start, line.color);
        push_world_gui_line_vertex(&mut vertices, line.end, line.color);
    }
    vertices
}

fn push_world_gui_line_vertex(vertices: &mut Vec<f32>, position: Vec3, color: [f32; 4]) {
    vertices.extend_from_slice(&[
        position.x, position.y, position.z, color[0], color[1], color[2], color[3],
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_one_quad_for_one_rect() {
        let mut draw = GuiDrawList::new();
        draw.fill(
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Color::rgba(255, 255, 255, 255),
        );
        let prepared = prepare_draws(draw.commands(), [100.0, 100.0]);
        assert_eq!(prepared.draws.len(), 1);
        assert_eq!(prepared.vertices.len(), 6 * FLOATS_PER_VERTEX);
    }

    #[test]
    fn prepares_textured_quad_with_uvs() {
        let mut draw = GuiDrawList::new();
        draw.texture(
            Rect::new(0.0, 0.0, 10.0, 10.0),
            mclone_ui::GuiTextureUv::new(0.25, 0.5, 0.75, 1.0),
            Color::WHITE,
        );

        let prepared = prepare_draws(draw.commands(), [100.0, 100.0]);

        assert_eq!(prepared.draws.len(), 1);
        assert_eq!(prepared.draws[0].kind, PreparedDrawKind::Textured);
        assert_eq!(prepared.vertices.len(), 6 * FLOATS_PER_VERTEX);
        assert_eq!(&prepared.vertices[2..4], &[0.25, 0.5]);
        assert_eq!(
            &prepared.vertices[FLOATS_PER_VERTEX + 2..FLOATS_PER_VERTEX + 4],
            &[0.75, 0.5]
        );
        assert_eq!(
            &prepared.vertices[FLOATS_PER_VERTEX * 2 + 2..FLOATS_PER_VERTEX * 2 + 4],
            &[0.75, 1.0]
        );
    }

    #[test]
    fn world_panel_vertices_face_camera_basis() {
        let panel = WorldGuiPanel::new(Vec3::ZERO, Vec3::X, Vec3::Y, 2.0, 1.0);
        let vertices = world_gui_panel_vertices(panel);

        assert_eq!(
            &vertices[0..5],
            &[-1.0, 0.5, 0.0, 0.0, 0.0],
            "top-left vertex"
        );
        assert_eq!(
            &vertices[5..10],
            &[1.0, 0.5, 0.0, 1.0, 0.0],
            "top-right vertex"
        );
        assert_eq!(
            &vertices[10..15],
            &[1.0, -0.5, 0.0, 1.0, 1.0],
            "bottom-right vertex"
        );
        assert_eq!(
            vertices.len(),
            QUAD_VERTEX_COUNT as usize * WORLD_PANEL_FLOATS_PER_VERTEX
        );
    }

    #[test]
    fn world_gui_line_vertices_pack_position_and_color() {
        let vertices = world_gui_line_vertices(&[WorldGuiLine::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(4.0, 5.0, 6.0),
            [0.1, 0.2, 0.3, 0.4],
        )]);

        assert_eq!(
            vertices,
            vec![
                1.0, 2.0, 3.0, 0.1, 0.2, 0.3, 0.4, 4.0, 5.0, 6.0, 0.1, 0.2, 0.3, 0.4,
            ]
        );
    }

    #[test]
    fn world_gui_multiview_uniform_serializes_distinct_views() {
        let left = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let right = Mat4::from_translation(Vec3::new(4.0, 5.0, 6.0));

        let bytes = multiview_matrix_bytes([left, right]);

        assert_eq!(bytes.len(), 128);
        assert_eq!(&bytes[..64], matrix_bytes(left).as_slice());
        assert_eq!(&bytes[64..], matrix_bytes(right).as_slice());
        assert_ne!(&bytes[..64], &bytes[64..]);
    }
}
