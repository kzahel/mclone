use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use anyhow::{Context, Result};
use mclone_mesh::{TexturedRenderSectionMesh, TexturedVisibleChunkMesh, VisibleChunkMesh};

use crate::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkDrawResources, ChunkRenderTarget, ChunkTextureAtlas,
    TexturedChunkDrawResources, TexturedSectionDrawResources, TexturedSectionRenderOptions,
};
use crate::gpu_util::{native_backends, optional_gpu_features};
use crate::target::{RenderFrameContext, RenderFrameTarget};

const BYTES_PER_PIXEL: u32 = 4;
const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;
const HEADLESS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

#[derive(Clone, Debug)]
pub struct HeadlessClearOptions {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub color: wgpu::Color,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadlessClearReport {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub byte_len: usize,
}

#[derive(Clone, Debug)]
pub struct HeadlessChunkOptions {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub color: wgpu::Color,
    pub camera: ChunkCamera,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadlessChunkReport {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub byte_len: usize,
    pub vertex_count: u32,
    pub index_count: u32,
}

#[derive(Clone, Debug)]
pub struct HeadlessTimedemoOptions {
    pub width: u32,
    pub height: u32,
    pub color: wgpu::Color,
    pub cameras: Vec<ChunkCamera>,
    pub render_options: TexturedSectionRenderOptions,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeadlessTimedemoReport {
    pub width: u32,
    pub height: u32,
    pub frame_count: usize,
    pub setup_ms: f64,
    pub total_frame_ms: f64,
    pub average_frame_ms: f64,
    pub min_frame_ms: f64,
    pub max_frame_ms: f64,
    pub loaded_section_count: usize,
    pub average_drawn_section_count: f64,
    pub max_drawn_section_count: usize,
    pub average_frustum_section_count: f64,
    pub max_frustum_section_count: usize,
    pub graph_cull_enabled_frame_count: usize,
    pub average_graph_culled_section_count: f64,
    pub max_graph_culled_section_count: usize,
    pub loaded_index_count: u32,
    pub average_drawn_index_count: f64,
    pub max_drawn_index_count: u32,
    pub average_frustum_index_count: f64,
    pub max_frustum_index_count: u32,
    pub average_graph_culled_index_count: f64,
    pub max_graph_culled_index_count: u32,
}

pub fn write_headless_clear_png(options: HeadlessClearOptions) -> Result<HeadlessClearReport> {
    let width = options.width.max(1);
    let height = options.height.max(1);
    let (device, queue) = create_headless_device()?;
    let target = OffscreenTarget::new(&device, width, height, HEADLESS_FORMAT);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_headless_clear_encoder"),
    });
    {
        let frame = RenderFrameContext::new(&device, &queue, &mut encoder, target.render_target());
        let _pass = frame
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mclone_headless_clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: frame.target.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(options.color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
    }
    queue.submit(std::iter::once(encoder.finish()));

    let pixels = read_rgba8(&device, &queue, &target.texture, width, height)?;
    save_rgba_png(&options.path, width, height, &pixels)?;

    Ok(HeadlessClearReport {
        path: options.path,
        width,
        height,
        byte_len: pixels.len(),
    })
}

pub fn write_headless_chunk_png(
    options: HeadlessChunkOptions,
    mesh: &VisibleChunkMesh,
) -> Result<HeadlessChunkReport> {
    let width = options.width.max(1);
    let height = options.height.max(1);
    let (device, queue) = create_headless_device()?;
    let target = OffscreenTarget::new(&device, width, height, HEADLESS_FORMAT);
    let depth = ChunkDepthTarget::new(&device, width, height);
    let draw = ChunkDrawResources::new(&device, HEADLESS_FORMAT, mesh)?;

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_headless_chunk_encoder"),
    });
    {
        let frame = RenderFrameContext::new(&device, &queue, &mut encoder, target.render_target());
        let render_view = options
            .camera
            .render_view(frame.target.size[0], frame.target.size[1]);
        let render_target = ChunkRenderTarget::from_frame_target(
            frame.target.with_depth(&depth.view),
            options.color,
        )?;
        draw.render(frame.queue, frame.encoder, render_target, render_view)?;
    }
    queue.submit(std::iter::once(encoder.finish()));

