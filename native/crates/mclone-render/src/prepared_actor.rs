use std::collections::{BTreeMap, BTreeSet};
use std::num::{NonZeroU32, NonZeroU64};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use anyhow::{Context, Result, bail};
use glam::{Mat4, Quat, Vec3};
use mclone_assets::{
    ActorFigureId, PreparedFigure, PreparedFigurePartRotationOverride, PreparedFigurePass,
    PreparedFigurePassRange, PreparedFigureVertex, chicken_figure_id,
    evaluate_prepared_figure_clip_into,
    evaluate_prepared_figure_clip_with_part_rotation_overrides_into,
    evaluate_prepared_figure_rest_pose_into,
};

use crate::GpuPassId;
use crate::asset_lab_figure::ActorFigureSet;
use crate::chunk::{ChunkRenderView, DEPTH_FORMAT, TexturedSectionRenderOptions};
use crate::entity::{
    ActorAnimationClip, ActorInstance, ActorInstanceId, ActorInstanceShape, ActorRenderStats,
};
use crate::placement::{CompositionClip, WorldCompositionContext};
use crate::target::RenderFrameTarget;
use crate::uniform::{PER_VIEW_UNIFORM_SLOT_COUNT, PerViewSlot, PerViewUniformBuffer};

const MAX_PREPARED_ACTOR_PARTS: usize = 64;
const VERTEX_BYTE_LEN: usize = 56;
const VERTEX_BYTE_SIZE: wgpu::BufferAddress = VERTEX_BYTE_LEN as wgpu::BufferAddress;
const VIEW_FLOAT_COUNT: usize = 48;
const VIEW_BYTE_LEN: usize = VIEW_FLOAT_COUNT * std::mem::size_of::<f32>();
const VIEW_BYTE_SIZE: wgpu::BufferAddress = VIEW_BYTE_LEN as wgpu::BufferAddress;
const MULTIVIEW_BYTE_LEN: usize = VIEW_BYTE_LEN * 2;
const MULTIVIEW_BYTE_SIZE: wgpu::BufferAddress = MULTIVIEW_BYTE_LEN as wgpu::BufferAddress;
const ACTOR_INSTANCE_BYTE_LEN: usize = 64;
const ACTOR_INSTANCE_BYTE_SIZE: wgpu::BufferAddress =
    ACTOR_INSTANCE_BYTE_LEN as wgpu::BufferAddress;
const PALETTE_TEXELS_PER_MATRIX: usize = 3;
#[cfg(test)]
const PALETTE_MATRIX_BYTE_LEN: usize = PALETTE_TEXELS_PER_MATRIX * 4 * std::mem::size_of::<f32>();
const PALETTE_TEXEL_BYTE_LEN: usize = 4 * std::mem::size_of::<f32>();

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PreparedActorSharedSnapshot {
    pub figure_count: usize,
    pub immutable_upload_count: u64,
    pub immutable_vertex_bytes: u64,
    pub immutable_index_bytes: u64,
    pub immutable_atlas_bytes: u64,
    pub multiview_pipeline_count: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PreparedActorDrawSnapshot {
    pub actor_record_count: usize,
    pub prepared_actor_count: usize,
    pub legacy_actor_count: usize,
    pub unchanged_actor_reuse_count: u64,
    pub pose_evaluation_count: u64,
    pub palette_write_count: u64,
    pub palette_written_bytes: u64,
    pub actor_write_count: u64,
    pub actor_written_bytes: u64,
    pub view_write_count: u64,
    pub multiview_view_write_count: u64,
    pub draw_count: u64,
    pub drawn_instance_count: u64,
    pub max_instances_per_draw: u32,
    pub instance_bucket_count: usize,
    pub actor_buffer_reallocation_count: u64,
    pub palette_texture_reallocation_count: u64,
    pub prepare_count: u64,
    pub prepare_evaluation_ns: u64,
    pub prepare_upload_ns: u64,
    pub mutable_known_allocated_bytes: u64,
}

pub(crate) struct PreparedActorSharedResources {
    pipelines: PreparedActorPipelines,
    multiview_pipelines: Option<PreparedActorPipelines>,
    view_layout: wgpu::BindGroupLayout,
    multiview_view_layout: Option<wgpu::BindGroupLayout>,
    palette_layout: wgpu::BindGroupLayout,
    figures: BTreeMap<ActorFigureId, PreparedActorFigureResources>,
    snapshot: PreparedActorSharedSnapshot,
}

struct PreparedActorFigureResources {
    figure: PreparedFigure,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    _texture: wgpu::Texture,
    _texture_view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    texture_bind_group: wgpu::BindGroup,
    vertex_count: u32,
    index_count: u32,
    pass_ranges: Vec<PreparedFigurePassRange>,
    wing_part_ids: Option<[u16; 2]>,
}

pub(crate) struct PreparedActorDrawResources {
    views: PerViewUniformBuffer,
    view_bind_group: wgpu::BindGroup,
    multiview: Option<PreparedActorMultiviewDrawResources>,
    records: BTreeMap<ActorInstanceId, PreparedActorRecord>,
    buckets: BTreeMap<ActorFigureId, PreparedActorInstanceBucket>,
    legacy_actors: Vec<ActorInstance>,
    retained_ids: BTreeSet<ActorInstanceId>,
    snapshot: PreparedActorDrawSnapshot,
}

struct PreparedActorMultiviewDrawResources {
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

struct PreparedActorRecord {
    figure_id: ActorFigureId,
    last_input: Option<ActorInstance>,
    pose_palette: Vec<[[f32; 4]; 4]>,
    model: Mat4,
    packed_light: u32,
    opacity: f32,
}

struct PreparedActorInstanceBucket {
    figure_id: ActorFigureId,
    part_count: usize,
    order: Vec<ActorInstanceId>,
    next_order: Vec<ActorInstanceId>,
    actor_buffer: wgpu::Buffer,
    actor_capacity_bytes: u64,
    actor_upload_scratch: Vec<u8>,
    palette_texture: wgpu::Texture,
    palette_texture_view: wgpu::TextureView,
    palette_bind_group: wgpu::BindGroup,
    palette_dimensions: [u32; 2],
    palette_upload_scratch: Vec<u8>,
    max_instances: usize,
}

struct PreparedActorPipelines {
    opaque: wgpu::RenderPipeline,
    mask_threshold: wgpu::RenderPipeline,
    mask_dither: wgpu::RenderPipeline,
    blend_depth: wgpu::RenderPipeline,
    blend_color: wgpu::RenderPipeline,
    additive: wgpu::RenderPipeline,
}

impl PreparedActorSharedResources {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        figures: &ActorFigureSet,
    ) -> Result<Self> {
        let view_layout = uniform_layout(
            device,
            "mclone_prepared_actor_view_layout",
            VIEW_BYTE_SIZE,
            true,
        );
        let multiview_view_layout =
            device
                .features()
                .contains(wgpu::Features::MULTIVIEW)
                .then(|| {
                    uniform_layout(
                        device,
                        "mclone_prepared_actor_multiview_view_layout",
                        MULTIVIEW_BYTE_SIZE,
                        false,
                    )
                });
        let palette_layout = palette_texture_layout(device);
        let texture_layout = texture_layout(device);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_prepared_actor_shader"),
            source: wgpu::ShaderSource::Wgsl(
                crate::fog::inject_fog_wgsl(include_str!("shaders/prepared_actor.wgsl")).into(),
            ),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_prepared_actor_pipeline_layout"),
            bind_group_layouts: &[&view_layout, &palette_layout, &texture_layout],
            push_constant_ranges: &[],
        });
        let pipelines = create_pipelines(device, &pipeline_layout, &shader, color_format, None);
        let multiview_pipelines = multiview_view_layout.as_ref().map(|multiview_view_layout| {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("mclone_prepared_actor_multiview_shader"),
                source: wgpu::ShaderSource::Wgsl(
                    crate::fog::inject_fog_wgsl(include_str!(
                        "shaders/prepared_actor_multiview.wgsl"
                    ))
                    .into(),
                ),
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mclone_prepared_actor_multiview_pipeline_layout"),
                bind_group_layouts: &[multiview_view_layout, &palette_layout, &texture_layout],
                push_constant_ranges: &[],
            });
            create_pipelines(device, &layout, &shader, color_format, NonZeroU32::new(2))
        });

        let mut gpu_figures = BTreeMap::new();
        let mut snapshot = PreparedActorSharedSnapshot {
            multiview_pipeline_count: if multiview_pipelines.is_some() { 6 } else { 0 },
            ..PreparedActorSharedSnapshot::default()
        };
        for (id, figure) in figures.prepared_figures() {
            let resources = PreparedActorFigureResources::new(
                device,
                queue,
                &texture_layout,
                id,
                figure,
                &mut snapshot,
            )?;
            gpu_figures.insert(id, resources);
        }
        snapshot.figure_count = gpu_figures.len();
        Ok(Self {
            pipelines,
            multiview_pipelines,
            view_layout,
            multiview_view_layout,
            palette_layout,
            figures: gpu_figures,
            snapshot,
        })
    }

    pub(crate) const fn snapshot(&self) -> PreparedActorSharedSnapshot {
        self.snapshot
    }
}

