use std::num::{NonZeroU32, NonZeroU64};

use anyhow::{Context, Result, bail};
use mclone_assets::{
    PreparedFigure, PreparedFigurePass, PreparedFigurePassRange, PreparedFigureVertex,
};

use crate::GpuPassId;
use crate::chunk::{ChunkRenderView, DEPTH_FORMAT, REVERSED_Z_DEPTH_CLEAR};
use crate::target::RenderFrameTarget;
use crate::uniform::{
    PER_VIEW_UNIFORM_SLOT_COUNT, PerViewSlot, PerViewUniformBuffer, SINGLE_VIEW_SLOT,
};

const MAX_PREPARED_FIGURE_PARTS: usize = 64;
const VERTEX_BYTE_LEN: usize = 56;
const VERTEX_BYTE_SIZE: wgpu::BufferAddress = VERTEX_BYTE_LEN as wgpu::BufferAddress;
const VIEW_UNIFORM_BYTE_LEN: usize = 16 * std::mem::size_of::<f32>();
const VIEW_UNIFORM_BYTE_SIZE: wgpu::BufferAddress = VIEW_UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const MULTIVIEW_UNIFORM_BYTE_LEN: usize = VIEW_UNIFORM_BYTE_LEN * 2;
const MULTIVIEW_UNIFORM_BYTE_SIZE: wgpu::BufferAddress =
    MULTIVIEW_UNIFORM_BYTE_LEN as wgpu::BufferAddress;
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
    pub pipeline_count: u64,
    pub immutable_upload_count: u64,
    pub immutable_vertex_bytes: u64,
    pub immutable_index_bytes: u64,
    pub immutable_atlas_bytes: u64,
    pub immutable_palette_bytes: u64,
    pub palette_write_count: u64,
    pub palette_written_bytes: u64,
    pub view_uniform_write_count: u64,
    pub multiview_uniform_write_count: u64,
    pub multiview_pipeline_count: u64,
}

/// Immutable prepared-figure topology plus mutable palette/view uniforms.
///
/// This proof renderer intentionally draws one resident figure and one final
/// part palette. Ordinary stereo uses distinct live uniform slots; full-frame
/// stereo uses one two-view uniform and a matching multiview pipeline set.
pub struct PreparedFigureDrawResources {
    pipelines: PreparedFigurePipelines,
    view_uniforms: PerViewUniformBuffer,
    view_bind_group: wgpu::BindGroup,
    multiview: Option<PreparedFigureMultiviewResources>,
    palette: wgpu::Buffer,
    palette_bind_group: wgpu::BindGroup,
    palette_upload_scratch: Vec<u8>,
    _texture: wgpu::Texture,
    _texture_view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    texture_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    pass_ranges: Vec<PreparedFigurePassRange>,
    vertex_count: u32,
    index_count: u32,
    part_count: usize,
    snapshot: PreparedFigureGpuSnapshot,
}

struct PreparedFigureMultiviewResources {
    pipelines: PreparedFigurePipelines,
    view_uniform: wgpu::Buffer,
    view_bind_group: wgpu::BindGroup,
}

