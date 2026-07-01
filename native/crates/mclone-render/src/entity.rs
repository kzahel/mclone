use std::cell::RefCell;
use std::num::{NonZeroU32, NonZeroU64};

use anyhow::{Context, Result, bail};
use glam::{EulerRot, Mat4, Quat, Vec3};
use mclone_assets::{ActorFigureId, default_player_figure_id};
use wgpu::util::DeviceExt;

use crate::asset_lab_figure::{CompiledFigureClip, CompiledFigureTransform};
use crate::chunk::{ChunkRenderView, DEPTH_FORMAT, TexturedSectionRenderOptions};
use crate::light_texture::FULL_BRIGHT;
use crate::target::RenderFrameTarget;
use crate::uniform::{
    PER_VIEW_UNIFORM_SLOT_COUNT, PerViewSlot, PerViewUniformBuffer, SINGLE_VIEW_SLOT,
};

pub use crate::asset_lab_figure::{ActorFigureSet, CompiledFigure as ActorFigure};

const ACTOR_VERTEX_BYTE_LEN: usize = 3 * std::mem::size_of::<f32>()
    + 2 * std::mem::size_of::<f32>()
    + 4 * std::mem::size_of::<f32>()
    + std::mem::size_of::<u32>();
const ACTOR_VERTEX_BYTE_SIZE: wgpu::BufferAddress = ACTOR_VERTEX_BYTE_LEN as wgpu::BufferAddress;
const UNIFORM_FLOAT_COUNT: usize = 32;
const UNIFORM_BYTE_LEN: usize = UNIFORM_FLOAT_COUNT * std::mem::size_of::<f32>();
const UNIFORM_BYTE_SIZE: wgpu::BufferAddress = UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const MULTIVIEW_UNIFORM_BYTE_LEN: usize = UNIFORM_BYTE_LEN * 2;
const MULTIVIEW_UNIFORM_BYTE_SIZE: wgpu::BufferAddress =
    MULTIVIEW_UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const MODEL_PIXEL_SCALE: f32 = 1.0 / 16.0;
