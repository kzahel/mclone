use std::mem::size_of;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use image::{ColorType, ImageFormat};

use crate::options::ExplorerOptions;
use crate::smoke::{SmokeCheckpoint, SmokeFrameOutcome, SmokeRecorder, SmokeSequence};
use crate::terrain::ExplorerTerrain;

const BYTES_PER_PIXEL: u32 = 4;
const COPY_ROW_ALIGNMENT: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(15);
const SMOKE_TIMEOUT: Duration = Duration::from_secs(30);

pub fn run_capture(options: &ExplorerOptions, path: &Path, started: Instant) -> Result<()> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .context("no suitable World Explorer offscreen WGPU adapter")?;
    let adapter_info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("mclone_world_explorer_offscreen_device"),
        required_features: wgpu::Features::empty(),
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .context("failed to create World Explorer offscreen WGPU device")?;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mclone_world_explorer_offscreen_color"),
        size: wgpu::Extent3d {
            width: options.width,
            height: options.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut terrain = ExplorerTerrain::new(&device, &queue, format, options.clone(), started)?;

    let mut frame_count = 0_u64;
    loop {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_world_explorer_offscreen_frame"),
        });
        let stats = terrain.encode(&device, &queue, &mut encoder, &view)?;
        queue.submit(std::iter::once(encoder.finish()));
        terrain.poll_completed(&device)?;
        frame_count = frame_count.saturating_add(1);
        if stats.target_ready && !stats.needs_redraw {
            break;
        }
        if started.elapsed() >= CAPTURE_TIMEOUT {
            bail!(
                "World Explorer offscreen target did not become ready within {:.1} seconds: {}",
                CAPTURE_TIMEOUT.as_secs_f64(),
                terrain.diagnostics()
            );
        }
    }

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_world_explorer_capture_final_frame"),
    });
    terrain.set_depth_capture_enabled(true);
    terrain.encode(&device, &queue, &mut encoder, &view)?;
    let color_readback = TextureReadback::encode(
        &device,
        &mut encoder,
        &texture,
        options.width,
        options.height,
        format,
    )?;
    let depth_readback = DepthReadback::encode(
        &device,
        &mut encoder,
        &terrain,
        options.width,
        options.height,
    )?;
    queue.submit(std::iter::once(encoder.finish()));
    let pixels = color_readback.finish(&device)?;
    let depth_stats = depth_readback.finish_and_validate(
        &device,
        terrain.view_state().mode == mclone_view_control::WorldViewMode::Orbit,
    )?;
    let pixel_stats = save_validated_png(path, &pixels, options.width, options.height)?;
    log::info!(
        "World Explorer capture={} frames={} adapter={:?} backend={:?} \
         device_type={:?} asset_profile={} asset_bytes={} rgb_range={}..{} \
         opaque_pixels={} depth_range={:.6}..{:.6} depth_covered={} {}",
        path.display(),
        frame_count,
        adapter_info.name,
        adapter_info.backend,
        adapter_info.device_type,
        options.asset_profile.label(),
        options.asset_bytes()?,
        pixel_stats.min_rgb,
        pixel_stats.max_rgb,
        pixel_stats.opaque_pixels,
        depth_stats.min_depth,
        depth_stats.max_depth,
        depth_stats.covered_pixels,
        terrain.diagnostics(),
    );
    Ok(())
}