struct PreparedFigurePipelines {
    opaque: wgpu::RenderPipeline,
    mask_threshold: wgpu::RenderPipeline,
    mask_dither: wgpu::RenderPipeline,
    blend_depth: wgpu::RenderPipeline,
    blend_color: wgpu::RenderPipeline,
    additive: wgpu::RenderPipeline,
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
        validate_pass_ranges(&figure.pass_ranges, figure.indices.len() as u32)?;
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
        let view_uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_prepared_figure_view_uniforms",
            VIEW_UNIFORM_BYTE_SIZE,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let view_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_prepared_figure_view_layout"),
            entries: &[view_uniforms.layout_entry(0, wgpu::ShaderStages::VERTEX)],
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
        let pipelines =
            create_prepared_figure_pipelines(device, &pipeline_layout, &shader, color_format, None);
        let multiview = if device.features().contains(wgpu::Features::MULTIVIEW) {
            let multiview_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("mclone_prepared_figure_multiview_shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shaders/prepared_figure_multiview.wgsl").into(),
                ),
            });
            let view_uniform = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_prepared_figure_multiview_uniform"),
                size: MULTIVIEW_UNIFORM_BYTE_SIZE,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let view_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_prepared_figure_multiview_view_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(MULTIVIEW_UNIFORM_BYTE_SIZE),
                    },
                    count: None,
                }],
            });
            let view_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("mclone_prepared_figure_multiview_view_bind_group"),
                layout: &view_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view_uniform.as_entire_binding(),
                }],
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mclone_prepared_figure_multiview_pipeline_layout"),
                bind_group_layouts: &[&view_layout, &palette_layout, &texture_layout],
                push_constant_ranges: &[],
            });
            Some(PreparedFigureMultiviewResources {
                pipelines: create_prepared_figure_pipelines(
                    device,
                    &pipeline_layout,
                    &multiview_shader,
                    color_format,
                    NonZeroU32::new(2),
                ),
                view_uniform,
                view_bind_group,
            })
        } else {
            None
        };

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
        let view_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_prepared_figure_view_bind_group"),
            layout: &view_layout,
            entries: &[view_uniforms.bind_group_entry(0)],
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
            pipelines,
            view_uniforms,
            view_bind_group,
            multiview,
            palette,
            palette_bind_group,
            palette_upload_scratch: Vec::with_capacity(figure.parts.len() * 16 * 4),
            _texture: texture,
            _texture_view: texture_view,
            _sampler: sampler,
            texture_bind_group,
            vertex_buffer,
            index_buffer,
            pass_ranges: figure.pass_ranges.clone(),
            vertex_count: figure.vertices.len() as u32,
            index_count: figure.indices.len() as u32,
            part_count: figure.parts.len(),
            snapshot: PreparedFigureGpuSnapshot {
                pipeline_count: 6,
                immutable_upload_count: 4,
                immutable_vertex_bytes: vertex_bytes.len() as u64,
                immutable_index_bytes: index_bytes.len() as u64,
                immutable_atlas_bytes: figure.atlas.rgba.len() as u64,
                immutable_palette_bytes: palette_bytes.len() as u64,
                palette_write_count: 0,
                palette_written_bytes: 0,
                view_uniform_write_count: 0,
                multiview_uniform_write_count: 0,
                multiview_pipeline_count: if device.features().contains(wgpu::Features::MULTIVIEW) {
                    6
                } else {
                    0
                },
            },
        })
    }

    /// Replace the final actor-local part palette without touching resident
    /// topology or atlas resources.
    pub fn write_palette(&mut self, queue: &wgpu::Queue, palette: &[[[f32; 4]; 4]]) -> Result<()> {
        validate_palette(palette, self.part_count)?;
        self.palette_upload_scratch.clear();
        for matrix in palette {
            for column in matrix {
                push_f32s(&mut self.palette_upload_scratch, column);
            }
        }
        queue.write_buffer(&self.palette, 0, &self.palette_upload_scratch);
        self.snapshot.palette_write_count = self.snapshot.palette_write_count.saturating_add(1);
        self.snapshot.palette_written_bytes = self
            .snapshot
            .palette_written_bytes
            .saturating_add(self.palette_upload_scratch.len() as u64);
        Ok(())
    }

    pub fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_view: ChunkRenderView,
    ) -> Result<PreparedFigureRenderStats> {
        self.render_in_slot(queue, encoder, target, render_view, SINGLE_VIEW_SLOT)
    }

    pub fn render_in_slot(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_view: ChunkRenderView,
        view_slot: PerViewSlot,
    ) -> Result<PreparedFigureRenderStats> {
        let depth_view = target
            .depth_view
            .context("prepared figure render pass requires a depth attachment")?;
        let uniform_offset = self.view_uniforms.write_slot(
            queue,
            view_slot,
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
        pass.set_bind_group(0, &self.view_bind_group, &[uniform_offset]);
        pass.set_bind_group(1, &self.palette_bind_group, &[]);
        pass.set_bind_group(2, &self.texture_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        let draw_count = draw_prepared_figure_ranges(&mut pass, &self.pipelines, &self.pass_ranges);
        Ok(PreparedFigureRenderStats {
            draw_count,
            vertex_count: self.vertex_count,
            index_count: self.index_count,
        })
    }

    pub fn render_multiview(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_views: [ChunkRenderView; 2],
        clear_color: Option<wgpu::Color>,
    ) -> Result<PreparedFigureRenderStats> {
        let depth_view = target
            .depth_view
            .context("prepared figure multiview pass requires a depth attachment")?;
        let multiview = self
            .multiview
            .as_ref()
            .context("prepared figure multiview render requires wgpu MULTIVIEW")?;
        queue.write_buffer(
            &multiview.view_uniform,
            0,
            &multiview_view_bytes(render_views),
        );
        self.snapshot.multiview_uniform_write_count = self
            .snapshot
            .multiview_uniform_write_count
            .saturating_add(1);
        let color_load = clear_color
            .map(wgpu::LoadOp::Clear)
            .unwrap_or(wgpu::LoadOp::Load);
        let depth_load = if clear_color.is_some() {
            wgpu::LoadOp::Clear(REVERSED_Z_DEPTH_CLEAR)
        } else {
            wgpu::LoadOp::Load
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_prepared_figure_multiview_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: color_load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: depth_load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: target.gpu_timestamp_writes(GpuPassId::Actor),
            ..Default::default()
        });
        pass.set_bind_group(0, &multiview.view_bind_group, &[]);
        pass.set_bind_group(1, &self.palette_bind_group, &[]);
        pass.set_bind_group(2, &self.texture_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        let draw_count =
            draw_prepared_figure_ranges(&mut pass, &multiview.pipelines, &self.pass_ranges);
        Ok(PreparedFigureRenderStats {
            draw_count,
            vertex_count: self.vertex_count,
            index_count: self.index_count,
        })
    }

    pub fn multiview_supported(&self) -> bool {
        self.multiview.is_some()
    }

    pub fn snapshot(&self) -> PreparedFigureGpuSnapshot {
        self.snapshot
    }
}

fn validate_palette(palette: &[[[f32; 4]; 4]], expected_part_count: usize) -> Result<()> {
    if palette.len() != expected_part_count {
        bail!(
            "prepared figure palette has {} matrices; expected {}",
            palette.len(),
            expected_part_count
        );
    }
    if palette
        .iter()
        .flatten()
        .flatten()
        .any(|value| !value.is_finite())
    {
        bail!("prepared figure palette contains a non-finite matrix");
    }
    Ok(())
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

fn validate_pass_ranges(ranges: &[PreparedFigurePassRange], index_count: u32) -> Result<()> {
    let mut next_index = 0;
    let mut previous_pass = None;
    for range in ranges {
        if range.index_count == 0 || range.first_index != next_index {
            bail!("prepared figure has invalid or non-contiguous pass ranges");
        }
        if previous_pass.is_some_and(|previous| previous >= range.pass) {
            bail!("prepared figure pass ranges are not in canonical order");
        }
        next_index = next_index
            .checked_add(range.index_count)
            .context("prepared figure pass range overflow")?;
        previous_pass = Some(range.pass);
    }
    if next_index != index_count {
        bail!(
            "prepared figure pass ranges cover {} indices; expected {}",
            next_index,
            index_count
        );
    }
    Ok(())
}

fn draw_prepared_figure_ranges<'pass>(
    pass: &mut wgpu::RenderPass<'pass>,
    pipelines: &'pass PreparedFigurePipelines,
    ranges: &[PreparedFigurePassRange],
) -> u32 {
    let mut draw_count = 0;
    for range in ranges {
        let indices = range.first_index..range.first_index + range.index_count;
        match range.pass {
            PreparedFigurePass::Opaque => pass.set_pipeline(&pipelines.opaque),
            PreparedFigurePass::MaskThreshold => pass.set_pipeline(&pipelines.mask_threshold),
            PreparedFigurePass::MaskDither => pass.set_pipeline(&pipelines.mask_dither),
            PreparedFigurePass::Blend => {
                pass.set_pipeline(&pipelines.blend_depth);
                pass.draw_indexed(indices.clone(), 0, 0..1);
                draw_count += 1;
                pass.set_pipeline(&pipelines.blend_color);
            }
            PreparedFigurePass::Additive => pass.set_pipeline(&pipelines.additive),
        }
        pass.draw_indexed(indices, 0, 0..1);
        draw_count += 1;
    }
    draw_count
}

#[derive(Clone, Copy)]
enum PreparedFigurePipelineKind {
    Opaque,
    MaskThreshold,
    MaskDither,
    BlendDepth,
    BlendColor,
    Additive,
}

fn create_prepared_figure_pipelines(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    multiview: Option<NonZeroU32>,
) -> PreparedFigurePipelines {
    let create = |kind, label| {
        create_prepared_figure_pipeline(
            device,
            layout,
            shader,
            color_format,
            label,
            multiview,
            kind,
        )
    };
    PreparedFigurePipelines {
        opaque: create(
            PreparedFigurePipelineKind::Opaque,
            "mclone_prepared_figure_opaque",
        ),
        mask_threshold: create(
            PreparedFigurePipelineKind::MaskThreshold,
            "mclone_prepared_figure_mask_threshold",
        ),
        mask_dither: create(
            PreparedFigurePipelineKind::MaskDither,
            "mclone_prepared_figure_mask_dither",
        ),
        blend_depth: create(
            PreparedFigurePipelineKind::BlendDepth,
            "mclone_prepared_figure_blend_depth",
        ),
        blend_color: create(
            PreparedFigurePipelineKind::BlendColor,
            "mclone_prepared_figure_blend_color",
        ),
        additive: create(
            PreparedFigurePipelineKind::Additive,
            "mclone_prepared_figure_additive",
        ),
    }
}

fn create_prepared_figure_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    label: &'static str,
    multiview: Option<NonZeroU32>,
    kind: PreparedFigurePipelineKind,
) -> wgpu::RenderPipeline {
    let (fragment_entry, blend, write_mask, depth_write_enabled, depth_compare) = match kind {
        PreparedFigurePipelineKind::Opaque => (
            "fs_opaque",
            None,
            wgpu::ColorWrites::ALL,
            true,
            wgpu::CompareFunction::GreaterEqual,
        ),
        PreparedFigurePipelineKind::MaskThreshold => (
            "fs_mask_threshold",
            None,
            wgpu::ColorWrites::ALL,
            true,
            wgpu::CompareFunction::GreaterEqual,
        ),
        PreparedFigurePipelineKind::MaskDither => (
            "fs_mask_dither",
            None,
            wgpu::ColorWrites::ALL,
            true,
            wgpu::CompareFunction::GreaterEqual,
        ),
        PreparedFigurePipelineKind::BlendDepth => (
            "fs_blend_depth",
            None,
            wgpu::ColorWrites::empty(),
            true,
            wgpu::CompareFunction::GreaterEqual,
        ),
        PreparedFigurePipelineKind::BlendColor => (
            "fs_blend_color",
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            wgpu::ColorWrites::ALL,
            false,
            wgpu::CompareFunction::Equal,
        ),
        PreparedFigurePipelineKind::Additive => (
            "fs_additive",
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            }),
            wgpu::ColorWrites::ALL,
            false,
            wgpu::CompareFunction::GreaterEqual,
        ),
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
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
                    wgpu::VertexAttribute {
                        offset: 52,
                        shader_location: 5,
                        format: wgpu::VertexFormat::Float32,
                    },
                ],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment_entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend,
                write_mask,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled,
            depth_compare,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview,
        cache: None,
    })
}

