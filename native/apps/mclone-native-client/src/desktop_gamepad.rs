//! GilRs-backed ordinary gamepad collection for desktop flat and desktop XR.
//!
//! This adapter owns backend handles and hotplug bookkeeping only. Shared
//! dead zones, bindings, edges, repeat, contexts, and gameplay meaning stay in
//! `mclone-input` and `mclone-scene`.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context, Result};
use gilrs::{Axis, Button, EventType, Gamepad, Gilrs, GilrsBuilder};
use glam::Vec2;
use mclone_input::{
    ControllerInputBatch, ControllerInputObservation, InputSourceDescriptor, InputSourceId,
    InputSourceIdAllocator, StandardGamepadButtonState, StandardGamepadButtons,
    StandardGamepadSnapshot, classify_controller_layout,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct DesktopGamepadPoll {
    pub(crate) connected: Vec<(InputSourceId, InputSourceDescriptor)>,
    pub(crate) disconnected: Vec<InputSourceId>,
    pub(crate) input: ControllerInputBatch,
}

impl DesktopGamepadPoll {
    pub(crate) fn connected_count(&self) -> usize {
        self.input.terminal_snapshot_count()
    }
}

#[derive(Clone, Debug)]
struct DesktopGamepadSource {
    source_id: InputSourceId,
    connected: bool,
}

#[derive(Clone, Debug)]
struct ObservedGamepad {
    backend_id: usize,
    descriptor: InputSourceDescriptor,
    snapshot: StandardGamepadSnapshot,
}

#[derive(Clone, Debug)]
struct PendingGamepadObservation {
    backend_id: usize,
    descriptor: InputSourceDescriptor,
    sample_time: Duration,
    snapshot: StandardGamepadSnapshot,
}

pub(crate) struct DesktopGamepadCollector {
    gilrs: Gilrs,
    allocator: InputSourceIdAllocator,
    sources: BTreeMap<usize, DesktopGamepadSource>,
    started_at: Instant,
    started_system_time: SystemTime,
    next_observation_sequence: u64,
}

impl DesktopGamepadCollector {
    pub(crate) fn new() -> Result<Self> {
        let gilrs = GilrsBuilder::new()
            // Shared input owns dead zones and jitter/activity filtering.
            .with_default_filters(false)
            .with_force_feedback(false)
            .build()
            .map_err(|error| {
                anyhow::anyhow!("initialize GilRs desktop gamepad backend: {error}")
            })?;
        Ok(Self {
            gilrs,
            allocator: InputSourceIdAllocator::new(),
            sources: BTreeMap::new(),
            started_at: Instant::now(),
            started_system_time: SystemTime::now(),
            next_observation_sequence: 0,
        })
    }

    pub(crate) fn poll(&mut self) -> Result<DesktopGamepadPoll> {
        // GilRs updates its cached state one event at a time. Capture that
        // intermediate state before draining the next event.
        let mut pending_observations = Vec::new();
        let mut backend_dropped_observations = 0_u64;
        while let Some(event) = self.gilrs.next_event() {
            if matches!(event.event, EventType::Dropped) {
                backend_dropped_observations = backend_dropped_observations.saturating_add(1);
                continue;
            }
            if matches!(event.event, EventType::Disconnected) {
                continue;
            }
            let gamepad = self.gilrs.gamepad(event.id);
            pending_observations.push(PendingGamepadObservation {
                backend_id: usize::from(event.id),
                descriptor: descriptor_from_gilrs(&gamepad),
                sample_time: event
                    .time
                    .duration_since(self.started_system_time)
                    .unwrap_or(Duration::ZERO),
                snapshot: snapshot_from_gilrs(&gamepad),
            });
        }
        let sample_time = self.started_at.elapsed();
        let mut input = ControllerInputBatch::new(sample_time);
        input.note_dropped_observations(backend_dropped_observations);
        let mut poll = DesktopGamepadPoll {
            input,
            ..DesktopGamepadPoll::default()
        };

        for observation in pending_observations {
            let (source_id, newly_connected) =
                self.ensure_source_connected(observation.backend_id)?;
            if newly_connected {
                poll.connected.push((source_id, observation.descriptor));
            }
            poll.input.push_observation(ControllerInputObservation {
                source_id,
                sample_time: observation.sample_time,
                sequence: self.next_observation_sequence,
                snapshot: observation.snapshot,
            });
            self.next_observation_sequence = self.next_observation_sequence.saturating_add(1);
        }

        let observed = self
            .gilrs
            .gamepads()
            .map(|(id, gamepad)| ObservedGamepad {
                backend_id: usize::from(id),
                descriptor: descriptor_from_gilrs(&gamepad),
                snapshot: snapshot_from_gilrs(&gamepad),
            })
            .collect::<Vec<_>>();
        let observed_ids = observed
            .iter()
            .map(|gamepad| gamepad.backend_id)
            .collect::<BTreeSet<_>>();

        for gamepad in observed {
            let (source_id, newly_connected) = self.ensure_source_connected(gamepad.backend_id)?;
            if newly_connected {
                poll.connected.push((source_id, gamepad.descriptor));
            }
            poll.input
                .set_terminal_snapshot(source_id, gamepad.snapshot);
        }

        for (backend_id, source) in &mut self.sources {
            if source.connected && !observed_ids.contains(backend_id) {
                source.connected = false;
                poll.disconnected.push(source.source_id);
            }
        }
        Ok(poll)
    }

    fn ensure_source_connected(&mut self, backend_id: usize) -> Result<(InputSourceId, bool)> {
        if !self.sources.contains_key(&backend_id) {
            let source_id = self
                .allocator
                .allocate()
                .context("desktop gamepad source ID space exhausted")?;
            self.sources.insert(
                backend_id,
                DesktopGamepadSource {
                    source_id,
                    connected: false,
                },
            );
        }
        let source = self
            .sources
            .get_mut(&backend_id)
            .expect("desktop gamepad source exists");
        let newly_connected = !source.connected;
        source.connected = true;
        Ok((source.source_id, newly_connected))
    }
}

fn descriptor_from_gilrs(gamepad: &Gamepad<'_>) -> InputSourceDescriptor {
    let label = gamepad.name().trim();
    let display_label = (!label.is_empty()).then(|| label.to_owned());
    InputSourceDescriptor::standard_gamepad(
        classify_controller_layout(display_label.as_deref(), gamepad.vendor_id()),
        display_label,
    )
}

fn snapshot_from_gilrs(gamepad: &Gamepad<'_>) -> StandardGamepadSnapshot {
    let dpad_x = gamepad.value(Axis::DPadX);
    let dpad_y = gamepad.value(Axis::DPadY);
    StandardGamepadSnapshot {
        // GilRs uses the engine's desired convention: positive Y points up.
        left_stick: Vec2::new(
            gamepad.value(Axis::LeftStickX),
            gamepad.value(Axis::LeftStickY),
        ),
        right_stick: Vec2::new(
            gamepad.value(Axis::RightStickX),
            gamepad.value(Axis::RightStickY),
        ),
        buttons: StandardGamepadButtons {
            south: gilrs_button(gamepad, Button::South),
            east: gilrs_button(gamepad, Button::East),
            west: gilrs_button(gamepad, Button::West),
            north: gilrs_button(gamepad, Button::North),
            left_shoulder: gilrs_button(gamepad, Button::LeftTrigger),
            right_shoulder: gilrs_button(gamepad, Button::RightTrigger),
            left_trigger: gilrs_button(gamepad, Button::LeftTrigger2),
            right_trigger: gilrs_button(gamepad, Button::RightTrigger2),
            select: gilrs_button(gamepad, Button::Select),
            start: gilrs_button(gamepad, Button::Start),
            left_stick: gilrs_button(gamepad, Button::LeftThumb),
            right_stick: gilrs_button(gamepad, Button::RightThumb),
            dpad_up: merged_digital_button(gilrs_button(gamepad, Button::DPadUp), dpad_y > 0.5),
            dpad_down: merged_digital_button(
                gilrs_button(gamepad, Button::DPadDown),
                dpad_y < -0.5,
            ),
            dpad_left: merged_digital_button(
                gilrs_button(gamepad, Button::DPadLeft),
                dpad_x < -0.5,
            ),
            dpad_right: merged_digital_button(
                gilrs_button(gamepad, Button::DPadRight),
                dpad_x > 0.5,
            ),
            guide: gilrs_button(gamepad, Button::Mode),
        },
    }
    .normalized()
}

fn gilrs_button(gamepad: &Gamepad<'_>, button: Button) -> StandardGamepadButtonState {
    gamepad
        .button_data(button)
        .map(|state| StandardGamepadButtonState {
            value: state.value(),
            pressed: state.is_pressed(),
        })
        .unwrap_or(StandardGamepadButtonState::RELEASED)
}

fn merged_digital_button(
    button: StandardGamepadButtonState,
    axis_pressed: bool,
) -> StandardGamepadButtonState {
    if axis_pressed {
        StandardGamepadButtonState::pressed()
    } else {
        button
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpad_axis_fallback_preserves_named_button_values() {
        let analog = StandardGamepadButtonState {
            value: 0.75,
            pressed: true,
        };
        assert_eq!(merged_digital_button(analog, false), analog);
        assert_eq!(
            merged_digital_button(StandardGamepadButtonState::RELEASED, true),
            StandardGamepadButtonState::pressed()
        );
    }
}