    let pixels = read_rgba8(&device, &queue, &target.texture, width, height)?;
    save_rgba_png(&options.path, width, height, &pixels)?;
    let stats = mesh.stats();

    Ok(HeadlessChunkReport {
        path: options.path,
        width,
        height,
        byte_len: pixels.len(),
        vertex_count: stats.vertex_count,
        index_count: stats.index_count,
    })
}

pub fn write_headless_textured_chunk_png(
    options: HeadlessChunkOptions,
    mesh: &TexturedVisibleChunkMesh,
    atlas: ChunkTextureAtlas<'_>,
) -> Result<HeadlessChunkReport> {
    let width = options.width.max(1);
    let height = options.height.max(1);
    let (device, queue) = create_headless_device()?;
    let target = OffscreenTarget::new(&device, width, height, HEADLESS_FORMAT);
    let depth = ChunkDepthTarget::new(&device, width, height);
    let draw = TexturedChunkDrawResources::new(&device, &queue, HEADLESS_FORMAT, mesh, atlas)?;

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_headless_textured_chunk_encoder"),
    });
    {
        let frame = RenderFrameContext::new(&device, &queue, &mut encoder, target.render_target());
        let render_view = options
            .camera
            .render_view(frame.target.size[0], frame.target.size[1]);
        let render_target = ChunkRenderTarget::from_frame_target(
            frame.target.with_depth(&depth.view),
            options.color,
        )?;
        draw.render(frame.queue, frame.encoder, render_target, render_view)?;
    }
    queue.submit(std::iter::once(encoder.finish()));

    let pixels = read_rgba8(&device, &queue, &target.texture, width, height)?;
    save_rgba_png(&options.path, width, height, &pixels)?;
    let stats = mesh.stats();

    Ok(HeadlessChunkReport {
        path: options.path,
        width,
        height,
        byte_len: pixels.len(),
        vertex_count: stats.vertex_count,
        index_count: stats.index_count,
    })
}

pub fn write_headless_textured_sections_png(
    options: HeadlessChunkOptions,
    sections: &[TexturedRenderSectionMesh],
    atlas: ChunkTextureAtlas<'_>,
) -> Result<HeadlessChunkReport> {
    write_headless_textured_sections_png_with_options(
        options,
        sections,
        atlas,
        TexturedSectionRenderOptions::default(),
    )
}

pub fn write_headless_textured_sections_png_with_options(
    options: HeadlessChunkOptions,
    sections: &[TexturedRenderSectionMesh],
    atlas: ChunkTextureAtlas<'_>,
    render_options: TexturedSectionRenderOptions,
) -> Result<HeadlessChunkReport> {
    let width = options.width.max(1);
    let height = options.height.max(1);
    let (device, queue) = create_headless_device()?;
    let target = OffscreenTarget::new(&device, width, height, HEADLESS_FORMAT);
    let depth = ChunkDepthTarget::new(&device, width, height);
    let draw =
        TexturedSectionDrawResources::new(&device, &queue, HEADLESS_FORMAT, sections, atlas)?;

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_headless_textured_sections_encoder"),
    });
    {
        let frame = RenderFrameContext::new(&device, &queue, &mut encoder, target.render_target());
        let render_view = options
            .camera
            .render_view(frame.target.size[0], frame.target.size[1]);
        let render_target = ChunkRenderTarget::from_frame_target(
            frame.target.with_depth(&depth.view),
            options.color,
        )?;
        draw.render_with_options(
            frame.queue,
            frame.encoder,
            render_target,
            render_view,
            render_options,
        )?;
    }
    queue.submit(std::iter::once(encoder.finish()));

    let pixels = read_rgba8(&device, &queue, &target.texture, width, height)?;
    save_rgba_png(&options.path, width, height, &pixels)?;
    let vertex_count = sections
        .iter()
        .map(|section| section.stats().vertex_count)
        .sum();
    let index_count = sections
        .iter()
        .map(|section| section.stats().index_count)
        .sum();

    Ok(HeadlessChunkReport {
        path: options.path,
        width,
        height,
        byte_len: pixels.len(),
        vertex_count,
        index_count,
    })
}

