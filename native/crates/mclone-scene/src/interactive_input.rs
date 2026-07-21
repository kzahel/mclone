use anyhow::Result;
use mclone_input::{
    FlatInputAction, FlatInputFrame, KeyboardKey, KeyboardMouseInputAdapter, MouseWheelDirection,
    PointerButton, TouchLookDelta,
};
use mclone_ui::{GameHelpParent, GameUiAction, GuiKey, Point};

use crate::{
    HostEffects, McloneSceneHost, MonoInputFrameOutcome, MonoUiActionOutcome, MonoWorldActionStatus,
};

/// Mechanical result returned to an interactive platform adapter.
///
/// The adapter may use these facts to request a redraw or clear platform-local
/// gesture state. Game and UI action identities stay inside the shared router.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MonoInputDisposition {
    pub handled: bool,
    pub scene_changed: bool,
    pub clear_transient_input: bool,
    pub request_pointer_capture_when_ready: bool,
}

impl MonoInputDisposition {
    fn from_ui(handled: bool, outcome: MonoUiActionOutcome) -> Self {
        Self {
            handled,
            scene_changed: handled || outcome.scene_replaced,
            clear_transient_input: outcome.clear_gameplay_input,
            request_pointer_capture_when_ready: outcome.session_start_requested,
        }
    }

    fn merge(&mut self, other: Self) {
        self.handled |= other.handled;
        self.scene_changed |= other.scene_changed;
        self.clear_transient_input |= other.clear_transient_input;
        self.request_pointer_capture_when_ready |= other.request_pointer_capture_when_ready;
    }
}

/// Shared synchronous binding/context/action route for conventional mono hosts.
///
/// Winit, browser, and other leaf adapters normalize their physical APIs into
/// these host-neutral values. The router owns keyboard/mouse held state and
/// asks `McloneSceneHost` to select UI, camera, hotbar, or world behavior.
#[derive(Clone, Debug, Default)]
pub struct MonoInteractiveInputRouter {
    keyboard_mouse: KeyboardMouseInputAdapter,
}

