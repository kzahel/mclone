const LAND_COAST_CONTINENTALNESS: f64 = 0.16;
const OCEAN_COAST_CONTINENTALNESS: f64 = 0.10;
const SANDY_CHARACTER_MAX: f64 = -0.28;
const GRAVEL_CHARACTER_MAX: f64 = 0.04;
const ORDINARY_CHARACTER_MAX: f64 = 0.30;
const TRANSITION_CHARACTER_WIDTH: f64 = 0.12;
pub(super) const MCLONE_OVERWORLD_DEPOSITIONAL_COAST_MIN_PROXIMITY: f64 = 0.86;
pub(super) const MCLONE_OVERWORLD_ROCKY_COAST_MIN_PROXIMITY: f64 = 0.12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McloneOverworldCoastFamily {
    Offshore,
    Sandy,
    Gravel,
    Ordinary,
    Rocky,
    Inland,
}

impl McloneOverworldCoastFamily {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Offshore => "offshore",
            Self::Sandy => "sandy",
            Self::Gravel => "gravel",
            Self::Ordinary => "ordinary",
            Self::Rocky => "rocky",
            Self::Inland => "inland",
        }
    }

    pub const fn is_coast(self) -> bool {
        matches!(
            self,
            Self::Sandy | Self::Gravel | Self::Ordinary | Self::Rocky
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct McloneOverworldCoastIntent {
    pub family: McloneOverworldCoastFamily,
    /// Continentalness is the current cheap signed coast-distance proxy.
    ///
    /// It is negative on ocean intent and positive on land intent, but it is
    /// not measured in blocks and must not be presented as physical distance.
    pub signed_distance_proxy: f64,
    /// Zero outside the bounded coast band and one at the land/ocean
    /// threshold.
    pub proximity: f64,
    /// Independent broad, topology-aware selector in `[-1, 1]`.
    pub selector: f64,
    /// Terrain-constrained alongshore character used by the family bands.
    pub character: f64,
    pub depositional_suitability: f64,
    pub rocky_suitability: f64,
    /// One at a family boundary and zero away from the nearest boundary.
    pub transition: f64,
    /// Low-altitude cold response independent of the base coast family.
    pub cold_response: f64,
}

impl McloneOverworldCoastIntent {
    pub const OFFSHORE: Self = Self {
        family: McloneOverworldCoastFamily::Offshore,
        signed_distance_proxy: -1.0,
        proximity: 0.0,
        selector: 0.0,
        character: 0.0,
        depositional_suitability: 0.0,
        rocky_suitability: 0.0,
        transition: 0.0,
        cold_response: 0.0,
    };

    pub const INLAND: Self = Self {
        family: McloneOverworldCoastFamily::Inland,
        signed_distance_proxy: 1.0,
        ..Self::OFFSHORE
    };
}

pub(super) fn coast_intent(
    continentalness: f64,
    relief: f64,
    ruggedness: f64,
    ridges: f64,
    temperature: f64,
    selector: f64,
) -> McloneOverworldCoastIntent {
    let proximity = if continentalness >= 0.0 {
        1.0 - smoothstep((continentalness / LAND_COAST_CONTINENTALNESS).clamp(0.0, 1.0))
    } else {
        1.0 - smoothstep((-continentalness / OCEAN_COAST_CONTINENTALNESS).clamp(0.0, 1.0))
    };
    let rugged = smoothstep(((ruggedness + 0.30) / 1.10).clamp(0.0, 1.0));
    let ridge = smoothstep(((ridges - 0.12) / 0.88).clamp(0.0, 1.0));
    let relief_energy = smoothstep(((relief.abs() - 0.04) / 0.72).clamp(0.0, 1.0));
    let rocky_suitability = (rugged * 0.58 + ridge * 0.27 + relief_energy * 0.15).clamp(0.0, 1.0);
    let depositional_suitability =
        (1.0 - rocky_suitability * 0.78 - relief_energy * 0.22).clamp(0.0, 1.0);
    let character =
        (selector * 0.72 + ruggedness * 0.18 + (ridges * 2.0 - 1.0) * 0.07 + relief * 0.03)
            .clamp(-1.0, 1.0);
    let family = if continentalness < -OCEAN_COAST_CONTINENTALNESS {
        McloneOverworldCoastFamily::Offshore
    } else if continentalness > LAND_COAST_CONTINENTALNESS {
        McloneOverworldCoastFamily::Inland
    } else if character <= SANDY_CHARACTER_MAX && depositional_suitability >= 0.22 {
        McloneOverworldCoastFamily::Sandy
    } else if character <= GRAVEL_CHARACTER_MAX {
        McloneOverworldCoastFamily::Gravel
    } else if character <= ORDINARY_CHARACTER_MAX {
        McloneOverworldCoastFamily::Ordinary
    } else if rocky_suitability >= 0.26 {
        McloneOverworldCoastFamily::Rocky
    } else {
        McloneOverworldCoastFamily::Ordinary
    };
    let nearest_boundary = [
        (character - SANDY_CHARACTER_MAX).abs(),
        (character - GRAVEL_CHARACTER_MAX).abs(),
        (character - ORDINARY_CHARACTER_MAX).abs(),
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min);
    let transition =
        1.0 - smoothstep((nearest_boundary / TRANSITION_CHARACTER_WIDTH).clamp(0.0, 1.0));
    let cold_response = smoothstep(((-temperature - 0.18) / 0.42).clamp(0.0, 1.0));

    McloneOverworldCoastIntent {
        family,
        signed_distance_proxy: continentalness,
        proximity,
        selector,
        character,
        depositional_suitability,
        rocky_suitability,
        transition: transition * proximity,
        cold_response: cold_response * proximity,
    }
}

pub(super) fn coast_adjusted_land_surface_y(
    provisional_surface_y: i32,
    intent: McloneOverworldCoastIntent,
    relief: f64,
    ridges: f64,
    mountain_detail: f64,
) -> i32 {
    if !intent.family.is_coast() || intent.proximity == 0.0 {
        return provisional_surface_y;
    }
    let rocky_gate = smoothstep(((intent.character - 0.18) / 0.22).clamp(0.0, 1.0));
    let rocky_support = smoothstep(((intent.rocky_suitability - 0.18) / 0.52).clamp(0.0, 1.0));
    let rocky_influence = rocky_gate * rocky_support * intent.proximity;
    let gravel_rise = if intent.family == McloneOverworldCoastFamily::Gravel {
        intent.proximity * (0.35 + intent.rocky_suitability * 1.65)
    } else {
        0.0
    };
    let ridge_shoulder = smoothstep(((ridges - 0.16) / 0.84).clamp(0.0, 1.0));
    let rocky_lift = 4.0
        + intent.rocky_suitability * 14.0
        + ridge_shoulder * 6.0
        + relief.max(0.0) * 4.0
        + mountain_detail * 3.0;
    let adjustment = (rocky_influence * rocky_lift + gravel_rise).clamp(0.0, 22.0);
    (f64::from(provisional_surface_y) + adjustment)
        .round()
        .clamp(62.0, 160.0) as i32
}

const fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coast_classifier_preserves_all_selected_outcomes() {
        assert_eq!(
            coast_intent(0.04, 0.0, -0.5, 0.1, 0.2, -0.8).family,
            McloneOverworldCoastFamily::Sandy
        );
        assert_eq!(
            coast_intent(0.04, 0.0, 0.0, 0.3, 0.2, -0.1).family,
            McloneOverworldCoastFamily::Gravel
        );
        assert_eq!(
            coast_intent(0.04, 0.0, -0.4, 0.2, 0.2, 0.25).family,
            McloneOverworldCoastFamily::Ordinary
        );
        assert_eq!(
            coast_intent(0.04, 0.3, 0.7, 0.8, 0.2, 0.8).family,
            McloneOverworldCoastFamily::Rocky
        );
        assert_eq!(
            coast_intent(-0.3, 0.0, 0.0, 0.0, 0.0, 0.0).family,
            McloneOverworldCoastFamily::Offshore
        );
        assert_eq!(
            coast_intent(0.3, 0.0, 0.0, 0.0, 0.0, 0.0).family,
            McloneOverworldCoastFamily::Inland
        );
    }

    #[test]
    fn coast_proximity_and_cold_response_are_bounded() {
        for continentalness in [-1.0, -0.1, -0.01, 0.0, 0.01, 0.16, 1.0] {
            let intent = coast_intent(continentalness, 0.2, 0.4, 0.7, -0.8, 0.3);
            assert!((0.0..=1.0).contains(&intent.proximity));
            assert!((0.0..=1.0).contains(&intent.transition));
            assert!((0.0..=1.0).contains(&intent.cold_response));
        }
        assert_eq!(
            coast_intent(0.0, 0.0, 0.0, 0.0, -1.0, 0.0).cold_response,
            1.0
        );
        assert_eq!(
            coast_intent(0.0, 0.0, 0.0, 0.0, 1.0, 0.0).cold_response,
            0.0
        );
    }

    #[test]
    fn coast_geometry_is_bounded_and_only_raises_supported_families() {
        let sandy = coast_intent(0.01, 0.0, -0.5, 0.1, 0.2, -0.8);
        let gravel = coast_intent(0.01, 0.0, 0.0, 0.3, 0.2, -0.1);
        let ordinary = coast_intent(0.01, 0.0, -0.4, 0.2, 0.2, 0.25);
        let rocky = coast_intent(0.01, 0.3, 0.7, 0.8, 0.2, 0.8);

        assert_eq!(coast_adjusted_land_surface_y(64, sandy, 0.0, 0.1, 0.0), 64);
        assert!((64..=66).contains(&coast_adjusted_land_surface_y(64, gravel, 0.0, 0.3, 0.0)));
        assert_eq!(
            coast_adjusted_land_surface_y(64, ordinary, 0.0, 0.2, 0.0),
            64
        );
        let rocky_y = coast_adjusted_land_surface_y(64, rocky, 0.3, 0.8, 0.8);
        assert!((70..=86).contains(&rocky_y), "{rocky_y}");
    }
}
