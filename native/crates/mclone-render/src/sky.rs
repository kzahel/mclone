//! Sky color math for the day/night cycle.
//!
//! Parity ports of the Java 1.17.1 sky color path, kept small and pure so the
//! clear color (Phase 1) and later sky-dome geometry (Phase 2+) share one source
//! of truth. Rain/thunder/lightning-flash terms and biome blending are
//! intentionally omitted for now; a single fixed biome sky color is used.
//!
//! Reference:
//! - `ClientLevel.getSkyColor` — `reference/.../client/multiplayer/ClientLevel.java:555`
//! - `VanillaBiomes.calculateSkyColor` — `reference/.../data/worldgen/biome/VanillaBiomes.java:27`
//! - `Mth.hsvToRgb` — `reference/.../util/Mth.java:504`

use std::f32::consts::{PI, TAU};

/// Plains temperature, the biome whose sky color we use until biome data reaches
/// the client renderer (`VanillaBiomes` passes `0.8` for plains).
pub const PLAINS_TEMPERATURE: f32 = 0.8;

/// Port of `Mth.hsvToRgb`, returning `[0, 1]` RGB components.
fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> [f32; 3] {
    let sextant = (hue * 6.0) as i32 % 6;
    let frac = hue * 6.0 - sextant as f32;
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - frac * saturation);
    let t = value * (1.0 - (1.0 - frac) * saturation);
    let (r, g, b) = match sextant {
        0 => (value, t, p),
        1 => (q, value, p),
        2 => (p, value, t),
        3 => (p, q, value),
        4 => (t, p, value),
        _ => (value, p, q),
    };
    // Java rounds through 0..=255 ints; mirror that quantization for parity.
    [quantize(r), quantize(g), quantize(b)]
}

fn quantize(component: f32) -> f32 {
    ((component * 255.0) as i32).clamp(0, 255) as f32 / 255.0
}

/// Port of `VanillaBiomes.calculateSkyColor`: a biome's base sky color from its
/// temperature, as `[0, 1]` RGB.
pub fn calculate_sky_color(temperature: f32) -> [f32; 3] {
    let t = (temperature / 3.0).clamp(-1.0, 1.0);
    hsv_to_rgb(0.62222224 - t * 0.05, 0.5 + t * 0.1, 1.0)
}

/// The day/night brightness factor from `getSkyColor`:
/// `clamp(cos(timeOfDay·2π)·2 + 0.5, 0, 1)`. `1.0` near noon, `0.0` at night.
pub fn day_brightness_factor(time_of_day: f32) -> f32 {
    ((time_of_day * TAU).cos() * 2.0 + 0.5).clamp(0.0, 1.0)
}

/// Port of `ClientLevel.getSkyColor` (no rain/thunder/flash, fixed biome color):
/// the base biome sky color scaled by the day/night brightness factor.
pub fn sky_color(time_of_day: f32, base_biome_color: [f32; 3]) -> [f32; 3] {
    let factor = day_brightness_factor(time_of_day);
    [
        base_biome_color[0] * factor,
        base_biome_color[1] * factor,
        base_biome_color[2] * factor,
    ]
}

/// Convenience: the plains overworld sky clear color for a given celestial phase.
pub fn overworld_clear_color(time_of_day: f32) -> wgpu::Color {
    let [r, g, b] = sky_color(time_of_day, calculate_sky_color(PLAINS_TEMPERATURE));
    wgpu::Color {
        r: r as f64,
        g: g as f64,
        b: b as f64,
        a: 1.0,
    }
}

/// Port of `DimensionSpecialEffects.getSunriseColor` (overworld). Returns the
/// sunrise/sunset glow color as `[r, g, b, a]` when the sun is within ±0.4 of the
/// dawn/dusk band (in `cos(timeOfDay·2π)` space), or `None` outside it.
///
/// Reference: `reference/.../client/renderer/DimensionSpecialEffects.java:40`.
pub fn sunrise_color(time_of_day: f32) -> Option<[f32; 4]> {
    let cos_phase = (time_of_day * TAU).cos();
    if !(-0.4..=0.4).contains(&cos_phase) {
        return None;
    }
    let t = cos_phase / 0.4 * 0.5 + 0.5;
    let mut alpha = 1.0 - (1.0 - (t * PI).sin()) * 0.99;
    alpha *= alpha;
    Some([
        t * 0.3 + 0.7,
        t * t * 0.7 + 0.2,
        t * t * 0.0 + 0.2,
        alpha,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plains_base_color_matches_vanilla_packed_value() {
        // calculateSkyColor(0.8) packs to 0x78A7FF in vanilla.
        let [r, g, b] = calculate_sky_color(PLAINS_TEMPERATURE);
        let packed = ((r * 255.0).round() as u32) << 16
            | ((g * 255.0).round() as u32) << 8
            | (b * 255.0).round() as u32;
        assert_eq!(packed, 0x78A7FF, "got {packed:#08X}");
    }

    #[test]
    fn brightness_is_high_at_noon_and_zero_at_night() {
        // time_of_day 0.0 == noon (sun overhead) -> full brightness.
        assert!((day_brightness_factor(0.0) - 1.0).abs() < 1e-6);
        // 0.5 == midnight -> cos(pi)*2+0.5 = -1.5, clamped to 0.
        assert_eq!(day_brightness_factor(0.5), 0.0);
    }

    #[test]
    fn night_sky_is_black() {
        let color = overworld_clear_color(0.5);
        assert_eq!((color.r, color.g, color.b), (0.0, 0.0, 0.0));
    }

    #[test]
    fn sunrise_glow_is_present_at_dawn_band_and_absent_at_noon() {
        // Noon (phase 0.0): cos = 1.0, well outside the ±0.4 band.
        assert!(sunrise_color(0.0).is_none());
        // Midnight (phase 0.5): cos = -1.0, also outside the band.
        assert!(sunrise_color(0.5).is_none());
        // Dusk-ish: phase where cos(phase·2π) sits inside ±0.4 (e.g. 0.25 -> cos=0).
        let glow = sunrise_color(0.25).expect("dusk band should produce a glow color");
        // Center of the band: t = 0.5, so reddish-orange with mid alpha.
        assert!((glow[0] - 0.85).abs() < 1e-5, "r = {}", glow[0]);
        assert!(glow[1] > glow[2], "green {} should exceed blue {}", glow[1], glow[2]);
        assert!((0.0..=1.0).contains(&glow[3]), "alpha {} out of range", glow[3]);
    }

    #[test]
    fn noon_sky_is_the_plains_blue() {
        let color = overworld_clear_color(0.0);
        let [r, g, b] = calculate_sky_color(PLAINS_TEMPERATURE);
        assert!((color.r - r as f64).abs() < 1e-6);
        assert!((color.g - g as f64).abs() < 1e-6);
        assert!((color.b - b as f64).abs() < 1e-6);
    }
}