impl MonoInteractiveInputRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_transient_input(&mut self) {
        self.keyboard_mouse.clear_held();
    }

    pub fn held_frame(&self) -> Option<FlatInputFrame> {
        self.keyboard_mouse.held_frame()
    }

    pub fn has_continuous_movement_input(&self) -> bool {
        self.keyboard_mouse.has_continuous_movement_input()
    }

    pub fn advance_held_frame(
        &self,
        host: &mut McloneSceneHost,
        supplemental: Option<FlatInputFrame>,
        dt_seconds: f64,
    ) -> Result<MonoInputFrameOutcome> {
        let mut frame = self.held_frame().unwrap_or_default();
        if let Some(supplemental) = supplemental {
            frame.merge_from(supplemental);
        }
        host.advance_mono_input_frame(frame, dt_seconds)
    }

    pub fn route_key<H>(
        &mut self,
        host: &mut McloneSceneHost,
        key: KeyboardKey,
        pressed: bool,
        repeat: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effects: &mut H,
    ) -> Result<MonoInputDisposition>
    where
        H: HostEffects,
    {
        if host.mono_ui_is_active() {
            if !pressed {
                let event = self.keyboard_mouse.handle_key(key, false, repeat);
                return Ok(MonoInputDisposition {
                    handled: event.handled,
                    ..MonoInputDisposition::default()
                });
            }
            if repeat {
                return Ok(MonoInputDisposition {
                    handled: true,
                    ..MonoInputDisposition::default()
                });
            }
            let Some(gui_key) = gui_key_from_keyboard_key(key) else {
                return Ok(MonoInputDisposition {
                    handled: true,
                    ..MonoInputDisposition::default()
                });
            };
            let (handled, action) = host.mono_ui_key_pressed(gui_key);
            let mut disposition = MonoInputDisposition {
                handled,
                scene_changed: handled,
                ..MonoInputDisposition::default()
            };
            if let Some(action) = action {
                disposition.merge(Self::apply_ui_action(
                    host, action, false, device, queue, effects,
                )?);
            }
            self.clear_if_requested(disposition);
            return Ok(disposition);
        }

        let event = self.keyboard_mouse.handle_key(key, pressed, repeat);
        let mut disposition = MonoInputDisposition {
            handled: event.handled,
            ..MonoInputDisposition::default()
        };
        if let Some(frame) = event.frame {
            disposition.merge(Self::route_resolved_flat_frame(
                host, frame, device, queue, effects,
            )?);
        }
        self.clear_if_requested(disposition);
        Ok(disposition)
    }

    pub fn route_pointer_button<H>(
        &mut self,
        host: &mut McloneSceneHost,
        button: PointerButton,
        pressed: bool,
        ui_point: Option<Point>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effects: &mut H,
    ) -> Result<MonoInputDisposition>
    where
        H: HostEffects,
    {
        if host.mono_ui_is_active() {
            if button != PointerButton::Primary {
                return Ok(MonoInputDisposition {
                    handled: true,
                    ..MonoInputDisposition::default()
                });
            }
            let Some(point) = ui_point else {
                return Ok(MonoInputDisposition {
                    handled: true,
                    ..MonoInputDisposition::default()
                });
            };
            if pressed {
                let handled = host.mono_ui_pointer_down(point);
                return Ok(MonoInputDisposition {
                    handled,
                    scene_changed: handled,
                    ..MonoInputDisposition::default()
                });
            }
            let (handled, action) = host.mono_ui_pointer_up(point);
            let mut disposition = MonoInputDisposition {
                handled,
                scene_changed: handled,
                ..MonoInputDisposition::default()
            };
            if let Some(action) = action {
                disposition.merge(Self::apply_ui_action(
                    host, action, true, device, queue, effects,
                )?);
            }
            self.clear_if_requested(disposition);
            return Ok(disposition);
        }

        let event = self.keyboard_mouse.handle_mouse_button(button, pressed);
        let mut disposition = MonoInputDisposition {
            handled: event.handled,
            ..MonoInputDisposition::default()
        };
        if let Some(frame) = event.frame {
            disposition.merge(Self::route_resolved_flat_frame(
                host, frame, device, queue, effects,
            )?);
        }
        self.clear_if_requested(disposition);
        Ok(disposition)
    }

    pub fn route_pointer_move<H>(
        &mut self,
        host: &mut McloneSceneHost,
        point: Point,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effects: &mut H,
    ) -> Result<MonoInputDisposition>
    where
        H: HostEffects,
    {
        if !host.mono_ui_is_active() {
            return Ok(MonoInputDisposition::default());
        }
        let (handled, action) = host.mono_ui_pointer_move(point);
        let mut disposition = MonoInputDisposition {
            handled,
            scene_changed: handled,
            ..MonoInputDisposition::default()
        };
        if let Some(action) = action {
            disposition.merge(Self::apply_ui_action(
                host, action, true, device, queue, effects,
            )?);
        }
        self.clear_if_requested(disposition);
        Ok(disposition)
    }

    pub fn route_mouse_motion<H>(
        &mut self,
        host: &mut McloneSceneHost,
        delta_x: f32,
        delta_y: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effects: &mut H,
    ) -> Result<MonoInputDisposition>
    where
        H: HostEffects,
    {
        if host.mono_ui_is_active() {
            return Ok(MonoInputDisposition::default());
        }
        let Some(frame) = self.keyboard_mouse.mouse_motion_frame(delta_x, delta_y) else {
            return Ok(MonoInputDisposition::default());
        };
        Self::route_resolved_flat_frame(host, frame, device, queue, effects)
    }

    pub fn route_wheel<H>(
        &mut self,
        host: &mut McloneSceneHost,
        direction: MouseWheelDirection,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effects: &mut H,
    ) -> Result<MonoInputDisposition>
    where
        H: HostEffects,
    {
        if host.mono_ui_is_active() {
            return Ok(MonoInputDisposition {
                handled: true,
                ..MonoInputDisposition::default()
            });
        }
        let event = self.keyboard_mouse.handle_mouse_wheel(direction);
        let mut disposition = MonoInputDisposition {
            handled: event.handled,
            ..MonoInputDisposition::default()
        };
        if let Some(frame) = event.frame {
            disposition.merge(Self::route_resolved_flat_frame(
                host, frame, device, queue, effects,
            )?);
        }
        Ok(disposition)
    }

    pub fn route_touch_look(
        &mut self,
        host: &mut McloneSceneHost,
        delta: TouchLookDelta,
    ) -> MonoInputDisposition {
        let changed = host.apply_mono_touch_look(delta);
        MonoInputDisposition {
            handled: changed,
            scene_changed: changed,
            ..MonoInputDisposition::default()
        }
    }

    pub fn route_flat_frame<H>(
        &mut self,
        host: &mut McloneSceneHost,
        frame: FlatInputFrame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effects: &mut H,
    ) -> Result<MonoInputDisposition>
    where
        H: HostEffects,
    {
        let disposition = Self::route_resolved_flat_frame(host, frame, device, queue, effects)?;
        self.clear_if_requested(disposition);
        Ok(disposition)
    }

    fn route_resolved_flat_frame<H>(
        host: &mut McloneSceneHost,
        frame: FlatInputFrame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effects: &mut H,
    ) -> Result<MonoInputDisposition>
    where
        H: HostEffects,
    {
        let mut disposition = MonoInputDisposition {
            handled: frame != FlatInputFrame::default(),
            ..MonoInputDisposition::default()
        };
        if frame.open_menu {
            host.open_mono_pause_menu();
            disposition.scene_changed = true;
            disposition.clear_transient_input = true;
            return Ok(disposition);
        }
        if frame.open_block_palette {
            host.open_mono_block_palette();
            disposition.scene_changed = true;
            disposition.clear_transient_input = true;
            return Ok(disposition);
        }
        if frame.open_help {
            disposition.merge(Self::apply_ui_action(
                host,
                GameUiAction::OpenHelp(GameHelpParent::Game),
                false,
                device,
                queue,
                effects,
            )?);
            return Ok(disposition);
        }
        if frame.toggle_camera_view {
            let view = host.toggle_mono_camera_view();
            log::info!("camera view mode {}", view.label());
            disposition.scene_changed = true;
        }
        if let Some(slot) = frame.selected_hotbar_slot {
            disposition.scene_changed |= host.select_mono_hotbar_slot(slot);
        }
        if frame.hotbar_step != 0 {
            disposition.scene_changed |= host.step_mono_hotbar_slot(frame.hotbar_step);
        }
        for (pressed, action) in [
            (frame.attack, FlatInputAction::Attack),
            (frame.use_item, FlatInputAction::Use),
        ] {
            if !pressed {
                continue;
            }
            match host.handle_mono_world_action(action)? {
                MonoWorldActionStatus::Submitted { target } => {
                    log::info!(
                        "gameplay interaction {:?} submitted at ({}, {}, {}) face={:?}",
                        action,
                        target.hit.block_pos.x,
                        target.hit.block_pos.y,
                        target.hit.block_pos.z,
                        target.hit.direction
                    );
                    disposition.scene_changed = true;
                }
                MonoWorldActionStatus::EmbeddedWorldActivationRequested => {
                    log::info!("embedded-world activation requested through {action:?}");
                    disposition.scene_changed = true;
                }
                MonoWorldActionStatus::DeniedByWorldBehavior => {
                    log::info!("gameplay interaction denied by active world behavior");
                }
                MonoWorldActionStatus::NoRuntime
                | MonoWorldActionStatus::NoTarget
                | MonoWorldActionStatus::NoCommand => {}
            }
        }
        disposition.scene_changed |= host.apply_mono_look_frame(frame);
        Ok(disposition)
    }

    fn apply_ui_action<H>(
        host: &mut McloneSceneHost,
        action: GameUiAction,
        from_pointer_click: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effects: &mut H,
    ) -> Result<MonoInputDisposition>
    where
        H: HostEffects,
    {
        let outcome =
            host.apply_mono_ui_action(action, from_pointer_click, device, queue, effects)?;
        Ok(MonoInputDisposition::from_ui(true, outcome))
    }

    fn clear_if_requested(&mut self, disposition: MonoInputDisposition) {
        if disposition.clear_transient_input {
            self.clear_transient_input();
        }
    }
}

fn gui_key_from_keyboard_key(key: KeyboardKey) -> Option<GuiKey> {
    match key {
        KeyboardKey::Escape => Some(GuiKey::Escape),
        KeyboardKey::F1 => Some(GuiKey::F1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_keys_are_derived_from_neutral_keyboard_keys() {
        assert_eq!(
            gui_key_from_keyboard_key(KeyboardKey::Escape),
            Some(GuiKey::Escape)
        );
        assert_eq!(gui_key_from_keyboard_key(KeyboardKey::F1), Some(GuiKey::F1));
        assert_eq!(gui_key_from_keyboard_key(KeyboardKey::Space), None);
    }

    #[test]
    fn router_owns_shared_held_keyboard_state() {
        let mut router = MonoInteractiveInputRouter::new();
        let event = router
            .keyboard_mouse
            .handle_key(KeyboardKey::KeyW, true, false);
        assert!(event.handled);
        assert!(event.frame.is_none());
        assert!(router.has_continuous_movement_input());
        assert!(router.held_frame().is_some_and(|frame| frame.forward));

        router.clear_transient_input();
        assert!(!router.has_continuous_movement_input());
        assert!(router.held_frame().is_none());
    }
}
