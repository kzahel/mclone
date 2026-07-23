//! Browser Gamepad API collection and W3C standard-mapping normalization.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use glam::Vec2;
use mclone_input::{
    ControllerInputBatch, ControllerInputObservation, InputSourceDescriptor, InputSourceId,
    InputSourceIdAllocator, StandardGamepadButtonState, StandardGamepadButtons,
    StandardGamepadSnapshot, classify_controller_layout,
};

const W3C_STANDARD_AXIS_COUNT: usize = 4;
const W3C_STANDARD_BUTTON_COUNT: usize = 17;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct BrowserGamepadButton {
    value: f32,
    pressed: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct BrowserGamepadState {
    index: u32,
    id: String,
    timestamp_millis: f64,
    standard_mapping: bool,
    axes: Vec<f32>,
    buttons: Vec<BrowserGamepadButton>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct BrowserGamepadPoll {
    pub(crate) connected: Vec<(InputSourceId, InputSourceDescriptor)>,
    pub(crate) disconnected: Vec<InputSourceId>,
    pub(crate) input: ControllerInputBatch,
}

impl BrowserGamepadPoll {
    pub(crate) fn connected_count(&self) -> usize {
        self.input.terminal_snapshot_count()
    }
}

#[derive(Clone, Debug)]
struct BrowserGamepadSource {
    source_id: InputSourceId,
    identity: String,
    connected: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct BrowserGamepadCollector {
    allocator: InputSourceIdAllocator,
    sources: BTreeMap<u32, BrowserGamepadSource>,
    next_observation_sequence: u64,
}

impl BrowserGamepadCollector {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn poll_browser(
        &mut self,
        now_millis: f64,
    ) -> Result<BrowserGamepadPoll, wasm_bindgen::JsValue> {
        self.update(
            duration_from_browser_millis(now_millis),
            browser_gamepad_states()?,
        )
        .map_err(|message| wasm_bindgen::JsValue::from_str(&message))
    }

    fn update(
        &mut self,
        sample_time: Duration,
        gamepads: impl IntoIterator<Item = BrowserGamepadState>,
    ) -> Result<BrowserGamepadPoll, String> {
        let gamepads = gamepads
            .into_iter()
            .filter_map(|gamepad| {
                normalize_standard_mapping(&gamepad).map(|snapshot| (gamepad, snapshot))
            })
            .collect::<Vec<_>>();
        let observed_indices = gamepads
            .iter()
            .map(|(gamepad, _)| gamepad.index)
            .collect::<BTreeSet<_>>();
        let mut poll = BrowserGamepadPoll {
            input: ControllerInputBatch::new(sample_time),
            ..BrowserGamepadPoll::default()
        };

        for (gamepad, snapshot) in gamepads {
            let replace = self
                .sources
                .get(&gamepad.index)
                .is_some_and(|source| source.identity != gamepad.id);
            if replace
                && let Some(previous) = self.sources.remove(&gamepad.index)
                && previous.connected
            {
                poll.disconnected.push(previous.source_id);
            }
            if !self.sources.contains_key(&gamepad.index) {
                let source_id = self
                    .allocator
                    .allocate()
                    .ok_or_else(|| "browser gamepad source ID space exhausted".to_owned())?;
                self.sources.insert(
                    gamepad.index,
                    BrowserGamepadSource {
                        source_id,
                        identity: gamepad.id.clone(),
                        connected: false,
                    },
                );
            }
            let source = self
                .sources
                .get_mut(&gamepad.index)
                .expect("browser gamepad source exists");
            if !source.connected {
                source.connected = true;
                let label = (!gamepad.id.trim().is_empty()).then(|| gamepad.id.clone());
                poll.connected.push((
                    source.source_id,
                    InputSourceDescriptor::standard_gamepad(
                        classify_controller_layout(label.as_deref(), None),
                        label,
                    ),
                ));
            }
            let observed_at =
                if gamepad.timestamp_millis.is_finite() && gamepad.timestamp_millis > 0.0 {
                    duration_from_browser_millis(gamepad.timestamp_millis)
                } else {
                    sample_time
                };
            poll.input.push_observation(ControllerInputObservation {
                source_id: source.source_id,
                sample_time: observed_at,
                sequence: self.next_observation_sequence,
                snapshot,
            });
            self.next_observation_sequence = self.next_observation_sequence.saturating_add(1);
        }

        for (index, source) in &mut self.sources {
            if source.connected && !observed_indices.contains(index) {
                source.connected = false;
                poll.disconnected.push(source.source_id);
            }
        }
        Ok(poll)
    }
}

fn normalize_standard_mapping(gamepad: &BrowserGamepadState) -> Option<StandardGamepadSnapshot> {
    if !gamepad.standard_mapping
        || gamepad.axes.len() < W3C_STANDARD_AXIS_COUNT
        || gamepad.buttons.len() < W3C_STANDARD_BUTTON_COUNT
    {
        return None;
    }
    let button = |index: usize| {
        let button = gamepad.buttons[index];
        StandardGamepadButtonState {
            value: button.value,
            pressed: button.pressed,
        }
    };
    Some(
        StandardGamepadSnapshot {
            // The browser standard mapping uses positive Y down; the shared
            // ordinary-controller convention uses positive Y up.
            left_stick: Vec2::new(gamepad.axes[0], -gamepad.axes[1]),
            right_stick: Vec2::new(gamepad.axes[2], -gamepad.axes[3]),
            buttons: StandardGamepadButtons {
                south: button(0),
                east: button(1),
                west: button(2),
                north: button(3),
                left_shoulder: button(4),
                right_shoulder: button(5),
                left_trigger: button(6),
                right_trigger: button(7),
                select: button(8),
                start: button(9),
                left_stick: button(10),
                right_stick: button(11),
                dpad_up: button(12),
                dpad_down: button(13),
                dpad_left: button(14),
                dpad_right: button(15),
                guide: button(16),
            },
        }
        .normalized(),
    )
}

fn duration_from_browser_millis(now_millis: f64) -> Duration {
    if now_millis.is_finite() && now_millis > 0.0 {
        Duration::from_secs_f64(now_millis / 1_000.0)
    } else {
        Duration::ZERO
    }
}

#[cfg(target_arch = "wasm32")]
fn browser_gamepad_states() -> Result<Vec<BrowserGamepadState>, wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;
    use web_sys::{Gamepad, GamepadButton, GamepadMappingType};

    let Some(window) = web_sys::window() else {
        return Ok(Vec::new());
    };
    let gamepads = window.navigator().get_gamepads()?;
    let mut states = Vec::new();
    for value in gamepads.iter() {
        if value.is_null() || value.is_undefined() {
            continue;
        }
        let gamepad = value.dyn_into::<Gamepad>()?;
        if !gamepad.connected() {
            continue;
        }
        let axes = gamepad
            .axes()
            .iter()
            .map(|value| value.as_f64().unwrap_or_default() as f32)
            .collect();
        let buttons = gamepad
            .buttons()
            .iter()
            .map(|value| {
                value
                    .dyn_into::<GamepadButton>()
                    .map(|button| BrowserGamepadButton {
                        value: button.value() as f32,
                        pressed: button.pressed(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        states.push(BrowserGamepadState {
            index: gamepad.index(),
            id: gamepad.id(),
            timestamp_millis: gamepad.timestamp(),
            standard_mapping: gamepad.mapping() == GamepadMappingType::Standard,
            axes,
            buttons,
        });
    }
    Ok(states)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standard_gamepad(index: u32, id: &str) -> BrowserGamepadState {
        let mut buttons = vec![BrowserGamepadButton::default(); W3C_STANDARD_BUTTON_COUNT];
        buttons[0] = BrowserGamepadButton {
            value: 1.0,
            pressed: true,
        };
        buttons[7] = BrowserGamepadButton {
            value: 0.75,
            pressed: false,
        };
        BrowserGamepadState {
            index,
            id: id.to_owned(),
            timestamp_millis: 0.0,
            standard_mapping: true,
            axes: vec![0.25, -0.5, -0.75, 1.0],
            buttons,
        }
    }

    #[test]
    fn w3c_standard_mapping_normalizes_axes_buttons_and_triggers() {
        let snapshot = normalize_standard_mapping(&standard_gamepad(0, "Xbox Controller"))
            .expect("standard mapping");
        assert_eq!(snapshot.left_stick, Vec2::new(0.25, 0.5));
        assert_eq!(snapshot.right_stick, Vec2::new(-0.6, -0.8));
        assert!(snapshot.buttons.south.pressed);
        assert_eq!(snapshot.buttons.right_trigger.value, 0.75);

        let mut nonstandard = standard_gamepad(0, "raw HID");
        nonstandard.standard_mapping = false;
        assert_eq!(normalize_standard_mapping(&nonstandard), None);
    }

    #[test]
    fn collector_preserves_reconnect_source_and_replaces_reused_index() {
        let mut collector = BrowserGamepadCollector::new();
        let connected = collector
            .update(Duration::ZERO, [standard_gamepad(2, "Xbox Controller")])
            .expect("connect");
        let source = connected.connected[0].0;
        assert_eq!(connected.input.sample_time(), Duration::ZERO);
        assert_eq!(connected.connected_count(), 1);

        let disconnected = collector
            .update(Duration::from_millis(1), [])
            .expect("disconnect");
        assert_eq!(disconnected.disconnected, [source]);
        let reconnected = collector
            .update(
                Duration::from_millis(2),
                [standard_gamepad(2, "Xbox Controller")],
            )
            .expect("reconnect");
        assert_eq!(reconnected.connected[0].0, source);

        let replaced = collector
            .update(
                Duration::from_millis(3),
                [standard_gamepad(2, "DualSense Wireless Controller")],
            )
            .expect("replace");
        assert_eq!(replaced.disconnected, [source]);
        assert_ne!(replaced.connected[0].0, source);
    }
}