const MODEL_FEET_Y_PIXELS: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorInstance {
    pub feet_position: Vec3,
    /// Native world yaw in radians. Local actor +Z is the forward/front side.
    pub yaw_radians: f32,
    pub pitch_radians: f32,
    pub rotation_pivot: Vec3,
    pub orientation: Option<Quat>,
    pub shape: ActorInstanceShape,
    pub first_person_body_only: bool,
    pub width: f32,
    pub height: f32,
    pub body_color: [f32; 4],
    pub accent_color: [f32; 4],
    pub packed_light: u32,
    pub animation: Option<ActorAnimation>,
    pub chicken_wing_flap_radians: Option<f32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorInstanceShape {
    Figure(ActorFigureId),
    Humanoid,
    QuadrupedPlaceholder,
    CowModel,
    DebugCube,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorAnimation {
    pub clip: ActorAnimationClip,
    pub distance: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorAnimationClip {
    Walk,
}

impl ActorInstance {
    pub fn local_player(feet_position: Vec3, y_rot_degrees: f32) -> Self {
        Self::local_player_with_figure(feet_position, y_rot_degrees, default_player_figure_id())
    }

    pub fn local_player_with_figure(
        feet_position: Vec3,
        y_rot_degrees: f32,
        figure: ActorFigureId,
    ) -> Self {
        Self {
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            pitch_radians: 0.0,
            rotation_pivot: Vec3::ZERO,
            orientation: None,
            shape: ActorInstanceShape::Figure(figure),
            first_person_body_only: false,
            width: 0.6,
            height: 1.8,
            body_color: [0.18, 0.38, 0.82, 1.0],
            accent_color: [0.92, 0.70, 0.54, 1.0],
            packed_light: FULL_BRIGHT,
            animation: None,
            chicken_wing_flap_radians: None,
        }
    }

    pub fn remote_player(feet_position: Vec3, y_rot_degrees: f32) -> Self {
        Self {
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            pitch_radians: 0.0,
            rotation_pivot: Vec3::ZERO,
            orientation: None,
            shape: ActorInstanceShape::Figure(default_player_figure_id()),
            first_person_body_only: false,
            width: 0.6,
            height: 1.8,
            body_color: [0.10, 0.58, 0.68, 1.0],
            accent_color: [0.95, 0.80, 0.24, 1.0],
            packed_light: FULL_BRIGHT,
            animation: None,
            chicken_wing_flap_radians: None,
        }
    }

    pub fn remote_player_with_figure(
        feet_position: Vec3,
        y_rot_degrees: f32,
        figure: ActorFigureId,
    ) -> Self {
        Self {
            shape: ActorInstanceShape::Figure(figure),
            ..Self::remote_player(feet_position, y_rot_degrees)
        }
    }

    pub fn cow_placeholder(
        feet_position: Vec3,
        y_rot_degrees: f32,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            pitch_radians: 0.0,
            rotation_pivot: Vec3::ZERO,
            orientation: None,
            shape: ActorInstanceShape::QuadrupedPlaceholder,
            first_person_body_only: false,
            width,
            height,
            body_color: [0.33, 0.19, 0.10, 1.0],
            accent_color: [0.92, 0.86, 0.74, 1.0],
            packed_light: FULL_BRIGHT,
            animation: None,
            chicken_wing_flap_radians: None,
        }
    }

    pub fn cow_model(feet_position: Vec3, y_rot_degrees: f32, width: f32, height: f32) -> Self {
        Self {
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            pitch_radians: 0.0,
            rotation_pivot: Vec3::ZERO,
            orientation: None,
            shape: ActorInstanceShape::CowModel,
            first_person_body_only: false,
            width,
            height,
            body_color: [0.28, 0.17, 0.10, 1.0],
            accent_color: [0.90, 0.86, 0.72, 1.0],
            packed_light: FULL_BRIGHT,
            animation: None,
            chicken_wing_flap_radians: None,
        }
    }

    pub fn chicken_placeholder(
        feet_position: Vec3,
        y_rot_degrees: f32,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            pitch_radians: 0.0,
            rotation_pivot: Vec3::ZERO,
            orientation: None,
            shape: ActorInstanceShape::QuadrupedPlaceholder,
            first_person_body_only: false,
            width,
            height,
            body_color: [0.92, 0.90, 0.82, 1.0],
            accent_color: [0.92, 0.18, 0.12, 1.0],
            packed_light: FULL_BRIGHT,
            animation: None,
            chicken_wing_flap_radians: None,
        }
    }

    pub fn debug_cube(
        feet_position: Vec3,
        y_rot_degrees: f32,
        x_rot_degrees: f32,
        orientation: Option<Quat>,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            pitch_radians: x_rot_degrees.to_radians(),
            rotation_pivot: Vec3::new(0.0, height.max(0.1) * 0.5, 0.0),
            orientation: orientation.map(|rotation| rotation.normalize()),
            shape: ActorInstanceShape::DebugCube,
            first_person_body_only: false,
            width,
            height,
            body_color: [0.13, 0.48, 0.72, 1.0],
            accent_color: [0.95, 0.78, 0.22, 1.0],
            packed_light: FULL_BRIGHT,
            animation: None,
            chicken_wing_flap_radians: None,
        }
    }

    pub fn with_packed_light(mut self, packed_light: u32) -> Self {
        self.packed_light = packed_light;
        self
    }

    pub fn with_first_person_body_only(mut self, first_person_body_only: bool) -> Self {
        self.first_person_body_only = first_person_body_only;
        self
    }

    pub fn with_dimensions(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_walk_animation_distance(mut self, distance: f32) -> Self {
        if distance.is_finite() {
            self.animation = Some(ActorAnimation {
                clip: ActorAnimationClip::Walk,
                distance,
            });
        }
        self
    }

    pub fn with_chicken_wing_flap_radians(mut self, radians: Option<f32>) -> Self {
        self.chicken_wing_flap_radians = radians.filter(|radians| radians.is_finite());
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActorRenderStats {
    pub submitted_actor_count: usize,
    pub drawn_actor_count: usize,
    pub vertex_count: u32,
    pub index_count: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct ActorTextureAtlas<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
    pub layout: ActorTextureLayout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActorTextureLayout {
    pub white: ActorTextureRegion,
    pub cow: ActorTextureRegion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActorTextureRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub struct ActorDrawResources {
    renderer: ActorRenderer,
    atlas: GpuActorTextureAtlas,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
    actor_figures: ActorFigureSet,
}

impl ActorDrawResources {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        atlas: ActorTextureAtlas<'_>,
        actor_figures: Option<&ActorFigureSet>,
    ) -> Result<Self> {
        let renderer = ActorRenderer::new(device, color_format);
        let gpu_atlas =
            GpuActorTextureAtlas::new(device, queue, &renderer.texture_bind_group_layout, atlas)?;
        Ok(Self {
            renderer,
            atlas: gpu_atlas,
            texture_layout: atlas.layout,
            atlas_size: [atlas.width.max(1), atlas.height.max(1)],
            actor_figures: actor_figures.cloned().unwrap_or_default(),
        })
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_view: ChunkRenderView,
        render_options: TexturedSectionRenderOptions,
        actors: &[ActorInstance],
    ) -> Result<ActorRenderStats> {
        self.render_in_slot(
            device,
            queue,
            encoder,
            target,
            render_view,
            render_options,
            actors,
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
        render_view: ChunkRenderView,
        render_options: TexturedSectionRenderOptions,
        actors: &[ActorInstance],
        view_slot: PerViewSlot,
    ) -> Result<ActorRenderStats> {
        if actors.is_empty() {
            return Ok(ActorRenderStats::default());
        }
        let depth_view = target
            .depth_view
            .context("actor render pass requires a depth attachment")?;
        let mesh = actor_mesh(
            actors,
            self.texture_layout,
            self.atlas_size,
            &self.actor_figures,
        );
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return Ok(ActorRenderStats {
                submitted_actor_count: actors.len(),
                ..ActorRenderStats::default()
            });
        }

        let uniform_offset = self.renderer.uniforms.write_slot(
            queue,
            view_slot,
            &uniform_bytes(render_view, render_options),
        );
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_actor_vertices"),
            contents: &actor_vertex_bytes(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_actor_indices"),
            contents: &index_bytes(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_actor_render_pass"),
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
            ..Default::default()
        });
        pass.set_pipeline(&self.renderer.pipeline);
        pass.set_bind_group(0, &self.renderer.bind_group, &[uniform_offset]);
        pass.set_bind_group(1, &self.atlas.bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);

        Ok(ActorRenderStats {
            submitted_actor_count: actors.len(),
            drawn_actor_count: actors.len(),
            vertex_count: mesh.vertices.len() as u32,
            index_count: mesh.indices.len() as u32,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_multiview(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_views: [ChunkRenderView; 2],
        render_options: [TexturedSectionRenderOptions; 2],
        actors: &[ActorInstance],
    ) -> Result<ActorRenderStats> {
        if actors.is_empty() {
            return Ok(ActorRenderStats::default());
        }
        let depth_view = target
            .depth_view
            .context("actor multiview render pass requires a depth attachment")?;
        let mesh = actor_mesh(
            actors,
            self.texture_layout,
            self.atlas_size,
            &self.actor_figures,
        );
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return Ok(ActorRenderStats {
                submitted_actor_count: actors.len(),
                ..ActorRenderStats::default()
            });
        }

        let renderer = self.renderer.multiview_renderer(device)?;
        renderer.write_uniforms(queue, render_views, render_options);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_actor_multiview_vertices"),
            contents: &actor_vertex_bytes(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_actor_multiview_indices"),
            contents: &index_bytes(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_actor_multiview_render_pass"),
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
            ..Default::default()
        });
        pass.set_pipeline(&renderer.pipeline);
        pass.set_bind_group(0, &renderer.bind_group, &[]);
        pass.set_bind_group(1, &self.atlas.bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);

        Ok(ActorRenderStats {
            submitted_actor_count: actors.len(),
            drawn_actor_count: actors.len(),
            vertex_count: mesh.vertices.len() as u32,
            index_count: mesh.indices.len() as u32,
        })
    }
}

struct ActorRenderer {
    pipeline: wgpu::RenderPipeline,
    uniforms: PerViewUniformBuffer,
    bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    color_format: wgpu::TextureFormat,
    multiview: RefCell<Option<ActorMultiviewRenderer>>,
}

impl ActorRenderer {
    fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_actor_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/entity_actor.wgsl").into()),
        });
        let uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_actor_uniforms",
            UNIFORM_BYTE_SIZE,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_actor_bind_group_layout"),
            entries: &[uniforms.layout_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_actor_bind_group"),
            layout: &bind_group_layout,
            entries: &[uniforms.bind_group_entry(0)],
        });
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_actor_texture_bind_group_layout"),
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
            label: Some("mclone_actor_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = create_actor_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_actor_pipeline",
            None,
        );

        Self {
            pipeline,
            uniforms,
            bind_group,
            texture_bind_group_layout,
            color_format,
            multiview: RefCell::new(None),
        }
    }

    fn multiview_renderer(
        &self,
        device: &wgpu::Device,
    ) -> Result<std::cell::Ref<'_, ActorMultiviewRenderer>> {
        if !device.features().contains(wgpu::Features::MULTIVIEW) {
            bail!("actor multiview render requires wgpu MULTIVIEW");
        }
        if self.multiview.borrow().is_none() {
            let renderer = ActorMultiviewRenderer::new(
                device,
                self.color_format,
                &self.texture_bind_group_layout,
            );
            *self.multiview.borrow_mut() = Some(renderer);
        }
        Ok(std::cell::Ref::map(self.multiview.borrow(), |renderer| {
            renderer
                .as_ref()
                .expect("actor multiview renderer initialized above")
        }))
    }
}

struct ActorMultiviewRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl ActorMultiviewRenderer {
    fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_actor_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/entity_actor_multiview.wgsl").into(),
            ),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_actor_multiview_uniforms"),
            size: MULTIVIEW_UNIFORM_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_actor_multiview_uniform_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(MULTIVIEW_UNIFORM_BYTE_SIZE),
                    },
                    count: None,
                }],
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_actor_multiview_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_actor_multiview_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = create_actor_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_actor_multiview_pipeline",
            NonZeroU32::new(2),
        );
        Self {
            pipeline,
            uniform_buffer,
            bind_group,
        }
    }

    fn write_uniforms(
        &self,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        render_options: [TexturedSectionRenderOptions; 2],
    ) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            &multiview_uniform_bytes(render_views, render_options),
        );
    }
}

