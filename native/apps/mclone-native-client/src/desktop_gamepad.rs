//! GilRs-backed ordinary gamepad collection for desktop flat and desktop XR.
//!
//! This adapter owns backend handles and hotplug bookkeeping only. Shared
//! dead zones, bindings, edges, repeat, contexts, and gameplay meaning stay in
//! `mclone-input` and `mclone-scene`.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use gilrs::{Axis, Button, Gamepad, Gilrs, GilrsBuilder};
use glam::Vec2;
use mclone_input::{
    InputSourceDescriptor, InputSourceId, InputSourceIdAllocator, StandardGamepadButtonState,
    StandardGamepadButtons, StandardGamepadSnapshot, classify_controller_layout,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct DesktopGamepadPoll {
    pub(crate) sample_time: Duration,
    pub(crate) connected: Vec<(InputSourceId, InputSourceDescriptor)>,
    pub(crate) disconnected: Vec<InputSourceId>,
    pub(crate) samples: Vec<(InputSourceId, StandardGamepadSnapshot)>,
}

impl DesktopGamepadPoll {
    pub(crate) fn connected_count(&self) -> usize {
        self.samples.len()
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

pub(crate) struct DesktopGamepadCollector {
    gilrs: Gilrs,
    allocator: InputSourceIdAllocator,
    sources: BTreeMap<usize, DesktopGamepadSource>,
    started_at: Instant,
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
        })
    }

    pub(crate) fn poll(&mut self) -> Result<DesktopGamepadPoll> {
        // GilRs updates its cached state while the event queue is drained.
        while self.gilrs.next_event().is_some() {}

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
        let mut poll = DesktopGamepadPoll {
            sample_time: self.started_at.elapsed(),
            ..DesktopGamepadPoll::default()
        };

        for gamepad in observed {
            let source = match self.sources.get_mut(&gamepad.backend_id) {
                Some(source) => source,
                None => {
                    let source_id = self
                        .allocator
                        .allocate()
                        .context("desktop gamepad source ID space exhausted")?;
                    self.sources.insert(
                        gamepad.backend_id,
                        DesktopGamepadSource {
                            source_id,
                            connected: false,
                        },
                    );
                    self.sources
                        .get_mut(&gamepad.backend_id)
                        .expect("inserted desktop gamepad source")
                }
            };
            if !source.connected {
                source.connected = true;
                poll.connected.push((source.source_id, gamepad.descriptor));
            }
            poll.samples.push((source.source_id, gamepad.snapshot));
        }

        for (backend_id, source) in &mut self.sources {
            if source.connected && !observed_ids.contains(backend_id) {
                source.connected = false;
                poll.disconnected.push(source.source_id);
            }
        }
        Ok(poll)
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
