//! Host-neutral per-view render pose contracts.
//!
//! OpenXR, synthetic desktop stereo, and future browser XR adapters all produce
//! these values. Keeping them below the scene host lets `mclone-scene` consume
//! mono/stereo views without depending on an OpenXR runtime crate.

use anyhow::{Result, bail};
use glam::{Mat4, Quat, Vec3, Vec4};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XrViewPose {
    pub position: Vec3,
    pub orientation: Quat,
}

/// Four tangent-space half angles in radians, matching OpenXR's FOV shape
/// without depending on OpenXR types.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XrFov {
    pub angle_left: f32,
    pub angle_right: f32,
    pub angle_up: f32,
    pub angle_down: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XrView {
    pub pose: XrViewPose,
    pub fov: XrFov,
}

#[derive(Clone, Copy, Debug)]
pub struct XrRenderView {
    pub view: Mat4,
    pub projection: Mat4,
    pub view_projection: Mat4,
    pub camera_position: Vec3,
    pub camera_forward: Vec3,
    pub camera_right: Vec3,
    pub camera_up: Vec3,
    pub aspect: f32,
    pub fov_y_radians: f32,
    pub z_near: f32,
    pub z_far: f32,
}

pub fn render_view_from_world_pose(
    pose: XrViewPose,
    fov: XrFov,
    near: f32,
    far: f32,
) -> Result<XrRenderView> {
    if !pose.position.is_finite()
        || !finite_quat(pose.orientation)
        || pose.orientation.length_squared() < 0.5
    {
        bail!("XR render view received an invalid world pose");
    }
    let orientation = pose.orientation.normalize();
    let camera_forward = (orientation * Vec3::NEG_Z).normalize_or_zero();
    let camera_right = (orientation * Vec3::X).normalize_or_zero();
    let camera_up = (orientation * Vec3::Y).normalize_or_zero();
    if camera_forward.length_squared() < 1.0e-6 || camera_up.length_squared() < 1.0e-6 {
        bail!("XR render view received an invalid view orientation");
    }
    let view = Mat4::from_rotation_translation(orientation, pose.position).inverse();
    let projection = xr_fov_to_projection_rh(fov, near, far)?;
    Ok(XrRenderView {
        view,
        projection,
        view_projection: projection * view,
        camera_position: pose.position,
        camera_forward,
        camera_right,
        camera_up,
        aspect: xr_fov_aspect(fov),
        fov_y_radians: (fov.angle_up - fov.angle_down).abs(),
        z_near: near,
        z_far: far,
    })
}

pub fn xr_fov_to_projection_rh(fov: XrFov, near: f32, far: f32) -> Result<Mat4> {
    if !near.is_finite() || !far.is_finite() || near <= 0.0 || far <= near {
        bail!("invalid XR projection clipping planes near={near} far={far}");
    }
    let tan_left = fov.angle_left.tan();
    let tan_right = fov.angle_right.tan();
    let tan_up = fov.angle_up.tan();
    let tan_down = fov.angle_down.tan();
    let tan_width = tan_right - tan_left;
    let tan_height = tan_up - tan_down;
    if !tan_width.is_finite()
        || !tan_height.is_finite()
        || tan_width.abs() <= f32::EPSILON
        || tan_height.abs() <= f32::EPSILON
    {
        bail!("invalid XR FOV {fov:?}");
    }
    // Reversed-Z depth mapping (near→1, far→0), matching the shared renderer.
    Ok(Mat4::from_cols(
        Vec4::new(2.0 / tan_width, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 2.0 / tan_height, 0.0, 0.0),
        Vec4::new(
            (tan_right + tan_left) / tan_width,
            (tan_up + tan_down) / tan_height,
            near / (far - near),
            -1.0,
        ),
        Vec4::new(0.0, 0.0, (near * far) / (far - near), 0.0),
    ))
}

pub fn xr_fov_aspect(fov: XrFov) -> f32 {
    let width = fov.angle_right.tan() - fov.angle_left.tan();
    let height = fov.angle_up.tan() - fov.angle_down.tan();
    if width.is_finite() && height.is_finite() && height.abs() > f32::EPSILON {
        (width / height).abs().max(0.01)
    } else {
        1.0
    }
}

fn finite_quat(value: Quat) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite() && value.w.is_finite()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_mat4_close(actual: Mat4, expected: Mat4) {
        for (actual, expected) in actual
            .to_cols_array()
            .into_iter()
            .zip(expected.to_cols_array())
        {
            assert!(
                (actual - expected).abs() < 1.0e-5,
                "matrix mismatch: actual={actual} expected={expected}"
            );
        }
    }

    #[test]
    fn symmetric_xr_fov_matches_chunk_projection_convention() {
        let fov_y = 58.0_f32.to_radians();
        let aspect = 1.25;
        let near = 0.05;
        let far = 700.0;
        let tan_y = (fov_y * 0.5).tan();
        let tan_x = tan_y * aspect;
        let fov = XrFov {
            angle_left: -tan_x.atan(),
            angle_right: tan_x.atan(),
            angle_up: tan_y.atan(),
            angle_down: -tan_y.atan(),
        };

        let actual = xr_fov_to_projection_rh(fov, near, far).unwrap();
        // Reversed-Z: the XR matrix maps near→1, far→0. Cross-check it against
        // the same reverse-Z remap (clip `z' = w - z`) applied to glam's standard
        // forward projection. See tactical 158.
        let reverse_z = Mat4::from_cols(
            Vec4::new(1.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 1.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, -1.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 1.0),
        );
        let expected = reverse_z * Mat4::perspective_rh(fov_y, aspect, near, far);

        assert_mat4_close(actual, expected);
    }

    #[test]
    fn render_view_from_world_pose_builds_camera_vectors() {
        let fov = XrFov {
            angle_left: -0.5,
            angle_right: 0.5,
            angle_up: 0.5,
            angle_down: -0.5,
        };
        let pose = XrViewPose {
            position: Vec3::new(4.0, 5.0, 6.0),
            orientation: Quat::IDENTITY,
        };

        let view = render_view_from_world_pose(pose, fov, 0.05, 700.0).unwrap();

        assert!((view.camera_position - pose.position).length() < 1.0e-6);
        assert!((view.camera_forward - Vec3::NEG_Z).length() < 1.0e-6);
        assert!((view.camera_right - Vec3::X).length() < 1.0e-6);
        assert!((view.camera_up - Vec3::Y).length() < 1.0e-6);
        assert_mat4_close(view.view_projection, view.projection * view.view);
    }
}
