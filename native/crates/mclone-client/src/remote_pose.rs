use std::collections::VecDeque;

use mclone_core::{HorizontalTopology, Vec3d};
use mclone_protocol::{PlayerBodyPoseSample, sequence_is_newer};

const MAX_REMOTE_POSE_SAMPLES: usize = 32;
const MAX_PLAUSIBLE_SAMPLE_GAP_MILLIS: u32 = 10_000;
const MIN_INTERPOLATION_DELAY_MILLIS: u32 = 35;
const MAX_INTERPOLATION_DELAY_MILLIS: u32 = 150;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RemotePoseTimelineDiagnostics {
    pub received_samples: u64,
    pub accepted_samples: u64,
    pub stale_samples: u64,
    pub duplicate_samples: u64,
    pub missing_sequences: u64,
    pub discontinuities: u64,
    pub superseded_samples: u64,
    pub jitter_millis: f32,
    pub interpolation_delay_millis: u32,
    pub latest_pose_age_millis: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RemotePosePushResult {
    Accepted,
    Stale,
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TimedRemotePose {
    pose: PlayerBodyPoseSample,
    arrival_time_millis: u64,
    timeline_time_millis: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RemotePlayerPoseTimeline {
    samples: VecDeque<TimedRemotePose>,
    received_samples: u64,
    accepted_samples: u64,
    stale_samples: u64,
    duplicate_samples: u64,
    missing_sequences: u64,
    discontinuities: u64,
    superseded_samples: u64,
    jitter_millis: f32,
}

impl RemotePlayerPoseTimeline {
    pub(crate) fn push(
        &mut self,
        pose: PlayerBodyPoseSample,
        arrival_time_millis: u64,
    ) -> RemotePosePushResult {
        self.received_samples = self.received_samples.saturating_add(1);
        let previous = self.samples.back().copied();
        if let Some(previous) = previous {
            if pose.presentation_epoch != previous.pose.presentation_epoch {
                if !sequence_is_newer(pose.presentation_epoch, previous.pose.presentation_epoch) {
                    self.stale_samples = self.stale_samples.saturating_add(1);
                    return RemotePosePushResult::Stale;
                }
                self.samples.clear();
                self.discontinuities = self.discontinuities.saturating_add(1);
            } else if pose.sequence == previous.pose.sequence {
                self.duplicate_samples = self.duplicate_samples.saturating_add(1);
                return RemotePosePushResult::Duplicate;
            } else if !sequence_is_newer(pose.sequence, previous.pose.sequence) {
                self.stale_samples = self.stale_samples.saturating_add(1);
                return RemotePosePushResult::Stale;
            } else {
                let sequence_distance =
                    nonzero_sequence_distance(pose.sequence, previous.pose.sequence);
                self.missing_sequences = self
                    .missing_sequences
                    .saturating_add(u64::from(sequence_distance.saturating_sub(1)));
            }
        }

        if pose.discontinuity && !self.samples.is_empty() {
            self.samples.clear();
            self.discontinuities = self.discontinuities.saturating_add(1);
        }

        let timeline_time_millis = self
            .samples
            .back()
            .map(|previous| {
                let sender_delta = pose
                    .sample_time_millis
                    .wrapping_sub(previous.pose.sample_time_millis);
                let arrival_delta =
                    arrival_time_millis.saturating_sub(previous.arrival_time_millis);
                if sender_delta > 0 && sender_delta <= MAX_PLAUSIBLE_SAMPLE_GAP_MILLIS {
                    let deviation = arrival_delta.abs_diff(u64::from(sender_delta)) as f32;
                    self.jitter_millis = if self.accepted_samples <= 1 {
                        deviation
                    } else {
                        self.jitter_millis * 0.9 + deviation * 0.1
                    };
                    previous
                        .timeline_time_millis
                        .saturating_add(u64::from(sender_delta))
                        .max(arrival_time_millis)
                } else {
                    arrival_time_millis.max(previous.timeline_time_millis.saturating_add(1))
                }
            })
            .unwrap_or(arrival_time_millis);
        self.samples.push_back(TimedRemotePose {
            pose,
            arrival_time_millis,
            timeline_time_millis,
        });
        self.accepted_samples = self.accepted_samples.saturating_add(1);
        while self.samples.len() > MAX_REMOTE_POSE_SAMPLES {
            self.samples.pop_front();
            self.superseded_samples = self.superseded_samples.saturating_add(1);
        }
        RemotePosePushResult::Accepted
    }

    pub(crate) fn sample(
        &self,
        topology: HorizontalTopology,
        now_millis: u64,
        report_rate_hz: u32,
    ) -> Option<PlayerBodyPoseSample> {
        let first = self.samples.front()?.pose;
        let delay_millis = self.interpolation_delay_millis(report_rate_hz);
        let presentation_time = now_millis.saturating_sub(u64::from(delay_millis));
        let first_timed = self.samples.front()?;
        if presentation_time <= first_timed.timeline_time_millis {
            return Some(first);
        }
        let last = self.samples.back()?;
        if presentation_time >= last.timeline_time_millis {
            return Some(last.pose);
        }

        for index in 1..self.samples.len() {
            let from = self.samples[index - 1];
            let to = self.samples[index];
            if presentation_time > to.timeline_time_millis {
                continue;
            }
            let duration = to
                .timeline_time_millis
                .saturating_sub(from.timeline_time_millis);
            if duration == 0 {
                return Some(to.pose);
            }
            let elapsed = presentation_time.saturating_sub(from.timeline_time_millis);
            let factor = (elapsed as f64 / duration as f64).clamp(0.0, 1.0);
            return Some(interpolate_pose(topology, from.pose, to.pose, factor));
        }
        Some(last.pose)
    }

    pub(crate) fn diagnostics(
        &self,
        now_millis: u64,
        report_rate_hz: u32,
    ) -> RemotePoseTimelineDiagnostics {
        RemotePoseTimelineDiagnostics {
            received_samples: self.received_samples,
            accepted_samples: self.accepted_samples,
            stale_samples: self.stale_samples,
            duplicate_samples: self.duplicate_samples,
            missing_sequences: self.missing_sequences,
            discontinuities: self.discontinuities,
            superseded_samples: self.superseded_samples,
            jitter_millis: self.jitter_millis,
            interpolation_delay_millis: self.interpolation_delay_millis(report_rate_hz),
            latest_pose_age_millis: self.samples.back().map_or(0, |sample| {
                now_millis.saturating_sub(sample.arrival_time_millis)
            }),
        }
    }

    fn interpolation_delay_millis(&self, report_rate_hz: u32) -> u32 {
        let interval_millis = 1_000_u32.div_ceil(report_rate_hz.clamp(1, 1_000));
        let jitter_margin = (self.jitter_millis * 2.0).round().max(0.0) as u32;
        interval_millis
            .saturating_mul(2)
            .saturating_add(jitter_margin)
            .clamp(
                MIN_INTERPOLATION_DELAY_MILLIS,
                MAX_INTERPOLATION_DELAY_MILLIS,
            )
    }
}

fn nonzero_sequence_distance(candidate: u32, current: u32) -> u32 {
    let wrapping_distance = candidate.wrapping_sub(current);
    if candidate < current {
        wrapping_distance.saturating_sub(1)
    } else {
        wrapping_distance
    }
}

fn interpolate_pose(
    topology: HorizontalTopology,
    from: PlayerBodyPoseSample,
    mut to: PlayerBodyPoseSample,
    factor: f64,
) -> PlayerBodyPoseSample {
    to.position = topology.nearest_position_lift(to.position, from.position);
    PlayerBodyPoseSample {
        presentation_epoch: to.presentation_epoch,
        sequence: to.sequence,
        sample_time_millis: to.sample_time_millis,
        position: Vec3d::new(
            lerp(from.position.x, to.position.x, factor),
            lerp(from.position.y, to.position.y, factor),
            lerp(from.position.z, to.position.z, factor),
        ),
        y_rot_degrees: lerp_degrees(from.y_rot_degrees, to.y_rot_degrees, factor as f32),
        x_rot_degrees: lerp_degrees(from.x_rot_degrees, to.x_rot_degrees, factor as f32),
        on_ground: if factor < 0.5 {
            from.on_ground
        } else {
            to.on_ground
        },
        discontinuity: false,
    }
}

fn lerp(from: f64, to: f64, factor: f64) -> f64 {
    from + (to - from) * factor
}

fn lerp_degrees(from: f32, to: f32, factor: f32) -> f32 {
    let delta = (to - from + 180.0).rem_euclid(360.0) - 180.0;
    from + delta * factor
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(epoch: u32, sequence: u32, time: u32, x: f64) -> PlayerBodyPoseSample {
        PlayerBodyPoseSample::new(
            epoch,
            sequence,
            time,
            Vec3d::new(x, 64.0, 0.0),
            x as f32 * 10.0,
            0.0,
            true,
        )
    }

    #[test]
    fn interpolation_uses_sender_cadence_and_a_delayed_local_timeline() {
        let mut timeline = RemotePlayerPoseTimeline::default();
        assert_eq!(
            timeline.push(pose(1, 1, 1_000, 0.0), 10_000),
            RemotePosePushResult::Accepted
        );
        assert_eq!(
            timeline.push(pose(1, 2, 1_050, 10.0), 10_050),
            RemotePosePushResult::Accepted
        );

        let sampled = timeline
            .sample(HorizontalTopology::UNBOUNDED, 10_125, 20)
            .unwrap();
        assert!((sampled.position.x - 5.0).abs() < 1.0e-9);
        assert!((sampled.y_rot_degrees - 50.0).abs() < 1.0e-6);
    }

    #[test]
    fn loss_duplicate_reordering_and_wrap_never_replace_newer_state() {
        let mut timeline = RemotePlayerPoseTimeline::default();
        assert_eq!(
            timeline.push(pose(1, u32::MAX - 1, 1_000, 1.0), 1_000),
            RemotePosePushResult::Accepted
        );
        assert_eq!(
            timeline.push(pose(1, u32::MAX, 1_050, 2.0), 1_050),
            RemotePosePushResult::Accepted
        );
        assert_eq!(
            timeline.push(pose(1, 1, 1_100, 3.0), 1_100),
            RemotePosePushResult::Accepted
        );
        assert_eq!(
            timeline.push(pose(1, 1, 1_100, 9.0), 1_101),
            RemotePosePushResult::Duplicate
        );
        assert_eq!(
            timeline.push(pose(1, u32::MAX, 1_050, 8.0), 1_102),
            RemotePosePushResult::Stale
        );
        assert_eq!(
            timeline.push(pose(1, 4, 1_250, 4.0), 1_250),
            RemotePosePushResult::Accepted
        );

        let diagnostics = timeline.diagnostics(1_250, 20);
        assert_eq!(diagnostics.duplicate_samples, 1);
        assert_eq!(diagnostics.stale_samples, 1);
        assert_eq!(diagnostics.missing_sequences, 2);
        assert_eq!(
            timeline
                .sample(HorizontalTopology::UNBOUNDED, 10_000, 20)
                .unwrap()
                .position
                .x,
            4.0
        );
    }

    #[test]
    fn epoch_and_discontinuity_reset_the_interpolation_track() {
        let mut timeline = RemotePlayerPoseTimeline::default();
        timeline.push(pose(1, 1, 0, 1.0), 0);
        timeline.push(pose(1, 2, 50, 2.0), 50);
        assert_eq!(
            timeline.push(pose(2, 1, 100, 20.0), 100),
            RemotePosePushResult::Accepted
        );
        assert_eq!(
            timeline
                .sample(HorizontalTopology::UNBOUNDED, 100, 20)
                .unwrap()
                .position
                .x,
            20.0
        );
        assert_eq!(timeline.diagnostics(100, 20).discontinuities, 1);
    }

    #[test]
    fn periodic_interpolation_crosses_the_short_seam_lift() {
        let mut timeline = RemotePlayerPoseTimeline::default();
        timeline.push(pose(1, 1, 0, 511.5), 1_000);
        timeline.push(pose(1, 2, 50, 0.5), 1_050);
        let sampled = timeline
            .sample(HorizontalTopology::cylinder_x(0, 32), 1_125, 20)
            .unwrap();
        assert!((sampled.position.x - 512.0).abs() < 1.0e-9);
    }

    #[test]
    fn bounded_pressure_discards_only_oldest_already_superseded_samples() {
        let mut timeline = RemotePlayerPoseTimeline::default();
        for sequence in 1..=100 {
            assert_eq!(
                timeline.push(
                    pose(1, sequence, sequence * 16, f64::from(sequence)),
                    u64::from(sequence * 16),
                ),
                RemotePosePushResult::Accepted
            );
        }

        let diagnostics = timeline.diagnostics(1_600, 60);
        assert_eq!(diagnostics.accepted_samples, 100);
        assert_eq!(diagnostics.superseded_samples, 68);
        assert_eq!(
            timeline
                .sample(HorizontalTopology::UNBOUNDED, 10_000, 60)
                .unwrap()
                .position
                .x,
            100.0
        );
    }

    #[test]
    fn arrival_jitter_increases_delay_within_explicit_clamps() {
        let mut timeline = RemotePlayerPoseTimeline::default();
        timeline.push(pose(1, 1, 0, 0.0), 1_000);
        timeline.push(pose(1, 2, 50, 1.0), 1_090);

        let diagnostics = timeline.diagnostics(1_090, 20);
        assert_eq!(diagnostics.jitter_millis, 40.0);
        assert_eq!(diagnostics.interpolation_delay_millis, 150);
        assert_eq!(diagnostics.latest_pose_age_millis, 0);
    }
}
