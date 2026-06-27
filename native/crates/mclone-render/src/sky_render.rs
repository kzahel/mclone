//! Sky dome + sunrise/sunset glow render pass.
//!
//! Parity port of the flat-shaded portion of `LevelRenderer.renderSky`
//! (`reference/.../client/renderer/LevelRenderer.java:1699`): the sky disc
//! (`buildSkyDisc(+16)`, `:587`) tinted by the day/night sky color, and the
//! sunrise/sunset glow `TRIANGLE_FAN` (`:1717`). Drawn before the chunk pass with
//! depth writes off, into a rotation-only (camera-at-infinity) view-projection.
//!
//! Textured celestial bodies (sun/moon) and the star field are Phase 3 and live
//! elsewhere; this module only covers the flat geometry. Triangle fans are
//! expanded to indexed triangle lists since wgpu has no fan topology.

use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

use crate::color_profile::{
    RenderColorProfile, RenderConfig, RenderTargetColorTransform, color_transform_rgb,
    color_transform_wgpu,
};
use crate::sky::sunrise_color;

const SKY_UNIFORM_BYTE_SIZE: wgpu::BufferAddress = 64;
const SKY_VERTEX_FLOAT_COUNT: usize = 7; // position(3) + color(4)
const SKY_VERTEX_BYTE_SIZE: wgpu::BufferAddress =
    (SKY_VERTEX_FLOAT_COUNT * std::mem::size_of::<f32>()) as wgpu::BufferAddress;

// Sky disc: a center vertex plus a ring sampled every 45° from -180..=180.
const DISC_HEIGHT: f32 = 16.0;
const DISC_RADIUS: f32 = 512.0;
const DISC_RING_COUNT: usize = 9; // -180, -135, ..., 180
const DISC_VERTEX_COUNT: usize = DISC_RING_COUNT + 1;

// Glow fan: a center vertex plus 17 ring points (0..=16), per the reference loop.
const GLOW_RING_COUNT: usize = 17;
const GLOW_VERTEX_COUNT: usize = GLOW_RING_COUNT + 1;

type SkyVertex = [f32; SKY_VERTEX_FLOAT_COUNT];

/// Renders the flat sky geometry (disc + sunrise/sunset glow) into a color
/// attachment, clearing it first. Owns the shared shader/pipelines and the
/// per-frame vertex buffers.
pub struct SkyRenderer {
    disc_pipeline: wgpu::RenderPipeline,
    glow_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    disc_vertex_buffer: wgpu::Buffer,
    disc_index_buffer: wgpu::Buffer,
    disc_index_count: u32,
    glow_vertex_buffer: wgpu::Buffer,
    glow_index_buffer: wgpu::Buffer,
    glow_index_count: u32,
    color_transform: RenderTargetColorTransform,
}

impl SkyRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        Self::new_with_color_profile(device, color_format, RenderColorProfile::default())
    }

    pub fn new_with_color_profile(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        color_profile: RenderColorProfile,
    ) -> Self {
        Self::new_with_config(
            device,
            RenderConfig::for_color_target(color_profile, color_format),
        )
    }

    pub fn new_with_config(device: &wgpu::Device, render_config: RenderConfig) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_sky_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sky.wgsl").into()),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_sky_uniforms"),
            size: SKY_UNIFORM_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_sky_bind_group_layout"),
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
            label: Some("mclone_sky_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_sky_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: SKY_VERTEX_BYTE_SIZE,
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
        };

        let make_pipeline = |label: &str, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[vertex_layout.clone()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: render_config.color_format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                // The sky pass owns no depth attachment; it is drawn before the
                // chunk pass clears depth, with depth writes implicitly off.
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };

        let disc_pipeline = make_pipeline("mclone_sky_disc_pipeline", None);
        // Glow blends additively over the disc, matching vanilla's
        // SRC_ALPHA, ONE color factors (alpha left untouched).
        let glow_pipeline = make_pipeline(
            "mclone_sky_glow_pipeline",
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
        );

        let disc_indices = fan_indices(DISC_VERTEX_COUNT);
        let glow_indices = fan_indices(GLOW_VERTEX_COUNT);
        let disc_index_count = disc_indices.len() as u32;
        let glow_index_count = glow_indices.len() as u32;
        let disc_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_sky_disc_indices"),
            contents: &index_bytes(&disc_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let glow_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_sky_glow_indices"),
            contents: &index_bytes(&glow_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let disc_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_sky_disc_vertices"),
            size: SKY_VERTEX_BYTE_SIZE * DISC_VERTEX_COUNT as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let glow_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_sky_glow_vertices"),
            size: SKY_VERTEX_BYTE_SIZE * GLOW_VERTEX_COUNT as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            disc_pipeline,
            glow_pipeline,
            uniform_buffer,
            bind_group,
            disc_vertex_buffer,
            disc_index_buffer,
            disc_index_count,
            glow_vertex_buffer,
            glow_index_buffer,
            glow_index_count,
            color_transform: render_config.target_color_transform(),
        }
    }

    /// Clears the color attachment to `clear_color` and draws the sky disc (tinted
    /// by the same color) plus the sunrise/sunset glow when within the dawn/dusk
    /// band. `sky_view_projection` must be rotation-only (see
    /// [`crate::chunk::ChunkRenderView::sky_view_projection`]).
    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        sky_view_projection: Mat4,
        time_of_day: f32,
        sun_angle: f32,
    ) {
        let clear_color = color_transform_wgpu(clear_color, self.color_transform);
        let sky_color = [
            clear_color.r as f32,
            clear_color.g as f32,
            clear_color.b as f32,
        ];
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            &matrix_bytes(sky_view_projection.to_cols_array_2d()),
        );
        queue.write_buffer(
            &self.disc_vertex_buffer,
            0,
            &vertex_bytes(&disc_vertices(sky_color)),
        );
        let glow = sunrise_color(time_of_day);
        if let Some(color) = glow {
            queue.write_buffer(
                &self.glow_vertex_buffer,
                0,
                &vertex_bytes(&glow_vertices(
                    color_transform_rgba(color, self.color_transform),
                    sun_angle,
                )),
            );
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_sky_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_pipeline(&self.disc_pipeline);
        pass.set_vertex_buffer(0, self.disc_vertex_buffer.slice(..));
        pass.set_index_buffer(self.disc_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.disc_index_count, 0, 0..1);
        if glow.is_some() {
            pass.set_pipeline(&self.glow_pipeline);
            pass.set_vertex_buffer(0, self.glow_vertex_buffer.slice(..));
            pass.set_index_buffer(self.glow_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.glow_index_count, 0, 0..1);
        }
    }
}

