#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderFog {
    pub enabled: bool,
    pub color: [f32; 3],
    pub start: f32,
    pub end: f32,
}

impl RenderFog {
    pub const WATER_COLOR: [f32; 3] = rgb_from_u24(0x050533);
    pub const WATER_START: f32 = -8.0;
    pub const WATER_END: f32 = 96.0;

    pub const fn none() -> Self {
        Self {
            enabled: false,
            color: [0.0, 0.0, 0.0],
            start: 0.0,
            end: 0.0,
        }
    }

    pub const fn underwater() -> Self {
        Self {
            enabled: true,
            color: Self::WATER_COLOR,
            start: Self::WATER_START,
            end: Self::WATER_END,
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

const fn rgb_from_u24(value: u32) -> [f32; 3] {
    [
        ((value >> 16) & 0xff) as f32 / 255.0,
        ((value >> 8) & 0xff) as f32 / 255.0,
        (value & 0xff) as f32 / 255.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn underwater_fog_matches_java_fallback_constants() {
        let fog = RenderFog::underwater();

        assert!(fog.enabled);
        assert_eq!(fog.color, [5.0 / 255.0, 5.0 / 255.0, 51.0 / 255.0]);
        assert_eq!(fog.start, -8.0);
        assert_eq!(fog.end, 96.0);
    }
}
