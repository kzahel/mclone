use super::*;

#[derive(Clone, Copy, Debug)]
pub struct XrTrackingOrigin {
    pub(super) origin_stage: Vec3,
    pub(super) stage_yaw: f32,
    pub(super) mode: XrViewAlignmentMode,
}

#[derive(Clone, Copy, Debug)]
pub struct XrStageToWorld {
    pub(super) origin_stage: Vec3,
    pub(super) origin_world: Vec3,
    pub(super) stage_to_world_rotation: Quat,
}

impl XrTrackingOrigin {
    pub fn from_initial_views(views: &[XrView], mode: XrViewAlignmentMode) -> Result<Self> {
        let left_stage_pose = views[0].pose;
        let right_stage_pose = views[1].pose;
        let origin_stage = (left_stage_pose.position + right_stage_pose.position) * 0.5;
        Self::from_stage_view(origin_stage, left_stage_pose.orientation, mode)
    }

    pub fn from_stage_view(
        origin_stage: Vec3,
        left_stage_orientation: Quat,
        mode: XrViewAlignmentMode,
    ) -> Result<Self> {
        let stage_forward = left_stage_orientation * Vec3::NEG_Z;
        let stage_yaw = yaw_from_forward(stage_forward)
            .ok_or_else(|| anyhow!("OpenXR returned an invalid tracking-origin yaw"))?;
        Ok(Self {
            origin_stage,
            stage_yaw,
            mode,
        })
    }

    pub fn mode_label(self) -> &'static str {
        self.mode.label()
    }

    pub fn origin_stage(self) -> Vec3 {
        self.origin_stage
    }

    pub fn stage_yaw(self) -> f32 {
        self.stage_yaw
    }

    pub fn consume_world_movement(self, world_movement: Vec3, transform: XrStageToWorld) -> Self {
        if !world_movement.is_finite() || world_movement.length_squared() <= f32::EPSILON {
            return self;
        }
        let stage_movement = transform.stage_direction_from_world(world_movement);
        if !stage_movement.is_finite() {
            return self;
        }
        Self {
            origin_stage: self.origin_stage + stage_movement,
            ..self
        }
    }

    pub fn rebase_for_stage_position_world_position(
        self,
        stage_position: Vec3,
        world_position: Vec3,
        snapshot: EngineCameraSnapshot,
    ) -> Result<Self> {
        if !stage_position.is_finite() || !world_position.is_finite() {
            bail!("invalid XR tracking-origin rebase position");
        }
        let transform = XrStageToWorld::from_tracking_origin(self, snapshot)?;
        let origin_world = glam_vec3_from_vec3d(snapshot.eye);
        let stage_offset = transform.stage_direction_from_world(world_position - origin_world);
        if !stage_offset.is_finite() {
            bail!("invalid XR tracking-origin rebase offset");
        }
        Ok(Self {
            origin_stage: stage_position - stage_offset,
            ..self
        })
    }
}

impl XrStageToWorld {
    pub fn from_tracking_origin(
        origin: XrTrackingOrigin,
        snapshot: EngineCameraSnapshot,
    ) -> Result<Self> {
        let world_yaw = snapshot.yaw_radians as f32;
        if !world_yaw.is_finite() {
            bail!("invalid XR player root yaw {}", snapshot.yaw_radians);
        }
        Ok(Self {
            origin_stage: origin.origin_stage,
            origin_world: glam_vec3_from_vec3d(snapshot.eye),
            stage_to_world_rotation: Quat::from_rotation_y(normalize_angle(
                world_yaw - origin.stage_yaw,
            )),
        })
    }

    pub fn transform_pose(self, stage_position: Vec3, stage_orientation: Quat) -> (Vec3, Quat) {
        (
            self.origin_world
                + self
                    .stage_to_world_rotation
                    .mul_vec3(stage_position - self.origin_stage),
            (self.stage_to_world_rotation * stage_orientation).normalize(),
        )
    }

    pub fn transform_position(self, stage_position: Vec3) -> Vec3 {
        self.origin_world
            + self
                .stage_to_world_rotation
                .mul_vec3(stage_position - self.origin_stage)
    }

    pub fn transform_direction(self, stage_direction: Vec3) -> Vec3 {
        self.stage_to_world_rotation
            .mul_vec3(stage_direction)
            .normalize_or_zero()
    }

