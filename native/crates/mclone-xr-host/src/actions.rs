use anyhow::{Context, Result};
use glam::{Quat, Vec2, Vec3};
use openxr as xr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XrHand {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug)]
pub struct XrControllerSnapshot {
    pub hand: XrHand,
    pub aim_position: Option<Vec3>,
    pub aim_direction: Option<Vec3>,
    pub grip_position: Option<Vec3>,
    pub trigger: f32,
    pub squeeze: f32,
    pub select_pressed: bool,
    pub a_pressed: bool,
    pub b_pressed: bool,
    pub y_pressed: bool,
    pub thumbstick: Vec2,
    pub thumbstick_pressed: bool,
}

pub struct OpenXrControllerActions {
    action_set: xr::ActionSet,
    left_aim: xr::Action<xr::Posef>,
    right_aim: xr::Action<xr::Posef>,
    left_grip: xr::Action<xr::Posef>,
    right_grip: xr::Action<xr::Posef>,
    left_aim_space: xr::Space,
    right_aim_space: xr::Space,
    left_grip_space: xr::Space,
    right_grip_space: xr::Space,
    left_trigger: xr::Action<f32>,
    right_trigger: xr::Action<f32>,
    left_squeeze: xr::Action<f32>,
    right_squeeze: xr::Action<f32>,
    left_select: xr::Action<bool>,
    right_select: xr::Action<bool>,
    right_a_click: xr::Action<bool>,
    right_b_click: xr::Action<bool>,
    left_y_click: xr::Action<bool>,
    left_thumbstick_x: xr::Action<f32>,
    left_thumbstick_y: xr::Action<f32>,
    right_thumbstick_x: xr::Action<f32>,
    right_thumbstick_y: xr::Action<f32>,
    left_thumbstick_click: xr::Action<bool>,
    right_thumbstick_click: xr::Action<bool>,
}

impl OpenXrControllerActions {
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
        let left_trigger = action_set.create_action::<f32>("left_trigger", "Left Trigger", &[])?;
        let right_trigger =
            action_set.create_action::<f32>("right_trigger", "Right Trigger", &[])?;
        let left_squeeze = action_set.create_action::<f32>("left_squeeze", "Left Squeeze", &[])?;
        let right_squeeze =
            action_set.create_action::<f32>("right_squeeze", "Right Squeeze", &[])?;
        let left_select = action_set.create_action::<bool>("left_select", "Left Select", &[])?;
        let right_select = action_set.create_action::<bool>("right_select", "Right Select", &[])?;
        let right_a_click =
            action_set.create_action::<bool>("right_a_click", "Right A Button", &[])?;
        let right_b_click =
            action_set.create_action::<bool>("right_b_click", "Right B Button", &[])?;
        let left_y_click =
            action_set.create_action::<bool>("left_y_click", "Left Y Button", &[])?;
        let left_thumbstick_x =
            action_set.create_action::<f32>("left_thumbstick_x", "Left Thumbstick X", &[])?;
        let left_thumbstick_y =
            action_set.create_action::<f32>("left_thumbstick_y", "Left Thumbstick Y", &[])?;
        let right_thumbstick_x =
            action_set.create_action::<f32>("right_thumbstick_x", "Right Thumbstick X", &[])?;
        let right_thumbstick_y =
            action_set.create_action::<f32>("right_thumbstick_y", "Right Thumbstick Y", &[])?;
        let left_thumbstick_click = action_set.create_action::<bool>(
            "left_thumbstick_click",
            "Left Thumbstick Click",
            &[],
        )?;
        let right_thumbstick_click = action_set.create_action::<bool>(
            "right_thumbstick_click",
            "Right Thumbstick Click",
            &[],
        )?;