pub fn run_headless_textured_sections_timedemo(
    options: HeadlessTimedemoOptions,
    sections: &[TexturedRenderSectionMesh],
    atlas: ChunkTextureAtlas<'_>,
) -> Result<HeadlessTimedemoReport> {
    let width = options.width.max(1);
    let height = options.height.max(1);
    let setup_start = Instant::now();
    let (device, queue) = create_headless_device()?;
    let target = OffscreenTarget::new(&device, width, height, HEADLESS_FORMAT);
    let depth = ChunkDepthTarget::new(&device, width, height);
    let draw =
        TexturedSectionDrawResources::new(&device, &queue, HEADLESS_FORMAT, sections, atlas)?;
    let setup_ms = elapsed_ms(setup_start.elapsed());

    let mut total_frame_ms = 0.0;
    let mut min_frame_ms = f64::INFINITY;
    let mut max_frame_ms = 0.0_f64;
    let mut drawn_section_count = 0_usize;
    let mut max_drawn_section_count = 0_usize;
    let mut frustum_section_count = 0_usize;
    let mut max_frustum_section_count = 0_usize;
    let mut graph_cull_enabled_frame_count = 0_usize;
    let mut graph_culled_section_count = 0_usize;
    let mut max_graph_culled_section_count = 0_usize;
    let mut drawn_index_count = 0_u64;
    let mut max_drawn_index_count = 0_u32;
    let mut frustum_index_count = 0_u64;
    let mut max_frustum_index_count = 0_u32;
    let mut graph_culled_index_count = 0_u64;
    let mut max_graph_culled_index_count = 0_u32;

    for camera in &options.cameras {
        let frame_start = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_headless_timedemo_encoder"),
        });
        let stats = {
            let frame =
                RenderFrameContext::new(&device, &queue, &mut encoder, target.render_target());
            let render_view = camera.render_view(frame.target.size[0], frame.target.size[1]);
            let render_target = ChunkRenderTarget::from_frame_target(
                frame.target.with_depth(&depth.view),
                options.color,
            )?;
            draw.render_with_options(
                frame.queue,
                frame.encoder,
                render_target,
                render_view,
                options.render_options,
            )?
        };
        queue.submit(std::iter::once(encoder.finish()));
        device
            .poll(wgpu::PollType::Wait)
            .context("device poll failed")?;

        let frame_ms = elapsed_ms(frame_start.elapsed());
        total_frame_ms += frame_ms;
        min_frame_ms = min_frame_ms.min(frame_ms);
        max_frame_ms = max_frame_ms.max(frame_ms);
        drawn_section_count += stats.drawn_section_count;
        max_drawn_section_count = max_drawn_section_count.max(stats.drawn_section_count);
        frustum_section_count += stats.frustum_section_count;
        max_frustum_section_count = max_frustum_section_count.max(stats.frustum_section_count);
        if stats.graph_cull_enabled {
            graph_cull_enabled_frame_count += 1;
        }
        graph_culled_section_count += stats.graph_culled_section_count;
        max_graph_culled_section_count =
            max_graph_culled_section_count.max(stats.graph_culled_section_count);
        drawn_index_count += u64::from(stats.drawn_index_count);
        max_drawn_index_count = max_drawn_index_count.max(stats.drawn_index_count);
        frustum_index_count += u64::from(stats.frustum_index_count);
        max_frustum_index_count = max_frustum_index_count.max(stats.frustum_index_count);
        graph_culled_index_count += u64::from(stats.graph_culled_index_count);
        max_graph_culled_index_count =
            max_graph_culled_index_count.max(stats.graph_culled_index_count);
    }

    let frame_count = options.cameras.len();
    let average_frame_ms = if frame_count == 0 {
        0.0
    } else {
        total_frame_ms / frame_count as f64
    };
    let min_frame_ms = if frame_count == 0 { 0.0 } else { min_frame_ms };
    let average_drawn_section_count = if frame_count == 0 {
        0.0
    } else {
        drawn_section_count as f64 / frame_count as f64
    };
    let average_frustum_section_count = if frame_count == 0 {
        0.0
    } else {
        frustum_section_count as f64 / frame_count as f64
    };
    let average_graph_culled_section_count = if frame_count == 0 {
        0.0
    } else {
        graph_culled_section_count as f64 / frame_count as f64
    };
    let average_drawn_index_count = if frame_count == 0 {
        0.0
    } else {
        drawn_index_count as f64 / frame_count as f64
    };
    let average_frustum_index_count = if frame_count == 0 {
        0.0
    } else {
        frustum_index_count as f64 / frame_count as f64
    };
    let average_graph_culled_index_count = if frame_count == 0 {
        0.0
    } else {
        graph_culled_index_count as f64 / frame_count as f64
    };

    Ok(HeadlessTimedemoReport {
        width,
        height,
        frame_count,
        setup_ms,
        total_frame_ms,
        average_frame_ms,
        min_frame_ms,
        max_frame_ms,
        loaded_section_count: draw.section_count(),
        average_drawn_section_count,
        max_drawn_section_count,
        average_frustum_section_count,
        max_frustum_section_count,
        graph_cull_enabled_frame_count,
        average_graph_culled_section_count,
        max_graph_culled_section_count,
        loaded_index_count: draw.index_count(),
        average_drawn_index_count,
        max_drawn_index_count,
        average_frustum_index_count,
        max_frustum_index_count,
        average_graph_culled_index_count,
        max_graph_culled_index_count,
    })
}