pub fn run_smoke(options: &ExplorerOptions, root: &Path, started: Instant) -> Result<()> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .context("no suitable World Explorer smoke WGPU adapter")?;
    let adapter_info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("mclone_world_explorer_smoke_device"),
        required_features: wgpu::Features::empty(),
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .context("failed to create World Explorer smoke WGPU device")?;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mclone_world_explorer_smoke_color"),
        size: wgpu::Extent3d {
            width: options.width,
            height: options.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut terrain = ExplorerTerrain::new(&device, &queue, format, options.clone(), started)?;
    terrain.set_depth_capture_enabled(true);
    let viewport = mclone_view_control::ViewportMetrics::new(
        f64::from(options.width),
        f64::from(options.height),
    );
    let mut sequence = SmokeSequence::default();
    let mut recorder = SmokeRecorder::new("offscreen", root, &adapter_info, format)?;

    while !sequence.is_complete() {
        let frame_started = Instant::now();
        let input_applied = sequence.prepare_frame(&mut terrain, viewport)?;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_world_explorer_smoke_frame"),
        });
        let stats = terrain.encode(&device, &queue, &mut encoder, &view)?;
        let outcome = sequence.after_frame(stats);
        let captures = match outcome {
            SmokeFrameOutcome::Capture { label, .. } => Some((
                label,
                TextureReadback::encode(
                    &device,
                    &mut encoder,
                    &texture,
                    options.width,
                    options.height,
                    format,
                )?,
                DepthReadback::encode(
                    &device,
                    &mut encoder,
                    &terrain,
                    options.width,
                    options.height,
                )?,
            )),
            _ => None,
        };
        queue.submit(std::iter::once(encoder.finish()));
        terrain.poll_completed(&device)?;
        let frame_time = frame_started.elapsed();
        recorder.note_frame(frame_time, input_applied, stats);

        if let Some((label, color, depth)) = captures {
            let pixels = color.finish(&device)?;
            let depth = depth.finish_and_validate(
                &device,
                terrain.view_state().mode == mclone_view_control::WorldViewMode::Orbit,
            )?;
            let path = recorder.checkpoint_path(label);
            let pixel_stats = save_validated_png(&path, &pixels, options.width, options.height)?;
            recorder.note_checkpoint(
                label,
                frame_time,
                &path,
                SmokeCheckpoint {
                    state: terrain.view_state(),
                    stats,
                    pixels: pixel_stats,
                    depth,
                },
            )?;
        }
        if matches!(
            outcome,
            SmokeFrameOutcome::Complete
                | SmokeFrameOutcome::Capture {
                    complete_after_capture: true,
                    ..
                }
        ) {
            break;
        }
        if started.elapsed() >= SMOKE_TIMEOUT {
            bail!(
                "World Explorer offscreen smoke exceeded {:.1} seconds: {}",
                SMOKE_TIMEOUT.as_secs_f64(),
                terrain.diagnostics()
            );
        }
    }
    let receipt = recorder.write(&terrain, options)?;
    log::info!(
        "World Explorer offscreen smoke receipt={} {}",
        receipt.display(),
        terrain.diagnostics()
    );
    Ok(())
}

pub(crate) struct PixelStats {
    pub min_rgb: u8,
    pub max_rgb: u8,
    pub opaque_pixels: u64,
}

fn validate_pixels(pixels: &[u8]) -> Result<PixelStats> {
    if pixels.is_empty() || !pixels.len().is_multiple_of(BYTES_PER_PIXEL as usize) {
        bail!("World Explorer capture returned invalid RGBA byte length");
    }
    let mut min_rgb = u8::MAX;
    let mut max_rgb = u8::MIN;
    let mut opaque_pixels = 0_u64;
    for pixel in pixels.chunks_exact(BYTES_PER_PIXEL as usize) {
        for channel in &pixel[..3] {
            min_rgb = min_rgb.min(*channel);
            max_rgb = max_rgb.max(*channel);
        }
        if pixel[3] == u8::MAX {
            opaque_pixels = opaque_pixels.saturating_add(1);
        }
    }
    if max_rgb.saturating_sub(min_rgb) < 16 {
        bail!("World Explorer capture has insufficient color range: {min_rgb}..{max_rgb}");
    }
    if opaque_pixels == 0 {
        bail!("World Explorer capture contains no opaque pixels");
    }
    Ok(PixelStats {
        min_rgb,
        max_rgb,
        opaque_pixels,
    })
}

pub(crate) struct TextureReadback {
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    padded_row_bytes: u32,
    format: wgpu::TextureFormat,
}

