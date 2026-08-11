use std::collections::{BTreeMap, BTreeSet};
use std::num::{NonZeroU32, NonZeroU64};

use anyhow::{Context, Result, bail};
use glam::{Mat4, Quat, Vec3};
use mclone_assets::{
    ActorFigureId, PreparedFigure, PreparedFigurePartRotationOverride, PreparedFigurePass,
    PreparedFigurePassRange, PreparedFigureVertex, chicken_figure_id, default_player_figure_id,
    evaluate_prepared_figure_clip_into,
    evaluate_prepared_figure_clip_with_part_rotation_overrides_into,
    evaluate_prepared_figure_rest_pose_into, mallard_duck_figure_id,
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
const ACTOR_FLOAT_COUNT: usize = 20;
const ACTOR_BYTE_LEN: usize = ACTOR_FLOAT_COUNT * std::mem::size_of::<f32>();
const ACTOR_BYTE_SIZE: wgpu::BufferAddress = ACTOR_BYTE_LEN as wgpu::BufferAddress;
const PALETTE_FLOAT_COUNT: usize = MAX_PREPARED_ACTOR_PARTS * 16;
const PALETTE_BYTE_LEN: usize = PALETTE_FLOAT_COUNT * std::mem::size_of::<f32>();
const PALETTE_BYTE_SIZE: wgpu::BufferAddress = PALETTE_BYTE_LEN as wgpu::BufferAddress;

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
    pub pose_evaluation_count: u64,
    pub palette_write_count: u64,
    pub palette_written_bytes: u64,
    pub actor_write_count: u64,
    pub actor_written_bytes: u64,
    pub view_write_count: u64,
    pub multiview_view_write_count: u64,
    pub draw_count: u64,
    pub mutable_known_allocated_bytes: u64,
}

