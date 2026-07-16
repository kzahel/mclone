use std::num::NonZeroU64;

use anyhow::{Context, Result, bail};
use mclone_assets::{PreparedFigure, PreparedFigureVertex};

use crate::GpuPassId;
use crate::chunk::{ChunkRenderView, DEPTH_FORMAT, REVERSED_Z_DEPTH_CLEAR};
use crate::target::RenderFrameTarget;

const MAX_PREPARED_FIGURE_PARTS: usize = 64;
const VERTEX_BYTE_LEN: usize = 52;
const VERTEX_BYTE_SIZE: wgpu::BufferAddress = VERTEX_BYTE_LEN as wgpu::BufferAddress;
const VIEW_UNIFORM_BYTE_LEN: usize = 16 * std::mem::size_of::<f32>();
const VIEW_UNIFORM_BYTE_SIZE: wgpu::BufferAddress = VIEW_UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const PALETTE_FLOAT_COUNT: usize = MAX_PREPARED_FIGURE_PARTS * 16;
const PALETTE_BYTE_LEN: usize = PALETTE_FLOAT_COUNT * std::mem::size_of::<f32>();
const PALETTE_BYTE_SIZE: wgpu::BufferAddress = PALETTE_BYTE_LEN as wgpu::BufferAddress;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PreparedFigureRenderStats {
    pub draw_count: u32,
    pub vertex_count: u32,
    pub index_count: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PreparedFigureGpuSnapshot {
    pub immutable_upload_count: u64,
    pub immutable_vertex_bytes: u64,
    pub immutable_index_bytes: u64,
    pub immutable_atlas_bytes: u64,
    pub immutable_palette_bytes: u64,
    pub view_uniform_write_count: u64,
}

/// Immutable prepared-figure topology plus one mutable per-view uniform.
///
/// This proof renderer intentionally draws one resident figure and one static
/// rest palette. Actor records, animation updates, instancing, stereo, and
/// multiview are later contracts.
pub struct PreparedFigureDrawResources {
    pipeline: wgpu::RenderPipeline,
    view_uniform: wgpu::Buffer,
    view_bind_group: wgpu::BindGroup,
    _palette: wgpu::Buffer,
    palette_bind_group: wgpu::BindGroup,
    _texture: wgpu::Texture,
    _texture_view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    texture_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_count: u32,
    index_count: u32,
    snapshot: PreparedFigureGpuSnapshot,
}

impl PreparedFigureDrawResources {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        figure: &PreparedFigure,
    ) -> Result<Self> {
        if figure.parts.is_empty() || figure.vertices.is_empty() || figure.indices.is_empty() {
            bail!("prepared figure '{}' has no drawable geometry", figure.name);
        }
        if figure.parts.len() > MAX_PREPARED_FIGURE_PARTS {
            bail!(
                "prepared figure '{}' has {} parts; proof renderer limit is {}",
                figure.name,
                figure.parts.len(),
                MAX_PREPARED_FIGURE_PARTS
            );
        }
        let expected_atlas_bytes = figure.atlas.width as usize * figure.atlas.height as usize * 4;
        if figure.atlas.width == 0
            || figure.atlas.height == 0
            || figure.atlas.rgba.len() != expected_atlas_bytes
        {
            bail!(
                "prepared figure '{}' has invalid {}x{} atlas with {} bytes",
                figure.name,
                figure.atlas.width,
                figure.atlas.height,
                figure.atlas.rgba.len()
            );
        }

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_prepared_figure_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/prepared_figure.wgsl").into()),
        });
        let view_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_prepared_figure_view_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(VIEW_UNIFORM_BYTE_SIZE),
                },
                count: None,
            }],
        });
        let palette_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_prepared_figure_palette_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(PALETTE_BYTE_SIZE),
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_prepared_figure_texture_layout"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_prepared_figure_pipeline_layout"),
            bind_group_layouts: &[&view_layout, &palette_layout, &texture_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_prepared_figure_pipeline"),
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
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Uint32,
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
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let vertex_bytes = prepared_vertex_bytes(&figure.vertices);
        let index_bytes = prepared_index_bytes(&figure.indices);
        let palette_bytes = prepared_palette_bytes(figure)?;
        let vertex_buffer = upload_buffer(
            device,
            queue,
            "mclone_prepared_figure_vertices",
            wgpu::BufferUsages::VERTEX,
            &vertex_bytes,
        );
        let index_buffer = upload_buffer(
            device,
            queue,
            "mclone_prepared_figure_indices",
            wgpu::BufferUsages::INDEX,
            &index_bytes,
        );
        let palette = upload_buffer(
            device,
            queue,
            "mclone_prepared_figure_rest_palette",
            wgpu::BufferUsages::UNIFORM,
            &palette_bytes,
        );
        let view_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_prepared_figure_view_uniform"),
            size: VIEW_UNIFORM_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let view_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_prepared_figure_view_bind_group"),
            layout: &view_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: view_uniform.as_entire_binding(),
            }],
        });
        let palette_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_prepared_figure_palette_bind_group"),
            layout: &palette_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: palette.as_entire_binding(),
            }],
        });

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_prepared_figure_atlas"),
            size: wgpu::Extent3d {
                width: figure.atlas.width,
                height: figure.atlas.height,
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
                aspect: wgpu::TextureAspect::All,
            },
            &figure.atlas.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(figure.atlas.width * 4),
                rows_per_image: Some(figure.atlas.height),
            },
            wgpu::Extent3d {
                width: figure.atlas.width,
                height: figure.atlas.height,
                depth_or_array_layers: 1,
            },
        );
        let texture_view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mclone_prepared_figure_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_prepared_figure_texture_bind_group"),
            layout: &texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Ok(Self {
            pipeline,
            view_uniform,
            view_bind_group,
            _palette: palette,
            palette_bind_group,
            _texture: texture,
            _texture_view: texture_view,
            _sampler: sampler,
            texture_bind_group,
            vertex_buffer,
            index_buffer,
            vertex_count: figure.vertices.len() as u32,
            index_count: figure.indices.len() as u32,
            snapshot: PreparedFigureGpuSnapshot {
                immutable_upload_count: 4,
                immutable_vertex_bytes: vertex_bytes.len() as u64,
                immutable_index_bytes: index_bytes.len() as u64,
                immutable_atlas_bytes: figure.atlas.rgba.len() as u64,
                immutable_palette_bytes: palette_bytes.len() as u64,
                view_uniform_write_count: 0,
            },
        })
    }

    pub fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_view: ChunkRenderView,
    ) -> Result<PreparedFigureRenderStats> {
        let depth_view = target
            .depth_view
            .context("prepared figure render pass requires a depth attachment")?;
        queue.write_buffer(
            &self.view_uniform,
            0,
            &float_bytes(&render_view.view_projection.to_cols_array()),
        );
        self.snapshot.view_uniform_write_count =
            self.snapshot.view_uniform_write_count.saturating_add(1);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_prepared_figure_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: target.gpu_timestamp_writes(GpuPassId::Actor),
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.view_bind_group, &[]);
        pass.set_bind_group(1, &self.palette_bind_group, &[]);
        pass.set_bind_group(2, &self.texture_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
        Ok(PreparedFigureRenderStats {
            draw_count: 1,
            vertex_count: self.vertex_count,
            index_count: self.index_count,
        })
    }

    pub fn snapshot(&self) -> PreparedFigureGpuSnapshot {
        self.snapshot
    }
}

