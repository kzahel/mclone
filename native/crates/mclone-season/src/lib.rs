#![forbid(unsafe_code)]

//! Pure climate-coordinate and solar-position vocabulary for Mclone worlds.
//!
//! This crate deliberately owns no world, renderer, clock, persistence, or
//! platform state. A caller chooses a coordinate policy, samples it at one
//! observer position, and combines that latitude with a global orbital phase
//! and solar time.

use std::f64::consts::{PI, TAU};

use mclone_core::{AxisTopology, HorizontalTopology};

/// Accepted review candidate for one full cyclical-plane latitude wave.
///
/// Equator-to-polar-crest travel is one quarter of this value: 24,576 blocks.
/// The value is sixteen times the optional 6,144-block cylinder circumference,
/// making comparisons easy without coupling the plane policy to that topology.
pub const MCLONE_PLANE_LATITUDE_WAVELENGTH_BLOCKS: f64 = 98_304.0;

/// Place the ordinary origin/spawn search in the equatorial family.
pub const MCLONE_PLANE_LATITUDE_PHASE_ORIGIN_Z: f64 = 0.0;

/// Scale for the opt-in cylinder's asymptotic north/south climate coordinate.
pub const MCLONE_CYLINDER_LATITUDE_SCALE_BLOCKS: f64 = 12_288.0;

/// Restrained stylized axial tilt selected for the first solar-path proof.
pub const MCLONE_AXIAL_TILT_DEGREES: f64 = 27.0;

/// Fixed-point resolution for client-local Debug orbital state.
pub const ORBITAL_PHASE_STEPS: u16 = 10_000;

/// A normalized orbital turn stored without accumulating floating-point drift.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct OrbitalPhase(u16);

impl OrbitalPhase {
    pub const NORTHWARD_EQUINOX: Self = Self(0);
    pub const NORTHERN_SOLSTICE: Self = Self(ORBITAL_PHASE_STEPS / 4);
    pub const SOUTHWARD_EQUINOX: Self = Self(ORBITAL_PHASE_STEPS / 2);
    pub const SOUTHERN_SOLSTICE: Self = Self(ORBITAL_PHASE_STEPS * 3 / 4);

    pub const fn from_steps_wrapped(steps: u16) -> Self {
        Self(steps % ORBITAL_PHASE_STEPS)
    }

    pub fn from_turns_wrapped(turns: f64) -> Result<Self, SolarInputError> {
        if !turns.is_finite() {
            return Err(SolarInputError::NonFinite("orbital_phase"));
        }
        let wrapped = turns.rem_euclid(1.0);
        let steps = (wrapped * f64::from(ORBITAL_PHASE_STEPS)).round() as u16;
        Ok(Self::from_steps_wrapped(steps))
    }

    pub const fn steps(self) -> u16 {
        self.0
    }

    pub fn turns(self) -> f64 {
        f64::from(self.0) / f64::from(ORBITAL_PHASE_STEPS)
    }
}

/// Dimension-level selection for mapping world coordinates to solar latitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SolarCoordinatePolicy {
    /// Retained profiles keep their existing vanilla-shaped fixed path.
    VanillaFixed,
    /// The default Mclone plane repeats smooth latitude bands along world Z.
    McloneCyclicPlane {
        wavelength_blocks: f64,
        phase_origin_z: f64,
    },
    /// The opt-in X-periodic cylinder approaches polar plateaus along Z.
    McloneAsymptoticCylinder { climate_scale_blocks: f64 },
}

impl SolarCoordinatePolicy {
    pub const MCLONE_PLANE: Self = Self::McloneCyclicPlane {
        wavelength_blocks: MCLONE_PLANE_LATITUDE_WAVELENGTH_BLOCKS,
        phase_origin_z: MCLONE_PLANE_LATITUDE_PHASE_ORIGIN_Z,
    };

    pub const MCLONE_CYLINDER: Self = Self::McloneAsymptoticCylinder {
        climate_scale_blocks: MCLONE_CYLINDER_LATITUDE_SCALE_BLOCKS,
    };

