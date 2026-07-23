//! Source-aware Android controller normalization shared by flat and XR apps.
//!
//! Android/Java glue emits raw device, key, and axis facts. This reducer owns
//! session-local device identity and the Android-to-standard layout only;
//! dead zones, bindings, edges, contexts, and gameplay meaning remain in
//! `mclone-input` and `mclone-scene`.

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Mutex, OnceLock},
    time::Duration,
};

use glam::Vec2;
use mclone_input::{
    ControllerInputBatch, ControllerInputObservation, InputSourceDescriptor, InputSourceId,
    InputSourceIdAllocator, StandardGamepadButtonState, StandardGamepadButtons,
    StandardGamepadSnapshot, classify_controller_layout,
};

pub const ANDROID_SOURCE_DPAD: u32 = 0x0000_0201;
pub const ANDROID_SOURCE_GAMEPAD: u32 = 0x0000_0401;
pub const ANDROID_SOURCE_JOYSTICK: u32 = 0x0100_0010;

pub const ANDROID_AXIS_X: u32 = 1 << 0;
pub const ANDROID_AXIS_Y: u32 = 1 << 1;
pub const ANDROID_AXIS_Z: u32 = 1 << 2;
pub const ANDROID_AXIS_RZ: u32 = 1 << 3;
pub const ANDROID_AXIS_RX: u32 = 1 << 4;
pub const ANDROID_AXIS_RY: u32 = 1 << 5;
pub const ANDROID_AXIS_HAT_X: u32 = 1 << 6;
pub const ANDROID_AXIS_HAT_Y: u32 = 1 << 7;
pub const ANDROID_AXIS_LTRIGGER: u32 = 1 << 8;
pub const ANDROID_AXIS_RTRIGGER: u32 = 1 << 9;
pub const ANDROID_AXIS_BRAKE: u32 = 1 << 10;
pub const ANDROID_AXIS_GAS: u32 = 1 << 11;

