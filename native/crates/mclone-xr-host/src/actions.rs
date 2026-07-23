use anyhow::{Context, Result};
use glam::{Quat, Vec2, Vec3};
use mclone_input::{
    ControllerInputPreferences, TrackedControllerState, XrActionChange, XrActionControl,
    XrActionSnapshot, XrControllerSpecificState, XrHand, XrInputFrame, XrInputFrameAssembler,
    XrSpecificInput,
};
use openxr as xr;

pub struct OpenXrControllerActions {
    action_set: xr::ActionSet,
    input_assembler: XrInputFrameAssembler,
    left_aim: xr::Action<xr::Posef>,
    right_aim: xr::Action<xr::Posef>,
    left_grip: xr::Action<xr::Posef>,
    right_grip: xr::Action<xr::Posef>,
    left_aim_space: xr::Space,
    right_aim_space: xr::Space,
    left_grip_space: xr::Space,
    right_grip_space: xr::Space,
    left_pointer_select_value: xr::Action<f32>,
    right_attack_value: xr::Action<f32>,
    left_squeeze_value: xr::Action<f32>,
    right_use_value: xr::Action<f32>,
    open_menu: xr::Action<bool>,
    right_simple_attack: xr::Action<bool>,
    jump: xr::Action<bool>,
    descend: xr::Action<bool>,
    sprint: xr::Action<bool>,
    move_x: xr::Action<f32>,
    move_y: xr::Action<f32>,
    turn_x: xr::Action<f32>,
    turn_y: xr::Action<f32>,
    open_block_palette: xr::Action<bool>,
    sneak: xr::Action<bool>,
}

impl OpenXrControllerActions {
    pub fn apply_controller_preferences(&mut self, preferences: &ControllerInputPreferences) {
        self.input_assembler.apply_settings(preferences.settings);
    }

    pub fn clear_transient_input(&mut self) {
        self.input_assembler.clear();
    }

    pub fn create<G: xr::Graphics>(
        instance: &xr::Instance,
        session: &xr::Session<G>,
    ) -> Result<Self> {
        Self::create_with_binding_logger(instance, session, |_, _| {})
    }

