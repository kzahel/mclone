use mclone_season::{EvaluatedLocalSeason, LocalSnowPulse};

const WGSL_MARKER: &str = "// __MCLONE_SEASONAL_APPEARANCE_WGSL__";

/// Observer-local, presentation-only seasonal state consumed by exact terrain
/// and lush grass. Mesh response facts remain stable when this frame state
/// changes, so calendar scrubbing never requires remeshing or reuploading.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SeasonalAppearanceRenderState {
    pub enabled: bool,
    pub local_phase: f32,
    pub response_strength: f32,
    pub thermal_forcing: f32,
    pub recent_snow: Option<LocalSnowPulse>,
}

impl SeasonalAppearanceRenderState {
    pub fn evaluated(
        enabled: bool,
        local_season: EvaluatedLocalSeason,
        recent_snow: Option<LocalSnowPulse>,
    ) -> Self {
        if !enabled {
            return Self::default();
        }
        Self {
            enabled: true,
            local_phase: local_season.local_phase.rem_euclid(1.0),
            response_strength: local_season.response_strength.clamp(0.0, 1.0),
            thermal_forcing: local_season.thermal_forcing.clamp(-1.0, 1.0),
            recent_snow,
        }
    }

    pub(crate) fn uniform_values(self) -> ([f32; 4], [f32; 4]) {
        let local = [
            if self.enabled { 1.0 } else { 0.0 },
            self.local_phase.rem_euclid(1.0),
            self.response_strength.clamp(0.0, 1.0),
            self.thermal_forcing.clamp(-1.0, 1.0),
        ];
        let pulse = self.recent_snow.map_or([0.0; 4], |pulse| {
            [
                pulse.center_x as f32,
                pulse.center_z as f32,
                f32::from(pulse.radius_blocks),
                pulse.intensity.unit(),
            ]
        });
        (local, pulse)
    }
}

pub(crate) fn inject_seasonal_appearance_wgsl(template: &str) -> String {
    assert_eq!(
        template.matches(WGSL_MARKER).count(),
        1,
        "seasonal appearance WGSL template must contain exactly one marker"
    );
    template.replace(
        WGSL_MARKER,
        include_str!("shaders/seasonal_appearance.wgsl"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::HorizontalTopology;
    use mclone_season::{LocalSeasonInput, OrbitalPhase, UnitU16};

    #[test]
    fn disabled_state_is_an_exact_zero_uniform_payload() {
        let evaluated = EvaluatedLocalSeason::evaluate(LocalSeasonInput {
            orbital_phase: OrbitalPhase::SOUTHERN_SOLSTICE,
            effective_latitude_degrees: 60.0,
            mean_temperature: 0.0,
            moisture: 0.8,
            altitude_blocks: 90.0,
        });
        let pulse =
            LocalSnowPulse::anchored(HorizontalTopology::UNBOUNDED, 10.0, 20.0, UnitU16::FULL);
        assert_eq!(
            SeasonalAppearanceRenderState::evaluated(false, evaluated, Some(pulse))
                .uniform_values(),
            ([0.0; 4], [0.0; 4])
        );
    }

    #[test]
    fn shader_source_has_an_explicit_preview_off_identity_path() {
        let source = inject_seasonal_appearance_wgsl(WGSL_MARKER);
        assert!(source.contains("if (season_local.x < 0.5 || family == 0u)"));
        assert!(source.contains("return vec4<f32>(base_color, 1.0);"));
        assert!(source.contains("let response_key = packed_response & 255u;"));
    }

    #[test]
    fn seasonal_shader_module_parses_with_coherent_snow_and_autumn_dormancy() {
        let source = inject_seasonal_appearance_wgsl(WGSL_MARKER);
        naga::front::wgsl::parse_str(&source).expect("seasonal appearance WGSL parses");
        assert!(source.contains("mclone_seasonal_value_noise"));
        assert!(source.contains("let autumn_dormancy"));
    }
}
