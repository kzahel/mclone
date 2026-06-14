use anyhow::{Context, Result, bail};
use glam::{Mat4, Vec3};
use mclone_mesh::VisibleChunkMesh;
use wgpu::util::DeviceExt;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

const VERTEX_FLOAT_COUNT: usize = 7;
const VERTEX_BYTE_SIZE: wgpu::BufferAddress =
    (VERTEX_FLOAT_COUNT * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const UNIFORM_BYTE_SIZE: wgpu::BufferAddress = 64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkCamera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov_y_radians: f32,
    pub z_near: f32,
    pub z_far: f32,
}

impl ChunkCamera {
    pub fn overview_for_chunk(chunk_x: i32, chunk_z: i32) -> Self {
        let center_x = chunk_x as f32 * 16.0 + 8.0;
        let center_z = chunk_z as f32 * 16.0 + 8.0;
        Self {
            eye: [center_x + 54.0, 116.0, center_z - 66.0],
            target: [center_x, 48.0, center_z],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 56.0_f32.to_radians(),
            z_near: 0.1,
            z_far: 420.0,
        }
    }

    pub fn view_projection(self, width: u32, height: u32) -> [[f32; 4]; 4] {
        let aspect = width.max(1) as f32 / height.max(1) as f32;
        let view = Mat4::look_at_rh(
            Vec3::from_array(self.eye),
            Vec3::from_array(self.target),
            Vec3::from_array(self.up),
        );
        let projection = Mat4::perspective_rh(
            self.fov_y_radians,
            aspect.max(0.01),
            self.z_near,
            self.z_far,
        );
        (projection * view).to_cols_array_2d()
    }
}

pub struct GpuChunkMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl GpuChunkMesh {
    pub fn new(device: &wgpu::Device, mesh: &VisibleChunkMesh) -> Result<Self> {
        if mesh.is_empty() {
            bail!("cannot upload an empty chunk mesh");
        }
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_chunk_vertices"),
            contents: &vertex_bytes(mesh),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_chunk_indices"),
            contents: &index_bytes(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Ok(Self {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
        })
    }

    pub fn index_count(&self) -> u32 {
        self.index_count
    }
}

pub struct DepthTarget {
    _texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

impl DepthTarget {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_chunk_depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            _texture: texture,
            view,
            width,
            height,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width.max(1) && self.height == height.max(1) {
            return;
        }
        *self = Self::new(device, width, height);
    }
}

pub struct ChunkRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl ChunkRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_chunk_flat_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/chunk_flat.wgsl").into()),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_chunk_uniforms"),
            size: UNIFORM_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_chunk_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_chunk_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_chunk_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_chunk_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: VERTEX_BYTE_SIZE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
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
            uniform_buffer,
            bind_group,
        }
    }
}

pub struct ChunkDrawResources {
    renderer: ChunkRenderer,
    mesh: GpuChunkMesh,
    depth: DepthTarget,
}

impl ChunkDrawResources {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        mesh: &VisibleChunkMesh,
    ) -> Result<Self> {
        Ok(Self {
            renderer: ChunkRenderer::new(device, color_format),
            mesh: GpuChunkMesh::new(device, mesh).context("failed to upload chunk mesh")?,
            depth: DepthTarget::new(device, width, height),
        })
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.depth.resize(device, width, height);
    }

    pub fn index_count(&self) -> u32 {
        self.mesh.index_count()
    }

    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        size: [u32; 2],
        camera: ChunkCamera,
        clear_color: wgpu::Color,
    ) -> Result<()> {
        let view_projection = camera.view_projection(size[0], size[1]);
        queue.write_buffer(
            &self.renderer.uniform_buffer,
            0,
            &matrix_bytes(view_projection),
        );

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_chunk_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.renderer.pipeline);
        pass.set_bind_group(0, &self.renderer.bind_group, &[]);
        pass.set_vertex_buffer(0, self.mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(self.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.mesh.index_count, 0, 0..1);
        Ok(())
    }
}

fn vertex_bytes(mesh: &VisibleChunkMesh) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(mesh.vertices.len() * VERTEX_FLOAT_COUNT * 4);
    for vertex in &mesh.vertices {
        for value in vertex.position.into_iter().chain(vertex.color) {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
    bytes
}

fn index_bytes(indices: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(indices));
    for index in indices {
        bytes.extend_from_slice(&index.to_ne_bytes());
    }
    bytes
}

fn matrix_bytes(matrix: [[f32; 4]; 4]) -> [u8; 64] {
    let mut bytes = [0; 64];
    for (index, value) in matrix.into_iter().flatten().enumerate() {
        let start = index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_mesh::{ChunkMeshInput, build_visible_chunk_mesh};

    #[test]
    fn camera_matrix_serializes_to_uniform_size() {
        let matrix = ChunkCamera::overview_for_chunk(0, 0).view_projection(640, 480);
        assert_eq!(
            matrix_bytes(matrix).len() as wgpu::BufferAddress,
            UNIFORM_BYTE_SIZE
        );
        assert!(matrix.into_iter().flatten().all(f32::is_finite));
    }

    #[test]
    fn vertex_and_index_bytes_match_gpu_layout() {
        let mut blocks = vec![0; 16 * 16 * 16];
        blocks[0] = 1;
        let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

        assert_eq!(
            vertex_bytes(&mesh).len() as wgpu::BufferAddress,
            mesh.vertices.len() as wgpu::BufferAddress * VERTEX_BYTE_SIZE
        );
        assert_eq!(index_bytes(&mesh.indices).len(), mesh.indices.len() * 4);
    }
}
