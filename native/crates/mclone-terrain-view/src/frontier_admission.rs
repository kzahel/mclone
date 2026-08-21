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
    Warming,
    SynchronousFallback,
    PreparingPreferred,
    Preferred,
    Rejected,
}

impl TerrainFrontierAdmissionState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Warming => "warming",
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

/// Renderer-neutral facts about one active or preparing resource generation.
///
/// Concrete WGPU buffers stay in the viewport renderer, while this module owns
/// the admission state and its monotonic lifecycle counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct TerrainFrontierAdmissionResource {
    pub committed: bool,
    pub exact_generation: u64,
    pub presentation: TerrainFrontierPresentationIdentity,
    pub support_capacity: u32,
    pub support_tiles: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct TerrainFrontierAdmissionTracker {
    fallback_commits: u64,
    preferred_commits: u64,
    coalesced_generations: u64,
}

impl TerrainFrontierAdmissionTracker {
    pub fn record_fallback_commit(&mut self) {
        self.fallback_commits = self.fallback_commits.saturating_add(1);
    }

    pub fn record_preferred_commit(&mut self) {
        self.preferred_commits = self.preferred_commits.saturating_add(1);
    }

    pub fn record_coalesced_generation(&mut self) {
        self.coalesced_generations = self.coalesced_generations.saturating_add(1);
    }

    pub fn receipt(
        self,
        exact_frontier_required: bool,
        warming: bool,
        active: Option<TerrainFrontierAdmissionResource>,
        pending: Option<TerrainFrontierAdmissionResource>,
    ) -> TerrainFrontierAdmissionReceipt {
        let state = if !exact_frontier_required {
            TerrainFrontierAdmissionState::Disabled
        } else if warming {
            TerrainFrontierAdmissionState::Warming
        } else if active.is_none_or(|resource| !resource.committed) {
            TerrainFrontierAdmissionState::Rejected
        } else if pending.is_some() {
            TerrainFrontierAdmissionState::PreparingPreferred
        } else if active.is_some_and(|resource| resource.support_capacity == 0) {
            TerrainFrontierAdmissionState::SynchronousFallback
        } else {
            TerrainFrontierAdmissionState::Preferred
        };
        TerrainFrontierAdmissionReceipt {
            state,
            exact_generation: active.map_or(0, |resource| resource.exact_generation),
            presentation: active
                .map_or_else(TerrainFrontierPresentationIdentity::default, |resource| {
                    resource.presentation
                }),
            active_support_capacity: active.map_or(0, |resource| resource.support_capacity),
            active_support_tiles: active.map_or(0, |resource| resource.support_tiles),
            pending_support_capacity: pending.map_or(0, |resource| resource.support_capacity),
            pending_support_tiles: pending.map_or(0, |resource| resource.support_tiles),
            fallback_commits: self.fallback_commits,
            preferred_commits: self.preferred_commits,
            coalesced_generations: self.coalesced_generations,
        }
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
                state: TerrainFrontierAdmissionState::Warming,
                ..Default::default()
            }
            .complete()
        );
        assert!(
            !TerrainFrontierAdmissionReceipt {
                state: TerrainFrontierAdmissionState::Rejected,
                ..Default::default()
            }
            .complete()
        );
    }

    #[test]
    fn tracker_projects_complete_fallback_preparation_and_preferred_states() {
        let identity = TerrainFrontierPresentationIdentity {
            level_count: 6,
            semantic_hash: 42,
        };
        let fallback = TerrainFrontierAdmissionResource {
            committed: true,
            exact_generation: 9,
            presentation: identity,
            support_capacity: 0,
            support_tiles: 0,
        };
        let preferred = TerrainFrontierAdmissionResource {
            support_capacity: 32,
            support_tiles: 20,
            ..fallback
        };
        let mut tracker = TerrainFrontierAdmissionTracker::default();
        tracker.record_fallback_commit();
        assert_eq!(
            tracker.receipt(true, false, Some(fallback), None).state,
            TerrainFrontierAdmissionState::SynchronousFallback
        );
        tracker.record_coalesced_generation();
        assert_eq!(
            tracker
                .receipt(true, false, Some(fallback), Some(preferred))
                .state,
            TerrainFrontierAdmissionState::PreparingPreferred
        );
        tracker.record_preferred_commit();
        let receipt = tracker.receipt(true, false, Some(preferred), None);
        assert_eq!(receipt.state, TerrainFrontierAdmissionState::Preferred);
        assert_eq!(receipt.fallback_commits, 1);
        assert_eq!(receipt.preferred_commits, 1);
        assert_eq!(receipt.coalesced_generations, 1);
    }

    #[test]
    fn tracker_projects_incomplete_cold_start_as_warming() {
        let tracker = TerrainFrontierAdmissionTracker::default();
        let receipt = tracker.receipt(true, true, None, None);

        assert_eq!(receipt.state, TerrainFrontierAdmissionState::Warming);
        assert!(!receipt.complete());
    }
}