fn color_transform_rgba(color: [f32; 4], transform: RenderTargetColorTransform) -> [f32; 4] {
    let [r, g, b] = color_transform_rgb([color[0], color[1], color[2]], transform);
    [r, g, b, color[3]]
}

/// Triangle-list indices for a fan with vertex 0 at the center and the remainder
/// forming the ring: `(0, i, i+1)` for each adjacent ring pair.
fn fan_indices(vertex_count: usize) -> Vec<u32> {
    let mut indices = Vec::new();
    for i in 1..vertex_count.saturating_sub(1) {
        indices.push(0);
        indices.push(i as u32);
        indices.push(i as u32 + 1);
    }
    indices
}

/// Port of `buildSkyDisc(16.0)`: a flat disc at y=+16, radius 512, tinted flat by
/// the day/night sky color.
fn disc_vertices(color: [f32; 3]) -> Vec<SkyVertex> {
    let [r, g, b] = color;
    let mut vertices = Vec::with_capacity(DISC_VERTEX_COUNT);
    vertices.push([0.0, DISC_HEIGHT, 0.0, r, g, b, 1.0]);
    let mut degrees = -180_i32;
    while degrees <= 180 {
        let radians = (degrees as f32).to_radians();
        vertices.push([
            DISC_RADIUS * radians.cos(),
            DISC_HEIGHT,
            DISC_RADIUS * radians.sin(),
            r,
            g,
            b,
            1.0,
        ]);
        degrees += 45;
    }
    vertices
}

/// Port of the `getSunriseColor` glow `TRIANGLE_FAN` (`LevelRenderer:1717`),
/// baking the celestial rig rotation into camera-relative vertex positions.
fn glow_vertices(color: [f32; 4], sun_angle: f32) -> Vec<SkyVertex> {
    let flip_degrees = if sun_angle.sin() < 0.0 { 180.0 } else { 0.0 };
    // Rig: XP(90) * ZP(flip) * ZP(90), applied right-to-left to each vertex.
    let rig = Quat::from_rotation_x(90.0_f32.to_radians())
        * Quat::from_rotation_z((flip_degrees as f32).to_radians())
        * Quat::from_rotation_z(90.0_f32.to_radians());
    let [r, g, b, alpha] = color;
    let mut vertices = Vec::with_capacity(GLOW_VERTEX_COUNT);
    let center = rig * Vec3::new(0.0, 100.0, 0.0);
    vertices.push([center.x, center.y, center.z, r, g, b, alpha]);
    for i in 0..GLOW_RING_COUNT {
        let angle = i as f32 * std::f32::consts::TAU / 16.0;
        let (sin_a, cos_a) = angle.sin_cos();
        let point = rig * Vec3::new(sin_a * 120.0, cos_a * 120.0, -cos_a * 40.0 * alpha);
        // Ring vertices fade to zero alpha so the glow blends out radially.
        vertices.push([point.x, point.y, point.z, r, g, b, 0.0]);
    }
    vertices
}

fn vertex_bytes(vertices: &[SkyVertex]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vertices.len() * SKY_VERTEX_BYTE_SIZE as usize);
    for vertex in vertices {
        for value in vertex {
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

    #[test]
    fn disc_has_expected_vertex_and_index_counts() {
        let vertices = disc_vertices([0.1, 0.2, 0.3]);
        assert_eq!(vertices.len(), DISC_VERTEX_COUNT);
        // Center is at y = +16, ring radius reaches 512.
        assert_eq!(vertices[0][1], DISC_HEIGHT);
        assert!((vertices[1][0].hypot(vertices[1][2]) - DISC_RADIUS).abs() < 1e-2);
        assert_eq!(
            fan_indices(DISC_VERTEX_COUNT).len(),
            (DISC_RING_COUNT - 1) * 3
        );
    }

    #[test]
    fn glow_center_sits_on_the_horizon_and_flips_with_sun_angle() {
        let color = [0.85, 0.4, 0.2, 0.9];
        // sun_angle with positive sine -> no flip; center hugs the horizon (y≈0).
        let dawn = glow_vertices(color, 1.0);
        assert_eq!(dawn.len(), GLOW_VERTEX_COUNT);
        assert!(dawn[0][1].abs() < 1e-3, "center y = {}", dawn[0][1]);
        // Flipping the sun below the horizon mirrors the glow to the opposite side.
        let dusk = glow_vertices(color, -1.0);
        assert!(
            (dawn[0][0] + dusk[0][0]).abs() < 1e-3,
            "glow should mirror across origin"
        );
        // Center keeps full alpha; ring fades to zero.
        assert_eq!(dawn[0][6], color[3]);
        assert_eq!(dawn[1][6], 0.0);
    }
}
