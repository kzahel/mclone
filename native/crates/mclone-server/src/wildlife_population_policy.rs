use serde::{Deserialize, Serialize};

use crate::WorldGenerationProfile;

/// Selects the sole automatic wildlife producer for one dimension.
///
/// This policy is persisted independently of terrain generation. A generator
/// may provide habitat evidence, but it does not own the creature roster or
/// choose which population producer runs.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WildlifePopulationPolicy {
    Disabled,
    #[default]
    HabitatDrivenV1,
    ReferencePassiveV1,
}

impl WildlifePopulationPolicy {
    pub const fn default_for_profile(profile: WorldGenerationProfile) -> Self {
        match profile {
            WorldGenerationProfile::TopologyProbeV1
            | WorldGenerationProfile::AuthoredOnly { .. } => Self::Disabled,
            WorldGenerationProfile::Overworld
            | WorldGenerationProfile::FlatGrassV1
            | WorldGenerationProfile::SmallIslandV1
            | WorldGenerationProfile::McloneOverworldV1
            | WorldGenerationProfile::McloneOverworldV2
            | WorldGenerationProfile::McloneOverworldV3
            | WorldGenerationProfile::AlphaV1 { .. }
            | WorldGenerationProfile::BetaV1 => Self::HabitatDrivenV1,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::HabitatDrivenV1 => "habitat-driven-v1",
            Self::ReferencePassiveV1 => "reference-passive-v1",
        }
    }

    pub fn parse_label(value: &str) -> Result<Self, String> {
        match value.trim() {
            "disabled" => Ok(Self::Disabled),
            "habitat-driven-v1" | "habitat_driven_v1" | "habitatDrivenV1" => {
                Ok(Self::HabitatDrivenV1)
            }
            "reference-passive-v1" | "reference_passive_v1" | "referencePassiveV1" => {
                Ok(Self::ReferencePassiveV1)
            }
            value => Err(format!(
                "wildlife population policy must be disabled, habitat-driven-v1, or reference-passive-v1, got `{value}`"
            )),
        }
    }

    pub(crate) const fn codec_tag(self) -> u8 {
        match self {
            Self::Disabled => 0,
            Self::HabitatDrivenV1 => 1,
            Self::ReferencePassiveV1 => 2,
        }
    }

    pub(crate) const fn from_codec_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Disabled),
            1 => Some(Self::HabitatDrivenV1),
            2 => Some(Self::ReferencePassiveV1),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_profiles_default_to_habitat_driven_population() {
        for profile in [
            WorldGenerationProfile::McloneOverworldV1,
            WorldGenerationProfile::McloneOverworldV2,
            WorldGenerationProfile::McloneOverworldV3,
            WorldGenerationProfile::Overworld,
            WorldGenerationProfile::FlatGrassV1,
            WorldGenerationProfile::SmallIslandV1,
            WorldGenerationProfile::alpha_v1(false),
            WorldGenerationProfile::alpha_v1(true),
            WorldGenerationProfile::BetaV1,
        ] {
            assert_eq!(
                WildlifePopulationPolicy::default_for_profile(profile),
                WildlifePopulationPolicy::HabitatDrivenV1,
                "{}",
                profile.label()
            );
        }
    }

    #[test]
    fn authored_and_probe_profiles_default_disabled() {
        assert_eq!(
            WildlifePopulationPolicy::default_for_profile(WorldGenerationProfile::authored_only()),
            WildlifePopulationPolicy::Disabled
        );
        assert_eq!(
            WildlifePopulationPolicy::default_for_profile(WorldGenerationProfile::TopologyProbeV1),
            WildlifePopulationPolicy::Disabled
        );
    }

    #[test]
    fn labels_and_json_are_stable() {
        for policy in [
            WildlifePopulationPolicy::Disabled,
            WildlifePopulationPolicy::HabitatDrivenV1,
            WildlifePopulationPolicy::ReferencePassiveV1,
        ] {
            assert_eq!(
                WildlifePopulationPolicy::parse_label(policy.label()),
                Ok(policy)
            );
            let json = serde_json::to_string(&policy).unwrap();
            assert_eq!(
                serde_json::from_str::<WildlifePopulationPolicy>(&json).unwrap(),
                policy
            );
        }
    }
}