impl PreparedActorFigureResources {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_layout: &wgpu::BindGroupLayout,
        id: ActorFigureId,
        figure: &PreparedFigure,
        snapshot: &mut PreparedActorSharedSnapshot,
    ) -> Result<Self> {
        if figure.parts.is_empty() || figure.vertices.is_empty() || figure.indices.is_empty() {
            bail!(
                "prepared actor figure {} has no drawable geometry",
                id.as_str()
            );
        }
        if figure.parts.len() > MAX_PREPARED_ACTOR_PARTS {
            bail!(
                "prepared actor figure {} has {} parts; runtime limit is {}",
                id.as_str(),
                figure.parts.len(),
                MAX_PREPARED_ACTOR_PARTS
            );
        }
        validate_pass_ranges(&figure.pass_ranges, figure.indices.len() as u32)?;
        let expected_atlas_bytes = figure.atlas.width as usize * figure.atlas.height as usize * 4;
        if figure.atlas.width == 0
            || figure.atlas.height == 0
            || figure.atlas.rgba.len() != expected_atlas_bytes
        {
            bail!("prepared actor figure {} has an invalid atlas", id.as_str());
        }
        let vertex_bytes = vertex_bytes(&figure.vertices);
        let index_bytes = index_bytes(&figure.indices);
        let vertex_buffer = upload_buffer(
            device,
            queue,
            "mclone_prepared_actor_vertices",
            wgpu::BufferUsages::VERTEX,
            &vertex_bytes,
        );
        let index_buffer = upload_buffer(
            device,
            queue,
            "mclone_prepared_actor_indices",
            wgpu::BufferUsages::INDEX,
            &index_bytes,
        );
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_prepared_actor_atlas"),
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
            label: Some("mclone_prepared_actor_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_prepared_actor_texture_bind_group"),
            layout: texture_layout,
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
        snapshot.immutable_upload_count = snapshot.immutable_upload_count.saturating_add(3);
        snapshot.immutable_vertex_bytes = snapshot
            .immutable_vertex_bytes
            .saturating_add(vertex_bytes.len() as u64);
        snapshot.immutable_index_bytes = snapshot
            .immutable_index_bytes
            .saturating_add(index_bytes.len() as u64);
        snapshot.immutable_atlas_bytes = snapshot
            .immutable_atlas_bytes
            .saturating_add(figure.atlas.rgba.len() as u64);
        let wing_part_ids = if id == chicken_figure_id() {
            let left = figure.parts.iter().position(|part| part.name == "wing_l");
            let right = figure.parts.iter().position(|part| part.name == "wing_r");
            left.zip(right)
                .map(|(left, right)| [left as u16, right as u16])
        } else {
            None
        };
        Ok(Self {
            figure: figure.clone(),
            vertex_buffer,
            index_buffer,
            _texture: texture,
            _texture_view: texture_view,
            _sampler: sampler,
            texture_bind_group,
            vertex_count: figure.vertices.len() as u32,
            index_count: figure.indices.len() as u32,
            pass_ranges: figure.pass_ranges.clone(),
            wing_part_ids,
        })
    }
}

