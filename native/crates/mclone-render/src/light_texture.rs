//! Java `LightTexture` color math for packed block/sky light.
//!
//! This ports the stable part of
//! `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LightTexture.java`
//! for the native textured chunk shader. Dynamic effects such as torch
//! flicker, night vision, conduit power, boss-world darkening, and user gamma
//! are intentionally left as follow-up render options.

const TAU: f32 = std::f32::consts::PI * 2.0;

pub const FULL_BLOCK: u32 = 240;
pub const FULL_SKY: u32 = 15_728_640;
pub const FULL_BRIGHT: u32 = 15_728_880;

/// Overworld `DimensionType.fillBrightnessRamp(ambientLight=0)`.
pub fn dimension_brightness(light: u8) -> f32 {
    let light = light.min(15) as f32 / 15.0;
    light / (4.0 - 3.0 * light)
}

/// `ClientLevel.getSkyDarken(partialTicks)` without rain or thunder.
///
/// The native sky code uses the same celestial phase convention as Java:
/// `0.0` is noon and `0.5` is midnight.
pub fn sky_darken(time_of_day: f32) -> f32 {
    let mut darken = 1.0 - ((time_of_day * TAU).cos() * 2.0 + 0.2);
    darken = darken.clamp(0.0, 1.0);
    (1.0 - darken) * 0.8 + 0.2
}

/// RGB multiplier from Java's default overworld lightmap.
pub fn lightmap_color(block_light: u8, sky_light: u8, sky_darken: f32) -> [f32; 3] {
    let sky_darken = sky_darken.clamp(0.0, 1.0);
    let sky = dimension_brightness(sky_light) * (sky_darken * 0.95 + 0.05);
    let block = dimension_brightness(block_light) * 1.5;
    let mut color = [
        block,
        block * ((block * 0.6 + 0.4) * 0.6 + 0.4),
        block * (block * block * 0.6 + 0.4),
    ];
    let sky_tint = lerp_vec3([sky_darken, sky_darken, 1.0], [1.0, 1.0, 1.0], 0.35);
    color[0] += sky_tint[0] * sky;
    color[1] += sky_tint[1] * sky;
    color[2] += sky_tint[2] * sky;
    color = lerp_vec3(color, [0.75, 0.75, 0.75], 0.04);
    color = clamp_vec3(color, 0.0, 1.0);
    color = lerp_vec3(color, [0.75, 0.75, 0.75], 0.04);
    clamp_vec3(color, 0.0, 1.0)
}

pub fn packed_lightmap_color(packed_light: u32, sky_darken: f32) -> [f32; 3] {
    lightmap_color(
        packed_block_light(packed_light),
        packed_sky_light(packed_light),
        sky_darken,
    )
}

pub const fn packed_block_light(packed_light: u32) -> u8 {
    ((packed_light >> 4) & 15) as u8
}

pub const fn packed_sky_light(packed_light: u32) -> u8 {
    ((packed_light >> 20) & 15) as u8
}

fn lerp_vec3(left: [f32; 3], right: [f32; 3], delta: f32) -> [f32; 3] {
    [
        left[0] + (right[0] - left[0]) * delta,
        left[1] + (right[1] - left[1]) * delta,
        left[2] + (right[2] - left[2]) * delta,
    ]
}

fn clamp_vec3(value: [f32; 3], min: f32, max: f32) -> [f32; 3] {
    [
        value[0].clamp(min, max),
        value[1].clamp(min, max),
        value[2].clamp(min, max),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.00001,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn packed_light_helpers_match_java_layout() {
        assert_eq!(FULL_BLOCK, 15 << 4);
        assert_eq!(FULL_SKY, 15 << 20);
        assert_eq!(FULL_BRIGHT, FULL_BLOCK | FULL_SKY);
        assert_eq!(packed_block_light(FULL_BRIGHT), 15);
        assert_eq!(packed_sky_light(FULL_BRIGHT), 15);
    }

    #[test]
    fn brightness_ramp_matches_overworld_dimension_type() {
        assert_close(dimension_brightness(0), 0.0);
        assert_close(dimension_brightness(15), 1.0);
        assert_close(
            dimension_brightness(7),
            (7.0 / 15.0) / (4.0 - 3.0 * (7.0 / 15.0)),
        );
    }

    #[test]
    fn sky_darken_matches_clear_day_night_curve() {
        assert_close(sky_darken(0.0), 1.0);
        assert_close(sky_darken(0.5), 0.2);
    }

    #[test]
    fn lightmap_keeps_full_bright_near_white() {
        let color = packed_lightmap_color(FULL_BRIGHT, 1.0);

        assert!(color.iter().all(|component| *component > 0.98));
        assert!(color.iter().all(|component| *component <= 1.0));
    }

    #[test]
    fn block_only_light_is_warmer_than_sky_light() {
        let color = lightmap_color(8, 0, 1.0);

        assert!(color[0] >= color[1]);
        assert!(color[1] > color[2]);
    }

    #[test]
    fn dark_lightmap_preserves_java_gray_lift() {
        let color = lightmap_color(0, 0, 1.0);

        for component in color {
            assert_close(component, 0.0588);
        }
    }
}
