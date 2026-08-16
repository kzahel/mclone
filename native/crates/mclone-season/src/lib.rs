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

/// Fixed-point resolution for the client-local lunar presentation cycle.
pub const LUNAR_PHASE_STEPS: u16 = 10_000;

/// Provisional visual-only lunar cycle: 29.5 ordinary Minecraft days.
pub const LUNAR_SYNODIC_PERIOD_TICKS: u64 = mclone_core::time::DAY_LENGTH_TICKS * 59 / 2;

/// Restrained inclination keeps conjunctions from reading as monthly eclipses.
pub const MCLONE_LUNAR_ORBIT_INCLINATION_DEGREES: f64 = 5.0;

/// Synthetic Debug-only year length selected by Tactical 306.
///
/// This is a display projection over [`OrbitalPhase`], not an authoritative
/// calendar or persistence contract.
pub const PREVIEW_CALENDAR_DAYS: u16 = 112;

/// Hard-bounded radius for the one client-local recent-snow preview pulse.
pub const RECENT_SNOW_RADIUS_BLOCKS: u16 = 96;

pub const PREVIEW_LATITUDE_TENTHS_PER_DEGREE: i16 = 10;
pub const PREVIEW_SOLAR_TIME_MINUTES_PER_DAY: u16 = 1_440;

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

/// A normalized synodic lunar turn stored without floating-point drift.
///
/// Zero is New Moon, one half turn is Full Moon, and the value wraps before
/// one full turn. It is presentation vocabulary rather than saved world state.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct LunarPhase(u16);

impl LunarPhase {
    pub const NEW: Self = Self(0);
    pub const FIRST_QUARTER: Self = Self(LUNAR_PHASE_STEPS / 4);
    pub const FULL: Self = Self(LUNAR_PHASE_STEPS / 2);
    pub const LAST_QUARTER: Self = Self(LUNAR_PHASE_STEPS * 3 / 4);

    pub const fn from_steps_wrapped(steps: u16) -> Self {
        Self(steps % LUNAR_PHASE_STEPS)
    }

    pub fn from_turns_wrapped(turns: f64) -> Result<Self, SolarInputError> {
        if !turns.is_finite() {
            return Err(SolarInputError::NonFinite("lunar_phase"));
        }
        let steps = (turns.rem_euclid(1.0) * f64::from(LUNAR_PHASE_STEPS)).round() as u16;
        Ok(Self::from_steps_wrapped(steps))
    }

    /// Derive the provisional visual cycle without ticking or persisting it.
    pub fn from_day_time(day_time: u64) -> Self {
        let tick = day_time % LUNAR_SYNODIC_PERIOD_TICKS;
        let steps = tick * u64::from(LUNAR_PHASE_STEPS) / LUNAR_SYNODIC_PERIOD_TICKS;
        Self::from_steps_wrapped(steps as u16)
    }

    pub const fn steps(self) -> u16 {
        self.0
    }

    pub fn turns(self) -> f64 {
        f64::from(self.0) / f64::from(LUNAR_PHASE_STEPS)
    }

