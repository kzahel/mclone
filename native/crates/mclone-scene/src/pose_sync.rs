use std::time::Duration;

use mclone_app_runtime::monotonic::MonotonicInstant;

/// Mclone's platform hosts have different presentation cadences, so the shared
/// scene owns a negotiated publication deadline independently of presentation.
/// Reliable compatibility currently selects 20 Hz while mixed-reliability
/// carriers select 60 Hz. A late frame advances the deadline from the observed
/// time instead of emitting a catch-up burst.
pub(crate) const DEFAULT_PLAYER_POSE_SYNC_RATE_HZ: u32 = 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlayerPoseSyncCadence {
    next_due: Option<MonotonicInstant>,
    interval: Duration,
}

impl Default for PlayerPoseSyncCadence {
    fn default() -> Self {
        Self {
            next_due: None,
            interval: interval_for_rate_hz(DEFAULT_PLAYER_POSE_SYNC_RATE_HZ),
        }
    }
}

impl PlayerPoseSyncCadence {
    pub(crate) fn is_due(self, now: MonotonicInstant) -> bool {
        self.next_due.is_none_or(|deadline| now >= deadline)
    }

    pub(crate) fn record_attempt(&mut self, now: MonotonicInstant) {
        self.next_due = Some(now.saturating_add(self.interval));
    }

    pub(crate) fn set_rate_hz(&mut self, rate_hz: u32) {
        let interval = interval_for_rate_hz(rate_hz);
        if self.interval != interval {
            self.interval = interval;
            self.next_due = None;
        }
    }

    pub(crate) fn reset(&mut self) {
        self.next_due = None;
    }
}

fn interval_for_rate_hz(rate_hz: u32) -> Duration {
    Duration::from_secs_f64(1.0 / f64::from(rate_hz.clamp(1, 1_000)))
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
        assert!(
            cadence.is_due(
                late.saturating_add(interval_for_rate_hz(DEFAULT_PLAYER_POSE_SYNC_RATE_HZ))
            )
        );
    }

    #[test]
    fn reset_makes_the_next_frame_due() {
        let mut cadence = PlayerPoseSyncCadence::default();
        let now = MonotonicInstant::from_nanos(1_000);
        cadence.record_attempt(now);
        cadence.reset();
        assert!(cadence.is_due(now));
    }

    #[test]
    fn negotiated_rate_changes_the_deadline_without_a_catch_up_burst() {
        let mut cadence = PlayerPoseSyncCadence::default();
        let start = MonotonicInstant::from_nanos(1_000);
        cadence.set_rate_hz(60);
        cadence.record_attempt(start);

        assert!(!cadence.is_due(start.saturating_add(Duration::from_millis(16))));
        assert!(cadence.is_due(start.saturating_add(Duration::from_millis(17))));
    }
}