pub(crate) struct PreparedActorSharedResources {
    pipelines: PreparedActorPipelines,
    multiview_pipelines: Option<PreparedActorPipelines>,
    view_layout: wgpu::BindGroupLayout,
    multiview_view_layout: Option<wgpu::BindGroupLayout>,
    actor_layout: wgpu::BindGroupLayout,
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
    draw_order: Vec<ActorInstanceId>,
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
    palette: wgpu::Buffer,
    palette_bind_group: wgpu::BindGroup,
    actor: wgpu::Buffer,
    actor_bind_group: wgpu::BindGroup,
    pose_palette: Vec<[[f32; 4]; 4]>,
    palette_upload_scratch: Vec<u8>,
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
        let actor_layout = uniform_layout(
            device,
            "mclone_prepared_actor_model_layout",
            ACTOR_BYTE_SIZE,
            false,
        );
        let palette_layout = uniform_layout(
            device,
            "mclone_prepared_actor_palette_layout",
            PALETTE_BYTE_SIZE,
            false,
        );
        let texture_layout = texture_layout(device);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_prepared_actor_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/prepared_actor.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_prepared_actor_pipeline_layout"),
            bind_group_layouts: &[
                &view_layout,
                &actor_layout,
                &palette_layout,
                &texture_layout,
            ],
            push_constant_ranges: &[],
        });
        let pipelines = create_pipelines(device, &pipeline_layout, &shader, color_format, None);
        let multiview_pipelines = multiview_view_layout.as_ref().map(|multiview_view_layout| {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("mclone_prepared_actor_multiview_shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shaders/prepared_actor_multiview.wgsl").into(),
                ),
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mclone_prepared_actor_multiview_pipeline_layout"),
                bind_group_layouts: &[
                    multiview_view_layout,
                    &actor_layout,
                    &palette_layout,
                    &texture_layout,
                ],
                push_constant_ranges: &[],
            });
            create_pipelines(device, &layout, &shader, color_format, NonZeroU32::new(2))
        });

        let mut gpu_figures = BTreeMap::new();
        let mut snapshot = PreparedActorSharedSnapshot {
            multiview_pipeline_count: if multiview_pipelines.is_some() { 6 } else { 0 },
            ..PreparedActorSharedSnapshot::default()
        };
        for (id, figure) in figures.prepared_figures().filter(|(id, _)| {
            *id == default_player_figure_id()
                || *id == chicken_figure_id()
                || *id == mallard_duck_figure_id()
        }) {
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
            actor_layout,
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
            draw_order: Vec::new(),
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
        self.draw_order.clear();
        self.legacy_actors.clear();
        self.retained_ids.clear();
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
            let record = self.records.entry(id).or_insert_with(|| {
                PreparedActorRecord::new(device, shared, figure_id, figure.figure.parts.len())
            });
            if record.figure_id != figure_id {
                *record =
                    PreparedActorRecord::new(device, shared, figure_id, figure.figure.parts.len());
            }
            if record
                .evaluate_and_write(queue, figure, *actor, &mut self.snapshot)
                .is_err()
            {
                self.legacy_actors.push(*actor);
                self.retained_ids.remove(&id);
                continue;
            }
            self.draw_order.push(id);
        }
        self.records.retain(|id, _| self.retained_ids.contains(id));
        self.snapshot.actor_record_count = self.records.len();
        self.snapshot.prepared_actor_count = self.draw_order.len();
        self.snapshot.legacy_actor_count = self.legacy_actors.len();
        self.snapshot.mutable_known_allocated_bytes = self
            .views
            .allocated_byte_size()
            .saturating_add(if self.multiview.is_some() {
                MULTIVIEW_BYTE_SIZE
            } else {
                0
            })
            .saturating_add(self.records.len() as u64 * (ACTOR_BYTE_SIZE + PALETTE_BYTE_SIZE));
    }

    pub(crate) fn legacy_actors(&self) -> &[ActorInstance] {
        &self.legacy_actors
    }

    pub(crate) fn prepared_input_count(&self) -> usize {
        self.draw_order.len() + self.legacy_actors.len()
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
        if self.draw_order.is_empty() {
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
        let stats = draw_records(
            &mut pass,
            &shared.pipelines,
            &self.draw_order,
            &self.records,
            &shared.figures,
        );
        self.snapshot.draw_count = self.snapshot.draw_count.saturating_add(stats.draw_count);
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
        if self.draw_order.is_empty() {
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
        let stats = draw_records(
            &mut pass,
            pipelines,
            &self.draw_order,
            &self.records,
            &shared.figures,
        );
        self.snapshot.draw_count = self.snapshot.draw_count.saturating_add(stats.draw_count);
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
    fn new(
        device: &wgpu::Device,
        shared: &PreparedActorSharedResources,
        figure_id: ActorFigureId,
        part_count: usize,
    ) -> Self {
        let palette = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_prepared_actor_palette"),
            size: PALETTE_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let palette_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_prepared_actor_palette_bind_group"),
            layout: &shared.palette_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: palette.as_entire_binding(),
            }],
        });
        let actor = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_prepared_actor_model"),
            size: ACTOR_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let actor_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_prepared_actor_model_bind_group"),
            layout: &shared.actor_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: actor.as_entire_binding(),
            }],
        });
        Self {
            figure_id,
            palette,
            palette_bind_group,
            actor,
            actor_bind_group,
            pose_palette: Vec::with_capacity(part_count),
            palette_upload_scratch: Vec::with_capacity(part_count * 16 * 4),
        }
    }

    fn evaluate_and_write(
        &mut self,
        queue: &wgpu::Queue,
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
        let model = actor_model_matrix(actor).context("prepared actor has invalid transform")?;
        if !actor.opacity.is_finite() || !(0.0..=1.0).contains(&actor.opacity) {
            bail!("prepared actor has invalid opacity");
        }
        self.palette_upload_scratch.clear();
        for matrix in &self.pose_palette {
            for column in matrix {
                push_f32s(&mut self.palette_upload_scratch, column);
            }
        }
        queue.write_buffer(&self.palette, 0, &self.palette_upload_scratch);
        queue.write_buffer(
            &self.actor,
            0,
            &actor_bytes(model, actor.packed_light, actor.opacity),
        );
        snapshot.pose_evaluation_count = snapshot.pose_evaluation_count.saturating_add(1);
        snapshot.palette_write_count = snapshot.palette_write_count.saturating_add(1);
        snapshot.palette_written_bytes = snapshot
            .palette_written_bytes
            .saturating_add(self.palette_upload_scratch.len() as u64);
        snapshot.actor_write_count = snapshot.actor_write_count.saturating_add(1);
        snapshot.actor_written_bytes = snapshot
            .actor_written_bytes
            .saturating_add(ACTOR_BYTE_LEN as u64);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct EncodedPreparedActorDraws {
    draw_count: u64,
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

fn draw_records<'pass>(
    pass: &mut wgpu::RenderPass<'pass>,
    pipelines: &'pass PreparedActorPipelines,
    order: &[ActorInstanceId],
    records: &'pass BTreeMap<ActorInstanceId, PreparedActorRecord>,
    figures: &'pass BTreeMap<ActorFigureId, PreparedActorFigureResources>,
) -> EncodedPreparedActorDraws {
    let mut stats = EncodedPreparedActorDraws::default();
    for id in order {
        let Some(record) = records.get(id) else {
            continue;
        };
        let Some(figure) = figures.get(&record.figure_id) else {
            continue;
        };
        stats.actor_count += 1;
        stats.vertex_count = stats.vertex_count.saturating_add(figure.vertex_count);
        stats.index_count = stats.index_count.saturating_add(figure.index_count);
    }

    for (prepared_pass, pipeline) in [
        (PreparedFigurePass::Opaque, &pipelines.opaque),
        (PreparedFigurePass::MaskThreshold, &pipelines.mask_threshold),
        (PreparedFigurePass::MaskDither, &pipelines.mask_dither),
    ] {
        pass.set_pipeline(pipeline);
        stats.draw_count = stats.draw_count.saturating_add(draw_record_pass(
            pass,
            prepared_pass,
            order,
            records,
            figures,
        ));
    }

    pass.set_pipeline(&pipelines.blend_depth);
    stats.draw_count = stats.draw_count.saturating_add(draw_record_pass(
        pass,
        PreparedFigurePass::Blend,
        order,
        records,
        figures,
    ));
    pass.set_pipeline(&pipelines.blend_color);
    stats.draw_count = stats.draw_count.saturating_add(draw_record_pass(
        pass,
        PreparedFigurePass::Blend,
        order,
        records,
        figures,
    ));
    pass.set_pipeline(&pipelines.additive);
    stats.draw_count = stats.draw_count.saturating_add(draw_record_pass(
        pass,
        PreparedFigurePass::Additive,
        order,
        records,
        figures,
    ));
    stats
}

fn draw_record_pass<'pass>(
    pass: &mut wgpu::RenderPass<'pass>,
    prepared_pass: PreparedFigurePass,
    order: &[ActorInstanceId],
    records: &'pass BTreeMap<ActorInstanceId, PreparedActorRecord>,
    figures: &'pass BTreeMap<ActorFigureId, PreparedActorFigureResources>,
) -> u64 {
    let mut draw_count = 0;
    for id in order {
        let Some(record) = records.get(id) else {
            continue;
        };
        let Some(figure) = figures.get(&record.figure_id) else {
            continue;
        };
        let Some(range) = figure
            .pass_ranges
            .iter()
            .find(|range| range.pass == prepared_pass)
        else {
            continue;
        };
        pass.set_bind_group(1, &record.actor_bind_group, &[]);
        pass.set_bind_group(2, &record.palette_bind_group, &[]);
        pass.set_bind_group(3, &figure.texture_bind_group, &[]);
        pass.set_vertex_buffer(0, figure.vertex_buffer.slice(..));
        pass.set_index_buffer(figure.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(
            range.first_index..range.first_index + range.index_count,
            0,
            0..1,
        );
        draw_count += 1;
    }
    draw_count
}

fn prepared_actor_key(actor: ActorInstance) -> Option<(ActorInstanceId, ActorFigureId)> {
    let id = actor.id?;
    if !matches!(id, ActorInstanceId::Entity(_)) {
        return None;
    }
    let ActorInstanceShape::Figure(figure_id) = actor.shape else {
        return None;
    };
    (figure_id == chicken_figure_id()
        || figure_id == default_player_figure_id()
        || figure_id == mallard_duck_figure_id())
    .then_some((id, figure_id))
}

fn actor_model_matrix(actor: ActorInstance) -> Option<Mat4> {
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
    let scale = actor.height.max(0.1);
    let matrix = Mat4::from_translation(actor.feet_position + actor.rotation_pivot)
        * Mat4::from_quat(rotation)
        * Mat4::from_translation(-actor.rotation_pivot)
        * Mat4::from_scale(Vec3::splat(scale));
    matrix.is_finite().then_some(matrix)
}

fn actor_bytes(model: Mat4, packed_light: u32, opacity: f32) -> [u8; ACTOR_BYTE_LEN] {
    let mut bytes = [0_u8; ACTOR_BYTE_LEN];
    let mut offset = 0;
    for value in model.to_cols_array() {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        offset += 4;
    }
    bytes[offset..offset + 4].copy_from_slice(&packed_light.to_ne_bytes());
    bytes[offset + 4..offset + 8].copy_from_slice(&opacity.to_ne_bytes());
    bytes
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
        if render_options.fog.enabled { 1.0 } else { 0.0 },
        0.0,
    ]);
    let camera = render_view.camera_position;
    values[20..24].copy_from_slice(&[camera.x, camera.y, camera.z, 0.0]);
    values[24..28].copy_from_slice(&[
        render_options.fog.color[0],
        render_options.fog.color[1],
        render_options.fog.color[2],
        1.0,
    ]);
    values[28..32].copy_from_slice(&[render_options.fog.start, render_options.fog.end, 0.0, 0.0]);
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
    use std::time::Instant;

    use super::*;

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
        crate::asset_lab_figure::load_first_party_actor_figures(&source).unwrap()
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
        let model = actor_model_matrix(actor).expect("finite model");
        assert_eq!(model.transform_point3(Vec3::ZERO), actor.feet_position);
        assert!((model.transform_point3(Vec3::Y).y - 4.8).abs() < 1.0e-6);
    }

    #[test]
    fn prepared_actor_selection_requires_stable_entity_identity() {
        let anonymous =
            ActorInstance::remote_player_with_figure(Vec3::ZERO, 0.0, chicken_figure_id());
        assert!(prepared_actor_key(anonymous).is_none());
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
        let model = actor_model_matrix(actor).unwrap();
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
        let direct = naga::front::wgsl::parse_str(include_str!("shaders/prepared_actor.wgsl"))
            .expect("direct WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&direct)
        .expect("direct WGSL validates");
        let multiview =
            naga::front::wgsl::parse_str(include_str!("shaders/prepared_actor_multiview.wgsl"))
                .expect("multiview WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::MULTIVIEW,
        )
        .validate(&multiview)
        .expect("multiview WGSL validates");
    }

    #[test]
    fn prepared_actor_vertex_and_uniform_bytes_carry_alpha_fields() {
        let figure = prepared_chicken();
        let vertex_bytes = vertex_bytes(&figure.vertices[..1]);
        assert_eq!(vertex_bytes.len(), VERTEX_BYTE_LEN);
        assert_eq!(
            &vertex_bytes[52..56],
            &figure.vertices[0].alpha_cutoff.to_ne_bytes()
        );

        let bytes = actor_bytes(Mat4::IDENTITY, 0x00f0_00f0, 0.42);
        assert_eq!(&bytes[64..68], &0x00f0_00f0_u32.to_ne_bytes());
        assert_eq!(&bytes[68..72], &0.42_f32.to_ne_bytes());
    }

    #[test]
    #[ignore = "GPU scale characterization; run explicitly on a host with a wgpu adapter"]
    fn thousand_chicken_non_instanced_baseline_and_residency() -> Result<()> {
        const ACTOR_COUNT: usize = 1_000;
        let chicken = prepared_chicken();
        let draws_per_actor = chicken.pass_ranges.len() as u64
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
        assert_eq!(first.pose_evaluation_count, ACTOR_COUNT as u64);
        assert_eq!(first.palette_write_count, ACTOR_COUNT as u64);
        assert_eq!(first.actor_write_count, ACTOR_COUNT as u64);
        assert!(first.mutable_known_allocated_bytes >= ACTOR_COUNT as u64 * 4_176);
        assert_eq!(shared.snapshot(), immutable);

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
        assert_eq!(
            world.snapshot().draw_count,
            ACTOR_COUNT as u64 * draws_per_actor
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
