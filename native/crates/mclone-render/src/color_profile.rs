pub use mclone_render_color::{
    RenderColorProfile, RenderTargetColorTransform, color_transform_rgb, color_transform_wgpu,
    preferred_non_srgb_format,
};

pub const DEFAULT_RENDER_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEFAULT_RENDER_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const DEFAULT_RENDER_SAMPLE_COUNT: u32 = 1;
pub const DEFAULT_RENDER_SCALE: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderConfig {
    pub color_profile: RenderColorProfile,
    pub color_format: wgpu::TextureFormat,
    pub depth_format: wgpu::TextureFormat,
    pub sample_count: u32,
    pub render_scale: f32,
    pub hdr: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            color_profile: RenderColorProfile::default(),
            color_format: DEFAULT_RENDER_COLOR_FORMAT,
            depth_format: DEFAULT_RENDER_DEPTH_FORMAT,
            sample_count: DEFAULT_RENDER_SAMPLE_COUNT,
            render_scale: DEFAULT_RENDER_SCALE,
            hdr: false,
        }
    }
}

impl RenderConfig {
    pub const fn for_color_target(
        color_profile: RenderColorProfile,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            color_profile,
            color_format,
            depth_format: DEFAULT_RENDER_DEPTH_FORMAT,
            sample_count: DEFAULT_RENDER_SAMPLE_COUNT,
            render_scale: DEFAULT_RENDER_SCALE,
            hdr: false,
        }
    }

    pub const fn with_color_profile(mut self, color_profile: RenderColorProfile) -> Self {
        self.color_profile = color_profile;
        self
    }

    pub const fn with_color_format(mut self, color_format: wgpu::TextureFormat) -> Self {
        self.color_format = color_format;
        self
    }

    pub const fn with_depth_format(mut self, depth_format: wgpu::TextureFormat) -> Self {
        self.depth_format = depth_format;
        self
    }

    pub const fn with_sample_count(mut self, sample_count: u32) -> Self {
        self.sample_count = sample_count;
        self
    }

    pub const fn with_render_scale(mut self, render_scale: f32) -> Self {
        self.render_scale = render_scale;
        self
    }

    pub const fn with_hdr(mut self, hdr: bool) -> Self {
        self.hdr = hdr;
        self
    }

    pub fn validate(self) -> Result<Self, RenderConfigError> {
        if self.sample_count == 0 {
            return Err(RenderConfigError::InvalidSampleCount(self.sample_count));
        }
        if !self.render_scale.is_finite() || self.render_scale <= 0.0 {
            return Err(RenderConfigError::InvalidRenderScale(self.render_scale));
        }
        Ok(self)
    }

    pub fn target_color_transform(self) -> RenderTargetColorTransform {
        self.color_profile.target_color_transform(self.color_format)
    }

    pub fn preferred_surface_format_for_profile(
        caps: &wgpu::SurfaceCapabilities,
        profile: RenderColorProfile,
    ) -> Option<wgpu::TextureFormat> {
        mclone_render_color::preferred_surface_format_for_profile(caps, profile)
    }

    pub fn diagnostic_label(self) -> String {
        format!(
            "profile={} color={:?} depth={:?} samples={} scale={:.3} hdr={}",
            self.color_profile.as_str(),
            self.color_format,
            self.depth_format,
            self.sample_count,
            self.render_scale,
            self.hdr
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RenderConfigError {
    InvalidSampleCount(u32),
    InvalidRenderScale(f32),
}

impl std::fmt::Display for RenderConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::InvalidSampleCount(sample_count) => {
                write!(
                    formatter,
                    "render sample count must be greater than zero, got {sample_count}"
                )
            }
            Self::InvalidRenderScale(render_scale) => {
                write!(
                    formatter,
                    "render scale must be finite and greater than zero, got {render_scale}"
                )
            }
        }
    }
}

impl std::error::Error for RenderConfigError {}

