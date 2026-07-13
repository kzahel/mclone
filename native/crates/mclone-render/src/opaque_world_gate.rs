use std::cell::RefCell;
use std::num::{NonZeroU32, NonZeroU64};

use anyhow::{Result, bail};
use glam::{Mat4, Vec3};

use crate::{
    chunk::{ChunkDepthTarget, ChunkMultiviewDepthTarget, ChunkRenderView, DEPTH_FORMAT},
    target::RenderFrameTarget,
    uniform::{PER_VIEW_UNIFORM_SLOT_COUNT, PerViewSlot, PerViewUniformBuffer, SINGLE_VIEW_SLOT},
};

const GATE_WGSL: &str = r#"
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
    return vec4<f32>(input.color.rgb, 1.0);
}
"#;

const GATE_MULTIVIEW_WGSL: &str = r#"
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
    out.position = stereo_uniforms.views[u32(view_index)].view_projection * vec4<f32>(input.position, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color.rgb, 1.0);
}
"#;

const GATE_VERTEX_FLOATS: usize = 7;
const GATE_VERTEX_COUNT: usize = 6;
const GATE_VERTEX_SIZE: wgpu::BufferAddress =
    (GATE_VERTEX_FLOATS * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const GATE_VERTEX_SLOT_SIZE: wgpu::BufferAddress = GATE_VERTEX_SIZE * GATE_VERTEX_COUNT as u64;

/// One opaque, two-sided world-space gate surface.
///
/// The renderer deliberately knows nothing about world selection or crossing.
/// Scene ownership supplies the active endpoint and readiness color each frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpaqueWorldGate {
    pub center: Vec3,
    pub normal: Vec3,
    pub width: f32,
    pub height: f32,
    pub color: [f32; 4],
}

impl OpaqueWorldGate {
    pub fn new(center: Vec3, normal: Vec3, width: f32, height: f32, color: [f32; 4]) -> Self {
        Self {
            center,
            normal,
            width,
            height,
            color,
        }
    }

    fn vertices(self) -> Option<[f32; GATE_VERTEX_COUNT * GATE_VERTEX_FLOATS]> {
        if !self.center.is_finite()
            || !self.normal.is_finite()
            || !self.width.is_finite()
            || !self.height.is_finite()
            || self.width <= 0.0
            || self.height <= 0.0
            || self.color.iter().any(|component| !component.is_finite())
        {
            return None;
        }
        let normal = self.normal.try_normalize()?;
        let tangent = Vec3::Y.cross(normal).try_normalize()?;
        let half_right = tangent * (self.width * 0.5);
        let half_up = Vec3::Y * (self.height * 0.5);
        let corners = [
            self.center - half_right - half_up,
            self.center + half_right - half_up,
            self.center + half_right + half_up,
            self.center - half_right + half_up,
        ];
        let mut vertices = [0.0; GATE_VERTEX_COUNT * GATE_VERTEX_FLOATS];
        for (vertex, corner) in [
            corners[0], corners[1], corners[2], corners[0], corners[2], corners[3],
        ]
        .into_iter()
        .enumerate()
        {
            let offset = vertex * GATE_VERTEX_FLOATS;
            vertices[offset..offset + 3].copy_from_slice(&corner.to_array());
            vertices[offset + 3..offset + 7].copy_from_slice(&self.color);
        }
        Some(vertices)
    }
}

/// Opaque gate renderer for mono/per-eye and full-frame multiview paths.
///
/// It uses the terrain depth format and reversed-Z comparison, writes depth,
/// disables blending, and disables culling so either side is a closed surface.
pub struct OpaqueWorldGateRenderer {
    pipeline: wgpu::RenderPipeline,
    uniforms: PerViewUniformBuffer,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    color_format: wgpu::TextureFormat,
    multiview: RefCell<Option<OpaqueWorldGateMultiviewRenderer>>,
}