fn create_actor_pipeline(
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
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: ACTOR_VERTEX_BYTE_SIZE,
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
                        format: wgpu::VertexFormat::Float32x2,
                    },
                    wgpu::VertexAttribute {
                        offset: 20,
                        shader_location: 2,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                    wgpu::VertexAttribute {
                        offset: 36,
                        shader_location: 3,
                        format: wgpu::VertexFormat::Uint32,
                    },
                ],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
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
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview,
        cache: None,
    })
}

struct GpuActorTextureAtlas {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
}

impl GpuActorTextureAtlas {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        atlas: ActorTextureAtlas<'_>,
    ) -> Result<Self> {
        let width = atlas.width.max(1);
        let height = atlas.height.max(1);
        let expected_len = width as usize * height as usize * 4;
        if atlas.rgba.len() != expected_len {
            bail!(
                "actor texture atlas has {} bytes; expected {expected_len} for {}x{} RGBA",
                atlas.rgba.len(),
                width,
                height
            );
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_actor_texture_atlas"),
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
            label: Some("mclone_actor_texture_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_actor_texture_bind_group"),
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

#[derive(Clone, Debug, Default, PartialEq)]
struct ActorMesh {
    vertices: Vec<ActorVertex>,
    indices: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActorVertex {
    position: [f32; 3],
    uv: [f32; 2],
    color: [f32; 4],
    packed_light: u32,
}

fn actor_mesh(
    actors: &[ActorInstance],
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
    actor_figures: &ActorFigureSet,
) -> ActorMesh {
    let mut mesh = ActorMesh::default();
    for actor in actors {
        append_actor(&mut mesh, *actor, texture_layout, atlas_size, actor_figures);
    }
    mesh
}

fn append_actor(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
    actor_figures: &ActorFigureSet,
) {
    match actor.shape {
        ActorInstanceShape::Figure(figure) => append_asset_lab_figure_model(
            mesh,
            actor,
            texture_layout,
            atlas_size,
            actor_figures.get(figure),
        ),
        ActorInstanceShape::Humanoid => {
            append_humanoid_model(mesh, actor, texture_layout, atlas_size)
        }
        ActorInstanceShape::QuadrupedPlaceholder => {
            append_quadruped_placeholder(mesh, actor, texture_layout, atlas_size)
        }
        ActorInstanceShape::CowModel => append_cow_model(mesh, actor, texture_layout, atlas_size),
        ActorInstanceShape::DebugCube => append_debug_cube(mesh, actor, texture_layout, atlas_size),
    }
}

fn append_asset_lab_figure_model(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
    figure: Option<&ActorFigure>,
) {
    let Some(figure) = figure else {
        append_humanoid_model(mesh, actor, texture_layout, atlas_size);
        return;
    };

    let white_uv = texture_region_center_uv(texture_layout.white, atlas_size);
    let model_scale = actor.height.max(0.1);
    let sampled_transforms =
        sampled_figure_part_transforms(figure, actor.animation, actor.chicken_wing_flap_radians);
    let content_matrices = figure_content_matrices(figure, &sampled_transforms);
    for (part_index, part) in figure.parts.iter().enumerate() {
        let content_matrix = content_matrices[part_index];
        for cuboid in &part.cuboids {
            if actor.first_person_body_only && !cuboid.first_person_visible {
                continue;
            }
            append_asset_lab_local_box(
                mesh,
                actor,
                figure,
                content_matrix,
                cuboid.min,
                cuboid.max,
                white_uv,
                lab_order_face_colors(cuboid.face_colors),
                model_scale,
            );
        }
        for cuboid in &part.overlay_cuboids {
            if actor.first_person_body_only && !cuboid.first_person_visible {
                continue;
            }
            append_asset_lab_local_box(
                mesh,
                actor,
                figure,
                content_matrix,
                cuboid.min,
                cuboid.max,
                white_uv,
                [cuboid.color; 6],
                model_scale,
            );
        }
    }
}

fn sampled_figure_part_transforms(
    figure: &ActorFigure,
    animation: Option<ActorAnimation>,
    chicken_wing_flap_radians: Option<f32>,
) -> Vec<CompiledFigureTransform> {
    let mut transforms = vec![CompiledFigureTransform::default(); figure.parts.len()];
    if let Some(animation) = animation {
        let clip = match animation.clip {
            ActorAnimationClip::Walk => figure.clips.get("walk"),
        };
        if let Some(clip) = clip {
            let time_seconds = clip_time_for_animation_distance(clip, animation.distance);
            for (part_index, keys) in &clip.tracks {
                if *part_index < transforms.len() {
                    transforms[*part_index] = sample_clip_track(keys, time_seconds, clip);
                }
            }
        }
    }
    if let Some(wing_flap) = chicken_wing_flap_radians {
        apply_chicken_wing_flap(figure, &mut transforms, wing_flap);
    }
    transforms
}

fn apply_chicken_wing_flap(
    figure: &ActorFigure,
    transforms: &mut [CompiledFigureTransform],
    wing_flap_radians: f32,
) {
    if !wing_flap_radians.is_finite() || wing_flap_radians.abs() <= f32::EPSILON {
        return;
    }
    for (part_index, part) in figure.parts.iter().enumerate() {
        let z_delta = match part.name.as_str() {
            "wing_l" => wing_flap_radians,
            "wing_r" => -wing_flap_radians,
            _ => continue,
        };
        if let Some(transform) = transforms.get_mut(part_index) {
            let rot = transform.rot_radians.unwrap_or(Vec3::ZERO);
            transform.rot_radians = Some(rot + Vec3::new(0.0, 0.0, z_delta));
        }
    }
}

fn clip_time_for_animation_distance(clip: &CompiledFigureClip, distance: f32) -> f32 {
    if clip.duration_seconds <= 0.0 {
        return 0.0;
    }
    if let Some(locomotion) = &clip.locomotion
        && locomotion.cycle_distance > 0.0
    {
        return (distance.max(0.0) / locomotion.cycle_distance) * clip.duration_seconds;
    }
    distance.max(0.0)
}

fn sample_clip_track(
    keys: &[crate::asset_lab_figure::CompiledFigureKey],
    time_seconds: f32,
    clip: &CompiledFigureClip,
) -> CompiledFigureTransform {
    let Some(first) = keys.first() else {
        return CompiledFigureTransform::default();
    };
    let local_time = if clip.looped && clip.duration_seconds > 0.0 {
        time_seconds.rem_euclid(clip.duration_seconds)
    } else {
        time_seconds.clamp(0.0, clip.duration_seconds)
    };
    let mut left = first;
    let mut right = keys.last().unwrap_or(first);
    for (index, current) in keys.iter().enumerate() {
        let next = keys.get(index + 1);
        if next.is_none_or(|next| local_time < next.time_seconds) {
            left = current;
            right = next.unwrap_or(current);
            break;
        }
    }
    let alpha = if (right.time_seconds - left.time_seconds).abs() <= f32::EPSILON {
        0.0
    } else {
        ((local_time - left.time_seconds) / (right.time_seconds - left.time_seconds))
            .clamp(0.0, 1.0)
    };
    CompiledFigureTransform {
        at: mix_optional_vec3(left.transform.at, right.transform.at, alpha),
        rot_radians: mix_optional_vec3(
            left.transform.rot_radians,
            right.transform.rot_radians,
            alpha,
        ),
    }
}

fn mix_optional_vec3(left: Option<Vec3>, right: Option<Vec3>, alpha: f32) -> Option<Vec3> {
    match (left, right) {
        (None, None) => None,
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (Some(left), Some(right)) => Some(left + (right - left) * alpha),
    }
}

fn figure_content_matrices(
    figure: &ActorFigure,
    transforms: &[CompiledFigureTransform],
) -> Vec<Mat4> {
    let mut cache = vec![None; figure.parts.len()];
    for index in 0..figure.parts.len() {
        let _ = figure_content_matrix(index, figure, transforms, &mut cache);
    }
    cache
        .into_iter()
        .map(|matrix| matrix.unwrap_or(Mat4::IDENTITY))
        .collect()
}

fn figure_content_matrix(
    index: usize,
    figure: &ActorFigure,
    transforms: &[CompiledFigureTransform],
    cache: &mut [Option<Mat4>],
) -> Mat4 {
    if let Some(matrix) = cache[index] {
        return matrix;
    }
    let part = &figure.parts[index];
    let parent_content = part
        .parent
        .map(|parent| figure_content_matrix(parent, figure, transforms, cache))
        .unwrap_or(Mat4::IDENTITY);
    let transform = transforms.get(index).copied().unwrap_or_default();
    let position = part.base_position + transform.at.unwrap_or(Vec3::ZERO);
    let rotation = part.base_rotation_radians + transform.rot_radians.unwrap_or(Vec3::ZERO);
    let group = parent_content
        * Mat4::from_translation(position)
        * Mat4::from_quat(Quat::from_euler(
            EulerRot::XYZ,
            rotation.x,
            rotation.y,
            rotation.z,
        ));
    let content = group * Mat4::from_translation(-part.pivot);
    cache[index] = Some(content);
    content
}

#[allow(clippy::too_many_arguments)]
fn append_asset_lab_local_box(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    figure: &ActorFigure,
    content_matrix: Mat4,
    min: Vec3,
    max: Vec3,
    uv: [f32; 2],
    face_colors: [[f32; 4]; 6],
    model_scale: f32,
) {
    let corners = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];
    append_transformed_box(
        mesh,
        face_colors,
        [[uv; 4]; 6],
        actor.packed_light,
        |corner_index| {
            let lab_position = content_matrix.transform_point3(corners[corner_index]);
            actor_world_position(
                actor,
                lab_point_to_actor_local(figure, lab_position) * model_scale,
            )
        },
    );
}

fn lab_point_to_actor_local(figure: &ActorFigure, point: Vec3) -> Vec3 {
    let normalized = (point - figure.normalization_origin) * figure.inv_height;
    Vec3::new(normalized.x, normalized.y, -normalized.z)
}

fn lab_order_face_colors(actor_order: [[f32; 4]; 6]) -> [[f32; 4]; 6] {
    [
        actor_order[1],
        actor_order[0],
        actor_order[2],
        actor_order[3],
        actor_order[4],
        actor_order[5],
    ]
}

fn append_humanoid_model(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
) {
    let white_uv = texture_region_center_uv(texture_layout.white, atlas_size);
    let clothing_dark = scale_color(actor.body_color, 0.48);
    let clothing_side = scale_color(actor.body_color, 0.78);
    let clothing_light = scale_color(actor.body_color, 1.10);
    let skin_shadow = scale_color(actor.accent_color, 0.72);
    let skin_side = scale_color(actor.accent_color, 0.88);
    let skin_light = scale_color(actor.accent_color, 1.05);
    let hair = scale_color(actor.body_color, 0.22);
    let eye = [0.04, 0.035, 0.03, 1.0];
    let mouth = [0.36, 0.11, 0.10, 1.0];
    append_box(
        mesh,
        actor,
        Vec3::new(-0.21, 0.0, -0.12),
        Vec3::new(-0.04, 0.76, 0.12),
        white_uv,
        [
            clothing_dark,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_side,
            actor.body_color,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(0.04, 0.0, -0.12),
        Vec3::new(0.21, 0.76, 0.12),
        white_uv,
        [
            clothing_dark,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_side,
            actor.body_color,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.30, 0.74, -0.15),
        Vec3::new(0.30, 1.36, 0.15),
        white_uv,
        [
            clothing_dark,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_light,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.46, 0.62, -0.12),
        Vec3::new(-0.31, 1.30, 0.12),
        white_uv,
        [
            clothing_dark,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_light,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(0.31, 0.62, -0.12),
        Vec3::new(0.46, 1.30, 0.12),
        white_uv,
        [
            clothing_dark,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_side,
            clothing_light,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.45, 0.48, -0.11),
        Vec3::new(-0.32, 0.64, 0.11),
        white_uv,
        [
            skin_shadow,
            skin_side,
            skin_side,
            skin_side,
            skin_side,
            skin_light,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(0.32, 0.48, -0.11),
        Vec3::new(0.45, 0.64, 0.11),
        white_uv,
        [
            skin_shadow,
            skin_side,
            skin_side,
            skin_side,
            skin_side,
            skin_light,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.24, 1.34, -0.24),
        Vec3::new(0.24, 1.80, 0.24),
        white_uv,
        [
            skin_shadow,
            skin_side,
            skin_side,
            skin_side,
            skin_side,
            skin_light,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.255, 1.66, -0.255),
        Vec3::new(0.255, 1.84, 0.255),
        white_uv,
        [
            scale_color(hair, 0.70),
            hair,
            hair,
            scale_color(hair, 1.12),
            scale_color(hair, 0.92),
            scale_color(hair, 1.18),
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.135, 1.56, 0.236),
        Vec3::new(-0.065, 1.635, 0.258),
        white_uv,
        [eye; 6],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(0.065, 1.56, 0.236),
        Vec3::new(0.135, 1.635, 0.258),
        white_uv,
        [eye; 6],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.075, 1.455, 0.237),
        Vec3::new(0.075, 1.500, 0.258),
        white_uv,
        [mouth; 6],
    );
}

fn append_quadruped_placeholder(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
) {
    let white_uv = texture_region_center_uv(texture_layout.white, atlas_size);
    let width = actor.width.max(0.1);
    let height = actor.height.max(0.1);
    let half_width = width * 0.5;
    let leg_half = (width * 0.12).clamp(0.04, 0.16);
    let body_bottom = height * 0.32;
    let body_top = height * 0.82;
    let body_back = -width * 0.62;
    let body_front = width * 0.46;
    let head_bottom = height * 0.54;
    let head_top = height;
    let head_half = width * 0.28;
    let head_front = body_front + width * 0.38;

    let dark = scale_color(actor.body_color, 0.58);
    let side = scale_color(actor.body_color, 0.78);
    let light = scale_color(actor.body_color, 1.12);
    let accent_side = scale_color(actor.accent_color, 0.86);
    append_box(
        mesh,
        actor,
        Vec3::new(-half_width, body_bottom, body_back),
        Vec3::new(half_width, body_top, body_front),
        white_uv,
        [dark, side, side, side, side, light],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-head_half, head_bottom, body_front),
        Vec3::new(head_half, head_top, head_front),
        white_uv,
        [
            scale_color(actor.accent_color, 0.70),
            accent_side,
            accent_side,
            scale_color(actor.accent_color, 0.92),
            scale_color(actor.accent_color, 0.92),
            actor.accent_color,
        ],
    );
    for x in [-half_width + leg_half * 1.2, half_width - leg_half * 1.2] {
        for z in [body_back + leg_half * 1.2, body_front - leg_half * 1.2] {
            append_box(
                mesh,
                actor,
                Vec3::new(x - leg_half, 0.0, z - leg_half),
                Vec3::new(x + leg_half, body_bottom, z + leg_half),
                white_uv,
                [dark, side, side, side, side, actor.body_color],
            );
        }
    }
}

fn append_debug_cube(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
) {
    let white_uv = texture_region_center_uv(texture_layout.white, atlas_size);
    let width = actor.width.max(0.1);
    let height = actor.height.max(0.1);
    let half_width = width * 0.5;
    append_box(
        mesh,
        actor,
        Vec3::new(-half_width, 0.0, -half_width),
        Vec3::new(half_width, height, half_width),
        white_uv,
        [
            scale_color(actor.body_color, 0.56),
            scale_color(actor.body_color, 0.78),
            scale_color(actor.body_color, 0.84),
            scale_color(actor.accent_color, 0.82),
            actor.body_color,
            actor.accent_color,
        ],
    );
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActorModelPart {
    offset_pixels: [f32; 3],
    rotation_radians: [f32; 3],
    cuboids: &'static [ActorModelCuboid],
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActorModelCuboid {
    texture_offset: [u16; 2],
    origin_pixels: [f32; 3],
    size_pixels: [f32; 3],
}

const COW_HEAD_CUBOIDS: &[ActorModelCuboid] = &[
    ActorModelCuboid {
        texture_offset: [0, 0],
        origin_pixels: [-4.0, -4.0, -6.0],
        size_pixels: [8.0, 8.0, 6.0],
    },
    ActorModelCuboid {
        texture_offset: [22, 0],
        origin_pixels: [-5.0, -5.0, -4.0],
        size_pixels: [1.0, 3.0, 1.0],
    },
    ActorModelCuboid {
        texture_offset: [22, 0],
        origin_pixels: [4.0, -5.0, -4.0],
        size_pixels: [1.0, 3.0, 1.0],
    },
];

const COW_BODY_CUBOIDS: &[ActorModelCuboid] = &[
    ActorModelCuboid {
        texture_offset: [18, 4],
        origin_pixels: [-6.0, -10.0, -7.0],
        size_pixels: [12.0, 18.0, 10.0],
    },
    ActorModelCuboid {
        texture_offset: [52, 0],
        origin_pixels: [-2.0, 2.0, -8.0],
        size_pixels: [4.0, 6.0, 1.0],
    },
];

const COW_LEG_CUBOIDS: &[ActorModelCuboid] = &[ActorModelCuboid {
    texture_offset: [0, 16],
    origin_pixels: [-2.0, 0.0, -2.0],
    size_pixels: [4.0, 12.0, 4.0],
}];

const COW_MODEL_PARTS: &[ActorModelPart] = &[
    ActorModelPart {
        offset_pixels: [0.0, 4.0, -8.0],
        rotation_radians: [0.0, 0.0, 0.0],
        cuboids: COW_HEAD_CUBOIDS,
    },
    ActorModelPart {
        offset_pixels: [0.0, 5.0, 2.0],
        rotation_radians: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
        cuboids: COW_BODY_CUBOIDS,
    },
    ActorModelPart {
        offset_pixels: [-4.0, 12.0, 7.0],
        rotation_radians: [0.0, 0.0, 0.0],
        cuboids: COW_LEG_CUBOIDS,
    },
    ActorModelPart {
        offset_pixels: [4.0, 12.0, 7.0],
        rotation_radians: [0.0, 0.0, 0.0],
        cuboids: COW_LEG_CUBOIDS,
    },
    ActorModelPart {
        offset_pixels: [-4.0, 12.0, -6.0],
        rotation_radians: [0.0, 0.0, 0.0],
        cuboids: COW_LEG_CUBOIDS,
    },
    ActorModelPart {
        offset_pixels: [4.0, 12.0, -6.0],
        rotation_radians: [0.0, 0.0, 0.0],
        cuboids: COW_LEG_CUBOIDS,
    },
];

fn append_cow_model(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    texture_layout: ActorTextureLayout,
    atlas_size: [u32; 2],
) {
    for part in COW_MODEL_PARTS {
        for cuboid in part.cuboids {
            append_model_cuboid(
                mesh,
                actor,
                part,
                cuboid,
                cow_cuboid_face_colors(*cuboid),
                texture_layout.cow,
                atlas_size,
            );
        }
    }
}

fn append_model_cuboid(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    part: &ActorModelPart,
    cuboid: &ActorModelCuboid,
    face_colors: [[f32; 4]; 6],
    texture_region: ActorTextureRegion,
    atlas_size: [u32; 2],
) {
    let origin = Vec3::from_array(cuboid.origin_pixels);
    let size = Vec3::from_array(cuboid.size_pixels);
    let max = origin + size;
    let corners = [
        Vec3::new(origin.x, origin.y, origin.z),
        Vec3::new(max.x, origin.y, origin.z),
        Vec3::new(max.x, max.y, origin.z),
        Vec3::new(origin.x, max.y, origin.z),
        Vec3::new(origin.x, origin.y, max.z),
        Vec3::new(max.x, origin.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(origin.x, max.y, max.z),
    ];
    append_transformed_box(
        mesh,
        face_colors,
        cow_cuboid_face_uvs(*cuboid, texture_region, atlas_size),
        actor.packed_light,
        |corner_index| {
            let model_position = transform_model_part_point(corners[corner_index], part);
            actor_world_position(actor, model_pixels_to_actor_local(model_position))
        },
    );
}

fn append_transformed_box(
    mesh: &mut ActorMesh,
    face_colors: [[f32; 4]; 6],
    face_uvs: [[[f32; 2]; 4]; 6],
    packed_light: u32,
    mut world_corner: impl FnMut(usize) -> Vec3,
) {
    let faces = [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 4, 7, 3],
        [1, 2, 6, 5],
        [0, 1, 5, 4],
        [3, 7, 6, 2],
    ];

    for (face_index, face) in faces.into_iter().enumerate() {
        let base = mesh.vertices.len() as u32;
        for (vertex_index, corner_index) in face.into_iter().enumerate() {
            mesh.vertices.push(ActorVertex {
                position: world_corner(corner_index).to_array(),
                uv: face_uvs[face_index][vertex_index],
                color: face_colors[face_index],
                packed_light,
            });
        }
        mesh.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

fn transform_model_part_point(point_pixels: Vec3, part: &ActorModelPart) -> Vec3 {
    rotate_model_point(point_pixels, part.rotation_radians) + Vec3::from_array(part.offset_pixels)
}

fn rotate_model_point(point: Vec3, rotation_radians: [f32; 3]) -> Vec3 {
    let [x_rot, y_rot, z_rot] = rotation_radians;
    let mut rotated = point;
    if z_rot != 0.0 {
        let (sin, cos) = z_rot.sin_cos();
        rotated = Vec3::new(
            rotated.x * cos - rotated.y * sin,
            rotated.x * sin + rotated.y * cos,
            rotated.z,
        );
    }
    if y_rot != 0.0 {
        let (sin, cos) = y_rot.sin_cos();
        rotated = Vec3::new(
            rotated.x * cos + rotated.z * sin,
            rotated.y,
            -rotated.x * sin + rotated.z * cos,
        );
    }
    if x_rot != 0.0 {
        let (sin, cos) = x_rot.sin_cos();
        rotated = Vec3::new(
            rotated.x,
            rotated.y * cos - rotated.z * sin,
            rotated.y * sin + rotated.z * cos,
        );
    }
    rotated
}

fn model_pixels_to_actor_local(model_position: Vec3) -> Vec3 {
    Vec3::new(
        model_position.x * MODEL_PIXEL_SCALE,
        (MODEL_FEET_Y_PIXELS - model_position.y) * MODEL_PIXEL_SCALE,
        -model_position.z * MODEL_PIXEL_SCALE,
    )
}

fn cow_cuboid_face_colors(cuboid: ActorModelCuboid) -> [[f32; 4]; 6] {
    match cuboid.texture_offset {
        [0, 0] | [18, 4] | [22, 0] | [52, 0] | [0, 16] => [[1.0, 1.0, 1.0, 1.0]; 6],
        _ => shaded_faces([1.0, 1.0, 1.0, 1.0]),
    }
}

fn cow_cuboid_face_uvs(
    cuboid: ActorModelCuboid,
    texture_region: ActorTextureRegion,
    atlas_size: [u32; 2],
) -> [[[f32; 2]; 4]; 6] {
    let [u, v] = [
        cuboid.texture_offset[0] as f32,
        cuboid.texture_offset[1] as f32,
    ];
    let [width, height, depth] = cuboid.size_pixels;
    let west = face_uv_rect(
        texture_region,
        atlas_size,
        u,
        v + depth,
        u + depth,
        v + depth + height,
    );
    let north = face_uv_rect(
        texture_region,
        atlas_size,
        u + depth,
        v + depth,
        u + depth + width,
        v + depth + height,
    );
    let east = face_uv_rect(
        texture_region,
        atlas_size,
        u + depth + width,
        v + depth,
        u + depth + width + depth,
        v + depth + height,
    );
    let south = face_uv_rect(
        texture_region,
        atlas_size,
        u + depth + width + depth,
        v + depth,
        u + depth + width + depth + width,
        v + depth + height,
    );
    let down = face_uv_rect(
        texture_region,
        atlas_size,
        u + depth,
        v,
        u + depth + width,
        v + depth,
    );
    let up = face_uv_rect(
        texture_region,
        atlas_size,
        u + depth + width,
        v,
        u + depth + width + width,
        v + depth,
    );

    // append_transformed_box face order is minZ, maxZ, minX, maxX, minY, maxY.
    [north, south, west, east, down, up]
}

fn face_uv_rect(
    texture_region: ActorTextureRegion,
    atlas_size: [u32; 2],
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
) -> [[f32; 2]; 4] {
    [
        texture_region_uv(texture_region, atlas_size, u0, v0),
        texture_region_uv(texture_region, atlas_size, u0, v1),
        texture_region_uv(texture_region, atlas_size, u1, v1),
        texture_region_uv(texture_region, atlas_size, u1, v0),
    ]
}

fn texture_region_center_uv(region: ActorTextureRegion, atlas_size: [u32; 2]) -> [f32; 2] {
    texture_region_uv(
        region,
        atlas_size,
        0.5 * region.width as f32,
        0.5 * region.height as f32,
    )
}

fn texture_region_uv(
    region: ActorTextureRegion,
    atlas_size: [u32; 2],
    texture_u: f32,
    texture_v: f32,
) -> [f32; 2] {
    [
        (region.x as f32 + texture_u) / atlas_size[0].max(1) as f32,
        (region.y as f32 + texture_v) / atlas_size[1].max(1) as f32,
    ]
}

fn shaded_faces(color: [f32; 4]) -> [[f32; 4]; 6] {
    [
        scale_color(color, 0.58),
        scale_color(color, 0.90),
        scale_color(color, 0.72),
        scale_color(color, 0.82),
        scale_color(color, 0.66),
        scale_color(color, 1.08),
    ]
}

fn append_box(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    min: Vec3,
    max: Vec3,
    uv: [f32; 2],
    face_colors: [[f32; 4]; 6],
) {
    let corners = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];
    append_transformed_box(
        mesh,
        face_colors,
        [[uv; 4]; 6],
        actor.packed_light,
        |corner_index| actor_world_position(actor, corners[corner_index]),
    );
}

fn actor_world_position(actor: ActorInstance, local: Vec3) -> Vec3 {
    let local = local - actor.rotation_pivot;
    let local = if let Some(orientation) = actor.orientation {
        orientation * local
    } else {
        let (pitch_sin, pitch_cos) = actor.pitch_radians.sin_cos();
        let local = Vec3::new(
            local.x,
            local.y * pitch_cos - local.z * pitch_sin,
            local.y * pitch_sin + local.z * pitch_cos,
        );
        let (yaw_sin, yaw_cos) = actor.yaw_radians.sin_cos();
        Vec3::new(
            local.x * yaw_cos + local.z * yaw_sin,
            local.y,
            -local.x * yaw_sin + local.z * yaw_cos,
        )
    };
    actor.feet_position + actor.rotation_pivot + local
}

fn scale_color(color: [f32; 4], factor: f32) -> [f32; 4] {
    [
        (color[0] * factor).clamp(0.0, 1.0),
        (color[1] * factor).clamp(0.0, 1.0),
        (color[2] * factor).clamp(0.0, 1.0),
        color[3],
    ]
}

fn actor_vertex_bytes(vertices: &[ActorVertex]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vertices.len() * ACTOR_VERTEX_BYTE_LEN);
    for vertex in vertices {
        for value in vertex
            .position
            .into_iter()
            .chain(vertex.uv)
            .chain(vertex.color)
        {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        bytes.extend_from_slice(&vertex.packed_light.to_ne_bytes());
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

fn uniform_bytes(
    render_view: ChunkRenderView,
    render_options: TexturedSectionRenderOptions,
) -> [u8; UNIFORM_BYTE_LEN] {
    let mut bytes = [0_u8; UNIFORM_BYTE_LEN];
    let mut offset = 0;
    for value in render_view.uniform_matrix().into_iter().flatten() {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        offset += 4;
    }
    for value in [
        if render_options.force_fullbright {
            1.0
        } else {
            0.0
        },
        render_options.sky_darken.clamp(0.0, 1.0),
        if render_options.fog.enabled { 1.0 } else { 0.0 },
        0.0,
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        offset += 4;
    }
    let camera_position = render_view.camera_position.to_array();
    for value in [
        camera_position[0],
        camera_position[1],
        camera_position[2],
        0.0,
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        offset += 4;
    }
    for value in [
        render_options.fog.color[0],
        render_options.fog.color[1],
        render_options.fog.color[2],
        1.0,
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        offset += 4;
    }
    for value in [render_options.fog.start, render_options.fog.end, 0.0, 0.0] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        offset += 4;
    }
    bytes
}

fn multiview_uniform_bytes(
    render_views: [ChunkRenderView; 2],
    render_options: [TexturedSectionRenderOptions; 2],
) -> [u8; MULTIVIEW_UNIFORM_BYTE_LEN] {
    let mut bytes = [0u8; MULTIVIEW_UNIFORM_BYTE_LEN];
    bytes[..UNIFORM_BYTE_LEN].copy_from_slice(&uniform_bytes(render_views[0], render_options[0]));
    bytes[UNIFORM_BYTE_LEN..].copy_from_slice(&uniform_bytes(render_views[1], render_options[1]));
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_actor_texture_layout() -> ActorTextureLayout {
        ActorTextureLayout {
            white: ActorTextureRegion {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            cow: ActorTextureRegion {
                x: 1,
                y: 0,
                width: 64,
                height: 32,
            },
        }
    }

    fn test_actor_texture_atlas_size() -> [u32; 2] {
        [65, 32]
    }

    fn test_player_figure() -> ActorFigure {
        let json = include_str!("../../../../assets/mclone/figures/player.figure.json");
        let asset: mclone_assets::FigureAsset = serde_json::from_str(json).unwrap();
        crate::asset_lab_figure::compile_figure_asset(&asset).unwrap()
    }

    fn test_player_figures() -> ActorFigureSet {
        ActorFigureSet::new([(default_player_figure_id(), test_player_figure())])
    }

    fn test_chicken_figure() -> ActorFigure {
        let json = include_str!("../../../../assets/mclone/figures/chicken.figure.json");
        let asset: mclone_assets::FigureAsset = serde_json::from_str(json).unwrap();
        crate::asset_lab_figure::compile_figure_asset(&asset).unwrap()
    }

    fn test_chicken_figures() -> ActorFigureSet {
        ActorFigureSet::new([(mclone_assets::chicken_figure_id(), test_chicken_figure())])
    }

    #[test]
    fn actor_mesh_emits_asset_lab_player_model() {
        let figures = test_player_figures();
        let mesh = actor_mesh(
            &[ActorInstance::remote_player(Vec3::new(1.0, 2.0, 3.0), 0.0)],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );

        assert_eq!(mesh.vertices.len(), 76 * 6 * 4);
        assert_eq!(mesh.indices.len(), 76 * 6 * 6);
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| vertex.uv == [0.5 / 65.0, 0.5 / 32.0])
        );

        let bounds = mesh_bounds(&mesh);
        assert!((bounds.min.y - 2.0).abs() < 1.0e-6);
        assert!((bounds.max.y - 3.8).abs() < 1.0e-6);
        assert!(bounds.min.x < 0.56);
        assert!(bounds.max.x > 1.44);
        assert!(bounds.max.z > 3.18);
    }

    #[test]
    fn asset_lab_player_model_has_front_face_details() {
        let actor = ActorInstance::remote_player(Vec3::ZERO, 0.0);
        let figures = test_player_figures();
        let mesh = actor_mesh(
            &[actor],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );
        let dark_detail_vertices = mesh
            .vertices
            .iter()
            .filter(|vertex| vertex.color == [25.0 / 255.0, 18.0 / 255.0, 14.0 / 255.0, 1.0])
            .count();

        assert_eq!(dark_detail_vertices, 8 * 6 * 4);
        assert!(
            mesh.vertices
                .iter()
                .filter(|vertex| vertex.color == [25.0 / 255.0, 18.0 / 255.0, 14.0 / 255.0, 1.0])
                .all(|vertex| vertex.position[2] > 0.18)
        );
    }

    #[test]
    fn asset_lab_player_walk_animation_changes_vertices() {
        let figures = test_player_figures();
        let static_mesh = actor_mesh(
            &[ActorInstance::remote_player(Vec3::ZERO, 0.0)],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );
        let animated_mesh = actor_mesh(
            &[ActorInstance::remote_player(Vec3::ZERO, 0.0).with_walk_animation_distance(0.25)],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );

        assert_eq!(animated_mesh.vertices.len(), static_mesh.vertices.len());
        assert_eq!(animated_mesh.indices.len(), static_mesh.indices.len());
        assert_ne!(animated_mesh.vertices, static_mesh.vertices);
    }

    #[test]
    fn asset_lab_chicken_wing_flap_changes_vertices() {
        let figures = test_chicken_figures();
        let static_actor = ActorInstance::remote_player_with_figure(
            Vec3::ZERO,
            0.0,
            mclone_assets::chicken_figure_id(),
        )
        .with_dimensions(0.4, 0.7);
        let static_mesh = actor_mesh(
            &[static_actor],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );
        let flapping_mesh = actor_mesh(
            &[static_actor.with_chicken_wing_flap_radians(Some(0.45))],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );

        assert_eq!(flapping_mesh.vertices.len(), static_mesh.vertices.len());
        assert_eq!(flapping_mesh.indices.len(), static_mesh.indices.len());
        assert_ne!(flapping_mesh.vertices, static_mesh.vertices);
    }

    #[test]
    fn first_person_asset_lab_player_model_hides_head_and_face_details() {
        let actor = ActorInstance::local_player(Vec3::ZERO, 0.0).with_first_person_body_only(true);
        let figures = test_player_figures();
        let mesh = actor_mesh(
            &[actor],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &figures,
        );

        assert_eq!(mesh.vertices.len(), 10 * 6 * 4);
        assert_eq!(mesh.indices.len(), 10 * 6 * 6);
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| vertex.color != [25.0 / 255.0, 18.0 / 255.0, 14.0 / 255.0, 1.0])
        );

        let bounds = mesh_bounds(&mesh);
        assert!(bounds.max.y < 1.62);
    }

    #[test]
    fn actor_mesh_still_emits_hardcoded_humanoid_debug_shape() {
        let actor = ActorInstance {
            shape: ActorInstanceShape::Humanoid,
            ..ActorInstance::remote_player(Vec3::ZERO, 0.0)
        };
        let mesh = actor_mesh(
            &[actor],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &ActorFigureSet::default(),
        );

        assert_eq!(mesh.vertices.len(), 12 * 6 * 4);
        assert_eq!(mesh.indices.len(), 12 * 6 * 6);
    }

    #[test]
    fn actor_mesh_uses_humanoid_fallback_for_unknown_figure_ids() {
        let actor = ActorInstance::remote_player_with_figure(
            Vec3::ZERO,
            0.0,
            mclone_assets::ActorFigureId::from_static("mclone:missing"),
        );
        let mesh = actor_mesh(
            &[actor],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &ActorFigureSet::default(),
        );

        assert_eq!(mesh.vertices.len(), 12 * 6 * 4);
        assert_eq!(mesh.indices.len(), 12 * 6 * 6);
    }

    #[test]
    fn actor_mesh_emits_quadruped_placeholder() {
        let mesh = actor_mesh(
            &[ActorInstance::cow_placeholder(
                Vec3::new(1.0, 2.0, 3.0),
                0.0,
                0.9,
                1.4,
            )],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &ActorFigureSet::default(),
        );

        assert_eq!(mesh.vertices.len(), 6 * 6 * 4);
        assert_eq!(mesh.indices.len(), 6 * 6 * 6);
    }

    #[test]
    fn actor_mesh_emits_debug_cube() {
        let mesh = actor_mesh(
            &[ActorInstance::debug_cube(
                Vec3::ZERO,
                0.0,
                0.0,
                None,
                1.0,
                1.0,
            )],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &ActorFigureSet::default(),
        );

        assert_eq!(mesh.vertices.len(), 6 * 4);
        assert_eq!(mesh.indices.len(), 6 * 6);
    }

    #[test]
    fn debug_cube_pitch_rotates_around_center() {
        let actor = ActorInstance::debug_cube(Vec3::ZERO, 0.0, 90.0, None, 1.0, 1.0);
        let bottom_center = actor_world_position(actor, Vec3::new(0.0, 0.0, 0.0));
        let top_center = actor_world_position(actor, Vec3::new(0.0, 1.0, 0.0));

        assert!((bottom_center.y - 0.5).abs() < 1.0e-6);
        assert!((top_center.y - 0.5).abs() < 1.0e-6);
        assert!((bottom_center.z + 0.5).abs() < 1.0e-6);
        assert!((top_center.z - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn debug_cube_quaternion_orientation_rotates_around_center() {
        let actor = ActorInstance::debug_cube(
            Vec3::ZERO,
            0.0,
            0.0,
            Some(Quat::from_rotation_z(90.0_f32.to_radians())),
            1.0,
            1.0,
        );
        let left_center = actor_world_position(actor, Vec3::new(-0.5, 0.5, 0.0));
        let right_center = actor_world_position(actor, Vec3::new(0.5, 0.5, 0.0));

        assert!((left_center.x - 0.0).abs() < 1.0e-6);
        assert!((right_center.x - 0.0).abs() < 1.0e-6);
        assert!((left_center.y - 0.0).abs() < 1.0e-6);
        assert!((right_center.y - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn actor_uniform_serializes_underwater_fog() {
        let render_view = crate::chunk::ChunkCamera {
            eye: [4.0, 5.0, 6.0],
            target: [4.0, 5.0, 5.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 70.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 256.0,
        }
        .render_view(640, 480);
        let bytes = uniform_bytes(
            render_view,
            TexturedSectionRenderOptions::default().with_fog(crate::fog::RenderFog::underwater()),
        );

        assert_eq!(bytes.len(), UNIFORM_BYTE_LEN);
        assert_eq!(f32::from_ne_bytes(bytes[72..76].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_ne_bytes(bytes[80..84].try_into().unwrap()), 4.0);
        assert_eq!(f32::from_ne_bytes(bytes[84..88].try_into().unwrap()), 5.0);
        assert_eq!(f32::from_ne_bytes(bytes[88..92].try_into().unwrap()), 6.0);
        assert_eq!(
            f32::from_ne_bytes(bytes[96..100].try_into().unwrap()),
            5.0 / 255.0
        );
        assert_eq!(
            f32::from_ne_bytes(bytes[112..116].try_into().unwrap()),
            -8.0
        );
        assert_eq!(
            f32::from_ne_bytes(bytes[116..120].try_into().unwrap()),
            96.0
        );
    }

    #[test]
    fn actor_multiview_uniform_serializes_distinct_left_right_views() {
        let left = crate::chunk::ChunkCamera {
            eye: [1.0, 2.0, 3.0],
            target: [1.0, 2.0, 2.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 70.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 256.0,
        }
        .render_view(640, 480);
        let right = crate::chunk::ChunkCamera {
            eye: [4.0, 5.0, 6.0],
            target: [4.0, 5.0, 5.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 75.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 256.0,
        }
        .render_view(640, 480);
        let options = [
            TexturedSectionRenderOptions::default(),
            TexturedSectionRenderOptions::default().with_fog(crate::fog::RenderFog::underwater()),
        ];

        let bytes = multiview_uniform_bytes([left, right], options);

        assert_eq!(bytes.len(), MULTIVIEW_UNIFORM_BYTE_LEN);
        assert_eq!(
            &bytes[..UNIFORM_BYTE_LEN],
            uniform_bytes(left, options[0]).as_slice()
        );
        assert_eq!(
            &bytes[UNIFORM_BYTE_LEN..],
            uniform_bytes(right, options[1]).as_slice()
        );
        assert_ne!(&bytes[..UNIFORM_BYTE_LEN], &bytes[UNIFORM_BYTE_LEN..]);
    }

    #[test]
    fn cow_model_uses_vanilla_cuboid_parts() {
        assert_eq!(COW_MODEL_PARTS.len(), 6);
        assert_eq!(COW_HEAD_CUBOIDS.len(), 3);
        assert_eq!(COW_BODY_CUBOIDS.len(), 2);
        assert_eq!(COW_LEG_CUBOIDS.len(), 1);
        assert_eq!(COW_MODEL_PARTS[0].offset_pixels, [0.0, 4.0, -8.0]);
        assert_eq!(
            COW_MODEL_PARTS[1].rotation_radians,
            [std::f32::consts::FRAC_PI_2, 0.0, 0.0]
        );
        assert_eq!(COW_BODY_CUBOIDS[0].texture_offset, [18, 4]);
        assert_eq!(COW_BODY_CUBOIDS[1].texture_offset, [52, 0]);
    }

    #[test]
    fn actor_mesh_emits_cow_model_at_feet_position() {
        let mesh = actor_mesh(
            &[ActorInstance::cow_model(Vec3::ZERO, 0.0, 0.9, 1.4)],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &ActorFigureSet::default(),
        );

        assert_eq!(mesh.vertices.len(), 9 * 6 * 4);
        assert_eq!(mesh.indices.len(), 9 * 6 * 6);

        let bounds = mesh_bounds(&mesh);
        assert!((bounds.min.y - 0.0).abs() < 1.0e-6);
        assert!((bounds.max.y - 25.0 / 16.0).abs() < 1.0e-6);
        assert!(bounds.max.z > 0.85);
        assert!(bounds.min.z < -0.60);
    }

    #[test]
    fn cow_model_uses_cow_texture_region() {
        let mesh = actor_mesh(
            &[ActorInstance::cow_model(Vec3::ZERO, 0.0, 0.9, 1.4)],
            test_actor_texture_layout(),
            test_actor_texture_atlas_size(),
            &ActorFigureSet::default(),
        );
        let white_uv = [0.5 / 65.0, 0.5 / 32.0];

        assert!(mesh.vertices.iter().any(|vertex| vertex.uv != white_uv));
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| vertex.uv[0] >= 1.0 / 65.0)
        );
        assert!(mesh.vertices.iter().all(|vertex| vertex.uv[0] <= 1.0));
    }

    #[test]
    fn remote_player_yaw_uses_java_sign_convention() {
        let actor = ActorInstance::remote_player(Vec3::ZERO, -90.0);
        let forward = actor_world_position(actor, Vec3::new(0.0, 0.0, 1.0));

        assert!((forward.x - 1.0).abs() < 1.0e-6);
        assert!(forward.y.abs() < 1.0e-6);
        assert!(forward.z.abs() < 1.0e-6);
    }

    #[test]
    fn actor_world_position_places_geometry_at_feet_position() {
        let actor = ActorInstance::remote_player(Vec3::new(10.0, 64.0, -4.0), 0.0);
        let world = actor_world_position(actor, Vec3::new(0.0, 1.8, 0.0));

        assert_eq!(world, Vec3::new(10.0, 65.8, -4.0));
    }

    #[derive(Clone, Copy, Debug)]
    struct Bounds {
        min: Vec3,
        max: Vec3,
    }

    fn mesh_bounds(mesh: &ActorMesh) -> Bounds {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in &mesh.vertices {
            let position = Vec3::from_array(vertex.position);
            min = min.min(position);
            max = max.max(position);
        }
        Bounds { min, max }
    }
}
