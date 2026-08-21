use mclone_core::{ChunkPos, ChunkStatus, HorizontalTopology, LiftedChunkPos};
use mclone_season::SeasonCalendarPolicy;
use mclone_worldgen::levelgen::{
    ChunkGenerationPlan, ChunkStatusRequirement, McloneOverworldSamplingTopology,
    MutableChunkBlockBuffer, validate_topology_probe_topology,
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
    #[serde(rename = "mclone-overworld-v2")]
    McloneOverworldV2,
    #[serde(rename = "topology-probe-v1")]
    TopologyProbeV1,
    #[serde(rename = "alpha-v1")]
    AlphaV1 {
        #[serde(default)]
        winter: bool,
    },
    #[serde(rename = "beta-v1")]
    BetaV1,
    AuthoredOnly {
        #[serde(rename = "missingChunk")]
        missing_chunk: AuthoredMissingChunk,
    },
}

impl WorldGenerationProfile {
    pub const fn season_calendar_policy(self) -> SeasonCalendarPolicy {
        match self {
            Self::McloneOverworldV1 | Self::McloneOverworldV2 => {
                SeasonCalendarPolicy::MCLONE_OVERWORLD_V1
            }
            Self::Overworld
            | Self::FlatGrassV1
            | Self::SmallIslandV1
            | Self::TopologyProbeV1
            | Self::AlphaV1 { .. }
            | Self::BetaV1
            | Self::AuthoredOnly { .. } => SeasonCalendarPolicy::Disabled,
        }
    }

    pub const fn authored_only() -> Self {
        Self::AuthoredOnly {
            missing_chunk: AuthoredMissingChunk::Void,
        }
    }

    pub const fn alpha_v1(winter: bool) -> Self {
        Self::AlphaV1 { winter }
    }

    pub const fn alpha_winter(self) -> Option<bool> {
        match self {
            Self::AlphaV1 { winter } => Some(winter),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Overworld => "overworld",
            Self::FlatGrassV1 => "flat-grass-v1",
            Self::SmallIslandV1 => "small-island-v1",
            Self::McloneOverworldV1 => "mclone-overworld-v1",
            Self::McloneOverworldV2 => "mclone-overworld-v2",
            Self::TopologyProbeV1 => "topology-probe-v1",
            Self::AlphaV1 { winter: false } => "alpha-v1",
            Self::AlphaV1 { winter: true } => "alpha-v1-winter",
            Self::BetaV1 => "beta-v1",
            Self::AuthoredOnly { .. } => "authored-only",
        }
    }

    pub fn parse_label(value: &str) -> Result<Self, String> {
        match value.trim() {
            "overworld" => Ok(Self::Overworld),
            "flat-grass-v1" | "flat_grass_v1" | "flatGrassV1" => Ok(Self::FlatGrassV1),
            "small-island-v1" | "small_island_v1" | "smallIslandV1" => Ok(Self::SmallIslandV1),
            "mclone-overworld-v1" | "mclone_overworld_v1" | "mcloneOverworldV1" => {
                Ok(Self::McloneOverworldV1)
            }
            "mclone-overworld-v2" | "mclone_overworld_v2" | "mcloneOverworldV2" => {
                Ok(Self::McloneOverworldV2)
            }
            "topology-probe-v1" | "topology_probe_v1" | "topologyProbeV1" => {
                Ok(Self::TopologyProbeV1)
            }
            "alpha-v1" | "alpha_v1" | "alphaV1" => Ok(Self::alpha_v1(false)),
            "alpha-v1-winter" | "alpha_v1_winter" | "alphaV1Winter" => Ok(Self::alpha_v1(true)),
            "beta-v1" | "beta_v1" | "betaV1" => Ok(Self::BetaV1),
            "authored-only" | "authored_only" | "authoredOnly" => Ok(Self::authored_only()),
            value => Err(format!(
                "world generation profile must be overworld, flat-grass-v1, small-island-v1, mclone-overworld-v1, mclone-overworld-v2, topology-probe-v1, alpha-v1, beta-v1, or authored-only, got `{value}`"
            )),
        }
    }

