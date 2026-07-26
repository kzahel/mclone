use std::time::Instant;

use mclone_view_control::{
    ContactEvent, ContactGestureReducer, ContactPurpose, ViewPoint, ViewportMetrics,
    WorldViewHeldDirection, WorldViewIntent, WorldViewMode, WorldViewProjection, WorldViewState,
};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, Touch, TouchPhase};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

const MOUSE_CONTACT_ID: u64 = u64::MAX;

pub struct NativeViewInput {
    contacts: ContactGestureReducer,
    cursor: ViewPoint,
    active_mouse_button: Option<MouseButton>,
    modifiers: ModifiersState,
    started: Instant,
}

impl NativeViewInput {
    pub fn new(started: Instant) -> Self {
        Self {
            contacts: ContactGestureReducer::default(),
            cursor: ViewPoint::default(),
            active_mouse_button: None,
            modifiers: ModifiersState::empty(),
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

    pub fn held_motion(
        &self,
        event: &winit::event::KeyEvent,
    ) -> Option<(WorldViewHeldDirection, bool)> {
        let PhysicalKey::Code(code) = event.physical_key else {
            return None;
        };
        held_direction(code).map(|direction| (direction, event.state == ElementState::Pressed))
    }

    pub fn cancel(&mut self, state: WorldViewState) -> Vec<WorldViewIntent> {
        self.active_mouse_button = None;
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

fn held_direction(code: KeyCode) -> Option<WorldViewHeldDirection> {
    match code {
        KeyCode::ArrowUp | KeyCode::KeyW => Some(WorldViewHeldDirection::Forward),
        KeyCode::ArrowDown | KeyCode::KeyS => Some(WorldViewHeldDirection::Backward),
        KeyCode::ArrowLeft | KeyCode::KeyA => Some(WorldViewHeldDirection::Left),
        KeyCode::ArrowRight | KeyCode::KeyD => Some(WorldViewHeldDirection::Right),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_arrows_and_wasd_map_to_the_same_held_directions() {
        for (arrow, letter, expected) in [
            (
                KeyCode::ArrowUp,
                KeyCode::KeyW,
                WorldViewHeldDirection::Forward,
            ),
            (
                KeyCode::ArrowDown,
                KeyCode::KeyS,
                WorldViewHeldDirection::Backward,
            ),
            (
                KeyCode::ArrowLeft,
                KeyCode::KeyA,
                WorldViewHeldDirection::Left,
            ),
            (
                KeyCode::ArrowRight,
                KeyCode::KeyD,
                WorldViewHeldDirection::Right,
            ),
        ] {
            assert_eq!(held_direction(arrow), Some(expected));
            assert_eq!(held_direction(letter), Some(expected));
        }
        assert_eq!(held_direction(KeyCode::KeyM), None);
    }
}