const KEYCODE_DPAD_UP: i32 = 19;
const KEYCODE_DPAD_DOWN: i32 = 20;
const KEYCODE_DPAD_LEFT: i32 = 21;
const KEYCODE_DPAD_RIGHT: i32 = 22;
const KEYCODE_BUTTON_A: i32 = 96;
const KEYCODE_BUTTON_B: i32 = 97;
const KEYCODE_BUTTON_X: i32 = 99;
const KEYCODE_BUTTON_Y: i32 = 100;
const KEYCODE_BUTTON_L1: i32 = 102;
const KEYCODE_BUTTON_R1: i32 = 103;
const KEYCODE_BUTTON_L2: i32 = 104;
const KEYCODE_BUTTON_R2: i32 = 105;
const KEYCODE_BUTTON_THUMBL: i32 = 106;
const KEYCODE_BUTTON_THUMBR: i32 = 107;
const KEYCODE_BUTTON_START: i32 = 108;
const KEYCODE_BUTTON_SELECT: i32 = 109;
const KEYCODE_BUTTON_MODE: i32 = 110;
const MAX_PENDING_ANDROID_CONTROLLER_EVENTS: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AndroidControllerDeviceChange {
    Added,
    Changed,
    Removed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AndroidControllerDevice {
    pub device_id: i32,
    pub sources: u32,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub axis_support: u32,
    pub display_label: Option<String>,
}

impl AndroidControllerDevice {
    pub fn descriptor(&self) -> InputSourceDescriptor {
        InputSourceDescriptor::standard_gamepad(
            classify_controller_layout(self.display_label.as_deref(), self.vendor_id),
            self.display_label.clone(),
        )
    }

    fn identity(&self) -> AndroidControllerIdentity {
        AndroidControllerIdentity {
            vendor_id: self.vendor_id,
            product_id: self.product_id,
            display_label: self.display_label.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AndroidControllerKeyAction {
    Down,
    Up,
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AndroidControllerMotion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rz: f32,
    pub rx: f32,
    pub ry: f32,
    pub hat_x: f32,
    pub hat_y: f32,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub brake: f32,
    pub gas: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AndroidControllerEvent {
    Discontinuity {
        dropped_events: u64,
    },
    Device {
        change: AndroidControllerDeviceChange,
        device: AndroidControllerDevice,
    },
    Key {
        device_id: i32,
        source: u32,
        event_time_millis: u64,
        key_code: i32,
        action: AndroidControllerKeyAction,
        repeat_count: i32,
    },
    Motion {
        device_id: i32,
        source: u32,
        event_time_millis: u64,
        axes: AndroidControllerMotion,
    },
}

#[derive(Clone, Debug, Default)]
pub struct AndroidControllerPoll {
    pub connected: Vec<(InputSourceId, InputSourceDescriptor)>,
    pub disconnected: Vec<InputSourceId>,
    pub input: ControllerInputBatch,
}

impl AndroidControllerPoll {
    pub fn connected_count(&self) -> usize {
        self.input.terminal_snapshot_count()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PendingAndroidControllerObservation {
    source_id: InputSourceId,
    event_time_millis: u64,
    snapshot: StandardGamepadSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AndroidControllerIdentity {
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    display_label: Option<String>,
}

#[derive(Clone, Debug)]
struct AndroidControllerState {
    source_id: InputSourceId,
    identity: AndroidControllerIdentity,
    descriptor: InputSourceDescriptor,
    axis_support: u32,
    connected: bool,
    digital_buttons: StandardGamepadButtons,
    axes: AndroidControllerMotion,
}

impl AndroidControllerState {
    fn new(source_id: InputSourceId, device: AndroidControllerDevice) -> Self {
        Self {
            source_id,
            identity: device.identity(),
            descriptor: device.descriptor(),
            axis_support: device.axis_support,
            connected: true,
            digital_buttons: StandardGamepadButtons::default(),
            axes: AndroidControllerMotion::default(),
        }
    }

    fn clear_controls(&mut self) {
        self.digital_buttons = StandardGamepadButtons::default();
        self.axes = AndroidControllerMotion::default();
    }

    fn snapshot(&self) -> StandardGamepadSnapshot {
        let right_stick = if self.axis_support & (ANDROID_AXIS_Z | ANDROID_AXIS_RZ) != 0 {
            Vec2::new(self.axes.z, -self.axes.rz)
        } else {
            Vec2::new(self.axes.rx, -self.axes.ry)
        };
        let left_trigger = if self.axis_support & ANDROID_AXIS_LTRIGGER != 0 {
            self.axes.left_trigger
        } else {
            self.axes.brake
        };
        let right_trigger = if self.axis_support & ANDROID_AXIS_RTRIGGER != 0 {
            self.axes.right_trigger
        } else {
            self.axes.gas
        };
        let mut buttons = self.digital_buttons;
        buttons.left_trigger = merge_analog_button(buttons.left_trigger, left_trigger);
        buttons.right_trigger = merge_analog_button(buttons.right_trigger, right_trigger);
        buttons.dpad_left = merge_digital_button(buttons.dpad_left, self.axes.hat_x < -0.5);
        buttons.dpad_right = merge_digital_button(buttons.dpad_right, self.axes.hat_x > 0.5);
        // Android's HAT_Y follows screen coordinates: negative is up.
        buttons.dpad_up = merge_digital_button(buttons.dpad_up, self.axes.hat_y < -0.5);
        buttons.dpad_down = merge_digital_button(buttons.dpad_down, self.axes.hat_y > 0.5);
        StandardGamepadSnapshot {
            // Android stick Y follows screen coordinates: positive is down.
            left_stick: Vec2::new(self.axes.x, -self.axes.y),
            right_stick,
            buttons,
        }
        .normalized()
    }
}

#[derive(Debug, Default)]
pub struct AndroidControllerCollector {
    allocator: InputSourceIdAllocator,
    devices: BTreeMap<i32, AndroidControllerState>,
    pending_connected: Vec<(InputSourceId, InputSourceDescriptor)>,
    pending_disconnected: Vec<InputSourceId>,
    pending_observations: Vec<PendingAndroidControllerObservation>,
    pending_dropped_events: u64,
    android_time_origin_millis: Option<u64>,
    input_time_origin: Duration,
    next_observation_sequence: u64,
}

impl AndroidControllerCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_event(&mut self, event: AndroidControllerEvent) -> bool {
        match event {
            AndroidControllerEvent::Discontinuity { dropped_events } => {
                self.pending_dropped_events =
                    self.pending_dropped_events.saturating_add(dropped_events);
                true
            }
            AndroidControllerEvent::Device { change, device } => {
                if change != AndroidControllerDeviceChange::Removed
                    && !is_android_controller_source(device.sources)
                {
                    return false;
                }
                self.handle_device(change, device);
                true
            }
            AndroidControllerEvent::Key {
                device_id,
                source,
                event_time_millis,
                key_code,
                action,
                repeat_count: _,
            } => {
                if !is_android_controller_source(source) {
                    return false;
                }
                let Some(pressed) = (match action {
                    AndroidControllerKeyAction::Down => Some(true),
                    AndroidControllerKeyAction::Up => Some(false),
                    AndroidControllerKeyAction::Other => None,
                }) else {
                    return true;
                };
                let state = self.ensure_implicit_device(device_id, source);
                let changed = map_key(&mut state.digital_buttons, key_code, pressed);
                if changed {
                    let observation = PendingAndroidControllerObservation {
                        source_id: state.source_id,
                        event_time_millis,
                        snapshot: state.snapshot(),
                    };
                    self.pending_observations.push(observation);
                }
                true
            }
            AndroidControllerEvent::Motion {
                device_id,
                source,
                event_time_millis,
                axes,
            } => {
                if !is_android_controller_source(source) {
                    return false;
                }
                let state = self.ensure_implicit_device(device_id, source);
                state.axes = axes;
                let observation = PendingAndroidControllerObservation {
                    source_id: state.source_id,
                    event_time_millis,
                    snapshot: state.snapshot(),
                };
                self.pending_observations.push(observation);
                true
            }
        }
    }

    pub fn handle_events(&mut self, events: impl IntoIterator<Item = AndroidControllerEvent>) {
        for event in events {
            self.handle_event(event);
        }
    }

    pub fn clear_controls_for_lifecycle(&mut self) {
        self.pending_observations.clear();
        self.pending_dropped_events = 0;
        for state in self.devices.values_mut() {
            state.clear_controls();
        }
    }

    /// Starts delivery to a newly constructed scene consumer without changing
    /// Android device/source identity.
    pub fn reannounce_connected_sources(&mut self) {
        self.pending_connected.clear();
        self.pending_disconnected.clear();
        self.pending_connected.extend(
            self.devices
                .values()
                .filter(|state| state.connected)
                .map(|state| (state.source_id, state.descriptor.clone())),
        );
    }

    pub fn poll(&mut self, sample_time: Duration) -> AndroidControllerPoll {
        self.initialize_time_mapping(sample_time);
        let mut input = ControllerInputBatch::new(sample_time);
        input.note_dropped_observations(std::mem::take(&mut self.pending_dropped_events));
        let pending_observations = std::mem::take(&mut self.pending_observations);
        for observation in pending_observations {
            input.push_observation(ControllerInputObservation {
                source_id: observation.source_id,
                sample_time: self.map_event_time(observation.event_time_millis),
                sequence: self.next_observation_sequence,
                snapshot: observation.snapshot,
            });
            self.next_observation_sequence = self.next_observation_sequence.saturating_add(1);
        }
        for state in self.devices.values().filter(|state| state.connected) {
            input.set_terminal_snapshot(state.source_id, state.snapshot());
        }
        AndroidControllerPoll {
            connected: std::mem::take(&mut self.pending_connected),
            disconnected: std::mem::take(&mut self.pending_disconnected),
            input,
        }
    }

    fn initialize_time_mapping(&mut self, sample_time: Duration) {
        if self.android_time_origin_millis.is_some() {
            return;
        }
        let Some(first) = self.pending_observations.first() else {
            return;
        };
        let latest_millis = self
            .pending_observations
            .iter()
            .map(|observation| observation.event_time_millis)
            .max()
            .unwrap_or(first.event_time_millis);
        self.android_time_origin_millis = Some(first.event_time_millis);
        self.input_time_origin = sample_time.saturating_sub(Duration::from_millis(
            latest_millis.saturating_sub(first.event_time_millis),
        ));
    }

    fn map_event_time(&self, event_time_millis: u64) -> Duration {
        let Some(origin) = self.android_time_origin_millis else {
            return self.input_time_origin;
        };
        self.input_time_origin.saturating_add(Duration::from_millis(
            event_time_millis.saturating_sub(origin),
        ))
    }

    fn handle_device(
        &mut self,
        change: AndroidControllerDeviceChange,
        device: AndroidControllerDevice,
    ) {
        if change == AndroidControllerDeviceChange::Removed {
            if let Some(state) = self.devices.get_mut(&device.device_id)
                && state.connected
            {
                state.connected = false;
                state.clear_controls();
                self.pending_disconnected.push(state.source_id);
            }
            return;
        }

        let identity = device.identity();
        if let Some(state) = self.devices.get_mut(&device.device_id)
            && state.identity == identity
        {
            let descriptor = device.descriptor();
            let descriptor_changed = state.descriptor != descriptor;
            state.axis_support = device.axis_support;
            state.descriptor = descriptor;
            if !state.connected {
                state.connected = true;
                state.clear_controls();
                self.pending_connected
                    .push((state.source_id, state.descriptor.clone()));
            } else if descriptor_changed {
                self.pending_disconnected.push(state.source_id);
                self.pending_connected
                    .push((state.source_id, state.descriptor.clone()));
            }
            return;
        }

        if let Some(previous) = self.devices.remove(&device.device_id)
            && previous.connected
        {
            self.pending_disconnected.push(previous.source_id);
        }
        let Some(source_id) = self.allocator.allocate() else {
            return;
        };
        let state = AndroidControllerState::new(source_id, device.clone());
        self.pending_connected
            .push((source_id, state.descriptor.clone()));
        self.devices.insert(device.device_id, state);
    }

    fn ensure_implicit_device(
        &mut self,
        device_id: i32,
        source: u32,
    ) -> &mut AndroidControllerState {
        if !self.devices.contains_key(&device_id) {
            self.handle_device(
                AndroidControllerDeviceChange::Added,
                AndroidControllerDevice {
                    device_id,
                    sources: source,
                    vendor_id: None,
                    product_id: None,
                    axis_support: ANDROID_AXIS_X
                        | ANDROID_AXIS_Y
                        | ANDROID_AXIS_Z
                        | ANDROID_AXIS_RZ
                        | ANDROID_AXIS_HAT_X
                        | ANDROID_AXIS_HAT_Y
                        | ANDROID_AXIS_LTRIGGER
                        | ANDROID_AXIS_RTRIGGER,
                    display_label: Some(format!("Android controller {device_id}")),
                },
            );
        }
        let state = self
            .devices
            .get_mut(&device_id)
            .expect("implicit Android controller was inserted");
        if !state.connected {
            state.connected = true;
            state.clear_controls();
            self.pending_connected
                .push((state.source_id, state.descriptor.clone()));
        }
        state
    }
}

pub fn is_android_controller_source(source: u32) -> bool {
    [
        ANDROID_SOURCE_GAMEPAD,
        ANDROID_SOURCE_JOYSTICK,
        ANDROID_SOURCE_DPAD,
    ]
    .into_iter()
    .any(|expected| source & expected == expected)
}

fn map_key(buttons: &mut StandardGamepadButtons, key_code: i32, pressed: bool) -> bool {
    let state = StandardGamepadButtonState {
        value: if pressed { 1.0 } else { 0.0 },
        pressed,
    };
    let target = match key_code {
        KEYCODE_BUTTON_A => &mut buttons.south,
        KEYCODE_BUTTON_B => &mut buttons.east,
        KEYCODE_BUTTON_X => &mut buttons.west,
        KEYCODE_BUTTON_Y => &mut buttons.north,
        KEYCODE_BUTTON_L1 => &mut buttons.left_shoulder,
        KEYCODE_BUTTON_R1 => &mut buttons.right_shoulder,
        KEYCODE_BUTTON_L2 => &mut buttons.left_trigger,
        KEYCODE_BUTTON_R2 => &mut buttons.right_trigger,
        KEYCODE_BUTTON_THUMBL => &mut buttons.left_stick,
        KEYCODE_BUTTON_THUMBR => &mut buttons.right_stick,
        KEYCODE_BUTTON_START => &mut buttons.start,
        KEYCODE_BUTTON_SELECT => &mut buttons.select,
        KEYCODE_BUTTON_MODE => &mut buttons.guide,
        KEYCODE_DPAD_UP => &mut buttons.dpad_up,
        KEYCODE_DPAD_DOWN => &mut buttons.dpad_down,
        KEYCODE_DPAD_LEFT => &mut buttons.dpad_left,
        KEYCODE_DPAD_RIGHT => &mut buttons.dpad_right,
        _ => return false,
    };
    *target = state;
    true
}

fn merge_analog_button(
    digital: StandardGamepadButtonState,
    analog_value: f32,
) -> StandardGamepadButtonState {
    let value = if analog_value.is_finite() {
        analog_value.clamp(0.0, 1.0)
    } else {
        0.0
    };
    StandardGamepadButtonState {
        value: digital.value.max(value),
        pressed: digital.pressed || value > 0.5,
    }
}

fn merge_digital_button(
    digital: StandardGamepadButtonState,
    axis_pressed: bool,
) -> StandardGamepadButtonState {
    if axis_pressed {
        StandardGamepadButtonState::pressed()
    } else {
        digital
    }
}

#[derive(Debug, Default)]
struct AndroidControllerEventQueue {
    events: VecDeque<AndroidControllerEvent>,
    dropped_events: u64,
}

static ANDROID_CONTROLLER_EVENT_QUEUE: OnceLock<Mutex<AndroidControllerEventQueue>> =
    OnceLock::new();

pub fn enqueue_android_controller_event(event: AndroidControllerEvent) {
    let mut queue = ANDROID_CONTROLLER_EVENT_QUEUE
        .get_or_init(|| Mutex::new(AndroidControllerEventQueue::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if queue.events.len() == MAX_PENDING_ANDROID_CONTROLLER_EVENTS {
        queue.events.pop_front();
        queue.dropped_events = queue.dropped_events.saturating_add(1);
    }
    queue.events.push_back(event);
}

pub fn drain_android_controller_events() -> Vec<AndroidControllerEvent> {
    let mut queue = ANDROID_CONTROLLER_EVENT_QUEUE
        .get_or_init(|| Mutex::new(AndroidControllerEventQueue::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut drained = Vec::with_capacity(queue.events.len().saturating_add(1));
    if queue.dropped_events > 0 {
        drained.push(AndroidControllerEvent::Discontinuity {
            dropped_events: std::mem::take(&mut queue.dropped_events),
        });
    }
    drained.extend(queue.events.drain(..));
    drained
}

/// Exports the fixed JNI entry points used by the shared Java controller
/// bridge. Invoke once from each Android app cdylib.
#[macro_export]
macro_rules! export_android_controller_jni_bridge {
    () => {
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "system" fn Java_com_kzahel_mclone_controller_ControllerInputBridge_nativeControllerDevice<'caller>(
            mut unowned_env: jni::EnvUnowned<'caller>,
            _class: jni::objects::JClass<'caller>,
            change: jni::sys::jint,
            device_id: jni::sys::jint,
            sources: jni::sys::jint,
            vendor_id: jni::sys::jint,
            product_id: jni::sys::jint,
            axis_support: jni::sys::jint,
            display_label: jni::objects::JString<'caller>,
        ) {
            use jni::errors::ThrowRuntimeExAndDefault as _;
            unowned_env
                .with_env(|env| -> jni::errors::Result<()> {
                    let display_label = display_label
                        .try_to_string(env)
                        .ok()
                        .filter(|label| !label.trim().is_empty());
                    let change = match change {
                        0 => $crate::AndroidControllerDeviceChange::Added,
                        1 => $crate::AndroidControllerDeviceChange::Changed,
                        2 => $crate::AndroidControllerDeviceChange::Removed,
                        _ => return Ok(()),
                    };
                    $crate::enqueue_android_controller_event(
                        $crate::AndroidControllerEvent::Device {
                            change,
                            device: $crate::AndroidControllerDevice {
                                device_id,
                                sources: sources as u32,
                                vendor_id: u16::try_from(vendor_id)
                                    .ok()
                                    .filter(|id| *id != 0),
                                product_id: u16::try_from(product_id)
                                    .ok()
                                    .filter(|id| *id != 0),
                                axis_support: axis_support as u32,
                                display_label,
                            },
                        },
                    );
                    Ok(())
                })
                .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
        }

        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "system" fn Java_com_kzahel_mclone_controller_ControllerInputBridge_nativeControllerKey<'caller>(
            mut unowned_env: jni::EnvUnowned<'caller>,
            _class: jni::objects::JClass<'caller>,
            device_id: jni::sys::jint,
            source: jni::sys::jint,
            event_time_millis: jni::sys::jlong,
            key_code: jni::sys::jint,
            action: jni::sys::jint,
            repeat_count: jni::sys::jint,
        ) {
            use jni::errors::ThrowRuntimeExAndDefault as _;
            unowned_env
                .with_env(|_| -> jni::errors::Result<()> {
                    let action = match action {
                        0 => $crate::AndroidControllerKeyAction::Down,
                        1 => $crate::AndroidControllerKeyAction::Up,
                        _ => $crate::AndroidControllerKeyAction::Other,
                    };
                    $crate::enqueue_android_controller_event(
                        $crate::AndroidControllerEvent::Key {
                            device_id,
                            source: source as u32,
                            event_time_millis: u64::try_from(event_time_millis)
                                .unwrap_or_default(),
                            key_code,
                            action,
                            repeat_count,
                        },
                    );
                    Ok(())
                })
                .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
        }

        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "system" fn Java_com_kzahel_mclone_controller_ControllerInputBridge_nativeControllerMotion<'caller>(
            mut unowned_env: jni::EnvUnowned<'caller>,
            _class: jni::objects::JClass<'caller>,
            device_id: jni::sys::jint,
            source: jni::sys::jint,
            event_time_millis: jni::sys::jlong,
            x: jni::sys::jfloat,
            y: jni::sys::jfloat,
            z: jni::sys::jfloat,
            rz: jni::sys::jfloat,
            rx: jni::sys::jfloat,
            ry: jni::sys::jfloat,
            hat_x: jni::sys::jfloat,
            hat_y: jni::sys::jfloat,
            left_trigger: jni::sys::jfloat,
            right_trigger: jni::sys::jfloat,
            brake: jni::sys::jfloat,
            gas: jni::sys::jfloat,
        ) {
            use jni::errors::ThrowRuntimeExAndDefault as _;
            unowned_env
                .with_env(|_| -> jni::errors::Result<()> {
                    $crate::enqueue_android_controller_event(
                        $crate::AndroidControllerEvent::Motion {
                            device_id,
                            source: source as u32,
                            event_time_millis: u64::try_from(event_time_millis)
                                .unwrap_or_default(),
                            axes: $crate::AndroidControllerMotion {
                                x,
                                y,
                                z,
                                rz,
                                rx,
                                ry,
                                hat_x,
                                hat_y,
                                left_trigger,
                                right_trigger,
                                brake,
                                gas,
                            },
                        },
                    );
                    Ok(())
                })
                .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_input::ControllerLayoutFamily;

    fn xbox_device(device_id: i32) -> AndroidControllerDevice {
        AndroidControllerDevice {
            device_id,
            sources: ANDROID_SOURCE_GAMEPAD | ANDROID_SOURCE_JOYSTICK,
            vendor_id: Some(0x045e),
            product_id: Some(0x02ea),
            axis_support: ANDROID_AXIS_X
                | ANDROID_AXIS_Y
                | ANDROID_AXIS_Z
                | ANDROID_AXIS_RZ
                | ANDROID_AXIS_HAT_X
                | ANDROID_AXIS_HAT_Y
                | ANDROID_AXIS_BRAKE
                | ANDROID_AXIS_GAS,
            display_label: Some("Xbox Wireless Controller".to_owned()),
        }
    }

    #[test]
    fn source_masks_accept_composite_controller_sources_only() {
        assert!(is_android_controller_source(ANDROID_SOURCE_GAMEPAD));
        assert!(is_android_controller_source(
            ANDROID_SOURCE_GAMEPAD | ANDROID_SOURCE_JOYSTICK
        ));
        assert!(is_android_controller_source(ANDROID_SOURCE_DPAD));
        assert!(!is_android_controller_source(0x0000_1002));
    }

    #[test]
    fn axes_keys_and_fallback_triggers_normalize_to_standard_snapshot() {
        let mut collector = AndroidControllerCollector::new();
        collector.handle_event(AndroidControllerEvent::Device {
            change: AndroidControllerDeviceChange::Added,
            device: xbox_device(7),
        });
        collector.handle_event(AndroidControllerEvent::Motion {
            device_id: 7,
            source: ANDROID_SOURCE_JOYSTICK,
            event_time_millis: 1_000,
            axes: AndroidControllerMotion {
                x: 0.25,
                y: -0.5,
                z: -0.75,
                rz: 1.0,
                hat_x: -1.0,
                brake: 0.4,
                gas: 0.8,
                ..AndroidControllerMotion::default()
            },
        });
        collector.handle_event(AndroidControllerEvent::Key {
            device_id: 7,
            source: ANDROID_SOURCE_GAMEPAD,
            event_time_millis: 1_004,
            key_code: KEYCODE_BUTTON_A,
            action: AndroidControllerKeyAction::Down,
            repeat_count: 0,
        });

        let poll = collector.poll(Duration::from_millis(16));
        assert_eq!(poll.connected.len(), 1);
        assert_eq!(
            poll.connected[0].1.controller_layout,
            ControllerLayoutFamily::XboxLike
        );
        let snapshot = poll
            .input
            .terminal_snapshots()
            .next()
            .expect("terminal snapshot")
            .1;
        assert_eq!(snapshot.left_stick, Vec2::new(0.25, 0.5));
        assert_eq!(snapshot.right_stick, Vec2::new(-0.6, -0.8));
        assert!(snapshot.buttons.south.pressed);
        assert!(snapshot.buttons.dpad_left.pressed);
        assert_eq!(snapshot.buttons.left_trigger.value, 0.4);
        assert_eq!(snapshot.buttons.right_trigger.value, 0.8);
    }

    #[test]
    fn repeat_disconnect_reconnect_and_device_id_reuse_are_deterministic() {
        let mut collector = AndroidControllerCollector::new();
        let device = xbox_device(9);
        collector.handle_event(AndroidControllerEvent::Device {
            change: AndroidControllerDeviceChange::Added,
            device: device.clone(),
        });
        let first = collector.poll(Duration::ZERO);
        let first_id = first.connected[0].0;

        collector.handle_event(AndroidControllerEvent::Key {
            device_id: 9,
            source: ANDROID_SOURCE_GAMEPAD,
            event_time_millis: 1_000,
            key_code: KEYCODE_BUTTON_A,
            action: AndroidControllerKeyAction::Down,
            repeat_count: 4,
        });
        assert!(
            collector
                .poll(Duration::from_millis(1))
                .input
                .terminal_snapshots()
                .next()
                .expect("terminal snapshot")
                .1
                .buttons
                .south
                .pressed
        );
        collector.handle_event(AndroidControllerEvent::Device {
            change: AndroidControllerDeviceChange::Removed,
            device: device.clone(),
        });
        let removed = collector.poll(Duration::from_millis(2));
        assert_eq!(removed.disconnected, vec![first_id]);
        assert_eq!(removed.input.terminal_snapshot_count(), 0);

        collector.handle_event(AndroidControllerEvent::Device {
            change: AndroidControllerDeviceChange::Added,
            device: device.clone(),
        });
        let reconnected = collector.poll(Duration::from_millis(3));
        assert_eq!(reconnected.connected[0].0, first_id);
        assert!(
            !reconnected
                .input
                .terminal_snapshots()
                .next()
                .expect("terminal snapshot")
                .1
                .buttons
                .south
                .pressed
        );

        let mut replacement = device;
        replacement.vendor_id = Some(0x054c);
        replacement.product_id = Some(0x0ce6);
        replacement.display_label = Some("DualSense Wireless Controller".to_owned());
        collector.handle_event(AndroidControllerEvent::Device {
            change: AndroidControllerDeviceChange::Changed,
            device: replacement,
        });
        let replaced = collector.poll(Duration::from_millis(4));
        assert_eq!(replaced.disconnected, vec![first_id]);
        assert_ne!(replaced.connected[0].0, first_id);
        assert_eq!(
            replaced.connected[0].1.controller_layout,
            ControllerLayoutFamily::PlayStationLike
        );
    }

    #[test]
    fn lifecycle_clear_neutralizes_cached_controls_without_disconnect() {
        let mut collector = AndroidControllerCollector::new();
        collector.handle_event(AndroidControllerEvent::Device {
            change: AndroidControllerDeviceChange::Added,
            device: xbox_device(3),
        });
        collector.handle_event(AndroidControllerEvent::Key {
            device_id: 3,
            source: ANDROID_SOURCE_GAMEPAD,
            event_time_millis: 1_000,
            key_code: KEYCODE_BUTTON_START,
            action: AndroidControllerKeyAction::Down,
            repeat_count: 0,
        });
        collector.clear_controls_for_lifecycle();
        let poll = collector.poll(Duration::ZERO);
        assert_eq!(poll.connected.len(), 1);
        assert!(
            !poll
                .input
                .terminal_snapshots()
                .next()
                .expect("terminal snapshot")
                .1
                .buttons
                .start
                .pressed
        );
    }

    #[test]
    fn new_scene_consumer_gets_only_current_connections() {
        let mut collector = AndroidControllerCollector::new();
        let first = xbox_device(3);
        let second = xbox_device(4);
        collector.handle_event(AndroidControllerEvent::Device {
            change: AndroidControllerDeviceChange::Added,
            device: first.clone(),
        });
        let first_id = collector.poll(Duration::ZERO).connected[0].0;
        collector.handle_event(AndroidControllerEvent::Device {
            change: AndroidControllerDeviceChange::Removed,
            device: first,
        });
        collector.handle_event(AndroidControllerEvent::Device {
            change: AndroidControllerDeviceChange::Added,
            device: second,
        });

        collector.reannounce_connected_sources();
        let poll = collector.poll(Duration::from_millis(1));
        assert!(poll.disconnected.is_empty());
        assert_eq!(poll.connected.len(), 1);
        assert_ne!(poll.connected[0].0, first_id);
        assert_eq!(poll.input.terminal_snapshot_count(), 1);
    }

    #[test]
    fn key_edges_and_historical_motion_keep_android_event_order() {
        let mut collector = AndroidControllerCollector::new();
        collector.handle_event(AndroidControllerEvent::Device {
            change: AndroidControllerDeviceChange::Added,
            device: xbox_device(11),
        });
        collector.handle_event(AndroidControllerEvent::Key {
            device_id: 11,
            source: ANDROID_SOURCE_GAMEPAD,
            event_time_millis: 10_000,
            key_code: KEYCODE_BUTTON_A,
            action: AndroidControllerKeyAction::Down,
            repeat_count: 0,
        });
        collector.handle_event(AndroidControllerEvent::Motion {
            device_id: 11,
            source: ANDROID_SOURCE_JOYSTICK,
            event_time_millis: 10_004,
            axes: AndroidControllerMotion {
                x: 0.25,
                ..AndroidControllerMotion::default()
            },
        });
        collector.handle_event(AndroidControllerEvent::Motion {
            device_id: 11,
            source: ANDROID_SOURCE_JOYSTICK,
            event_time_millis: 10_008,
            axes: AndroidControllerMotion {
                x: 0.75,
                ..AndroidControllerMotion::default()
            },
        });
        collector.handle_event(AndroidControllerEvent::Key {
            device_id: 11,
            source: ANDROID_SOURCE_GAMEPAD,
            event_time_millis: 10_012,
            key_code: KEYCODE_BUTTON_A,
            action: AndroidControllerKeyAction::Up,
            repeat_count: 0,
        });

        let poll = collector.poll(Duration::from_millis(20));
        assert_eq!(poll.input.observations().len(), 4);
        assert!(poll.input.observations()[0].snapshot.buttons.south.pressed);
        assert_eq!(poll.input.observations()[1].snapshot.left_stick.x, 0.25);
        assert_eq!(poll.input.observations()[2].snapshot.left_stick.x, 0.75);
        assert!(!poll.input.observations()[3].snapshot.buttons.south.pressed);
        assert!(
            poll.input
                .observations()
                .windows(2)
                .all(|pair| pair[0].sample_time < pair[1].sample_time)
        );
    }
}
