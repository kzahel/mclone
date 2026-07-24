use anyhow::Result;
use mclone_input::{
    AgedPlayerActionFrame, ControllerActionBatch, ControllerInputBatch, ControllerInputError,
    ControllerInputObservation, ControllerInputPreferences, ControllerInputSession,
    ControllerLayoutFamily, FlatInputAction, FlatInputFrame, InputContext, InputSourceDescriptor,
    InputSourceId, KEYBOARD_TURN_MOUSE_DELTA_PER_SECOND, KeyboardKey, KeyboardMouseInputAdapter,
    MouseWheelDirection, PlayerAction, PlayerActionFrame, PlayerActionFrameCombiner, PointerButton,
    StandardGamepadSnapshot, TouchLookDelta, XrInputFrame,
};
use mclone_ui::{
    GameHelpParent, GameLocalPlayControllerFamily, GameLocalPlayGuestInput, GameUiAction, GuiKey,
    GuiNavigation, Point,
};
use std::{collections::BTreeMap, time::Duration};

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
    pub meaningful_controller_activity: bool,
}

/// Mechanical result of routing an ordinary gamepad beside tracked XR input.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XrControllerInputDisposition {
    pub scene_changed: bool,
    pub meaningful_controller_activity: bool,
}

/// Shared ordinary-gamepad state for XR hosts.
///
/// Platform collectors supply canonical snapshots. This owner selects the
/// shared gameplay/menu context, retains continuous semantic actions for the
/// next XR scene frame, and sends focused world-panel navigation through the
/// same UI model as flat hosts. It never manufactures a tracked pose.
#[derive(Clone, Debug, Default)]
pub struct XrControllerInputRouter {
    controller: ControllerInputSession,
    combiner: PlayerActionFrameCombiner,
    latest_actions: PlayerActionFrame,
    latest_action_observations: Vec<AgedPlayerActionFrame>,
}

