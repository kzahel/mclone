//! Fixed-rate simulation lane cadence.
//!
//! This module is intentionally independent from the live runner loop for now.
//! It defines how a configurable host pump maps onto lower/higher-rate
//! simulation lanes without making one global tick mean every subsystem's time.

use std::time::Duration;

pub const DEFAULT_HOST_RATE_HZ: u32 = 20;
pub const DEFAULT_GAMEPLAY_RATE_HZ: u32 = 20;
pub const DEFAULT_PHYSICS_RATE_HZ: u32 = 60;
pub const DEFAULT_MAX_CATCH_UP_HOST_FRAMES: u32 = 4;

const NANOS_PER_SECOND: u128 = 1_000_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationCadenceConfig {
    pub host_rate_hz: u32,
    pub gameplay_rate_hz: u32,
    pub physics_rate_hz: u32,
    pub max_catch_up_host_frames: u32,
}

impl SimulationCadenceConfig {
    pub const fn new(host_rate_hz: u32, gameplay_rate_hz: u32, physics_rate_hz: u32) -> Self {
        Self {
            host_rate_hz,
            gameplay_rate_hz,
            physics_rate_hz,
            max_catch_up_host_frames: DEFAULT_MAX_CATCH_UP_HOST_FRAMES,
        }
    }

    pub const fn with_max_catch_up_host_frames(mut self, frames: u32) -> Self {
        self.max_catch_up_host_frames = frames;
        self
    }

    pub const fn is_valid(self) -> bool {
        self.host_rate_hz > 0
            && self.gameplay_rate_hz > 0
            && self.physics_rate_hz > 0
            && self.max_catch_up_host_frames > 0
    }
}

