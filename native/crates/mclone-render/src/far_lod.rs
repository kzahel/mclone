use glam::Mat4;

use crate::chunk::{ChunkDepthTarget, ChunkRenderView, DEPTH_FORMAT};
use crate::target::RenderFrameTarget;
use crate::uniform::{
    PER_VIEW_UNIFORM_SLOT_COUNT, PerViewSlot, PerViewUniformBuffer, SINGLE_VIEW_SLOT,
};

const FAR_TERRAIN_LOD_WGSL: &str = r#"
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

const FAR_TERRAIN_LOD_VERTEX_FLOATS: usize = 7;
const FAR_TERRAIN_LOD_VERTEX_SIZE: wgpu::BufferAddress =
    (FAR_TERRAIN_LOD_VERTEX_FLOATS * std::mem::size_of::<f32>()) as wgpu::BufferAddress;

#[derive(Clone, Debug, PartialEq)]
pub struct FarTerrainLodMesh {
    vertices: Vec<f32>,
    indices: Vec<u32>,
    revision: u64,
}

impl FarTerrainLodMesh {
    pub fn new(vertices: Vec<f32>, indices: Vec<u32>, revision: u64) -> Self {
        assert!(
            vertices.len() % FAR_TERRAIN_LOD_VERTEX_FLOATS == 0,
            "far terrain LOD vertex buffer must use position.xyz + color.rgba"
        );
        Self {
            vertices,
            indices,
            revision,
        }
    }

