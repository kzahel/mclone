use std::time::Duration;

use mclone_app_runtime::monotonic::MonotonicInstant;

/// Presentation-side projection of the replicated authoritative world clock.
///
/// The server still owns both clocks and periodically corrects the client. This
/// cadence only advances the cached sample between updates. It deliberately
/// follows the negotiated gameplay rate rather than body-pose publication:
/// mixed-reliability XR sessions publish poses at 60 Hz while simulation and
/// celestial time remain 20 Hz.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ClientClockCadence {
    next_due: Option<MonotonicInstant>,
    interval: Duration,
}

impl Default for ClientClockCadence {
    fn default() -> Self {
        Self {
            next_due: None,
            interval: interval_for_rate_hz(20),
        }
    }
}

impl ClientClockCadence {
    pub(crate) fn take_due_tick(&mut self, now: MonotonicInstant) -> bool {
        let Some(deadline) = self.next_due else {
            self.next_due = Some(now.saturating_add(self.interval));
            return true;
        };
        if now < deadline {
            return false;
        }

        // Preserve the requested average rate across ordinary display-frame
        // quantization, but do not replay a burst after a long late frame.
        self.next_due = Some(if now.saturating_duration_since(deadline) < self.interval {
            deadline.saturating_add(self.interval)
        } else {
            now.saturating_add(self.interval)
        });
        true
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
    fn xr_pose_rate_does_not_change_gameplay_clock_rate() {
        let configuration = mclone_protocol::SessionConfiguration::fixed_vanilla(
            2,
            2,
            mclone_protocol::SessionCapabilities::DEVELOPMENT_DEFAULT,
        )
        .with_pose_profile(
            60,
            60,
            mclone_protocol::EffectiveEphemeralTransport::NativeUdp,
        );
        assert_eq!(configuration.gameplay_rate_hz, 20);
        assert_eq!(configuration.body_pose_report_rate_hz, 60);

        let mut cadence = ClientClockCadence::default();
        cadence.set_rate_hz(configuration.gameplay_rate_hz);
        let start = MonotonicInstant::from_nanos(1_000);

        let due_ticks = (0..60)
            .filter(|frame| {
                let now = start.saturating_add(Duration::from_secs_f64(*frame as f64 / 60.0));
                cadence.take_due_tick(now)
            })
            .count();

        assert_eq!(due_ticks, 20);
    }

    #[test]
    fn display_frame_quantization_preserves_the_average_rate() {
        let mut cadence = ClientClockCadence::default();
        let start = MonotonicInstant::from_nanos(1_000);

        let due_ticks = (0..72)
            .filter(|frame| {
                let now = start.saturating_add(Duration::from_secs_f64(*frame as f64 / 72.0));
                cadence.take_due_tick(now)
            })
            .count();

        assert_eq!(due_ticks, 20);
    }

    #[test]
    fn long_late_frame_does_not_replay_a_clock_burst() {
        let mut cadence = ClientClockCadence::default();
        let start = MonotonicInstant::from_nanos(1_000);
        assert!(cadence.take_due_tick(start));

        let late = start.saturating_add(Duration::from_secs(5));
        assert!(cadence.take_due_tick(late));
        assert!(!cadence.take_due_tick(late));
    }

    #[test]
    fn reset_makes_the_next_frame_due() {
        let mut cadence = ClientClockCadence::default();
        let now = MonotonicInstant::from_nanos(1_000);
        assert!(cadence.take_due_tick(now));
        cadence.reset();
        assert!(cadence.take_due_tick(now));
    }
}