impl PreparedActorDrawResources {
    pub(crate) fn new(device: &wgpu::Device, shared: &PreparedActorSharedResources) -> Self {
        let views = PerViewUniformBuffer::new(
            device,
            "mclone_prepared_actor_views",
            VIEW_BYTE_SIZE,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let view_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_prepared_actor_view_bind_group"),
            layout: &shared.view_layout,
            entries: &[views.bind_group_entry(0)],
        });
        let multiview = shared
            .multiview_view_layout
            .as_ref()
            .map(|layout| PreparedActorMultiviewDrawResources::new(device, layout));
        Self {
            views,
            view_bind_group,
            multiview,
            records: BTreeMap::new(),
            buckets: BTreeMap::new(),
            legacy_actors: Vec::new(),
            retained_ids: BTreeSet::new(),
            snapshot: PreparedActorDrawSnapshot::default(),
        }
    }

    pub(crate) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shared: &PreparedActorSharedResources,
        actors: &[ActorInstance],
    ) {
        #[cfg(not(target_arch = "wasm32"))]
        let evaluation_start = Instant::now();
        self.legacy_actors.clear();
        self.retained_ids.clear();
        for bucket in self.buckets.values_mut() {
            bucket.next_order.clear();
        }
        let mut dirty_figures = BTreeSet::new();
        for actor in actors {
            let Some((id, figure_id)) = prepared_actor_key(*actor) else {
                self.legacy_actors.push(*actor);
                continue;
            };
            let Some(figure) = shared.figures.get(&figure_id) else {
                self.legacy_actors.push(*actor);
                continue;
            };
            if !self.retained_ids.insert(id) {
                self.legacy_actors.push(*actor);
                continue;
            }
            let bucket = self.buckets.entry(figure_id).or_insert_with(|| {
                PreparedActorInstanceBucket::new(
                    device,
                    &shared.palette_layout,
                    figure_id,
                    figure.figure.parts.len(),
                )
            });
            if bucket.next_order.len() >= bucket.max_instances {
                self.legacy_actors.push(*actor);
                self.retained_ids.remove(&id);
                continue;
            }
            let record = self
                .records
                .entry(id)
                .or_insert_with(|| PreparedActorRecord::new(figure_id, figure.figure.parts.len()));
            if record.figure_id != figure_id {
                *record = PreparedActorRecord::new(figure_id, figure.figure.parts.len());
            }
            if record.last_input == Some(*actor) {
                self.snapshot.unchanged_actor_reuse_count =
                    self.snapshot.unchanged_actor_reuse_count.saturating_add(1);
            } else {
                if record.evaluate(figure, *actor, &mut self.snapshot).is_err() {
                    self.legacy_actors.push(*actor);
                    self.retained_ids.remove(&id);
                    continue;
                }
                record.last_input = Some(*actor);
                dirty_figures.insert(figure_id);
            }
            bucket.next_order.push(id);
        }
        self.records.retain(|id, _| self.retained_ids.contains(id));
        self.buckets.retain(|figure_id, bucket| {
            if bucket.order != bucket.next_order {
                dirty_figures.insert(*figure_id);
            }
            std::mem::swap(&mut bucket.order, &mut bucket.next_order);
            !bucket.order.is_empty()
        });
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.snapshot.prepare_evaluation_ns = self
                .snapshot
                .prepare_evaluation_ns
                .saturating_add(elapsed_nanos(evaluation_start));
        }
        #[cfg(not(target_arch = "wasm32"))]
        let upload_start = Instant::now();
        for figure_id in dirty_figures {
            if let Some(bucket) = self.buckets.get_mut(&figure_id) {
                bucket.upload(
                    device,
                    queue,
                    &shared.palette_layout,
                    &self.records,
                    &mut self.snapshot,
                );
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.snapshot.prepare_upload_ns = self
                .snapshot
                .prepare_upload_ns
                .saturating_add(elapsed_nanos(upload_start));
        }
        self.snapshot.prepare_count = self.snapshot.prepare_count.saturating_add(1);
        self.snapshot.actor_record_count = self.records.len();
        self.snapshot.prepared_actor_count =
            self.buckets.values().map(|bucket| bucket.order.len()).sum();
        self.snapshot.legacy_actor_count = self.legacy_actors.len();
        self.snapshot.instance_bucket_count = self.buckets.len();
        self.snapshot.mutable_known_allocated_bytes = self
            .views
            .allocated_byte_size()
            .saturating_add(if self.multiview.is_some() {
                MULTIVIEW_BYTE_SIZE
            } else {
                0
            })
            .saturating_add(
                self.buckets
                    .values()
                    .map(PreparedActorInstanceBucket::allocated_byte_size)
                    .sum::<u64>(),
            );
    }

    pub(crate) fn legacy_actors(&self) -> &[ActorInstance] {
        &self.legacy_actors
    }

    pub(crate) fn prepared_input_count(&self) -> usize {
        self.buckets
            .values()
            .map(|bucket| bucket.order.len())
            .sum::<usize>()
            + self.legacy_actors.len()
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_in_slot(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_view: ChunkRenderView,
        render_options: TexturedSectionRenderOptions,
        context: Option<WorldCompositionContext>,
        view_slot: PerViewSlot,
        shared: &PreparedActorSharedResources,
    ) -> Result<ActorRenderStats> {
        if self.buckets.is_empty() {
            return Ok(ActorRenderStats::default());
        }
        let depth_view = target
            .depth_view
            .context("prepared actor render pass requires a depth attachment")?;
        let offset = self.views.write_slot(
            queue,
            view_slot,
            &view_bytes(render_view, render_options, context),
        );
        self.snapshot.view_write_count = self.snapshot.view_write_count.saturating_add(1);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_prepared_actor_render_pass"),
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
        pass.set_bind_group(0, &self.view_bind_group, &[offset]);
        let stats = draw_buckets(&mut pass, &shared.pipelines, &self.buckets, &shared.figures);
        self.snapshot.draw_count = self.snapshot.draw_count.saturating_add(stats.draw_count);
        self.snapshot.drawn_instance_count = self
            .snapshot
            .drawn_instance_count
            .saturating_add(stats.drawn_instance_count);
        self.snapshot.max_instances_per_draw = self
            .snapshot
            .max_instances_per_draw
            .max(stats.max_instances_per_draw);
        Ok(stats.actor_stats())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_multiview(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_views: [ChunkRenderView; 2],
        render_options: [TexturedSectionRenderOptions; 2],
        context: Option<WorldCompositionContext>,
        shared: &PreparedActorSharedResources,
    ) -> Result<ActorRenderStats> {
        if self.buckets.is_empty() {
            return Ok(ActorRenderStats::default());
        }
        let depth_view = target
            .depth_view
            .context("prepared actor multiview pass requires a depth attachment")?;
        let pipelines = shared
            .multiview_pipelines
            .as_ref()
            .context("prepared actor multiview requires wgpu MULTIVIEW")?;
        let multiview = self
            .multiview
            .as_ref()
            .context("prepared actor multiview state is unavailable")?;
        queue.write_buffer(
            &multiview.uniform,
            0,
            &multiview_view_bytes(render_views, render_options, context),
        );
        self.snapshot.multiview_view_write_count =
            self.snapshot.multiview_view_write_count.saturating_add(1);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_prepared_actor_multiview_render_pass"),
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
        pass.set_bind_group(0, &multiview.bind_group, &[]);
        let stats = draw_buckets(&mut pass, pipelines, &self.buckets, &shared.figures);
        self.snapshot.draw_count = self.snapshot.draw_count.saturating_add(stats.draw_count);
        self.snapshot.drawn_instance_count = self
            .snapshot
            .drawn_instance_count
            .saturating_add(stats.drawn_instance_count);
        self.snapshot.max_instances_per_draw = self
            .snapshot
            .max_instances_per_draw
            .max(stats.max_instances_per_draw);
        Ok(stats.actor_stats())
    }

    pub(crate) const fn snapshot(&self) -> PreparedActorDrawSnapshot {
        self.snapshot
    }
}

impl PreparedActorMultiviewDrawResources {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_prepared_actor_multiview_view_uniform"),
            size: MULTIVIEW_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_prepared_actor_multiview_view_bind_group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        Self {
            uniform,
            bind_group,
        }
    }
}

impl PreparedActorRecord {
    fn new(figure_id: ActorFigureId, part_count: usize) -> Self {
        Self {
            figure_id,
            last_input: None,
            pose_palette: Vec::with_capacity(part_count),
            model: Mat4::IDENTITY,
            packed_light: 0,
            opacity: 1.0,
        }
    }

    fn evaluate(
        &mut self,
        figure: &PreparedActorFigureResources,
        actor: ActorInstance,
        snapshot: &mut PreparedActorDrawSnapshot,
    ) -> Result<()> {
        let animation_time = actor.animation.and_then(|animation| {
            let clip_name = match animation.clip {
                ActorAnimationClip::Walk => "walk",
            };
            figure.figure.clips.get(clip_name).map(|clip| {
                let distance = animation.distance.max(0.0);
                clip.locomotion
                    .as_ref()
                    .map_or(f64::from(distance), |locomotion| {
                        f64::from(
                            distance / locomotion.cycle_distance.max(f32::EPSILON)
                                * clip.duration_seconds,
                        )
                    })
            })
        });
        match animation_time {
            Some(time) => {
                if let (Some(wing_flap), Some([left, right])) =
                    (actor.chicken_wing_flap_radians, figure.wing_part_ids)
                {
                    let overrides = [
                        PreparedFigurePartRotationOverride {
                            part_id: left,
                            rotation_delta_radians: [0.0, 0.0, wing_flap],
                        },
                        PreparedFigurePartRotationOverride {
                            part_id: right,
                            rotation_delta_radians: [0.0, 0.0, -wing_flap],
                        },
                    ];
                    evaluate_prepared_figure_clip_with_part_rotation_overrides_into(
                        &figure.figure,
                        "walk",
                        time,
                        &overrides,
                        &mut self.pose_palette,
                    )?;
                } else {
                    evaluate_prepared_figure_clip_into(
                        &figure.figure,
                        "walk",
                        time,
                        &mut self.pose_palette,
                    )?;
                }
            }
            None => {
                evaluate_prepared_figure_rest_pose_into(&figure.figure, &mut self.pose_palette)?
            }
        }
        let model = actor_model_matrix(actor, &figure.figure)
            .context("prepared semantic figure has invalid transform")?;
        if !actor.opacity.is_finite() || !(0.0..=1.0).contains(&actor.opacity) {
            bail!("prepared actor has invalid opacity");
        }
        self.model = model;
        self.packed_light = actor.packed_light;
        self.opacity = actor.opacity;
        snapshot.pose_evaluation_count = snapshot.pose_evaluation_count.saturating_add(1);
        Ok(())
    }
}