impl Default for SimulationCadenceConfig {
    fn default() -> Self {
        Self::new(
            DEFAULT_HOST_RATE_HZ,
            DEFAULT_GAMEPLAY_RATE_HZ,
            DEFAULT_PHYSICS_RATE_HZ,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SimulationCadenceFrame {
    pub gameplay_ticks: u32,
    pub physics_steps: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SimulationCadenceAdvance {
    pub host_frames: u32,
    pub dropped_host_frames: u32,
    pub gameplay_ticks: u32,
    pub physics_steps: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationCadence {
    config: SimulationCadenceConfig,
    gameplay_accumulator: u32,
    physics_accumulator: u32,
    host_frame_accumulator_nanos: u128,
}

impl SimulationCadence {
    pub fn new(config: SimulationCadenceConfig) -> Option<Self> {
        config.is_valid().then_some(Self {
            config,
            gameplay_accumulator: 0,
            physics_accumulator: 0,
            host_frame_accumulator_nanos: 0,
        })
    }

    pub const fn config(&self) -> SimulationCadenceConfig {
        self.config
    }

    pub fn advance_host_frame(&mut self) -> SimulationCadenceFrame {
        SimulationCadenceFrame {
            gameplay_ticks: advance_lane(
                &mut self.gameplay_accumulator,
                self.config.gameplay_rate_hz,
                self.config.host_rate_hz,
            ),
            physics_steps: advance_lane(
                &mut self.physics_accumulator,
                self.config.physics_rate_hz,
                self.config.host_rate_hz,
            ),
        }
    }

    pub fn advance_elapsed(&mut self, elapsed: Duration) -> SimulationCadenceAdvance {
        let elapsed_nanos = elapsed.as_nanos();
        self.host_frame_accumulator_nanos = self
            .host_frame_accumulator_nanos
            .saturating_add(elapsed_nanos.saturating_mul(u128::from(self.config.host_rate_hz)));
        let due_host_frames = self.host_frame_accumulator_nanos / NANOS_PER_SECOND;
        self.host_frame_accumulator_nanos %= NANOS_PER_SECOND;

        let max_frames = u128::from(self.config.max_catch_up_host_frames);
        let host_frames = due_host_frames.min(max_frames) as u32;
        let dropped_host_frames = due_host_frames
            .saturating_sub(u128::from(host_frames))
            .min(u128::from(u32::MAX)) as u32;

        let mut advance = SimulationCadenceAdvance {
            host_frames,
            dropped_host_frames,
            ..SimulationCadenceAdvance::default()
        };
        for _ in 0..host_frames {
            let frame = self.advance_host_frame();
            advance.gameplay_ticks = advance.gameplay_ticks.saturating_add(frame.gameplay_ticks);
            advance.physics_steps = advance.physics_steps.saturating_add(frame.physics_steps);
        }
        advance
    }
}

impl Default for SimulationCadence {
    fn default() -> Self {
        Self::new(SimulationCadenceConfig::default()).expect("default cadence is valid")
    }
}

fn advance_lane(accumulator: &mut u32, lane_rate_hz: u32, host_rate_hz: u32) -> u32 {
    *accumulator = accumulator.saturating_add(lane_rate_hz);
    let steps = *accumulator / host_rate_hz;
    *accumulator %= host_rate_hz;
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cadence(host_rate_hz: u32) -> SimulationCadence {
        SimulationCadence::new(SimulationCadenceConfig::new(host_rate_hz, 20, 60))
            .expect("valid cadence")
    }

    #[test]
    fn twenty_hz_host_runs_one_gameplay_tick_and_three_physics_steps() {
        let mut cadence = cadence(20);

        assert_eq!(
            cadence.advance_host_frame(),
            SimulationCadenceFrame {
                gameplay_ticks: 1,
                physics_steps: 3,
            }
        );
        assert_eq!(
            cadence.advance_host_frame(),
            SimulationCadenceFrame {
                gameplay_ticks: 1,
                physics_steps: 3,
            }
        );
    }

    #[test]
    fn sixty_hz_host_runs_physics_each_frame_and_gameplay_every_third_frame() {
        let mut cadence = cadence(60);

        let frames = (0..6)
            .map(|_| cadence.advance_host_frame())
            .collect::<Vec<_>>();

        assert_eq!(
            frames,
            vec![
                SimulationCadenceFrame {
                    gameplay_ticks: 0,
                    physics_steps: 1,
                },
                SimulationCadenceFrame {
                    gameplay_ticks: 0,
                    physics_steps: 1,
                },
                SimulationCadenceFrame {
                    gameplay_ticks: 1,
                    physics_steps: 1,
                },
                SimulationCadenceFrame {
                    gameplay_ticks: 0,
                    physics_steps: 1,
                },
                SimulationCadenceFrame {
                    gameplay_ticks: 0,
                    physics_steps: 1,
                },
                SimulationCadenceFrame {
                    gameplay_ticks: 1,
                    physics_steps: 1,
                },
            ]
        );
    }

    #[test]
    fn thirty_hz_host_keeps_fractional_gameplay_cadence_deterministic() {
        let mut cadence = cadence(30);

        let frames = (0..6)
            .map(|_| cadence.advance_host_frame())
            .collect::<Vec<_>>();

        assert_eq!(
            frames,
            vec![
                SimulationCadenceFrame {
                    gameplay_ticks: 0,
                    physics_steps: 2,
                },
                SimulationCadenceFrame {
                    gameplay_ticks: 1,
                    physics_steps: 2,
                },
                SimulationCadenceFrame {
                    gameplay_ticks: 1,
                    physics_steps: 2,
                },
                SimulationCadenceFrame {
                    gameplay_ticks: 0,
                    physics_steps: 2,
                },
                SimulationCadenceFrame {
                    gameplay_ticks: 1,
                    physics_steps: 2,
                },
                SimulationCadenceFrame {
                    gameplay_ticks: 1,
                    physics_steps: 2,
                },
            ]
        );
    }

    #[test]
    fn elapsed_time_accumulates_partial_host_frames() {
        let mut cadence = cadence(60);

        assert_eq!(
            cadence.advance_elapsed(Duration::from_millis(8)),
            SimulationCadenceAdvance::default()
        );
        assert_eq!(
            cadence.advance_elapsed(Duration::from_millis(9)),
            SimulationCadenceAdvance {
                host_frames: 1,
                dropped_host_frames: 0,
                gameplay_ticks: 0,
                physics_steps: 1,
            }
        );
    }

    #[test]
    fn long_elapsed_time_caps_catch_up_work() {
        let mut cadence = SimulationCadence::new(
            SimulationCadenceConfig::new(60, 20, 60).with_max_catch_up_host_frames(4),
        )
        .expect("valid cadence");

        assert_eq!(
            cadence.advance_elapsed(Duration::from_millis(200)),
            SimulationCadenceAdvance {
                host_frames: 4,
                dropped_host_frames: 8,
                gameplay_ticks: 1,
                physics_steps: 4,
            }
        );
    }

    #[test]
    fn invalid_cadence_config_is_rejected() {
        assert!(SimulationCadence::new(SimulationCadenceConfig::new(0, 20, 60)).is_none());
        assert!(SimulationCadence::new(SimulationCadenceConfig::new(60, 0, 60)).is_none());
        assert!(SimulationCadence::new(SimulationCadenceConfig::new(60, 20, 0)).is_none());
        assert!(
            SimulationCadence::new(
                SimulationCadenceConfig::new(60, 20, 60).with_max_catch_up_host_frames(0)
            )
            .is_none()
        );
    }
}
