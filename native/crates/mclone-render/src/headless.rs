use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use mclone_mesh::{
    RenderSectionKey, TexturedRenderSectionMesh, TexturedVisibleChunkMesh, VisibleChunkMesh,
};
use mclone_ui::{GuiDrawList, GuiScale};

use crate::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkDrawResources, ChunkRenderTarget, ChunkTextureAtlas,
    TexturedChunkDrawResources, TexturedSectionDrawResources, TexturedSectionRenderOptions,
};
use crate::gpu_util::{native_backends, optional_gpu_features};
use crate::gui::{GuiRenderOptions, GuiRenderer};
use crate::target::{RenderFrameContext, RenderFrameTarget};

const BYTES_PER_PIXEL: u32 = 4;
const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;
pub const HEADLESS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

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
pub struct HeadlessUiOptions {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub color: wgpu::Color,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadlessUiReport {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub byte_len: usize,
    pub command_count: usize,
}

#[derive(Clone, Debug)]
pub struct HeadlessFrameOptions {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadlessFrameReport {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub byte_len: usize,
    pub non_clear_rgb_pixel_count: usize,
}

#[derive(Clone, Debug)]
pub struct HeadlessStereoFrameOptions {
    pub path: PathBuf,
    /// Width of one eye. The saved side-by-side image is twice this width.
    pub eye_width: u32,
    pub eye_height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadlessStereoFrameReport {
    pub path: PathBuf,
    pub eye_width: u32,
    pub eye_height: u32,
    pub width: u32,
    pub height: u32,
    pub byte_len: usize,
    pub non_clear_rgb_pixel_count: usize,
    pub eye_pixel_difference_count: usize,
}

#[derive(Clone, Debug)]
pub struct HeadlessMultiviewFrameOptions {
    pub path: PathBuf,
    /// Width of one layer. The saved side-by-side image is twice this width.
    pub eye_width: u32,
    pub eye_height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadlessMultiviewFrameReport {
    pub path: PathBuf,
    pub eye_width: u32,
    pub eye_height: u32,
    pub width: u32,
    pub height: u32,
    pub byte_len: usize,
    pub non_clear_rgb_pixel_count: usize,
    pub eye_pixel_difference_count: usize,
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

#[derive(Clone, Debug)]
pub struct HeadlessFrameLoopOptions {
    pub width: u32,
    pub height: u32,
    pub frame_count: usize,
    pub pace_frame_duration: Option<Duration>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeadlessFrameLoopTiming {
    pub frame_ms: f64,
    pub encode_ms: f64,
    pub submit_ms: f64,
    pub device_poll_ms: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeadlessFrameLoopReport {
    pub width: u32,
    pub height: u32,
    pub frame_count: usize,
    pub setup_ms: f64,
    pub total_frame_ms: f64,
    pub average_frame_ms: f64,
    pub min_frame_ms: f64,
    pub max_frame_ms: f64,
    pub frames: Vec<HeadlessFrameLoopTiming>,
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
    write_headless_textured_sections_png_with_ready_sections(
        options,
        sections,
        atlas,
        render_options,
        None,
    )
}

pub fn write_headless_textured_sections_png_with_ready_sections(
    options: HeadlessChunkOptions,
    sections: &[TexturedRenderSectionMesh],
    atlas: ChunkTextureAtlas<'_>,
    render_options: TexturedSectionRenderOptions,
    ready_sections: Option<&BTreeSet<RenderSectionKey>>,
) -> Result<HeadlessChunkReport> {
    let width = options.width.max(1);
    let height = options.height.max(1);
    let (device, queue) = create_headless_device()?;
    let target = OffscreenTarget::new(&device, width, height, HEADLESS_FORMAT);
    let depth = ChunkDepthTarget::new(&device, width, height);
    let mut draw =
        TexturedSectionDrawResources::new(&device, &queue, HEADLESS_FORMAT, sections, atlas)?;
    if let Some(ready_sections) = ready_sections {
        draw.set_traversal_ready_sections(ready_sections);
    }

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

pub fn write_headless_ui_png(
    options: HeadlessUiOptions,
    draw_list: &GuiDrawList,
) -> Result<HeadlessUiReport> {
    let width = options.width.max(1);
    let height = options.height.max(1);
    let (device, queue) = create_headless_device()?;
    let target = OffscreenTarget::new(&device, width, height, HEADLESS_FORMAT);
    let mut gui = GuiRenderer::new(&device, HEADLESS_FORMAT);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_headless_ui_encoder"),
    });
    {
        let frame = RenderFrameContext::new(&device, &queue, &mut encoder, target.render_target());
        gui.render(
            frame.device,
            frame.queue,
            frame.encoder,
            frame.target,
            {
                let scale = GuiScale::from_pixels(width, height);
                [scale.width, scale.height]
            },
            draw_list,
            GuiRenderOptions::clear(options.color),
        )?;
    }
    queue.submit(std::iter::once(encoder.finish()));

    let pixels = read_rgba8(&device, &queue, &target.texture, width, height)?;
    save_rgba_png(&options.path, width, height, &pixels)?;

    Ok(HeadlessUiReport {
        path: options.path,
        width,
        height,
        byte_len: pixels.len(),
        command_count: draw_list.commands().len(),
    })
}

pub fn write_headless_frame_png<T, F>(
    options: HeadlessFrameOptions,
    render: F,
) -> Result<(HeadlessFrameReport, T)>
where
    F: FnOnce(RenderFrameContext<'_>) -> Result<T>,
{
    let (report, pixels, render_output) = capture_headless_frame(options, render)?;
    save_rgba_png(&report.path, report.width, report.height, &pixels)?;
    Ok((report, render_output))
}

pub fn capture_headless_frame<T, F>(
    options: HeadlessFrameOptions,
    render: F,
) -> Result<(HeadlessFrameReport, Vec<u8>, T)>
where
    F: FnOnce(RenderFrameContext<'_>) -> Result<T>,
{
    let width = options.width.max(1);
    let height = options.height.max(1);
    let (device, queue) = create_headless_device()?;
    let target = OffscreenTarget::new(&device, width, height, HEADLESS_FORMAT);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_headless_frame_encoder"),
    });
    let render_output = {
        let frame = RenderFrameContext::new(&device, &queue, &mut encoder, target.render_target());
        render(frame)?
    };
    queue.submit(std::iter::once(encoder.finish()));

    let pixels = read_rgba8(&device, &queue, &target.texture, width, height)?;

    Ok((
        HeadlessFrameReport {
            path: options.path,
            width,
            height,
            byte_len: pixels.len(),
            non_clear_rgb_pixel_count: count_non_clear_rgb_pixels(&pixels),
        },
        pixels,
        render_output,
    ))
}

/// Render two ordinary per-eye textures, read them back, and save one
/// side-by-side PNG. The callback owns submission because stereo scene hosts
/// commonly submit each eye through their existing renderer path rather than
/// a caller-owned command encoder.
pub fn write_headless_stereo_frame_png<T, F>(
    options: HeadlessStereoFrameOptions,
    render: F,
) -> Result<(HeadlessStereoFrameReport, T)>
where
    F: FnOnce(
        &wgpu::Device,
        &wgpu::Queue,
        wgpu::TextureFormat,
        [u32; 2],
        &wgpu::TextureView,
        &wgpu::TextureView,
    ) -> Result<T>,
{
    let eye_width = options.eye_width.max(1);
    let eye_height = options.eye_height.max(1);
    let width = eye_width
        .checked_mul(2)
        .context("headless stereo output width overflow")?;
    let (device, queue) = create_headless_device()?;
    let left = OffscreenTarget::new(&device, eye_width, eye_height, HEADLESS_FORMAT);
    let right = OffscreenTarget::new(&device, eye_width, eye_height, HEADLESS_FORMAT);
    let output = render(
        &device,
        &queue,
        HEADLESS_FORMAT,
        [eye_width, eye_height],
        &left.view,
        &right.view,
    )?;
    device
        .poll(wgpu::PollType::Wait)
        .context("device poll failed after stereo render")?;
    let left_pixels = read_rgba8(&device, &queue, &left.texture, eye_width, eye_height)?;
    let right_pixels = read_rgba8(&device, &queue, &right.texture, eye_width, eye_height)?;
    let eye_pixel_difference_count = left_pixels
        .chunks_exact(BYTES_PER_PIXEL as usize)
        .zip(right_pixels.chunks_exact(BYTES_PER_PIXEL as usize))
        .filter(|(left, right)| left != right)
        .count();
    let pixels = stitch_rgba8_side_by_side(eye_width, eye_height, &left_pixels, &right_pixels)?;
    save_rgba_png(&options.path, width, eye_height, &pixels)?;
    Ok((
        HeadlessStereoFrameReport {
            path: options.path,
            eye_width,
            eye_height,
            width,
            height: eye_height,
            byte_len: pixels.len(),
            non_clear_rgb_pixel_count: count_non_clear_rgb_pixels(&pixels),
            eye_pixel_difference_count,
        },
        output,
    ))
}

/// Render one full-frame two-layer multiview target, read both layers, and
/// save one side-by-side PNG. The callback owns submission so it can use the
/// renderer's ordinary full-frame multiview command path.
pub fn write_headless_multiview_frame_png<T, F>(
    options: HeadlessMultiviewFrameOptions,
    render: F,
) -> Result<(HeadlessMultiviewFrameReport, T)>
where
    F: FnOnce(
        &wgpu::Device,
        &wgpu::Queue,
        wgpu::TextureFormat,
        [u32; 2],
        &wgpu::TextureView,
        &wgpu::TextureView,
    ) -> Result<T>,
{
    let eye_width = options.eye_width.max(1);
    let eye_height = options.eye_height.max(1);
    let width = eye_width
        .checked_mul(2)
        .context("headless multiview output width overflow")?;
    let (device, queue) = create_headless_device()?;
    if !device.features().contains(wgpu::Features::MULTIVIEW) {
        bail!("headless adapter does not expose wgpu MULTIVIEW");
    }
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mclone_headless_multiview_color"),
        size: wgpu::Extent3d {
            width: eye_width,
            height: eye_height,
            depth_or_array_layers: 2,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HEADLESS_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let color_view = color.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        array_layer_count: Some(2),
        ..Default::default()
    });
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mclone_headless_multiview_depth"),
        size: wgpu::Extent3d {
            width: eye_width,
            height: eye_height,
            depth_or_array_layers: 2,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: crate::chunk::DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        array_layer_count: Some(2),
        ..Default::default()
    });
    let output = render(
        &device,
        &queue,
        HEADLESS_FORMAT,
        [eye_width, eye_height],
        &color_view,
        &depth_view,
    )?;
    device
        .poll(wgpu::PollType::Wait)
        .context("device poll failed after multiview render")?;
    let left_pixels = read_rgba8_layer(&device, &queue, &color, eye_width, eye_height, 0)?;
    let right_pixels = read_rgba8_layer(&device, &queue, &color, eye_width, eye_height, 1)?;
    let eye_pixel_difference_count = left_pixels
        .chunks_exact(BYTES_PER_PIXEL as usize)
        .zip(right_pixels.chunks_exact(BYTES_PER_PIXEL as usize))
        .filter(|(left, right)| left != right)
        .count();
    let pixels = stitch_rgba8_side_by_side(eye_width, eye_height, &left_pixels, &right_pixels)?;
    save_rgba_png(&options.path, width, eye_height, &pixels)?;
    Ok((
        HeadlessMultiviewFrameReport {
            path: options.path,
            eye_width,
            eye_height,
            width,
            height: eye_height,
            byte_len: pixels.len(),
            non_clear_rgb_pixel_count: count_non_clear_rgb_pixels(&pixels),
            eye_pixel_difference_count,
        },
        output,
    ))
}

fn stitch_rgba8_side_by_side(
    eye_width: u32,
    eye_height: u32,
    left: &[u8],
    right: &[u8],
) -> Result<Vec<u8>> {
    let row_bytes = eye_width as usize * BYTES_PER_PIXEL as usize;
    let expected = row_bytes * eye_height as usize;
    if left.len() != expected || right.len() != expected {
        bail!(
            "invalid stereo readback sizes: expected {expected} bytes per eye, left={} right={}",
            left.len(),
            right.len()
        );
    }
    let mut pixels = Vec::with_capacity(expected * 2);
    for row in 0..eye_height as usize {
        let start = row * row_bytes;
        let end = start + row_bytes;
        pixels.extend_from_slice(&left[start..end]);
        pixels.extend_from_slice(&right[start..end]);
    }
    Ok(pixels)
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

pub fn run_headless_frame_loop<S, Init, Frame>(
    options: HeadlessFrameLoopOptions,
    init: Init,
    mut render_frame: Frame,
) -> Result<(HeadlessFrameLoopReport, S)>
where
    Init: FnOnce(&wgpu::Device, &wgpu::Queue, wgpu::TextureFormat, [u32; 2]) -> Result<S>,
    Frame: FnMut(usize, RenderFrameContext<'_>, &mut S) -> Result<()>,
{
    let setup_start = Instant::now();
    let width = options.width.max(1);
    let height = options.height.max(1);
    let (device, queue) = create_headless_device()?;
    let target = OffscreenTarget::new(&device, width, height, HEADLESS_FORMAT);
    let mut state = init(&device, &queue, HEADLESS_FORMAT, [width, height])?;
    let setup_ms = elapsed_ms(setup_start.elapsed());
    let mut frames = Vec::with_capacity(options.frame_count);
    let mut total_frame_ms = 0.0;
    let mut min_frame_ms = f64::INFINITY;
    let mut max_frame_ms = 0.0_f64;

    for index in 0..options.frame_count {
        let frame_start = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_headless_frame_loop_encoder"),
        });
        let encode_start = Instant::now();
        {
            let frame =
                RenderFrameContext::new(&device, &queue, &mut encoder, target.render_target());
            render_frame(index, frame, &mut state)?;
        }
        let encode_ms = elapsed_ms(encode_start.elapsed());

        let submit_start = Instant::now();
        queue.submit(std::iter::once(encoder.finish()));
        let submit_ms = elapsed_ms(submit_start.elapsed());

        let poll_start = Instant::now();
        device
            .poll(wgpu::PollType::Wait)
            .context("device poll failed")?;
        let device_poll_ms = elapsed_ms(poll_start.elapsed());

        let frame_ms = elapsed_ms(frame_start.elapsed());
        total_frame_ms += frame_ms;
        min_frame_ms = min_frame_ms.min(frame_ms);
        max_frame_ms = max_frame_ms.max(frame_ms);
        frames.push(HeadlessFrameLoopTiming {
            frame_ms,
            encode_ms,
            submit_ms,
            device_poll_ms,
        });
        if let Some(frame_duration) = options.pace_frame_duration {
            let elapsed = frame_start.elapsed();
            if elapsed < frame_duration {
                std::thread::sleep(frame_duration - elapsed);
            }
        }
    }

    let frame_count = frames.len();
    let average_frame_ms = if frame_count == 0 {
        0.0
    } else {
        total_frame_ms / frame_count as f64
    };
    let min_frame_ms = if frame_count == 0 { 0.0 } else { min_frame_ms };

    Ok((
        HeadlessFrameLoopReport {
            width,
            height,
            frame_count,
            setup_ms,
            total_frame_ms,
            average_frame_ms,
            min_frame_ms,
            max_frame_ms,
            frames,
        },
        state,
    ))
}

pub fn run_headless_capture_loop<S, Init, Frame>(
    options: HeadlessFrameLoopOptions,
    init: Init,
    render_frame: Frame,
) -> Result<(HeadlessFrameLoopReport, Vec<Vec<u8>>, S)>
where
    Init: FnOnce(&wgpu::Device, &wgpu::Queue, wgpu::TextureFormat, [u32; 2]) -> Result<S>,
    Frame: FnMut(usize, RenderFrameContext<'_>, &mut S) -> Result<()>,
{
    let (report, pixels, _, state) =
        run_headless_capture_loop_with_aux(options, init, render_frame, |_, _, _, _| Ok(()))?;
    Ok((report, pixels, state))
}

/// Run the normal color capture loop and collect one caller-owned auxiliary
/// readback after each rendered frame has been submitted. This keeps probe-only
/// depth/ID capture out of production frame orchestration while guaranteeing
/// that the auxiliary texture reflects the same frame as the RGBA result.
pub fn run_headless_capture_loop_with_aux<S, A, Init, Frame, Capture>(
    options: HeadlessFrameLoopOptions,
    init: Init,
    mut render_frame: Frame,
    mut capture_frame: Capture,
) -> Result<(HeadlessFrameLoopReport, Vec<Vec<u8>>, Vec<A>, S)>
where
    Init: FnOnce(&wgpu::Device, &wgpu::Queue, wgpu::TextureFormat, [u32; 2]) -> Result<S>,
    Frame: FnMut(usize, RenderFrameContext<'_>, &mut S) -> Result<()>,
    Capture: FnMut(usize, &wgpu::Device, &wgpu::Queue, &S) -> Result<A>,
{
    let setup_start = Instant::now();
    let width = options.width.max(1);
    let height = options.height.max(1);
    let (device, queue) = create_headless_device()?;
    let target = OffscreenTarget::new(&device, width, height, HEADLESS_FORMAT);
    let mut state = init(&device, &queue, HEADLESS_FORMAT, [width, height])?;
    let setup_ms = elapsed_ms(setup_start.elapsed());
    let mut frames = Vec::with_capacity(options.frame_count);
    let mut pixels = Vec::with_capacity(options.frame_count);
    let mut auxiliary = Vec::with_capacity(options.frame_count);
    let mut total_frame_ms = 0.0;
    let mut min_frame_ms = f64::INFINITY;
    let mut max_frame_ms = 0.0_f64;

    for index in 0..options.frame_count {
        let frame_start = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_headless_capture_loop_encoder"),
        });
        let encode_start = Instant::now();
        {
            let frame =
                RenderFrameContext::new(&device, &queue, &mut encoder, target.render_target());
            render_frame(index, frame, &mut state)?;
        }
        let encode_ms = elapsed_ms(encode_start.elapsed());

        let submit_start = Instant::now();
        queue.submit(std::iter::once(encoder.finish()));
        let submit_ms = elapsed_ms(submit_start.elapsed());

        let poll_start = Instant::now();
        device
            .poll(wgpu::PollType::Wait)
            .context("device poll failed")?;
        let device_poll_ms = elapsed_ms(poll_start.elapsed());

        pixels.push(read_rgba8(&device, &queue, &target.texture, width, height)?);
        auxiliary.push(capture_frame(index, &device, &queue, &state)?);

        let frame_ms = elapsed_ms(frame_start.elapsed());
        total_frame_ms += frame_ms;
        min_frame_ms = min_frame_ms.min(frame_ms);
        max_frame_ms = max_frame_ms.max(frame_ms);
        frames.push(HeadlessFrameLoopTiming {
            frame_ms,
            encode_ms,
            submit_ms,
            device_poll_ms,
        });
        if let Some(frame_duration) = options.pace_frame_duration {
            let elapsed = frame_start.elapsed();
            if elapsed < frame_duration {
                std::thread::sleep(frame_duration - elapsed);
            }
        }
    }

    let frame_count = frames.len();
    let average_frame_ms = if frame_count == 0 {
        0.0
    } else {
        total_frame_ms / frame_count as f64
    };
    let min_frame_ms = if frame_count == 0 { 0.0 } else { min_frame_ms };

    Ok((
        HeadlessFrameLoopReport {
            width,
            height,
            frame_count,
            setup_ms,
            total_frame_ms,
            average_frame_ms,
            min_frame_ms,
            max_frame_ms,
            frames,
        },
        pixels,
        auxiliary,
        state,
    ))
}

/// Run an offscreen frame loop with a caller-owned post-submit readback while
/// retaining only the compact auxiliary result. This is intended for paced
/// diagnostics such as per-frame depth/ID coverage probes where keeping every
/// full RGBA or depth frame would add hundreds of megabytes of unrelated test
/// storage.
pub fn run_headless_aux_loop<S, A, Init, Frame, Capture>(
    options: HeadlessFrameLoopOptions,
    init: Init,
    mut render_frame: Frame,
    mut capture_frame: Capture,
) -> Result<(HeadlessFrameLoopReport, Vec<A>, S)>
where
    Init: FnOnce(&wgpu::Device, &wgpu::Queue, wgpu::TextureFormat, [u32; 2]) -> Result<S>,
    Frame: FnMut(usize, RenderFrameContext<'_>, &mut S) -> Result<()>,
    Capture: FnMut(usize, &wgpu::Device, &wgpu::Queue, &mut S) -> Result<A>,
{
    let setup_start = Instant::now();
    let width = options.width.max(1);
    let height = options.height.max(1);
    let (device, queue) = create_headless_device()?;
    let target = OffscreenTarget::new(&device, width, height, HEADLESS_FORMAT);
    let mut state = init(&device, &queue, HEADLESS_FORMAT, [width, height])?;
    let setup_ms = elapsed_ms(setup_start.elapsed());
    let mut frames = Vec::with_capacity(options.frame_count);
    let mut auxiliary = Vec::with_capacity(options.frame_count);
    let mut total_frame_ms = 0.0;
    let mut min_frame_ms = f64::INFINITY;
    let mut max_frame_ms = 0.0_f64;

    for index in 0..options.frame_count {
        let frame_start = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_headless_aux_loop_encoder"),
        });
        let encode_start = Instant::now();
        {
            let frame =
                RenderFrameContext::new(&device, &queue, &mut encoder, target.render_target());
            render_frame(index, frame, &mut state)?;
        }
        let encode_ms = elapsed_ms(encode_start.elapsed());

        let submit_start = Instant::now();
        queue.submit(std::iter::once(encoder.finish()));
        let submit_ms = elapsed_ms(submit_start.elapsed());

        let poll_start = Instant::now();
        device
            .poll(wgpu::PollType::Wait)
            .context("device poll failed")?;
        let device_poll_ms = elapsed_ms(poll_start.elapsed());

        auxiliary.push(capture_frame(index, &device, &queue, &mut state)?);

        let frame_ms = elapsed_ms(frame_start.elapsed());
        total_frame_ms += frame_ms;
        min_frame_ms = min_frame_ms.min(frame_ms);
        max_frame_ms = max_frame_ms.max(frame_ms);
        frames.push(HeadlessFrameLoopTiming {
            frame_ms,
            encode_ms,
            submit_ms,
            device_poll_ms,
        });
        if let Some(frame_duration) = options.pace_frame_duration {
            let elapsed = frame_start.elapsed();
            if elapsed < frame_duration {
                std::thread::sleep(frame_duration - elapsed);
            }
        }
    }