impl PreparedActorInstanceBucket {
    fn new(
        device: &wgpu::Device,
        palette_layout: &wgpu::BindGroupLayout,
        figure_id: ActorFigureId,
        part_count: usize,
    ) -> Self {
        let actor_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_prepared_actor_instances"),
            size: ACTOR_INSTANCE_BYTE_SIZE,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let palette_texture = create_palette_texture(device, [4, 1]);
        let palette_texture_view = palette_texture.create_view(&Default::default());
        let palette_bind_group =
            create_palette_bind_group(device, palette_layout, &palette_texture_view);
        let max_texture_dimension = device.limits().max_texture_dimension_2d as usize;
        let max_palette_instances = max_texture_dimension
            .saturating_mul(max_texture_dimension)
            .checked_div(part_count.saturating_mul(PALETTE_TEXELS_PER_MATRIX))
            .unwrap_or(0);
        let max_buffer_instances =
            usize::try_from(device.limits().max_buffer_size / ACTOR_INSTANCE_BYTE_SIZE)
                .unwrap_or(usize::MAX);
        Self {
            figure_id,
            part_count,
            order: Vec::new(),
            next_order: Vec::new(),
            actor_buffer,
            actor_capacity_bytes: ACTOR_INSTANCE_BYTE_SIZE,
            actor_upload_scratch: Vec::new(),
            palette_texture,
            palette_texture_view,
            palette_bind_group,
            palette_dimensions: [4, 1],
            palette_upload_scratch: Vec::new(),
            max_instances: max_palette_instances
                .min(max_buffer_instances)
                .min(u32::MAX as usize),
        }
    }

    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        palette_layout: &wgpu::BindGroupLayout,
        records: &BTreeMap<ActorInstanceId, PreparedActorRecord>,
        snapshot: &mut PreparedActorDrawSnapshot,
    ) {
        let instance_count = self.order.len();
        let required_actor_bytes = instance_count.saturating_mul(ACTOR_INSTANCE_BYTE_LEN);
        if required_actor_bytes as u64 > self.actor_capacity_bytes {
            self.actor_capacity_bytes = (required_actor_bytes as u64)
                .next_power_of_two()
                .min(device.limits().max_buffer_size);
            self.actor_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_prepared_actor_instances"),
                size: self.actor_capacity_bytes,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            snapshot.actor_buffer_reallocation_count =
                snapshot.actor_buffer_reallocation_count.saturating_add(1);
        }

        let required_palette_texels = instance_count
            .saturating_mul(self.part_count)
            .saturating_mul(PALETTE_TEXELS_PER_MATRIX);
        let palette_capacity_texels =
            self.palette_dimensions[0] as usize * self.palette_dimensions[1] as usize;
        if required_palette_texels > palette_capacity_texels {
            self.palette_dimensions = palette_texture_dimensions(
                required_palette_texels,
                device.limits().max_texture_dimension_2d,
            );
            self.palette_texture = create_palette_texture(device, self.palette_dimensions);
            self.palette_texture_view = self.palette_texture.create_view(&Default::default());
            self.palette_bind_group =
                create_palette_bind_group(device, palette_layout, &self.palette_texture_view);
            snapshot.palette_texture_reallocation_count = snapshot
                .palette_texture_reallocation_count
                .saturating_add(1);
        }

        self.actor_upload_scratch.clear();
        self.actor_upload_scratch.reserve(required_actor_bytes);
        self.palette_upload_scratch.clear();
        for (instance_index, id) in self.order.iter().enumerate() {
            let record = records
                .get(id)
                .expect("prepared actor bucket record remains resident");
            debug_assert_eq!(record.figure_id, self.figure_id);
            debug_assert_eq!(record.pose_palette.len(), self.part_count);
            push_actor_instance_bytes(
                &mut self.actor_upload_scratch,
                record.model,
                record.packed_light,
                record.opacity,
                instance_index.saturating_mul(self.part_count) as u32,
            );
            for matrix in &record.pose_palette {
                push_affine_matrix_texels(&mut self.palette_upload_scratch, matrix);
            }
        }
        if !self.actor_upload_scratch.is_empty() {
            queue.write_buffer(&self.actor_buffer, 0, &self.actor_upload_scratch);
            snapshot.actor_write_count = snapshot.actor_write_count.saturating_add(1);
            snapshot.actor_written_bytes = snapshot
                .actor_written_bytes
                .saturating_add(self.actor_upload_scratch.len() as u64);
        }
        if !self.palette_upload_scratch.is_empty() {
            let width = self.palette_dimensions[0] as usize;
            let texel_count = self.palette_upload_scratch.len() / PALETTE_TEXEL_BYTE_LEN;
            let uploaded_rows = texel_count.div_ceil(width);
            let uploaded_byte_len = uploaded_rows
                .saturating_mul(width)
                .saturating_mul(PALETTE_TEXEL_BYTE_LEN);
            self.palette_upload_scratch.resize(uploaded_byte_len, 0);
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.palette_texture,
                    mip_level: 0,
                    origin: Default::default(),
                    aspect: wgpu::TextureAspect::All,
                },
                &self.palette_upload_scratch,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.palette_dimensions[0] * PALETTE_TEXEL_BYTE_LEN as u32),
                    rows_per_image: Some(uploaded_rows as u32),
                },
                wgpu::Extent3d {
                    width: self.palette_dimensions[0],
                    height: uploaded_rows as u32,
                    depth_or_array_layers: 1,
                },
            );
            snapshot.palette_write_count = snapshot.palette_write_count.saturating_add(1);
            snapshot.palette_written_bytes = snapshot
                .palette_written_bytes
                .saturating_add(uploaded_byte_len as u64);
        }
    }

    fn allocated_byte_size(&self) -> u64 {
        self.actor_capacity_bytes.saturating_add(
            u64::from(self.palette_dimensions[0])
                .saturating_mul(u64::from(self.palette_dimensions[1]))
                .saturating_mul(PALETTE_TEXEL_BYTE_LEN as u64),
        )
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct EncodedPreparedActorDraws {
    draw_count: u64,
    drawn_instance_count: u64,
    max_instances_per_draw: u32,
    actor_count: usize,
    vertex_count: u32,
    index_count: u32,
}

impl EncodedPreparedActorDraws {
    fn actor_stats(self) -> ActorRenderStats {
        ActorRenderStats {
            submitted_actor_count: self.actor_count,
            drawn_actor_count: self.actor_count,
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            ..ActorRenderStats::default()
        }
    }
}

fn draw_buckets<'pass>(
    pass: &mut wgpu::RenderPass<'pass>,
    pipelines: &'pass PreparedActorPipelines,
    buckets: &'pass BTreeMap<ActorFigureId, PreparedActorInstanceBucket>,
    figures: &'pass BTreeMap<ActorFigureId, PreparedActorFigureResources>,
) -> EncodedPreparedActorDraws {
    let mut stats = EncodedPreparedActorDraws::default();
    for (figure_id, bucket) in buckets {
        let Some(figure) = figures.get(figure_id) else {
            continue;
        };
        let instance_count = bucket.order.len();
        stats.actor_count = stats.actor_count.saturating_add(instance_count);
        stats.vertex_count = stats
            .vertex_count
            .saturating_add(figure.vertex_count.saturating_mul(instance_count as u32));
        stats.index_count = stats
            .index_count
            .saturating_add(figure.index_count.saturating_mul(instance_count as u32));
    }

    for (prepared_pass, pipeline) in [
        (PreparedFigurePass::Opaque, &pipelines.opaque),
        (PreparedFigurePass::MaskThreshold, &pipelines.mask_threshold),
        (PreparedFigurePass::MaskDither, &pipelines.mask_dither),
    ] {
        pass.set_pipeline(pipeline);
        let pass_stats = draw_bucket_pass(pass, prepared_pass, buckets, figures);
        stats.include_pass(pass_stats);
    }

    pass.set_pipeline(&pipelines.blend_depth);
    let pass_stats = draw_bucket_pass(pass, PreparedFigurePass::Blend, buckets, figures);
    stats.include_pass(pass_stats);
    pass.set_pipeline(&pipelines.blend_color);
    let pass_stats = draw_bucket_pass(pass, PreparedFigurePass::Blend, buckets, figures);
    stats.include_pass(pass_stats);
    pass.set_pipeline(&pipelines.additive);
    let pass_stats = draw_bucket_pass(pass, PreparedFigurePass::Additive, buckets, figures);
    stats.include_pass(pass_stats);
    stats
}

#[derive(Clone, Copy, Debug, Default)]
struct EncodedPreparedActorPass {
    draw_count: u64,
    drawn_instance_count: u64,
    max_instances_per_draw: u32,
}