pub fn preferred_surface_format_for_profile(
    caps: &wgpu::SurfaceCapabilities,
    profile: RenderColorProfile,
) -> Option<wgpu::TextureFormat> {
    RenderConfig::preferred_surface_format_for_profile(caps, profile)
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
    fn defaults_to_vanilla_profile() {
        assert_eq!(RenderColorProfile::default(), RenderColorProfile::Vanilla);
        assert_eq!(RenderColorProfile::Vanilla.as_str(), "vanilla");
        assert_eq!(
            RenderColorProfile::StylizedBright.as_str(),
            "stylized-bright"
        );
        assert_eq!(
            RenderColorProfile::LinearExperimental.as_str(),
            "linear-experimental"
        );
    }

    #[test]
    fn render_config_defaults_to_vanilla_sdr_single_sample() {
        let config = RenderConfig::default();

        assert_eq!(config.color_profile, RenderColorProfile::Vanilla);
        assert_eq!(config.color_format, wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(config.depth_format, wgpu::TextureFormat::Depth32Float);
        assert_eq!(config.sample_count, 1);
        assert_eq!(config.render_scale, 1.0);
        assert!(!config.hdr);
        assert_eq!(config.validate().unwrap(), config);
    }

    #[test]
    fn render_config_builds_target_transform() {
        let vanilla_srgb = RenderConfig::for_color_target(
            RenderColorProfile::Vanilla,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        );
        let bright_unorm = RenderConfig::for_color_target(
            RenderColorProfile::StylizedBright,
            wgpu::TextureFormat::Bgra8Unorm,
        );

        assert_eq!(
            vanilla_srgb.target_color_transform(),
            RenderTargetColorTransform::SrgbDecode
        );
        assert_eq!(
            bright_unorm.target_color_transform(),
            RenderTargetColorTransform::SrgbEncode
        );
    }

    #[test]
    fn render_config_equality_identifies_no_op_changes() {
        let base = RenderConfig::default();
        let same = RenderConfig::default();
        let changed = base.with_hdr(true);

        assert_eq!(base, same);
        assert_ne!(base, changed);
    }

    #[test]
    fn render_config_rejects_invalid_values() {
        assert_eq!(
            RenderConfig::default().with_sample_count(0).validate(),
            Err(RenderConfigError::InvalidSampleCount(0))
        );
        assert_eq!(
            RenderConfig::default().with_render_scale(0.0).validate(),
            Err(RenderConfigError::InvalidRenderScale(0.0))
        );
        assert!(
            RenderConfig::default()
                .with_render_scale(f32::NAN)
                .validate()
                .is_err()
        );
    }

    #[test]
    fn render_config_exposes_diagnostic_label() {
        let label = RenderConfig::default()
            .with_color_profile(RenderColorProfile::StylizedBright)
            .with_color_format(wgpu::TextureFormat::Bgra8Unorm)
            .diagnostic_label();

        assert!(label.contains("profile=stylized-bright"));
        assert!(label.contains("color=Bgra8Unorm"));
        assert!(label.contains("samples=1"));
        assert!(label.contains("hdr=false"));
    }

    #[test]
    fn parses_profile_aliases() {
        assert_eq!(
            "vanilla".parse::<RenderColorProfile>().unwrap(),
            RenderColorProfile::Vanilla
        );
        assert_eq!(
            "bright".parse::<RenderColorProfile>().unwrap(),
            RenderColorProfile::StylizedBright
        );
        assert_eq!(
            "hdr".parse::<RenderColorProfile>().unwrap(),
            RenderColorProfile::LinearExperimental
        );
        assert!("bogus".parse::<RenderColorProfile>().is_err());
    }

    #[test]
    fn vanilla_surface_policy_prefers_non_srgb() {
        let caps = caps(vec![
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Bgra8Unorm,
        ]);

        assert_eq!(
            preferred_surface_format_for_profile(&caps, RenderColorProfile::Vanilla),
            Some(wgpu::TextureFormat::Bgra8Unorm)
        );
    }

    #[test]
    fn surface_policy_falls_back_to_srgb() {
        let caps = caps(vec![wgpu::TextureFormat::Rgba8UnormSrgb]);

        assert_eq!(
            preferred_surface_format_for_profile(&caps, RenderColorProfile::Vanilla),
            Some(wgpu::TextureFormat::Rgba8UnormSrgb)
        );
    }

    #[test]
    fn target_transform_compensates_for_srgb_targets() {
        assert_eq!(
            RenderColorProfile::Vanilla.target_color_transform(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderTargetColorTransform::SrgbDecode
        );
        assert_eq!(
            RenderColorProfile::StylizedBright
                .target_color_transform(wgpu::TextureFormat::Rgba8Unorm),
            RenderTargetColorTransform::SrgbEncode
        );
    }

    #[test]
    fn srgb_round_trip_is_stable() {
        let color = [0.0588, 0.25, 0.75];
        let encoded = color_transform_rgb(color, RenderTargetColorTransform::SrgbEncode);
        let decoded = color_transform_rgb(encoded, RenderTargetColorTransform::SrgbDecode);

        for (actual, expected) in decoded.into_iter().zip(color) {
            assert!((actual - expected).abs() < 0.0001);
        }
    }
}
