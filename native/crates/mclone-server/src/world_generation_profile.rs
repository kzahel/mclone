use mclone_core::ChunkPos;
use mclone_worldgen::levelgen::ChunkGenerationPlan;
use serde::{Deserialize, Serialize};

pub const AUTHORED_WORLD_MIN_Y: i32 = 0;
pub const AUTHORED_WORLD_HEIGHT: i32 = 256;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorldGenerationProfile {
    #[default]
    Overworld,
    #[serde(rename = "flat-grass-v1")]
    FlatGrassV1,
    #[serde(rename = "small-island-v1")]
    SmallIslandV1,
    AuthoredOnly {
        #[serde(rename = "missingChunk")]
        missing_chunk: AuthoredMissingChunk,
    },
}

impl WorldGenerationProfile {
    pub const fn authored_only() -> Self {
        Self::AuthoredOnly {
            missing_chunk: AuthoredMissingChunk::Void,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Overworld => "overworld",
            Self::FlatGrassV1 => "flat-grass-v1",
            Self::SmallIslandV1 => "small-island-v1",
            Self::AuthoredOnly { .. } => "authored-only",
        }
    }

    pub fn parse_label(value: &str) -> Result<Self, String> {
        match value.trim() {
            "overworld" | "default" => Ok(Self::Overworld),
            "flat-grass-v1" | "flat_grass_v1" | "flatGrassV1" => Ok(Self::FlatGrassV1),
            "small-island-v1" | "small_island_v1" | "smallIslandV1" => Ok(Self::SmallIslandV1),
            "authored-only" | "authored_only" | "authoredOnly" => Ok(Self::authored_only()),
            value => Err(format!(
                "world generation profile must be overworld, flat-grass-v1, small-island-v1, or authored-only, got `{value}`"
            )),
        }
    }

    pub const fn authored_missing_chunk(self) -> Option<AuthoredMissingChunk> {
        match self {
            Self::Overworld | Self::FlatGrassV1 | Self::SmallIslandV1 => None,
            Self::AuthoredOnly { missing_chunk } => Some(missing_chunk),
        }
    }

    /// Closed built-in dispatch for the pure FEATURES-stage generation plan.
    /// Scheduler priority and readiness policy are intentionally applied only
    /// after this generator-owned declaration returns.
    pub(crate) fn plan_features(
        self,
        targets: impl IntoIterator<Item = ChunkPos>,
    ) -> ChunkGenerationPlan {
        match self {
            Self::Overworld => ChunkGenerationPlan::overworld_features(targets),
            Self::FlatGrassV1 | Self::SmallIslandV1 => ChunkGenerationPlan::target_only(targets),
            Self::AuthoredOnly { .. } => {
                unreachable!("authored-only misses bypass procedural job creation")
            }
        }
    }

    pub(crate) const fn codec_tag(self) -> u8 {
        match self {
            Self::Overworld => 0,
            Self::AuthoredOnly { .. } => 1,
            Self::FlatGrassV1 => 2,
            Self::SmallIslandV1 => 3,
        }
    }

    pub(crate) const fn from_codec_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Overworld),
            1 => Some(Self::authored_only()),
            2 => Some(Self::FlatGrassV1),
            3 => Some(Self::SmallIslandV1),
            _ => None,
        }
    }
}

/// Complete immutable identity for one procedural generation session.
///
/// The profile is the persisted behavior/version identity. The seed remains a
/// separate stored world fact, but workers treat the pair as one descriptor so
/// resident generator state can never leak across either change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldGenerationDescriptor {
    pub profile: WorldGenerationProfile,
    pub seed: i64,
}

impl WorldGenerationDescriptor {
    pub const fn new(profile: WorldGenerationProfile, seed: i64) -> Self {
        Self { profile, seed }
    }

    pub const fn overworld(seed: i64) -> Self {
        Self::new(WorldGenerationProfile::Overworld, seed)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthoredMissingChunk {
    Void,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_defaults_and_labels_are_backward_compatible() {
        assert_eq!(
            WorldGenerationProfile::default(),
            WorldGenerationProfile::Overworld
        );
        assert_eq!(WorldGenerationProfile::Overworld.label(), "overworld");
        assert_eq!(WorldGenerationProfile::FlatGrassV1.label(), "flat-grass-v1");
        assert_eq!(
            WorldGenerationProfile::SmallIslandV1.label(),
            "small-island-v1"
        );
        assert_eq!(
            WorldGenerationProfile::authored_only().label(),
            "authored-only"
        );
        assert_eq!(
            WorldGenerationProfile::parse_label("default").unwrap(),
            WorldGenerationProfile::Overworld
        );
        assert_eq!(
            WorldGenerationProfile::parse_label("authored-only").unwrap(),
            WorldGenerationProfile::authored_only()
        );
        assert_eq!(
            WorldGenerationProfile::parse_label("flat-grass-v1").unwrap(),
            WorldGenerationProfile::FlatGrassV1
        );
        assert_eq!(
            WorldGenerationProfile::parse_label("small-island-v1").unwrap(),
            WorldGenerationProfile::SmallIslandV1
        );
        assert_eq!(
            serde_json::to_string(&WorldGenerationProfile::Overworld).unwrap(),
            r#""overworld""#
        );
        assert_eq!(
            serde_json::to_string(&WorldGenerationProfile::authored_only()).unwrap(),
            r#"{"authoredOnly":{"missingChunk":"void"}}"#
        );
        assert_eq!(
            serde_json::to_string(&WorldGenerationProfile::FlatGrassV1).unwrap(),
            r#""flat-grass-v1""#
        );
        assert_eq!(
            serde_json::to_string(&WorldGenerationProfile::SmallIslandV1).unwrap(),
            r#""small-island-v1""#
        );
        assert_eq!(
            serde_json::from_str::<WorldGenerationProfile>("\"overworld\"").unwrap(),
            WorldGenerationProfile::Overworld
        );
        assert_eq!(
            serde_json::from_str::<WorldGenerationProfile>(
                r#"{"authoredOnly":{"missingChunk":"void"}}"#,
            )
            .unwrap(),
            WorldGenerationProfile::authored_only()
        );
    }
}