fn multiview_view_bytes(render_views: [ChunkRenderView; 2]) -> [u8; MULTIVIEW_UNIFORM_BYTE_LEN] {
    let mut bytes = [0_u8; MULTIVIEW_UNIFORM_BYTE_LEN];
    for (view_index, render_view) in render_views.into_iter().enumerate() {
        let start = view_index * VIEW_UNIFORM_BYTE_LEN;
        let view_bytes = float_bytes(&render_view.view_projection.to_cols_array());
        bytes[start..start + VIEW_UNIFORM_BYTE_LEN].copy_from_slice(&view_bytes);
    }
    bytes
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
        bytes.extend_from_slice(&vertex.alpha_cutoff.to_ne_bytes());
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
    use crate::chunk::ChunkCamera;

    #[test]
    fn prepared_vertex_layout_is_56_bytes() {
        let vertex = PreparedFigureVertex {
            position: [1.0, 2.0, 3.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.25, 0.75],
            color: [1.0, 0.5, 0.25, 1.0],
            part_id: 7,
            alpha_cutoff: 0.35,
        };
        let bytes = prepared_vertex_bytes(&[vertex]);
        assert_eq!(bytes.len(), VERTEX_BYTE_LEN);
        assert_eq!(&bytes[48..52], &7_u32.to_ne_bytes());
        assert_eq!(&bytes[52..56], &0.35_f32.to_ne_bytes());
    }

    #[test]
    fn mutable_palette_requires_exact_finite_part_matrices() {
        let identity = glam::Mat4::IDENTITY.to_cols_array_2d();
        assert!(validate_palette(&[identity, identity], 2).is_ok());
        assert!(validate_palette(&[identity], 2).is_err());

        let mut non_finite = identity;
        non_finite[3][0] = f32::NAN;
        assert!(validate_palette(&[identity, non_finite], 2).is_err());
    }

    #[test]
    fn multiview_uniform_keeps_distinct_left_and_right_matrices() {
        let camera = |x| ChunkCamera {
            eye: [x, 0.5, 2.0],
            target: [x, 0.5, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 0.7,
            z_near: 0.01,
            z_far: 100.0,
        };
        let views = [
            camera(-0.02).render_view(360, 480),
            camera(0.02).render_view(360, 480),
        ];
        let bytes = multiview_view_bytes(views);
        assert_eq!(bytes.len(), MULTIVIEW_UNIFORM_BYTE_LEN);
        assert_ne!(
            &bytes[..VIEW_UNIFORM_BYTE_LEN],
            &bytes[VIEW_UNIFORM_BYTE_LEN..]
        );
    }

    #[test]
    fn prepared_multiview_shader_validates_with_multiview_capability() {
        let source = include_str!("shaders/prepared_figure_multiview.wgsl");
        let module = naga::front::wgsl::parse_str(source).expect("multiview WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::MULTIVIEW,
        )
        .validate(&module)
        .expect("multiview WGSL validates");
    }

    #[test]
    fn prepared_mono_shader_validates_all_material_entries() {
        let source = include_str!("shaders/prepared_figure.wgsl");
        let module = naga::front::wgsl::parse_str(source).expect("mono WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("mono WGSL validates");
    }

    #[test]
    fn prepared_figure_shaders_define_every_alpha_pass_for_mono_and_multiview() {
        for source in [
            include_str!("shaders/prepared_figure.wgsl"),
            include_str!("shaders/prepared_figure_multiview.wgsl"),
        ] {
            for entry in [
                "fs_opaque",
                "fs_mask_threshold",
                "fs_mask_dither",
                "fs_blend_depth",
                "fs_blend_color",
                "fs_additive",
            ] {
                assert!(source.contains(entry));
            }
            assert!(source.contains("coverage_hash"));
            assert!(source.contains("input.local_position"));
        }
    }
}
