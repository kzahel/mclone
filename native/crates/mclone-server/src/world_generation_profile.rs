use mclone_core::ChunkPos;
use mclone_worldgen::levelgen::{
    ChunkGenerationPlan, ChunkStatusRequirement, MutableChunkBlockBuffer,
};
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
    #[serde(rename = "mclone-overworld-v1")]
    McloneOverworldV1,
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
            Self::McloneOverworldV1 => "mclone-overworld-v1",
            Self::AuthoredOnly { .. } => "authored-only",
        }
    }

    pub fn parse_label(value: &str) -> Result<Self, String> {
        match value.trim() {
            "overworld" | "default" => Ok(Self::Overworld),
            "flat-grass-v1" | "flat_grass_v1" | "flatGrassV1" => Ok(Self::FlatGrassV1),
            "small-island-v1" | "small_island_v1" | "smallIslandV1" => Ok(Self::SmallIslandV1),
            "mclone-overworld-v1" | "mclone_overworld_v1" | "mcloneOverworldV1" => {
                Ok(Self::McloneOverworldV1)
            }
            "authored-only" | "authored_only" | "authoredOnly" => Ok(Self::authored_only()),
            value => Err(format!(
                "world generation profile must be overworld, flat-grass-v1, small-island-v1, mclone-overworld-v1, or authored-only, got `{value}`"
            )),
        }
    }

    pub const fn authored_missing_chunk(self) -> Option<AuthoredMissingChunk> {
        match self {
            Self::Overworld | Self::FlatGrassV1 | Self::SmallIslandV1 | Self::McloneOverworldV1 => {
                None
            }
            Self::AuthoredOnly { missing_chunk } => Some(missing_chunk),
        }
    }

    /// Closed built-in dispatch for the pure FEATURES-stage generation plan.
    /// Scheduler priority and readiness policy are intentionally applied only
    /// after this generator-owned declaration returns.
    fn plan_features(self, targets: impl IntoIterator<Item = ChunkPos>) -> ChunkGenerationPlan {
        match self {
            Self::Overworld => ChunkGenerationPlan::overworld_features(targets),
            Self::FlatGrassV1 => ChunkGenerationPlan::target_only(targets),
            Self::SmallIslandV1 => ChunkGenerationPlan::small_island_features(targets),
            Self::McloneOverworldV1 => ChunkGenerationPlan::target_only(targets),
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
            Self::McloneOverworldV1 => 4,
        }
    }

    pub(crate) const fn from_codec_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Overworld),
            1 => Some(Self::authored_only()),
            2 => Some(Self::FlatGrassV1),
            3 => Some(Self::SmallIslandV1),
            4 => Some(Self::McloneOverworldV1),
            _ => None,
        }
    }
}

/// Pure request from scheduler admission into generator-owned footprint
/// planning. The descriptor is included even when today's footprint does not
/// depend on seed so future profile planning cannot silently consult scheduler
/// state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GenerationPlanRequest {
    pub(crate) descriptor: WorldGenerationDescriptor,
    pub(crate) requested_outputs: Vec<ChunkPos>,
}

impl GenerationPlanRequest {
    pub(crate) fn new(
        descriptor: WorldGenerationDescriptor,
        requested_outputs: impl Into<Vec<ChunkPos>>,
    ) -> Self {
        Self {
            descriptor,
            requested_outputs: requested_outputs.into(),
        }
    }

    pub(crate) fn plan(&self) -> ChunkGenerationPlan {
        self.descriptor
            .profile
            .plan_features(self.requested_outputs.iter().copied())
    }
}

/// Currently supported typed artifact for one declared generation input.
/// Future metadata-only status inputs can extend this enum without converting
/// the scheduler/worker boundary back to untyped chunk buffers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GenerationInputArtifact {
    ChunkBlocks(MutableChunkBlockBuffer),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GenerationInput {
    pub(crate) requirement: ChunkStatusRequirement,
    pub(crate) artifact: GenerationInputArtifact,
}

impl GenerationInput {
    pub(crate) fn chunk_blocks(
        requirement: ChunkStatusRequirement,
        chunk: MutableChunkBlockBuffer,
    ) -> Result<Self, String> {
        let actual = ChunkPos::new(chunk.chunk_x, chunk.chunk_z);
        if actual != requirement.pos {
            return Err(format!(
                "generation input for ({}, {}) carried chunk blocks for ({}, {})",
                requirement.pos.x, requirement.pos.z, actual.x, actual.z
            ));
        }
        Ok(Self {
            requirement,
            artifact: GenerationInputArtifact::ChunkBlocks(chunk),
        })
    }

    pub(crate) fn into_chunk_blocks(self) -> MutableChunkBlockBuffer {
        match self.artifact {
            GenerationInputArtifact::ChunkBlocks(chunk) => chunk,
        }
    }
}

/// Typed worker execution request. The same plan request drives scheduler
/// prerequisite admission and worker-side validation/recomputation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GenerationExecutionRequest {
    pub(crate) plan: GenerationPlanRequest,
    pub(crate) seeded_inputs: Vec<GenerationInput>,
}

impl GenerationExecutionRequest {
    pub(crate) fn new(
        plan: GenerationPlanRequest,
        seeded_inputs: impl Into<Vec<GenerationInput>>,
    ) -> Self {
        Self {
            plan,
            seeded_inputs: seeded_inputs.into(),
        }
    }

    pub(crate) const fn descriptor(&self) -> WorldGenerationDescriptor {
        self.plan.descriptor
    }

    pub(crate) fn requested_outputs(&self) -> &[ChunkPos] {
        &self.plan.requested_outputs
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
            WorldGenerationProfile::McloneOverworldV1.label(),
            "mclone-overworld-v1"
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
            WorldGenerationProfile::parse_label("mclone-overworld-v1").unwrap(),
            WorldGenerationProfile::McloneOverworldV1
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
            serde_json::to_string(&WorldGenerationProfile::McloneOverworldV1).unwrap(),
            r#""mclone-overworld-v1""#
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
