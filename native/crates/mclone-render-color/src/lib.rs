#![forbid(unsafe_code)]

use std::str::FromStr;

pub const TARGET_COLOR_TRANSFER_WGSL_MARKER: &str = "// __MCLONE_TARGET_COLOR_TRANSFER_WGSL__";
pub const TARGET_COLOR_TRANSFORM_WGSL_MARKER: &str = "__MCLONE_TARGET_COLOR_TRANSFORM__";

pub const TARGET_COLOR_TRANSFER_WGSL: &str = r#"
fn mclone_srgb_decode_channel(value: f32) -> f32 {
    let clamped = clamp(value, 0.0, 1.0);
    return select(
        pow((clamped + 0.055) / 1.055, 2.4),
        clamped / 12.92,
        clamped <= 0.04045,
    );
}

fn mclone_srgb_encode_channel(value: f32) -> f32 {
    let clamped = clamp(value, 0.0, 1.0);
    return select(
        1.055 * pow(clamped, 1.0 / 2.4) - 0.055,
        clamped * 12.92,
        clamped <= 0.0031308,
    );
}

fn mclone_apply_target_color_transform_rgb(
    color: vec3<f32>,
    transform: f32,
) -> vec3<f32> {
    if transform > 1.5 {
        return vec3<f32>(
            mclone_srgb_encode_channel(color.r),
            mclone_srgb_encode_channel(color.g),
            mclone_srgb_encode_channel(color.b),
        );
    }
    if transform > 0.5 {
        return vec3<f32>(
            mclone_srgb_decode_channel(color.r),
            mclone_srgb_decode_channel(color.g),
            mclone_srgb_decode_channel(color.b),
        );
    }
    return color;
}

fn mclone_apply_target_color_transform_rgba(
    color: vec4<f32>,
    transform: f32,
) -> vec4<f32> {
    return vec4<f32>(
        mclone_apply_target_color_transform_rgb(color.rgb, transform),
        color.a,
    );
}
"#;

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
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::SrgbDecode => "srgb-decode",
            Self::SrgbEncode => "srgb-encode",
        }
    }

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
    profile: RenderColorProfile,
) -> Option<wgpu::TextureFormat> {
    let _ = profile;
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
        RenderTargetColorTransform::Identity => channel,
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

pub fn inject_target_color_transfer_wgsl(template: &str) -> Result<String, String> {
    replace_exactly_once(
        template,
        TARGET_COLOR_TRANSFER_WGSL_MARKER,
        TARGET_COLOR_TRANSFER_WGSL,
    )
}

pub fn inject_target_color_transform_wgsl(
    template: &str,
    transform: RenderTargetColorTransform,
) -> Result<String, String> {
    let source = inject_target_color_transfer_wgsl(template)?;
    replace_exactly_once(
        &source,
        TARGET_COLOR_TRANSFORM_WGSL_MARKER,
        &format!("{:.1}", transform.shader_code()),
    )
}

fn replace_exactly_once(source: &str, marker: &str, replacement: &str) -> Result<String, String> {
    match source.match_indices(marker).count() {
        1 => Ok(source.replacen(marker, replacement, 1)),
        count => Err(format!(
            "expected one {marker:?} marker in WGSL template, found {count}"
        )),
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

    #[test]
    fn vanilla_preserves_display_values_across_unorm_pairs() {
        assert_eq!(
            RenderColorProfile::Vanilla.target_color_transform(wgpu::TextureFormat::Rgba8Unorm),
            RenderTargetColorTransform::Identity
        );
        assert_eq!(
            RenderColorProfile::Vanilla.target_color_transform(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderTargetColorTransform::SrgbDecode
        );
    }

    #[test]
    fn cpu_transfer_round_trips_display_values() {
        let display = [0.11, 0.48, 0.69];
        let linear = color_transform_rgb(display, RenderTargetColorTransform::SrgbDecode);
        let round_trip = color_transform_rgb(linear, RenderTargetColorTransform::SrgbEncode);

        for (actual, expected) in round_trip.into_iter().zip(display) {
            assert!((actual - expected).abs() < 0.000_01);
        }
    }

    #[test]
    fn identity_transform_preserves_extended_range_values() {
        let color = [-0.25, 0.5, 1.25];

        assert_eq!(
            color_transform_rgb(color, RenderTargetColorTransform::Identity),
            color
        );
    }

    #[test]
    fn injected_transfer_is_valid_wgsl() {
        let source = inject_target_color_transform_wgsl(
            r#"
// __MCLONE_TARGET_COLOR_TRANSFER_WGSL__
const output_transform: f32 = __MCLONE_TARGET_COLOR_TRANSFORM__;

@fragment
fn fragment_main() -> @location(0) vec4<f32> {
    return mclone_apply_target_color_transform_rgba(
        vec4<f32>(0.11, 0.48, 0.69, 1.0),
        output_transform,
    );
}
"#,
            RenderTargetColorTransform::SrgbDecode,
        )
        .unwrap();

        naga::front::wgsl::parse_str(&source).unwrap();
        assert!(!source.contains("__MCLONE_TARGET_COLOR"));
    }
}
