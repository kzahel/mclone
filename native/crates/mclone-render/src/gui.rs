use anyhow::Result;
use mclone_ui::{ClipRect, Color, GuiDrawCommand, GuiDrawList, Rect};
use wgpu::util::DeviceExt;

use crate::target::RenderFrameTarget;

const FLOATS_PER_VERTEX: usize = 6;
const VERTEX_SIZE: wgpu::BufferAddress =
    (FLOATS_PER_VERTEX * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const QUAD_VERTEX_COUNT: u32 = 6;

const GUI_SOLID_WGSL: &str = r#"
struct VertexOut {
  @builtin(position) position: vec4f,
  @location(0) color: vec4f,
};

@vertex
fn vertex_main(
  @location(0) position: vec2f,
  @location(1) color: vec4f,
) -> VertexOut {
  var out: VertexOut;
  out.position = vec4f(position, 0.0, 1.0);
  out.color = color;
  return out;
}

@fragment
fn fragment_main(input: VertexOut) -> @location(0) vec4f {
  return input.color;
}
"#;

#[derive(Clone, Copy, Debug)]
pub struct GuiRenderOptions {
    pub clear_color: Option<wgpu::Color>,
}

impl GuiRenderOptions {
    pub fn overlay() -> Self {
        Self { clear_color: None }
    }

    pub fn clear(color: wgpu::Color) -> Self {
        Self {
            clear_color: Some(color),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PreparedDraw {
    first_vertex: u32,
    vertex_count: u32,
    clip: Option<ClipRect>,
}

pub struct GuiRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_buffer_size: wgpu::BufferAddress,
}

impl GuiRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_gui_solid_shader"),
            source: wgpu::ShaderSource::Wgsl(GUI_SOLID_WGSL.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_gui_solid_pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vertex_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: VERTEX_SIZE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            shader_location: 0,
                            offset: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            shader_location: 1,
                            offset: 8,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        Self {
            pipeline,
            vertex_buffer: None,
            vertex_buffer_size: 0,
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        gui_size: [f32; 2],
        gui: &GuiDrawList,
        options: GuiRenderOptions,
    ) -> Result<()> {
        let prepared = prepare_draws(gui.commands(), gui_size);
        let load = match options.clear_color {
            Some(color) => wgpu::LoadOp::Clear(color),
            None => wgpu::LoadOp::Load,
        };
        if prepared.draws.is_empty() {
            if options.clear_color.is_some() {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mclone_gui_clear_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target.color_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
            }
            return Ok(());
        }

        self.upload_vertices(device, queue, &prepared.vertices);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_gui_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        let vertex_buffer = self
            .vertex_buffer
            .as_ref()
            .expect("GUI vertex buffer exists");
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        for draw in prepared.draws {
            apply_scissor(&mut pass, target.size, gui_size, draw.clip);
            pass.draw(
                draw.first_vertex..draw.first_vertex + draw.vertex_count,
                0..1,
            );
        }
        Ok(())
    }

    fn upload_vertices(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, vertices: &[f32]) {
        let bytes = f32_bytes_vec(vertices);
        let required_size = bytes.len().max(4) as wgpu::BufferAddress;
        let needs_recreate = self
            .vertex_buffer
            .as_ref()
            .is_none_or(|_| self.vertex_buffer_size < required_size);
        if needs_recreate {
            self.vertex_buffer = Some(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("mclone_gui_vertices"),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                },
            ));
            self.vertex_buffer_size = required_size;
        } else if let Some(buffer) = &self.vertex_buffer {
            queue.write_buffer(buffer, 0, &bytes);
        }
    }
}

struct PreparedDraws {
    vertices: Vec<f32>,
    draws: Vec<PreparedDraw>,
}

fn prepare_draws(commands: &[GuiDrawCommand], gui_size: [f32; 2]) -> PreparedDraws {
    let mut vertices = Vec::new();
    let mut draws = Vec::new();
    for command in commands {
        match *command {
            GuiDrawCommand::SolidRect { rect, color, clip } => {
                let first_vertex = (vertices.len() / FLOATS_PER_VERTEX) as u32;
                push_quad(&mut vertices, gui_size, rect, color, color);
                draws.push(PreparedDraw {
                    first_vertex,
                    vertex_count: QUAD_VERTEX_COUNT,
                    clip,
                });
            }
            GuiDrawCommand::GradientRect {
                rect,
                top,
                bottom,
                clip,
            } => {
                let first_vertex = (vertices.len() / FLOATS_PER_VERTEX) as u32;
                push_quad(&mut vertices, gui_size, rect, top, bottom);
                draws.push(PreparedDraw {
                    first_vertex,
                    vertex_count: QUAD_VERTEX_COUNT,
                    clip,
                });
            }
        }
    }
    PreparedDraws { vertices, draws }
}

fn push_quad(vertices: &mut Vec<f32>, gui_size: [f32; 2], rect: Rect, top: Color, bottom: Color) {
    let left = clip_x(rect.x, gui_size[0]);
    let right = clip_x(rect.right(), gui_size[0]);
    let top_y = clip_y(rect.y, gui_size[1]);
    let bottom_y = clip_y(rect.bottom(), gui_size[1]);
    let top = top.to_linear_f32();
    let bottom = bottom.to_linear_f32();
    push_vertex(vertices, left, top_y, top);
    push_vertex(vertices, right, top_y, top);
    push_vertex(vertices, right, bottom_y, bottom);
    push_vertex(vertices, left, top_y, top);
    push_vertex(vertices, right, bottom_y, bottom);
    push_vertex(vertices, left, bottom_y, bottom);
}

fn push_vertex(vertices: &mut Vec<f32>, x: f32, y: f32, color: [f32; 4]) {
    vertices.extend_from_slice(&[x, y, color[0], color[1], color[2], color[3]]);
}

fn clip_x(x: f32, width: f32) -> f32 {
    (x / width.max(1.0)) * 2.0 - 1.0
}

fn clip_y(y: f32, height: f32) -> f32 {
    1.0 - (y / height.max(1.0)) * 2.0
}

fn apply_scissor(
    pass: &mut wgpu::RenderPass<'_>,
    target_size: [u32; 2],
    gui_size: [f32; 2],
    clip: Option<ClipRect>,
) {
    let Some(clip) = clip else {
        pass.set_scissor_rect(0, 0, target_size[0].max(1), target_size[1].max(1));
        return;
    };
    let scale_x = target_size[0].max(1) as f32 / gui_size[0].max(1.0);
    let scale_y = target_size[1].max(1) as f32 / gui_size[1].max(1.0);
    let x = (clip.x.max(0.0) * scale_x).floor() as u32;
    let y = (clip.y.max(0.0) * scale_y).floor() as u32;
    let right = ((clip.x + clip.width) * scale_x)
        .ceil()
        .clamp(0.0, target_size[0] as f32) as u32;
    let bottom = ((clip.y + clip.height) * scale_y)
        .ceil()
        .clamp(0.0, target_size[1] as f32) as u32;
    pass.set_scissor_rect(x, y, right.saturating_sub(x), bottom.saturating_sub(y));
}

fn f32_bytes_vec(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(values));
    for value in values {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_one_quad_for_one_rect() {
        let mut draw = GuiDrawList::new();
        draw.fill(
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Color::rgba(255, 255, 255, 255),
        );
        let prepared = prepare_draws(draw.commands(), [100.0, 100.0]);
        assert_eq!(prepared.draws.len(), 1);
        assert_eq!(prepared.vertices.len(), 6 * FLOATS_PER_VERTEX);
    }
}
