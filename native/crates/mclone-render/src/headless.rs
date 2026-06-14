use std::path::{Path, PathBuf};
use std::sync::mpsc;

use anyhow::{Context, Result};

use crate::gpu_util::{native_backends, optional_gpu_features};

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

pub fn write_headless_clear_png(options: HeadlessClearOptions) -> Result<HeadlessClearReport> {
    let width = options.width.max(1);
    let height = options.height.max(1);
    let (device, queue) = create_headless_device()?;
    let target = OffscreenTarget::new(&device, width, height, HEADLESS_FORMAT);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_headless_clear_encoder"),
    });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_headless_clear_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.view,
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
        Self { texture, view }
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
