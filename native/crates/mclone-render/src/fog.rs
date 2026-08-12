#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u32)]
pub enum RenderFogMode {
    #[default]
    Off = 0,
    Linear = 1,
    Exponential = 2,
    GroundHaze = 3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderFog {
    pub enabled: bool,
    pub mode: RenderFogMode,
    pub color: [f32; 3],
    /// Linear fog onset. Exponential modes use `visibility`.
    pub start: f32,
    /// Linear fog completion. Exponential modes use `coverage_end`.
    pub end: f32,
    /// Distance where exponential fog leaves five percent scene contrast.
    pub visibility: f32,
    /// Conservative end of available terrain coverage.
    pub coverage_end: f32,
    pub coverage_guard: bool,
    pub guard_start: f32,
    pub ground_base_y: f32,
    pub ground_falloff: f32,
    pub max_opacity: f32,
    pub exponential_squared: bool,
    pub far_cull: bool,
    /// Media fog may replace the sky with its own solid clear color.
    pub solid_background: bool,
}

impl RenderFog {
    pub const WATER_COLOR: [f32; 3] = rgb_from_u24(0x050533);
    pub const WATER_START: f32 = -8.0;
    pub const WATER_END: f32 = 96.0;
    pub const WATER_MIN_VISION: f32 = 0.25;

    const MODE_MASK: u32 = 0b11;
    const EXPONENTIAL_SQUARED_BIT: u32 = 1 << 2;
    const COVERAGE_GUARD_BIT: u32 = 1 << 3;
    const GROUND_FALLOFF_SHIFT: u32 = 4;
    const GROUND_FALLOFF_MASK: u32 = 0x3ff;
    const GUARD_START_SHIFT: u32 = 14;
    const GUARD_START_MASK: u32 = 0x7f;

    pub const fn none() -> Self {
        Self {
            enabled: false,
            mode: RenderFogMode::Off,
            color: [0.0, 0.0, 0.0],
            start: 0.0,
            end: 0.0,
            visibility: 0.0,
            coverage_end: 0.0,
            coverage_guard: false,
            guard_start: 0.0,
            ground_base_y: 0.0,
            ground_falloff: 64.0,
            max_opacity: 1.0,
            exponential_squared: false,
            far_cull: false,
            solid_background: false,
        }
    }

    pub const fn underwater() -> Self {
        Self {
            enabled: true,
            mode: RenderFogMode::Linear,
            color: Self::WATER_COLOR,
            start: Self::WATER_START,
            end: Self::WATER_END,
            visibility: Self::WATER_END,
            coverage_end: Self::WATER_END,
            coverage_guard: false,
            guard_start: 0.0,
            ground_base_y: 0.0,
            ground_falloff: 64.0,
            max_opacity: 1.0,
            exponential_squared: false,
            far_cull: false,
            solid_background: true,
        }
    }

