use anyhow::{Context, Result};
use glam::Vec3;
use wgpu::util::DeviceExt;

use crate::chunk::{ChunkRenderView, DEPTH_FORMAT};
use crate::target::RenderFrameTarget;

const ACTOR_VERTEX_FLOAT_COUNT: usize = 7;
const ACTOR_VERTEX_BYTE_LEN: usize = ACTOR_VERTEX_FLOAT_COUNT * std::mem::size_of::<f32>();
const ACTOR_VERTEX_BYTE_SIZE: wgpu::BufferAddress = ACTOR_VERTEX_BYTE_LEN as wgpu::BufferAddress;
const UNIFORM_BYTE_LEN: usize = 16 * std::mem::size_of::<f32>();
const UNIFORM_BYTE_SIZE: wgpu::BufferAddress = UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const MODEL_PIXEL_SCALE: f32 = 1.0 / 16.0;
const MODEL_FEET_Y_PIXELS: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorInstance {
    pub feet_position: Vec3,
    /// Native world yaw in radians. Local actor +Z is the forward/front side.
    pub yaw_radians: f32,
    pub shape: ActorInstanceShape,
    pub width: f32,
    pub height: f32,
    pub body_color: [f32; 4],
    pub accent_color: [f32; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorInstanceShape {
    Humanoid,
    QuadrupedPlaceholder,
    CowModel,
}

impl ActorInstance {
    pub fn remote_player(feet_position: Vec3, y_rot_degrees: f32) -> Self {
        Self {
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            shape: ActorInstanceShape::Humanoid,
            width: 0.6,
            height: 1.8,
            body_color: [0.10, 0.58, 0.68, 1.0],
            accent_color: [0.95, 0.80, 0.24, 1.0],
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
            shape: ActorInstanceShape::QuadrupedPlaceholder,
            width,
            height,
            body_color: [0.33, 0.19, 0.10, 1.0],
            accent_color: [0.92, 0.86, 0.74, 1.0],
        }
    }

    pub fn cow_model(feet_position: Vec3, y_rot_degrees: f32, width: f32, height: f32) -> Self {
        Self {
            feet_position,
            yaw_radians: -y_rot_degrees.to_radians(),
            shape: ActorInstanceShape::CowModel,
            width,
            height,
            body_color: [0.28, 0.17, 0.10, 1.0],
            accent_color: [0.90, 0.86, 0.72, 1.0],
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
            shape: ActorInstanceShape::QuadrupedPlaceholder,
            width,
            height,
            body_color: [0.92, 0.90, 0.82, 1.0],
            accent_color: [0.92, 0.18, 0.12, 1.0],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActorRenderStats {
    pub submitted_actor_count: usize,
    pub drawn_actor_count: usize,
    pub vertex_count: u32,
    pub index_count: u32,
}

pub struct ActorDrawResources {
    renderer: ActorRenderer,
}

impl ActorDrawResources {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        Self {
            renderer: ActorRenderer::new(device, color_format),
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        render_view: ChunkRenderView,
        actors: &[ActorInstance],
    ) -> Result<ActorRenderStats> {
        if actors.is_empty() {
            return Ok(ActorRenderStats::default());
        }
        let depth_view = target
            .depth_view
            .context("actor render pass requires a depth attachment")?;
        let mesh = actor_mesh(actors);
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return Ok(ActorRenderStats {
                submitted_actor_count: actors.len(),
                ..ActorRenderStats::default()
            });
        }

        queue.write_buffer(
            &self.renderer.uniform_buffer,
            0,
            &uniform_bytes(render_view.uniform_matrix()),
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
        pass.set_bind_group(0, &self.renderer.bind_group, &[]);
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
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl ActorRenderer {
    fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_actor_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/entity_actor.wgsl").into()),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_actor_uniforms"),
            size: UNIFORM_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_actor_bind_group_layout"),
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
            label: Some("mclone_actor_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_actor_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_actor_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
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

#[derive(Clone, Debug, Default, PartialEq)]
struct ActorMesh {
    vertices: Vec<ActorVertex>,
    indices: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActorVertex {
    position: [f32; 3],
    color: [f32; 4],
}

fn actor_mesh(actors: &[ActorInstance]) -> ActorMesh {
    let mut mesh = ActorMesh::default();
    for actor in actors {
        append_actor(&mut mesh, *actor);
    }
    mesh
}

fn append_actor(mesh: &mut ActorMesh, actor: ActorInstance) {
    match actor.shape {
        ActorInstanceShape::Humanoid => append_humanoid_placeholder(mesh, actor),
        ActorInstanceShape::QuadrupedPlaceholder => append_quadruped_placeholder(mesh, actor),
        ActorInstanceShape::CowModel => append_cow_model(mesh, actor),
    }
}

fn append_humanoid_placeholder(mesh: &mut ActorMesh, actor: ActorInstance) {
    let dark = scale_color(actor.body_color, 0.58);
    let side = scale_color(actor.body_color, 0.78);
    let light = scale_color(actor.body_color, 1.12);
    append_box(
        mesh,
        actor,
        Vec3::new(-0.22, 0.0, -0.13),
        Vec3::new(-0.04, 0.76, 0.13),
        [dark, side, side, side, side, actor.body_color],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(0.04, 0.0, -0.13),
        Vec3::new(0.22, 0.76, 0.13),
        [dark, side, side, side, side, actor.body_color],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.30, 0.72, -0.16),
        Vec3::new(0.30, 1.36, 0.16),
        [dark, side, side, side, side, light],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.24, 1.34, -0.24),
        Vec3::new(0.24, 1.80, 0.24),
        [
            scale_color(actor.accent_color, 0.70),
            scale_color(actor.accent_color, 0.86),
            scale_color(actor.accent_color, 0.86),
            scale_color(actor.accent_color, 0.92),
            scale_color(actor.accent_color, 0.92),
            actor.accent_color,
        ],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-0.11, 1.05, 0.15),
        Vec3::new(0.11, 1.22, 0.31),
        [
            actor.accent_color,
            actor.accent_color,
            actor.accent_color,
            actor.accent_color,
            actor.accent_color,
            [1.0, 0.98, 0.66, 1.0],
        ],
    );
}

fn append_quadruped_placeholder(mesh: &mut ActorMesh, actor: ActorInstance) {
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
        [dark, side, side, side, side, light],
    );
    append_box(
        mesh,
        actor,
        Vec3::new(-head_half, head_bottom, body_front),
        Vec3::new(head_half, head_top, head_front),
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
                [dark, side, side, side, side, actor.body_color],
            );
        }
    }
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

fn append_cow_model(mesh: &mut ActorMesh, actor: ActorInstance) {
    for part in COW_MODEL_PARTS {
        for cuboid in part.cuboids {
            append_model_cuboid(mesh, actor, part, cuboid, cow_cuboid_face_colors(*cuboid));
        }
    }
}

fn append_model_cuboid(
    mesh: &mut ActorMesh,
    actor: ActorInstance,
    part: &ActorModelPart,
    cuboid: &ActorModelCuboid,
    face_colors: [[f32; 4]; 6],
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
    append_transformed_box(mesh, face_colors, |corner_index| {
        let model_position = transform_model_part_point(corners[corner_index], part);
        actor_world_position(actor, model_pixels_to_actor_local(model_position))
    });
}

fn append_transformed_box(
    mesh: &mut ActorMesh,
    face_colors: [[f32; 4]; 6],
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
        for corner_index in face {
            mesh.vertices.push(ActorVertex {
                position: world_corner(corner_index).to_array(),
                color: face_colors[face_index],
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
        [0, 0] => shaded_faces([0.74, 0.68, 0.54, 1.0]),
        [18, 4] => [
            [0.20, 0.12, 0.07, 1.0],
            [0.70, 0.66, 0.54, 1.0],
            [0.26, 0.15, 0.08, 1.0],
            [0.78, 0.74, 0.62, 1.0],
            [0.18, 0.10, 0.06, 1.0],
            [0.48, 0.35, 0.23, 1.0],
        ],
        [22, 0] => shaded_faces([0.86, 0.82, 0.62, 1.0]),
        [52, 0] => shaded_faces([0.88, 0.56, 0.58, 1.0]),
        [0, 16] => [
            [0.16, 0.09, 0.05, 1.0],
            [0.28, 0.17, 0.10, 1.0],
            [0.21, 0.12, 0.07, 1.0],
            [0.74, 0.70, 0.60, 1.0],
            [0.16, 0.09, 0.05, 1.0],
            [0.36, 0.22, 0.13, 1.0],
        ],
        _ => shaded_faces([0.35, 0.25, 0.18, 1.0]),
    }
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
    append_transformed_box(mesh, face_colors, |corner_index| {
        actor_world_position(actor, corners[corner_index])
    });
}

fn actor_world_position(actor: ActorInstance, local: Vec3) -> Vec3 {
    let (sin, cos) = actor.yaw_radians.sin_cos();
    actor.feet_position
        + Vec3::new(
            local.x * cos + local.z * sin,
            local.y,
            -local.x * sin + local.z * cos,
        )
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

fn uniform_bytes(view_projection: [[f32; 4]; 4]) -> [u8; UNIFORM_BYTE_LEN] {
    let mut bytes = [0_u8; UNIFORM_BYTE_LEN];
    let mut offset = 0;
    for value in view_projection.into_iter().flatten() {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        offset += 4;
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_mesh_emits_one_placeholder_per_actor() {
        let mesh = actor_mesh(&[ActorInstance::remote_player(Vec3::new(1.0, 2.0, 3.0), 0.0)]);

        assert_eq!(mesh.vertices.len(), 5 * 6 * 4);
        assert_eq!(mesh.indices.len(), 5 * 6 * 6);
    }

    #[test]
    fn actor_mesh_emits_quadruped_placeholder() {
        let mesh = actor_mesh(&[ActorInstance::cow_placeholder(
            Vec3::new(1.0, 2.0, 3.0),
            0.0,
            0.9,
            1.4,
        )]);

        assert_eq!(mesh.vertices.len(), 6 * 6 * 4);
        assert_eq!(mesh.indices.len(), 6 * 6 * 6);
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
        let mesh = actor_mesh(&[ActorInstance::cow_model(Vec3::ZERO, 0.0, 0.9, 1.4)]);

        assert_eq!(mesh.vertices.len(), 9 * 6 * 4);
        assert_eq!(mesh.indices.len(), 9 * 6 * 6);

        let bounds = mesh_bounds(&mesh);
        assert!((bounds.min.y - 0.0).abs() < 1.0e-6);
        assert!((bounds.max.y - 25.0 / 16.0).abs() < 1.0e-6);
        assert!(bounds.max.z > 0.85);
        assert!(bounds.min.z < -0.60);
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