        Self::suggest_simple_controller_bindings(
            instance,
            &left_aim,
            &right_aim,
            &left_grip,
            &right_grip,
            &left_select,
            &right_select,
            &mut on_binding_warning,
        )?;
        Self::suggest_touch_controller_bindings(
            instance,
            &left_aim,
            &right_aim,
            &left_grip,
            &right_grip,
            &left_trigger,
            &right_trigger,
            &left_squeeze,
            &right_squeeze,
            &left_select,
            &right_a_click,
            &right_b_click,
            &left_y_click,
            &left_thumbstick_x,
            &left_thumbstick_y,
            &right_thumbstick_x,
            &right_thumbstick_y,
            &left_thumbstick_click,
            &right_thumbstick_click,
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
            left_aim,
            right_aim,
            left_grip,
            right_grip,
            left_aim_space,
            right_aim_space,
            left_grip_space,
            right_grip_space,
            left_trigger,
            right_trigger,
            left_squeeze,
            right_squeeze,
            left_select,
            right_select,
            right_a_click,
            right_b_click,
            left_y_click,
            left_thumbstick_x,
            left_thumbstick_y,
            right_thumbstick_x,
            right_thumbstick_y,
            left_thumbstick_click,
            right_thumbstick_click,
        })
    }

    pub fn poll<G: xr::Graphics>(
        &self,
        session: &xr::Session<G>,
        stage: &xr::Space,
        time: xr::Time,
    ) -> Result<Vec<XrControllerSnapshot>> {
        session
            .sync_actions(&[(&self.action_set).into()])
            .context("sync OpenXR controller actions")?;
        let mut snapshots = Vec::with_capacity(2);
        if let Some(snapshot) = self.read_hand(session, stage, time, XrHand::Left) {
            snapshots.push(snapshot);
        }
        if let Some(snapshot) = self.read_hand(session, stage, time, XrHand::Right) {
            snapshots.push(snapshot);
        }
        Ok(snapshots)
    }

    fn suggest_simple_controller_bindings<F>(
        instance: &xr::Instance,
        left_aim: &xr::Action<xr::Posef>,
        right_aim: &xr::Action<xr::Posef>,
        left_grip: &xr::Action<xr::Posef>,
        right_grip: &xr::Action<xr::Posef>,
        left_select: &xr::Action<bool>,
        right_select: &xr::Action<bool>,
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
                left_select,
                instance.string_to_path("/user/hand/left/input/select/click")?,
            ),
            xr::Binding::new(
                right_select,
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
        left_trigger: &xr::Action<f32>,
        right_trigger: &xr::Action<f32>,
        left_squeeze: &xr::Action<f32>,
        right_squeeze: &xr::Action<f32>,
        left_select: &xr::Action<bool>,
        right_a_click: &xr::Action<bool>,
        right_b_click: &xr::Action<bool>,
        left_y_click: &xr::Action<bool>,
        left_thumbstick_x: &xr::Action<f32>,
        left_thumbstick_y: &xr::Action<f32>,
        right_thumbstick_x: &xr::Action<f32>,
        right_thumbstick_y: &xr::Action<f32>,
        left_thumbstick_click: &xr::Action<bool>,
        right_thumbstick_click: &xr::Action<bool>,
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
                left_trigger,
                instance.string_to_path("/user/hand/left/input/trigger/value")?,
            ),
            xr::Binding::new(
                right_trigger,
                instance.string_to_path("/user/hand/right/input/trigger/value")?,
            ),
            xr::Binding::new(
                left_squeeze,
                instance.string_to_path("/user/hand/left/input/squeeze/value")?,
            ),
            xr::Binding::new(
                right_squeeze,
                instance.string_to_path("/user/hand/right/input/squeeze/value")?,
            ),
            xr::Binding::new(
                left_select,
                instance.string_to_path("/user/hand/left/input/x/click")?,
            ),
            xr::Binding::new(
                left_y_click,
                instance.string_to_path("/user/hand/left/input/y/click")?,
            ),
            xr::Binding::new(
                right_a_click,
                instance.string_to_path("/user/hand/right/input/a/click")?,
            ),
            xr::Binding::new(
                right_b_click,
                instance.string_to_path("/user/hand/right/input/b/click")?,
            ),
            xr::Binding::new(
                left_thumbstick_x,
                instance.string_to_path("/user/hand/left/input/thumbstick/x")?,
            ),
            xr::Binding::new(
                left_thumbstick_y,
                instance.string_to_path("/user/hand/left/input/thumbstick/y")?,
            ),
            xr::Binding::new(
                right_thumbstick_x,
                instance.string_to_path("/user/hand/right/input/thumbstick/x")?,
            ),
            xr::Binding::new(
                right_thumbstick_y,
                instance.string_to_path("/user/hand/right/input/thumbstick/y")?,
            ),
            xr::Binding::new(
                left_thumbstick_click,
                instance.string_to_path("/user/hand/left/input/thumbstick/click")?,
            ),
            xr::Binding::new(
                right_thumbstick_click,
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

    fn read_hand<G: xr::Graphics>(
        &self,
        session: &xr::Session<G>,
        stage: &xr::Space,
        time: xr::Time,
        hand: XrHand,
    ) -> Option<XrControllerSnapshot> {
        let (
            aim_action,
            aim_space,
            grip_action,
            grip_space,
            trigger_action,
            squeeze_action,
            select_action,
            thumbstick_x_action,
            thumbstick_y_action,
            thumbstick_click_action,
            a_click_action,
            b_click_action,
            y_click_action,
        ) = match hand {
            XrHand::Left => (
                &self.left_aim,
                &self.left_aim_space,
                &self.left_grip,
                &self.left_grip_space,
                &self.left_trigger,
                &self.left_squeeze,
                &self.left_select,
                &self.left_thumbstick_x,
                &self.left_thumbstick_y,
                &self.left_thumbstick_click,
                None,
                None,
                Some(&self.left_y_click),
            ),
            XrHand::Right => (
                &self.right_aim,
                &self.right_aim_space,
                &self.right_grip,
                &self.right_grip_space,
                &self.right_trigger,
                &self.right_squeeze,
                &self.right_select,
                &self.right_thumbstick_x,
                &self.right_thumbstick_y,
                &self.right_thumbstick_click,
                Some(&self.right_a_click),
                Some(&self.right_b_click),
                None,
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

        Some(XrControllerSnapshot {
            hand,
            aim_position: aim_pose.map(|pose| pose.position),
            aim_direction: aim_pose.map(|pose| pose.forward),
            grip_position: grip_pose.map(|pose| pose.position),
            trigger: Self::read_float_action(session, trigger_action),
            squeeze: Self::read_float_action(session, squeeze_action),
            select_pressed: Self::read_bool_action(session, select_action),
            a_pressed: a_click_action
                .map(|action| Self::read_bool_action(session, action))
                .unwrap_or(false),
            b_pressed: b_click_action
                .map(|action| Self::read_bool_action(session, action))
                .unwrap_or(false),
            y_pressed: y_click_action
                .map(|action| Self::read_bool_action(session, action))
                .unwrap_or(false),
            thumbstick: Vec2::new(
                Self::read_float_action(session, thumbstick_x_action),
                Self::read_float_action(session, thumbstick_y_action),
            ),
            thumbstick_pressed: Self::read_bool_action(session, thumbstick_click_action),
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
            forward: (orientation.normalize() * Vec3::NEG_Z).normalize_or_zero(),
        })
    }

    fn read_float_action<G: xr::Graphics>(
        session: &xr::Session<G>,
        action: &xr::Action<f32>,
    ) -> f32 {
        action
            .state(session, xr::Path::NULL)
            .ok()
            .filter(|state| state.is_active)
            .map(|state| state.current_state)
            .unwrap_or(0.0)
    }

    fn read_bool_action<G: xr::Graphics>(
        session: &xr::Session<G>,
        action: &xr::Action<bool>,
    ) -> bool {
        action
            .state(session, xr::Path::NULL)
            .ok()
            .filter(|state| state.is_active)
            .map(|state| state.current_state)
            .unwrap_or(false)
    }
}

#[derive(Clone, Copy, Debug)]
struct XrActionPose {
    position: Vec3,
    forward: Vec3,
}

fn finite_quat(value: Quat) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite() && value.w.is_finite()
}
