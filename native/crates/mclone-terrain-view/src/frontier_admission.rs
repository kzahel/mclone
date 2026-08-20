use super::TerrainFrontierPresentationIdentity;

/// One-frame state of the exact/procedural frontier certificate.
///
/// The synchronous fallback is a complete product state, not a failure. It
/// keeps the current exact and base presentations drawable while bounded
/// spacing-one support is prepared.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TerrainFrontierAdmissionState {
    #[default]
    Disabled,
    SynchronousFallback,
    PreparingPreferred,
    Preferred,
    Rejected,
}

impl TerrainFrontierAdmissionState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::SynchronousFallback => "synchronous-fallback",
            Self::PreparingPreferred => "preparing-preferred",
            Self::Preferred => "preferred",
            Self::Rejected => "rejected",
        }
    }
}

/// Bounded lifecycle receipt for the immutable frontier consumed by mono,
/// per-eye, and multiview submissions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainFrontierAdmissionReceipt {
    pub state: TerrainFrontierAdmissionState,
    pub exact_generation: u64,
    pub presentation: TerrainFrontierPresentationIdentity,
    pub active_support_capacity: u32,
    pub active_support_tiles: u32,
    pub pending_support_capacity: u32,
    pub pending_support_tiles: u32,
    pub fallback_commits: u64,
    pub preferred_commits: u64,
    pub coalesced_generations: u64,
}

impl TerrainFrontierAdmissionReceipt {
    pub const fn complete(self) -> bool {
        matches!(
            self.state,
            TerrainFrontierAdmissionState::SynchronousFallback
                | TerrainFrontierAdmissionState::PreparingPreferred
                | TerrainFrontierAdmissionState::Preferred
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_and_preferred_states_are_complete_certificates() {
        for state in [
            TerrainFrontierAdmissionState::SynchronousFallback,
            TerrainFrontierAdmissionState::PreparingPreferred,
            TerrainFrontierAdmissionState::Preferred,
        ] {
            assert!(
                TerrainFrontierAdmissionReceipt {
                    state,
                    ..Default::default()
                }
                .complete()
            );
        }
        assert!(!TerrainFrontierAdmissionReceipt::default().complete());
        assert!(
            !TerrainFrontierAdmissionReceipt {
                state: TerrainFrontierAdmissionState::Rejected,
                ..Default::default()
            }
            .complete()
        );
    }
}