    pub fn underwater_with_water_vision(water_vision: f32) -> Self {
        let vision = if water_vision.is_finite() {
            water_vision.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let end = Self::WATER_END * vision.max(Self::WATER_MIN_VISION);
        Self {
            end,
            visibility: end,
            coverage_end: end,
            ..Self::underwater()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn open_air(
        mode: RenderFogMode,
        color: [f32; 3],
        visibility: f32,
        classic_start_ratio: f32,
        coverage_end: f32,
        coverage_guard: bool,
        guard_start: f32,
        ground_base_y: f32,
        ground_falloff: f32,
        max_opacity: f32,
        exponential_squared: bool,
        far_cull: bool,
    ) -> Self {
        if mode == RenderFogMode::Off {
            return Self::none();
        }
        let visibility = finite_positive_or(visibility, 32_768.0);
        let coverage_end = finite_positive_or(coverage_end, visibility);
        let classic_start_ratio = finite_or(classic_start_ratio, 0.75).clamp(0.0, 0.99);
        let linear_end = if coverage_guard {
            visibility.min(coverage_end)
        } else {
            visibility
        };
        Self {
            enabled: true,
            mode,
            color: color.map(|channel| finite_or(channel, 0.0).clamp(0.0, 1.0)),
            start: linear_end * classic_start_ratio,
            end: linear_end,
            visibility,
            coverage_end,
            coverage_guard,
            guard_start: finite_or(guard_start, 0.9).clamp(0.0, 0.99),
            ground_base_y: finite_or(ground_base_y, 64.0),
            ground_falloff: finite_positive_or(ground_falloff, 64.0).clamp(1.0, 1023.0),
            max_opacity: finite_or(max_opacity, 0.98).clamp(0.0, 1.0),
            exponential_squared,
            far_cull,
            solid_background: false,
        }
    }

    /// Bit-packed shader policy carried in the existing render-options lane.
    ///
    /// Keeping the packed value below bit 21 guarantees a finite subnormal
    /// `f32`; WGSL recovers the exact integer with `bitcast<u32>`.
    pub fn shader_options(self) -> f32 {
        let mut bits = (self.mode as u32) & Self::MODE_MASK;
        if self.exponential_squared {
            bits |= Self::EXPONENTIAL_SQUARED_BIT;
        }
        if self.coverage_guard {
            bits |= Self::COVERAGE_GUARD_BIT;
        }
        let falloff = self.ground_falloff.round().clamp(1.0, 1023.0) as u32;
        bits |= (falloff & Self::GROUND_FALLOFF_MASK) << Self::GROUND_FALLOFF_SHIFT;
        let guard_start = (self.guard_start.clamp(0.0, 1.0) * 127.0).round() as u32;
        bits |= (guard_start & Self::GUARD_START_MASK) << Self::GUARD_START_SHIFT;
        f32::from_bits(bits)
    }

    pub fn shader_distances(self) -> [f32; 2] {
        match self.mode {
            RenderFogMode::Off => [0.0, 0.0],
            RenderFogMode::Linear => [self.start, self.end],
            RenderFogMode::Exponential | RenderFogMode::GroundHaze => {
                [self.visibility, self.coverage_end]
            }
        }
    }

    pub const fn clears_background(self) -> bool {
        self.enabled && self.solid_background
    }

    /// Conservative distance after which fragments are effectively opaque.
    ///
    /// Ground haze is height-dependent, so only its explicit coverage guard
    /// can authorize geometry culling. Exponential modes without a guard
    /// require full opacity before the 99.5% contrast threshold is safe.
    pub fn far_cull_distance(self) -> Option<f32> {
        if !self.enabled || !self.far_cull {
            return None;
        }
        let coverage =
            (self.coverage_guard && self.coverage_end.is_finite() && self.coverage_end > 0.0)
                .then_some(self.coverage_end);
        let atmosphere = match self.mode {
            RenderFogMode::Off | RenderFogMode::GroundHaze => None,
            RenderFogMode::Linear => Some(self.end),
            RenderFogMode::Exponential if self.max_opacity >= 0.995 => {
                let ratio = if self.exponential_squared {
                    (200.0_f32.ln() / 20.0_f32.ln()).sqrt()
                } else {
                    200.0_f32.ln() / 20.0_f32.ln()
                };
                Some(self.visibility * ratio)
            }
            RenderFogMode::Exponential => None,
        }
        .filter(|distance| distance.is_finite() && *distance > 0.0);
        match (coverage, atmosphere) {
            (Some(coverage), Some(atmosphere)) => Some(coverage.min(atmosphere)),
            (Some(coverage), None) => Some(coverage),
            (None, Some(atmosphere)) => Some(atmosphere),
            (None, None) => None,
        }
    }

    pub fn clear_color(self) -> wgpu::Color {
        wgpu::Color {
            r: self.color[0] as f64,
            g: self.color[1] as f64,
            b: self.color[2] as f64,
            a: 1.0,
        }
    }
}

impl Default for RenderFog {
    fn default() -> Self {
        Self::none()
    }
}

pub fn inject_fog_wgsl(template: &str) -> String {
    const MARKER: &str = "// MCLONE_FOG_FUNCTION";
    assert_eq!(
        template.matches(MARKER).count(),
        1,
        "fog-aware WGSL must contain exactly one fog marker"
    );
    template.replace(MARKER, include_str!("shaders/fog.wgsl"))
}

const fn rgb_from_u24(value: u32) -> [f32; 3] {
    [
        ((value >> 16) & 0xff) as f32 / 255.0,
        ((value >> 8) & 0xff) as f32 / 255.0,
        (value & 0xff) as f32 / 255.0,
    ]
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

fn finite_positive_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn underwater_fog_matches_java_fallback_constants() {
        let fog = RenderFog::underwater();

        assert!(fog.enabled);
        assert_eq!(fog.mode, RenderFogMode::Linear);
        assert_eq!(fog.color, [5.0 / 255.0, 5.0 / 255.0, 51.0 / 255.0]);
        assert_eq!(fog.start, -8.0);
        assert_eq!(fog.end, 96.0);
        assert!(fog.clears_background());
    }

    #[test]
    fn underwater_fog_uses_java_water_vision_distance_ramp() {
        let entering = RenderFog::underwater_with_water_vision(0.0);
        let half = RenderFog::underwater_with_water_vision(0.5);
        let clear = RenderFog::underwater_with_water_vision(1.0);

        assert_eq!(entering.start, -8.0);
        assert_eq!(entering.end, 24.0);
        assert_eq!(half.end, 48.0);
        assert_eq!(clear.end, 96.0);
    }

    #[test]
    fn shader_options_round_trip_without_nan_payloads() {
        let fog = RenderFog::open_air(
            RenderFogMode::GroundHaze,
            [0.5; 3],
            32_768.0,
            0.75,
            140_000.0,
            true,
            0.9,
            64.0,
            512.0,
            0.98,
            true,
            true,
        );
        let bits = fog.shader_options().to_bits();

        assert!(fog.shader_options().is_finite());
        assert_eq!(
            bits & RenderFog::MODE_MASK,
            RenderFogMode::GroundHaze as u32
        );
        assert_ne!(bits & RenderFog::EXPONENTIAL_SQUARED_BIT, 0);
        assert_ne!(bits & RenderFog::COVERAGE_GUARD_BIT, 0);
        assert_eq!(
            (bits >> RenderFog::GROUND_FALLOFF_SHIFT) & RenderFog::GROUND_FALLOFF_MASK,
            512
        );
    }

    #[test]
    fn far_cull_requires_a_conservative_opaque_boundary() {
        let natural = RenderFog::open_air(
            RenderFogMode::Exponential,
            [0.5; 3],
            1_000.0,
            0.75,
            2_000.0,
            false,
            0.9,
            64.0,
            64.0,
            0.98,
            false,
            true,
        );
        assert_eq!(natural.far_cull_distance(), None);

        let opaque = RenderFog {
            max_opacity: 1.0,
            ..natural
        };
        assert!(opaque.far_cull_distance().is_some_and(|distance| {
            distance > opaque.visibility && distance < opaque.coverage_end
        }));

        let guarded_ground = RenderFog {
            mode: RenderFogMode::GroundHaze,
            coverage_guard: true,
            ..natural
        };
        assert_eq!(
            guarded_ground.far_cull_distance(),
            Some(guarded_ground.coverage_end)
        );
    }
}