impl EncodedPreparedActorDraws {
    fn include_pass(&mut self, pass: EncodedPreparedActorPass) {
        self.draw_count = self.draw_count.saturating_add(pass.draw_count);
        self.drawn_instance_count = self
            .drawn_instance_count
            .saturating_add(pass.drawn_instance_count);
        self.max_instances_per_draw = self.max_instances_per_draw.max(pass.max_instances_per_draw);
    }
}

fn draw_bucket_pass<'pass>(
    pass: &mut wgpu::RenderPass<'pass>,
    prepared_pass: PreparedFigurePass,
    buckets: &'pass BTreeMap<ActorFigureId, PreparedActorInstanceBucket>,
    figures: &'pass BTreeMap<ActorFigureId, PreparedActorFigureResources>,
) -> EncodedPreparedActorPass {
    let mut stats = EncodedPreparedActorPass::default();
    for (figure_id, bucket) in buckets {
        let Some(figure) = figures.get(figure_id) else {
            continue;
        };
        let Some(range) = figure
            .pass_ranges
            .iter()
            .find(|range| range.pass == prepared_pass)
        else {
            continue;
        };
        let instance_count = bucket.order.len() as u32;
        pass.set_bind_group(1, &bucket.palette_bind_group, &[]);
        pass.set_bind_group(2, &figure.texture_bind_group, &[]);
        pass.set_vertex_buffer(0, figure.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, bucket.actor_buffer.slice(..));
        pass.set_index_buffer(figure.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(
            range.first_index..range.first_index + range.index_count,
            0,
            0..instance_count,
        );
        stats.draw_count = stats.draw_count.saturating_add(1);
        stats.drawn_instance_count = stats
            .drawn_instance_count
            .saturating_add(u64::from(instance_count));
        stats.max_instances_per_draw = stats.max_instances_per_draw.max(instance_count);
    }
    stats
}

fn prepared_actor_key(actor: ActorInstance) -> Option<(ActorInstanceId, ActorFigureId)> {
    let id = actor.id?;
    if !matches!(id, ActorInstanceId::Entity(_)) {
        return None;
    }
    let figure_id = match actor.shape {
        ActorInstanceShape::Figure(figure_id) | ActorInstanceShape::SemanticProp(figure_id) => {
            figure_id
        }
        _ => return None,
    };
    Some((id, figure_id))
}

fn actor_model_matrix(actor: ActorInstance, figure: &PreparedFigure) -> Option<Mat4> {
    if !actor.feet_position.is_finite()
        || !actor.rotation_pivot.is_finite()
        || !actor.height.is_finite()
        || !actor.yaw_radians.is_finite()
        || !actor.pitch_radians.is_finite()
    {
        return None;
    }
    let rotation = actor
        .orientation
        .filter(|orientation| orientation.is_finite())
        .unwrap_or_else(|| {
            Quat::from_rotation_y(actor.yaw_radians) * Quat::from_rotation_x(actor.pitch_radians)
        });
    let scale = if matches!(actor.shape, ActorInstanceShape::SemanticProp(_)) {
        let span = (figure.bounds.max[0] - figure.bounds.min[0])
            .max(figure.bounds.max[2] - figure.bounds.min[2]);
        actor.width.max(0.01) / span.max(f32::EPSILON)
    } else {
        actor.height.max(0.1)
    };
    let matrix = Mat4::from_translation(actor.feet_position + actor.rotation_pivot)
        * Mat4::from_quat(rotation)
        * Mat4::from_translation(-actor.rotation_pivot)
        * Mat4::from_scale(Vec3::splat(scale));
    matrix.is_finite().then_some(matrix)
}

fn push_actor_instance_bytes(
    bytes: &mut Vec<u8>,
    model: Mat4,
    packed_light: u32,
    opacity: f32,
    palette_base: u32,
) {
    push_affine_matrix_texels(bytes, &model.to_cols_array_2d());
    bytes.extend_from_slice(&packed_light.to_ne_bytes());
    bytes.extend_from_slice(&opacity.to_ne_bytes());
    bytes.extend_from_slice(&palette_base.to_ne_bytes());
    bytes.extend_from_slice(&0_u32.to_ne_bytes());
    debug_assert_eq!(bytes.len() % ACTOR_INSTANCE_BYTE_LEN, 0);
}

fn create_palette_texture(device: &wgpu::Device, dimensions: [u32; 2]) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mclone_prepared_actor_palette_texture"),
        size: wgpu::Extent3d {
            width: dimensions[0],
            height: dimensions[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn create_palette_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    texture_view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("mclone_prepared_actor_palette_bind_group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(texture_view),
        }],
    })
}

fn palette_texture_dimensions(required_texels: usize, max_dimension: u32) -> [u32; 2] {
    debug_assert!(required_texels > 0);
    let max_dimension = max_dimension.max(4) as usize;
    let target_capacity = required_texels.next_power_of_two();
    let mut width = 4_usize;
    while width.saturating_mul(width) < target_capacity && width < max_dimension {
        width = width.saturating_mul(2).min(max_dimension);
    }
    let height = target_capacity.div_ceil(width).min(max_dimension);
    debug_assert!(width.saturating_mul(height) >= required_texels);
    [width as u32, height as u32]
}

fn view_bytes(
    render_view: ChunkRenderView,
    render_options: TexturedSectionRenderOptions,
    context: Option<WorldCompositionContext>,
) -> [u8; VIEW_BYTE_LEN] {
    let mut values = [0.0_f32; VIEW_FLOAT_COUNT];
    for (index, value) in render_view
        .uniform_matrix()
        .into_iter()
        .flatten()
        .enumerate()
    {
        values[index] = value;
    }
    values[16..20].copy_from_slice(&[
        if render_options.force_fullbright {
            1.0
        } else {
            0.0
        },
        render_options.sky_darken.clamp(0.0, 1.0),
        render_options.fog.shader_options(),
        0.0,
    ]);
    let camera = render_view.camera_position;
    values[20..24].copy_from_slice(&[
        camera.x,
        camera.y,
        camera.z,
        render_options.fog.ground_base_y,
    ]);
    values[24..28].copy_from_slice(&[
        render_options.fog.color[0],
        render_options.fog.color[1],
        render_options.fog.color[2],
        render_options.fog.max_opacity,
    ]);
    let fog_distances = render_options.fog.shader_distances();
    values[28..32].copy_from_slice(&[fog_distances[0], fog_distances[1], 0.0, 0.0]);
    let (source_anchor_scale, composition_anchor, clip_plane, clip_enabled) =
        context.map_or(([0.0, 0.0, 0.0, 1.0], [0.0; 4], [0.0; 4], 0.0), |context| {
            let (source_anchor_scale, composition_anchor) = context.placement().shader_values();
            let (clip_plane, clip_enabled) = match context.clip() {
                CompositionClip::Unbounded => ([0.0; 4], 0.0),
                CompositionClip::HalfSpace(half_space) => (
                    [
                        half_space.normal().x,
                        half_space.normal().y,
                        half_space.normal().z,
                        half_space.offset(),
                    ],
                    1.0,
                ),
            };
            (
                source_anchor_scale,
                composition_anchor,
                clip_plane,
                clip_enabled,
            )
        });
    values[32..36].copy_from_slice(&source_anchor_scale);
    values[36..40].copy_from_slice(&composition_anchor);
    values[40..44].copy_from_slice(&clip_plane);
    values[44] = clip_enabled;
    f32_array_bytes(values)
}

fn multiview_view_bytes(
    render_views: [ChunkRenderView; 2],
    render_options: [TexturedSectionRenderOptions; 2],
    context: Option<WorldCompositionContext>,
) -> [u8; MULTIVIEW_BYTE_LEN] {
    let mut bytes = [0_u8; MULTIVIEW_BYTE_LEN];
    bytes[..VIEW_BYTE_LEN].copy_from_slice(&view_bytes(
        render_views[0],
        render_options[0],
        context,
    ));
    bytes[VIEW_BYTE_LEN..].copy_from_slice(&view_bytes(
        render_views[1],
        render_options[1],
        context,
    ));
    bytes
}

