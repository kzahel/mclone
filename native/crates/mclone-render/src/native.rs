use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::color_profile::{RenderColorProfile, RenderConfig};
use crate::gpu_util::{native_backends, optional_gpu_features};
use crate::target::{RenderFrameContext, RenderFrameTarget};

const INITIAL_PRESENT_MODE: wgpu::PresentMode = wgpu::PresentMode::Fifo;
const SURFACE_FORMAT_OVERRIDE_ENV: &str = "MCLONE_FORCE_SURFACE_FORMAT";

pub struct NativeSurfaceContext {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub render_config: RenderConfig,
    supported_present_modes: Vec<wgpu::PresentMode>,
}

impl NativeSurfaceContext {
    pub fn new(window: Arc<Window>) -> Result<Self> {
        Self::new_with_color_profile(window, RenderColorProfile::default())
    }

    pub fn new_with_color_profile(
        window: Arc<Window>,
        color_profile: RenderColorProfile,
    ) -> Result<Self> {
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
        let format = selected_surface_format(&caps, color_profile);
        let render_config = RenderConfig::for_color_target(color_profile, format);
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
            "wgpu surface format={format:?} color_profile={} alpha_mode={:?} initial_present_mode={:?} present_modes={:?}",
            render_config.color_profile.as_str(),
            config.alpha_mode,
            config.present_mode,
            caps.present_modes
        );
        if format.is_srgb() {
            log::warn!(
                "wgpu surface format {format:?} is sRGB; renderer color profile {} will use a shader presentation transform where available",
                render_config.color_profile.as_str()
            );
        }
        log::info!("wgpu render config: {}", render_config.diagnostic_label());

        Ok(Self {
            surface,
            device,
            queue,
            config,
            render_config,
            supported_present_modes: caps.present_modes,
        })
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.config.width = size.width.max(1);
        self.config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    pub fn set_present_mode_preference(
        &mut self,
        preference: SurfacePresentModePreference,
    ) -> wgpu::PresentMode {
        self.set_present_mode(preference.requested_present_mode())
    }

    pub fn present_mode(&self) -> wgpu::PresentMode {
        self.config.present_mode
    }

    fn set_present_mode(&mut self, requested: wgpu::PresentMode) -> wgpu::PresentMode {
        let mode = selected_present_mode(&self.supported_present_modes, requested);
        if self.config.present_mode == mode {
            return mode;
        }
        log::info!(
            "reconfiguring native surface present_mode {:?} -> {:?} (requested {:?})",
            self.config.present_mode,
            mode,
            requested
        );
        self.config.present_mode = mode;
        self.surface.configure(&self.device, &self.config);
        mode
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
        Ok(self.render_with_report(encode)?.status)
    }