pub(crate) struct DepthReadback {
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    padded_row_bytes: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DepthStats {
    pub min_depth: f32,
    pub max_depth: f32,
    pub covered_pixels: u64,
    pub clear_pixels: u64,
}

impl DepthReadback {
    pub(crate) fn encode(
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        terrain: &ExplorerTerrain,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let unpadded_row_bytes = width
            .checked_mul(size_of::<f32>() as u32)
            .context("World Explorer depth row byte length overflow")?;
        let padded_row_bytes = unpadded_row_bytes.div_ceil(COPY_ROW_ALIGNMENT) * COPY_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_world_explorer_depth_readback"),
            size: u64::from(padded_row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        terrain.copy_depth_to_buffer(encoder, &buffer, padded_row_bytes)?;
        Ok(Self {
            buffer,
            width,
            height,
            padded_row_bytes,
        })
    }

    pub(crate) fn finish_and_validate(
        self,
        device: &wgpu::Device,
        require_variation: bool,
    ) -> Result<DepthStats> {
        let slice = self.buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device
            .poll(wgpu::PollType::Wait)
            .context("World Explorer depth device poll failed")?;
        receiver
            .recv()
            .context("World Explorer depth callback disconnected")?
            .context("World Explorer depth readback mapping failed")?;

        let mapped = slice.get_mapped_range();
        let row_bytes = self.width as usize * size_of::<f32>();
        let mut min_depth = f32::INFINITY;
        let mut max_depth = f32::NEG_INFINITY;
        let mut covered_pixels = 0_u64;
        let mut clear_pixels = 0_u64;
        for row in 0..self.height {
            let start = row as usize * self.padded_row_bytes as usize;
            for bytes in mapped[start..start + row_bytes].chunks_exact(size_of::<f32>()) {
                let depth =
                    f32::from_ne_bytes(bytes.try_into().expect("depth chunk is four bytes"));
                if !depth.is_finite() || !(0.0..=1.0).contains(&depth) {
                    bail!("World Explorer capture contains invalid depth value {depth}");
                }
                min_depth = min_depth.min(depth);
                max_depth = max_depth.max(depth);
                if depth > 1.0e-6 {
                    covered_pixels = covered_pixels.saturating_add(1);
                } else {
                    clear_pixels = clear_pixels.saturating_add(1);
                }
            }
        }
        drop(mapped);
        self.buffer.unmap();
        if covered_pixels == 0 || (require_variation && max_depth - min_depth < 1.0e-5) {
            bail!(
                "World Explorer capture lacks meaningful depth: range \
                 {min_depth:.6}..{max_depth:.6}, covered={covered_pixels}"
            );
        }
        Ok(DepthStats {
            min_depth,
            max_depth,
            covered_pixels,
            clear_pixels,
        })
    }
}

impl TextureReadback {
    pub(crate) fn encode(
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        if !matches!(
            format,
            wgpu::TextureFormat::Rgba8Unorm
                | wgpu::TextureFormat::Rgba8UnormSrgb
                | wgpu::TextureFormat::Bgra8Unorm
                | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            bail!("unsupported World Explorer capture format {format:?}");
        }
        let unpadded_row_bytes = width * BYTES_PER_PIXEL;
        let padded_row_bytes = unpadded_row_bytes.div_ceil(COPY_ROW_ALIGNMENT) * COPY_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_world_explorer_capture_readback"),
            size: u64::from(padded_row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
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
        Ok(Self {
            buffer,
            width,
            height,
            padded_row_bytes,
            format,
        })
    }

    pub(crate) fn finish(self, device: &wgpu::Device) -> Result<Vec<u8>> {
        let slice = self.buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device
            .poll(wgpu::PollType::Wait)
            .context("World Explorer capture device poll failed")?;
        receiver
            .recv()
            .context("World Explorer capture callback disconnected")?
            .context("World Explorer capture readback mapping failed")?;

        let mapped = slice.get_mapped_range();
        let unpadded_row_bytes = self.width * BYTES_PER_PIXEL;
        let mut pixels = Vec::with_capacity((self.width * self.height * BYTES_PER_PIXEL) as usize);
        for row in 0..self.height {
            let start = (row * self.padded_row_bytes) as usize;
            pixels.extend_from_slice(&mapped[start..start + unpadded_row_bytes as usize]);
        }
        drop(mapped);
        self.buffer.unmap();
        if matches!(
            self.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for pixel in pixels.chunks_exact_mut(BYTES_PER_PIXEL as usize) {
                pixel.swap(0, 2);
            }
        }
        Ok(pixels)
    }
}

pub(crate) fn save_validated_png(
    path: &Path,
    pixels: &[u8],
    width: u32,
    height: u32,
) -> Result<PixelStats> {
    let stats = validate_pixels(pixels)?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create capture directory {}", parent.display()))?;
    }
    image::save_buffer_with_format(
        path,
        pixels,
        width,
        height,
        ColorType::Rgba8,
        ImageFormat::Png,
    )
    .with_context(|| format!("failed to write World Explorer capture {}", path.display()))?;
    Ok(stats)
}