    pub fn stage_direction_from_world(self, world_direction: Vec3) -> Vec3 {
        self.stage_to_world_rotation
            .inverse()
            .mul_vec3(world_direction)
    }
}

pub fn xr_headset_stage_position_from_views(views: &[XrView]) -> Result<Vec3> {
    if views.len() < 2 {
        bail!("OpenXR runtime returned fewer than two stereo views");
    }
    let left_pose = views[0].pose;
    let right_pose = views[1].pose;
    Ok((left_pose.position + right_pose.position) * 0.5)
}

pub fn xr_view_to_chunk_render_view(
    view: &XrView,
    transform: XrStageToWorld,
    near: f32,
    far: f32,
) -> Result<ChunkRenderView> {
    let stage_pose = view.pose;
    let (camera_position, camera_orientation) =
        transform.transform_pose(stage_pose.position, stage_pose.orientation);
    let render_view = render_view_from_world_pose(
        XrViewPose {
            position: camera_position,
            orientation: camera_orientation,
        },
        view.fov,
        near,
        far,
    )?;
    Ok(chunk_render_view_from_xr_render_view(render_view))
}

pub fn fixed_startup_view_pose_render_views(
    view_pose: XrStartupViewPose,
    eye_fovs: [XrFov; 2],
    far: f32,
) -> Result<[ChunkRenderView; 2]> {
    fixed_startup_view_pose_render_views_with_far(view_pose, eye_fovs, far)
}

pub fn fixed_startup_view_pose_render_views_with_far(
    view_pose: XrStartupViewPose,
    eye_fovs: [XrFov; 2],
    far: f32,
) -> Result<[ChunkRenderView; 2]> {
    let yaw_radians = view_pose.yaw_degrees.to_radians();
    if !yaw_radians.is_finite() {
        bail!("invalid XR fixed render view yaw {}", view_pose.yaw_degrees);
    }
    let center = Vec3::from_array(view_pose.position);
    if !center.is_finite() {
        bail!(
            "invalid XR fixed render view position {:?}",
            view_pose.position
        );
    }
    let orientation = Quat::from_rotation_y(yaw_radians);
    let eye_right = orientation * Vec3::X;
    let half_eye_offset = eye_right * (XR_FIXED_RENDER_EYE_SEPARATION_BLOCKS * 0.5);
    let left = fixed_startup_view_pose_render_view(
        center - half_eye_offset,
        orientation,
        eye_fovs[0],
        far,
    )?;
    let right = fixed_startup_view_pose_render_view(
        center + half_eye_offset,
        orientation,
        eye_fovs[1],
        far,
    )?;
    Ok([left, right])
}

pub(crate) fn fixed_startup_view_pose_render_view(
    position: Vec3,
    orientation: Quat,
    fov: XrFov,
    far: f32,
) -> Result<ChunkRenderView> {
    let render_view = render_view_from_world_pose(
        XrViewPose {
            position,
            orientation,
        },
        fov,
        XR_NEAR,
        far,
    )?;
    Ok(chunk_render_view_from_xr_render_view(render_view))
}

pub fn chunk_render_view_from_xr_render_view(view: XrRenderView) -> ChunkRenderView {
    ChunkRenderView {
        view: view.view,
        projection: view.projection,
        view_projection: view.view_projection,
        camera_position: view.camera_position,
        camera_forward: view.camera_forward,
        camera_right: view.camera_right,
        camera_up: view.camera_up,
        aspect: view.aspect,
        fov_y_radians: view.fov_y_radians,
        z_near: view.z_near,
        z_far: view.z_far,
        projection_kind: ChunkProjectionKind::External,
    }
}

pub fn yaw_from_forward(forward: Vec3) -> Option<f32> {
    if !forward.is_finite() {
        return None;
    }
    let horizontal = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    if horizontal.length_squared() <= f32::EPSILON {
        return None;
    }
    Some((-horizontal.x).atan2(-horizontal.z))
}