    pub fn render_with_report<F>(&mut self, encode: F) -> Result<SurfaceFrameReport>
    where
        F: FnOnce(RenderFrameContext<'_>) -> Result<()>,
    {
        let acquire_start = Instant::now();
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(SurfaceFrameReport::new(
                    SurfaceFrameStatus::Reconfigured,
                    elapsed_ms(acquire_start),
                ));
            }
            Err(wgpu::SurfaceError::OutOfMemory) => bail!("wgpu surface out of memory"),
            Err(err) => {
                log::warn!("surface acquire failed: {err:?}");
                return Ok(SurfaceFrameReport::new(
                    SurfaceFrameStatus::Skipped,
                    elapsed_ms(acquire_start),
                ));
            }
        };
        let acquire_ms = elapsed_ms(acquire_start);

        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_native_frame_encoder"),
            });
        let target =
            RenderFrameTarget::color(&view, [self.config.width.max(1), self.config.height.max(1)]);
        let encode_start = Instant::now();
        encode(RenderFrameContext::new(
            &self.device,
            &self.queue,
            &mut encoder,
            target,
        ))?;
        let encode_ms = elapsed_ms(encode_start);
        let submit_start = Instant::now();
        self.queue.submit(std::iter::once(encoder.finish()));
        let submit_ms = elapsed_ms(submit_start);
        let present_start = Instant::now();
        frame.present();
        let present_ms = elapsed_ms(present_start);
        Ok(SurfaceFrameReport {
            status: SurfaceFrameStatus::Presented,
            acquire_ms,
            encode_ms,
            submit_ms,
            present_ms,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfacePresentModePreference {
    Vsync,
    NoVsync,
}

impl SurfacePresentModePreference {
    fn requested_present_mode(self) -> wgpu::PresentMode {
        match self {
            Self::Vsync => wgpu::PresentMode::Fifo,
            Self::NoVsync => wgpu::PresentMode::AutoNoVsync,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceFrameStatus {
    Presented,
    Reconfigured,
    Skipped,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceFrameReport {
    pub status: SurfaceFrameStatus,
    pub acquire_ms: f64,
    pub encode_ms: f64,
    pub submit_ms: f64,
    pub present_ms: f64,
}

impl SurfaceFrameReport {
    fn new(status: SurfaceFrameStatus, acquire_ms: f64) -> Self {
        Self {
            status,
            acquire_ms,
            encode_ms: 0.0,
            submit_ms: 0.0,
            present_ms: 0.0,
        }
    }
}

pub fn surface_present_mode_label(mode: wgpu::PresentMode) -> &'static str {
    match mode {
        wgpu::PresentMode::AutoVsync => "auto-vsync",
        wgpu::PresentMode::AutoNoVsync => "auto-no-vsync",
        wgpu::PresentMode::Fifo => "fifo",
        wgpu::PresentMode::FifoRelaxed => "fifo-relaxed",
        wgpu::PresentMode::Immediate => "immediate",
        wgpu::PresentMode::Mailbox => "mailbox",
    }
}

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn selected_surface_format(
    caps: &wgpu::SurfaceCapabilities,
    color_profile: RenderColorProfile,
) -> wgpu::TextureFormat {
    selected_surface_format_with_override(caps, color_profile, surface_format_override())
}

fn selected_present_mode(
    supported: &[wgpu::PresentMode],
    requested: wgpu::PresentMode,
) -> wgpu::PresentMode {
    if supported.contains(&requested) {
        return requested;
    }
    if requested == wgpu::PresentMode::AutoNoVsync
        && supported.contains(&wgpu::PresentMode::Immediate)
    {
        return wgpu::PresentMode::Immediate;
    }
    if requested == wgpu::PresentMode::AutoNoVsync
        && supported.contains(&wgpu::PresentMode::Mailbox)
    {
        return wgpu::PresentMode::Mailbox;
    }
    if supported.contains(&wgpu::PresentMode::Fifo) {
        return wgpu::PresentMode::Fifo;
    }
    supported
        .first()
        .copied()
        .unwrap_or(wgpu::PresentMode::Fifo)
}

fn selected_surface_format_with_override(
    caps: &wgpu::SurfaceCapabilities,
    color_profile: RenderColorProfile,
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
    RenderConfig::preferred_surface_format_for_profile(caps, color_profile)
        .unwrap_or(caps.formats[0])
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
    fn surface_format_prefers_non_srgb_for_vanilla() {
        let caps = caps(vec![
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        ]);

        assert_eq!(
            selected_surface_format_with_override(&caps, RenderColorProfile::Vanilla, None),
            wgpu::TextureFormat::Rgba8Unorm
        );
    }

    #[test]
    fn valid_surface_format_override_wins() {
        let caps = caps(vec![
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        ]);

        assert_eq!(
            selected_surface_format_with_override(
                &caps,
                RenderColorProfile::Vanilla,
                Some(wgpu::TextureFormat::Rgba8Unorm),
            ),
            wgpu::TextureFormat::Rgba8Unorm
        );
    }

    #[test]
    fn unsupported_surface_format_override_is_ignored() {
        let caps = caps(vec![wgpu::TextureFormat::Rgba8Unorm]);

        assert_eq!(
            selected_surface_format_with_override(
                &caps,
                RenderColorProfile::Vanilla,
                Some(wgpu::TextureFormat::Bgra8UnormSrgb),
            ),
            wgpu::TextureFormat::Rgba8Unorm
        );
    }

    #[test]
    fn present_mode_uses_requested_when_supported() {
        assert_eq!(
            selected_present_mode(
                &[wgpu::PresentMode::Fifo, wgpu::PresentMode::AutoNoVsync],
                wgpu::PresentMode::AutoNoVsync,
            ),
            wgpu::PresentMode::AutoNoVsync
        );
    }

    #[test]
    fn present_mode_falls_back_to_fifo() {
        assert_eq!(
            selected_present_mode(&[wgpu::PresentMode::Fifo], wgpu::PresentMode::AutoNoVsync),
            wgpu::PresentMode::Fifo
        );
    }
}
