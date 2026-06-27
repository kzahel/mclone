use std::str::FromStr;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderColorProfile {
    #[default]
    Vanilla,
    StylizedBright,
    LinearExperimental,
}

impl RenderColorProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::StylizedBright => "stylized-bright",
            Self::LinearExperimental => "linear-experimental",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Vanilla => "VANILLA",
            Self::StylizedBright => "BRIGHT",
            Self::LinearExperimental => "LINEAR",
        }
    }

    pub fn target_color_transform(
        self,
        target_format: wgpu::TextureFormat,
    ) -> RenderTargetColorTransform {
        match (self, target_format.is_srgb()) {
            (Self::Vanilla, true) => RenderTargetColorTransform::SrgbDecode,
            (Self::StylizedBright, false) => RenderTargetColorTransform::SrgbEncode,
            _ => RenderTargetColorTransform::Identity,
        }
    }
}

impl FromStr for RenderColorProfile {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value
            .trim()
            .to_ascii_lowercase()
            .replace('_', "-")
            .replace(' ', "-");
        match normalized.as_str() {
            "vanilla" | "java" | "minecraft" | "minecraft-vanilla" => Ok(Self::Vanilla),
            "stylized-bright" | "stylized" | "bright" => Ok(Self::StylizedBright),
            "linear-experimental" | "linear" | "pbr" | "hdr" => Ok(Self::LinearExperimental),
            _ => Err(format!(
                "expected vanilla, stylized-bright, or linear-experimental, got `{value}`"
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderTargetColorTransform {
    Identity,
    SrgbDecode,
    SrgbEncode,
}

impl RenderTargetColorTransform {
    pub const fn shader_code(self) -> f32 {
        match self {
            Self::Identity => 0.0,
            Self::SrgbDecode => 1.0,
            Self::SrgbEncode => 2.0,
        }
    }
}

pub fn preferred_surface_format_for_profile(
    caps: &wgpu::SurfaceCapabilities,
    _profile: RenderColorProfile,
) -> Option<wgpu::TextureFormat> {
    preferred_non_srgb_format(caps.formats.iter().copied())
        .or_else(|| {
            caps.formats
                .iter()
                .copied()
                .find(wgpu::TextureFormat::is_srgb)
        })
        .or_else(|| caps.formats.first().copied())
}

pub fn preferred_non_srgb_format(
    formats: impl IntoIterator<Item = wgpu::TextureFormat>,
) -> Option<wgpu::TextureFormat> {
    let formats = formats.into_iter().collect::<Vec<_>>();
    [
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba8Unorm,
    ]
    .into_iter()
    .find(|format| formats.contains(format))
    .or_else(|| formats.into_iter().find(|format| !format.is_srgb()))
}

pub fn color_transform_rgb(color: [f32; 3], transform: RenderTargetColorTransform) -> [f32; 3] {
    color.map(|channel| match transform {
        RenderTargetColorTransform::Identity => channel.clamp(0.0, 1.0),
        RenderTargetColorTransform::SrgbDecode => srgb_decode(channel),
        RenderTargetColorTransform::SrgbEncode => srgb_encode(channel),
    })
}

pub fn color_transform_wgpu(
    color: wgpu::Color,
    transform: RenderTargetColorTransform,
) -> wgpu::Color {
    let [r, g, b] =
        color_transform_rgb([color.r as f32, color.g as f32, color.b as f32], transform);
    wgpu::Color {
        r: f64::from(r),
        g: f64::from(g),
        b: f64::from(b),
        a: color.a,
    }
}

fn srgb_decode(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_encode(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
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