    let frame_count = frames.len();
    let average_frame_ms = if frame_count == 0 {
        0.0
    } else {
        total_frame_ms / frame_count as f64
    };
    let min_frame_ms = if frame_count == 0 { 0.0 } else { min_frame_ms };

    Ok((
        HeadlessFrameLoopReport {
            width,
            height,
            frame_count,
            setup_ms,
            total_frame_ms,
            average_frame_ms,
            min_frame_ms,
            max_frame_ms,
            frames,
        },
        auxiliary,
        state,
    ))
}

pub fn create_headless_device() -> Result<(wgpu::Device, wgpu::Queue)> {
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
    read_rgba8_layer(device, queue, texture, width, height, 0)
}

/// Read one caller-owned RGBA8 offscreen texture after submission. Product
/// render paths never call this; diagnostic hosts use it to collect auxiliary
/// targets produced in the same frame as the primary capture.
pub fn read_headless_rgba8_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    read_rgba8(device, queue, texture, width.max(1), height.max(1))
}

/// Read a single-sample `Depth32Float` target in row-major pixel order.
/// Reversed-Z terrain targets clear to `0.0`, so callers can distinguish
/// untouched pixels from any real or synthetic geometry without consulting
/// color output.
pub fn read_depth32(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    depth: &ChunkDepthTarget,
) -> Result<Vec<f32>> {
    let unpadded_row_bytes = depth.width * std::mem::size_of::<f32>() as u32;
    let padded_row_bytes = padded_row_bytes(unpadded_row_bytes);
    let buffer_size = padded_row_bytes as u64 * depth.height as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mclone_headless_depth_readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_headless_depth_readback_encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: depth.texture(),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::DepthOnly,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes),
                rows_per_image: Some(depth.height),
            },
        },
        wgpu::Extent3d {
            width: depth.width,
            height: depth.height,
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
        .context("failed to receive depth readback map result")?
        .context("failed to map depth readback buffer")?;

    let mapped = slice.get_mapped_range();
    let mut values = Vec::with_capacity((depth.width * depth.height) as usize);
    for row in 0..depth.height {
        let start = (row * padded_row_bytes) as usize;
        let row_bytes = &mapped[start..start + unpadded_row_bytes as usize];
        values.extend(
            row_bytes
                .chunks_exact(4)
                .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
        );
    }
    drop(mapped);
    staging.unmap();
    Ok(values)
}