    pub const fn named_phase(self) -> LunarPhaseLabel {
        let eighth = LUNAR_PHASE_STEPS / 8;
        match ((self.0 + eighth / 2) / eighth) % 8 {
            0 => LunarPhaseLabel::NewMoon,
            1 => LunarPhaseLabel::WaxingCrescent,
            2 => LunarPhaseLabel::FirstQuarter,
            3 => LunarPhaseLabel::WaxingGibbous,
            4 => LunarPhaseLabel::FullMoon,
            5 => LunarPhaseLabel::WaningGibbous,
            6 => LunarPhaseLabel::LastQuarter,
            _ => LunarPhaseLabel::WaningCrescent,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LunarPhaseLabel {
    NewMoon,
    WaxingCrescent,
    FirstQuarter,
    WaxingGibbous,
    FullMoon,
    WaningGibbous,
    LastQuarter,
    WaningCrescent,
}

impl LunarPhaseLabel {
    pub const fn label(self) -> &'static str {
        match self {
            Self::NewMoon => "New Moon",
            Self::WaxingCrescent => "Waxing Crescent",
            Self::FirstQuarter => "First Quarter",
            Self::WaxingGibbous => "Waxing Gibbous",
            Self::FullMoon => "Full Moon",
            Self::WaningGibbous => "Waning Gibbous",
            Self::LastQuarter => "Last Quarter",
            Self::WaningCrescent => "Waning Crescent",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum MoonPhaseSource {
    #[default]
    WorldClock,
    ManualPreview,
}

impl MoonPhaseSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::WorldClock => "World Clock",
            Self::ManualPreview => "Manual Preview",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::WorldClock => Self::ManualPreview,
            Self::ManualPreview => Self::WorldClock,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum CelestialStarDensity {
    Off,
    Quarter,
    Half,
    #[default]
    Full,
}

impl CelestialStarDensity {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Quarter => "Quarter",
            Self::Half => "Half",
            Self::Full => "Full",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::Quarter,
            Self::Quarter => Self::Half,
            Self::Half => Self::Full,
            Self::Full => Self::Off,
        }
    }

    pub const fn selected_count(self, full_count: u32) -> u32 {
        match self {
            Self::Off => 0,
            Self::Quarter => full_count.div_ceil(4),
            Self::Half => full_count.div_ceil(2),
            Self::Full => full_count,
        }
    }
}

/// Client-local celestial review and cost-isolation settings.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CelestialDebugSettings {
    pub sun_body_enabled: bool,
    pub sun_halo_enabled: bool,
    pub horizon_glow_enabled: bool,
    pub moon_body_enabled: bool,
    pub moonlight_enabled: bool,
    pub moon_phase_source: MoonPhaseSource,
    pub manual_lunar_phase: LunarPhase,
    pub star_density: CelestialStarDensity,
}

impl Default for CelestialDebugSettings {
    fn default() -> Self {
        Self {
            sun_body_enabled: true,
            sun_halo_enabled: true,
            horizon_glow_enabled: true,
            moon_body_enabled: true,
            moonlight_enabled: true,
            moon_phase_source: MoonPhaseSource::WorldClock,
            manual_lunar_phase: LunarPhase::NEW,
            star_density: CelestialStarDensity::Full,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PreviewCalendarDate {
    day: u16,
}

impl PreviewCalendarDate {
    pub const fn from_orbital_phase(phase: OrbitalPhase) -> Self {
        let day = (phase.steps() as u32 * PREVIEW_CALENDAR_DAYS as u32 / ORBITAL_PHASE_STEPS as u32)
            as u16
            + 1;
        Self { day }
    }

    pub const fn day(self) -> u16 {
        self.day
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OrbitalMilestone {
    NorthwardEquinox,
    NorthernSolstice,
    SouthwardEquinox,
    SouthernSolstice,
}

impl OrbitalMilestone {
    pub const fn nearest(phase: OrbitalPhase) -> Self {
        let quarter_steps = ORBITAL_PHASE_STEPS / 4;
        let rounded = (phase.steps() + quarter_steps / 2) / quarter_steps;
        match rounded % 4 {
            0 => Self::NorthwardEquinox,
            1 => Self::NorthernSolstice,
            2 => Self::SouthwardEquinox,
            _ => Self::SouthernSolstice,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::NorthwardEquinox => "Northward Equinox",
            Self::NorthernSolstice => "Northern Solstice",
            Self::SouthwardEquinox => "Southward Equinox",
            Self::SouthernSolstice => "Southern Solstice",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LocalSeasonLabel {
    Spring,
    Summer,
    Autumn,
    Winter,
    WeakThermalCycle,
}

impl LocalSeasonLabel {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Spring => "Spring",
            Self::Summer => "Summer",
            Self::Autumn => "Autumn",
            Self::Winter => "Winter",
            Self::WeakThermalCycle => "Weak Thermal Cycle",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalSeasonInput {
    pub orbital_phase: OrbitalPhase,
    pub effective_latitude_degrees: f64,
    /// Normalized annual-mean thermal character in `[-1, 1]`.
    pub mean_temperature: f32,
    /// Normalized local moisture in `[0, 1]`.
    pub moisture: f32,
    pub altitude_blocks: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvaluatedLocalSeason {
    pub local_phase: f32,
    pub response_strength: f32,
    pub thermal_forcing: f32,
    pub current_temperature: f32,
    pub snow_tendency: f32,
    pub day_length_fraction: f32,
    pub label: LocalSeasonLabel,
}

impl EvaluatedLocalSeason {
    pub fn evaluate(input: LocalSeasonInput) -> Self {
        let latitude = finite_f64_or(input.effective_latitude_degrees, 0.0).clamp(-90.0, 90.0);
        let mean_temperature = finite_f32_or(input.mean_temperature, 0.0).clamp(-1.0, 1.0);
        let moisture = finite_f32_or(input.moisture, 0.5).clamp(0.0, 1.0);
        let altitude = finite_f32_or(input.altitude_blocks, 64.0).clamp(-2_048.0, 4_096.0);
        let altitude_cooling = ((altitude - 80.0) / 160.0).clamp(0.0, 1.0) * 0.8;
        let annual_temperature = (mean_temperature - altitude_cooling).clamp(-1.0, 1.0);
        let latitude_strength = smoothstep(8.0, 45.0, latitude.abs()) as f32;
        let warm_suppression =
            1.0 - smoothstep(0.55, 0.95, f64::from(annual_temperature)) as f32 * 0.55;
        let response_strength = (latitude_strength * warm_suppression).clamp(0.0, 1.0);
        let hemisphere_shift = if latitude < 0.0 { 0.5 } else { 0.0 };
        let local_phase = (input.orbital_phase.turns() + hemisphere_shift).rem_euclid(1.0) as f32;
        let latitude_sign = if latitude < 0.0 {
            -1.0
        } else if latitude > 0.0 {
            1.0
        } else {
            0.0
        };
        let thermal_forcing =
            ((TAU * input.orbital_phase.turns()).sin() as f32 * latitude_sign * response_strength)
                .clamp(-1.0, 1.0);
        let current_temperature = (annual_temperature + thermal_forcing * 0.6).clamp(-1.0, 1.0);
        let cold_retention =
            (1.0 - smoothstep(-0.45, 0.18, f64::from(current_temperature)) as f32).clamp(0.0, 1.0);
        let snow_tendency = (cold_retention * (0.45 + moisture * 0.55)).clamp(0.0, 1.0);
        let label = if response_strength < 0.2 {
            LocalSeasonLabel::WeakThermalCycle
        } else {
            match (local_phase * 4.0).floor() as u8 % 4 {
                0 => LocalSeasonLabel::Spring,
                1 => LocalSeasonLabel::Summer,
                2 => LocalSeasonLabel::Autumn,
                _ => LocalSeasonLabel::Winter,
            }
        };
        let day_length_fraction = SolarSample::compute(SolarInput {
            orbital_phase: input.orbital_phase,
            effective_latitude_degrees: latitude,
            axial_tilt_degrees: MCLONE_AXIAL_TILT_DEGREES,
            solar_time_fraction: 0.5,
        })
        .map_or(0.5, |sample| sample.day_length_fraction);

        Self {
            local_phase,
            response_strength,
            thermal_forcing,
            current_temperature,
            snow_tendency,
            day_length_fraction,
            label,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum SeasonalSurfaceFamily {
    #[default]
    Inert = 0,
    NaturalGround = 1,
    Grass = 2,
    DeciduousFoliage = 3,
    EvergreenFoliage = 4,
}

impl SeasonalSurfaceFamily {
    const fn from_bits(bits: u8) -> Self {
        match bits {
            1 => Self::NaturalGround,
            2 => Self::Grass,
            3 => Self::DeciduousFoliage,
            4 => Self::EvergreenFoliage,
            _ => Self::Inert,
        }
    }

    const fn snow_weight(self) -> f32 {
        match self {
            Self::Inert => 0.0,
            Self::NaturalGround => 1.0,
            Self::Grass => 0.9,
            Self::DeciduousFoliage => 0.52,
            Self::EvergreenFoliage => 0.68,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum SeasonalTemperatureClass {
    Cold = 0,
    Cool = 1,
    #[default]
    Mild = 2,
    Warm = 3,
}

impl SeasonalTemperatureClass {
    const fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0 => Self::Cold,
            1 => Self::Cool,
            2 => Self::Mild,
            _ => Self::Warm,
        }
    }

    pub const fn representative(self) -> f32 {
        match self {
            Self::Cold => -0.75,
            Self::Cool => -0.25,
            Self::Mild => 0.25,
            Self::Warm => 0.75,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum SeasonalMoistureClass {
    Arid = 0,
    Dry = 1,
    #[default]
    Moist = 2,
    Wet = 3,
}

impl SeasonalMoistureClass {
    const fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0 => Self::Arid,
            1 => Self::Dry,
            2 => Self::Moist,
            _ => Self::Wet,
        }
    }

    pub const fn representative(self) -> f32 {
        match self {
            Self::Arid => 0.05,
            Self::Dry => 0.30,
            Self::Moist => 0.65,
            Self::Wet => 0.95,
        }
    }
}

/// Eight-bit mesh-time response key stored in packed-light's unused high byte.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct StaticSeasonalResponse {
    pub family: SeasonalSurfaceFamily,
    pub upward_exposed: bool,
    pub temperature: SeasonalTemperatureClass,
    pub moisture: SeasonalMoistureClass,
}

impl StaticSeasonalResponse {
    pub fn from_climate(
        family: SeasonalSurfaceFamily,
        upward_exposed: bool,
        mean_temperature: f32,
        moisture: f32,
        altitude_blocks: f32,
    ) -> Self {
        let temperature = finite_f32_or(mean_temperature, 0.0).clamp(-1.0, 1.0);
        let moisture = finite_f32_or(moisture, 0.5).clamp(0.0, 1.0);
        let altitude = finite_f32_or(altitude_blocks, 64.0).clamp(-2_048.0, 4_096.0);
        let adjusted_temperature =
            (temperature - ((altitude - 80.0) / 160.0).clamp(0.0, 1.0) * 0.8).clamp(-1.0, 1.0);
        let temperature = if adjusted_temperature < -0.4 {
            SeasonalTemperatureClass::Cold
        } else if adjusted_temperature < 0.1 {
            SeasonalTemperatureClass::Cool
        } else if adjusted_temperature < 0.55 {
            SeasonalTemperatureClass::Mild
        } else {
            SeasonalTemperatureClass::Warm
        };
        let moisture = if moisture < 0.15 {
            SeasonalMoistureClass::Arid
        } else if moisture < 0.45 {
            SeasonalMoistureClass::Dry
        } else if moisture < 0.78 {
            SeasonalMoistureClass::Moist
        } else {
            SeasonalMoistureClass::Wet
        };
        Self {
            family,
            upward_exposed,
            temperature,
            moisture,
        }
    }

    pub const fn encode(self) -> u8 {
        (self.family as u8)
            | ((self.upward_exposed as u8) << 3)
            | ((self.temperature as u8) << 4)
            | ((self.moisture as u8) << 6)
    }

    pub const fn decode(value: u8) -> Self {
        Self {
            family: SeasonalSurfaceFamily::from_bits(value & 0b111),
            upward_exposed: value & (1 << 3) != 0,
            temperature: SeasonalTemperatureClass::from_bits(value >> 4),
            moisture: SeasonalMoistureClass::from_bits(value >> 6),
        }
    }

    pub const fn pack_in_light(self, packed_light: u32) -> u32 {
        (packed_light & 0x00ff_ffff) | ((self.encode() as u32) << 24)
    }

    pub const fn unpack_from_light(packed_light: u32) -> Self {
        Self::decode((packed_light >> 24) as u8)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct UnitU16(u16);

impl UnitU16 {
    pub const ZERO: Self = Self(0);
    pub const FULL: Self = Self(u16::MAX);

    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    pub fn from_unit_clamped(value: f32) -> Self {
        let value = finite_f32_or(value, 0.0).clamp(0.0, 1.0);
        Self((value * f32::from(u16::MAX)).round() as u16)
    }

    pub const fn raw(self) -> u16 {
        self.0
    }

    pub fn unit(self) -> f32 {
        f32::from(self.0) / f32::from(u16::MAX)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LocalSnowPulse {
    pub center_x: i32,
    pub center_z: i32,
    pub radius_blocks: u16,
    pub intensity: UnitU16,
}

impl LocalSnowPulse {
    pub fn anchored(
        topology: HorizontalTopology,
        world_x: f64,
        world_z: f64,
        intensity: UnitU16,
    ) -> Self {
        let world_x = finite_f64_or(world_x, 0.0)
            .floor()
            .clamp(i32::MIN as f64, i32::MAX as f64) as i32;
        let world_z = finite_f64_or(world_z, 0.0)
            .floor()
            .clamp(i32::MIN as f64, i32::MAX as f64) as i32;
        Self {
            center_x: topology.x.canonical_block(world_x).unwrap_or(world_x),
            center_z: topology.z.canonical_block(world_z).unwrap_or(world_z),
            radius_blocks: RECENT_SNOW_RADIUS_BLOCKS,
            intensity,
        }
    }

    pub fn coverage_at(self, topology: HorizontalTopology, world_x: f64, world_z: f64) -> f32 {
        if self.radius_blocks == 0 || self.intensity == UnitU16::ZERO {
            return 0.0;
        }
        let world_x = finite_f64_or(world_x, f64::from(self.center_x));
        let world_z = finite_f64_or(world_z, f64::from(self.center_z));
        let dx = topology
            .x
            .shortest_position_displacement(f64::from(self.center_x), world_x);
        let dz = topology
            .z
            .shortest_position_displacement(f64::from(self.center_z), world_z);
        let normalized = (dx.hypot(dz) / f64::from(self.radius_blocks)).clamp(0.0, 1.0);
        let falloff = 1.0 - smoothstep(0.62, 1.0, normalized);
        (falloff as f32 * self.intensity.unit()).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeasonalSurfaceAppearance {
    pub tint: [f32; 3],
    pub seasonal_snow: f32,
    pub recent_snow: f32,
    pub total_snow: f32,
    pub vegetation_visibility: f32,
}

impl SeasonalSurfaceAppearance {
    pub const NEUTRAL: Self = Self {
        tint: [1.0; 3],
        seasonal_snow: 0.0,
        recent_snow: 0.0,
        total_snow: 0.0,
        vegetation_visibility: 1.0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeasonalSurfaceInput {
    pub preview_enabled: bool,
    pub local_season: EvaluatedLocalSeason,
    pub response: StaticSeasonalResponse,
    pub topology: HorizontalTopology,
    pub world_x: f64,
    pub world_z: f64,
    pub recent_snow: Option<LocalSnowPulse>,
}

pub fn evaluate_surface_appearance(input: SeasonalSurfaceInput) -> SeasonalSurfaceAppearance {
    if !input.preview_enabled || input.response.family == SeasonalSurfaceFamily::Inert {
        return SeasonalSurfaceAppearance::NEUTRAL;
    }

    let phase = input.local_season.local_phase.rem_euclid(1.0);
    let weights = [
        landmark_weight(phase, 0.0),
        landmark_weight(phase, 0.25),
        landmark_weight(phase, 0.5),
        landmark_weight(phase, 0.75),
    ];
    let moisture = input.response.moisture.representative();
    let warmth = ((input.response.temperature.representative() + 1.0) * 0.5).clamp(0.0, 1.0);
    let regional_strength =
        (input.local_season.response_strength * (1.0 - warmth * moisture * 0.58)).clamp(0.0, 1.0);
    let targets = seasonal_tint_targets(input.response.family, moisture);
    let target = weighted_color(targets, weights);
    let tint = mix_color([1.0; 3], target, regional_strength);

    let current_temperature = (input.response.temperature.representative()
        + input.local_season.thermal_forcing * 0.6)
        .clamp(-1.0, 1.0);
    let retention =
        (1.0 - smoothstep(-0.45, 0.18, f64::from(current_temperature)) as f32).clamp(0.0, 1.0);
    let snow_weight = if input.response.upward_exposed {
        input.response.family.snow_weight()
    } else {
        0.0
    };
    let seasonal_snow = (weights[3]
        * input.local_season.response_strength
        * retention
        * (0.45 + moisture * 0.55)
        * snow_weight)
        .clamp(0.0, 1.0);
    let recent_snow = input.recent_snow.map_or(0.0, |pulse| {
        pulse.coverage_at(input.topology, input.world_x, input.world_z) * retention * snow_weight
    });
    let total_snow = (seasonal_snow + recent_snow).clamp(0.0, 1.0);
    let dormancy = match input.response.family {
        SeasonalSurfaceFamily::Grass => {
            let temperature = input.response.temperature.representative();
            let temperate_fit = (smoothstep(-0.65, -0.15, f64::from(temperature))
                * (1.0 - smoothstep(0.55, 0.85, f64::from(temperature))))
                as f32;
            let autumn_dormancy =
                weights[2] * regional_strength * temperate_fit * (0.76 - moisture * 0.36);
            let winter_dormancy = weights[3] * regional_strength * 0.28;
            autumn_dormancy + winter_dormancy
        }
        _ => 0.0,
    };
    let snow_suppression = match input.response.family {
        SeasonalSurfaceFamily::Grass => total_snow * 0.86,
        SeasonalSurfaceFamily::DeciduousFoliage | SeasonalSurfaceFamily::EvergreenFoliage => {
            total_snow * 0.08
        }
        _ => 0.0,
    };

    SeasonalSurfaceAppearance {
        tint,
        seasonal_snow,
        recent_snow,
        total_snow,
        vegetation_visibility: (1.0 - dormancy - snow_suppression).clamp(0.0, 1.0),
    }
}

fn landmark_weight(phase: f32, landmark: f32) -> f32 {
    let distance = (phase - landmark + 0.5).rem_euclid(1.0) - 0.5;
    (distance * std::f32::consts::TAU).cos().max(0.0).powi(2)
}

fn seasonal_tint_targets(family: SeasonalSurfaceFamily, moisture: f32) -> [[f32; 3]; 4] {
    match family {
        SeasonalSurfaceFamily::Grass => [
            mix_color([0.98, 1.01, 0.96], [0.88, 1.12, 0.84], moisture),
            [1.0, 1.0, 1.0],
            mix_color([1.22, 0.60, 0.26], [1.05, 0.86, 0.58], moisture),
            [0.72, 0.76, 0.68],
        ],
        SeasonalSurfaceFamily::DeciduousFoliage => [
            [0.90, 1.10, 0.86],
            [1.0, 1.0, 1.0],
            [1.18, 0.58, 0.30],
            [0.64, 0.60, 0.52],
        ],
        SeasonalSurfaceFamily::EvergreenFoliage => [
            [0.96, 1.03, 0.95],
            [1.0, 1.0, 1.0],
            [0.93, 0.95, 0.85],
            [0.70, 0.84, 0.83],
        ],
        SeasonalSurfaceFamily::NaturalGround | SeasonalSurfaceFamily::Inert => [[1.0; 3]; 4],
    }
}

fn weighted_color(colors: [[f32; 3]; 4], weights: [f32; 4]) -> [f32; 3] {
    let total = weights.into_iter().sum::<f32>().max(f32::EPSILON);
    let mut result = [0.0; 3];
    for (color, weight) in colors.into_iter().zip(weights) {
        for channel in 0..3 {
            result[channel] += color[channel] * weight / total;
        }
    }
    result
}

fn mix_color(left: [f32; 3], right: [f32; 3], amount: f32) -> [f32; 3] {
    let amount = amount.clamp(0.0, 1.0);
    [
        left[0] + (right[0] - left[0]) * amount,
        left[1] + (right[1] - left[1]) * amount,
        left[2] + (right[2] - left[2]) * amount,
    ]
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum LatitudeSource {
    #[default]
    World,
    Manual,
}

impl LatitudeSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::World => "World",
            Self::Manual => "Manual",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::World => Self::Manual,
            Self::Manual => Self::World,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum SolarTimeSource {
    #[default]
    WorldClock,
    Manual,
}

impl SolarTimeSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::WorldClock => "World Clock",
            Self::Manual => "Manual",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::WorldClock => Self::Manual,
            Self::Manual => Self::WorldClock,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct PreviewLatitude(i16);

impl PreviewLatitude {
    pub const EQUATOR: Self = Self(0);

    pub const fn from_tenths_clamped(tenths: i16) -> Self {
        Self(if tenths < -900 {
            -900
        } else if tenths > 900 {
            900
        } else {
            tenths
        })
    }

    pub fn from_degrees_clamped(degrees: f64) -> Self {
        let degrees = if degrees.is_finite() { degrees } else { 0.0 };
        Self::from_tenths_clamped((degrees * 10.0).round().clamp(-900.0, 900.0) as i16)
    }

    pub const fn tenths(self) -> i16 {
        self.0
    }

    pub fn degrees(self) -> f64 {
        f64::from(self.0) / f64::from(PREVIEW_LATITUDE_TENTHS_PER_DEGREE)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct PreviewSolarTime(u16);

impl PreviewSolarTime {
    pub const MIDNIGHT: Self = Self(0);
    pub const NOON: Self = Self(PREVIEW_SOLAR_TIME_MINUTES_PER_DAY / 2);

    pub const fn from_minutes_wrapped(minutes: u16) -> Self {
        Self(minutes % PREVIEW_SOLAR_TIME_MINUTES_PER_DAY)
    }

    pub fn from_hours_wrapped(hours: f64) -> Self {
        let hours = if hours.is_finite() { hours } else { 0.0 };
        let minutes = (hours.rem_euclid(24.0) * 60.0).round() as u16;
        Self::from_minutes_wrapped(minutes)
    }

    pub const fn minutes(self) -> u16 {
        self.0
    }

    pub fn hours(self) -> f64 {
        f64::from(self.0) / 60.0
    }

    pub fn fraction(self) -> f64 {
        f64::from(self.0) / f64::from(PREVIEW_SOLAR_TIME_MINUTES_PER_DAY)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeasonPreviewSettings {
    /// Drives the client-only seasonal sun, sky, and daylight sample.
    pub enabled: bool,
    /// Admits exact-terrain tint, vegetation dormancy, and derived snow.
    pub appearance_enabled: bool,
    pub orbital_phase: OrbitalPhase,
    pub latitude_source: LatitudeSource,
    pub manual_latitude: PreviewLatitude,
    pub solar_time_source: SolarTimeSource,
    pub manual_solar_time: PreviewSolarTime,
    pub recent_snow: Option<LocalSnowPulse>,
}

impl Default for SeasonPreviewSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            appearance_enabled: false,
            orbital_phase: OrbitalPhase::NORTHWARD_EQUINOX,
            latitude_source: LatitudeSource::World,
            manual_latitude: PreviewLatitude::EQUATOR,
            solar_time_source: SolarTimeSource::WorldClock,
            manual_solar_time: PreviewSolarTime::NOON,
            recent_snow: None,
        }
    }
}

impl SeasonPreviewSettings {
    /// Whether shared local climate evaluation is needed for either visual
    /// consumer. Neither flag mutates authoritative game or day time.
    pub const fn evaluation_enabled(self) -> bool {
        self.enabled || self.appearance_enabled
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

impl PolarState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::PolarDay => "polar-day",
            Self::PolarNight => "polar-night",
        }
    }
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LunarInput {
    pub orbital_phase: OrbitalPhase,
    pub lunar_phase: LunarPhase,
    pub effective_latitude_degrees: f64,
    /// Conventional local solar time in turns: midnight=0, noon=0.5.
    pub solar_time_fraction: f64,
    pub axial_tilt_degrees: f64,
    pub orbital_inclination_degrees: f64,
    /// Ecliptic node in turns. This remains fixed in the first visual model.
    pub node_phase: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LunarSample {
    /// Observer-to-moon direction in world axes: +X east, +Y up, +Z south.
    pub direction: [f32; 3],
    pub elevation_degrees: f32,
    pub elongation_degrees: f32,
    pub illuminated_fraction: f32,
    pub waxing: bool,
    /// Unit tangent at the moon center pointing toward the illuminating sun.
    pub lit_limb_tangent: [f32; 3],
    pub named_phase: LunarPhaseLabel,
    pub moonlight_factor: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EquatorialCoordinate {
    /// Right ascension in wrapped turns.
    pub right_ascension_turns: f64,
    pub declination_degrees: f64,
}

impl LunarSample {
    pub fn compute(input: LunarInput) -> Result<Self, SolarInputError> {
        validate_finite(
            "effective_latitude_degrees",
            input.effective_latitude_degrees,
        )?;
        validate_finite("solar_time_fraction", input.solar_time_fraction)?;
        validate_finite("axial_tilt_degrees", input.axial_tilt_degrees)?;
        validate_finite(
            "orbital_inclination_degrees",
            input.orbital_inclination_degrees,
        )?;
        validate_finite("node_phase", input.node_phase)?;
        if !(-90.0..=90.0).contains(&input.effective_latitude_degrees) {
            return Err(SolarInputError::OutOfRange("effective_latitude_degrees"));
        }
        if !(0.0..90.0).contains(&input.axial_tilt_degrees) {
            return Err(SolarInputError::OutOfRange("axial_tilt_degrees"));
        }
        if !(-45.0..=45.0).contains(&input.orbital_inclination_degrees) {
            return Err(SolarInputError::OutOfRange("orbital_inclination_degrees"));
        }

        let solar = SolarSample::compute(SolarInput {
            orbital_phase: input.orbital_phase,
            effective_latitude_degrees: input.effective_latitude_degrees,
            axial_tilt_degrees: input.axial_tilt_degrees,
            solar_time_fraction: input.solar_time_fraction,
        })?;
        let obliquity = input.axial_tilt_degrees.to_radians();
        let sun_longitude = TAU * input.orbital_phase.turns();
        let moon_longitude = sun_longitude + TAU * input.lunar_phase.turns();
        let inclination = input.orbital_inclination_degrees.to_radians();
        let moon_latitude = inclination * (moon_longitude - TAU * input.node_phase).sin();
        let moon_equatorial = ecliptic_to_equatorial(moon_longitude, moon_latitude, obliquity);
        let sidereal_angle = local_sidereal_angle_turns(
            input.orbital_phase,
            input.solar_time_fraction,
            input.axial_tilt_degrees,
        )?;
        let direction = equatorial_direction(
            moon_equatorial,
            input.effective_latitude_degrees,
            sidereal_angle,
        )?;
        let moon = direction_f64(direction);
        let sun = direction_f64(solar.direction);
        let dot = dot3(moon, sun).clamp(-1.0, 1.0);
        let elongation_degrees = dot.acos().to_degrees();
        let tangent = subtract3(sun, scale3(moon, dot));
        let lit_limb_tangent = normalize3(tangent).unwrap_or_else(|| {
            let fallback = cross3(moon, [0.0, 1.0, 0.0]);
            normalize3(fallback).unwrap_or([1.0, 0.0, 0.0])
        });
        let phase_angle = TAU * input.lunar_phase.turns();
        let illuminated_fraction = ((1.0 - phase_angle.cos()) * 0.5) as f32;
        let elevation_degrees = moon[1].clamp(-1.0, 1.0).asin().to_degrees() as f32;
        let horizon_visibility = smoothstep(-2.0, 4.0, f64::from(elevation_degrees)) as f32;
        let moonlight_factor =
            (illuminated_fraction.powf(1.35) * horizon_visibility).clamp(0.0, 1.0);

        Ok(Self {
            direction,
            elevation_degrees,
            elongation_degrees: elongation_degrees as f32,
            illuminated_fraction,
            waxing: input.lunar_phase.steps() > 0
                && input.lunar_phase.steps() < LUNAR_PHASE_STEPS / 2,
            lit_limb_tangent: to_f32x3(lit_limb_tangent),
            named_phase: input.lunar_phase.named_phase(),
            moonlight_factor,
        })
    }
}

/// Local sidereal rotation coherent with the accepted solar clock.
pub fn local_sidereal_angle_turns(
    orbital_phase: OrbitalPhase,
    solar_time_fraction: f64,
    axial_tilt_degrees: f64,
) -> Result<f64, SolarInputError> {
    validate_finite("solar_time_fraction", solar_time_fraction)?;
    validate_finite("axial_tilt_degrees", axial_tilt_degrees)?;
    if !(0.0..90.0).contains(&axial_tilt_degrees) {
        return Err(SolarInputError::OutOfRange("axial_tilt_degrees"));
    }
    let longitude = TAU * orbital_phase.turns();
    let obliquity = axial_tilt_degrees.to_radians();
    let sun_right_ascension = (longitude.sin() * obliquity.cos()).atan2(longitude.cos());
    let sun_hour_angle = TAU * (solar_time_fraction.rem_euclid(1.0) - 0.5);
    Ok(((sun_right_ascension + sun_hour_angle) / TAU).rem_euclid(1.0))
}

/// Transform one stable equatorial catalog direction into the local horizon.
pub fn equatorial_direction(
    coordinate: EquatorialCoordinate,
    effective_latitude_degrees: f64,
    local_sidereal_angle_turns: f64,
) -> Result<[f32; 3], SolarInputError> {
    validate_finite("right_ascension_turns", coordinate.right_ascension_turns)?;
    validate_finite("declination_degrees", coordinate.declination_degrees)?;
    validate_finite("effective_latitude_degrees", effective_latitude_degrees)?;
    validate_finite("local_sidereal_angle", local_sidereal_angle_turns)?;
    if !(-90.0..=90.0).contains(&coordinate.declination_degrees) {
        return Err(SolarInputError::OutOfRange("declination_degrees"));
    }
    if !(-90.0..=90.0).contains(&effective_latitude_degrees) {
        return Err(SolarInputError::OutOfRange("effective_latitude_degrees"));
    }

    let latitude = effective_latitude_degrees.to_radians();
    let declination = coordinate.declination_degrees.to_radians();
    let hour_angle =
        TAU * (local_sidereal_angle_turns - coordinate.right_ascension_turns).rem_euclid(1.0);
    let (sin_latitude, cos_latitude) = latitude.sin_cos();
    let (sin_declination, cos_declination) = declination.sin_cos();
    let (sin_hour, cos_hour) = hour_angle.sin_cos();
    let east = -cos_declination * sin_hour;
    let north = cos_latitude * sin_declination - sin_latitude * cos_declination * cos_hour;
    let up = sin_latitude * sin_declination + cos_latitude * cos_declination * cos_hour;
    let direction = [east, up, -north];
    let direction = normalize3(direction).ok_or(SolarInputError::DegenerateDirection)?;
    Ok(to_f32x3(direction))
}

/// Night visibility derived from solar elevation rather than a clock window.
pub fn star_visibility_from_solar_elevation(elevation_degrees: f32) -> f32 {
    if !elevation_degrees.is_finite() {
        return 0.0;
    }
    (1.0 - smoothstep(-12.0, -4.0, f64::from(elevation_degrees))) as f32
}

fn ecliptic_to_equatorial(longitude: f64, latitude: f64, obliquity: f64) -> EquatorialCoordinate {
    let (sin_longitude, cos_longitude) = longitude.sin_cos();
    let (sin_latitude, cos_latitude) = latitude.sin_cos();
    let (sin_obliquity, cos_obliquity) = obliquity.sin_cos();
    let x = cos_latitude * cos_longitude;
    let y = cos_latitude * sin_longitude * cos_obliquity - sin_latitude * sin_obliquity;
    let z = cos_latitude * sin_longitude * sin_obliquity + sin_latitude * cos_obliquity;
    EquatorialCoordinate {
        right_ascension_turns: (y.atan2(x) / TAU).rem_euclid(1.0),
        declination_degrees: z.clamp(-1.0, 1.0).asin().to_degrees(),
    }
}

fn direction_f64(direction: [f32; 3]) -> [f64; 3] {
    [
        f64::from(direction[0]),
        f64::from(direction[1]),
        f64::from(direction[2]),
    ]
}

fn to_f32x3(direction: [f64; 3]) -> [f32; 3] {
    [
        direction[0] as f32,
        direction[1] as f32,
        direction[2] as f32,
    ]
}

fn dot3(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn subtract3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale3(value: [f64; 3], scale: f64) -> [f64; 3] {
    [value[0] * scale, value[1] * scale, value[2] * scale]
}

fn cross3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn normalize3(value: [f64; 3]) -> Option<[f64; 3]> {
    let length = dot3(value, value).sqrt();
    (length.is_finite() && length > f64::EPSILON).then(|| scale3(value, 1.0 / length))
}

/// Exact client-local inputs and output used to reproduce one seasonal frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarFrameDiagnostics {
    pub settings: SeasonPreviewSettings,
    pub policy: SolarCoordinatePolicy,
    pub observer_world_x: f64,
    pub observer_world_z: f64,
    pub observer_world_y: f64,
    pub observer_biome_id: i32,
    pub mean_temperature: f32,
    pub moisture: f32,
    pub world_latitude: LatitudeSample,
    pub effective_latitude_degrees: f64,
    pub solar_time_fraction: f64,
    pub sample: SolarSample,
    pub local_season: EvaluatedLocalSeason,
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

fn finite_f32_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

fn finite_f64_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() { value } else { fallback }
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

    fn moon(latitude: f64, orbital: OrbitalPhase, lunar: LunarPhase, hour: f64) -> LunarSample {
        LunarSample::compute(LunarInput {
            orbital_phase: orbital,
            lunar_phase: lunar,
            effective_latitude_degrees: latitude,
            solar_time_fraction: hour / 24.0,
            axial_tilt_degrees: MCLONE_AXIAL_TILT_DEGREES,
            orbital_inclination_degrees: MCLONE_LUNAR_ORBIT_INCLINATION_DEGREES,
            node_phase: 0.0,
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
    fn preview_settings_are_bounded_typed_and_reset_client_local() {
        assert_eq!(
            SeasonPreviewSettings::default(),
            SeasonPreviewSettings {
                enabled: false,
                appearance_enabled: false,
                orbital_phase: OrbitalPhase::NORTHWARD_EQUINOX,
                latitude_source: LatitudeSource::World,
                manual_latitude: PreviewLatitude::EQUATOR,
                solar_time_source: SolarTimeSource::WorldClock,
                manual_solar_time: PreviewSolarTime::NOON,
                recent_snow: None,
            }
        );
        assert!(!SeasonPreviewSettings::default().evaluation_enabled());
        assert!(
            SeasonPreviewSettings {
                enabled: true,
                ..SeasonPreviewSettings::default()
            }
            .evaluation_enabled()
        );
        assert!(
            SeasonPreviewSettings {
                appearance_enabled: true,
                ..SeasonPreviewSettings::default()
            }
            .evaluation_enabled()
        );
        assert_eq!(
            PreviewLatitude::from_degrees_clamped(-200.0).degrees(),
            -90.0
        );
        assert_eq!(PreviewLatitude::from_degrees_clamped(200.0).degrees(), 90.0);
        assert_eq!(
            PreviewLatitude::from_degrees_clamped(f64::NAN).degrees(),
            0.0
        );
        assert_eq!(PreviewSolarTime::from_hours_wrapped(25.5).minutes(), 90);
        assert_eq!(PreviewSolarTime::from_hours_wrapped(-1.0).minutes(), 1_380);
    }

    fn local(
        latitude: f64,
        phase: OrbitalPhase,
        temperature: f32,
        moisture: f32,
        altitude: f32,
    ) -> EvaluatedLocalSeason {
        EvaluatedLocalSeason::evaluate(LocalSeasonInput {
            orbital_phase: phase,
            effective_latitude_degrees: latitude,
            mean_temperature: temperature,
            moisture,
            altitude_blocks: altitude,
        })
    }

    fn response(
        family: SeasonalSurfaceFamily,
        exposed: bool,
        temperature: SeasonalTemperatureClass,
        moisture: SeasonalMoistureClass,
    ) -> StaticSeasonalResponse {
        StaticSeasonalResponse {
            family,
            upward_exposed: exposed,
            temperature,
            moisture,
        }
    }

    fn appearance(
        local_season: EvaluatedLocalSeason,
        response: StaticSeasonalResponse,
        recent_snow: Option<LocalSnowPulse>,
        world_x: f64,
        world_z: f64,
        topology: HorizontalTopology,
    ) -> SeasonalSurfaceAppearance {
        evaluate_surface_appearance(SeasonalSurfaceInput {
            preview_enabled: true,
            local_season,
            response,
            topology,
            world_x,
            world_z,
            recent_snow,
        })
    }

    #[test]
    fn synthetic_calendar_projects_quarter_days_and_wrap() {
        for (phase, day, milestone) in [
            (
                OrbitalPhase::NORTHWARD_EQUINOX,
                1,
                OrbitalMilestone::NorthwardEquinox,
            ),
            (
                OrbitalPhase::NORTHERN_SOLSTICE,
                29,
                OrbitalMilestone::NorthernSolstice,
            ),
            (
                OrbitalPhase::SOUTHWARD_EQUINOX,
                57,
                OrbitalMilestone::SouthwardEquinox,
            ),
            (
                OrbitalPhase::SOUTHERN_SOLSTICE,
                85,
                OrbitalMilestone::SouthernSolstice,
            ),
        ] {
            assert_eq!(PreviewCalendarDate::from_orbital_phase(phase).day(), day);
            assert_eq!(OrbitalMilestone::nearest(phase), milestone);
        }
        assert_eq!(
            PreviewCalendarDate::from_orbital_phase(OrbitalPhase::from_steps_wrapped(9_999)).day(),
            112
        );
        assert_eq!(
            PreviewCalendarDate::from_orbital_phase(OrbitalPhase::from_steps_wrapped(10_000)).day(),
            1
        );
    }

    #[test]
    fn temperate_hemispheres_evaluate_opposite_local_seasons() {
        for (phase, north, south) in [
            (
                OrbitalPhase::NORTHWARD_EQUINOX,
                LocalSeasonLabel::Spring,
                LocalSeasonLabel::Autumn,
            ),
            (
                OrbitalPhase::NORTHERN_SOLSTICE,
                LocalSeasonLabel::Summer,
                LocalSeasonLabel::Winter,
            ),
            (
                OrbitalPhase::SOUTHWARD_EQUINOX,
                LocalSeasonLabel::Autumn,
                LocalSeasonLabel::Spring,
            ),
            (
                OrbitalPhase::SOUTHERN_SOLSTICE,
                LocalSeasonLabel::Winter,
                LocalSeasonLabel::Summer,
            ),
        ] {
            assert_eq!(local(45.0, phase, 0.1, 0.65, 70.0).label, north);
            assert_eq!(local(-45.0, phase, 0.1, 0.65, 70.0).label, south);
        }
    }

    #[test]
    fn documented_late_winter_dates_remain_in_local_winter() {
        assert_eq!(
            local(
                45.0,
                OrbitalPhase::from_steps_wrapped(8_750),
                0.1,
                0.65,
                70.0,
            )
            .label,
            LocalSeasonLabel::Winter,
        );
        assert_eq!(
            local(
                -45.0,
                OrbitalPhase::from_steps_wrapped(3_750),
                0.1,
                0.65,
                70.0,
            )
            .label,
            LocalSeasonLabel::Winter,
        );
    }

    #[test]
    fn equatorial_and_non_finite_inputs_produce_a_finite_weak_cycle() {
        for latitude in [-5.0, 0.0, 5.0, f64::NAN] {
            let sample = local(
                latitude,
                OrbitalPhase::SOUTHERN_SOLSTICE,
                f32::NAN,
                f32::INFINITY,
                f32::NAN,
            );
            assert_eq!(sample.label, LocalSeasonLabel::WeakThermalCycle);
            assert!(sample.response_strength < 0.2);
            assert!(sample.local_phase.is_finite());
            assert!(sample.current_temperature.is_finite());
            assert!(sample.snow_tendency.is_finite());
            assert!(sample.day_length_fraction.is_finite());
        }
    }

    #[test]
    fn evaluated_day_length_matches_the_accepted_solar_sample() {
        for latitude in [-75.0, -45.0, 0.0, 45.0, 75.0] {
            for phase in [
                OrbitalPhase::NORTHWARD_EQUINOX,
                OrbitalPhase::NORTHERN_SOLSTICE,
                OrbitalPhase::SOUTHERN_SOLSTICE,
            ] {
                let evaluated = local(latitude, phase, 0.0, 0.5, 64.0);
                let solar = sample(latitude, phase, 3.0);
                close(
                    f64::from(evaluated.day_length_fraction),
                    f64::from(solar.day_length_fraction),
                    1.0e-6,
                );
            }
        }
    }

    #[test]
    fn static_response_round_trips_in_packed_lights_unused_high_byte() {
        for family in [
            SeasonalSurfaceFamily::Inert,
            SeasonalSurfaceFamily::NaturalGround,
            SeasonalSurfaceFamily::Grass,
            SeasonalSurfaceFamily::DeciduousFoliage,
            SeasonalSurfaceFamily::EvergreenFoliage,
        ] {
            for exposed in [false, true] {
                for temperature in [
                    SeasonalTemperatureClass::Cold,
                    SeasonalTemperatureClass::Cool,
                    SeasonalTemperatureClass::Mild,
                    SeasonalTemperatureClass::Warm,
                ] {
                    for moisture in [
                        SeasonalMoistureClass::Arid,
                        SeasonalMoistureClass::Dry,
                        SeasonalMoistureClass::Moist,
                        SeasonalMoistureClass::Wet,
                    ] {
                        let response = response(family, exposed, temperature, moisture);
                        assert_eq!(StaticSeasonalResponse::decode(response.encode()), response);
                        let packed = response.pack_in_light(0x00f0_00a0);
                        assert_eq!(packed & 0x00ff_ffff, 0x00f0_00a0);
                        assert_eq!(StaticSeasonalResponse::unpack_from_light(packed), response);
                    }
                }
            }
        }
    }

    #[test]
    fn altitude_and_climate_quantization_distinguish_regional_fixtures() {
        let warm_dry = StaticSeasonalResponse::from_climate(
            SeasonalSurfaceFamily::Grass,
            true,
            0.8,
            0.05,
            68.0,
        );
        let warm_wet = StaticSeasonalResponse::from_climate(
            SeasonalSurfaceFamily::Grass,
            true,
            0.8,
            0.95,
            68.0,
        );
        let temperate = StaticSeasonalResponse::from_climate(
            SeasonalSurfaceFamily::Grass,
            true,
            0.2,
            0.65,
            70.0,
        );
        let cold_high = StaticSeasonalResponse::from_climate(
            SeasonalSurfaceFamily::NaturalGround,
            true,
            0.0,
            0.45,
            180.0,
        );
        assert_eq!(warm_dry.temperature, SeasonalTemperatureClass::Warm);
        assert_eq!(warm_dry.moisture, SeasonalMoistureClass::Arid);
        assert_eq!(warm_wet.moisture, SeasonalMoistureClass::Wet);
        assert_eq!(temperate.temperature, SeasonalTemperatureClass::Mild);
        assert_eq!(cold_high.temperature, SeasonalTemperatureClass::Cold);
    }

    #[test]
    fn regional_surface_matrix_changes_continuously_and_semantically() {
        let landmarks = [
            OrbitalPhase::NORTHWARD_EQUINOX,
            OrbitalPhase::NORTHERN_SOLSTICE,
            OrbitalPhase::SOUTHWARD_EQUINOX,
            OrbitalPhase::SOUTHERN_SOLSTICE,
            OrbitalPhase::from_steps_wrapped(1_250),
            OrbitalPhase::from_steps_wrapped(3_750),
            OrbitalPhase::from_steps_wrapped(6_250),
            OrbitalPhase::from_steps_wrapped(8_750),
        ];
        for phase in landmarks {
            for (temperature, moisture) in [
                (SeasonalTemperatureClass::Warm, SeasonalMoistureClass::Wet),
                (SeasonalTemperatureClass::Warm, SeasonalMoistureClass::Arid),
                (SeasonalTemperatureClass::Mild, SeasonalMoistureClass::Moist),
                (SeasonalTemperatureClass::Cold, SeasonalMoistureClass::Dry),
            ] {
                let appearance = appearance(
                    local(
                        45.0,
                        phase,
                        temperature.representative(),
                        moisture.representative(),
                        70.0,
                    ),
                    response(SeasonalSurfaceFamily::Grass, true, temperature, moisture),
                    None,
                    0.0,
                    0.0,
                    HorizontalTopology::UNBOUNDED,
                );
                assert!(appearance.tint.iter().all(|value| value.is_finite()));
                assert!((0.0..=1.0).contains(&appearance.total_snow));
                assert!((0.0..=1.0).contains(&appearance.vegetation_visibility));
            }
        }

        let autumn = local(45.0, OrbitalPhase::SOUTHWARD_EQUINOX, 0.1, 0.65, 70.0);
        let deciduous = appearance(
            autumn,
            response(
                SeasonalSurfaceFamily::DeciduousFoliage,
                true,
                SeasonalTemperatureClass::Mild,
                SeasonalMoistureClass::Moist,
            ),
            None,
            0.0,
            0.0,
            HorizontalTopology::UNBOUNDED,
        );
        let evergreen = appearance(
            autumn,
            response(
                SeasonalSurfaceFamily::EvergreenFoliage,
                true,
                SeasonalTemperatureClass::Cool,
                SeasonalMoistureClass::Moist,
            ),
            None,
            0.0,
            0.0,
            HorizontalTopology::UNBOUNDED,
        );
        assert!(deciduous.tint[0] > deciduous.tint[1]);
        assert!((evergreen.tint[0] - evergreen.tint[1]).abs() < 0.12);

        for center in [0, 2_500, 5_000, 7_500] {
            let before = local(
                45.0,
                OrbitalPhase::from_steps_wrapped(
                    (center + ORBITAL_PHASE_STEPS - 1) % ORBITAL_PHASE_STEPS,
                ),
                0.1,
                0.65,
                70.0,
            );
            let after = local(
                45.0,
                OrbitalPhase::from_steps_wrapped((center + 1) % ORBITAL_PHASE_STEPS),
                0.1,
                0.65,
                70.0,
            );
            let key = response(
                SeasonalSurfaceFamily::DeciduousFoliage,
                true,
                SeasonalTemperatureClass::Mild,
                SeasonalMoistureClass::Moist,
            );
            let before = appearance(before, key, None, 0.0, 0.0, HorizontalTopology::UNBOUNDED);
            let after = appearance(after, key, None, 0.0, 0.0, HorizontalTopology::UNBOUNDED);
            for channel in 0..3 {
                assert!((before.tint[channel] - after.tint[channel]).abs() < 0.002);
            }
            assert!((before.total_snow - after.total_snow).abs() < 0.002);
        }
    }

    #[test]
    fn temperate_dry_autumn_browns_and_dorms_grass_more_than_wet_or_warm_regions() {
        let phase = OrbitalPhase::SOUTHWARD_EQUINOX;
        let mild_dry = appearance(
            local(45.0, phase, 0.25, 0.30, 70.0),
            response(
                SeasonalSurfaceFamily::Grass,
                true,
                SeasonalTemperatureClass::Mild,
                SeasonalMoistureClass::Dry,
            ),
            None,
            0.0,
            0.0,
            HorizontalTopology::UNBOUNDED,
        );
        let mild_wet = appearance(
            local(45.0, phase, 0.25, 0.95, 70.0),
            response(
                SeasonalSurfaceFamily::Grass,
                true,
                SeasonalTemperatureClass::Mild,
                SeasonalMoistureClass::Wet,
            ),
            None,
            0.0,
            0.0,
            HorizontalTopology::UNBOUNDED,
        );
        let warm_dry = appearance(
            local(45.0, phase, 0.75, 0.30, 70.0),
            response(
                SeasonalSurfaceFamily::Grass,
                true,
                SeasonalTemperatureClass::Warm,
                SeasonalMoistureClass::Dry,
            ),
            None,
            0.0,
            0.0,
            HorizontalTopology::UNBOUNDED,
        );

        assert!(mild_dry.tint[0] > mild_dry.tint[1]);
        assert!(mild_dry.tint[1] < mild_wet.tint[1]);
        assert!(mild_dry.vegetation_visibility < 0.45);
        assert!(mild_wet.vegetation_visibility > mild_dry.vegetation_visibility);
        assert!(warm_dry.vegetation_visibility > mild_dry.vegetation_visibility);
    }

    #[test]
    fn snow_pulse_falloff_is_monotonic_bounded_and_cylinder_seam_safe() {
        let cylinder = HorizontalTopology::cylinder_x(0, 32);
        let half = LocalSnowPulse::anchored(cylinder, 0.0, 0.0, UnitU16::from_unit_clamped(0.5));
        let full = LocalSnowPulse::anchored(cylinder, 0.0, 0.0, UnitU16::FULL);
        assert!(full.coverage_at(cylinder, 0.0, 0.0) > half.coverage_at(cylinder, 0.0, 0.0));
        assert!(full.coverage_at(cylinder, 32.0, 0.0) > full.coverage_at(cylinder, 80.0, 0.0));
        assert_eq!(full.coverage_at(cylinder, 97.0, 0.0), 0.0);
        close(
            f64::from(full.coverage_at(cylinder, -1.0, 0.0)),
            f64::from(full.coverage_at(cylinder, 511.0, 0.0)),
            1.0e-6,
        );
    }

    #[test]
    fn late_winter_recent_snow_dusts_cold_ground_and_canopy_but_not_sides_or_warm_ground() {
        let late_winter = local(
            45.0,
            OrbitalPhase::from_steps_wrapped(8_750),
            -0.2,
            0.65,
            96.0,
        );
        let pulse =
            LocalSnowPulse::anchored(HorizontalTopology::UNBOUNDED, 10.0, 20.0, UnitU16::FULL);
        let cold_ground = appearance(
            late_winter,
            response(
                SeasonalSurfaceFamily::NaturalGround,
                true,
                SeasonalTemperatureClass::Cold,
                SeasonalMoistureClass::Moist,
            ),
            Some(pulse),
            10.0,
            20.0,
            HorizontalTopology::UNBOUNDED,
        );
        let canopy = appearance(
            late_winter,
            response(
                SeasonalSurfaceFamily::EvergreenFoliage,
                true,
                SeasonalTemperatureClass::Cool,
                SeasonalMoistureClass::Moist,
            ),
            Some(pulse),
            10.0,
            20.0,
            HorizontalTopology::UNBOUNDED,
        );
        let side = appearance(
            late_winter,
            response(
                SeasonalSurfaceFamily::EvergreenFoliage,
                false,
                SeasonalTemperatureClass::Cool,
                SeasonalMoistureClass::Moist,
            ),
            Some(pulse),
            10.0,
            20.0,
            HorizontalTopology::UNBOUNDED,
        );
        let warm_ground = appearance(
            late_winter,
            response(
                SeasonalSurfaceFamily::NaturalGround,
                true,
                SeasonalTemperatureClass::Warm,
                SeasonalMoistureClass::Wet,
            ),
            Some(pulse),
            10.0,
            20.0,
            HorizontalTopology::UNBOUNDED,
        );
        assert!(cold_ground.recent_snow > canopy.recent_snow);
        assert!(canopy.recent_snow > 0.0);
        assert_eq!(side.total_snow, 0.0);
        assert!(warm_ground.recent_snow < cold_ground.recent_snow * 0.1);
    }

    #[test]
    fn preview_disabled_is_an_exact_neutral_surface_output() {
        let evaluated = local(45.0, OrbitalPhase::SOUTHERN_SOLSTICE, -0.5, 0.8, 120.0);
        let pulse =
            LocalSnowPulse::anchored(HorizontalTopology::UNBOUNDED, 0.0, 0.0, UnitU16::FULL);
        assert_eq!(
            evaluate_surface_appearance(SeasonalSurfaceInput {
                preview_enabled: false,
                local_season: evaluated,
                response: response(
                    SeasonalSurfaceFamily::DeciduousFoliage,
                    true,
                    SeasonalTemperatureClass::Cold,
                    SeasonalMoistureClass::Wet,
                ),
                topology: HorizontalTopology::UNBOUNDED,
                world_x: 0.0,
                world_z: 0.0,
                recent_snow: Some(pulse),
            }),
            SeasonalSurfaceAppearance::NEUTRAL
        );
    }

    #[test]
    fn mclone_policy_selection_distinguishes_plane_and_opt_in_cylinder() {
        assert_eq!(
            SolarCoordinatePolicy::mclone_for_topology(HorizontalTopology::UNBOUNDED),
            SolarCoordinatePolicy::MCLONE_PLANE,
        );
        assert_eq!(
            SolarCoordinatePolicy::mclone_for_topology(HorizontalTopology::cylinder_x(0, 384)),
            SolarCoordinatePolicy::MCLONE_CYLINDER,
        );
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
    fn lunar_phase_is_fixed_point_named_and_clock_derived() {
        let labels = [
            LunarPhaseLabel::NewMoon,
            LunarPhaseLabel::WaxingCrescent,
            LunarPhaseLabel::FirstQuarter,
            LunarPhaseLabel::WaxingGibbous,
            LunarPhaseLabel::FullMoon,
            LunarPhaseLabel::WaningGibbous,
            LunarPhaseLabel::LastQuarter,
            LunarPhaseLabel::WaningCrescent,
        ];
        for (index, label) in labels.into_iter().enumerate() {
            let steps = index * usize::from(LUNAR_PHASE_STEPS) / 8;
            let phase = LunarPhase::from_steps_wrapped(steps as u16);
            assert_eq!(phase.named_phase(), label);
        }
        assert_eq!(
            LunarPhase::from_turns_wrapped(1.0).unwrap(),
            LunarPhase::NEW
        );
        assert_eq!(LunarPhase::from_day_time(0), LunarPhase::NEW);
        assert_eq!(
            LunarPhase::from_day_time(LUNAR_SYNODIC_PERIOD_TICKS / 2),
            LunarPhase::FULL
        );
        assert_eq!(
            LunarPhase::from_day_time(LUNAR_SYNODIC_PERIOD_TICKS),
            LunarPhase::NEW
        );
    }

    #[test]
    fn lunar_illumination_and_orbit_are_continuous_and_solar_coherent() {
        let new = moon(35.0, OrbitalPhase::NORTHWARD_EQUINOX, LunarPhase::NEW, 12.0);
        let first = moon(
            35.0,
            OrbitalPhase::NORTHWARD_EQUINOX,
            LunarPhase::FIRST_QUARTER,
            12.0,
        );
        let full = moon(
            35.0,
            OrbitalPhase::NORTHWARD_EQUINOX,
            LunarPhase::FULL,
            12.0,
        );
        assert!(new.illuminated_fraction < 1.0e-6);
        assert!((first.illuminated_fraction - 0.5).abs() < 1.0e-6);
        assert!((full.illuminated_fraction - 1.0).abs() < 1.0e-6);
        assert!(new.elongation_degrees < 5.1);
        assert!((first.elongation_degrees - 90.0).abs() < 5.1);
        assert!(full.elongation_degrees > 174.9);
        assert!(first.waxing);
        assert!(!full.waxing);

        for phase in (0..LUNAR_PHASE_STEPS).step_by(137) {
            let sample = moon(
                -48.0,
                OrbitalPhase::from_steps_wrapped(3_100),
                LunarPhase::from_steps_wrapped(phase),
                21.25,
            );
            let direction = direction_f64(sample.direction);
            let tangent = direction_f64(sample.lit_limb_tangent);
            close(dot3(direction, direction), 1.0, 2.0e-6);
            close(dot3(tangent, tangent), 1.0, 2.0e-6);
            close(dot3(direction, tangent), 0.0, 2.0e-5);
            assert!((0.0..=1.0).contains(&sample.illuminated_fraction));
            assert!((0.0..=1.0).contains(&sample.moonlight_factor));
        }
    }

    #[test]
    fn equatorial_catalog_rotates_with_time_latitude_and_orbital_date() {
        let star = EquatorialCoordinate {
            right_ascension_turns: 0.0,
            declination_degrees: 0.0,
        };
        let sidereal_noon = local_sidereal_angle_turns(
            OrbitalPhase::NORTHWARD_EQUINOX,
            0.5,
            MCLONE_AXIAL_TILT_DEGREES,
        )
        .unwrap();
        let sidereal_evening = local_sidereal_angle_turns(
            OrbitalPhase::NORTHWARD_EQUINOX,
            0.75,
            MCLONE_AXIAL_TILT_DEGREES,
        )
        .unwrap();
        let noon = equatorial_direction(star, 0.0, sidereal_noon).unwrap();
        let evening = equatorial_direction(star, 0.0, sidereal_evening).unwrap();
        assert!(dot3(direction_f64(noon), direction_f64(evening)).abs() < 1.0e-5);

        let north_pole = EquatorialCoordinate {
            right_ascension_turns: 0.0,
            declination_degrees: 90.0,
        };
        let equator = equatorial_direction(north_pole, 0.0, 0.37).unwrap();
        let pole = equatorial_direction(north_pole, 90.0, 0.81).unwrap();
        assert!(equator[1].abs() < 1.0e-5);
        assert!(pole[1] > 0.9999);

        let later_date = local_sidereal_angle_turns(
            OrbitalPhase::NORTHERN_SOLSTICE,
            0.5,
            MCLONE_AXIAL_TILT_DEGREES,
        )
        .unwrap();
        assert!((later_date - sidereal_noon).rem_euclid(1.0) > 0.1);
    }

    #[test]
    fn celestial_settings_and_visibility_have_exact_off_baselines() {
        assert_eq!(CelestialStarDensity::Off.selected_count(2_048), 0);
        assert_eq!(CelestialStarDensity::Quarter.selected_count(2_048), 512);
        assert_eq!(CelestialStarDensity::Half.selected_count(2_048), 1_024);
        assert_eq!(CelestialStarDensity::Full.selected_count(2_048), 2_048);
        assert_eq!(star_visibility_from_solar_elevation(4.0), 0.0);
        assert_eq!(star_visibility_from_solar_elevation(-14.0), 1.0);
        let defaults = CelestialDebugSettings::default();
        assert!(defaults.sun_body_enabled);
        assert!(defaults.sun_halo_enabled);
        assert!(defaults.horizon_glow_enabled);
        assert!(defaults.moon_body_enabled);
        assert!(defaults.moonlight_enabled);
        assert_eq!(defaults.star_density, CelestialStarDensity::Full);
        assert_eq!(defaults.moon_phase_source, MoonPhaseSource::WorldClock);
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
        assert!(LunarPhase::from_turns_wrapped(f64::NAN).is_err());
        assert!(
            SolarSample::compute(SolarInput {
                orbital_phase: OrbitalPhase::NORTHWARD_EQUINOX,
                effective_latitude_degrees: 91.0,
                axial_tilt_degrees: MCLONE_AXIAL_TILT_DEGREES,
                solar_time_fraction: 0.5,
            })
            .is_err()
        );
        assert!(
            LunarSample::compute(LunarInput {
                orbital_phase: OrbitalPhase::NORTHWARD_EQUINOX,
                lunar_phase: LunarPhase::NEW,
                effective_latitude_degrees: 0.0,
                solar_time_fraction: 0.5,
                axial_tilt_degrees: MCLONE_AXIAL_TILT_DEGREES,
                orbital_inclination_degrees: f64::INFINITY,
                node_phase: 0.0,
            })
            .is_err()
        );
        assert!(
            equatorial_direction(
                EquatorialCoordinate {
                    right_ascension_turns: 0.0,
                    declination_degrees: 91.0,
                },
                0.0,
                0.0,
            )
            .is_err()
        );
    }
}
