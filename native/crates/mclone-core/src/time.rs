//! World time and day/night cycle math.
//!
//! Ports the overworld time-of-day curve from Java 1.17.1. The two consumers are
//! the renderer (sky color, celestial rig rotation) and any system that needs the
//! sun angle. Kept free of rendering and platform concerns so it is unit-testable
//! against the reference formula.
//!
//! Reference:
//! - `DimensionType.timeOfDay(long)` — `reference/.../world/level/dimension/DimensionType.java:432`
//! - `Level.getSunAngle(float)` — `reference/.../world/level/Level.java:422`

use std::f64::consts::PI;

/// Ticks in one full day/night cycle (`24000` in vanilla).
pub const DAY_LENGTH_TICKS: u64 = 24000;

/// `Mth.frac(double)` — fractional part, always in `[0, 1)`.
fn frac(value: f64) -> f64 {
    value - value.floor()
}

/// Port of `DimensionType.timeOfDay` for the overworld (no `fixedTime`).
///
/// Returns the celestial phase in `[0, 1)`. Because of the cosine smoothing the
/// mapping from `day_time` is non-linear: `dayTime` 6000 (noon) maps to `0.0`,
/// 18000 (midnight) to `0.5`; sunset is near `0.25` and sunrise near `0.75`.
pub fn time_of_day(day_time: u64) -> f32 {
    let frac = frac(day_time as f64 / DAY_LENGTH_TICKS as f64 - 0.25);
    let smooth = 0.5 - (frac * PI).cos() / 2.0;
    ((frac * 2.0 + smooth) / 3.0) as f32
}

/// Port of `Level.getSunAngle`: the celestial rig rotation in radians.
pub fn sun_angle(day_time: u64) -> f32 {
    time_of_day(day_time) * std::f32::consts::TAU
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirror of the reference formula at `f64` precision for cross-checking.
    fn reference_time_of_day(day_time: u64) -> f32 {
        let v3 = frac(day_time as f64 / 24000.0 - 0.25);
        let v5 = 0.5 - f64::cos(v3 * PI) / 2.0;
        (v3 * 2.0 + v5) as f32 / 3.0
    }

    #[test]
    fn matches_reference_formula_across_a_day() {
        for tick in (0..DAY_LENGTH_TICKS).step_by(137) {
            assert!((time_of_day(tick) - reference_time_of_day(tick)).abs() < 1e-6);
        }
    }

    #[test]
    fn phase_is_in_unit_interval() {
        for tick in (0..DAY_LENGTH_TICKS).step_by(101) {
            let phase = time_of_day(tick);
            assert!((0.0..1.0).contains(&phase), "phase {phase} out of range");
        }
    }

    #[test]
    fn noon_maps_to_zero_phase() {
        // dayTime 6000 is noon; the smoothed curve puts the sun overhead at 0.0.
        assert!(time_of_day(6000).abs() < 1e-4);
    }

    #[test]
    fn midnight_maps_to_half_phase() {
        assert!((time_of_day(18000) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn wraps_each_day() {
        assert_eq!(time_of_day(1000), time_of_day(1000 + DAY_LENGTH_TICKS));
    }
}
