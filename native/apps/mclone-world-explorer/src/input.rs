use std::collections::BTreeSet;
use std::time::Instant;

use mclone_view_control::{
    ContactEvent, ContactGestureReducer, ContactPurpose, ViewPoint, ViewportMetrics,
    WorldViewIntent, WorldViewMode, WorldViewProjection, WorldViewState,
};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, Touch, TouchPhase};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

const MOUSE_CONTACT_ID: u64 = u64::MAX;
const PAN_FOOTPRINTS_PER_SECOND: f64 = 0.4;
const MAX_CONTINUOUS_STEP_SECONDS: f64 = 0.1;

pub struct NativeViewInput {
    contacts: ContactGestureReducer,
    cursor: ViewPoint,
    active_mouse_button: Option<MouseButton>,
    modifiers: ModifiersState,
    held_movement_keys: BTreeSet<KeyCode>,
    last_continuous_update: Instant,
    started: Instant,
}

impl NativeViewInput {
    pub fn new(started: Instant) -> Self {
        Self {
            contacts: ContactGestureReducer::default(),
            cursor: ViewPoint::default(),
            active_mouse_button: None,
            modifiers: ModifiersState::empty(),
            held_movement_keys: BTreeSet::new(),
            last_continuous_update: Instant::now(),
            started,
        }
    }

    pub fn set_modifiers(&mut self, modifiers: ModifiersState) {
        self.modifiers = modifiers;
    }

    pub fn cursor_moved(
        &mut self,
        x: f64,
        y: f64,
        state: WorldViewState,
        viewport: ViewportMetrics,
    ) -> Vec<WorldViewIntent> {
        self.cursor = ViewPoint::new(x, y);
        if self.active_mouse_button.is_none() {
            return Vec::new();
        }
        self.contacts.handle(
            state,
            ContactEvent::Moved {
                id: MOUSE_CONTACT_ID,
                position: self.cursor,
                time_seconds: self.elapsed_seconds(),
                viewport,
            },
        )
    }

    pub fn mouse_button(
        &mut self,
        button: MouseButton,
        element_state: ElementState,
        state: WorldViewState,
        viewport: ViewportMetrics,
    ) -> Vec<WorldViewIntent> {
        match element_state {
            ElementState::Pressed if mouse_purpose(button, self.modifiers).is_some() => {
                if self.active_mouse_button.is_some() {
                    self.contacts.handle(state, ContactEvent::CancelAll);
                }
                self.active_mouse_button = Some(button);
                self.contacts.handle(
                    state,
                    ContactEvent::Down {
                        id: MOUSE_CONTACT_ID,
                        position: self.cursor,
                        purpose: mouse_purpose(button, self.modifiers)
                            .expect("accepted mouse button has a purpose"),
                        time_seconds: self.elapsed_seconds(),
                        viewport,
                    },
                )
            }
            ElementState::Released if self.active_mouse_button == Some(button) => {
                self.active_mouse_button = None;
                self.contacts.handle(
                    state,
                    ContactEvent::Up {
                        id: MOUSE_CONTACT_ID,
                        position: self.cursor,
                        time_seconds: self.elapsed_seconds(),
                        viewport,
                    },
                )
            }
            _ => Vec::new(),
        }
    }

    pub fn mouse_wheel(
        &self,
        delta: MouseScrollDelta,
        state: WorldViewState,
        viewport: ViewportMetrics,
    ) -> WorldViewIntent {
        let log_delta = match delta {
            MouseScrollDelta::LineDelta(_, y) => -f64::from(y) * 0.15,
            MouseScrollDelta::PixelDelta(position) => -position.y * 0.0015,
        };
        WorldViewIntent::AnchoredZoom {
            log_delta,
            normalized_anchor: if state.mode == WorldViewMode::Map {
                viewport.normalized_anchor(self.cursor)
            } else {
                ViewPoint::default()
            },
            viewport,
        }
    }

    pub fn touch(
        &mut self,
        touch: Touch,
        state: WorldViewState,
        viewport: ViewportMetrics,
    ) -> Vec<WorldViewIntent> {
        let position = ViewPoint::new(touch.location.x, touch.location.y);
        let time_seconds = self.elapsed_seconds();
        let event = match touch.phase {
            TouchPhase::Started => ContactEvent::Down {
                id: touch.id,
                position,
                purpose: ContactPurpose::ViewDefault,
                time_seconds,
                viewport,
            },
            TouchPhase::Moved => ContactEvent::Moved {
                id: touch.id,
                position,
                time_seconds,
                viewport,
            },
            TouchPhase::Ended => ContactEvent::Up {
                id: touch.id,
                position,
                time_seconds,
                viewport,
            },
            TouchPhase::Cancelled => ContactEvent::CancelAll,
        };
        self.contacts.handle(state, event)
    }

