/// Shared player-facing quality bound for the procedural terrain horizon.
///
/// Preset semantics are platform-independent. Platform profiles may choose a
/// different preset only when no explicit player or launch preference exists.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TerrainLodPreset {
    #[default]
    Off,
    Low,
    Medium,
    High,
}

impl TerrainLodPreset {
    pub const ALL: [Self; 4] = [Self::Off, Self::Low, Self::Medium, Self::High];

    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::Low,
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::Off,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    pub const fn startup_label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub const fn horizon_enabled(self) -> bool {
        !matches!(self, Self::Off)
    }

    pub fn parse_startup_label(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" | "exact-only" | "exact" | "false" => Ok(Self::Off),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" | "composed" | "horizon" | "on" | "true" => Ok(Self::High),
            _ => Err(format!(
                "terrain LOD quality must be off, low, medium, or high, got `{value}`"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_cycle_and_parse_with_legacy_aliases() {
        assert_eq!(TerrainLodPreset::Off.next(), TerrainLodPreset::Low);
        assert_eq!(TerrainLodPreset::Low.next(), TerrainLodPreset::Medium);
        assert_eq!(TerrainLodPreset::Medium.next(), TerrainLodPreset::High);
        assert_eq!(TerrainLodPreset::High.next(), TerrainLodPreset::Off);

        for preset in TerrainLodPreset::ALL {
            assert_eq!(
                TerrainLodPreset::parse_startup_label(preset.startup_label()).unwrap(),
                preset
            );
        }
        assert_eq!(
            TerrainLodPreset::parse_startup_label("exact-only").unwrap(),
            TerrainLodPreset::Off
        );
        assert_eq!(
            TerrainLodPreset::parse_startup_label("composed").unwrap(),
            TerrainLodPreset::High
        );
    }
}