fn uniform_layout(
    device: &wgpu::Device,
    label: &'static str,
    size: wgpu::BufferAddress,
    dynamic: bool,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: dynamic,
                min_binding_size: NonZeroU64::new(size),
            },
            count: None,
        }],
    })
}

fn palette_texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mclone_prepared_actor_palette_texture_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    })
}

fn texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mclone_prepared_actor_texture_layout"),
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
    })
}

fn validate_pass_ranges(ranges: &[PreparedFigurePassRange], index_count: u32) -> Result<()> {
    let mut next_index = 0;
    let mut previous_pass = None;
    for range in ranges {
        if range.index_count == 0 || range.first_index != next_index {
            bail!("prepared actor figure has invalid or non-contiguous pass ranges");
        }
        if previous_pass.is_some_and(|previous| previous >= range.pass) {
            bail!("prepared actor figure pass ranges are not in canonical order");
        }
        next_index = next_index
            .checked_add(range.index_count)
            .context("prepared actor pass range overflow")?;
        previous_pass = Some(range.pass);
    }
    if next_index != index_count {
        bail!(
            "prepared actor pass ranges cover {} indices; expected {}",
            next_index,
            index_count
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum PreparedActorPipelineKind {
    Opaque,
    MaskThreshold,
    MaskDither,
    BlendDepth,
    BlendColor,
    Additive,
}

fn create_pipelines(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    multiview: Option<NonZeroU32>,
) -> PreparedActorPipelines {
    let create =
        |kind, label| create_pipeline(device, layout, shader, color_format, label, multiview, kind);
    PreparedActorPipelines {
        opaque: create(
            PreparedActorPipelineKind::Opaque,
            "mclone_prepared_actor_opaque",
        ),
        mask_threshold: create(
            PreparedActorPipelineKind::MaskThreshold,
            "mclone_prepared_actor_mask_threshold",
        ),
        mask_dither: create(
            PreparedActorPipelineKind::MaskDither,
            "mclone_prepared_actor_mask_dither",
        ),
        blend_depth: create(
            PreparedActorPipelineKind::BlendDepth,
            "mclone_prepared_actor_blend_depth",
        ),
        blend_color: create(
            PreparedActorPipelineKind::BlendColor,
            "mclone_prepared_actor_blend_color",
        ),
        additive: create(
            PreparedActorPipelineKind::Additive,
            "mclone_prepared_actor_additive",
        ),
    }
}

fn create_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    label: &'static str,
    multiview: Option<NonZeroU32>,
    kind: PreparedActorPipelineKind,
) -> wgpu::RenderPipeline {
    let (fragment_entry, blend, write_mask, depth_write_enabled, depth_compare) = match kind {
        PreparedActorPipelineKind::Opaque => (
            "fs_opaque",
            None,
            wgpu::ColorWrites::ALL,
            true,
            wgpu::CompareFunction::GreaterEqual,
        ),
        PreparedActorPipelineKind::MaskThreshold => (
            "fs_mask_threshold",
            None,
            wgpu::ColorWrites::ALL,
            true,
            wgpu::CompareFunction::GreaterEqual,
        ),
        PreparedActorPipelineKind::MaskDither => (
            "fs_mask_dither",
            None,
            wgpu::ColorWrites::ALL,
            true,
            wgpu::CompareFunction::GreaterEqual,
        ),
        PreparedActorPipelineKind::BlendDepth => (
            "fs_blend_depth",
            None,
            wgpu::ColorWrites::empty(),
            true,
            wgpu::CompareFunction::GreaterEqual,
        ),
        PreparedActorPipelineKind::BlendColor => (
            "fs_blend_color",
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            wgpu::ColorWrites::ALL,
            false,
            wgpu::CompareFunction::Equal,
        ),
        PreparedActorPipelineKind::Additive => (
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
            buffers: &[
                wgpu::VertexBufferLayout {
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
                },
                wgpu::VertexBufferLayout {
                    array_stride: ACTOR_INSTANCE_BYTE_SIZE,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 7,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 8,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 9,
                            format: wgpu::VertexFormat::Uint32,
                        },
                        wgpu::VertexAttribute {
                            offset: 52,
                            shader_location: 10,
                            format: wgpu::VertexFormat::Float32,
                        },
                        wgpu::VertexAttribute {
                            offset: 56,
                            shader_location: 11,
                            format: wgpu::VertexFormat::Uint32,
                        },
                    ],
                },
            ],
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

fn upload_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &'static str,
    usage: wgpu::BufferUsages,
    bytes: &[u8],
) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: bytes.len().max(4) as u64,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytes);
    buffer
}