    pub fn empty(revision: u64) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            revision,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() || self.indices.is_empty()
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / FAR_TERRAIN_LOD_VERTEX_FLOATS
    }

    pub fn index_count(&self) -> usize {
        self.indices.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FarTerrainLodRenderStats {
    pub vertex_count: usize,
    pub index_count: usize,
    pub triangle_count: usize,
}

pub struct FarTerrainLodRenderer {
    pipeline: wgpu::RenderPipeline,
    uniforms: PerViewUniformBuffer,
    bind_group: wgpu::BindGroup,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_buffer_slot_size: wgpu::BufferAddress,
    index_buffer: Option<wgpu::Buffer>,
    index_buffer_size: wgpu::BufferAddress,
    uploaded_revision: Option<u64>,
    uploaded_index_count: u32,
    uploaded_stats: FarTerrainLodRenderStats,
}

impl FarTerrainLodRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_far_terrain_lod_shader"),
            source: wgpu::ShaderSource::Wgsl(FAR_TERRAIN_LOD_WGSL.into()),
        });
        let uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_far_terrain_lod_uniforms",
            64,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_far_terrain_lod_bind_group_layout"),
            entries: &[uniforms.layout_entry(0, wgpu::ShaderStages::VERTEX)],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_far_terrain_lod_bind_group"),
            layout: &bind_group_layout,
            entries: &[uniforms.bind_group_entry(0)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_far_terrain_lod_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_far_terrain_lod_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: FAR_TERRAIN_LOD_VERTEX_SIZE,
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
                module: &shader,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
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
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        Self {
            pipeline,
            uniforms,
            bind_group,
            vertex_buffer: None,
            vertex_buffer_slot_size: 0,
            index_buffer: None,
            index_buffer_size: 0,
            uploaded_revision: None,
            uploaded_index_count: 0,
            uploaded_stats: FarTerrainLodRenderStats::default(),
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        depth: &ChunkDepthTarget,
        render_view: ChunkRenderView,
        mesh: Option<&FarTerrainLodMesh>,
    ) -> FarTerrainLodRenderStats {
        self.render_in_slot(
            device,
            queue,
            encoder,
            target,
            depth,
            render_view,
            mesh,
            SINGLE_VIEW_SLOT,
        )
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
        mesh: Option<&FarTerrainLodMesh>,
        view_slot: PerViewSlot,
    ) -> FarTerrainLodRenderStats {
        self.upload_mesh(device, queue, view_slot, mesh);
        if self.uploaded_index_count == 0 {
            return FarTerrainLodRenderStats::default();
        }
        let uniform_offset =
            self.uniforms
                .write_slot(queue, view_slot, &matrix_bytes(render_view.view_projection));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_far_terrain_lod_pass"),
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
                    load: wgpu::LoadOp::Clear(1.0),
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
                .expect("far terrain LOD vertex buffer exists")
                .slice(view_slot.byte_range(self.vertex_buffer_slot_size)),
        );
        pass.set_index_buffer(
            self.index_buffer
                .as_ref()
                .expect("far terrain LOD index buffer exists")
                .slice(..),
            wgpu::IndexFormat::Uint32,
        );
        pass.draw_indexed(0..self.uploaded_index_count, 0, 0..1);
        self.uploaded_stats
    }

    fn upload_mesh(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view_slot: PerViewSlot,
        mesh: Option<&FarTerrainLodMesh>,
    ) {
        let Some(mesh) = mesh else {
            self.uploaded_revision = None;
            self.uploaded_index_count = 0;
            self.uploaded_stats = FarTerrainLodRenderStats::default();
            return;
        };
        if mesh.is_empty() {
            self.uploaded_revision = Some(mesh.revision());
            self.uploaded_index_count = 0;
            self.uploaded_stats = FarTerrainLodRenderStats::default();
            return;
        }
        if self.uploaded_revision == Some(mesh.revision()) {
            return;
        }

        let vertex_bytes = f32_bytes_vec(&mesh.vertices);
        let vertex_slot_size = vertex_bytes.len().max(4) as wgpu::BufferAddress;
        if self
            .vertex_buffer
            .as_ref()
            .is_none_or(|_| self.vertex_buffer_slot_size < vertex_slot_size)
        {
            self.vertex_buffer_slot_size = vertex_slot_size;
            self.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_far_terrain_lod_vertices"),
                size: self.vertex_buffer_slot_size
                    * PER_VIEW_UNIFORM_SLOT_COUNT as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let vertex_range = view_slot.byte_range(self.vertex_buffer_slot_size);
        if let Some(buffer) = &self.vertex_buffer {
            queue.write_buffer(buffer, vertex_range.start, &vertex_bytes);
        }

        let index_bytes = u32_bytes_vec(&mesh.indices);
        let index_size = index_bytes.len().max(4) as wgpu::BufferAddress;
        if self
            .index_buffer
            .as_ref()
            .is_none_or(|_| self.index_buffer_size < index_size)
        {
            self.index_buffer_size = index_size;
            self.index_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_far_terrain_lod_indices"),
                size: self.index_buffer_size,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(buffer) = &self.index_buffer {
            queue.write_buffer(buffer, 0, &index_bytes);
        }

        self.uploaded_revision = Some(mesh.revision());
        self.uploaded_index_count =
            u32::try_from(mesh.index_count()).expect("far terrain LOD index count fits u32");
        self.uploaded_stats = FarTerrainLodRenderStats {
            vertex_count: mesh.vertex_count(),
            index_count: mesh.index_count(),
            triangle_count: mesh.triangle_count(),
        };
    }
}

fn f32_bytes_vec(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(values));
    for value in values {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn u32_bytes_vec(values: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(values));
    for value in values {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn matrix_bytes(matrix: Mat4) -> [u8; 64] {
    let mut bytes = [0; 64];
    for (index, value) in matrix.to_cols_array().into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn far_terrain_lod_mesh_counts_vertices_indices_and_triangles() {
        let mesh = FarTerrainLodMesh::new(
            vec![
                0.0, 64.0, 0.0, 0.2, 0.8, 0.2, 1.0, 1.0, 64.0, 0.0, 0.2, 0.8, 0.2, 1.0, 0.0, 64.0,
                1.0, 0.2, 0.8, 0.2, 1.0,
            ],
            vec![0, 1, 2],
            7,
        );
        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.index_count(), 3);
        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.revision(), 7);
    }
}