fn create_headless_device() -> Result<(wgpu::Device, wgpu::Queue)> {
    pollster::block_on(async {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: native_backends(),
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: None,
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
            })
            .await
            .context("no suitable headless wgpu adapter")?;
        let info = adapter.get_info();
        log::info!(
            "headless adapter '{}' backend={:?} driver={} driver_info={}",
            info.name,
            info.backend,
            info.driver,
            info.driver_info
        );
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("mclone_headless_device"),
                required_features: optional_gpu_features(adapter.features()),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .context("failed to create headless wgpu device")?;
        Ok((device, queue))
    })
}

struct OffscreenTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: [u32; 2],
}

impl OffscreenTarget {
    fn new(device: &wgpu::Device, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_headless_clear_target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            texture,
            view,
            size: [width, height],
        }
    }

    fn render_target(&self) -> RenderFrameTarget<'_> {
        RenderFrameTarget::color(&self.view, self.size)
    }
}

fn read_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    let unpadded_row_bytes = width * BYTES_PER_PIXEL;
    let padded_row_bytes = padded_row_bytes(unpadded_row_bytes);
    let buffer_size = padded_row_bytes as u64 * height as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mclone_headless_clear_readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_headless_clear_readback_encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: Default::default(),
            aspect: Default::default(),
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(std::iter::once(encoder.finish()));

    let slice = staging.slice(..);
    let (sender, receiver) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::Wait)
        .context("device poll failed")?;
    receiver
        .recv()
        .context("failed to receive readback map result")?
        .context("failed to map readback buffer")?;

    let mapped = slice.get_mapped_range();
    let mut pixels = Vec::with_capacity((width * height * BYTES_PER_PIXEL) as usize);
    for row in 0..height {
        let start = (row * padded_row_bytes) as usize;
        let end = start + unpadded_row_bytes as usize;
        pixels.extend_from_slice(&mapped[start..end]);
    }
    drop(mapped);
    staging.unmap();
    Ok(pixels)
}

fn save_rgba_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
    ensure_parent_dir(path)?;
    let image =
        image::RgbaImage::from_raw(width, height, pixels.to_vec()).context("invalid image size")?;
    image
        .save(path)
        .with_context(|| format!("failed to save `{}`", path.display()))?;
    Ok(())
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create `{}`", parent.display()))?;
    }
    Ok(())
}

fn padded_row_bytes(unpadded_row_bytes: u32) -> u32 {
    unpadded_row_bytes.div_ceil(COPY_BYTES_PER_ROW_ALIGNMENT) * COPY_BYTES_PER_ROW_ALIGNMENT
}

fn elapsed_ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_padding_uses_wgpu_copy_alignment() {
        assert_eq!(padded_row_bytes(4), 256);
        assert_eq!(padded_row_bytes(256), 256);
        assert_eq!(padded_row_bytes(260), 512);
    }
}
