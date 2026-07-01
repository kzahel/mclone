#![allow(dead_code)]

use crate::FullChunkStatus;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntityVisibility {
    Hidden,
    Tracked,
    Ticking,
}

impl EntityVisibility {
    pub(crate) const fn is_accessible(self) -> bool {
        matches!(self, Self::Tracked | Self::Ticking)
    }

    pub(crate) const fn is_ticking(self) -> bool {
        matches!(self, Self::Ticking)
    }

    pub(crate) fn from_full_chunk_status(status: FullChunkStatus) -> Self {
        if status.is_or_after(FullChunkStatus::EntityTicking) {
            Self::Ticking
        } else if status.is_or_after(FullChunkStatus::Border) {
            Self::Tracked
        } else {
            Self::Hidden
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_chunk_status_maps_to_java_entity_visibility() {
        assert_eq!(
            EntityVisibility::from_full_chunk_status(FullChunkStatus::Inaccessible),
            EntityVisibility::Hidden
        );
        assert_eq!(
            EntityVisibility::from_full_chunk_status(FullChunkStatus::Border),
            EntityVisibility::Tracked
        );
        assert_eq!(
            EntityVisibility::from_full_chunk_status(FullChunkStatus::Ticking),
            EntityVisibility::Tracked
        );
        assert_eq!(
            EntityVisibility::from_full_chunk_status(FullChunkStatus::EntityTicking),
            EntityVisibility::Ticking
        );
    }

    #[test]
    fn visibility_reports_accessible_and_ticking_flags() {
        assert!(!EntityVisibility::Hidden.is_accessible());
        assert!(!EntityVisibility::Hidden.is_ticking());

        assert!(EntityVisibility::Tracked.is_accessible());
        assert!(!EntityVisibility::Tracked.is_ticking());

        assert!(EntityVisibility::Ticking.is_accessible());
        assert!(EntityVisibility::Ticking.is_ticking());
    }
}