    pub const fn authored_missing_chunk(self) -> Option<AuthoredMissingChunk> {
        match self {
            Self::Overworld
            | Self::FlatGrassV1
            | Self::SmallIslandV1
            | Self::McloneOverworldV1
            | Self::McloneOverworldV2
            | Self::TopologyProbeV1
            | Self::AlphaV1 { .. }
            | Self::BetaV1 => None,
            Self::AuthoredOnly { missing_chunk } => Some(missing_chunk),
        }
    }

    pub fn validate_topology(self, topology: HorizontalTopology) -> Result<(), String> {
        topology
            .validate()
            .map_err(|error| format!("invalid dimension topology: {error}"))?;
        if matches!(self, Self::TopologyProbeV1) {
            return validate_topology_probe_topology(topology);
        }
        if topology.is_unbounded()
            || matches!(self, Self::FlatGrassV1 | Self::AuthoredOnly { .. })
            || matches!(self, Self::McloneOverworldV1)
                && McloneOverworldSamplingTopology::from_horizontal_topology(topology).is_ok()
        {
            Ok(())
        } else {
            Err(format!(
                "world generation profile {} does not support finite or periodic topology",
                self.label()
            ))
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
            Self::McloneOverworldV1 => ChunkGenerationPlan::mclone_overworld_features(targets),
            Self::McloneOverworldV2 => ChunkGenerationPlan::target_only(targets),
            Self::TopologyProbeV1 => {
                ChunkGenerationPlan::feature_region(targets, 0, 1, ChunkStatus::Surface)
            }
            Self::AlphaV1 { .. } => ChunkGenerationPlan::alpha_features(targets),
            Self::BetaV1 => ChunkGenerationPlan::beta_features(targets),
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
            Self::AlphaV1 { winter: false } => 5,
            Self::AlphaV1 { winter: true } => 6,
            Self::BetaV1 => 7,
            Self::TopologyProbeV1 => 8,
            Self::McloneOverworldV2 => 9,
        }
    }

