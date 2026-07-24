//! Ordinary gamepad collection for desktop flat and desktop XR.
//!
//! This adapter owns backend handles and hotplug bookkeeping only. Shared
//! dead zones, bindings, edges, repeat, contexts, and gameplay meaning stay in
//! `mclone-input` and `mclone-scene`. macOS uses Apple GameController profiles;
//! Linux and Windows use GilRs.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
#[cfg(not(target_os = "macos"))]
use gilrs::{Axis, Button, EventType, Gamepad, Gilrs, GilrsBuilder};
use glam::Vec2;
#[cfg(target_os = "macos")]
use mclone_input::MAX_CONTROLLER_OBSERVATIONS_PER_BATCH;
use mclone_input::{
    ControllerInputBatch, ControllerInputObservation, InputSourceDescriptor, InputSourceId,
    InputSourceIdAllocator, StandardGamepadButtonState, StandardGamepadButtons,
    StandardGamepadSnapshot, classify_controller_layout,
};
#[cfg(not(target_os = "macos"))]
use std::time::SystemTime;
#[cfg(target_os = "macos")]
use {
    block2::RcBlock,
    objc2::rc::{Retained, autoreleasepool},
    objc2_game_controller::{
        GCController, GCControllerButtonInput, GCControllerElement, GCDevice, GCExtendedGamepad,
    },
    std::{cell::RefCell, collections::VecDeque, ptr::NonNull, rc::Rc},
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
#[cfg(not(target_os = "macos"))]
struct DesktopGamepadSource {
    source_id: InputSourceId,
    connected: bool,
}

#[derive(Clone, Debug)]
#[cfg(not(target_os = "macos"))]
struct ObservedGamepad {
    backend_id: usize,
    descriptor: InputSourceDescriptor,
    snapshot: StandardGamepadSnapshot,
}

#[derive(Clone, Debug)]
#[cfg(not(target_os = "macos"))]
struct PendingGamepadObservation {
    backend_id: usize,
    descriptor: InputSourceDescriptor,
    sample_time: Duration,
    snapshot: StandardGamepadSnapshot,
}

#[cfg(not(target_os = "macos"))]
pub(crate) struct DesktopGamepadCollector {
    gilrs: Gilrs,
    allocator: InputSourceIdAllocator,
    sources: BTreeMap<usize, DesktopGamepadSource>,
    started_at: Instant,
    started_system_time: SystemTime,
    next_observation_sequence: u64,
}

#[cfg(not(target_os = "macos"))]
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

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct PendingAppleObservation {
    source_id: InputSourceId,
    sample_time: Duration,
    sequence: u64,
    snapshot: StandardGamepadSnapshot,
}

#[cfg(target_os = "macos")]
#[derive(Default)]
struct AppleObservationQueue {
    observations: VecDeque<PendingAppleObservation>,
    dropped_observations: u64,
    next_sequence: u64,
}

#[cfg(target_os = "macos")]
impl AppleObservationQueue {
    fn push(
        &mut self,
        source_id: InputSourceId,
        sample_time: Duration,
        snapshot: StandardGamepadSnapshot,
    ) {
        if self.observations.len() == MAX_CONTROLLER_OBSERVATIONS_PER_BATCH {
            self.observations.pop_front();
            self.dropped_observations = self.dropped_observations.saturating_add(1);
        }
        self.observations.push_back(PendingAppleObservation {
            source_id,
            sample_time,
            sequence: self.next_sequence,
            snapshot,
        });
        self.next_sequence = self.next_sequence.saturating_add(1);
    }
}

#[cfg(target_os = "macos")]
struct AppleGamepadSource {
    source_id: InputSourceId,
    profile: Retained<GCExtendedGamepad>,
}

/// Native GameController-backed collector for macOS.
///
/// Apple recommends GameController profiles instead of raw IOKit HID
/// interpretation for current Xbox, PlayStation, and third-party controllers.
/// In particular, macOS 14+ may expose a physical device and an optional
/// synthetic HID compatibility device; treating either as a generic HID
/// layout produces incorrect stick/trigger mappings.
#[cfg(target_os = "macos")]
pub(crate) struct DesktopGamepadCollector {
    allocator: InputSourceIdAllocator,
    sources: BTreeMap<usize, AppleGamepadSource>,
    observations: Rc<RefCell<AppleObservationQueue>>,
    started_at: Instant,
}

#[cfg(target_os = "macos")]
impl DesktopGamepadCollector {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            allocator: InputSourceIdAllocator::new(),
            sources: BTreeMap::new(),
            observations: Rc::new(RefCell::new(AppleObservationQueue::default())),
            started_at: Instant::now(),
        })
    }

    pub(crate) fn poll(&mut self) -> Result<DesktopGamepadPoll> {
        let controllers = autoreleasepool(|_| unsafe { GCController::controllers() });
        let mut observed_keys = BTreeSet::new();
        let mut terminal_snapshots = Vec::new();
        let mut poll = DesktopGamepadPoll::default();

        for controller in controllers {
            let controller_key = Retained::as_ptr(&controller) as usize;
            let Some(profile) = (unsafe { controller.extendedGamepad() }) else {
                continue;
            };
            observed_keys.insert(controller_key);

            if !self.sources.contains_key(&controller_key) {
                let source_id = self
                    .allocator
                    .allocate()
                    .context("desktop gamepad source ID space exhausted")?;
                let descriptor = descriptor_from_apple_controller(&controller);
                self.install_apple_observation_handler(source_id, &profile);
                log::info!(
                    "macOS GameController connected: source={source_id:?}, label={}, layout={:?}",
                    descriptor.display_label.as_deref().unwrap_or("unknown"),
                    descriptor.controller_layout
                );
                self.sources.insert(
                    controller_key,
                    AppleGamepadSource {
                        source_id,
                        profile: profile.clone(),
                    },
                );
                poll.connected.push((source_id, descriptor));
            }

            let snapshot = unsafe { snapshot_from_apple_gamepad(&profile) };
            let source = self
                .sources
                .get(&controller_key)
                .expect("observed Apple gamepad source exists");
            terminal_snapshots.push((source.source_id, snapshot));
        }

        let disconnected_keys = self
            .sources
            .keys()
            .filter(|key| !observed_keys.contains(key))
            .copied()
            .collect::<Vec<_>>();
        for controller_key in disconnected_keys {
            let source = self
                .sources
                .remove(&controller_key)
                .expect("disconnected Apple gamepad source exists");
            unsafe {
                source.profile.setValueChangedHandler(std::ptr::null_mut());
            }
            poll.disconnected.push(source.source_id);
        }

        let sample_time = self.started_at.elapsed();
        let mut input = ControllerInputBatch::new(sample_time);
        {
            let mut observations = self.observations.borrow_mut();
            input.note_dropped_observations(observations.dropped_observations);
            observations.dropped_observations = 0;
            while let Some(observation) = observations.observations.pop_front() {
                if self
                    .sources
                    .values()
                    .any(|source| source.source_id == observation.source_id)
                {
                    input.push_observation(ControllerInputObservation {
                        source_id: observation.source_id,
                        sample_time: observation.sample_time,
                        sequence: observation.sequence,
                        snapshot: observation.snapshot,
                    });
                }
            }
        }
        for (source_id, snapshot) in terminal_snapshots {
            input.set_terminal_snapshot(source_id, snapshot);
        }
        poll.input = input;
        Ok(poll)
    }

    fn install_apple_observation_handler(
        &self,
        source_id: InputSourceId,
        profile: &GCExtendedGamepad,
    ) {
        // GameController submits handlers on the main queue by default. Both
        // desktop product loops own this collector on that same thread.
        let observations = Rc::clone(&self.observations);
        let started_at = self.started_at;
        let handler: RcBlock<dyn Fn(NonNull<GCExtendedGamepad>, NonNull<GCControllerElement>)> =
            RcBlock::new(
                move |gamepad: NonNull<GCExtendedGamepad>,
                      _element: NonNull<GCControllerElement>| {
                    // SAFETY: GameController guarantees both block arguments remain
                    // valid for the duration of this callback.
                    let snapshot = unsafe { snapshot_from_apple_gamepad(gamepad.as_ref()) };
                    observations
                        .borrow_mut()
                        .push(source_id, started_at.elapsed(), snapshot);
                },
            );
        // SAFETY: The block signature matches GCExtendedGamepad's declared
        // handler type. The Objective-C property copies the block.
        unsafe {
            profile.setValueChangedHandler(RcBlock::as_ptr(&handler));
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for DesktopGamepadCollector {
    fn drop(&mut self) {
        for source in self.sources.values() {
            unsafe {
                source.profile.setValueChangedHandler(std::ptr::null_mut());
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn descriptor_from_apple_controller(controller: &GCController) -> InputSourceDescriptor {
    let vendor = unsafe { controller.vendorName() }.map(|name| name.to_string());
    let category = unsafe { controller.productCategory() }.to_string();
    let display_label = match vendor.as_deref() {
        Some(vendor) if !vendor.trim().is_empty() && vendor != "Controller" => {
            Some(format!("{vendor} ({category})"))
        }
        _ if !category.trim().is_empty() => Some(category),
        _ => vendor,
    };
    InputSourceDescriptor::standard_gamepad(
        classify_controller_layout(display_label.as_deref(), None),
        display_label,
    )
}

#[cfg(target_os = "macos")]
unsafe fn snapshot_from_apple_gamepad(gamepad: &GCExtendedGamepad) -> StandardGamepadSnapshot {
    let left_stick = unsafe { gamepad.leftThumbstick() };
    let right_stick = unsafe { gamepad.rightThumbstick() };
    let dpad = unsafe { gamepad.dpad() };
    StandardGamepadSnapshot {
        left_stick: Vec2::new(unsafe { left_stick.xAxis().value() }, unsafe {
            left_stick.yAxis().value()
        }),
        right_stick: Vec2::new(unsafe { right_stick.xAxis().value() }, unsafe {
            right_stick.yAxis().value()
        }),
        buttons: StandardGamepadButtons {
            south: unsafe { apple_button(&gamepad.buttonA()) },
            east: unsafe { apple_button(&gamepad.buttonB()) },
            west: unsafe { apple_button(&gamepad.buttonX()) },
            north: unsafe { apple_button(&gamepad.buttonY()) },
            left_shoulder: unsafe { apple_button(&gamepad.leftShoulder()) },
            right_shoulder: unsafe { apple_button(&gamepad.rightShoulder()) },
            left_trigger: unsafe { apple_button(&gamepad.leftTrigger()) },
            right_trigger: unsafe { apple_button(&gamepad.rightTrigger()) },
            select: unsafe { apple_optional_button(gamepad.buttonOptions()) },
            start: unsafe { apple_button(&gamepad.buttonMenu()) },
            left_stick: unsafe { apple_optional_button(gamepad.leftThumbstickButton()) },
            right_stick: unsafe { apple_optional_button(gamepad.rightThumbstickButton()) },
            dpad_up: unsafe { apple_button(&dpad.up()) },
            dpad_down: unsafe { apple_button(&dpad.down()) },
            dpad_left: unsafe { apple_button(&dpad.left()) },
            dpad_right: unsafe { apple_button(&dpad.right()) },
            guide: unsafe { apple_optional_button(gamepad.buttonHome()) },
        },
    }
    .normalized()
}

#[cfg(target_os = "macos")]
unsafe fn apple_button(button: &GCControllerButtonInput) -> StandardGamepadButtonState {
    StandardGamepadButtonState {
        value: unsafe { button.value() },
        pressed: unsafe { button.isPressed() },
    }
}

#[cfg(target_os = "macos")]
unsafe fn apple_optional_button(
    button: Option<Retained<GCControllerButtonInput>>,
) -> StandardGamepadButtonState {
    button
        .as_deref()
        .map(|button| unsafe { apple_button(button) })
        .unwrap_or(StandardGamepadButtonState::RELEASED)
}

#[cfg(not(target_os = "macos"))]
fn descriptor_from_gilrs(gamepad: &Gamepad<'_>) -> InputSourceDescriptor {
    let label = gamepad.name().trim();
    let display_label = (!label.is_empty()).then(|| label.to_owned());
    InputSourceDescriptor::standard_gamepad(
        classify_controller_layout(display_label.as_deref(), gamepad.vendor_id()),
        display_label,
    )
}

#[cfg(not(target_os = "macos"))]
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

#[cfg(not(target_os = "macos"))]
fn gilrs_button(gamepad: &Gamepad<'_>, button: Button) -> StandardGamepadButtonState {
    gamepad
        .button_data(button)
        .map(|state| StandardGamepadButtonState {
            value: state.value(),
            pressed: state.is_pressed(),
        })
        .unwrap_or(StandardGamepadButtonState::RELEASED)
}

#[cfg(not(target_os = "macos"))]
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

#[cfg(all(test, not(target_os = "macos")))]
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

#[cfg(all(test, target_os = "macos"))]
mod apple_tests {
    use super::*;

    #[test]
    fn apple_extended_profile_maps_sticks_triggers_and_buttons_by_semantics() {
        let controller = unsafe { GCController::controllerWithExtendedGamepad() };
        let gamepad = unsafe { controller.extendedGamepad() }
            .expect("snapshot controller has an extended gamepad");

        unsafe {
            gamepad.leftThumbstick().setValueForXAxis_yAxis(0.25, -0.5);
            gamepad.rightThumbstick().setValueForXAxis_yAxis(-0.6, 0.8);
            gamepad.leftTrigger().setValue(0.4);
            gamepad.rightTrigger().setValue(0.8);
            gamepad.buttonA().setValue(1.0);
            gamepad.buttonOptions().unwrap().setValue(1.0);
        }

        let snapshot = unsafe { snapshot_from_apple_gamepad(&gamepad) };
        assert_eq!(snapshot.left_stick, Vec2::new(0.25, -0.5));
        assert_eq!(snapshot.right_stick, Vec2::new(-0.6, 0.8));
        assert_eq!(snapshot.buttons.left_trigger.value, 0.4);
        assert_eq!(snapshot.buttons.right_trigger.value, 0.8);
        assert!(snapshot.buttons.south.pressed);
        assert!(snapshot.buttons.select.pressed);
        assert!(!snapshot.buttons.start.pressed);
    }

    #[test]
    fn apple_observation_queue_is_bounded_and_reports_loss() {
        let mut queue = AppleObservationQueue::default();
        let source_id = InputSourceIdAllocator::new().allocate().unwrap();
        for sequence in 0..=MAX_CONTROLLER_OBSERVATIONS_PER_BATCH {
            queue.push(
                source_id,
                Duration::from_millis(sequence as u64),
                StandardGamepadSnapshot::default(),
            );
        }
        assert_eq!(
            queue.observations.len(),
            MAX_CONTROLLER_OBSERVATIONS_PER_BATCH
        );
        assert_eq!(queue.dropped_observations, 1);
        assert_eq!(queue.observations.front().unwrap().sequence, 1);
    }
}