    pub fn keyboard(
        &mut self,
        event: &winit::event::KeyEvent,
        state: WorldViewState,
        viewport: ViewportMetrics,
    ) -> Vec<WorldViewIntent> {
        let PhysicalKey::Code(code) = event.physical_key else {
            return Vec::new();
        };
        if is_movement_key(code) {
            match event.state {
                ElementState::Pressed => {
                    self.held_movement_keys.insert(code);
                }
                ElementState::Released => {
                    self.held_movement_keys.remove(&code);
                }
            }
        }
        if event.state != ElementState::Pressed || event.repeat {
            return Vec::new();
        }
        match code {
            KeyCode::KeyM => vec![WorldViewIntent::SetMode(match state.mode {
                WorldViewMode::Map => WorldViewMode::Orbit,
                WorldViewMode::Orbit => WorldViewMode::Map,
            })],
            KeyCode::KeyP => {
                vec![WorldViewIntent::SetProjection(match state.projection {
                    WorldViewProjection::Orthographic => WorldViewProjection::Perspective,
                    WorldViewProjection::Perspective => WorldViewProjection::Orthographic,
                })]
            }
            KeyCode::Home => vec![WorldViewIntent::Recenter {
                world_x: 0.0,
                world_z: 0.0,
                blocks_across: None,
            }],
            KeyCode::Equal | KeyCode::NumpadAdd => vec![WorldViewIntent::AnchoredZoom {
                log_delta: 0.5_f64.ln(),
                normalized_anchor: ViewPoint::default(),
                viewport,
            }],
            KeyCode::Minus | KeyCode::NumpadSubtract => {
                vec![WorldViewIntent::AnchoredZoom {
                    log_delta: 2.0_f64.ln(),
                    normalized_anchor: ViewPoint::default(),
                    viewport,
                }]
            }
            _ => Vec::new(),
        }
    }

    pub fn continuous_intent(
        &mut self,
        now: Instant,
        state: WorldViewState,
    ) -> Option<WorldViewIntent> {
        let delta_seconds = now
            .saturating_duration_since(self.last_continuous_update)
            .as_secs_f64()
            .min(MAX_CONTINUOUS_STEP_SECONDS);
        self.last_continuous_update = now;
        let horizontal = axis(
            &self.held_movement_keys,
            &[KeyCode::ArrowRight, KeyCode::KeyD],
            &[KeyCode::ArrowLeft, KeyCode::KeyA],
        );
        let vertical = axis(
            &self.held_movement_keys,
            &[KeyCode::ArrowDown, KeyCode::KeyS],
            &[KeyCode::ArrowUp, KeyCode::KeyW],
        );
        if horizontal == 0.0 && vertical == 0.0 {
            return None;
        }
        let length = horizontal.hypot(vertical).max(1.0);
        let distance = state.blocks_across * PAN_FOOTPRINTS_PER_SECOND * delta_seconds;
        Some(WorldViewIntent::PanWorld {
            delta_x: horizontal / length * distance,
            delta_z: vertical / length * distance,
        })
    }

    pub fn has_continuous_input(&self) -> bool {
        !self.held_movement_keys.is_empty()
    }

    pub fn cancel(&mut self, state: WorldViewState) -> Vec<WorldViewIntent> {
        self.active_mouse_button = None;
        self.held_movement_keys.clear();
        self.contacts.handle(state, ContactEvent::CancelAll)
    }

    fn elapsed_seconds(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }
}

fn mouse_purpose(button: MouseButton, modifiers: ModifiersState) -> Option<ContactPurpose> {
    match button {
        MouseButton::Left if modifiers.shift_key() => Some(ContactPurpose::Pan),
        MouseButton::Left => Some(ContactPurpose::ViewDefault),
        MouseButton::Middle | MouseButton::Right => Some(ContactPurpose::Pan),
        _ => None,
    }
}

fn is_movement_key(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::ArrowUp
            | KeyCode::ArrowDown
            | KeyCode::ArrowLeft
            | KeyCode::ArrowRight
            | KeyCode::KeyW
            | KeyCode::KeyA
            | KeyCode::KeyS
            | KeyCode::KeyD
    )
}

fn axis(keys: &BTreeSet<KeyCode>, positive: &[KeyCode], negative: &[KeyCode]) -> f64 {
    let positive = positive.iter().any(|key| keys.contains(key)) as u8;
    let negative = negative.iter().any(|key| keys.contains(key)) as u8;
    f64::from(positive) - f64::from(negative)
}