fn read_rgba8_layer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    layer: u32,
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
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: layer,
            },
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

pub fn save_rgba_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
    ensure_parent_dir(path)?;
    let image =
        image::RgbaImage::from_raw(width, height, pixels.to_vec()).context("invalid image size")?;
    image
        .save(path)
        .with_context(|| format!("failed to save `{}`", path.display()))?;
    Ok(())
}

fn count_non_clear_rgb_pixels(pixels: &[u8]) -> usize {
    let Some(clear) = pixels.get(0..3) else {
        return 0;
    };
    pixels
        .chunks_exact(4)
        .filter(|pixel| &pixel[0..3] != clear)
        .count()
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
    use crate::placement::{EmbeddedChunkRegion, WorldCompositionContext, WorldPlacement};
    use crate::uniform::{
        LEFT_EYE_VIEW_SLOT, MAX_PRESENTATION_VIEW_COUNT, PerViewSlot, PerViewUniformBuffer,
        PresentationViewIndex, RIGHT_EYE_VIEW_SLOT, SINGLE_VIEW_SLOT,
    };
    use glam::Vec3;
    use mclone_core::{ChunkPos, Vec3d};
    use mclone_mesh::{GrassPatch, TexturedChunkVertex, VisibilitySet};

    #[test]
    fn row_padding_uses_wgpu_copy_alignment() {
        assert_eq!(padded_row_bytes(4), 256);
        assert_eq!(padded_row_bytes(256), 256);
        assert_eq!(padded_row_bytes(260), 512);
    }

    #[test]
    #[ignore = "GPU visual proof for Tactical 238 grass quality and wind"]
    fn static_grass_renders_request_gated_patch_artifacts() -> Result<()> {
        const WIDTH: u32 = 960;
        const HEIGHT: u32 = 640;
        let mut section = fixture_active_section();
        for z in -2..=2 {
            for x in -3..=3 {
                section.grass_patches.push(GrassPatch {
                    root: [x, 1, z],
                    packed_tint: 0x0048_b850,
                    packed_light: 0x00f0_00f0,
                    seed: (x as u32).wrapping_mul(0x9e37_79b9)
                        ^ (z as u32).wrapping_mul(0x85eb_ca6b),
                    flags: 0,
                    reserved: 0,
                });
            }
        }
        let sections = [section];
        let atlas_rgba = [255, 255, 255, 255];
        let camera = fixture_camera(0.0);
        let baseline_path = PathBuf::from("/tmp/mclone-238-static-grass-off.png");
        let base_options = HeadlessChunkOptions {
            path: baseline_path.clone(),
            width: WIDTH,
            height: HEIGHT,
            color: fixture_clear_color(),
            camera,
        };
        let atlas = ChunkTextureAtlas {
            width: 1,
            height: 1,
            rgba: &atlas_rgba,
        };
        let render_options = TexturedSectionRenderOptions {
            force_fullbright: true,
            section_occlusion_culling: false,
            ..TexturedSectionRenderOptions::default()
        };
        write_headless_textured_sections_png_with_options(
            base_options.clone(),
            &sections,
            atlas,
            render_options,
        )?;
        let baseline = image::ImageReader::open(baseline_path)?
            .decode()?
            .to_rgba8();
        let mut previous_changed_pixels = 0;
        for (quality, label) in [
            (crate::GrassQuality::Sparse, "sparse"),
            (crate::GrassQuality::Lush, "lush"),
            (crate::GrassQuality::Ultra, "ultra"),
        ] {
            let path = PathBuf::from(format!("/tmp/mclone-238-static-grass-{label}.png"));
            write_headless_textured_sections_png_with_options(
                HeadlessChunkOptions {
                    path: path.clone(),
                    ..base_options.clone()
                },
                &sections,
                atlas,
                render_options.with_grass_detail(quality),
            )?;
            let enabled = image::ImageReader::open(path)?.decode()?.to_rgba8();
            let changed_pixels = baseline
                .pixels()
                .zip(enabled.pixels())
                .filter(|(baseline, enabled)| baseline != enabled)
                .count();
            assert!(
                changed_pixels > previous_changed_pixels,
                "{label} should add pixels beyond the preceding quality: \
                 {changed_pixels} <= {previous_changed_pixels}"
            );
            previous_changed_pixels = changed_pixels;
        }
        assert!(previous_changed_pixels > 500);

        let wind_start_path = PathBuf::from("/tmp/mclone-238-static-grass-lush.png");
        let wind_later_path = PathBuf::from("/tmp/mclone-238-wind-lush-t1.png");
        write_headless_textured_sections_png_with_options(
            HeadlessChunkOptions {
                path: wind_later_path.clone(),
                ..base_options.clone()
            },
            &sections,
            atlas,
            render_options
                .with_grass_detail(crate::GrassQuality::Lush)
                .with_grass_time_seconds(3.25),
        )?;
        let wind_start = image::ImageReader::open(wind_start_path)?
            .decode()?
            .to_rgba8();
        let wind_later = image::ImageReader::open(wind_later_path)?
            .decode()?
            .to_rgba8();
        let animated_pixels = wind_start
            .pixels()
            .zip(wind_later.pixels())
            .filter(|(start, later)| start != later)
            .count();
        assert!(
            animated_pixels > 100,
            "fixed-camera wind times should produce visible deformation"
        );

        let mut interactors = crate::GrassInteractorSet::default();
        interactors.push(
            crate::GrassInteractor::new(
                crate::GrassInteractorIdentity::LocalPlayer,
                [0.0, 1.0, 0.0],
                1.0,
            )
            .expect("fixture interactor is finite"),
        );
        let interaction_path = PathBuf::from("/tmp/mclone-238-interaction-contact.png");
        let recovery_path = PathBuf::from("/tmp/mclone-238-interaction-recovery.png");
        let recovery_control_path =
            PathBuf::from("/tmp/mclone-238-interaction-recovery-control.png");
        write_headless_textured_sections_png_with_options(
            HeadlessChunkOptions {
                path: recovery_control_path.clone(),
                ..base_options.clone()
            },
            &sections,
            atlas,
            render_options
                .with_grass_detail(crate::GrassQuality::Lush)
                .with_grass_time_seconds(4.0),
        )?;
        let (device, queue) = create_headless_device()?;
        let target = OffscreenTarget::new(&device, WIDTH, HEIGHT, HEADLESS_FORMAT);
        let depth = ChunkDepthTarget::new(&device, WIDTH, HEIGHT);
        let draw =
            TexturedSectionDrawResources::new(&device, &queue, HEADLESS_FORMAT, &sections, atlas)?;
        let interaction_options = render_options
            .with_grass_detail(crate::GrassQuality::Lush)
            .with_grass_interactors(interactors);
        for step in 0..=16 {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_headless_grass_interaction_recovery_encoder"),
            });
            let frame =
                RenderFrameContext::new(&device, &queue, &mut encoder, target.render_target());
            let render_view = camera.render_view(WIDTH, HEIGHT);
            let render_target = ChunkRenderTarget::from_frame_target(
                frame.target.with_depth(&depth.view),
                fixture_clear_color(),
            )?;
            draw.render_with_options(
                frame.queue,
                frame.encoder,
                render_target,
                render_view,
                if step == 0 {
                    interaction_options
                } else {
                    render_options
                        .with_grass_detail(crate::GrassQuality::Lush)
                        .with_grass_time_seconds(step as f32 * 0.25)
                },
            )?;
            queue.submit(std::iter::once(encoder.finish()));
            if step == 0 {
                let pixels = read_rgba8(&device, &queue, &target.texture, WIDTH, HEIGHT)?;
                save_rgba_png(&interaction_path, WIDTH, HEIGHT, &pixels)?;
            } else if step == 16 {
                let pixels = read_rgba8(&device, &queue, &target.texture, WIDTH, HEIGHT)?;
                save_rgba_png(&recovery_path, WIDTH, HEIGHT, &pixels)?;
            }
        }
        let interaction = image::ImageReader::open(&interaction_path)?
            .decode()?
            .to_rgba8();
        let recovery = image::ImageReader::open(&recovery_path)?
            .decode()?
            .to_rgba8();
        let recovery_control = image::ImageReader::open(&recovery_control_path)?
            .decode()?
            .to_rgba8();
        let interaction_pixels = wind_start
            .pixels()
            .zip(interaction.pixels())
            .filter(|(start, interaction)| start != interaction)
            .count();
        assert!(
            interaction_pixels > 40,
            "a qualifying footprint should visibly bend nearby grass"
        );
        let recovery_pixels = recovery_control
            .pixels()
            .zip(recovery.pixels())
            .filter(|(control, recovery)| control != recovery)
            .count();
        assert!(
            recovery_pixels < interaction_pixels,
            "grass should recover toward its unstamped pose: \
             {recovery_pixels} >= {interaction_pixels}"
        );
        Ok(())
    }

    #[test]
    #[ignore = "GPU validation proof for 107 Slice D; run on hosts with a wgpu adapter"]
    fn one_submit_keeps_four_presentation_view_uniforms_live() -> Result<()> {
        const SHADER: &str = r#"
@group(0) @binding(0)
var<uniform> color: vec4<f32>;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VertexOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return color;
}
"#;
        let (device, queue) = create_headless_device()?;
        let targets = (0..MAX_PRESENTATION_VIEW_COUNT)
            .map(|_| OffscreenTarget::new(&device, 8, 8, HEADLESS_FORMAT))
            .collect::<Vec<_>>();
        let uniforms = PerViewUniformBuffer::new(
            &device,
            "mclone_test_per_view_uniforms",
            16,
            MAX_PRESENTATION_VIEW_COUNT,
        );
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_test_per_view_uniform_layout"),
            entries: &[uniforms.layout_entry(0, wgpu::ShaderStages::FRAGMENT)],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_test_per_view_uniform_bind_group"),
            layout: &bind_group_layout,
            entries: &[uniforms.bind_group_entry(0)],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_test_per_view_uniform_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_test_per_view_uniform_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_test_per_view_uniform_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HEADLESS_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_test_per_view_uniform_encoder"),
        });
        let colors = [
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [1.0, 1.0, 0.0, 1.0],
        ];
        for (view_index, (target, color)) in targets.iter().zip(colors).enumerate() {
            let slot = PerViewSlot::for_view(PresentationViewIndex::new(view_index as u32));
            let offset = uniforms.write_slot(&queue, slot, &color_bytes(color));
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mclone_test_per_view_uniform_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[offset]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));
        let expected = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 0, 255],
        ];
        for (target, expected) in targets.iter().zip(expected) {
            let pixels = read_rgba8(&device, &queue, &target.texture, 8, 8)?;
            assert_eq!(pixels.get(0..4), Some(&expected[..]));
        }
        Ok(())
    }

    #[test]
    #[ignore = "GPU visual proof for live-diorama Slice 2; run on hosts with a wgpu adapter"]
    fn placed_terrain_composes_with_shared_depth_and_stereo() -> Result<()> {
        const WIDTH: u32 = 960;
        const HEIGHT: u32 = 640;
        let (device, queue) = create_headless_device()?;
        let atlas_rgba = [255, 255, 255, 255];
        let atlas = ChunkTextureAtlas {
            width: 1,
            height: 1,
            rgba: &atlas_rgba,
        };
        let active_sections = [fixture_active_section()];
        let mut preview_section = fixture_preview_section();
        for z in 4..=12 {
            for x in 4..=12 {
                preview_section.grass_patches.push(GrassPatch {
                    root: [x, 12, z],
                    packed_tint: 0x0048_b850,
                    packed_light: 0x00f0_00f0,
                    seed: (x as u32).wrapping_mul(0x9e37_79b9)
                        ^ (z as u32).wrapping_mul(0x85eb_ca6b),
                    flags: 0,
                    reserved: 0,
                });
            }
        }
        let preview_sections = [preview_section];
        let active = TexturedSectionDrawResources::new(
            &device,
            &queue,
            HEADLESS_FORMAT,
            &active_sections,
            atlas,
        )?;
        let preview = TexturedSectionDrawResources::new_with_shared_resources(
            &device,
            &queue,
            &preview_sections,
            active.shared_resources(),
        )?;
        assert!(active.shares_immutable_resources_with(&preview));
        assert_eq!(active.shared_resource_owner_count(), 2);
        let placed_renderer = preview.create_placed_renderer(&device);
        assert!(!placed_renderer.clipped_renderer_materialized());
        let placement =
            WorldPlacement::new(Vec3d::new(8.0, 0.0, 8.0), Vec3d::new(0.0, 0.5, 0.0), 0.2)?;
        let region = EmbeddedChunkRegion::new(ChunkPos::new(0, 0), 0, 0, 0)?;
        let context = WorldCompositionContext::unbounded(placement, Some(region.source_bounds()));
        let preview_records = preview.prepare_render_records_for_context(context);
        let mut render_options = TexturedSectionRenderOptions::default();
        render_options.force_fullbright = true;
        render_options.section_occlusion_culling = false;
        let mut interactors = crate::GrassInteractorSet::default();
        interactors.push(
            crate::GrassInteractor::new(
                crate::GrassInteractorIdentity::Entity(238),
                [8.0, 12.0, 8.0],
                1.2,
            )
            .expect("placed fixture interactor is finite"),
        );
        render_options = render_options
            .with_grass_detail(crate::GrassQuality::Lush)
            .with_grass_interactors(interactors);
        let view = fixture_camera(0.0).render_view(WIDTH, HEIGHT);

        let active_only = OffscreenTarget::new(&device, WIDTH, HEIGHT, HEADLESS_FORMAT);
        let active_only_depth = ChunkDepthTarget::new(&device, WIDTH, HEIGHT);
        render_placed_fixture_view(
            &device,
            &queue,
            &active_only,
            &active_only_depth,
            &active,
            &preview,
            &placed_renderer,
            &preview_records,
            view,
            render_options,
            context,
            LEFT_EYE_VIEW_SLOT,
            true,
            false,
        )?;
        let preview_only = OffscreenTarget::new(&device, WIDTH, HEIGHT, HEADLESS_FORMAT);
        let preview_only_depth = ChunkDepthTarget::new(&device, WIDTH, HEIGHT);
        render_placed_fixture_view(
            &device,
            &queue,
            &preview_only,
            &preview_only_depth,
            &active,
            &preview,
            &placed_renderer,
            &preview_records,
            view,
            render_options,
            context,
            LEFT_EYE_VIEW_SLOT,
            false,
            true,
        )?;
        let composed = OffscreenTarget::new(&device, WIDTH, HEIGHT, HEADLESS_FORMAT);
        let composed_depth = ChunkDepthTarget::new(&device, WIDTH, HEIGHT);
        let composed_stats = render_placed_fixture_view(
            &device,
            &queue,
            &composed,
            &composed_depth,
            &active,
            &preview,
            &placed_renderer,
            &preview_records,
            view,
            render_options,
            context,
            LEFT_EYE_VIEW_SLOT,
            true,
            true,
        )?;
        let active_pixels = read_rgba8(&device, &queue, &active_only.texture, WIDTH, HEIGHT)?;
        let preview_pixels = read_rgba8(&device, &queue, &preview_only.texture, WIDTH, HEIGHT)?;
        let composed_pixels = read_rgba8(&device, &queue, &composed.texture, WIDTH, HEIGHT)?;
        let active_over_preview = pixel_transition_count(
            &active_pixels,
            &preview_pixels,
            &composed_pixels,
            is_brown,
            is_green,
            is_brown,
        );
        let preview_over_active = pixel_transition_count(
            &active_pixels,
            &preview_pixels,
            &composed_pixels,
            is_blue,
            is_green,
            is_green,
        );
        assert!(
            active_over_preview > 100,
            "table did not occlude preview: {active_over_preview}"
        );
        assert!(
            preview_over_active > 100,
            "preview did not occlude far active wall: {preview_over_active}"
        );
        assert_eq!(composed_stats.drawn_section_count, 1);
        assert!(composed_stats.grass_drawn_patch_count > 0);
        assert_eq!(composed_stats.grass_interaction_field_count, 1);
        assert!(
            composed_stats.grass_interaction_active_cell_count > 0,
            "placed interaction did not stamp source grass: {composed_stats:?}"
        );
        assert_eq!(composed_stats.grass_interaction_uploaded_bytes, 65_536);
        save_rgba_png(
            Path::new("/tmp/mclone-live-diorama-slice2-mono.png"),
            WIDTH,
            HEIGHT,
            &composed_pixels,
        )?;

        let left = OffscreenTarget::new(&device, 640, 640, HEADLESS_FORMAT);
        let right = OffscreenTarget::new(&device, 640, 640, HEADLESS_FORMAT);
        let left_depth = ChunkDepthTarget::new(&device, 640, 640);
        let right_depth = ChunkDepthTarget::new(&device, 640, 640);
        let stereo_views = [
            fixture_camera(-0.12).render_view(640, 640),
            fixture_camera(0.12).render_view(640, 640),
        ];
        let prepared_stereo = preview.prepare_placed_stereo_draw(
            &preview_records,
            stereo_views,
            [render_options; 2],
            context,
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_placed_fixture_stereo_encoder"),
        });
        for (target, depth, render_view, view_slot) in [
            (&left, &left_depth, stereo_views[0], LEFT_EYE_VIEW_SLOT),
            (&right, &right_depth, stereo_views[1], RIGHT_EYE_VIEW_SLOT),
        ] {
            active.render_with_options_in_slot(
                &queue,
                &mut encoder,
                ChunkRenderTarget::new(
                    &target.view,
                    &depth.view,
                    target.size,
                    fixture_clear_color(),
                ),
                render_view,
                render_options,
                view_slot,
            )?;
            preview.render_placed_prepared_stereo_draw_with_options_in_slot(
                &placed_renderer,
                &device,
                &prepared_stereo,
                &queue,
                &mut encoder,
                ChunkRenderTarget::new(
                    &target.view,
                    &depth.view,
                    target.size,
                    fixture_clear_color(),
                )
                .with_loaded_color()
                .with_loaded_depth(),
                render_view,
                render_options,
                context,
                view_slot,
            )?;
        }
        queue.submit(std::iter::once(encoder.finish()));
        let left_pixels = read_rgba8(&device, &queue, &left.texture, 640, 640)?;
        let right_pixels = read_rgba8(&device, &queue, &right.texture, 640, 640)?;
        let left_green = color_pixel_centroid_x(&left_pixels, 640, is_green);
        let right_green = color_pixel_centroid_x(&right_pixels, 640, is_green);
        assert!(left_green.1 > 100 && right_green.1 > 100);
        assert!(
            (left_green.0 - right_green.0).abs() > 0.5,
            "missing stereo parallax"
        );
        let eye_difference_count = left_pixels
            .chunks_exact(4)
            .zip(right_pixels.chunks_exact(4))
            .filter(|(left, right)| left != right)
            .count();
        assert!(eye_difference_count > 1_000);
        let stereo_pixels = stitch_rgba8_side_by_side(640, 640, &left_pixels, &right_pixels)?;
        save_rgba_png(
            Path::new("/tmp/mclone-live-diorama-slice2-stereo.png"),
            1280,
            640,
            &stereo_pixels,
        )?;

        if device.features().contains(wgpu::Features::MULTIVIEW) {
            preview.materialize_placed_multiview_renderer(&device, &placed_renderer)?;
            render_placed_fixture_multiview(
                &device,
                &queue,
                &preview,
                &placed_renderer,
                &prepared_stereo,
                stereo_views,
                render_options,
                context,
            )?;
        } else {
            eprintln!("placed terrain multiview device proof unavailable: adapter lacks MULTIVIEW");
        }

        eprintln!(
            "placed terrain proof: active-over-preview={active_over_preview} \
             preview-over-active={preview_over_active} eye-differences={eye_difference_count} \
             green-centroids=({:.2},{:.2})",
            left_green.0, right_green.0,
        );
        assert!(!placed_renderer.clipped_renderer_materialized());
        assert!(!placed_renderer.clipped_multiview_renderer_materialized());
        Ok(())
    }

    #[test]
    #[ignore = "GPU acceptance proof for Tactical 179 Slice 2"]
    fn complementary_half_space_terrain_renders_mono_stereo_and_multiview() -> Result<()> {
        use crate::composition_fixture::ComplementaryHalfSpaceTerrainFixture;

        const WIDTH: u32 = 960;
        const HEIGHT: u32 = 640;
        let (device, queue) = create_headless_device()?;
        let fixture = ComplementaryHalfSpaceTerrainFixture::new(&device, &queue, HEADLESS_FORMAT)?;
        assert!(fixture.shares_immutable_resources());
        assert!(!fixture.clipped_renderer_materialized());

        let mono_target = OffscreenTarget::new(&device, WIDTH, HEIGHT, HEADLESS_FORMAT);
        let mono_depth = ChunkDepthTarget::new(&device, WIDTH, HEIGHT);
        let mono_view = ComplementaryHalfSpaceTerrainFixture::render_view([WIDTH, HEIGHT], 0.0);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_half_space_fixture_mono_encoder"),
        });
        let report = fixture.render_mono(
            &device,
            &queue,
            &mut encoder,
            ChunkRenderTarget::new(
                &mono_target.view,
                &mono_depth.view,
                mono_target.size,
                fixture_clear_color(),
            ),
            mono_view,
            SINGLE_VIEW_SLOT,
        )?;
        queue.submit(std::iter::once(encoder.finish()));
        assert!(fixture.clipped_renderer_materialized());
        assert_eq!(report.left.drawn_section_count, 1);
        assert_eq!(report.right.drawn_section_count, 1);
        assert_eq!(report.left_translucent_section_count, 1);
        assert_eq!(report.right_translucent_section_count, 1);

        let mono_pixels = read_rgba8(&device, &queue, &mono_target.texture, WIDTH, HEIGHT)?;
        save_rgba_png(
            Path::new("/tmp/mclone-179-slice2-half-space-mono.png"),
            WIDTH,
            HEIGHT,
            &mono_pixels,
        )?;
        let mut orange_left = 0usize;
        let mut orange_right = 0usize;
        let mut blue_left = 0usize;
        let mut blue_right = 0usize;
        let mut open_seam_pixels = 0usize;
        for (index, pixel) in mono_pixels.chunks_exact(4).enumerate() {
            let x = index as u32 % WIDTH;
            let y = index as u32 / WIDTH;
            if is_fixture_orange(pixel) {
                if x + 8 < WIDTH / 2 {
                    orange_left += 1;
                } else if x > WIDTH / 2 + 8 {
                    orange_right += 1;
                }
            }
            if is_fixture_blue(pixel) {
                if x + 8 < WIDTH / 2 {
                    blue_left += 1;
                } else if x > WIDTH / 2 + 8 {
                    blue_right += 1;
                }
            }
            if x.abs_diff(WIDTH / 2) < 24
                && (HEIGHT / 3..HEIGHT * 2 / 3).contains(&y)
                && is_fixture_clear(pixel)
            {
                open_seam_pixels += 1;
            }
        }
        assert!(
            orange_left > 5_000,
            "missing left source pixels: {orange_left}"
        );
        assert!(
            blue_right > 5_000,
            "missing right source pixels: {blue_right}"
        );
        assert_eq!(orange_right, 0, "left source leaked right of clip plane");
        assert_eq!(blue_left, 0, "right source leaked left of clip plane");
        assert!(
            open_seam_pixels > 100,
            "authored open seam was not visible: {open_seam_pixels}"
        );
        let stereo_views = [
            ComplementaryHalfSpaceTerrainFixture::render_view([640, 640], -0.12),
            ComplementaryHalfSpaceTerrainFixture::render_view([640, 640], 0.12),
        ];
        let prepared = fixture.prepare_stereo(stereo_views);
        let selection = prepared.stats();
        assert_eq!(selection[0][0].drawn_section_count, 1);
        assert_eq!(selection[0][1].drawn_section_count, 1);
        assert_eq!(selection[1][0].drawn_section_count, 1);
        assert_eq!(selection[1][1].drawn_section_count, 1);
        let left_target = OffscreenTarget::new(&device, 640, 640, HEADLESS_FORMAT);
        let right_target = OffscreenTarget::new(&device, 640, 640, HEADLESS_FORMAT);
        let left_depth = ChunkDepthTarget::new(&device, 640, 640);
        let right_depth = ChunkDepthTarget::new(&device, 640, 640);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_half_space_fixture_stereo_encoder"),
        });
        for (target, depth, render_view, view_slot) in [
            (
                &left_target,
                &left_depth,
                stereo_views[0],
                LEFT_EYE_VIEW_SLOT,
            ),
            (
                &right_target,
                &right_depth,
                stereo_views[1],
                RIGHT_EYE_VIEW_SLOT,
            ),
        ] {
            fixture.render_prepared_stereo_eye(
                &prepared,
                &device,
                &queue,
                &mut encoder,
                ChunkRenderTarget::new(
                    &target.view,
                    &depth.view,
                    target.size,
                    fixture_clear_color(),
                ),
                render_view,
                view_slot,
            )?;
        }
        queue.submit(std::iter::once(encoder.finish()));
        let left_pixels = read_rgba8(&device, &queue, &left_target.texture, 640, 640)?;
        let right_pixels = read_rgba8(&device, &queue, &right_target.texture, 640, 640)?;
        let eye_differences = left_pixels
            .chunks_exact(4)
            .zip(right_pixels.chunks_exact(4))
            .filter(|(left, right)| left != right)
            .count();
        assert!(eye_differences > 1_000);
        let stereo_pixels = stitch_rgba8_side_by_side(640, 640, &left_pixels, &right_pixels)?;
        save_rgba_png(
            Path::new("/tmp/mclone-179-slice2-half-space-stereo.png"),
            1280,
            640,
            &stereo_pixels,
        )?;

        if device.features().contains(wgpu::Features::MULTIVIEW) {
            let color = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("mclone_half_space_fixture_multiview_color"),
                size: wgpu::Extent3d {
                    width: 640,
                    height: 640,
                    depth_or_array_layers: 2,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HEADLESS_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let color_view = color.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                array_layer_count: Some(2),
                ..Default::default()
            });
            let depth = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("mclone_half_space_fixture_multiview_depth"),
                size: wgpu::Extent3d {
                    width: 640,
                    height: 640,
                    depth_or_array_layers: 2,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: crate::chunk::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let depth_view = depth.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                array_layer_count: Some(2),
                ..Default::default()
            });
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_half_space_fixture_multiview_encoder"),
            });
            fixture.render_prepared_multiview(
                &prepared,
                &device,
                &queue,
                &mut encoder,
                crate::chunk::ChunkMultiviewRenderTarget::new(
                    &color_view,
                    &depth_view,
                    [640, 640],
                    fixture_clear_color(),
                ),
                stereo_views,
            )?;
            queue.submit(std::iter::once(encoder.finish()));
            assert!(fixture.clipped_multiview_renderer_materialized());
            let multiview_left = read_rgba8_layer(&device, &queue, &color, 640, 640, 0)?;
            let multiview_right = read_rgba8_layer(&device, &queue, &color, 640, 640, 1)?;
            assert_eq!(multiview_left, left_pixels);
            assert_eq!(multiview_right, right_pixels);
        } else {
            eprintln!("half-space multiview device proof unavailable: adapter lacks MULTIVIEW");
        }

        eprintln!(
            "half-space terrain proof: orange-left={orange_left} blue-right={blue_right} \
             open-seam={open_seam_pixels} eye-differences={eye_differences}"
        );
        Ok(())
    }

    #[test]
    #[ignore = "GPU acceptance proof for Tactical 179 Slice 4"]
    fn placed_actor_fixture_renders_mono_stereo_and_multiview() -> Result<()> {
        use crate::actor_composition_fixture::ActorCompositionFixture;
        use crate::composition_fixture::ComplementaryHalfSpaceTerrainFixture;

        const WIDTH: u32 = 960;
        const HEIGHT: u32 = 640;
        let (device, queue) = create_headless_device()?;
        let mut fixture = ActorCompositionFixture::new(&device, &queue, HEADLESS_FORMAT)?;
        assert!(fixture.shares_immutable_resources());

        let mono_target = OffscreenTarget::new(&device, WIDTH, HEIGHT, HEADLESS_FORMAT);
        let mono_depth = ChunkDepthTarget::new(&device, WIDTH, HEIGHT);
        let mono_view = ActorCompositionFixture::render_view([WIDTH, HEIGHT], 0.0);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_actor_composition_fixture_mono_encoder"),
        });
        let report = fixture.render_mono(
            &device,
            &queue,
            &mut encoder,
            ChunkRenderTarget::new(
                &mono_target.view,
                &mono_depth.view,
                mono_target.size,
                ActorCompositionFixture::clear_color(),
            ),
            mono_view,
            SINGLE_VIEW_SLOT,
        )?;
        queue.submit(std::iter::once(encoder.finish()));
        assert_eq!(report.terrain.left.drawn_section_count, 1);
        assert_eq!(report.terrain.right.drawn_section_count, 1);
        assert_eq!(report.unbounded.submitted_actor_count, 3);
        assert_eq!(report.unbounded.drawn_actor_count, 3);
        assert_eq!(report.left.submitted_actor_count, 6);
        assert_eq!(report.left.drawn_actor_count, 3);
        assert_eq!(report.left.source_rejected_actor_count, 1);
        assert_eq!(report.left.clip_rejected_actor_count, 1);
        assert_eq!(report.left.frustum_rejected_actor_count, 1);
        assert_eq!(report.right.submitted_actor_count, 3);
        assert_eq!(report.right.drawn_actor_count, 3);
        assert_eq!(report.left_resources.mesh.cached_actor_count, 3);
        assert_eq!(report.left_resources.mesh.rebuild_count, 1);
        assert_eq!(report.left_resources.mesh.upload_count, 1);
        assert_eq!(report.right_resources.mesh.cached_actor_count, 1);
        assert_eq!(report.right_resources.mesh.rebuild_count, 1);
        assert_eq!(report.right_resources.mesh.upload_count, 1);
        assert_eq!(report.right_resources.prepared_shared.figure_count, 2);
        assert_eq!(
            report
                .right_resources
                .prepared_shared
                .immutable_upload_count,
            6
        );
        assert_eq!(report.right_resources.prepared_world.actor_record_count, 2);
        assert_eq!(
            report.right_resources.prepared_world.prepared_actor_count,
            2
        );
        assert_eq!(report.right_resources.prepared_world.legacy_actor_count, 1);
        assert_eq!(
            report.right_resources.prepared_world.pose_evaluation_count,
            2
        );
        assert_eq!(report.right_resources.prepared_world.palette_write_count, 2);
        assert_eq!(report.right_resources.prepared_world.actor_write_count, 2);
        assert_eq!(report.right_resources.prepared_world.draw_count, 2);
        assert_eq!(report.left_resources.placed_pipeline_count, 1);
        assert_eq!(report.left_resources.clipped_placed_pipeline_count, 1);
        assert_eq!(report.left_resources.placed_multiview_pipeline_count, 0);

        let mono_pixels = read_rgba8(&device, &queue, &mono_target.texture, WIDTH, HEIGHT)?;
        save_rgba_png(
            Path::new("/tmp/mclone-179-slice4-actor-composition-mono.png"),
            WIDTH,
            HEIGHT,
            &mono_pixels,
        )?;
        let terrain_target = OffscreenTarget::new(&device, WIDTH, HEIGHT, HEADLESS_FORMAT);
        let terrain_depth = ChunkDepthTarget::new(&device, WIDTH, HEIGHT);
        let terrain = ComplementaryHalfSpaceTerrainFixture::new(&device, &queue, HEADLESS_FORMAT)?;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_actor_composition_terrain_control_encoder"),
        });
        terrain.render_mono(
            &device,
            &queue,
            &mut encoder,
            ChunkRenderTarget::new(
                &terrain_target.view,
                &terrain_depth.view,
                terrain_target.size,
                ActorCompositionFixture::clear_color(),
            ),
            mono_view,
            SINGLE_VIEW_SLOT,
        )?;
        queue.submit(std::iter::once(encoder.finish()));
        let terrain_pixels = read_rgba8(&device, &queue, &terrain_target.texture, WIDTH, HEIGHT)?;
        let actor_pixel_differences = mono_pixels
            .chunks_exact(4)
            .zip(terrain_pixels.chunks_exact(4))
            .filter(|(actors, terrain)| actors != terrain)
            .count();
        assert!(
            actor_pixel_differences > 1_000,
            "placed actors did not materially change pixels: {actor_pixel_differences}"
        );

        let stereo_views = [
            ActorCompositionFixture::render_view([640, 640], -0.12),
            ActorCompositionFixture::render_view([640, 640], 0.12),
        ];
        let prepared = fixture.prepare_stereo(stereo_views);
        let left_target = OffscreenTarget::new(&device, 640, 640, HEADLESS_FORMAT);
        let right_target = OffscreenTarget::new(&device, 640, 640, HEADLESS_FORMAT);
        let left_depth = ChunkDepthTarget::new(&device, 640, 640);
        let right_depth = ChunkDepthTarget::new(&device, 640, 640);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_actor_composition_fixture_stereo_encoder"),
        });
        let prepared_before_stereo = report.right_resources.prepared_world;
        let mut prepared_after_left = None;
        for (target, depth, render_view, view_slot) in [
            (
                &left_target,
                &left_depth,
                stereo_views[0],
                LEFT_EYE_VIEW_SLOT,
            ),
            (
                &right_target,
                &right_depth,
                stereo_views[1],
                RIGHT_EYE_VIEW_SLOT,
            ),
        ] {
            let report = fixture.render_stereo_eye(
                &prepared,
                &device,
                &queue,
                &mut encoder,
                ChunkRenderTarget::new(
                    &target.view,
                    &depth.view,
                    target.size,
                    ActorCompositionFixture::clear_color(),
                ),
                render_view,
                view_slot,
            )?;
            assert_eq!(report.left_resources.mesh.rebuild_count, 1);
            assert_eq!(report.left_resources.mesh.upload_count, 1);
            assert_eq!(report.right_resources.mesh.rebuild_count, 1);
            assert_eq!(report.right_resources.mesh.upload_count, 1);
            if view_slot == LEFT_EYE_VIEW_SLOT {
                prepared_after_left = Some(report.right_resources.prepared_world);
            } else {
                let after_left = prepared_after_left.expect("left eye rendered first");
                assert_eq!(
                    report.right_resources.prepared_world.pose_evaluation_count,
                    after_left.pose_evaluation_count
                );
                assert_eq!(
                    report.right_resources.prepared_world.palette_write_count,
                    after_left.palette_write_count
                );
                assert_eq!(
                    report.right_resources.prepared_world.actor_write_count,
                    after_left.actor_write_count
                );
                assert_eq!(
                    report.right_resources.prepared_world.draw_count,
                    after_left.draw_count + 2
                );
            }
        }
        let prepared_after_left = prepared_after_left.expect("left eye snapshot");
        assert_eq!(
            prepared_after_left.pose_evaluation_count,
            prepared_before_stereo.pose_evaluation_count + 2
        );
        assert_eq!(
            prepared_after_left.palette_write_count,
            prepared_before_stereo.palette_write_count + 2
        );
        queue.submit(std::iter::once(encoder.finish()));
        let left_pixels = read_rgba8(&device, &queue, &left_target.texture, 640, 640)?;
        let right_pixels = read_rgba8(&device, &queue, &right_target.texture, 640, 640)?;
        let eye_differences = left_pixels
            .chunks_exact(4)
            .zip(right_pixels.chunks_exact(4))
            .filter(|(left, right)| left != right)
            .count();
        assert!(eye_differences > 1_000);
        let stereo_pixels = stitch_rgba8_side_by_side(640, 640, &left_pixels, &right_pixels)?;
        save_rgba_png(
            Path::new("/tmp/mclone-179-slice4-actor-composition-stereo.png"),
            1280,
            640,
            &stereo_pixels,
        )?;

        if device.features().contains(wgpu::Features::MULTIVIEW) {
            let color = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("mclone_actor_composition_fixture_multiview_color"),
                size: wgpu::Extent3d {
                    width: 640,
                    height: 640,
                    depth_or_array_layers: 2,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HEADLESS_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let color_view = color.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                array_layer_count: Some(2),
                ..Default::default()
            });
            let depth = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("mclone_actor_composition_fixture_multiview_depth"),
                size: wgpu::Extent3d {
                    width: 640,
                    height: 640,
                    depth_or_array_layers: 2,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: crate::chunk::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let depth_view = depth.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                array_layer_count: Some(2),
                ..Default::default()
            });
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_actor_composition_fixture_multiview_encoder"),
            });
            let report = fixture.render_multiview(
                &prepared,
                &device,
                &queue,
                &mut encoder,
                crate::chunk::ChunkMultiviewRenderTarget::new(
                    &color_view,
                    &depth_view,
                    [640, 640],
                    ActorCompositionFixture::clear_color(),
                ),
                stereo_views,
            )?;
            queue.submit(std::iter::once(encoder.finish()));
            assert_eq!(report.left_resources.mesh.rebuild_count, 1);
            assert_eq!(report.left_resources.mesh.upload_count, 1);
            assert_eq!(report.left_resources.placed_multiview_pipeline_count, 1);
            assert_eq!(
                report
                    .left_resources
                    .clipped_placed_multiview_pipeline_count,
                1
            );
            let multiview_left = read_rgba8_layer(&device, &queue, &color, 640, 640, 0)?;
            let multiview_right = read_rgba8_layer(&device, &queue, &color, 640, 640, 1)?;
            assert_eq!(multiview_left, left_pixels);
            assert_eq!(multiview_right, right_pixels);
        } else {
            eprintln!("placed actor multiview proof unavailable: adapter lacks MULTIVIEW");
        }

        eprintln!(
            "placed actor proof: actor-pixel-differences={actor_pixel_differences} \
             eye-differences={eye_differences} left={}/{} right={}/{}",
            report.left.drawn_actor_count,
            report.left.submitted_actor_count,
            report.right.drawn_actor_count,
            report.right.submitted_actor_count,
        );
        Ok(())
    }

    #[test]
    #[ignore = "GPU validation proof for 107 Slice E; run on hosts with wgpu MULTIVIEW support"]
    fn multiview_renders_distinct_view_index_layers() -> Result<()> {
        const SHADER: &str = r#"
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) view_index: i32,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(view_index) view_index: i32,
) -> VertexOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VertexOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.view_index = view_index;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    if (in.view_index == 0i) {
        return vec4<f32>(1.0, 0.0, 0.0, 1.0);
    }
    return vec4<f32>(0.0, 1.0, 0.0, 1.0);
}
"#;
        let (device, queue) = create_headless_device()?;
        if !device.features().contains(wgpu::Features::MULTIVIEW) {
            eprintln!("skipping multiview proof: headless adapter does not expose wgpu MULTIVIEW");
            return Ok(());
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_test_multiview_target"),
            size: wgpu::Extent3d {
                width: 8,
                height: 8,
                depth_or_array_layers: 2,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HEADLESS_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("mclone_test_multiview_target_view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            base_array_layer: 0,
            array_layer_count: Some(2),
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_test_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_test_multiview_pipeline_layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_test_multiview_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HEADLESS_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: std::num::NonZeroU32::new(2),
            cache: None,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_test_multiview_encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mclone_test_multiview_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&pipeline);
            pass.draw(0..3, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));

        let left_pixels = read_rgba8_layer(&device, &queue, &texture, 8, 8, 0)?;
        let right_pixels = read_rgba8_layer(&device, &queue, &texture, 8, 8, 1)?;
        assert_eq!(left_pixels.get(0..4), Some(&[255, 0, 0, 255][..]));
        assert_eq!(right_pixels.get(0..4), Some(&[0, 255, 0, 255][..]));
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn render_placed_fixture_view(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &OffscreenTarget,
        depth: &ChunkDepthTarget,
        active: &TexturedSectionDrawResources,
        preview: &TexturedSectionDrawResources,
        placed_renderer: &crate::chunk::PlacedTexturedSectionRenderer,
        preview_records: &crate::chunk::PreparedTexturedSectionRecords,
        render_view: crate::chunk::ChunkRenderView,
        render_options: TexturedSectionRenderOptions,
        context: WorldCompositionContext,
        view_slot: crate::uniform::PerViewSlot,
        draw_active: bool,
        draw_preview: bool,
    ) -> Result<crate::chunk::TexturedSectionRenderStats> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_placed_fixture_view_encoder"),
        });
        if draw_active {
            active.render_with_options_in_slot(
                queue,
                &mut encoder,
                ChunkRenderTarget::new(
                    &target.view,
                    &depth.view,
                    target.size,
                    fixture_clear_color(),
                ),
                render_view,
                render_options,
                view_slot,
            )?;
        }
        let stats = if draw_preview {
            let mut render_target = ChunkRenderTarget::new(
                &target.view,
                &depth.view,
                target.size,
                fixture_clear_color(),
            );
            if draw_active {
                render_target = render_target.with_loaded_color().with_loaded_depth();
            }
            preview.render_placed_prepared_with_options_in_slot(
                placed_renderer,
                device,
                preview_records,
                queue,
                &mut encoder,
                render_target,
                render_view,
                render_options,
                context,
                view_slot,
            )?
        } else {
            crate::chunk::TexturedSectionRenderStats::default()
        };
        queue.submit(std::iter::once(encoder.finish()));
        Ok(stats)
    }

    #[allow(clippy::too_many_arguments)]
    fn render_placed_fixture_multiview(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        preview: &TexturedSectionDrawResources,
        placed_renderer: &crate::chunk::PlacedTexturedSectionRenderer,
        prepared_stereo: &crate::chunk::PreparedTexturedSectionStereoDraw,
        render_views: [crate::chunk::ChunkRenderView; 2],
        render_options: TexturedSectionRenderOptions,
        context: WorldCompositionContext,
    ) -> Result<()> {
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_placed_fixture_multiview_color"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 2,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HEADLESS_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor {
            label: Some("mclone_placed_fixture_multiview_color_view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(2),
            ..Default::default()
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_placed_fixture_multiview_depth"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 2,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: crate::chunk::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor {
            label: Some("mclone_placed_fixture_multiview_depth_view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(2),
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_placed_fixture_multiview_encoder"),
        });
        preview.render_placed_prepared_multiview_stereo_draw_with_options(
            placed_renderer,
            prepared_stereo,
            device,
            queue,
            &mut encoder,
            crate::chunk::ChunkMultiviewRenderTarget::new(
                &color_view,
                &depth_view,
                [64, 64],
                fixture_clear_color(),
            ),
            render_views,
            [render_options; 2],
            context,
        )?;
        queue.submit(std::iter::once(encoder.finish()));
        let left = read_rgba8_layer(device, queue, &color, 64, 64, 0)?;
        let right = read_rgba8_layer(device, queue, &color, 64, 64, 1)?;
        assert!(left.chunks_exact(4).any(is_green));
        assert!(right.chunks_exact(4).any(is_green));
        Ok(())
    }

    fn fixture_camera(eye_offset: f32) -> ChunkCamera {
        ChunkCamera {
            eye: [eye_offset, 5.0, 12.0],
            target: [eye_offset, 1.25, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 58.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 100.0,
        }
    }

    fn fixture_active_section() -> TexturedRenderSectionMesh {
        let mut mesh = TexturedVisibleChunkMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            solid_index_count: 0,
            opaque_index_count: 0,
        };
        append_fixture_cube(
            &mut mesh,
            Vec3::new(-4.0, 0.0, -3.0),
            Vec3::new(4.0, 0.5, 3.0),
            [0.65, 0.25, 0.06, 1.0],
        );
        append_fixture_cube(
            &mut mesh,
            Vec3::new(-5.0, 0.0, -5.0),
            Vec3::new(5.0, 6.0, -4.5),
            [0.08, 0.18, 0.85, 1.0],
        );
        mesh.solid_index_count = mesh.indices.len() as u32;
        mesh.opaque_index_count = mesh.solid_index_count;
        TexturedRenderSectionMesh {
            key: RenderSectionKey::new(0, 0, 0),
            mesh,
            grass_patches: Vec::new(),
            visibility: VisibilitySet::all_visible(),
        }
    }

    fn fixture_preview_section() -> TexturedRenderSectionMesh {
        let mut mesh = TexturedVisibleChunkMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            solid_index_count: 0,
            opaque_index_count: 0,
        };
        append_fixture_cube(
            &mut mesh,
            Vec3::new(0.0, -4.0, 0.0),
            Vec3::new(16.0, 12.0, 16.0),
            [0.06, 0.85, 0.12, 1.0],
        );
        mesh.solid_index_count = mesh.indices.len() as u32;
        mesh.opaque_index_count = mesh.solid_index_count;
        TexturedRenderSectionMesh {
            key: RenderSectionKey::new(0, 0, 0),
            mesh,
            grass_patches: Vec::new(),
            visibility: VisibilitySet::all_visible(),
        }
    }

    fn append_fixture_cube(
        mesh: &mut TexturedVisibleChunkMesh,
        min: Vec3,
        max: Vec3,
        color: [f32; 4],
    ) {
        let faces = [
            [
                [min.x, min.y, max.z],
                [max.x, min.y, max.z],
                [max.x, max.y, max.z],
                [min.x, max.y, max.z],
            ],
            [
                [max.x, min.y, min.z],
                [min.x, min.y, min.z],
                [min.x, max.y, min.z],
                [max.x, max.y, min.z],
            ],
            [
                [max.x, min.y, max.z],
                [max.x, min.y, min.z],
                [max.x, max.y, min.z],
                [max.x, max.y, max.z],
            ],
            [
                [min.x, min.y, min.z],
                [min.x, min.y, max.z],
                [min.x, max.y, max.z],
                [min.x, max.y, min.z],
            ],
            [
                [min.x, max.y, max.z],
                [max.x, max.y, max.z],
                [max.x, max.y, min.z],
                [min.x, max.y, min.z],
            ],
            [
                [min.x, min.y, min.z],
                [max.x, min.y, min.z],
                [max.x, min.y, max.z],
                [min.x, min.y, max.z],
            ],
        ];
        for face in faces {
            let base = mesh.vertices.len() as u32;
            mesh.vertices
                .extend(face.map(|position| TexturedChunkVertex {
                    position,
                    uv: [0.5, 0.5],
                    color,
                    packed_light: 15_728_880,
                }));
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    fn fixture_clear_color() -> wgpu::Color {
        wgpu::Color {
            r: 0.02,
            g: 0.025,
            b: 0.03,
            a: 1.0,
        }
    }

    fn pixel_transition_count(
        active: &[u8],
        preview: &[u8],
        composed: &[u8],
        active_predicate: fn(&[u8]) -> bool,
        preview_predicate: fn(&[u8]) -> bool,
        composed_predicate: fn(&[u8]) -> bool,
    ) -> usize {
        active
            .chunks_exact(4)
            .zip(preview.chunks_exact(4))
            .zip(composed.chunks_exact(4))
            .filter(|((active, preview), composed)| {
                active_predicate(active)
                    && preview_predicate(preview)
                    && composed_predicate(composed)
            })
            .count()
    }

    fn color_pixel_centroid_x(
        pixels: &[u8],
        width: u32,
        predicate: fn(&[u8]) -> bool,
    ) -> (f64, usize) {
        let mut x_sum = 0_u64;
        let mut count = 0_usize;
        for (index, pixel) in pixels.chunks_exact(4).enumerate() {
            if predicate(pixel) {
                x_sum += index as u64 % u64::from(width);
                count += 1;
            }
        }
        let centroid = if count == 0 {
            0.0
        } else {
            x_sum as f64 / count as f64
        };
        (centroid, count)
    }

    fn is_brown(pixel: &[u8]) -> bool {
        pixel[0] > 100 && pixel[0] > pixel[1] * 2 && pixel[1] > pixel[2]
    }

    fn is_blue(pixel: &[u8]) -> bool {
        pixel[2] > 100 && pixel[2] > pixel[0] * 2 && pixel[2] > pixel[1] * 2
    }

    fn is_green(pixel: &[u8]) -> bool {
        pixel[1] > 100 && pixel[1] > pixel[0] * 2 && pixel[1] > pixel[2] * 2
    }

    fn is_fixture_orange(pixel: &[u8]) -> bool {
        u16::from(pixel[0]) > 130
            && u16::from(pixel[0]) > u16::from(pixel[1]) * 2
            && u16::from(pixel[0]) > u16::from(pixel[2]) * 2
    }

    fn is_fixture_blue(pixel: &[u8]) -> bool {
        u16::from(pixel[2]) > 130
            && u16::from(pixel[2]) > u16::from(pixel[0]) * 2
            && pixel[2] > pixel[1]
    }

    fn is_fixture_clear(pixel: &[u8]) -> bool {
        pixel[0] < 20 && pixel[1] < 20 && pixel[2] < 20
    }

    fn color_bytes(color: [f32; 4]) -> [u8; 16] {
        let mut bytes = [0_u8; 16];
        for (index, value) in color.into_iter().enumerate() {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }
}
