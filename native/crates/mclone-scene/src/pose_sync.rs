use std::time::Duration;

use mclone_app_runtime::monotonic::MonotonicInstant;

/// Vanilla publishes local-player movement state once per 20 Hz client tick.
///
/// Mclone's platform hosts have different presentation cadences, so the shared
/// scene owns an at-most-20-Hz publication deadline. A late frame advances the
/// deadline from the observed time instead of emitting a catch-up burst.
pub(crate) const PLAYER_POSE_SYNC_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlayerPoseSyncCadence {
    next_due: Option<MonotonicInstant>,
}

impl PlayerPoseSyncCadence {
    pub(crate) fn is_due(self, now: MonotonicInstant) -> bool {
        self.next_due.is_none_or(|deadline| now >= deadline)
    }

    pub(crate) fn record_attempt(&mut self, now: MonotonicInstant) {
        self.next_due = Some(now.saturating_add(PLAYER_POSE_SYNC_INTERVAL));
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_attempt_is_due_then_waits_one_client_tick() {
        let mut cadence = PlayerPoseSyncCadence::default();
        let start = MonotonicInstant::from_nanos(1_000);

        assert!(cadence.is_due(start));
        cadence.record_attempt(start);
        assert!(!cadence.is_due(start.saturating_add(Duration::from_millis(49))));
        assert!(cadence.is_due(start.saturating_add(Duration::from_millis(50))));
    }

    #[test]
    fn late_frame_does_not_create_a_catch_up_burst() {
        let mut cadence = PlayerPoseSyncCadence::default();
        let start = MonotonicInstant::from_nanos(1_000);
        cadence.record_attempt(start);
        let late = start.saturating_add(Duration::from_secs(5));

        assert!(cadence.is_due(late));
        cadence.record_attempt(late);
        assert!(!cadence.is_due(late));
        assert!(cadence.is_due(late.saturating_add(PLAYER_POSE_SYNC_INTERVAL)));
    }

    #[test]
    fn reset_makes_the_next_frame_due() {
        let mut cadence = PlayerPoseSyncCadence::default();
        let now = MonotonicInstant::from_nanos(1_000);
        cadence.record_attempt(now);
        cadence.reset();
        assert!(cadence.is_due(now));
    }
}
