use super::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct XrBlinkTeleportState {
    active: bool,
    preview: Option<TeleportPreview>,
    first_request_id: Option<TeleportPreviewRequestId>,
    preview_request_id: Option<TeleportPreviewRequestId>,
    base_yaw_degrees: f64,
    target_yaw_degrees: f64,
    activation_left_stick_angle_radians: Option<f64>,
    last_submitted_intent: Option<TeleportIntent>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct XrBlinkTeleportFrame {
    pub(super) suppress_left_stick_movement: bool,
}

pub(crate) fn xr_blink_teleport_config() -> TeleportConfig {
    TeleportConfig {
        max_distance: XR_BLINK_TELEPORT_MAX_DISTANCE,
        arc_height: XR_BLINK_TELEPORT_ARC_HEIGHT,
        ..TeleportConfig::default()
    }
}

pub(crate) fn xr_blink_teleport_disabled_frame(
    travel_assist_mode: GameTravelAssistMode,
    _controllers: &[XrControllerSnapshot],
) -> Option<XrBlinkTeleportFrame> {
    (travel_assist_mode != GameTravelAssistMode::Blink).then_some(XrBlinkTeleportFrame {
        suppress_left_stick_movement: false,
    })
}

pub(crate) fn xr_blink_teleport_intent(
    camera: &EngineCameraController,
    controllers: &[XrControllerSnapshot],
    transform: XrStageToWorld,
    target_yaw_degrees: f64,
) -> Option<TeleportIntent> {
    let controller = controllers
        .iter()
        .find(|controller| controller.hand == XrHand::Left)?;
    let aim_origin = transform.transform_position(controller.aim_position?);
    let aim_direction = transform.transform_direction(controller.aim_direction?);
    if !aim_origin.is_finite()
        || !aim_direction.is_finite()
        || aim_direction.length_squared() <= f32::EPSILON
    {
        return None;
    }
    let pose = camera.player().pose();
    Some(TeleportIntent::new(
        pose.position,
        vec3d_from_glam(aim_origin),
        vec3d_from_glam(aim_direction.normalize_or_zero()),
        target_yaw_degrees,
    ))
}

pub(crate) fn teleport_intent_query_matches(left: TeleportIntent, right: TeleportIntent) -> bool {
    left.start_feet == right.start_feet
        && left.aim_origin == right.aim_origin
        && left.aim_direction == right.aim_direction
}

pub(crate) fn xr_headset_player_yaw_degrees_from_views(
    views: &[xr::View],
    transform: XrStageToWorld,
) -> Result<f64> {
    xr_headset_world_yaw_from_views(views, transform).map(|yaw| -f64::from(yaw).to_degrees())
}

pub(crate) fn xr_blink_teleport_target_yaw_degrees_from_stick(
    base_yaw_degrees: f64,
    previous_target_yaw_degrees: f64,
    activation_angle_radians: &mut Option<f64>,
    axis: Vec2,
) -> f64 {
    if !axis.is_finite() || axis.length() <= XR_BLINK_TELEPORT_HEADING_STICK_THRESHOLD {
        return previous_target_yaw_degrees;
    }
    let current_angle_radians = f64::from(axis.y.atan2(axis.x));
    let start_angle_radians = *activation_angle_radians.get_or_insert(current_angle_radians);
    target_yaw_degrees_from_stick_delta(
        base_yaw_degrees,
        start_angle_radians,
        current_angle_radians,
    )
}

pub(crate) fn target_yaw_degrees_from_stick_delta(
    base_yaw_degrees: f64,
    start_angle_radians: f64,
    current_angle_radians: f64,
) -> f64 {
    let delta_degrees =
        normalize_radians_180(current_angle_radians - start_angle_radians).to_degrees();
    normalize_degrees_180(base_yaw_degrees - delta_degrees)
}

pub(crate) fn xr_blink_teleport_landing_yaw_radians(
    current_root_yaw_radians: f64,
    views: &[xr::View],
    transform: XrStageToWorld,
    target_yaw_degrees: f64,
) -> Result<f64> {
    if !current_root_yaw_radians.is_finite() || !target_yaw_degrees.is_finite() {
        bail!("invalid XR Blink landing yaw input");
    }
    let current_headset_yaw_radians = f64::from(xr_headset_world_yaw_from_views(views, transform)?);
    let target_headset_yaw_radians = -target_yaw_degrees.to_radians();
    let yaw_delta = normalize_radians_180(target_headset_yaw_radians - current_headset_yaw_radians);
    Ok(normalize_radians_180(current_root_yaw_radians + yaw_delta))
}

pub(crate) fn normalize_radians_180(radians: f64) -> f64 {
    if !radians.is_finite() {
        return 0.0;
    }
    (radians + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

pub(crate) fn normalize_degrees_180(degrees: f64) -> f64 {
    if !degrees.is_finite() {
        return 0.0;
    }
    (degrees + 180.0).rem_euclid(360.0) - 180.0
}

pub(crate) fn xr_blink_teleport_lines(preview: &TeleportPreview) -> Vec<WorldGuiLine> {
    let mut lines = Vec::new();
    let arc_color = if preview.is_valid() {
        XR_BLINK_TELEPORT_VALID_ARC_COLOR
    } else {
        XR_BLINK_TELEPORT_INVALID_ARC_COLOR
    };
    for points in preview.arc_points.windows(2) {
        lines.push(WorldGuiLine::new(
            glam_vec3_from_vec3d(points[0]),
            glam_vec3_from_vec3d(points[1]),
            arc_color,
        ));
    }
    if let Some(feet) = preview.target_feet {
        push_xr_blink_teleport_cross(
            &mut lines,
            feet,
            XR_BLINK_TELEPORT_MARKER_RADIUS,
            XR_BLINK_TELEPORT_FEET_COLOR,
        );
        push_xr_blink_teleport_heading_arrow(
            &mut lines,
            feet,
            preview.target_yaw_degrees,
            XR_BLINK_TELEPORT_HEADING_COLOR,
        );
    }
    if let (Some(feet), Some(dot)) = (preview.target_feet, preview.marker_dot) {
        lines.push(WorldGuiLine::new(
            glam_vec3_from_vec3d(feet),
            glam_vec3_from_vec3d(dot),
            XR_BLINK_TELEPORT_DOT_COLOR,
        ));
        push_xr_blink_teleport_cross(
            &mut lines,
            dot,
            XR_BLINK_TELEPORT_DOT_RADIUS,
            XR_BLINK_TELEPORT_DOT_COLOR,
        );
    }
    lines
}

pub(crate) fn push_xr_blink_teleport_cross(
    lines: &mut Vec<WorldGuiLine>,
    center: Vec3d,
    radius: f64,
    color: [f32; 4],
) {
    lines.push(WorldGuiLine::new(
        glam_vec3_from_vec3d(center.add(Vec3d::new(-radius, 0.0, 0.0))),
        glam_vec3_from_vec3d(center.add(Vec3d::new(radius, 0.0, 0.0))),
        color,
    ));
    lines.push(WorldGuiLine::new(
        glam_vec3_from_vec3d(center.add(Vec3d::new(0.0, 0.0, -radius))),
        glam_vec3_from_vec3d(center.add(Vec3d::new(0.0, 0.0, radius))),
        color,
    ));
}

pub(crate) fn push_xr_blink_teleport_heading_arrow(
    lines: &mut Vec<WorldGuiLine>,
    feet: Vec3d,
    yaw_degrees: f64,
    color: [f32; 4],
) {
    let forward = horizontal_forward_from_player_yaw_degrees(yaw_degrees);
    if forward == Vec3d::ZERO {
        return;
    }
    let start = feet.add(Vec3d::new(0.0, 0.05, 0.0));
    let end = start.add(forward.scale(XR_BLINK_TELEPORT_HEADING_ARROW_LENGTH));
    lines.push(WorldGuiLine::new(
        glam_vec3_from_vec3d(start),
        glam_vec3_from_vec3d(end),
        color,
    ));

    let right = Vec3d::new(forward.z, 0.0, -forward.x);
    let head_angle = XR_BLINK_TELEPORT_HEADING_ARROW_HEAD_ANGLE_DEGREES.to_radians();
    let back = forward.scale(-head_angle.cos() * XR_BLINK_TELEPORT_HEADING_ARROW_HEAD_LENGTH);
    let side = right.scale(head_angle.sin() * XR_BLINK_TELEPORT_HEADING_ARROW_HEAD_LENGTH);
    for head in [back.add(side), back.subtract(side)] {
        lines.push(WorldGuiLine::new(
            glam_vec3_from_vec3d(end),
            glam_vec3_from_vec3d(end.add(head)),
            color,
        ));
    }
}

pub(crate) fn horizontal_forward_from_player_yaw_degrees(yaw_degrees: f64) -> Vec3d {
    if !yaw_degrees.is_finite() {
        return Vec3d::ZERO;
    }
    let forward = view_vector_from_rot_degrees(0.0, yaw_degrees);
    Vec3d::new(forward.x, 0.0, forward.z)
}

impl<S> XrMcloneTerrainState<S>
where
    S: RemoteDedicatedServerSession,
{
    pub(crate) fn update_xr_blink_teleport(
        &mut self,
        controllers: &[XrControllerSnapshot],
        views: &[xr::View],
        transform: XrStageToWorld,
    ) -> Result<XrBlinkTeleportFrame> {
        if let Some(frame) = xr_blink_teleport_disabled_frame(self.travel_assist_mode, controllers)
        {
            self.clear_xr_blink_teleport();
            return Ok(frame);
        }

        let left_axis = xr_left_stick_raw_axis(controllers);
        let left_axis_active = left_axis.length() > XR_JOYPAD_DEAD_ZONE;
        let blink_engaged = xr_left_stick_blink_engaged(controllers);
        if blink_engaged {
            if !self.blink_teleport.active {
                self.begin_xr_blink_teleport(views, transform);
            }
            self.submit_xr_blink_teleport_request(controllers, transform)?;
            self.poll_xr_blink_teleport_worker();
            return Ok(XrBlinkTeleportFrame {
                suppress_left_stick_movement: true,
            });
        }

        if self.blink_teleport.active {
            self.commit_xr_blink_teleport(views, transform)?;
            return Ok(XrBlinkTeleportFrame {
                suppress_left_stick_movement: true,
            });
        }

        Ok(XrBlinkTeleportFrame {
            suppress_left_stick_movement: left_axis_active,
        })
    }

    pub(crate) fn begin_xr_blink_teleport(
        &mut self,
        views: &[xr::View],
        transform: XrStageToWorld,
    ) {
        if self.runtime.is_none() {
            self.clear_xr_blink_teleport();
            return;
        }
        let base_yaw_degrees = match xr_headset_player_yaw_degrees_from_views(views, transform) {
            Ok(yaw_degrees) => yaw_degrees,
            Err(error) => {
                log::warn!(
                    "failed to capture XR Blink headset heading, falling back to body yaw: {error:#}"
                );
                self.camera.player().pose().y_rot_degrees
            }
        };
        self.ensure_xr_blink_teleport_worker();
        self.blink_teleport.active = true;
        self.blink_teleport.preview = None;
        self.blink_teleport.first_request_id = None;
        self.blink_teleport.preview_request_id = None;
        self.blink_teleport.base_yaw_degrees = base_yaw_degrees;
        self.blink_teleport.target_yaw_degrees = base_yaw_degrees;
        self.blink_teleport.activation_left_stick_angle_radians = None;
        self.blink_teleport.last_submitted_intent = None;
        log::info!("XR Blink teleport preview armed");
    }

    pub(crate) fn clear_xr_blink_teleport(&mut self) {
        self.blink_teleport = XrBlinkTeleportState::default();
    }

    pub(crate) fn ensure_xr_blink_teleport_worker(&mut self) {
        if self.blink_teleport_worker.is_some() {
            return;
        }
        match NativeTeleportPreviewWorker::new() {
            Ok(worker) => {
                self.blink_teleport_worker = Some(worker);
            }
            Err(error) => {
                log::warn!("failed to start XR Blink teleport worker: {error}");
            }
        }
    }

    pub(crate) fn submit_xr_blink_teleport_request(
        &mut self,
        controllers: &[XrControllerSnapshot],
        transform: XrStageToWorld,
    ) -> Result<bool> {
        let target_yaw_degrees = self.xr_blink_teleport_target_yaw_degrees(controllers);
        self.blink_teleport.target_yaw_degrees = target_yaw_degrees;
        if let Some(preview) = self.blink_teleport.preview.as_mut() {
            preview.target_yaw_degrees = target_yaw_degrees;
        }
        let Some(intent) =
            xr_blink_teleport_intent(&self.camera, controllers, transform, target_yaw_degrees)
        else {
            return Ok(false);
        };
        if self
            .blink_teleport
            .last_submitted_intent
            .is_some_and(|last| teleport_intent_query_matches(last, intent))
        {
            return Ok(false);
        }
        self.ensure_xr_blink_teleport_worker();
        let Some(runtime) = self.runtime.as_ref() else {
            return Ok(false);
        };
        let Some(worker) = self.blink_teleport_worker.as_mut() else {
            return Ok(false);
        };
        match worker.submit_from_world(runtime.client(), intent, xr_blink_teleport_config()) {
            Ok(request_id) => {
                self.blink_teleport
                    .first_request_id
                    .get_or_insert(request_id);
                self.blink_teleport.last_submitted_intent = Some(intent);
                Ok(true)
            }
            Err(error) => {
                log::warn!("XR Blink teleport preview submit failed: {error}");
                self.blink_teleport_worker = None;
                Ok(false)
            }
        }
    }

    pub(crate) fn poll_xr_blink_teleport_worker(&mut self) -> bool {
        let Some(worker) = self.blink_teleport_worker.as_mut() else {
            return false;
        };
        match worker.try_recv_latest() {
            Ok(Some(result)) => self.accept_xr_blink_teleport_result(result),
            Ok(None) => false,
            Err(error) => {
                log::warn!("XR Blink teleport worker failed: {error}");
                self.blink_teleport_worker = None;
                false
            }
        }
    }

    pub(crate) fn accept_xr_blink_teleport_result(
        &mut self,
        result: TeleportPreviewResult,
    ) -> bool {
        if !self.blink_teleport.active {
            return false;
        }
        if self
            .blink_teleport
            .first_request_id
            .is_some_and(|first_request_id| result.id < first_request_id)
        {
            return false;
        }
        if self
            .blink_teleport
            .preview_request_id
            .is_some_and(|preview_request_id| result.id <= preview_request_id)
        {
            return false;
        }
        let mut preview = result.preview;
        preview.target_yaw_degrees = self.blink_teleport.target_yaw_degrees;
        self.blink_teleport.preview = Some(preview);
        self.blink_teleport.preview_request_id = Some(result.id);
        true
    }

    pub(crate) fn commit_xr_blink_teleport(
        &mut self,
        views: &[xr::View],
        transform: XrStageToWorld,
    ) -> Result<()> {
        self.poll_xr_blink_teleport_worker();
        let preview = self.blink_teleport.preview.take();
        self.clear_xr_blink_teleport();
        let Some(preview) = preview else {
            log::info!("XR Blink teleport release had no completed preview");
            return Ok(());
        };
        let Some(target_feet) = preview.target_feet.filter(|_| preview.is_valid()) else {
            log::info!(
                "XR Blink teleport release had no valid preview: {:?}",
                preview.validity
            );
            return Ok(());
        };

        let snapshot = self.camera.snapshot();
        let landing_yaw_radians = match xr_blink_teleport_landing_yaw_radians(
            snapshot.yaw_radians,
            views,
            transform,
            preview.target_yaw_degrees,
        ) {
            Ok(yaw_radians) => yaw_radians,
            Err(error) => {
                log::warn!(
                    "failed to solve XR Blink landing yaw from headset pose, falling back to body yaw: {error:#}"
                );
                -preview.target_yaw_degrees.to_radians()
            }
        };
        self.camera
            .set_player_feet_pose(target_feet, landing_yaw_radians, snapshot.pitch_radians);
        if let Some(runtime) = self.runtime.as_ref() {
            self.camera
                .probe_ground(runtime.client(), XR_BLINK_TELEPORT_GROUND_PROBE_DISTANCE);
        }
        log::info!(
            "XR Blink teleport committed feet=({:.2}, {:.2}, {:.2})",
            target_feet.x,
            target_feet.y,
            target_feet.z
        );
        Ok(())
    }

    pub(crate) fn xr_blink_teleport_target_yaw_degrees(
        &mut self,
        controllers: &[XrControllerSnapshot],
    ) -> f64 {
        let base_yaw_degrees = self.blink_teleport.base_yaw_degrees;
        let previous_target_yaw_degrees = self.blink_teleport.target_yaw_degrees;
        xr_blink_teleport_target_yaw_degrees_from_stick(
            base_yaw_degrees,
            previous_target_yaw_degrees,
            &mut self.blink_teleport.activation_left_stick_angle_radians,
            xr_left_stick_raw_axis(controllers),
        )
    }

    pub(crate) fn xr_blink_teleport_lines(&self) -> Vec<WorldGuiLine> {
        let Some(preview) = self.blink_teleport.preview.as_ref() else {
            return Vec::new();
        };
        xr_blink_teleport_lines(preview)
    }
}