    pub fn create_with_binding_logger<G, F>(
        instance: &xr::Instance,
        session: &xr::Session<G>,
        mut on_binding_warning: F,
    ) -> Result<Self>
    where
        G: xr::Graphics,
        F: FnMut(&'static str, xr::sys::Result),
    {
        let action_set = instance
            .create_action_set("mclone_input", "mclone input", 0)
            .context("create OpenXR mclone input action set")?;
        let left_aim = action_set.create_action::<xr::Posef>("left_aim", "Left Aim", &[])?;
        let right_aim = action_set.create_action::<xr::Posef>("right_aim", "Right Aim", &[])?;
        let left_grip = action_set.create_action::<xr::Posef>("left_grip", "Left Grip", &[])?;
        let right_grip = action_set.create_action::<xr::Posef>("right_grip", "Right Grip", &[])?;
        let left_pointer_select_value =
            action_set.create_action::<f32>("left_pointer_select", "Left Pointer Select", &[])?;
        let right_attack_value = action_set.create_action::<f32>("attack", "Attack", &[])?;
        let left_squeeze_value =
            action_set.create_action::<f32>("left_hand_squeeze", "Left Hand Squeeze", &[])?;
        let right_use_value = action_set.create_action::<f32>("use", "Use", &[])?;
        let open_menu = action_set.create_action::<bool>("open_menu", "Open Menu", &[])?;
        let right_simple_attack =
            action_set.create_action::<bool>("simple_attack", "Attack", &[])?;
        let jump = action_set.create_action::<bool>("jump", "Jump", &[])?;
        let descend = action_set.create_action::<bool>("descend", "Descend", &[])?;
        let sprint = action_set.create_action::<bool>("sprint", "Sprint", &[])?;
        let move_x = action_set.create_action::<f32>("move_x", "Move X", &[])?;
        let move_y = action_set.create_action::<f32>("move_y", "Move Y", &[])?;
        let turn_x = action_set.create_action::<f32>("turn_x", "Turn X", &[])?;
        let turn_y = action_set.create_action::<f32>("turn_y", "Turn Y", &[])?;
        let open_block_palette =
            action_set.create_action::<bool>("open_block_palette", "Open Block Palette", &[])?;
        let sneak = action_set.create_action::<bool>("sneak", "Sneak", &[])?;

        Self::suggest_simple_controller_bindings(
            instance,
            &left_aim,
            &right_aim,
            &left_grip,
            &right_grip,
            &open_menu,
            &right_simple_attack,
            &mut on_binding_warning,
        )?;
        Self::suggest_touch_controller_bindings(
            instance,
            &left_aim,
            &right_aim,
            &left_grip,
            &right_grip,
            &left_pointer_select_value,
            &right_attack_value,
            &left_squeeze_value,
            &right_use_value,
            &open_menu,
            &jump,
            &descend,
            &sprint,
            &move_x,
            &move_y,
            &turn_x,
            &turn_y,
            &open_block_palette,
            &sneak,
            &mut on_binding_warning,
        )?;

        session
            .attach_action_sets(&[&action_set])
            .context("attach OpenXR input action set")?;
        let left_aim_space = left_aim.create_space(session, xr::Path::NULL, xr::Posef::IDENTITY)?;
        let right_aim_space =
            right_aim.create_space(session, xr::Path::NULL, xr::Posef::IDENTITY)?;
        let left_grip_space =
            left_grip.create_space(session, xr::Path::NULL, xr::Posef::IDENTITY)?;
        let right_grip_space =
            right_grip.create_space(session, xr::Path::NULL, xr::Posef::IDENTITY)?;

        Ok(Self {
            action_set,
            input_assembler: XrInputFrameAssembler::new(),
            left_aim,
            right_aim,
            left_grip,
            right_grip,
            left_aim_space,
            right_aim_space,
            left_grip_space,
            right_grip_space,
            left_pointer_select_value,
            right_attack_value,
            left_squeeze_value,
            right_use_value,
            open_menu,
            right_simple_attack,
            jump,
            descend,
            sprint,
            move_x,
            move_y,
            turn_x,
            turn_y,
            open_block_palette,
            sneak,
        })
    }

    pub fn poll<G: xr::Graphics>(
        &mut self,
        session: &xr::Session<G>,
        stage: &xr::Space,
        time: xr::Time,
    ) -> Result<XrInputFrame> {
        session
            .sync_actions(&[(&self.action_set).into()])
            .context("sync OpenXR controller actions")?;
        let mut tracked = Vec::with_capacity(2);
        if let Some(state) = self.read_tracked_hand(session, stage, time, XrHand::Left) {
            tracked.push(state);
        }
        if let Some(state) = self.read_tracked_hand(session, stage, time, XrHand::Right) {
            tracked.push(state);
        }
        let mut action_changes = Vec::new();
        let movement_axis = Vec2::new(
            Self::read_float_action(
                session,
                &self.move_x,
                XrActionControl::MovementX,
                &mut action_changes,
            ),
            Self::read_float_action(
                session,
                &self.move_y,
                XrActionControl::MovementY,
                &mut action_changes,
            ),
        );
        let turn_axis = Vec2::new(
            Self::read_float_action(
                session,
                &self.turn_x,
                XrActionControl::TurnX,
                &mut action_changes,
            ),
            Self::read_float_action(
                session,
                &self.turn_y,
                XrActionControl::TurnY,
                &mut action_changes,
            ),
        );
        let left_pointer_select_value = Self::read_float_action(
            session,
            &self.left_pointer_select_value,
            XrActionControl::PointerSelect,
            &mut action_changes,
        );
        let right_attack_value = Self::read_float_action(
            session,
            &self.right_attack_value,
            XrActionControl::Attack,
            &mut action_changes,
        )
        .max(f32::from(Self::read_bool_action(
            session,
            &self.right_simple_attack,
            XrActionControl::SimpleAttack,
            &mut action_changes,
        )));
        let left_squeeze_value = Self::read_float_action(
            session,
            &self.left_squeeze_value,
            XrActionControl::Squeeze,
            &mut action_changes,
        );
        let right_use_value = Self::read_float_action(
            session,
            &self.right_use_value,
            XrActionControl::Use,
            &mut action_changes,
        );
        let mut frame = self.input_assembler.sample(
            XrActionSnapshot {
                movement_axis,
                turn_axis,
                attack_value: right_attack_value,
                use_value: right_use_value,
                jump: Self::read_bool_action(
                    session,
                    &self.jump,
                    XrActionControl::Jump,
                    &mut action_changes,
                ),
                sprint: Self::read_bool_action(
                    session,
                    &self.sprint,
                    XrActionControl::Sprint,
                    &mut action_changes,
                ),
                sneak: Self::read_bool_action(
                    session,
                    &self.sneak,
                    XrActionControl::Sneak,
                    &mut action_changes,
                ),
                descend: Self::read_bool_action(
                    session,
                    &self.descend,
                    XrActionControl::Descend,
                    &mut action_changes,
                ),
                open_menu: Self::read_bool_action(
                    session,
                    &self.open_menu,
                    XrActionControl::OpenMenu,
                    &mut action_changes,
                ),
                open_block_palette: Self::read_bool_action(
                    session,
                    &self.open_block_palette,
                    XrActionControl::OpenBlockPalette,
                    &mut action_changes,
                ),
            },
            tracked,
            XrSpecificInput {
                controllers: vec![
                    XrControllerSpecificState {
                        hand: Some(XrHand::Left),
                        pointer_select_value: left_pointer_select_value,
                        squeeze_value: left_squeeze_value,
                        locomotion_axis: movement_axis,
                        turn_axis: Vec2::ZERO,
                    },
                    XrControllerSpecificState {
                        hand: Some(XrHand::Right),
                        pointer_select_value: right_attack_value,
                        squeeze_value: right_use_value,
                        locomotion_axis: Vec2::ZERO,
                        turn_axis,
                    },
                ],
            },
        );
        frame.action_changes = action_changes;
        Ok(frame)
    }

    fn suggest_simple_controller_bindings<F>(
        instance: &xr::Instance,
        left_aim: &xr::Action<xr::Posef>,
        right_aim: &xr::Action<xr::Posef>,
        left_grip: &xr::Action<xr::Posef>,
        right_grip: &xr::Action<xr::Posef>,
        open_menu: &xr::Action<bool>,
        right_simple_attack: &xr::Action<bool>,
        on_binding_warning: &mut F,
    ) -> Result<()>
    where
        F: FnMut(&'static str, xr::sys::Result),
    {
        let bindings = [
            xr::Binding::new(
                left_aim,
                instance.string_to_path("/user/hand/left/input/aim/pose")?,
            ),
            xr::Binding::new(
                right_aim,
                instance.string_to_path("/user/hand/right/input/aim/pose")?,
            ),
            xr::Binding::new(
                left_grip,
                instance.string_to_path("/user/hand/left/input/grip/pose")?,
            ),
            xr::Binding::new(
                right_grip,
                instance.string_to_path("/user/hand/right/input/grip/pose")?,
            ),
            xr::Binding::new(
                open_menu,
                instance.string_to_path("/user/hand/left/input/select/click")?,
            ),
            xr::Binding::new(
                right_simple_attack,
                instance.string_to_path("/user/hand/right/input/select/click")?,
            ),
        ];
        Self::suggest_bindings(
            instance,
            "/interaction_profiles/khr/simple_controller",
            &bindings,
            on_binding_warning,
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn suggest_touch_controller_bindings<F>(
        instance: &xr::Instance,
        left_aim: &xr::Action<xr::Posef>,
        right_aim: &xr::Action<xr::Posef>,
        left_grip: &xr::Action<xr::Posef>,
        right_grip: &xr::Action<xr::Posef>,
        left_pointer_select_value: &xr::Action<f32>,
        right_attack_value: &xr::Action<f32>,
        left_squeeze_value: &xr::Action<f32>,
        right_use_value: &xr::Action<f32>,
        open_menu: &xr::Action<bool>,
        jump: &xr::Action<bool>,
        descend: &xr::Action<bool>,
        sprint: &xr::Action<bool>,
        move_x: &xr::Action<f32>,
        move_y: &xr::Action<f32>,
        turn_x: &xr::Action<f32>,
        turn_y: &xr::Action<f32>,
        open_block_palette: &xr::Action<bool>,
        sneak: &xr::Action<bool>,
        on_binding_warning: &mut F,
    ) -> Result<()>
    where
        F: FnMut(&'static str, xr::sys::Result),
    {
        let bindings = vec![
            xr::Binding::new(
                left_aim,
                instance.string_to_path("/user/hand/left/input/aim/pose")?,
            ),
            xr::Binding::new(
                right_aim,
                instance.string_to_path("/user/hand/right/input/aim/pose")?,
            ),
            xr::Binding::new(
                left_grip,
                instance.string_to_path("/user/hand/left/input/grip/pose")?,
            ),
            xr::Binding::new(
                right_grip,
                instance.string_to_path("/user/hand/right/input/grip/pose")?,
            ),
            xr::Binding::new(
                left_pointer_select_value,
                instance.string_to_path("/user/hand/left/input/trigger/value")?,
            ),
            xr::Binding::new(
                right_attack_value,
                instance.string_to_path("/user/hand/right/input/trigger/value")?,
            ),
            xr::Binding::new(
                left_squeeze_value,
                instance.string_to_path("/user/hand/left/input/squeeze/value")?,
            ),
            xr::Binding::new(
                right_use_value,
                instance.string_to_path("/user/hand/right/input/squeeze/value")?,
            ),
            xr::Binding::new(
                open_menu,
                instance.string_to_path("/user/hand/left/input/x/click")?,
            ),
            xr::Binding::new(
                sprint,
                instance.string_to_path("/user/hand/left/input/y/click")?,
            ),
            xr::Binding::new(
                jump,
                instance.string_to_path("/user/hand/right/input/a/click")?,
            ),
            xr::Binding::new(
                descend,
                instance.string_to_path("/user/hand/right/input/b/click")?,
            ),
            xr::Binding::new(
                move_x,
                instance.string_to_path("/user/hand/left/input/thumbstick/x")?,
            ),
            xr::Binding::new(
                move_y,
                instance.string_to_path("/user/hand/left/input/thumbstick/y")?,
            ),
            xr::Binding::new(
                turn_x,
                instance.string_to_path("/user/hand/right/input/thumbstick/x")?,
            ),
            xr::Binding::new(
                turn_y,
                instance.string_to_path("/user/hand/right/input/thumbstick/y")?,
            ),
            xr::Binding::new(
                open_block_palette,
                instance.string_to_path("/user/hand/left/input/thumbstick/click")?,
            ),
            xr::Binding::new(
                sneak,
                instance.string_to_path("/user/hand/right/input/thumbstick/click")?,
            ),
        ];
        for profile in [
            "/interaction_profiles/oculus/touch_controller",
            "/interaction_profiles/valve/index_controller",
            "/interaction_profiles/htc/vive_controller",
            "/interaction_profiles/microsoft/motion_controller",
        ] {
            Self::suggest_bindings(instance, profile, &bindings, on_binding_warning);
        }
        Ok(())
    }

    fn suggest_bindings<F>(
        instance: &xr::Instance,
        profile: &'static str,
        bindings: &[xr::Binding<'_>],
        on_binding_warning: &mut F,
    ) where
        F: FnMut(&'static str, xr::sys::Result),
    {
        if let Err(err) = instance.string_to_path(profile).and_then(|profile_path| {
            instance.suggest_interaction_profile_bindings(profile_path, bindings)
        }) {
            on_binding_warning(profile, err);
        }
    }

    fn read_tracked_hand<G: xr::Graphics>(
        &self,
        session: &xr::Session<G>,
        stage: &xr::Space,
        time: xr::Time,
        hand: XrHand,
    ) -> Option<TrackedControllerState> {
        let (aim_action, aim_space, grip_action, grip_space) = match hand {
            XrHand::Left => (
                &self.left_aim,
                &self.left_aim_space,
                &self.left_grip,
                &self.left_grip_space,
            ),
            XrHand::Right => (
                &self.right_aim,
                &self.right_aim_space,
                &self.right_grip,
                &self.right_grip_space,
            ),
        };

        let aim_active = aim_action
            .is_active(session, xr::Path::NULL)
            .unwrap_or(false);
        let grip_active = grip_action
            .is_active(session, xr::Path::NULL)
            .unwrap_or(false);
        let aim_pose = aim_active
            .then(|| Self::locate_pose(aim_space, stage, time))
            .flatten();
        let grip_pose = grip_active
            .then(|| Self::locate_pose(grip_space, stage, time))
            .flatten();
        if !aim_active && !grip_active {
            return None;
        }

        Some(TrackedControllerState {
            hand,
            aim_position: aim_pose.map(|pose| pose.position),
            aim_direction: aim_pose.map(|pose| pose.forward),
            grip_position: grip_pose.map(|pose| pose.position),
            grip_orientation: grip_pose.map(|pose| pose.orientation),
        })
    }

    fn locate_pose(space: &xr::Space, stage: &xr::Space, time: xr::Time) -> Option<XrActionPose> {
        let location = space.locate(stage, time).ok()?;
        if !location
            .location_flags
            .contains(xr::SpaceLocationFlags::POSITION_VALID)
            || !location
                .location_flags
                .contains(xr::SpaceLocationFlags::ORIENTATION_VALID)
        {
            return None;
        }
        let position = Vec3::new(
            location.pose.position.x,
            location.pose.position.y,
            location.pose.position.z,
        );
        let orientation = Quat::from_xyzw(
            location.pose.orientation.x,
            location.pose.orientation.y,
            location.pose.orientation.z,
            location.pose.orientation.w,
        );
        if !position.is_finite() || !finite_quat(orientation) || orientation.length_squared() < 0.5
        {
            return None;
        }
        Some(XrActionPose {
            position,
            orientation: orientation.normalize(),
            forward: (orientation.normalize() * Vec3::NEG_Z).normalize_or_zero(),
        })
    }

    fn read_float_action<G: xr::Graphics>(
        session: &xr::Session<G>,
        action: &xr::Action<f32>,
        control: XrActionControl,
        changes: &mut Vec<XrActionChange>,
    ) -> f32 {
        let Ok(state) = action.state(session, xr::Path::NULL) else {
            return 0.0;
        };
        if state.changed_since_last_sync {
            changes.push(XrActionChange {
                control,
                source_time_nanos: state.last_change_time.as_nanos(),
            });
        }
        if state.is_active {
            state.current_state
        } else {
            0.0
        }
    }

    fn read_bool_action<G: xr::Graphics>(
        session: &xr::Session<G>,
        action: &xr::Action<bool>,
        control: XrActionControl,
        changes: &mut Vec<XrActionChange>,
    ) -> bool {
        let Ok(state) = action.state(session, xr::Path::NULL) else {
            return false;
        };
        if state.changed_since_last_sync {
            changes.push(XrActionChange {
                control,
                source_time_nanos: state.last_change_time.as_nanos(),
            });
        }
        state.is_active && state.current_state
    }
}

#[derive(Clone, Copy, Debug)]
struct XrActionPose {
    position: Vec3,
    orientation: Quat,
    forward: Vec3,
}

fn finite_quat(value: Quat) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite() && value.w.is_finite()
}
