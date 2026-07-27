use mclone_core::HorizontalTopology;

use crate::{ExactPaintedCoverageSnapshot, TerrainCompositionSourceIdentity};

/// Semantic relationship between an exact terrain feed and world authority.
///
/// This is deliberately independent from execution mechanics. A detached
/// source may run on a native thread or browser Worker; a live source may be
/// local or remote.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TerrainViewTruthRole {
    DetachedCanonical,
    LiveAuthoritative,
    BoundedObserver,
}

impl TerrainViewTruthRole {
    pub const fn label(self) -> &'static str {
        match self {
            Self::DetachedCanonical => "detached-canonical",
            Self::LiveAuthoritative => "live-authoritative",
            Self::BoundedObserver => "bounded-observer",
        }
    }

    pub const fn authoritative(self) -> bool {
        matches!(self, Self::LiveAuthoritative)
    }
}

/// Source-qualified identity for one committed terrain representation.
///
/// `procedural` is optional because a remote observer publication may not
/// disclose enough generator facts to reconstruct a horizon. Current detached
/// and local live composition require it; callers must use
/// [`Self::composition_source`] before attempting procedural rendering.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TerrainViewSourceIdentity {
    procedural: Option<TerrainCompositionSourceIdentity>,
    truth_role: TerrainViewTruthRole,
    topology: HorizontalTopology,
    generation: u64,
    source_revision: u64,
}

impl TerrainViewSourceIdentity {
    pub fn new(
        procedural: Option<TerrainCompositionSourceIdentity>,
        truth_role: TerrainViewTruthRole,
        topology: HorizontalTopology,
        generation: u64,
        source_revision: u64,
    ) -> Result<Self, String> {
        if generation == 0 {
            return Err("terrain-view source generation must be non-zero".to_owned());
        }
        if source_revision == 0 {
            return Err("terrain-view source revision must be non-zero".to_owned());
        }
        topology
            .validate()
            .map_err(|error| format!("invalid terrain-view source topology: {error}"))?;
        Ok(Self {
            procedural,
            truth_role,
            topology,
            generation,
            source_revision,
        })
    }

    pub fn detached(
        source: TerrainCompositionSourceIdentity,
        topology: HorizontalTopology,
        generation: u64,
        source_revision: u64,
    ) -> Result<Self, String> {
        Self::new(
            Some(source),
            TerrainViewTruthRole::DetachedCanonical,
            topology,
            generation,
            source_revision,
        )
    }

    pub fn live(
        source: TerrainCompositionSourceIdentity,
        topology: HorizontalTopology,
        generation: u64,
        source_revision: u64,
    ) -> Result<Self, String> {
        Self::new(
            Some(source),
            TerrainViewTruthRole::LiveAuthoritative,
            topology,
            generation,
            source_revision,
        )
    }

    pub const fn procedural(self) -> Option<TerrainCompositionSourceIdentity> {
        self.procedural
    }

    pub fn composition_source(self) -> Result<TerrainCompositionSourceIdentity, String> {
        self.procedural.ok_or_else(|| {
            "terrain-view source does not disclose a procedural composition identity".to_owned()
        })
    }

    pub const fn truth_role(self) -> TerrainViewTruthRole {
        self.truth_role
    }

    pub const fn topology(self) -> HorizontalTopology {
        self.topology
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn source_revision(self) -> u64 {
        self.source_revision
    }
}

/// Immutable exact facts consumed by the shared terrain representation engine.
///
/// The source adapter owns how those facts were acquired. The engine only
/// accepts coverage whose procedural identity agrees with the qualified
/// source, preventing a live or detached mask from suppressing another world.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainPreparedExactFrame {
    source: TerrainViewSourceIdentity,
    coverage: ExactPaintedCoverageSnapshot,
}

impl TerrainPreparedExactFrame {
    pub fn new(
        source: TerrainViewSourceIdentity,
        coverage: ExactPaintedCoverageSnapshot,
    ) -> Result<Self, String> {
        if coverage.source() != source.composition_source()? {
            return Err(
                "prepared exact coverage does not match its terrain-view source".to_owned(),
            );
        }
        Ok(Self { source, coverage })
    }

    pub const fn source(&self) -> TerrainViewSourceIdentity {
        self.source
    }

    pub const fn coverage(&self) -> &ExactPaintedCoverageSnapshot {
        &self.coverage
    }
}

#[cfg(test)]
mod tests {
    use mclone_core::{AxisTopology, ChunkPos};
    use mclone_worldgen::terrain_preview::TerrainPreviewProfile;

    use super::*;

    fn procedural(seed: i64) -> TerrainCompositionSourceIdentity {
        TerrainCompositionSourceIdentity::new(TerrainPreviewProfile::McloneOverworldV1, seed)
    }

    #[test]
    fn truth_role_is_independent_from_execution_host() {
        assert!(!TerrainViewTruthRole::DetachedCanonical.authoritative());
        assert!(TerrainViewTruthRole::LiveAuthoritative.authoritative());
        assert!(!TerrainViewTruthRole::BoundedObserver.authoritative());
    }

    #[test]
    fn source_rejects_zero_identity_and_invalid_topology() {
        assert_eq!(
            TerrainViewSourceIdentity::detached(
                procedural(1),
                HorizontalTopology::UNBOUNDED,
                0,
                1,
            )
            .unwrap_err(),
            "terrain-view source generation must be non-zero"
        );
        assert_eq!(
            TerrainViewSourceIdentity::detached(
                procedural(1),
                HorizontalTopology::UNBOUNDED,
                1,
                0,
            )
            .unwrap_err(),
            "terrain-view source revision must be non-zero"
        );
        assert!(
            TerrainViewSourceIdentity::detached(
                procedural(1),
                HorizontalTopology::new(AxisTopology::periodic(0, 0), AxisTopology::Unbounded),
                1,
                1,
            )
            .unwrap_err()
            .contains("invalid terrain-view source topology")
        );
    }

    #[test]
    fn hidden_observer_source_cannot_claim_procedural_composition() {
        let source = TerrainViewSourceIdentity::new(
            None,
            TerrainViewTruthRole::BoundedObserver,
            HorizontalTopology::UNBOUNDED,
            1,
            1,
        )
        .unwrap();
        assert_eq!(
            source.composition_source().unwrap_err(),
            "terrain-view source does not disclose a procedural composition identity"
        );
    }

    #[test]
    fn prepared_exact_frame_requires_matching_procedural_source() {
        let source =
            TerrainViewSourceIdentity::live(procedural(5), HorizontalTopology::UNBOUNDED, 7, 11)
                .unwrap();
        let matching =
            ExactPaintedCoverageSnapshot::new(procedural(5), 3, [ChunkPos::new(0, 0)]).unwrap();
        let frame = TerrainPreparedExactFrame::new(source, matching).unwrap();
        assert_eq!(
            frame.source().truth_role(),
            TerrainViewTruthRole::LiveAuthoritative
        );
        assert!(frame.coverage().contains(ChunkPos::new(0, 0)));

        let mismatched =
            ExactPaintedCoverageSnapshot::new(procedural(6), 4, [ChunkPos::new(0, 0)]).unwrap();
        assert_eq!(
            TerrainPreparedExactFrame::new(source, mismatched).unwrap_err(),
            "prepared exact coverage does not match its terrain-view source"
        );
    }
}