pub fn clear_prepared_figure_target(
    encoder: &mut wgpu::CommandEncoder,
    target: RenderFrameTarget<'_>,
    depth_view: &wgpu::TextureView,
    color: wgpu::Color,
) {
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("mclone_prepared_figure_clear_pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target.color_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(color),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(REVERSED_Z_DEPTH_CLEAR),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        ..Default::default()
    });
}

fn upload_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &'static str,
    usage: wgpu::BufferUsages,
    bytes: &[u8],
) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: bytes.len().max(4) as wgpu::BufferAddress,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    if !bytes.is_empty() {
        queue.write_buffer(&buffer, 0, bytes);
    }
    buffer
}

fn prepared_vertex_bytes(vertices: &[PreparedFigureVertex]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vertices.len() * VERTEX_BYTE_LEN);
    for vertex in vertices {
        push_f32s(&mut bytes, &vertex.position);
        push_f32s(&mut bytes, &vertex.normal);
        push_f32s(&mut bytes, &vertex.uv);
        push_f32s(&mut bytes, &vertex.color);
        bytes.extend_from_slice(&vertex.part_id.to_ne_bytes());
    }
    bytes
}

fn prepared_index_bytes(indices: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(indices));
    for index in indices {
        bytes.extend_from_slice(&index.to_ne_bytes());
    }
    bytes
}

fn prepared_palette_bytes(figure: &PreparedFigure) -> Result<Vec<u8>> {
    if figure.parts.len() > MAX_PREPARED_FIGURE_PARTS {
        bail!("prepared figure rest palette exceeds proof renderer limit");
    }
    let mut values = vec![0.0_f32; PALETTE_FLOAT_COUNT];
    for part_index in 0..MAX_PREPARED_FIGURE_PARTS {
        let matrix = figure
            .parts
            .get(part_index)
            .map(|part| part.rest_matrix)
            .unwrap_or_else(|| glam::Mat4::IDENTITY.to_cols_array_2d());
        for (column, values_column) in matrix.into_iter().enumerate() {
            let start = part_index * 16 + column * 4;
            values[start..start + 4].copy_from_slice(&values_column);
        }
    }
    Ok(float_bytes(&values))
}

fn float_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(values));
    push_f32s(&mut bytes, values);
    bytes
}

fn push_f32s(bytes: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_vertex_layout_is_52_bytes() {
        let vertex = PreparedFigureVertex {
            position: [1.0, 2.0, 3.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.25, 0.75],
            color: [1.0, 0.5, 0.25, 1.0],
            part_id: 7,
        };
        let bytes = prepared_vertex_bytes(&[vertex]);
        assert_eq!(bytes.len(), VERTEX_BYTE_LEN);
        assert_eq!(&bytes[48..52], &7_u32.to_ne_bytes());
    }
}