impl OpaqueWorldGateRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_opaque_world_gate_shader"),
            source: wgpu::ShaderSource::Wgsl(GATE_WGSL.into()),
        });
        let uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_opaque_world_gate_uniforms",
            64,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_opaque_world_gate_bind_group_layout"),
            entries: &[uniforms.layout_entry(0, wgpu::ShaderStages::VERTEX)],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_opaque_world_gate_bind_group"),
            layout: &bind_group_layout,
            entries: &[uniforms.bind_group_entry(0)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_opaque_world_gate_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = create_gate_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_opaque_world_gate_pipeline",
            None,
        );
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_opaque_world_gate_vertices"),
            size: GATE_VERTEX_SLOT_SIZE * PER_VIEW_UNIFORM_SLOT_COUNT as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            uniforms,
            bind_group,
            vertex_buffer,
            color_format,
            multiview: RefCell::new(None),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_in_slot(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        depth: &ChunkDepthTarget,
        render_view: ChunkRenderView,
        gate: Option<OpaqueWorldGate>,
        view_slot: PerViewSlot,
    ) {
        let Some(vertices) = gate.and_then(OpaqueWorldGate::vertices) else {
            return;
        };
        let vertex_range = view_slot.byte_range(GATE_VERTEX_SLOT_SIZE);
        queue.write_buffer(
            &self.vertex_buffer,
            vertex_range.start,
            &f32_bytes(&vertices),
        );
        let uniform_offset =
            self.uniforms
                .write_slot(queue, view_slot, &matrix_bytes(render_view.view_projection));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_opaque_world_gate_pass"),
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
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(vertex_range));
        pass.draw(0..GATE_VERTEX_COUNT as u32, 0..1);
    }

    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        depth: &ChunkDepthTarget,
        render_view: ChunkRenderView,
        gate: Option<OpaqueWorldGate>,
    ) {
        self.render_in_slot(
            queue,
            encoder,
            target,
            depth,
            render_view,
            gate,
            SINGLE_VIEW_SLOT,
        );
    }

    pub fn render_multiview(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        depth: &ChunkMultiviewDepthTarget,
        render_views: [ChunkRenderView; 2],
        gate: Option<OpaqueWorldGate>,
    ) -> Result<()> {
        let Some(vertices) = gate.and_then(OpaqueWorldGate::vertices) else {
            return Ok(());
        };
        queue.write_buffer(&self.vertex_buffer, 0, &f32_bytes(&vertices));
        let renderer = self.multiview_renderer(device)?;
        renderer.write_uniforms(queue, render_views);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_opaque_world_gate_multiview_pass"),
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
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(0..GATE_VERTEX_SLOT_SIZE));
        pass.draw(0..GATE_VERTEX_COUNT as u32, 0..1);
        Ok(())
    }

    pub fn materialize_multiview_renderer(&self, device: &wgpu::Device) -> Result<bool> {
        let was_materialized = self.multiview.borrow().is_some();
        drop(self.multiview_renderer(device)?);
        Ok(!was_materialized)
    }

    pub fn multiview_renderer_materialized(&self) -> bool {
        self.multiview.borrow().is_some()
    }

    fn multiview_renderer(
        &self,
        device: &wgpu::Device,
    ) -> Result<std::cell::Ref<'_, OpaqueWorldGateMultiviewRenderer>> {
        if !device.features().contains(wgpu::Features::MULTIVIEW) {
            bail!("opaque world gate multiview render requires wgpu MULTIVIEW");
        }
        if self.multiview.borrow().is_none() {
            *self.multiview.borrow_mut() = Some(OpaqueWorldGateMultiviewRenderer::new(
                device,
                self.color_format,
            ));
        }
        Ok(std::cell::Ref::map(self.multiview.borrow(), |renderer| {
            renderer
                .as_ref()
                .expect("opaque world gate multiview renderer initialized above")
        }))
    }
}

struct OpaqueWorldGateMultiviewRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl OpaqueWorldGateMultiviewRenderer {
    fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_opaque_world_gate_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(GATE_MULTIVIEW_WGSL.into()),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_opaque_world_gate_multiview_uniforms"),
            size: 128,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_opaque_world_gate_multiview_bind_group_layout"),
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
            label: Some("mclone_opaque_world_gate_multiview_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_opaque_world_gate_multiview_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = create_gate_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_opaque_world_gate_multiview_pipeline",
            NonZeroU32::new(2),
        );
        Self {
            pipeline,
            uniform_buffer,
            bind_group,
        }
    }

    fn write_uniforms(&self, queue: &wgpu::Queue, views: [ChunkRenderView; 2]) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            &multiview_matrix_bytes([views[0].view_projection, views[1].view_projection]),
        );
    }
}

fn create_gate_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    label: &'static str,
    multiview: Option<NonZeroU32>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vertex_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: GATE_VERTEX_SIZE,
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
            depth_compare: wgpu::CompareFunction::GreaterEqual,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview,
        cache: None,
    })
}

fn f32_bytes(values: &[f32]) -> Vec<u8> {
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
    fn gate_vertices_span_the_requested_two_sided_plane() {
        let gate = OpaqueWorldGate::new(
            Vec3::new(0.5, 66.0, 6.5),
            Vec3::NEG_Z,
            3.0,
            4.0,
            [0.2, 0.4, 0.8, 0.3],
        );

        let vertices = gate.vertices().expect("valid gate emits vertices");

        assert_eq!(vertices.len(), GATE_VERTEX_COUNT * GATE_VERTEX_FLOATS);
        let positions = vertices
            .chunks_exact(GATE_VERTEX_FLOATS)
            .map(|vertex| Vec3::new(vertex[0], vertex[1], vertex[2]))
            .collect::<Vec<_>>();
        assert!(positions.iter().all(|position| position.z == 6.5));
        assert_eq!(
            positions
                .iter()
                .map(|position| position.x)
                .fold(f32::INFINITY, f32::min),
            -1.0
        );
        assert_eq!(
            positions
                .iter()
                .map(|position| position.x)
                .fold(f32::NEG_INFINITY, f32::max),
            2.0
        );
        assert_eq!(
            positions
                .iter()
                .map(|position| position.y)
                .fold(f32::INFINITY, f32::min),
            64.0
        );
        assert_eq!(
            positions
                .iter()
                .map(|position| position.y)
                .fold(f32::NEG_INFINITY, f32::max),
            68.0
        );
        assert!(
            vertices
                .chunks_exact(GATE_VERTEX_FLOATS)
                .all(|vertex| { vertex[3..7] == [0.2, 0.4, 0.8, 0.3] })
        );
    }

    #[test]
    fn gate_rejects_degenerate_or_non_finite_geometry() {
        let valid = OpaqueWorldGate::new(Vec3::ZERO, Vec3::Z, 3.0, 4.0, [1.0; 4]);
        assert!(valid.vertices().is_some());
        assert!(
            OpaqueWorldGate::new(Vec3::ZERO, Vec3::ZERO, 3.0, 4.0, [1.0; 4])
                .vertices()
                .is_none()
        );
        assert!(
            OpaqueWorldGate::new(Vec3::ZERO, Vec3::Z, 0.0, 4.0, [1.0; 4])
                .vertices()
                .is_none()
        );
    }

    #[test]
    fn multiview_uniforms_keep_distinct_eye_matrices() {
        let left = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let right = Mat4::from_translation(Vec3::new(4.0, 5.0, 6.0));

        let bytes = multiview_matrix_bytes([left, right]);

        assert_eq!(&bytes[..64], matrix_bytes(left).as_slice());
        assert_eq!(&bytes[64..], matrix_bytes(right).as_slice());
        assert_ne!(&bytes[..64], &bytes[64..]);
    }
}