    pub fn mclone_for_topology(topology: HorizontalTopology) -> Self {
        match topology {
            HorizontalTopology {
                x: AxisTopology::Periodic { .. },
                z: AxisTopology::Unbounded,
            } => Self::MCLONE_CYLINDER,
            _ => Self::MCLONE_PLANE,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::VanillaFixed => "vanilla-fixed",
            Self::McloneCyclicPlane { .. } => "mclone-cyclic-plane",
            Self::McloneAsymptoticCylinder { .. } => "mclone-asymptotic-cylinder",
        }
    }

    pub fn latitude_at(
        self,
        world_x: f64,
        world_z: f64,
    ) -> Result<LatitudeSample, SolarInputError> {
        if !world_x.is_finite() {
            return Err(SolarInputError::NonFinite("world_x"));
        }
        if !world_z.is_finite() {
            return Err(SolarInputError::NonFinite("world_z"));
        }
        match self {
            Self::VanillaFixed => Ok(LatitudeSample {
                degrees: 0.0,
                phase: None,
            }),
            Self::McloneCyclicPlane {
                wavelength_blocks,
                phase_origin_z,
            } => {
                if !wavelength_blocks.is_finite() || wavelength_blocks <= 0.0 {
                    return Err(SolarInputError::InvalidPositive("wavelength_blocks"));
                }
                if !phase_origin_z.is_finite() {
                    return Err(SolarInputError::NonFinite("phase_origin_z"));
                }
                // Reduce before taking sin so huge finite coordinates retain a
                // stable bounded argument instead of losing turns in TAU*z.
                let phase = ((world_z - phase_origin_z) / wavelength_blocks).rem_euclid(1.0);
                Ok(LatitudeSample {
                    degrees: 90.0 * (TAU * phase).sin(),
                    phase: Some(phase),
                })
            }
            Self::McloneAsymptoticCylinder {
                climate_scale_blocks,
            } => {
                if !climate_scale_blocks.is_finite() || climate_scale_blocks <= 0.0 {
                    return Err(SolarInputError::InvalidPositive("climate_scale_blocks"));
                }
                Ok(LatitudeSample {
                    degrees: 90.0 * (world_z / climate_scale_blocks).tanh(),
                    phase: None,
                })
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatitudeSample {
    pub degrees: f64,
    /// Wrapped plane phase when the selected policy is cyclical.
    pub phase: Option<f64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolarState {
    Normal,
    PolarDay,
    PolarNight,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarInput {
    pub orbital_phase: OrbitalPhase,
    pub effective_latitude_degrees: f64,
    pub axial_tilt_degrees: f64,
    /// Conventional local solar time in turns: midnight=0, noon=0.5.
    pub solar_time_fraction: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarSample {
    /// Observer-to-sun direction in world axes: +X east, +Y up, +Z south.
    pub direction: [f32; 3],
    pub elevation_degrees: f32,
    /// Clockwise from world north (-Z), in `[0, 360)` degrees.
    pub azimuth_degrees: f32,
    pub declination_degrees: f32,
    pub daylight_factor: f32,
    pub twilight_factor: f32,
    pub day_length_fraction: f32,
    pub polar_state: PolarState,
}

impl SolarSample {
    pub fn compute(input: SolarInput) -> Result<Self, SolarInputError> {
        validate_finite(
            "effective_latitude_degrees",
            input.effective_latitude_degrees,
        )?;
        validate_finite("axial_tilt_degrees", input.axial_tilt_degrees)?;
        validate_finite("solar_time_fraction", input.solar_time_fraction)?;
        if !(-90.0..=90.0).contains(&input.effective_latitude_degrees) {
            return Err(SolarInputError::OutOfRange("effective_latitude_degrees"));
        }
        if !(0.0..90.0).contains(&input.axial_tilt_degrees) {
            return Err(SolarInputError::OutOfRange("axial_tilt_degrees"));
        }

        let latitude = input.effective_latitude_degrees.to_radians();
        let declination =
            input.axial_tilt_degrees.to_radians() * (TAU * input.orbital_phase.turns()).sin();
        let solar_time = input.solar_time_fraction.rem_euclid(1.0);
        let hour_angle = TAU * (solar_time - 0.5);
        let (sin_latitude, cos_latitude) = latitude.sin_cos();
        let (sin_declination, cos_declination) = declination.sin_cos();
        let (sin_hour, cos_hour) = hour_angle.sin_cos();

        let east = -cos_declination * sin_hour;
        let north = cos_latitude * sin_declination - sin_latitude * cos_declination * cos_hour;
        let up = sin_latitude * sin_declination + cos_latitude * cos_declination * cos_hour;
        let length = (east * east + north * north + up * up).sqrt();
        if !length.is_finite() || length <= f64::EPSILON {
            return Err(SolarInputError::DegenerateDirection);
        }
        let east = east / length;
        let north = north / length;
        let up = up / length;
        let elevation = up.clamp(-1.0, 1.0).asin();
        let azimuth = east.atan2(north).rem_euclid(TAU);

        let daily_center = sin_latitude * sin_declination;
        let daily_radius = cos_latitude * cos_declination;
        let daily_max = daily_center + daily_radius;
        let daily_min = daily_center - daily_radius;
        let epsilon = 1.0e-12;
        let (polar_state, day_length_fraction) = if daily_min > epsilon {
            (PolarState::PolarDay, 1.0)
        } else if daily_max < -epsilon {
            (PolarState::PolarNight, 0.0)
        } else if daily_radius <= epsilon {
            // Equinox at the exact pole: the geometric sun rides the horizon.
            (PolarState::Normal, 0.5)
        } else {
            let sunset_hour_angle = (-daily_center / daily_radius).clamp(-1.0, 1.0).acos();
            (PolarState::Normal, sunset_hour_angle / PI)
        };

        let elevation_degrees = elevation.to_degrees();
        let daylight_factor = smoothstep(-6.0, 3.0, elevation_degrees);
        let twilight_factor = smoothstep(-12.0, -3.0, elevation_degrees)
            * (1.0 - smoothstep(3.0, 12.0, elevation_degrees));
        let direction = [east as f32, up as f32, (-north) as f32];
        if !direction.iter().all(|component| component.is_finite()) {
            return Err(SolarInputError::DegenerateDirection);
        }

        Ok(Self {
            direction,
            elevation_degrees: elevation_degrees as f32,
            azimuth_degrees: azimuth.to_degrees() as f32,
            declination_degrees: declination.to_degrees() as f32,
            daylight_factor: daylight_factor as f32,
            twilight_factor: twilight_factor as f32,
            day_length_fraction: day_length_fraction as f32,
            polar_state,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolarInputError {
    NonFinite(&'static str),
    InvalidPositive(&'static str),
    OutOfRange(&'static str),
    DegenerateDirection,
}

impl std::fmt::Display for SolarInputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFinite(field) => write!(formatter, "{field} must be finite"),
            Self::InvalidPositive(field) => {
                write!(formatter, "{field} must be finite and positive")
            }
            Self::OutOfRange(field) => write!(formatter, "{field} is outside its supported range"),
            Self::DegenerateDirection => {
                formatter.write_str("solar direction is not finite and normalized")
            }
        }
    }
}

impl std::error::Error for SolarInputError {}

/// Convert replicated vanilla day ticks to conventional linear solar time.
pub fn solar_time_fraction_from_day_time(day_time: u64) -> f64 {
    ((day_time % mclone_core::time::DAY_LENGTH_TICKS) as f64
        / mclone_core::time::DAY_LENGTH_TICKS as f64
        + 0.25)
        .rem_euclid(1.0)
}

pub fn solar_time_hours_from_day_time(day_time: u64) -> f64 {
    solar_time_fraction_from_day_time(day_time) * 24.0
}

fn validate_finite(field: &'static str, value: f64) -> Result<(), SolarInputError> {
    value
        .is_finite()
        .then_some(())
        .ok_or(SolarInputError::NonFinite(field))
}

fn smoothstep(edge0: f64, edge1: f64, value: f64) -> f64 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    fn sample(latitude: f64, phase: OrbitalPhase, solar_hours: f64) -> SolarSample {
        SolarSample::compute(SolarInput {
            orbital_phase: phase,
            effective_latitude_degrees: latitude,
            axial_tilt_degrees: MCLONE_AXIAL_TILT_DEGREES,
            solar_time_fraction: solar_hours / 24.0,
        })
        .unwrap()
    }

    #[test]
    fn plane_landmarks_and_periodicity_match_the_selected_wave() {
        let policy = SolarCoordinatePolicy::MCLONE_PLANE;
        let wavelength = MCLONE_PLANE_LATITUDE_WAVELENGTH_BLOCKS;
        let expected = [
            (0.0, 0.0),
            (0.25, 90.0),
            (0.5, 0.0),
            (0.75, -90.0),
            (1.0, 0.0),
        ];
        for (phase, latitude) in expected {
            let position = MCLONE_PLANE_LATITUDE_PHASE_ORIGIN_Z + wavelength * phase;
            close(
                policy.latitude_at(123.0, position).unwrap().degrees,
                latitude,
                1.0e-9,
            );
        }
        for z in [-1.0e9, -12_345.5, 0.0, 98_304.25, 1.0e9] {
            let first = policy.latitude_at(-77.0, z).unwrap().degrees;
            let second = policy
                .latitude_at(99_000.0, z + wavelength)
                .unwrap()
                .degrees;
            close(first, second, 2.0e-9);
        }
    }

    #[test]
    fn plane_is_continuous_and_equal_on_opposite_polar_shoulders() {
        let policy = SolarCoordinatePolicy::MCLONE_PLANE;
        let wavelength = MCLONE_PLANE_LATITUDE_WAVELENGTH_BLOCKS;
        let crest = wavelength * 0.25;
        let left = policy.latitude_at(0.0, crest - 100.0).unwrap().degrees;
        let right = policy.latitude_at(0.0, crest + 100.0).unwrap().degrees;
        close(left, right, 1.0e-10);
        let immediate_left = policy.latitude_at(0.0, crest - 0.01).unwrap().degrees;
        let immediate_right = policy.latitude_at(0.0, crest + 0.01).unwrap().degrees;
        assert!((immediate_left - immediate_right).abs() < 1.0e-9);
        assert!(immediate_left <= 90.0 && immediate_right <= 90.0);
    }

    #[test]
    fn equal_latitudes_produce_equal_solar_samples_without_frame_flip() {
        let policy = SolarCoordinatePolicy::MCLONE_PLANE;
        let wavelength = MCLONE_PLANE_LATITUDE_WAVELENGTH_BLOCKS;
        let left = policy.latitude_at(0.0, wavelength * 0.125).unwrap();
        let right = policy.latitude_at(0.0, wavelength * 0.375).unwrap();
        close(left.degrees, right.degrees, 1.0e-10);
        let left_sun = sample(left.degrees, OrbitalPhase::NORTHERN_SOLSTICE, 15.0);
        let right_sun = sample(right.degrees, OrbitalPhase::NORTHERN_SOLSTICE, 15.0);
        assert_eq!(left_sun, right_sun);
    }

    #[test]
    fn cylinder_is_x_seam_invariant_and_asymptotically_bounded() {
        let policy = SolarCoordinatePolicy::MCLONE_CYLINDER;
        assert_eq!(
            policy.latitude_at(0.0, 321.0).unwrap(),
            policy.latitude_at(6_144.0, 321.0).unwrap()
        );
        let north = policy.latitude_at(0.0, 1.0e9).unwrap().degrees;
        let south = policy.latitude_at(0.0, -1.0e9).unwrap().degrees;
        assert!(north <= 90.0 && north > 89.999);
        assert!(south >= -90.0 && south < -89.999);
    }

    #[test]
    fn replicated_day_time_maps_to_linear_solar_clock_anchors() {
        for (ticks, hours) in [(0, 6.0), (6_000, 12.0), (12_000, 18.0), (18_000, 0.0)] {
            close(solar_time_hours_from_day_time(ticks), hours, 1.0e-12);
        }
    }

    #[test]
    fn equinox_day_length_is_half_at_representative_latitudes() {
        for latitude in [-75.0, -45.0, 0.0, 45.0, 75.0] {
            let sun = sample(latitude, OrbitalPhase::NORTHWARD_EQUINOX, 12.0);
            close(f64::from(sun.day_length_fraction), 0.5, 1.0e-6);
            assert_eq!(sun.polar_state, PolarState::Normal);
        }
    }

    #[test]
    fn opposite_hemispheres_have_opposite_solstice_forcing() {
        let north_summer = sample(45.0, OrbitalPhase::NORTHERN_SOLSTICE, 12.0);
        let north_winter = sample(45.0, OrbitalPhase::SOUTHERN_SOLSTICE, 12.0);
        let south_summer = sample(-45.0, OrbitalPhase::SOUTHERN_SOLSTICE, 12.0);
        let south_winter = sample(-45.0, OrbitalPhase::NORTHERN_SOLSTICE, 12.0);
        assert!(north_summer.elevation_degrees > north_winter.elevation_degrees);
        assert!(north_summer.day_length_fraction > north_winter.day_length_fraction);
        close(
            f64::from(north_summer.elevation_degrees),
            f64::from(south_summer.elevation_degrees),
            1.0e-5,
        );
        close(
            f64::from(north_winter.day_length_fraction),
            f64::from(south_winter.day_length_fraction),
            1.0e-5,
        );
    }

    #[test]
    fn solstices_produce_polar_day_and_night() {
        let day = sample(80.0, OrbitalPhase::NORTHERN_SOLSTICE, 0.0);
        let night = sample(80.0, OrbitalPhase::SOUTHERN_SOLSTICE, 12.0);
        assert_eq!(day.polar_state, PolarState::PolarDay);
        assert_eq!(day.day_length_fraction, 1.0);
        assert!(day.elevation_degrees > 0.0);
        assert_eq!(night.polar_state, PolarState::PolarNight);
        assert_eq!(night.day_length_fraction, 0.0);
        assert!(night.elevation_degrees < 0.0);
    }

    #[test]
    fn directions_are_normalized_and_finite_across_a_dense_matrix() {
        for latitude in (-90..=90).step_by(3) {
            for phase in (0..ORBITAL_PHASE_STEPS).step_by(197) {
                for minute in (0..1_440).step_by(37) {
                    let sun = SolarSample::compute(SolarInput {
                        orbital_phase: OrbitalPhase::from_steps_wrapped(phase),
                        effective_latitude_degrees: f64::from(latitude),
                        axial_tilt_degrees: MCLONE_AXIAL_TILT_DEGREES,
                        solar_time_fraction: f64::from(minute) / 1_440.0,
                    })
                    .unwrap();
                    let length = sun
                        .direction
                        .iter()
                        .map(|value| value * value)
                        .sum::<f32>()
                        .sqrt();
                    assert!(
                        (length - 1.0).abs() < 2.0e-6,
                        "length={length}, sun={sun:?}"
                    );
                    assert!(sun.elevation_degrees.is_finite());
                    assert!(sun.azimuth_degrees.is_finite());
                    assert!(sun.declination_degrees.is_finite());
                    assert!((0.0..=1.0).contains(&sun.daylight_factor));
                    assert!((0.0..=1.0).contains(&sun.twilight_factor));
                    assert!((0.0..=1.0).contains(&sun.day_length_fraction));
                }
            }
        }
    }

    #[test]
    fn orbital_wrap_is_fixed_point_and_continuous_at_slider_resolution() {
        assert_eq!(
            OrbitalPhase::from_turns_wrapped(1.0).unwrap(),
            OrbitalPhase::NORTHWARD_EQUINOX
        );
        assert_eq!(
            OrbitalPhase::from_turns_wrapped(-0.25).unwrap(),
            OrbitalPhase::SOUTHERN_SOLSTICE
        );
        let before = sample(45.0, OrbitalPhase::from_steps_wrapped(9_999), 9.0);
        let after = sample(45.0, OrbitalPhase::from_steps_wrapped(0), 9.0);
        assert!((before.elevation_degrees - after.elevation_degrees).abs() < 0.03);
    }

    #[test]
    fn invalid_inputs_are_rejected_without_nan_outputs() {
        assert!(
            SolarCoordinatePolicy::McloneCyclicPlane {
                wavelength_blocks: 0.0,
                phase_origin_z: 0.0,
            }
            .latitude_at(0.0, 0.0)
            .is_err()
        );
        assert!(
            SolarCoordinatePolicy::MCLONE_PLANE
                .latitude_at(f64::NAN, 0.0)
                .is_err()
        );
        assert!(OrbitalPhase::from_turns_wrapped(f64::INFINITY).is_err());
        assert!(
            SolarSample::compute(SolarInput {
                orbital_phase: OrbitalPhase::NORTHWARD_EQUINOX,
                effective_latitude_degrees: 91.0,
                axial_tilt_degrees: MCLONE_AXIAL_TILT_DEGREES,
                solar_time_fraction: 0.5,
            })
            .is_err()
        );
    }
}