pub fn normalize_angle(angle: f32) -> f32 {
    if !angle.is_finite() {
        return 0.0;
    }
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

pub fn glam_vec3_from_vec3d(value: Vec3d) -> Vec3 {
    Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

pub fn vec3d_from_glam(value: Vec3) -> Vec3d {
    Vec3d::new(f64::from(value.x), f64::from(value.y), f64::from(value.z))
}

pub fn apply_xr_startup_view_pose(
    camera: &mut EngineCameraController,
    position: [f32; 3],
    yaw_degrees: f32,
) -> Result<()> {
    let yaw_radians = yaw_degrees.to_radians();
    if !yaw_radians.is_finite() {
        bail!("invalid XR startup view yaw {}", yaw_degrees);
    }
    camera.set_eye_pose(
        vec3d_from_glam(Vec3::from_array(position)),
        f64::from(yaw_radians),
        0.0,
    );
    Ok(())
}

pub(crate) fn average_unit_direction(a: Vec3, b: Vec3, fallback: Vec3) -> Vec3 {
    let direction = (a + b) * 0.5;
    if direction.is_finite() && direction.length_squared() > f32::EPSILON {
        direction.normalize()
    } else {
        fallback
    }
}

pub fn xr_headset_world_yaw_from_views(views: &[XrView], transform: XrStageToWorld) -> Result<f32> {
    if views.len() < 2 {
        bail!("OpenXR runtime returned fewer than two stereo views");
    }
    let left_pose = views[0].pose;
    let right_pose = views[1].pose;
    let left_forward = left_pose.orientation * Vec3::NEG_Z;
    let right_forward = right_pose.orientation * Vec3::NEG_Z;
    let stage_forward = average_unit_direction(left_forward, right_forward, Vec3::NEG_Z);
    let world_forward = transform.transform_direction(stage_forward);
    engine_movement_yaw_from_forward(world_forward)
        .ok_or_else(|| anyhow!("OpenXR returned an invalid headset locomotion yaw"))
}

impl McloneSceneHost {
    pub(crate) fn render_views(&mut self, views: &[XrView]) -> Result<[ChunkRenderView; 2]> {
        if views.len() < 2 {
            bail!("OpenXR runtime returned fewer than two stereo views");
        }
        let tracking_origin = self.tracking_origin_for_views(views)?;
        let transform = XrStageToWorld::from_tracking_origin(
            tracking_origin,
            self.active_world
                .local_participant
                .presentation_camera_snapshot(),
        )?;
        let far = self.terrain_projection_far_distance(XR_FAR);
        Ok([
            xr_view_to_chunk_render_view(&views[0], transform, XR_NEAR, far)?,
            xr_view_to_chunk_render_view(&views[1], transform, XR_NEAR, far)?,
        ])
    }

    pub(crate) fn tracking_origin_for_views(
        &mut self,
        views: &[XrView],
    ) -> Result<XrTrackingOrigin> {
        if let Some(origin) = self.tracking_origin {
            return Ok(origin);
        }
        let origin = XrTrackingOrigin::from_initial_views(views, self.initial_alignment_mode)?;
        let snapshot = self.active_world.camera.snapshot();
        log::info!(
            "mclone XR terrain player-root alignment: mode={} root_eye=({:.2}, {:.2}, {:.2}) root_yaw_degrees={:.1} stage_center=({:.3}, {:.3}, {:.3}) stage_yaw_degrees={:.1}",
            origin.mode_label(),
            snapshot.eye.x,
            snapshot.eye.y,
            snapshot.eye.z,
            snapshot.yaw_radians.to_degrees(),
            origin.origin_stage.x,
            origin.origin_stage.y,
            origin.origin_stage.z,
            origin.stage_yaw.to_degrees()
        );
        self.tracking_origin = Some(origin);
        Ok(origin)
    }

    pub(crate) fn reconcile_room_scale_body_to_headset(
        &mut self,
        views: &[XrView],
    ) -> Result<XrStageToWorld> {
        let origin = self.tracking_origin_for_views(views)?;
        let transform =
            XrStageToWorld::from_tracking_origin(origin, self.active_world.camera.snapshot())?;
        let headset_stage_position = xr_headset_stage_position_from_views(views)?;
        let headset_world_position = transform.transform_position(headset_stage_position);
        let Some(runtime) = self.active_world.runtime.as_ref() else {
            return Ok(transform);
        };
        let reconciliation = self
            .active_world
            .local_participant
            .camera
            .reconcile_room_scale_headset(
                runtime.client(),
                vec3d_from_glam(headset_world_position),
            );
        let consumed_world = glam_vec3_from_vec3d(reconciliation.consumed_body_movement);
        let origin = origin.consume_world_movement(consumed_world, transform);
        self.tracking_origin = Some(origin);
        XrStageToWorld::from_tracking_origin(origin, self.active_world.camera.snapshot())
    }
}
