use anyhow::{Context, Result};
use glam::{Quat, Vec2, Vec3};
use openxr as xr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum XrHand {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct XrControllerSnapshot {
    pub(super) hand: XrHand,
    pub(super) aim_position: Option<Vec3>,
    pub(super) grip_position: Option<Vec3>,
    pub(super) trigger: f32,
    pub(super) squeeze: f32,
    pub(super) select_pressed: bool,
    pub(super) a_pressed: bool,
    pub(super) thumbstick: Vec2,
    pub(super) thumbstick_pressed: bool,
}

pub(super) struct OpenXrControllerActions {
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
    left_thumbstick_x: xr::Action<f32>,
    left_thumbstick_y: xr::Action<f32>,
    right_thumbstick_x: xr::Action<f32>,
    right_thumbstick_y: xr::Action<f32>,
    left_thumbstick_click: xr::Action<bool>,
    right_thumbstick_click: xr::Action<bool>,
}

impl OpenXrControllerActions {
    pub(super) fn create<G: xr::Graphics>(
        instance: &xr::Instance,
        session: &xr::Session<G>,
    ) -> Result<Self> {
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
            &right_select,
            &right_a_click,
            &left_thumbstick_x,
            &left_thumbstick_y,
            &right_thumbstick_x,
            &right_thumbstick_y,
            &left_thumbstick_click,
            &right_thumbstick_click,
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

        println!(
            "OpenXR controller actions: requested binding profiles=simple_controller, oculus_touch, valve_index, htc_vive, microsoft_motion_controller"
        );

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
            left_thumbstick_x,
            left_thumbstick_y,
            right_thumbstick_x,
            right_thumbstick_y,
            left_thumbstick_click,
            right_thumbstick_click,
        })
    }

    pub(super) fn poll<G: xr::Graphics>(
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

    fn suggest_simple_controller_bindings(
        instance: &xr::Instance,
        left_aim: &xr::Action<xr::Posef>,
        right_aim: &xr::Action<xr::Posef>,
        left_grip: &xr::Action<xr::Posef>,
        right_grip: &xr::Action<xr::Posef>,
        left_select: &xr::Action<bool>,
        right_select: &xr::Action<bool>,
    ) -> Result<()> {
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
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn suggest_touch_controller_bindings(
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
        right_select: &xr::Action<bool>,
        right_a_click: &xr::Action<bool>,
        left_thumbstick_x: &xr::Action<f32>,
        left_thumbstick_y: &xr::Action<f32>,
        right_thumbstick_x: &xr::Action<f32>,
        right_thumbstick_y: &xr::Action<f32>,
        left_thumbstick_click: &xr::Action<bool>,
        right_thumbstick_click: &xr::Action<bool>,
    ) -> Result<()> {
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
                left_select,
                instance.string_to_path("/user/hand/left/input/y/click")?,
            ),
            xr::Binding::new(
                right_a_click,
                instance.string_to_path("/user/hand/right/input/a/click")?,
            ),
            xr::Binding::new(
                right_select,
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
            Self::suggest_bindings(instance, profile, &bindings);
        }
        Ok(())
    }

    fn suggest_bindings(instance: &xr::Instance, profile: &str, bindings: &[xr::Binding<'_>]) {
        match instance.string_to_path(profile).and_then(|profile_path| {
            instance.suggest_interaction_profile_bindings(profile_path, bindings)
        }) {
            Ok(()) => {}
            Err(err) => println!("OpenXR binding suggestion unavailable for {profile}: {err:?}"),
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
            ),
        };

        let aim_active = aim_action
            .is_active(session, xr::Path::NULL)
            .unwrap_or(false);
        let grip_active = grip_action
            .is_active(session, xr::Path::NULL)
            .unwrap_or(false);
        let aim_position = aim_active
            .then(|| Self::locate_position(aim_space, stage, time))
            .flatten();
        let grip_position = grip_active
            .then(|| Self::locate_position(grip_space, stage, time))
            .flatten();
        if !aim_active && !grip_active {
            return None;
        }

        Some(XrControllerSnapshot {
            hand,
            aim_position,
            grip_position,
            trigger: Self::read_float_action(session, trigger_action),
            squeeze: Self::read_float_action(session, squeeze_action),
            select_pressed: Self::read_bool_action(session, select_action),
            a_pressed: a_click_action
                .map(|action| Self::read_bool_action(session, action))
                .unwrap_or(false),
            thumbstick: Vec2::new(
                Self::read_float_action(session, thumbstick_x_action),
                Self::read_float_action(session, thumbstick_y_action),
            ),
            thumbstick_pressed: Self::read_bool_action(session, thumbstick_click_action),
        })
    }

    fn locate_position(space: &xr::Space, stage: &xr::Space, time: xr::Time) -> Option<Vec3> {
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
        let (_, rotation) = openxr_pose_to_glam(location.pose);
        if !rotation.is_finite() {
            return None;
        }
        Some(Vec3::new(
            location.pose.position.x,
            location.pose.position.y,
            location.pose.position.z,
        ))
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

#[derive(Default)]
pub(super) struct XrControllerInputSummary {
    frames_polled: u32,
    left_active_frames: u32,
    right_active_frames: u32,
    left_tracked_frames: u32,
    right_tracked_frames: u32,
    max_trigger: f32,
    max_squeeze: f32,
    max_thumbstick: f32,
    select_pressed_frames: u32,
    a_pressed_frames: u32,
    latest_left: Option<XrControllerSnapshot>,
    latest_right: Option<XrControllerSnapshot>,
}

impl XrControllerInputSummary {
    pub(super) fn record(&mut self, snapshots: &[XrControllerSnapshot]) {
        self.frames_polled += 1;
        for snapshot in snapshots {
            let tracked = snapshot.aim_position.is_some() || snapshot.grip_position.is_some();
            match snapshot.hand {
                XrHand::Left => {
                    self.left_active_frames += 1;
                    self.left_tracked_frames += u32::from(tracked);
                    self.latest_left = Some(*snapshot);
                }
                XrHand::Right => {
                    self.right_active_frames += 1;
                    self.right_tracked_frames += u32::from(tracked);
                    self.latest_right = Some(*snapshot);
                }
            }
            self.max_trigger = self.max_trigger.max(snapshot.trigger);
            self.max_squeeze = self.max_squeeze.max(snapshot.squeeze);
            self.max_thumbstick = self.max_thumbstick.max(snapshot.thumbstick.length());
            self.select_pressed_frames += u32::from(snapshot.select_pressed);
            self.a_pressed_frames += u32::from(snapshot.a_pressed);
        }
    }

    pub(super) fn print_summary(&self) {
        println!(
            "OpenXR controller input summary: frames_polled={} left_active={} right_active={} left_tracked={} right_tracked={} max_trigger={:.3} max_squeeze={:.3} max_thumbstick={:.3} select_pressed_frames={} a_pressed_frames={}",
            self.frames_polled,
            self.left_active_frames,
            self.right_active_frames,
            self.left_tracked_frames,
            self.right_tracked_frames,
            self.max_trigger,
            self.max_squeeze,
            self.max_thumbstick,
            self.select_pressed_frames,
            self.a_pressed_frames
        );
        if let Some(left) = self.latest_left {
            println!("OpenXR controller latest left: {}", format_snapshot(left));
        }
        if let Some(right) = self.latest_right {
            println!("OpenXR controller latest right: {}", format_snapshot(right));
        }
    }
}

fn format_snapshot(snapshot: XrControllerSnapshot) -> String {
    format!(
        "aim={} grip={} trigger={:.3} squeeze={:.3} select={} a={} thumbstick=({:.3}, {:.3}) thumbstick_pressed={}",
        format_position(snapshot.aim_position),
        format_position(snapshot.grip_position),
        snapshot.trigger,
        snapshot.squeeze,
        snapshot.select_pressed,
        snapshot.a_pressed,
        snapshot.thumbstick.x,
        snapshot.thumbstick.y,
        snapshot.thumbstick_pressed
    )
}

fn format_position(position: Option<Vec3>) -> String {
    position
        .map(|position| format!("({:.3}, {:.3}, {:.3})", position.x, position.y, position.z))
        .unwrap_or_else(|| "untracked".to_owned())
}

fn openxr_pose_to_glam(pose: xr::Posef) -> (Vec3, Quat) {
    (
        Vec3::new(pose.position.x, pose.position.y, pose.position.z),
        Quat::from_xyzw(
            pose.orientation.x,
            pose.orientation.y,
            pose.orientation.z,
            pose.orientation.w,
        )
        .normalize(),
    )
}
