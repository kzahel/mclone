use std::cell::RefCell;
use std::num::{NonZeroU32, NonZeroU64};
use std::ops::Range;

use anyhow::{Result, bail};
use glam::{Mat4, Vec3};

use crate::{
    chunk::{ChunkDepthTarget, ChunkMultiviewDepthTarget, ChunkRenderView, DEPTH_FORMAT},
    target::RenderFrameTarget,
    uniform::{PER_VIEW_UNIFORM_SLOT_COUNT, PerViewSlot, PerViewUniformBuffer, SINGLE_VIEW_SLOT},
};

const WORLD_COLOR_MESH_WGSL: &str = r#"
struct Uniforms {
    view_projection: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vertex_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = uniforms.view_projection * vec4<f32>(input.position, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
"#;

const WORLD_COLOR_MESH_MULTIVIEW_WGSL: &str = r#"
struct ViewUniform {
    view_projection: mat4x4<f32>,
};

struct StereoUniforms {
    views: array<ViewUniform, 2>,
};

@group(0) @binding(0)
var<uniform> stereo_uniforms: StereoUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vertex_main(input: VertexInput, @builtin(view_index) view_index: i32) -> VertexOutput {
    var out: VertexOutput;
    out.position = stereo_uniforms.views[u32(view_index)].view_projection
        * vec4<f32>(input.position, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
"#;

const WORLD_COLOR_VERTEX_FLOATS: usize = 7;
const WORLD_COLOR_VERTEX_SIZE: wgpu::BufferAddress =
    (WORLD_COLOR_VERTEX_FLOATS * std::mem::size_of::<f32>()) as wgpu::BufferAddress;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldColorVertex {
    pub position: Vec3,
    pub color: [f32; 4],
}

impl WorldColorVertex {
    pub const fn new(position: Vec3, color: [f32; 4]) -> Self {
        Self { position, color }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldColorMesh {
    vertices: Vec<WorldColorVertex>,
}

impl WorldColorMesh {
    pub fn new(vertices: Vec<WorldColorVertex>) -> Self {
        Self { vertices }
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn vertices(&self) -> &[WorldColorVertex] {
        &self.vertices
    }
}

pub struct WorldColorMeshRenderer {
    pipeline: wgpu::RenderPipeline,
    uniforms: PerViewUniformBuffer,
    bind_group: wgpu::BindGroup,
    color_format: wgpu::TextureFormat,
    multiview: RefCell<Option<WorldColorMeshMultiviewRenderer>>,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_buffer_slot_size: wgpu::BufferAddress,
}

impl WorldColorMeshRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_world_color_mesh_shader"),
            source: wgpu::ShaderSource::Wgsl(WORLD_COLOR_MESH_WGSL.into()),
        });
        let uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_world_color_mesh_uniforms",
            64,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_world_color_mesh_bind_group_layout"),
            entries: &[uniforms.layout_entry(0, wgpu::ShaderStages::VERTEX)],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_world_color_mesh_bind_group"),
            layout: &bind_group_layout,
            entries: &[uniforms.bind_group_entry(0)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_world_color_mesh_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = create_world_color_mesh_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_world_color_mesh_pipeline",
            None,
        );
        Self {
            pipeline,
            uniforms,
            bind_group,
            color_format,
            multiview: RefCell::new(None),
            vertex_buffer: None,
            vertex_buffer_slot_size: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_in_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        depth: &ChunkDepthTarget,
        render_view: ChunkRenderView,
        mesh: Option<&WorldColorMesh>,
        view_slot: PerViewSlot,
    ) {
        let Some(mesh) = mesh.filter(|mesh| !mesh.is_empty()) else {
            return;
        };
        let vertices = pack_vertices(mesh.vertices());
        let vertex_range = self.upload_vertices(device, queue, view_slot, &vertices);
        let uniform_offset =
            self.uniforms
                .write_slot(queue, view_slot, &matrix_bytes(render_view.view_projection));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_world_color_mesh_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[uniform_offset]);
        pass.set_vertex_buffer(
            0,
            self.vertex_buffer
                .as_ref()
                .expect("world color mesh vertex buffer exists")
                .slice(vertex_range),
        );
        pass.draw(0..mesh.vertex_count() as u32, 0..1);
    }

    pub fn render_multiview(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        depth: &ChunkMultiviewDepthTarget,
        render_views: [ChunkRenderView; 2],
        mesh: Option<&WorldColorMesh>,
    ) -> Result<()> {
        let Some(mesh) = mesh.filter(|mesh| !mesh.is_empty()) else {
            return Ok(());
        };
        let vertices = pack_vertices(mesh.vertices());
        let vertex_range = self.upload_vertices(device, queue, SINGLE_VIEW_SLOT, &vertices);
        let renderer = self.multiview_renderer(device)?;
        renderer.write_uniforms(queue, render_views);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_world_color_mesh_multiview_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&renderer.pipeline);
        pass.set_bind_group(0, &renderer.bind_group, &[]);
        pass.set_vertex_buffer(
            0,
            self.vertex_buffer
                .as_ref()
                .expect("world color mesh vertex buffer exists")
                .slice(vertex_range),
        );
        pass.draw(0..mesh.vertex_count() as u32, 0..1);
        Ok(())
    }

    fn multiview_renderer(
        &self,
        device: &wgpu::Device,
    ) -> Result<std::cell::Ref<'_, WorldColorMeshMultiviewRenderer>> {
        if !device.features().contains(wgpu::Features::MULTIVIEW) {
            bail!("world color mesh multiview render requires wgpu MULTIVIEW");
        }
        if self.multiview.borrow().is_none() {
            *self.multiview.borrow_mut() = Some(WorldColorMeshMultiviewRenderer::new(
                device,
                self.color_format,
            ));
        }
        Ok(std::cell::Ref::map(self.multiview.borrow(), |renderer| {
            renderer
                .as_ref()
                .expect("world color mesh multiview renderer initialized above")
        }))
    }

    fn upload_vertices(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view_slot: PerViewSlot,
        vertices: &[f32],
    ) -> Range<wgpu::BufferAddress> {
        let bytes = f32_bytes_vec(vertices);
        let required_slot_size = bytes.len().max(4) as wgpu::BufferAddress;
        let needs_recreate = self
            .vertex_buffer
            .as_ref()
            .is_none_or(|_| self.vertex_buffer_slot_size < required_slot_size);
        if needs_recreate {
            self.vertex_buffer_slot_size = required_slot_size;
            self.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_world_color_mesh_vertices"),
                size: self.vertex_buffer_slot_size
                    * PER_VIEW_UNIFORM_SLOT_COUNT as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let range = view_slot.byte_range(self.vertex_buffer_slot_size);
        if let Some(buffer) = &self.vertex_buffer {
            queue.write_buffer(buffer, range.start, &bytes);
        }
        range.start..range.start + bytes.len() as wgpu::BufferAddress
    }
}

struct WorldColorMeshMultiviewRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl WorldColorMeshMultiviewRenderer {
    fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_world_color_mesh_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(WORLD_COLOR_MESH_MULTIVIEW_WGSL.into()),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_world_color_mesh_multiview_uniforms"),
            size: 128,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_world_color_mesh_multiview_bind_group_layout"),
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_world_color_mesh_multiview_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_world_color_mesh_multiview_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = create_world_color_mesh_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_world_color_mesh_multiview_pipeline",
            NonZeroU32::new(2),
        );
        Self {
            pipeline,
            uniform_buffer,
            bind_group,
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

fn create_world_color_mesh_pipeline(
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
                array_stride: WORLD_COLOR_VERTEX_SIZE,
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
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::GreaterEqual,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview,
        cache: None,
    })
}

fn pack_vertices(vertices: &[WorldColorVertex]) -> Vec<f32> {
    let mut packed = Vec::with_capacity(vertices.len() * WORLD_COLOR_VERTEX_FLOATS);
    for vertex in vertices {
        packed.extend_from_slice(&[
            vertex.position.x,
            vertex.position.y,
            vertex.position.z,
            vertex.color[0],
            vertex.color[1],
            vertex.color[2],
            vertex.color[3],
        ]);
    }
    packed
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_world_positions_and_colors() {
        let vertices = [
            WorldColorVertex::new(Vec3::new(1.0, 2.0, 3.0), [0.1, 0.2, 0.3, 0.4]),
            WorldColorVertex::new(Vec3::new(-1.0, -2.0, -3.0), [0.5, 0.6, 0.7, 0.8]),
        ];

        assert_eq!(
            pack_vertices(&vertices),
            vec![
                1.0, 2.0, 3.0, 0.1, 0.2, 0.3, 0.4, -1.0, -2.0, -3.0, 0.5, 0.6, 0.7, 0.8
            ]
        );
    }

    #[test]
    fn serializes_distinct_multiview_matrices() {
        let left = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let right = Mat4::from_translation(Vec3::new(4.0, 5.0, 6.0));

        let bytes = multiview_matrix_bytes([left, right]);

        assert_eq!(&bytes[..64], matrix_bytes(left).as_slice());
        assert_eq!(&bytes[64..], matrix_bytes(right).as_slice());
        assert_ne!(&bytes[..64], &bytes[64..]);
    }
}