    pub(crate) const fn from_codec_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Overworld),
            1 => Some(Self::authored_only()),
            2 => Some(Self::FlatGrassV1),
            3 => Some(Self::SmallIslandV1),
            4 => Some(Self::McloneOverworldV1),
            5 => Some(Self::alpha_v1(false)),
            6 => Some(Self::alpha_v1(true)),
            7 => Some(Self::BetaV1),
            8 => Some(Self::TopologyProbeV1),
            9 => Some(Self::McloneOverworldV2),
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
        let requested_outputs = requested_outputs
            .into()
            .into_iter()
            .map(|pos| {
                descriptor
                    .topology
                    .canonicalize_chunk(pos)
                    .expect("generation output must lie inside the dimension topology")
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        Self {
            descriptor,
            requested_outputs,
        }
    }

    pub(crate) fn plan(&self) -> ChunkGenerationPlan {
        let raw = self
            .descriptor
            .profile
            .plan_features(self.requested_outputs.iter().copied());
        topology_generation_plan(self.descriptor.topology, raw).canonical
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct GenerationChunkRef {
    pub(crate) canonical: ChunkPos,
    pub(crate) work_lift: LiftedChunkPos,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TopologyGenerationPlan {
    canonical: ChunkGenerationPlan,
    output_chunks: Vec<GenerationChunkRef>,
    backend_work_chunks: Vec<GenerationChunkRef>,
    prerequisites: Vec<(GenerationChunkRef, ChunkStatus)>,
}

fn topology_generation_plan(
    topology: HorizontalTopology,
    raw: ChunkGenerationPlan,
) -> TopologyGenerationPlan {
    let (outputs, backend_work, prerequisites) = raw.into_parts();
    let output_chunks = topology_generation_refs(topology, outputs);
    let backend_work_chunks = topology_generation_refs(topology, backend_work);
    let prerequisites = prerequisites
        .into_iter()
        .filter_map(|requirement| {
            topology_generation_ref(topology, requirement.pos)
                .map(|(canonical, chunk)| (canonical, (chunk, requirement.status)))
        })
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_values()
        .collect::<Vec<_>>();
    let canonical = ChunkGenerationPlan::from_parts(
        output_chunks.iter().map(|chunk| chunk.canonical),
        backend_work_chunks.iter().map(|chunk| chunk.canonical),
        prerequisites
            .iter()
            .map(|(chunk, status)| ChunkStatusRequirement::new(chunk.canonical, *status)),
    );
    TopologyGenerationPlan {
        canonical,
        output_chunks,
        backend_work_chunks,
        prerequisites,
    }
}

fn topology_generation_refs(
    topology: HorizontalTopology,
    positions: impl IntoIterator<Item = ChunkPos>,
) -> Vec<GenerationChunkRef> {
    positions
        .into_iter()
        .filter_map(|pos| topology_generation_ref(topology, pos))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_values()
        .collect()
}

fn topology_generation_ref(
    topology: HorizontalTopology,
    work_lift: ChunkPos,
) -> Option<(ChunkPos, GenerationChunkRef)> {
    let canonical = topology.canonicalize_chunk(work_lift)?;
    Some((
        canonical,
        GenerationChunkRef {
            canonical,
            work_lift: LiftedChunkPos::new(i64::from(work_lift.x), i64::from(work_lift.z)),
        },
    ))
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
/// separate stored world fact, but workers treat profile, seed, and topology as
/// one descriptor so resident generator state can never leak across a change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldGenerationDescriptor {
    pub profile: WorldGenerationProfile,
    pub seed: i64,
    pub topology: HorizontalTopology,
}

impl WorldGenerationDescriptor {
    pub const fn new(profile: WorldGenerationProfile, seed: i64) -> Self {
        Self::with_topology(profile, seed, HorizontalTopology::UNBOUNDED)
    }

    pub const fn with_topology(
        profile: WorldGenerationProfile,
        seed: i64,
        topology: HorizontalTopology,
    ) -> Self {
        Self {
            profile,
            seed,
            topology,
        }
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
    fn profile_labels_and_low_level_default_are_stable() {
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
            WorldGenerationProfile::McloneOverworldV2.label(),
            "mclone-overworld-v2"
        );
        assert_eq!(
            WorldGenerationProfile::TopologyProbeV1.label(),
            "topology-probe-v1"
        );
        assert_eq!(WorldGenerationProfile::alpha_v1(false).label(), "alpha-v1");
        assert_eq!(
            WorldGenerationProfile::alpha_v1(true).label(),
            "alpha-v1-winter"
        );
        assert_eq!(WorldGenerationProfile::BetaV1.label(), "beta-v1");
        assert_eq!(
            WorldGenerationProfile::authored_only().label(),
            "authored-only"
        );
        assert!(WorldGenerationProfile::parse_label("default").is_err());
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
            WorldGenerationProfile::parse_label("mclone-overworld-v2").unwrap(),
            WorldGenerationProfile::McloneOverworldV2
        );
        assert_eq!(
            WorldGenerationProfile::parse_label("topology-probe-v1").unwrap(),
            WorldGenerationProfile::TopologyProbeV1
        );
        assert_eq!(
            WorldGenerationProfile::parse_label("alpha-v1").unwrap(),
            WorldGenerationProfile::alpha_v1(false)
        );
        assert_eq!(
            WorldGenerationProfile::parse_label("alpha-v1-winter").unwrap(),
            WorldGenerationProfile::alpha_v1(true)
        );
        assert_eq!(
            WorldGenerationProfile::parse_label("beta-v1").unwrap(),
            WorldGenerationProfile::BetaV1
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
            serde_json::to_string(&WorldGenerationProfile::McloneOverworldV2).unwrap(),
            r#""mclone-overworld-v2""#
        );
        assert_eq!(
            serde_json::to_string(&WorldGenerationProfile::TopologyProbeV1).unwrap(),
            r#""topology-probe-v1""#
        );
        assert_eq!(
            serde_json::to_string(&WorldGenerationProfile::alpha_v1(false)).unwrap(),
            r#"{"alpha-v1":{"winter":false}}"#
        );
        assert_eq!(
            serde_json::to_string(&WorldGenerationProfile::alpha_v1(true)).unwrap(),
            r#"{"alpha-v1":{"winter":true}}"#
        );
        assert_eq!(
            serde_json::to_string(&WorldGenerationProfile::BetaV1).unwrap(),
            r#""beta-v1""#
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
        assert_eq!(
            serde_json::from_str::<WorldGenerationProfile>(r#"{"alpha-v1":{"winter":true}}"#,)
                .unwrap(),
            WorldGenerationProfile::alpha_v1(true)
        );
        assert_eq!(
            WorldGenerationProfile::from_codec_tag(5),
            Some(WorldGenerationProfile::alpha_v1(false))
        );
        assert_eq!(
            WorldGenerationProfile::from_codec_tag(6),
            Some(WorldGenerationProfile::alpha_v1(true))
        );
        assert_eq!(
            WorldGenerationProfile::from_codec_tag(7),
            Some(WorldGenerationProfile::BetaV1)
        );
        assert_eq!(
            WorldGenerationProfile::from_codec_tag(8),
            Some(WorldGenerationProfile::TopologyProbeV1)
        );
        assert_eq!(
            WorldGenerationProfile::from_codec_tag(9),
            Some(WorldGenerationProfile::McloneOverworldV2)
        );
    }

    #[test]
    fn bounded_topology_support_is_explicit_per_profile() {
        let finite = HorizontalTopology::new(
            mclone_core::AxisTopology::finite(0, 2),
            mclone_core::AxisTopology::finite(0, 2),
        );
        let cylinder = HorizontalTopology::new(
            mclone_core::AxisTopology::periodic(0, 32),
            mclone_core::AxisTopology::Unbounded,
        );
        let torus = HorizontalTopology::new(
            mclone_core::AxisTopology::periodic(0, 8),
            mclone_core::AxisTopology::periodic(0, 8),
        );

        assert!(
            WorldGenerationProfile::FlatGrassV1
                .validate_topology(finite)
                .is_ok()
        );
        assert!(
            WorldGenerationProfile::authored_only()
                .validate_topology(finite)
                .is_ok()
        );
        assert!(
            WorldGenerationProfile::FlatGrassV1
                .validate_topology(cylinder)
                .is_ok()
        );
        assert!(
            WorldGenerationProfile::McloneOverworldV1
                .validate_topology(HorizontalTopology::cylinder_x(
                    0,
                    mclone_worldgen::levelgen::MCLONE_OVERWORLD_PERIOD_CHUNKS,
                ))
                .is_ok()
        );
        assert!(
            WorldGenerationProfile::McloneOverworldV1
                .validate_topology(cylinder)
                .is_err()
        );
        assert!(
            WorldGenerationProfile::TopologyProbeV1
                .validate_topology(HorizontalTopology::UNBOUNDED)
                .is_ok()
        );
        assert!(
            WorldGenerationProfile::TopologyProbeV1
                .validate_topology(cylinder)
                .is_ok()
        );
        assert!(
            WorldGenerationProfile::TopologyProbeV1
                .validate_topology(torus)
                .is_ok()
        );
        assert!(
            WorldGenerationProfile::TopologyProbeV1
                .validate_topology(HorizontalTopology::cylinder_x(0, 7))
                .is_err()
        );
        assert!(
            WorldGenerationProfile::TopologyProbeV1
                .validate_topology(finite)
                .is_err()
        );
        for profile in [
            WorldGenerationProfile::Overworld,
            WorldGenerationProfile::SmallIslandV1,
            WorldGenerationProfile::alpha_v1(false),
            WorldGenerationProfile::BetaV1,
        ] {
            assert!(profile.validate_topology(finite).is_err(), "{profile:?}");
            assert!(profile.validate_topology(cylinder).is_err(), "{profile:?}");
            assert!(
                profile
                    .validate_topology(HorizontalTopology::UNBOUNDED)
                    .is_ok(),
                "{profile:?}"
            );
        }
        assert!(
            WorldGenerationProfile::McloneOverworldV1
                .validate_topology(finite)
                .is_err()
        );
        assert!(
            WorldGenerationProfile::McloneOverworldV1
                .validate_topology(HorizontalTopology::UNBOUNDED)
                .is_ok()
        );
        assert!(
            WorldGenerationProfile::McloneOverworldV2
                .validate_topology(HorizontalTopology::UNBOUNDED)
                .is_ok()
        );
        assert!(
            WorldGenerationProfile::McloneOverworldV2
                .validate_topology(cylinder)
                .is_err()
        );
    }

    #[test]
    fn both_mclone_overworlds_select_the_authoritative_season_calendar() {
        for profile in [
            WorldGenerationProfile::Overworld,
            WorldGenerationProfile::authored_only(),
            WorldGenerationProfile::FlatGrassV1,
            WorldGenerationProfile::SmallIslandV1,
            WorldGenerationProfile::alpha_v1(false),
            WorldGenerationProfile::alpha_v1(true),
            WorldGenerationProfile::BetaV1,
            WorldGenerationProfile::TopologyProbeV1,
        ] {
            assert_eq!(
                profile.season_calendar_policy(),
                mclone_season::SeasonCalendarPolicy::Disabled,
                "{profile:?}"
            );
        }
        assert_eq!(
            WorldGenerationProfile::McloneOverworldV1.season_calendar_policy(),
            mclone_season::SeasonCalendarPolicy::MCLONE_OVERWORLD_V1
        );
        assert_eq!(
            WorldGenerationProfile::McloneOverworldV2.season_calendar_policy(),
            mclone_season::SeasonCalendarPolicy::MCLONE_OVERWORLD_V1
        );
    }

    #[test]
    fn periodic_planning_keeps_work_lifts_while_deduplicating_canonical_chunks() {
        let topology = HorizontalTopology::new(
            mclone_core::AxisTopology::periodic(0, 32),
            mclone_core::AxisTopology::Unbounded,
        );
        let raw =
            ChunkGenerationPlan::feature_region([ChunkPos::new(31, 0)], 1, 1, ChunkStatus::Surface);

        let plan = topology_generation_plan(topology, raw);

        assert!(plan.backend_work_chunks.contains(&GenerationChunkRef {
            canonical: ChunkPos::new(0, 0),
            work_lift: LiftedChunkPos::new(32, 0),
        }));
        assert!(plan.prerequisites.contains(&(
            GenerationChunkRef {
                canonical: ChunkPos::new(0, 0),
                work_lift: LiftedChunkPos::new(32, 0),
            },
            ChunkStatus::Surface,
        )));
        assert_eq!(
            plan.canonical.backend_work_chunks().len(),
            plan.backend_work_chunks.len()
        );
        assert_eq!(
            plan.canonical.prerequisites().len(),
            plan.prerequisites.len()
        );
        assert_eq!(
            plan.output_chunks,
            vec![GenerationChunkRef {
                canonical: ChunkPos::new(31, 0),
                work_lift: LiftedChunkPos::new(31, 0),
            }]
        );
    }

    #[test]
    fn alpha_profile_declares_population_dependencies() {
        let plan = WorldGenerationProfile::alpha_v1(false).plan_features([ChunkPos::new(0, 0)]);
        assert_eq!(plan.output_chunks().len(), 1);
        assert_eq!(plan.backend_work_chunks().len(), 9);
        assert_eq!(plan.prerequisites().len(), 25);
    }

    #[test]
    fn beta_profile_declares_population_dependencies() {
        let plan = WorldGenerationProfile::BetaV1.plan_features([ChunkPos::new(0, 0)]);
        assert_eq!(plan.output_chunks().len(), 1);
        assert_eq!(plan.backend_work_chunks().len(), 9);
        assert_eq!(plan.prerequisites().len(), 25);
    }

    #[test]
    fn mclone_overworld_v2_declares_only_authoritative_outputs() {
        let descriptor =
            WorldGenerationDescriptor::new(WorldGenerationProfile::McloneOverworldV2, 12_345);
        let target = ChunkPos::new(-17, 31);
        let plan = GenerationPlanRequest::new(descriptor, vec![target]).plan();

        assert_eq!(
            plan.output_chunks().iter().copied().collect::<Vec<_>>(),
            [target]
        );
        assert_eq!(
            plan.backend_work_chunks()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [target]
        );
        assert!(plan.prerequisites().is_empty());
    }

    #[test]
    fn topology_probe_declares_the_light_neighbor_ring() {
        let plan = WorldGenerationProfile::TopologyProbeV1.plan_features([ChunkPos::new(0, 0)]);
        assert_eq!(plan.output_chunks().len(), 1);
        assert!(plan.output_chunks().contains(&ChunkPos::new(0, 0)));
        assert_eq!(plan.backend_work_chunks().len(), 1);
        assert!(plan.backend_work_chunks().contains(&ChunkPos::new(0, 0)));
        assert_eq!(plan.prerequisites().len(), 9);
        assert!(plan.prerequisites().contains(&ChunkStatusRequirement::new(
            ChunkPos::new(-1, 0),
            ChunkStatus::Surface
        )));
    }
}