impl XrControllerInputRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_controller_preferences(preferences: &ControllerInputPreferences) -> Self {
        Self {
            controller: ControllerInputSession::with_preferences(preferences),
            ..Self::default()
        }
    }

    pub fn apply_controller_preferences(&mut self, preferences: &ControllerInputPreferences) {
        self.controller.apply_preferences(preferences);
        self.combiner.clear();
        self.latest_actions = PlayerActionFrame::default();
        self.latest_action_observations.clear();
    }

    pub fn clear_transient_input(&mut self) {
        self.controller.clear_held();
        self.combiner.clear();
        self.latest_actions = PlayerActionFrame::default();
        self.latest_action_observations.clear();
    }

    pub fn connect_source(
        &mut self,
        source_id: InputSourceId,
        descriptor: InputSourceDescriptor,
    ) -> bool {
        self.controller.connect_source(source_id, descriptor)
    }

    pub fn disconnect_source(
        &mut self,
        source_id: InputSourceId,
    ) -> std::result::Result<bool, ControllerInputError> {
        let changed = self.controller.disconnect_source(source_id)?;
        if self.latest_actions.active_source == Some(source_id) {
            self.latest_actions = PlayerActionFrame::default();
        }
        Ok(changed)
    }

    pub fn source_count(&self) -> usize {
        self.controller.source_count()
    }

    pub fn latest_actions(&self) -> &PlayerActionFrame {
        &self.latest_actions
    }

    /// Compose ordinary and tracked-controller semantics without allowing one
    /// source's release edge to cancel another source's continuing hold.
    pub fn merge_into_frame(&mut self, frame: &mut XrInputFrame) {
        let primary_actions = frame.actions.clone();
        frame
            .action_observations
            .extend(
                self.latest_action_observations
                    .iter()
                    .cloned()
                    .map(|mut observation| {
                        let mut actions = primary_actions.clone();
                        actions.merge_from(observation.actions);
                        observation.actions = actions;
                        observation
                    }),
            );
        frame.actions = self.combiner.combine(
            std::mem::take(&mut frame.actions),
            self.latest_actions.clone(),
        );
    }

    pub fn route_samples(
        &mut self,
        host: &mut McloneSceneHost,
        now: Duration,
        samples: impl IntoIterator<Item = (InputSourceId, StandardGamepadSnapshot)>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<XrControllerInputDisposition> {
        self.route_batch(
            host,
            &ControllerInputBatch::from_snapshots(now, samples),
            device,
            queue,
        )
    }

    pub fn route_batch(
        &mut self,
        host: &mut McloneSceneHost,
        batch: &ControllerInputBatch,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<XrControllerInputDisposition> {
        let context = if host.mono_ui_is_active() {
            InputContext::Menu
        } else {
            InputContext::Gameplay
        };
        let action_batch = self.sample_batch(context, batch)?;
        let actions = &action_batch.actions;
        let mut disposition = XrControllerInputDisposition {
            meaningful_controller_activity: actions.activity_source.is_some(),
            ..XrControllerInputDisposition::default()
        };
        if context != InputContext::Gameplay {
            for observation in &action_batch.observations {
                for action in observation.actions.pressed.iter().copied() {
                    let Some(navigation) = gui_navigation_from_player_action(action) else {
                        continue;
                    };
                    let (handled, ui_action) = host.mono_ui_navigate(navigation);
                    disposition.scene_changed |= handled;
                    if let Some(ui_action) = ui_action {
                        disposition.scene_changed |=
                            host.apply_xr_ui_action(ui_action, device, queue)?;
                    }
                }
            }
        }
        self.latest_actions = action_batch.actions;
        Ok(disposition)
    }

    #[cfg(test)]
    fn sample_actions(
        &mut self,
        context: InputContext,
        now: Duration,
        samples: impl IntoIterator<Item = (InputSourceId, StandardGamepadSnapshot)>,
    ) -> std::result::Result<PlayerActionFrame, ControllerInputError> {
        Ok(self
            .sample_batch(context, &ControllerInputBatch::from_snapshots(now, samples))?
            .actions)
    }

    fn sample_batch(
        &mut self,
        context: InputContext,
        batch: &ControllerInputBatch,
    ) -> std::result::Result<ControllerActionBatch, ControllerInputError> {
        self.controller.set_context(context);
        let actions = self.controller.sample_batch(batch)?;
        self.latest_actions = actions.actions.clone();
        self.latest_action_observations = actions
            .observations
            .iter()
            .map(|observation| AgedPlayerActionFrame {
                age: batch.sample_time().saturating_sub(observation.sample_time),
                actions: observation.actions.clone(),
            })
            .collect();
        if self.latest_action_observations.is_empty() {
            self.latest_action_observations.push(AgedPlayerActionFrame {
                age: Duration::ZERO,
                actions: actions.actions.clone(),
            });
        }
        Ok(actions)
    }
}

impl MonoInputDisposition {
    fn from_ui(handled: bool, outcome: MonoUiActionOutcome) -> Self {
        Self {
            handled,
            scene_changed: handled || outcome.scene_replaced,
            clear_transient_input: outcome.clear_gameplay_input,
            request_pointer_capture_when_ready: outcome.session_start_requested,
            meaningful_controller_activity: false,
        }
    }

    fn merge(&mut self, other: Self) {
        self.handled |= other.handled;
        self.scene_changed |= other.scene_changed;
        self.clear_transient_input |= other.clear_transient_input;
        self.request_pointer_capture_when_ready |= other.request_pointer_capture_when_ready;
        self.meaningful_controller_activity |= other.meaningful_controller_activity;
    }
}

/// Shared synchronous binding/context/action route for conventional mono hosts.
///
/// Winit, browser, and other leaf adapters normalize their physical APIs into
/// these host-neutral values. The router owns keyboard/mouse held state and
/// asks `McloneSceneHost` to select UI, camera, hotbar, or world behavior.
#[derive(Clone, Copy, Debug)]
struct LocalGuestControllerAssignment {
    source_id: InputSourceId,
    family: GameLocalPlayControllerFamily,
    device_number: u8,
    connected: bool,
}

#[derive(Clone, Debug, Default)]
pub struct MonoInteractiveInputRouter {
    keyboard_mouse: KeyboardMouseInputAdapter,
    controller: ControllerInputSession,
    latest_controller_actions: PlayerActionFrame,
    controller_descriptors: BTreeMap<InputSourceId, InputSourceDescriptor>,
    controller_numbers: BTreeMap<InputSourceId, u8>,
    local_guest_controller: ControllerInputSession,
    local_guest_assignment: Option<LocalGuestControllerAssignment>,
    latest_local_guest_actions: PlayerActionFrame,
    local_guest_lifecycle_dirty: bool,
}

impl MonoInteractiveInputRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_controller_preferences(preferences: &ControllerInputPreferences) -> Self {
        Self {
            controller: ControllerInputSession::with_preferences(preferences),
            local_guest_controller: ControllerInputSession::with_preferences(preferences),
            ..Self::default()
        }
    }

    pub fn apply_controller_preferences(&mut self, preferences: &ControllerInputPreferences) {
        self.controller.apply_preferences(preferences);
        self.local_guest_controller.apply_preferences(preferences);
        self.latest_controller_actions = PlayerActionFrame::default();
        self.latest_local_guest_actions = PlayerActionFrame::default();
    }

    pub fn clear_transient_input(&mut self) {
        self.keyboard_mouse.clear_held();
        self.controller.clear_held();
        self.local_guest_controller.clear_held();
        self.latest_controller_actions = PlayerActionFrame::default();
        self.latest_local_guest_actions = PlayerActionFrame::default();
    }

    pub fn held_frame(&self) -> Option<FlatInputFrame> {
        let mut frame = self.keyboard_mouse.held_frame().unwrap_or_default();
        frame.merge_from(self.latest_controller_actions.to_flat_frame(0.0));
        (frame != FlatInputFrame::default()).then_some(frame)
    }

    pub fn has_continuous_movement_input(&self) -> bool {
        self.keyboard_mouse.has_continuous_movement_input()
            || self.latest_controller_actions.movement != Default::default()
            || self.latest_local_guest_actions.movement != Default::default()
            || self.latest_controller_actions.held.iter().any(|action| {
                matches!(
                    action,
                    mclone_input::PlayerAction::Jump
                        | mclone_input::PlayerAction::Sprint
                        | mclone_input::PlayerAction::Sneak
                        | mclone_input::PlayerAction::Descend
                )
            })
            || self.latest_local_guest_actions.held.iter().any(|action| {
                matches!(
                    action,
                    PlayerAction::Jump
                        | PlayerAction::Sprint
                        | PlayerAction::Sneak
                        | PlayerAction::Descend
                )
            })
    }

    pub fn connect_controller_source(
        &mut self,
        source_id: InputSourceId,
        descriptor: InputSourceDescriptor,
    ) -> bool {
        self.controller_descriptors
            .insert(source_id, descriptor.clone());
        if !self.controller_numbers.contains_key(&source_id) {
            let next = self
                .controller_numbers
                .values()
                .copied()
                .max()
                .unwrap_or(0)
                .saturating_add(1)
                .max(1);
            self.controller_numbers.insert(source_id, next);
        }
        if self
            .local_guest_assignment
            .is_some_and(|assignment| assignment.source_id == source_id)
        {
            let changed = self
                .local_guest_controller
                .connect_source(source_id, descriptor);
            if let Some(assignment) = self.local_guest_assignment.as_mut() {
                assignment.connected = true;
            }
            self.local_guest_lifecycle_dirty = true;
            self.local_guest_controller.clear_held();
            changed
        } else {
            self.controller.connect_source(source_id, descriptor)
        }
    }

    pub fn disconnect_controller_source(
        &mut self,
        source_id: InputSourceId,
    ) -> std::result::Result<bool, ControllerInputError> {
        let changed = if self
            .local_guest_assignment
            .is_some_and(|assignment| assignment.source_id == source_id)
        {
            let changed = self.local_guest_controller.disconnect_source(source_id)?;
            if let Some(assignment) = self.local_guest_assignment.as_mut() {
                assignment.connected = false;
            }
            self.latest_local_guest_actions = PlayerActionFrame::default();
            self.local_guest_lifecycle_dirty = true;
            changed
        } else {
            self.controller.disconnect_source(source_id)?
        };
        if self.latest_controller_actions.active_source == Some(source_id) {
            self.latest_controller_actions = PlayerActionFrame::default();
        }
        self.controller_descriptors.remove(&source_id);
        Ok(changed)
    }

    pub fn controller_source_count(&self) -> usize {
        self.controller_descriptors.len()
    }

    pub fn latest_controller_actions(&self) -> &PlayerActionFrame {
        &self.latest_controller_actions
    }

    pub fn advance_held_frame(
        &self,
        host: &mut McloneSceneHost,
        supplemental: Option<FlatInputFrame>,
        dt_seconds: f64,
    ) -> Result<MonoInputFrameOutcome> {
        let frame = self.composed_held_frame(supplemental, dt_seconds);
        let mut outcome = host.advance_mono_input_frame(frame, dt_seconds)?;
        if host.local_play_guest_active() {
            outcome.camera_changed |= host.advance_local_play_guest_input_frame(
                self.latest_local_guest_actions.to_flat_frame(dt_seconds),
                dt_seconds,
            );
        }
        Ok(outcome)
    }

    pub fn observe_supplemental_movement(
        &self,
        host: &mut McloneSceneHost,
        supplemental: Option<FlatInputFrame>,
    ) {
        host.observe_mono_movement_frame_ago(
            self.composed_held_frame(supplemental, 0.0),
            self.composed_look_rate_mouse_delta_per_second(&self.latest_controller_actions),
            Duration::ZERO,
        );
    }

    fn composed_held_frame(
        &self,
        supplemental: Option<FlatInputFrame>,
        dt_seconds: f64,
    ) -> FlatInputFrame {
        let mut frame = self.keyboard_mouse.held_frame().unwrap_or_default();
        frame.merge_from(self.latest_controller_actions.to_flat_frame(dt_seconds));
        if let Some(supplemental) = supplemental {
            frame.merge_from(supplemental);
        }
        frame
    }

    fn movement_frame_for_controller_actions(&self, actions: &PlayerActionFrame) -> FlatInputFrame {
        let mut frame = self.keyboard_mouse.held_frame().unwrap_or_default();
        frame.merge_from(actions.to_flat_frame(0.0));
        frame
    }

    fn composed_look_rate_mouse_delta_per_second(
        &self,
        controller_actions: &PlayerActionFrame,
    ) -> [f64; 2] {
        let keyboard_turn = self
            .keyboard_mouse
            .held_frame()
            .map(|frame| frame.keyboard_turn)
            .unwrap_or_default()
            .clamp(-1.0, 1.0);
        [
            f64::from(controller_actions.look_rate.x)
                - f64::from(keyboard_turn) * KEYBOARD_TURN_MOUSE_DELTA_PER_SECOND,
            f64::from(controller_actions.look_rate.y),
        ]
    }

    /// Consume ordinary-controller snapshots at one presentation boundary.
    /// The scene owns context selection; collectors supply only source IDs and
    /// normalized physical state.
    #[allow(clippy::too_many_arguments)]
    pub fn route_controller_samples<H>(
        &mut self,
        host: &mut McloneSceneHost,
        now: Duration,
        samples: impl IntoIterator<Item = (InputSourceId, StandardGamepadSnapshot)>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effects: &mut H,
    ) -> Result<MonoInputDisposition>
    where
        H: HostEffects,
    {
        self.route_controller_batch(
            host,
            &ControllerInputBatch::from_snapshots(now, samples),
            device,
            queue,
            effects,
        )
    }

    /// Consume an ordered ordinary-controller batch at one presentation
    /// boundary without collapsing physical transitions before shared action
    /// routing.
    pub fn route_controller_batch<H>(
        &mut self,
        host: &mut McloneSceneHost,
        batch: &ControllerInputBatch,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effects: &mut H,
    ) -> Result<MonoInputDisposition>
    where
        H: HostEffects,
    {
        self.reconcile_local_guest_assignment(host)?;
        if let Some(assignment) = self.local_guest_assignment {
            let primary_batch = controller_batch_without_source(batch, assignment.source_id);
            let local_guest_batch = controller_batch_for_source(batch, assignment.source_id);
            let mut disposition =
                self.route_primary_controller_batch(host, &primary_batch, device, queue, effects)?;
            disposition.merge(self.route_local_guest_controller_batch(
                host,
                &local_guest_batch,
                device,
                queue,
                effects,
            )?);
            self.reconcile_local_guest_assignment(host)?;
            return Ok(disposition);
        }
        let disposition =
            self.route_primary_controller_batch(host, batch, device, queue, effects)?;
        self.reconcile_local_guest_assignment(host)?;
        Ok(disposition)
    }

    fn route_primary_controller_batch<H>(
        &mut self,
        host: &mut McloneSceneHost,
        batch: &ControllerInputBatch,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effects: &mut H,
    ) -> Result<MonoInputDisposition>
    where
        H: HostEffects,
    {
        let context = if host.mono_ui_is_active() {
            InputContext::Menu
        } else {
            InputContext::Gameplay
        };
        let action_batch = self.sample_controller_batch(context, batch)?;
        let meaningful_controller_activity = action_batch.actions.activity_source.is_some();
        if context != InputContext::Gameplay {
            let mut disposition = MonoInputDisposition {
                handled: !action_batch.actions.pressed.is_empty()
                    || !action_batch.actions.released.is_empty(),
                meaningful_controller_activity,
                ..MonoInputDisposition::default()
            };
            'observations: for observation in action_batch.observations {
                if host.local_play_waiting_for_gamepad()
                    && observation
                        .actions
                        .pressed
                        .contains(&PlayerAction::UiConfirm)
                {
                    if let Some(source_id) = observation.source_id {
                        disposition.scene_changed |=
                            self.assign_local_guest_source(host, source_id)?;
                        disposition.handled = true;
                        break 'observations;
                    }
                }
                for action in observation.actions.pressed {
                    let Some(navigation) = gui_navigation_from_player_action(action) else {
                        continue;
                    };
                    let (handled, ui_action) = host.mono_ui_navigate(navigation);
                    disposition.handled |= handled;
                    disposition.scene_changed |= handled;
                    if let Some(ui_action) = ui_action {
                        let capture_source = (ui_action == GameUiAction::ToggleLocalPlayGuest)
                            .then_some(observation.source_id)
                            .flatten();
                        disposition.merge(Self::apply_ui_action(
                            host, ui_action, false, device, queue, effects,
                        )?);
                        if host.local_play_waiting_for_gamepad() {
                            if let Some(source_id) = capture_source {
                                disposition.scene_changed |=
                                    self.assign_local_guest_source(host, source_id)?;
                            }
                        }
                    }
                    if disposition.clear_transient_input {
                        break 'observations;
                    }
                }
            }
            self.clear_if_requested(disposition);
            return Ok(disposition);
        }
        for observation in &action_batch.observations {
            host.observe_mono_movement_frame_ago(
                self.movement_frame_for_controller_actions(&observation.actions),
                self.composed_look_rate_mouse_delta_per_second(&observation.actions),
                batch.sample_time().saturating_sub(observation.sample_time),
            );
        }
        if action_batch.observations.is_empty() {
            host.observe_mono_movement_frame_ago(
                self.movement_frame_for_controller_actions(&action_batch.actions),
                self.composed_look_rate_mouse_delta_per_second(&action_batch.actions),
                Duration::ZERO,
            );
        }
        // Continuous movement/look is applied exactly once by
        // `advance_held_frame`. The immediate route consumes only action edges
        // (and any already-integrated pointer delta from a future semantic
        // source), avoiding presentation-rate look being applied twice.
        let mut disposition = MonoInputDisposition::default();
        for observation in action_batch.observations {
            let mut frame = observation.actions.to_flat_frame(0.0);
            frame.forward = false;
            frame.backward = false;
            frame.left = false;
            frame.right = false;
            frame.movement = Default::default();
            frame.analog_movement = None;
            frame.jump = false;
            frame.sprint = false;
            frame.sneak = false;
            frame.descend = false;
            disposition.merge(Self::route_resolved_flat_frame(
                host, frame, device, queue, effects,
            )?);
            if disposition.clear_transient_input {
                break;
            }
        }
        disposition.meaningful_controller_activity = meaningful_controller_activity;
        self.clear_if_requested(disposition);
        Ok(disposition)
    }

    fn route_local_guest_controller_batch<H>(
        &mut self,
        host: &mut McloneSceneHost,
        batch: &ControllerInputBatch,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effects: &mut H,
    ) -> Result<MonoInputDisposition>
    where
        H: HostEffects,
    {
        let context = if host.mono_ui_is_active() {
            InputContext::Menu
        } else {
            InputContext::Gameplay
        };
        self.local_guest_controller.set_context(context);
        let action_batch = self.local_guest_controller.sample_batch(batch)?;
        self.latest_local_guest_actions = action_batch.actions.clone();
        let mut disposition = MonoInputDisposition {
            meaningful_controller_activity: action_batch.actions.activity_source.is_some(),
            ..MonoInputDisposition::default()
        };
        if context == InputContext::Menu {
            'observations: for observation in action_batch.observations {
                for action in observation.actions.pressed {
                    let Some(navigation) = gui_navigation_from_player_action(action) else {
                        continue;
                    };
                    let (handled, ui_action) = host.mono_ui_navigate(navigation);
                    disposition.handled |= handled;
                    disposition.scene_changed |= handled;
                    if let Some(ui_action) = ui_action {
                        disposition.merge(Self::apply_ui_action(
                            host, ui_action, false, device, queue, effects,
                        )?);
                    }
                    if disposition.clear_transient_input {
                        break 'observations;
                    }
                }
            }
            self.clear_if_requested(disposition);
            return Ok(disposition);
        }

        for observation in &action_batch.observations {
            host.observe_local_play_guest_movement_frame_ago(
                observation.actions.to_flat_frame(0.0),
                [
                    f64::from(observation.actions.look_rate.x),
                    f64::from(observation.actions.look_rate.y),
                ],
                batch.sample_time().saturating_sub(observation.sample_time),
            );
        }
        if action_batch.observations.iter().any(|observation| {
            observation
                .actions
                .pressed
                .contains(&PlayerAction::OpenMenu)
        }) {
            host.open_mono_pause_menu();
            disposition.handled = true;
            disposition.scene_changed = true;
            disposition.clear_transient_input = true;
        }
        self.clear_if_requested(disposition);
        Ok(disposition)
    }

    fn assign_local_guest_source(
        &mut self,
        host: &mut McloneSceneHost,
        source_id: InputSourceId,
    ) -> Result<bool> {
        let Some(descriptor) = self.controller_descriptors.get(&source_id).cloned() else {
            return Ok(false);
        };
        if self.local_guest_assignment.is_some() {
            return Ok(false);
        }
        let _ = self.controller.disconnect_source(source_id);
        self.local_guest_controller
            .connect_source(source_id, descriptor.clone());
        self.local_guest_controller.clear_held();
        self.latest_local_guest_actions = PlayerActionFrame::default();
        let family = local_play_controller_family(descriptor.controller_layout);
        let device_number = self
            .controller_numbers
            .get(&source_id)
            .copied()
            .unwrap_or(1);
        self.local_guest_assignment = Some(LocalGuestControllerAssignment {
            source_id,
            family,
            device_number,
            connected: true,
        });
        log::info!(
            "assigned {} to local Guest 2",
            descriptor
                .display_label
                .as_deref()
                .unwrap_or(family.label())
        );
        Ok(host.assign_local_play_guest_gamepad(family, device_number))
    }

    fn reconcile_local_guest_assignment(&mut self, host: &mut McloneSceneHost) -> Result<()> {
        if host.local_play_state().guest_input == GameLocalPlayGuestInput::Off {
            if let Some(assignment) = self.local_guest_assignment.take() {
                let _ = self
                    .local_guest_controller
                    .disconnect_source(assignment.source_id);
                self.latest_local_guest_actions = PlayerActionFrame::default();
                if assignment.connected {
                    if let Some(descriptor) = self
                        .controller_descriptors
                        .get(&assignment.source_id)
                        .cloned()
                    {
                        self.controller
                            .connect_source(assignment.source_id, descriptor);
                        self.controller.clear_held();
                    }
                }
            }
            self.local_guest_lifecycle_dirty = false;
            return Ok(());
        }
        if self.local_guest_lifecycle_dirty {
            if let Some(assignment) = self.local_guest_assignment {
                if assignment.connected {
                    host.assign_local_play_guest_gamepad(
                        assignment.family,
                        assignment.device_number,
                    );
                } else {
                    host.mark_local_play_guest_gamepad_disconnected();
                }
            }
            self.local_guest_lifecycle_dirty = false;
        }
        if let Some(assignment) = self
            .local_guest_assignment
            .filter(|assignment| assignment.connected)
        {
            if matches!(
                host.local_play_state().guest_input,
                GameLocalPlayGuestInput::Assigned { .. }
            ) && !host.local_play_guest_active()
            {
                host.assign_local_play_guest_gamepad(assignment.family, assignment.device_number);
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn sample_controller_actions(
        &mut self,
        context: InputContext,
        now: Duration,
        samples: impl IntoIterator<Item = (InputSourceId, StandardGamepadSnapshot)>,
    ) -> std::result::Result<PlayerActionFrame, ControllerInputError> {
        Ok(self
            .sample_controller_batch(context, &ControllerInputBatch::from_snapshots(now, samples))?
            .actions)
    }

    fn sample_controller_batch(
        &mut self,
        context: InputContext,
        batch: &ControllerInputBatch,
    ) -> std::result::Result<ControllerActionBatch, ControllerInputError> {
        self.controller.set_context(context);
        let actions = self.controller.sample_batch(batch)?;
        self.latest_controller_actions = actions.actions.clone();
        Ok(actions)
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
            if let Some(navigation) = gui_navigation_from_keyboard_key(key) {
                let (handled, action) = host.mono_ui_navigate(navigation);
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
        self.observe_supplemental_movement(host, None);
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
        self.route_pointer_button_with_capture_policy(
            host, button, pressed, ui_point, true, device, queue, effects,
        )
    }

    pub fn route_touch_pointer_button<H>(
        &mut self,
        host: &mut McloneSceneHost,
        pressed: bool,
        ui_point: Point,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effects: &mut H,
    ) -> Result<MonoInputDisposition>
    where
        H: HostEffects,
    {
        self.route_pointer_button_with_capture_policy(
            host,
            PointerButton::Primary,
            pressed,
            Some(ui_point),
            false,
            device,
            queue,
            effects,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn route_pointer_button_with_capture_policy<H>(
        &mut self,
        host: &mut McloneSceneHost,
        button: PointerButton,
        pressed: bool,
        ui_point: Option<Point>,
        request_pointer_capture: bool,
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
                    host,
                    action,
                    request_pointer_capture,
                    device,
                    queue,
                    effects,
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
            let changed = host.mono_ui_scroll(direction);
            return Ok(MonoInputDisposition {
                handled: true,
                scene_changed: changed,
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

fn gui_navigation_from_keyboard_key(key: KeyboardKey) -> Option<GuiNavigation> {
    match key {
        KeyboardKey::ArrowUp => Some(GuiNavigation::Up),
        KeyboardKey::ArrowDown => Some(GuiNavigation::Down),
        KeyboardKey::ArrowLeft => Some(GuiNavigation::Left),
        KeyboardKey::ArrowRight => Some(GuiNavigation::Right),
        KeyboardKey::Space => Some(GuiNavigation::Confirm),
        _ => None,
    }
}

fn gui_navigation_from_player_action(action: mclone_input::PlayerAction) -> Option<GuiNavigation> {
    match action {
        mclone_input::PlayerAction::UiNavigateUp => Some(GuiNavigation::Up),
        mclone_input::PlayerAction::UiNavigateDown => Some(GuiNavigation::Down),
        mclone_input::PlayerAction::UiNavigateLeft => Some(GuiNavigation::Left),
        mclone_input::PlayerAction::UiNavigateRight => Some(GuiNavigation::Right),
        mclone_input::PlayerAction::UiConfirm => Some(GuiNavigation::Confirm),
        mclone_input::PlayerAction::UiBack => Some(GuiNavigation::Back),
        mclone_input::PlayerAction::UiNextPage => Some(GuiNavigation::NextPage),
        mclone_input::PlayerAction::UiPreviousPage => Some(GuiNavigation::PreviousPage),
        _ => None,
    }
}

fn local_play_controller_family(layout: ControllerLayoutFamily) -> GameLocalPlayControllerFamily {
    match layout {
        ControllerLayoutFamily::XboxLike | ControllerLayoutFamily::SteamDeckLike => {
            GameLocalPlayControllerFamily::Xbox
        }
        ControllerLayoutFamily::PlayStationLike => GameLocalPlayControllerFamily::PlayStation,
        ControllerLayoutFamily::NintendoLike => GameLocalPlayControllerFamily::Nintendo,
        ControllerLayoutFamily::Generic | ControllerLayoutFamily::Unknown => {
            GameLocalPlayControllerFamily::Gamepad
        }
    }
}

fn controller_batch_for_source(
    batch: &ControllerInputBatch,
    source_id: InputSourceId,
) -> ControllerInputBatch {
    filtered_controller_batch(batch, |candidate| candidate == source_id)
}

fn controller_batch_without_source(
    batch: &ControllerInputBatch,
    source_id: InputSourceId,
) -> ControllerInputBatch {
    filtered_controller_batch(batch, |candidate| candidate != source_id)
}

fn filtered_controller_batch(
    batch: &ControllerInputBatch,
    mut include: impl FnMut(InputSourceId) -> bool,
) -> ControllerInputBatch {
    let mut filtered = ControllerInputBatch::new(batch.sample_time());
    if batch.is_discontinuous() {
        filtered.note_dropped_observations(batch.dropped_observations());
    } else {
        for observation in batch.observations() {
            if include(observation.source_id) {
                filtered.push_observation(ControllerInputObservation {
                    source_id: observation.source_id,
                    sample_time: observation.sample_time,
                    sequence: observation.sequence,
                    snapshot: observation.snapshot,
                });
            }
        }
    }
    for (source_id, snapshot) in batch.terminal_snapshots() {
        if include(source_id) {
            filtered.set_terminal_snapshot(source_id, snapshot);
        }
    }
    filtered
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;
    use mclone_input::{
        ControllerInputObservation, InputSourceIdAllocator, StandardGamepadButtonState,
        StandardGamepadButtons,
    };

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

    #[test]
    fn router_owns_shared_controller_state_and_context() {
        let mut allocator = InputSourceIdAllocator::new();
        let source_id = allocator.allocate().expect("source");
        let mut router = MonoInteractiveInputRouter::new();
        assert!(router.connect_controller_source(
            source_id,
            InputSourceDescriptor::scripted_gamepad("scene test")
        ));
        let gameplay = router
            .sample_controller_actions(
                InputContext::Gameplay,
                Duration::ZERO,
                [(
                    source_id,
                    StandardGamepadSnapshot {
                        left_stick: Vec2::new(-1.0, 0.5),
                        buttons: StandardGamepadButtons {
                            south: StandardGamepadButtonState::pressed(),
                            ..StandardGamepadButtons::default()
                        },
                        ..StandardGamepadSnapshot::default()
                    },
                )],
            )
            .expect("gameplay controller sample");
        assert_eq!(gameplay.active_source, Some(source_id));
        assert!(gameplay.pressed.contains(&mclone_input::PlayerAction::Jump));
        let held = router.held_frame().expect("controller held frame");
        assert!(held.jump);
        assert!(held.movement.left > 0.0);

        let menu = router
            .sample_controller_actions(
                InputContext::Menu,
                Duration::from_millis(1),
                [(
                    source_id,
                    StandardGamepadSnapshot {
                        buttons: StandardGamepadButtons {
                            dpad_down: StandardGamepadButtonState::pressed(),
                            ..StandardGamepadButtons::default()
                        },
                        ..StandardGamepadSnapshot::default()
                    },
                )],
            )
            .expect("menu controller sample");
        assert!(
            menu.pressed
                .contains(&mclone_input::PlayerAction::UiNavigateDown)
        );
        assert!(!router.has_continuous_movement_input());

        router.clear_transient_input();
        assert!(router.held_frame().is_none());
        router
            .disconnect_controller_source(source_id)
            .expect("disconnect controller");
        assert_eq!(router.controller_source_count(), 0);
    }

    #[test]
    fn router_preserves_controller_edges_inside_one_presentation_interval() {
        let mut allocator = InputSourceIdAllocator::new();
        let source_id = allocator.allocate().expect("source");
        let mut router = MonoInteractiveInputRouter::new();
        router.connect_controller_source(
            source_id,
            InputSourceDescriptor::scripted_gamepad("ordered scene test"),
        );
        let pressed = StandardGamepadSnapshot {
            buttons: StandardGamepadButtons {
                south: StandardGamepadButtonState::pressed(),
                ..StandardGamepadButtons::default()
            },
            ..StandardGamepadSnapshot::default()
        };
        let mut input = ControllerInputBatch::new(Duration::from_millis(16));
        input.push_observation(ControllerInputObservation {
            source_id,
            sample_time: Duration::from_millis(4),
            sequence: 20,
            snapshot: pressed,
        });
        input.push_observation(ControllerInputObservation {
            source_id,
            sample_time: Duration::from_millis(9),
            sequence: 21,
            snapshot: StandardGamepadSnapshot::default(),
        });

        let actions = router
            .sample_controller_batch(InputContext::Gameplay, &input)
            .expect("ordered controller batch");
        assert!(
            actions
                .actions
                .pressed
                .contains(&mclone_input::PlayerAction::Jump)
        );
        assert!(
            actions
                .actions
                .released
                .contains(&mclone_input::PlayerAction::Jump)
        );
        assert_eq!(
            actions.observations[0].sample_time,
            Duration::from_millis(4)
        );
        assert_eq!(
            actions.observations[1].sample_time,
            Duration::from_millis(9)
        );
        assert!(
            actions.observations[0]
                .actions
                .pressed
                .contains(&mclone_input::PlayerAction::Jump)
        );
        assert!(
            actions.observations[1]
                .actions
                .released
                .contains(&mclone_input::PlayerAction::Jump)
        );
        assert!(!router.has_continuous_movement_input());
    }

    #[test]
    fn local_guest_batch_isolated_from_primary_controller_batch() {
        let mut allocator = InputSourceIdAllocator::new();
        let primary_source = allocator.allocate().expect("primary source");
        let guest_source = allocator.allocate().expect("guest source");
        let primary_snapshot = StandardGamepadSnapshot {
            left_stick: Vec2::Y,
            ..StandardGamepadSnapshot::default()
        };
        let guest_snapshot = StandardGamepadSnapshot {
            left_stick: Vec2::X,
            ..StandardGamepadSnapshot::default()
        };
        let mut batch = ControllerInputBatch::new(Duration::from_millis(16));
        batch.push_observation(ControllerInputObservation {
            source_id: primary_source,
            sample_time: Duration::from_millis(4),
            sequence: 10,
            snapshot: primary_snapshot,
        });
        batch.push_observation(ControllerInputObservation {
            source_id: guest_source,
            sample_time: Duration::from_millis(8),
            sequence: 11,
            snapshot: guest_snapshot,
        });
        batch.set_terminal_snapshot(primary_source, primary_snapshot);
        batch.set_terminal_snapshot(guest_source, guest_snapshot);

        let primary = controller_batch_without_source(&batch, guest_source);
        assert_eq!(primary.observations().len(), 1);
        assert_eq!(primary.observations()[0].source_id, primary_source);
        assert_eq!(
            primary.terminal_snapshots().collect::<Vec<_>>(),
            vec![(primary_source, primary_snapshot)]
        );

        let guest = controller_batch_for_source(&batch, guest_source);
        assert_eq!(guest.observations().len(), 1);
        assert_eq!(guest.observations()[0].source_id, guest_source);
        assert_eq!(
            guest.terminal_snapshots().collect::<Vec<_>>(),
            vec![(guest_source, guest_snapshot)]
        );
    }

    #[test]
    fn local_play_uses_neutral_controller_family_labels() {
        assert_eq!(
            local_play_controller_family(ControllerLayoutFamily::XboxLike),
            GameLocalPlayControllerFamily::Xbox
        );
        assert_eq!(
            local_play_controller_family(ControllerLayoutFamily::SteamDeckLike),
            GameLocalPlayControllerFamily::Xbox
        );
        assert_eq!(
            local_play_controller_family(ControllerLayoutFamily::PlayStationLike),
            GameLocalPlayControllerFamily::PlayStation
        );
        assert_eq!(
            local_play_controller_family(ControllerLayoutFamily::NintendoLike),
            GameLocalPlayControllerFamily::Nintendo
        );
        assert_eq!(
            local_play_controller_family(ControllerLayoutFamily::Unknown),
            GameLocalPlayControllerFamily::Gamepad
        );
    }

    #[test]
    fn xr_router_keeps_pose_less_gamepad_actions_in_shared_semantics() {
        let mut allocator = InputSourceIdAllocator::new();
        let source = allocator.allocate().unwrap();
        let mut router = XrControllerInputRouter::new();
        router.connect_source(
            source,
            InputSourceDescriptor::scripted_gamepad("XR test pad"),
        );

        let actions = router
            .sample_actions(
                InputContext::Gameplay,
                Duration::from_millis(16),
                [(
                    source,
                    StandardGamepadSnapshot {
                        left_stick: Vec2::Y,
                        buttons: StandardGamepadButtons {
                            south: StandardGamepadButtonState::pressed(),
                            ..StandardGamepadButtons::default()
                        },
                        ..StandardGamepadSnapshot::default()
                    },
                )],
            )
            .unwrap();
        assert!(actions.movement.forward > 0.99);
        assert!(actions.held.contains(&mclone_input::PlayerAction::Jump));
        assert_eq!(router.source_count(), 1);

        router.clear_transient_input();
        assert!(router.latest_actions().is_idle());
        router.disconnect_source(source).unwrap();
        assert_eq!(router.source_count(), 0);
    }
}