fn vertex_bytes(vertices: &[PreparedFigureVertex]) -> Vec<u8> {
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

fn index_bytes(indices: &[u16]) -> Vec<u8> {
    indices
        .iter()
        .flat_map(|index| index.to_ne_bytes())
        .collect()
}

fn push_f32s(bytes: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
}

fn push_affine_matrix_texels(bytes: &mut Vec<u8>, columns: &[[f32; 4]; 4]) {
    debug_assert!(columns[0][3].abs() <= 1.0e-5);
    debug_assert!(columns[1][3].abs() <= 1.0e-5);
    debug_assert!(columns[2][3].abs() <= 1.0e-5);
    debug_assert!((columns[3][3] - 1.0).abs() <= 1.0e-5);
    for row in 0..3 {
        push_f32s(
            bytes,
            &[
                columns[0][row],
                columns[1][row],
                columns[2][row],
                columns[3][row],
            ],
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn elapsed_nanos(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn f32_array_bytes<const FLOATS: usize, const BYTES: usize>(values: [f32; FLOATS]) -> [u8; BYTES] {
    assert_eq!(FLOATS * 4, BYTES);
    let mut bytes = [0_u8; BYTES];
    for (index, value) in values.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_assets::mallard_duck_figure_id;

    fn prepared_actor_figures() -> ActorFigureSet {
        let mut source = mclone_assets::MemoryAssetSource::new();
        source.insert_text(
            mclone_assets::default_player_figure_path(),
            include_str!("../../../../assets/mclone/figures/player.figure.json"),
        );
        source.insert_text(
            mclone_assets::upright_bear_figure_path(),
            include_str!("../../../../assets/mclone/figures/upright_bear.figure.json"),
        );
        source.insert_text(
            mclone_assets::chicken_figure_path(),
            include_str!("../../../../assets/mclone/figures/chicken.figure.json"),
        );
        source.insert_text(
            mclone_assets::cow_figure_path(),
            include_str!("../../../../assets/mclone/figures/cow.figure.json"),
        );
        source.insert_text(
            mclone_assets::mallard_duck_figure_path(),
            include_str!("../../../../assets/mclone/figures/mallard_duck.figure.json"),
        );
        source.insert_text(
            mclone_assets::mallard_nest_figure_path(),
            include_str!("../../../../assets/mclone/figures/mallard_nest.figure.json"),
        );
        source.insert_text(
            mclone_assets::mallard_feather_figure_path(),
            include_str!("../../../../assets/mclone/figures/mallard_feather.figure.json"),
        );
        crate::asset_lab_figure::load_first_party_semantic_figures(&source).unwrap()
    }

    fn prepared_chicken() -> PreparedFigure {
        let asset: mclone_assets::FigureAsset = serde_json::from_str(include_str!(
            "../../../../assets/mclone/figures/chicken.figure.json"
        ))
        .unwrap();
        mclone_assets::prepare_figure_asset(&asset).unwrap()
    }

    #[test]
    fn prepared_actor_model_scales_normalized_figure_from_feet() {
        let actor = ActorInstance::remote_player(Vec3::new(2.0, 3.0, 4.0), 0.0)
            .with_dimensions(0.6, 1.8)
            .with_id(ActorInstanceId::Entity(7));
        let figures = prepared_actor_figures();
        let figure = figures
            .prepared(mclone_assets::default_player_figure_id())
            .unwrap();
        let model = actor_model_matrix(actor, figure).expect("finite model");
        assert_eq!(model.transform_point3(Vec3::ZERO), actor.feet_position);
        assert!((model.transform_point3(Vec3::Y).y - 4.8).abs() < 1.0e-6);
    }

    #[test]
    fn prepared_actor_selection_requires_stable_entity_identity() {
        let anonymous =
            ActorInstance::remote_player_with_figure(Vec3::ZERO, 0.0, chicken_figure_id());
        assert!(prepared_actor_key(anonymous).is_none());
        assert!(prepared_actor_key(anonymous.with_id(ActorInstanceId::RemotePlayer(9))).is_none());
        assert_eq!(
            prepared_actor_key(anonymous.with_id(ActorInstanceId::Entity(9))),
            Some((ActorInstanceId::Entity(9), chicken_figure_id()))
        );
        let mallard =
            ActorInstance::remote_player_with_figure(Vec3::ZERO, 0.0, mallard_duck_figure_id())
                .with_id(ActorInstanceId::Entity(10));
        assert_eq!(
            prepared_actor_key(mallard),
            Some((ActorInstanceId::Entity(10), mallard_duck_figure_id()))
        );
        let nest = ActorInstance::mallard_nest(Vec3::ZERO, 0.0, 0.8, 0.32)
            .with_id(ActorInstanceId::Entity(11));
        assert_eq!(
            prepared_actor_key(nest),
            Some((
                ActorInstanceId::Entity(11),
                mclone_assets::mallard_nest_figure_id()
            ))
        );
    }

    #[test]
    fn prepared_actor_selection_is_figure_capability_driven() {
        let cow = ActorInstance::remote_player_with_figure(
            Vec3::ZERO,
            0.0,
            mclone_assets::cow_figure_id(),
        )
        .with_id(ActorInstanceId::Entity(10));
        let bear = ActorInstance::remote_player_with_figure(
            Vec3::ZERO,
            0.0,
            mclone_assets::upright_bear_figure_id(),
        )
        .with_id(ActorInstanceId::Entity(11));

        assert_eq!(
            prepared_actor_key(cow),
            Some((ActorInstanceId::Entity(10), mclone_assets::cow_figure_id()))
        );
        assert_eq!(
            prepared_actor_key(bear),
            Some((
                ActorInstanceId::Entity(11),
                mclone_assets::upright_bear_figure_id()
            ))
        );
    }

    #[test]
    fn semantic_prop_model_uses_horizontal_span_and_ground_anchor() {
        let figures = prepared_actor_figures();
        let figure = figures
            .prepared(mclone_assets::mallard_nest_figure_id())
            .unwrap();
        let actor = ActorInstance::mallard_nest(Vec3::new(2.0, 64.0, 3.0), 0.0, 0.8, 0.32)
            .with_id(ActorInstanceId::Entity(12));
        let model = actor_model_matrix(actor, figure).unwrap();
        let min = model.transform_point3(Vec3::from_array(figure.bounds.min));
        let max = model.transform_point3(Vec3::from_array(figure.bounds.max));

        assert!((min.y - 64.0).abs() < 1.0e-6);
        assert!((max.x - min.x - 0.8).abs() < 1.0e-5);
        assert!(max.y - min.y > actor.height);
        assert!(max.y - min.y < 0.5);
    }

    #[test]
    fn semantic_prop_selection_preserves_semantic_resource_identity() {
        let actor = ActorInstance::semantic_prop(
            Vec3::ZERO,
            0.0,
            mclone_assets::ActorFigureId::from_static("mclone:missing-prop"),
            0.8,
            0.32,
        )
        .with_id(ActorInstanceId::Entity(13));
        assert_eq!(
            prepared_actor_key(actor),
            Some((
                ActorInstanceId::Entity(13),
                mclone_assets::ActorFigureId::from_static("mclone:missing-prop")
            ))
        );
    }

    #[test]
    fn animated_chicken_pose_stays_finite_and_actor_sized_across_cycle() {
        let figure = prepared_chicken();
        let wing_ids = ["wing_l", "wing_r"].map(|name| {
            figure
                .parts
                .iter()
                .position(|part| part.name == name)
                .unwrap() as u16
        });
        let actor = ActorInstance::remote_player_with_figure(
            Vec3::new(10.0, 64.0, -4.0),
            37.0,
            chicken_figure_id(),
        )
        .with_dimensions(0.4, 0.7)
        .with_id(ActorInstanceId::Entity(9));
        let model = actor_model_matrix(actor, &figure).unwrap();
        let walk = figure.clips.get("walk").unwrap();
        let mut palette = Vec::new();

        for step in 0..=240 {
            let phase = step as f32 / 240.0;
            let flap = (phase * std::f32::consts::TAU).sin() * 0.8;
            let overrides = [
                PreparedFigurePartRotationOverride {
                    part_id: wing_ids[0],
                    rotation_delta_radians: [0.0, 0.0, flap],
                },
                PreparedFigurePartRotationOverride {
                    part_id: wing_ids[1],
                    rotation_delta_radians: [0.0, 0.0, -flap],
                },
            ];
            evaluate_prepared_figure_clip_with_part_rotation_overrides_into(
                &figure,
                "walk",
                f64::from(phase * walk.duration_seconds),
                &overrides,
                &mut palette,
            )
            .unwrap();

            for vertex in &figure.vertices {
                let local = Mat4::from_cols_array_2d(&palette[vertex.part_id as usize])
                    .transform_point3(Vec3::from_array(vertex.position));
                let world = model.transform_point3(local);
                let actor_relative = world - actor.feet_position;
                assert!(world.is_finite());
                assert!(actor_relative.abs().max_element() < 2.0);
            }
        }
    }

    #[test]
    fn prepared_actor_shaders_validate_for_direct_and_multiview() {
        let direct_source =
            crate::fog::inject_fog_wgsl(include_str!("shaders/prepared_actor.wgsl"));
        let direct = naga::front::wgsl::parse_str(&direct_source).expect("direct WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&direct)
        .expect("direct WGSL validates");
        let multiview_source =
            crate::fog::inject_fog_wgsl(include_str!("shaders/prepared_actor_multiview.wgsl"));
        let multiview =
            naga::front::wgsl::parse_str(&multiview_source).expect("multiview WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::MULTIVIEW,
        )
        .validate(&multiview)
        .expect("multiview WGSL validates");
    }

    #[test]
    fn prepared_actor_vertex_and_instance_bytes_carry_alpha_fields() {
        let figure = prepared_chicken();
        let vertex_bytes = vertex_bytes(&figure.vertices[..1]);
        assert_eq!(vertex_bytes.len(), VERTEX_BYTE_LEN);
        assert_eq!(
            &vertex_bytes[52..56],
            &figure.vertices[0].alpha_cutoff.to_ne_bytes()
        );

        let mut bytes = Vec::new();
        push_actor_instance_bytes(&mut bytes, Mat4::IDENTITY, 0x00f0_00f0, 0.42, 1_234);
        assert_eq!(bytes.len(), ACTOR_INSTANCE_BYTE_LEN);
        assert_eq!(&bytes[48..52], &0x00f0_00f0_u32.to_ne_bytes());
        assert_eq!(&bytes[52..56], &0.42_f32.to_ne_bytes());
        assert_eq!(&bytes[56..60], &1_234_u32.to_ne_bytes());
    }

    #[test]
    fn affine_palette_packing_round_trips_scale_rotation_and_translation() {
        let matrix = Mat4::from_scale_rotation_translation(
            Vec3::new(0.25, 1.5, 2.0),
            Quat::from_rotation_x(0.7) * Quat::from_rotation_z(-0.3),
            Vec3::new(4.0, -2.0, 9.0),
        )
        .to_cols_array_2d();
        let mut bytes = Vec::new();
        push_affine_matrix_texels(&mut bytes, &matrix);
        assert_eq!(bytes.len(), PALETTE_MATRIX_BYTE_LEN);

        let values = bytes
            .chunks_exact(std::mem::size_of::<f32>())
            .map(|value| f32::from_ne_bytes(value.try_into().unwrap()))
            .collect::<Vec<_>>();
        let rebuilt = [
            [values[0], values[4], values[8], 0.0],
            [values[1], values[5], values[9], 0.0],
            [values[2], values[6], values[10], 0.0],
            [values[3], values[7], values[11], 1.0],
        ];
        assert_eq!(rebuilt, matrix);
    }

    #[test]
    fn palette_texture_dimensions_cover_required_texels_with_bounded_axes() {
        for required_texels in [1, 3, 4, 5, 17, 255, 256, 257, 3_000, 65_535] {
            let dimensions = palette_texture_dimensions(required_texels, 256);
            assert!(dimensions[0] <= 256);
            assert!(dimensions[1] <= 256);
            assert!(
                dimensions[0] as usize * dimensions[1] as usize >= required_texels,
                "{dimensions:?} does not cover {required_texels} texels"
            );
        }
    }

    #[test]
    #[ignore = "GPU scale characterization; run explicitly on a host with a wgpu adapter"]
    fn thousand_chicken_instanced_baseline_and_residency() -> Result<()> {
        const ACTOR_COUNT: usize = 1_000;
        let chicken = prepared_chicken();
        let draws_per_bucket = chicken.pass_ranges.len() as u64
            + u64::from(
                chicken
                    .pass_ranges
                    .iter()
                    .any(|range| range.pass == PreparedFigurePass::Blend),
            );
        let (device, queue) = crate::headless::create_headless_device()?;
        let figures = prepared_actor_figures();
        let shared = PreparedActorSharedResources::new(
            &device,
            &queue,
            crate::headless::HEADLESS_FORMAT,
            &figures,
        )?;
        let immutable = shared.snapshot();
        let immutable_bytes = immutable
            .immutable_vertex_bytes
            .saturating_add(immutable.immutable_index_bytes)
            .saturating_add(immutable.immutable_atlas_bytes);
        let mut world = PreparedActorDrawResources::new(&device, &shared);
        let mut other_world = PreparedActorDrawResources::new(&device, &shared);
        let mut actors = (0..ACTOR_COUNT)
            .map(|index| {
                let x = (index % 40) as f32 * 0.7 - 14.0;
                let z = (index / 40) as f32 * 0.7 - 8.0;
                ActorInstance::remote_player_with_figure(
                    Vec3::new(x, 0.0, z),
                    (index % 360) as f32,
                    chicken_figure_id(),
                )
                .with_id(ActorInstanceId::Entity(index as u64 + 1))
                .with_dimensions(0.4, 0.7)
                .with_walk_animation_distance(index as f32 * 0.013)
                .with_chicken_wing_flap_radians(Some((index as f32 * 0.17).sin() * 0.8))
            })
            .collect::<Vec<_>>();

        let first_prepare_start = Instant::now();
        world.prepare(&device, &queue, &shared, &actors);
        let first_prepare_ms = first_prepare_start.elapsed().as_secs_f64() * 1_000.0;
        let first = world.snapshot();
        assert_eq!(first.actor_record_count, ACTOR_COUNT);
        assert_eq!(first.prepared_actor_count, ACTOR_COUNT);
        assert_eq!(first.legacy_actor_count, 0);
        assert_eq!(first.instance_bucket_count, 1);
        assert_eq!(first.pose_evaluation_count, ACTOR_COUNT as u64);
        assert_eq!(first.palette_write_count, 1);
        assert_eq!(first.actor_write_count, 1);
        assert!(
            first.mutable_known_allocated_bytes
                >= ACTOR_COUNT as u64
                    * (chicken.parts.len() as u64 * PALETTE_MATRIX_BYTE_LEN as u64
                        + ACTOR_INSTANCE_BYTE_LEN as u64)
        );
        assert_eq!(shared.snapshot(), immutable);

        world.prepare(&device, &queue, &shared, &actors);
        let unchanged = world.snapshot();
        assert_eq!(unchanged.unchanged_actor_reuse_count, ACTOR_COUNT as u64);
        assert_eq!(unchanged.pose_evaluation_count, first.pose_evaluation_count);
        assert_eq!(unchanged.palette_write_count, first.palette_write_count);
        assert_eq!(unchanged.actor_write_count, first.actor_write_count);

        for actor in &mut actors {
            actor.feet_position.x += 0.01;
            if let Some(animation) = actor.animation.as_mut() {
                animation.distance += 0.01;
            }
        }
        let steady_prepare_start = Instant::now();
        world.prepare(&device, &queue, &shared, &actors);
        let steady_prepare_ms = steady_prepare_start.elapsed().as_secs_f64() * 1_000.0;
        assert_eq!(world.snapshot().actor_record_count, ACTOR_COUNT);
        assert_eq!(shared.snapshot(), immutable);

        other_world.prepare(&device, &queue, &shared, &actors[..1]);
        assert_eq!(other_world.snapshot().actor_record_count, 1);
        assert_eq!(world.snapshot().actor_record_count, ACTOR_COUNT);
        assert_eq!(shared.snapshot(), immutable);

        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_thousand_chicken_color"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: crate::headless::HEADLESS_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = color.create_view(&Default::default());
        let depth = crate::chunk::ChunkDepthTarget::new(&device, 64, 64);
        let target = RenderFrameTarget::color(&color_view, [64, 64]).with_depth(&depth.view);
        let render_view = crate::chunk::ChunkCamera {
            eye: [0.0, 8.0, 24.0],
            target: [0.0, 2.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 70.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 256.0,
        }
        .render_view(64, 64);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_thousand_chicken_encoder"),
        });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mclone_thousand_chicken_clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(crate::chunk::REVERSED_Z_DEPTH_CLEAR),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
        }
        let encode_submit_start = Instant::now();
        let stats = world.render_in_slot(
            &queue,
            &mut encoder,
            target,
            render_view,
            TexturedSectionRenderOptions::default(),
            None,
            crate::uniform::SINGLE_VIEW_SLOT,
            &shared,
        )?;
        let submission = queue.submit(std::iter::once(encoder.finish()));
        device
            .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
            .or_else(|_| device.poll(wgpu::PollType::Wait))?;
        let encode_submit_wait_ms = encode_submit_start.elapsed().as_secs_f64() * 1_000.0;
        assert_eq!(stats.drawn_actor_count, ACTOR_COUNT);
        assert_eq!(world.snapshot().draw_count, draws_per_bucket);
        assert_eq!(world.snapshot().max_instances_per_draw, ACTOR_COUNT as u32);
        assert_eq!(
            world.snapshot().drawn_instance_count,
            ACTOR_COUNT as u64 * draws_per_bucket
        );
        assert_eq!(shared.snapshot(), immutable);

        world.prepare(&device, &queue, &shared, &actors[..ACTOR_COUNT / 2]);
        assert_eq!(world.snapshot().actor_record_count, ACTOR_COUNT / 2);
        world.prepare(&device, &queue, &shared, &actors);
        let final_snapshot = world.snapshot();
        assert_eq!(final_snapshot.actor_record_count, ACTOR_COUNT);
        assert_eq!(shared.snapshot(), immutable);

        eprintln!(
            "prepared actor 1000 baseline: first-prepare={first_prepare_ms:.3}ms \
             steady-prepare={steady_prepare_ms:.3}ms \
             encode-submit-wait={encode_submit_wait_ms:.3}ms draws={} \
             mutable-bytes={} immutable-bytes={} immutable-uploads={}",
            final_snapshot.draw_count,
            final_snapshot.mutable_known_allocated_bytes,
            immutable_bytes,
            immutable.immutable_upload_count,
        );
        Ok(())
    }
}
