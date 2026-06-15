use std::sync::Arc;

use anyhow::{Context, Result, bail};
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::gpu_util::{native_backends, optional_gpu_features};
use crate::target::{RenderFrameContext, RenderFrameTarget};

const INITIAL_PRESENT_MODE: wgpu::PresentMode = wgpu::PresentMode::Fifo;
const SURFACE_FORMAT_OVERRIDE_ENV: &str = "MCLONE_FORCE_SURFACE_FORMAT";

pub struct NativeSurfaceContext {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
}

impl NativeSurfaceContext {
    pub fn new(window: Arc<Window>) -> Result<Self> {
        let size = window.inner_size();
        let backends = native_backends();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });
        let surface = instance
            .create_surface(window.clone())
            .context("failed to create wgpu surface")?;
        let (adapter, device, queue) = pollster::block_on(async {
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    force_fallback_adapter: false,
                })
                .await
                .context("no suitable wgpu adapter")?;
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("mclone_native_device"),
                    required_features: optional_gpu_features(adapter.features()),
                    required_limits: wgpu::Limits::default(),
                    ..Default::default()
                })
                .await
                .context("failed to create wgpu device")?;
            Result::<_>::Ok((adapter, device, queue))
        })?;

        let caps = surface.get_capabilities(&adapter);
        let format = selected_surface_format(&caps);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: INITIAL_PRESENT_MODE,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let info = adapter.get_info();
        log::info!(
            "wgpu adapter '{}' backend={:?} driver={} driver_info={} requested_backends={backends:?} features={:?}",
            info.name,
            info.backend,
            info.driver,
            info.driver_info,
            device.features()
        );
        log::info!(
            "wgpu surface format={format:?} alpha_mode={:?} initial_present_mode={:?} present_modes={:?}",
            config.alpha_mode,
            config.present_mode,
            caps.present_modes
        );

        Ok(Self {
            surface,
            device,
            queue,
            config,
        })
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.config.width = size.width.max(1);
        self.config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render_clear(&mut self, color: wgpu::Color) -> Result<SurfaceFrameStatus> {
        self.render_with(|frame| {
            let _pass = frame
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mclone_native_clear_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: frame.target.color_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(color),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
            Ok(())
        })
    }

    pub fn render_with<F>(&mut self, encode: F) -> Result<SurfaceFrameStatus>
    where
        F: FnOnce(RenderFrameContext<'_>) -> Result<()>,
    {
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(SurfaceFrameStatus::Reconfigured);
            }
            Err(wgpu::SurfaceError::OutOfMemory) => bail!("wgpu surface out of memory"),
            Err(err) => {
                log::warn!("surface acquire failed: {err:?}");
                return Ok(SurfaceFrameStatus::Skipped);
            }
        };

        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_native_frame_encoder"),
            });
        let target =
            RenderFrameTarget::color(&view, [self.config.width.max(1), self.config.height.max(1)]);
        encode(RenderFrameContext::new(
            &self.device,
            &self.queue,
            &mut encoder,
            target,
        ))?;
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(SurfaceFrameStatus::Presented)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceFrameStatus {
    Presented,
    Reconfigured,
    Skipped,
}

fn preferred_surface_format(caps: &wgpu::SurfaceCapabilities) -> wgpu::TextureFormat {
    [
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ]
    .into_iter()
    .find(|format| caps.formats.contains(format))
    .or_else(|| caps.formats.iter().copied().find(|format| format.is_srgb()))
    .or_else(|| {
        [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
        ]
        .into_iter()
        .find(|format| caps.formats.contains(format))
    })
    .unwrap_or(caps.formats[0])
}

fn selected_surface_format(caps: &wgpu::SurfaceCapabilities) -> wgpu::TextureFormat {
    selected_surface_format_with_override(caps, surface_format_override())
}

fn selected_surface_format_with_override(
    caps: &wgpu::SurfaceCapabilities,
    requested: Option<wgpu::TextureFormat>,
) -> wgpu::TextureFormat {
    if let Some(format) = requested {
        if caps.formats.contains(&format) {
            log::info!(
                "using surface format override {format:?} from {SURFACE_FORMAT_OVERRIDE_ENV}"
            );
            return format;
        }
        log::warn!(
            "ignoring {SURFACE_FORMAT_OVERRIDE_ENV}={format:?}; adapter surface formats are {:?}",
            caps.formats
        );
    }
    preferred_surface_format(caps)
}

fn surface_format_override() -> Option<wgpu::TextureFormat> {
    let Ok(value) = std::env::var(SURFACE_FORMAT_OVERRIDE_ENV) else {
        return None;
    };
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
        return None;
    }
    let parsed = parse_surface_format_override(trimmed);
    if parsed.is_none() {
        log::warn!("ignoring unsupported {SURFACE_FORMAT_OVERRIDE_ENV}={trimmed:?}");
    }
    parsed
}

fn parse_surface_format_override(value: &str) -> Option<wgpu::TextureFormat> {
    let normalized = value
        .trim()
        .to_ascii_lowercase()
        .replace('-', "")
        .replace('_', "");
    match normalized.as_str() {
        "bgra8unorm" => Some(wgpu::TextureFormat::Bgra8Unorm),
        "rgba8unorm" => Some(wgpu::TextureFormat::Rgba8Unorm),
        "bgra8unormsrgb" => Some(wgpu::TextureFormat::Bgra8UnormSrgb),
        "rgba8unormsrgb" => Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(formats: Vec<wgpu::TextureFormat>) -> wgpu::SurfaceCapabilities {
        wgpu::SurfaceCapabilities {
            formats,
            present_modes: vec![wgpu::PresentMode::Fifo],
            alpha_modes: vec![wgpu::CompositeAlphaMode::Auto],
            usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
        }
    }

    #[test]
    fn surface_format_prefers_srgb() {
        let caps = caps(vec![
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        ]);

        assert_eq!(
            selected_surface_format_with_override(&caps, None),
            wgpu::TextureFormat::Bgra8UnormSrgb
        );
    }

    #[test]
    fn valid_surface_format_override_wins() {
        let caps = caps(vec![
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        ]);

        assert_eq!(
            selected_surface_format_with_override(&caps, Some(wgpu::TextureFormat::Rgba8Unorm)),
            wgpu::TextureFormat::Rgba8Unorm
        );
    }

    #[test]
    fn unsupported_surface_format_override_is_ignored() {
        let caps = caps(vec![wgpu::TextureFormat::Rgba8Unorm]);

        assert_eq!(
            selected_surface_format_with_override(&caps, Some(wgpu::TextureFormat::Bgra8UnormSrgb)),
            wgpu::TextureFormat::Rgba8Unorm
        );
    }
}
