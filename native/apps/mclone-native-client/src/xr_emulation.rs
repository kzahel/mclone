use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use glam::{Quat, Vec3};
use mclone_input::{FlatInputFrame, KeyboardMouseInputAdapter};
use mclone_render::headless::{HeadlessStereoFrameOptions, write_headless_stereo_frame_png};
use mclone_render_session::{EngineCameraSnapshot, XrFov, XrView, XrViewPose};

use crate::cli::XrEmulationScreenshotOptions;
use crate::offscreen_scene_host::OffscreenDriver;
use crate::render_cache::load_asset_source;
use crate::scene_runtime::WindowSceneAssets;

const XR_EMULATION_IPD_BLOCKS: f32 = 0.064;
const XR_EMULATION_VERTICAL_FOV_RADIANS: f32 = std::f32::consts::FRAC_PI_2;
const XR_EMULATION_FRAME_TIME: Duration = Duration::from_micros(16_667);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct XrEmulationScreenshotReport {
    pub(crate) path: PathBuf,
    pub(crate) eye_width: u32,
    pub(crate) eye_height: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) byte_len: usize,
    pub(crate) eye_pixel_difference_count: usize,
    pub(crate) section_count: usize,
    pub(crate) drawn_section_count: usize,
    pub(crate) gui_command_count: usize,
    pub(crate) ui_panel_composite_count: u64,
}

pub(crate) fn run_xr_emulation_screenshot(
    options: &XrEmulationScreenshotOptions,
) -> Result<XrEmulationScreenshotReport> {
    let emulated_input = held_xr_emulation_input(&options.held_keys)?;
    let assets = WindowSceneAssets::load()?;
    let asset_source = load_asset_source()?;
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let input_frames = options.input_frames;
    let (capture, summary) = write_headless_stereo_frame_png(
        HeadlessStereoFrameOptions {
            path: options.path.clone(),
            eye_width: options.eye_width,
            eye_height: options.eye_height,
        },
        move |device, queue, format, size, left_view, right_view| {
            let mut driver = OffscreenDriver::new_stereo_emulation(
                device,
                queue,
                format,
                size,
                &scene,
                render_options,
                &assets,
                &asset_source,
            )?;
            let mut views = synthetic_stereo_views(driver.host().camera_snapshot(), size);
            driver.drive_stereo_until_streamed(device, queue, views)?;

            if let Some(frame) = emulated_input {
                // If startup leaves an XR screen open, close it through the
                // same controller edge used on headset before exercising held
                // locomotion input. The final block below opens the pause panel
                // so every capture proves world-quad composition.
                if driver.stereo_ui_is_active() {
                    apply_menu_toggle(&mut driver, views)?;
                }
                for _ in 0..input_frames {
                    std::thread::sleep(XR_EMULATION_FRAME_TIME);
                    views = synthetic_stereo_views(driver.host().camera_snapshot(), size);
                    driver.apply_stereo_input_frame(frame, views)?;
                    driver.render_stereo(device, queue, views, left_view, right_view)?;
                }
                views = synthetic_stereo_views(driver.host().camera_snapshot(), size);
            }
            if !driver.stereo_ui_is_active() {
                apply_menu_toggle(&mut driver, views)?;
            }

            views = synthetic_stereo_views(driver.host().camera_snapshot(), size);
            driver.apply_stereo_input_frame(FlatInputFrame::default(), views)?;
            driver.render_stereo(device, queue, views, left_view, right_view)
        },
    )?;

    if capture.non_clear_rgb_pixel_count == 0 || summary.drawn_section_count == 0 {
        bail!("XR emulation capture rendered no world pixels");
    }
    if capture.eye_pixel_difference_count == 0 {
        bail!("XR emulation eyes are pixel-identical; stereo parallax was not preserved");
    }
    if !summary.ui_active || summary.gui_command_count == 0 || summary.ui_panel.composite_count < 2
    {
        bail!(
            "XR emulation capture did not composite the active world UI into both eyes: active={} commands={} composites={}",
            summary.ui_active,
            summary.gui_command_count,
            summary.ui_panel.composite_count
        );
    }

    Ok(XrEmulationScreenshotReport {
        path: capture.path,
        eye_width: capture.eye_width,
        eye_height: capture.eye_height,
        width: capture.width,
        height: capture.height,
        byte_len: capture.byte_len,
        eye_pixel_difference_count: capture.eye_pixel_difference_count,
        section_count: summary.section_count,
        drawn_section_count: summary.drawn_section_count,
        gui_command_count: summary.gui_command_count,
        ui_panel_composite_count: summary.ui_panel.composite_count,
    })
}

fn held_xr_emulation_input(keys: &[mclone_input::KeyboardKey]) -> Result<Option<FlatInputFrame>> {
    if keys.is_empty() {
        return Ok(None);
    }
    let mut keyboard = KeyboardMouseInputAdapter::new();
    for &key in keys {
        let event = keyboard.handle_key(key, true, false);
        if !event.handled || event.frame.is_some() {
            bail!(
                "--xr-emulation-key `{}` is not a held locomotion binding; use WASD, arrows, Space, KeyX, Shift, or Control",
                key.label()
            );
        }
    }
    keyboard
        .held_frame()
        .map(Some)
        .context("--xr-emulation-key values did not produce held locomotion input")
}

fn apply_menu_toggle(driver: &mut OffscreenDriver, views: [XrView; 2]) -> Result<()> {
    driver.apply_stereo_input_frame(
        FlatInputFrame {
            open_menu: true,
            ..FlatInputFrame::default()
        },
        views,
    )?;
    driver.apply_stereo_input_frame(FlatInputFrame::default(), views)
}

fn synthetic_stereo_views(snapshot: EngineCameraSnapshot, size: [u32; 2]) -> [XrView; 2] {
    // Stage-space head translation stays fixed. The shared scene transform maps
    // it onto the engine camera/player root, including locomotion and snap-turn.
    // Pitch is the one head-relative component not carried by that yaw/root
    // transform, so project it into the synthetic pose directly.
    let pitch = if snapshot.pitch_radians.is_finite() {
        snapshot.pitch_radians as f32
    } else {
        0.0
    };
    let orientation = Quat::from_rotation_x(pitch);
    let half_ipd = orientation * Vec3::X * (XR_EMULATION_IPD_BLOCKS * 0.5);
    let fov = symmetric_fov(size);
    [
        XrView {
            pose: XrViewPose {
                position: -half_ipd,
                orientation,
            },
            fov,
        },
        XrView {
            pose: XrViewPose {
                position: half_ipd,
                orientation,
            },
            fov,
        },
    ]
}

fn symmetric_fov(size: [u32; 2]) -> XrFov {
    let aspect = size[0].max(1) as f32 / size[1].max(1) as f32;
    let tan_y = (XR_EMULATION_VERTICAL_FOV_RADIANS * 0.5).tan();
    let angle_y = tan_y.atan();
    let angle_x = (tan_y * aspect).atan();
    XrFov {
        angle_left: -angle_x,
        angle_right: angle_x,
        angle_up: angle_y,
        angle_down: -angle_y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{ChunkPos, Vec3d};

    #[test]
    fn synthetic_views_use_fixed_ipd_and_symmetric_fov() {
        let views = synthetic_stereo_views(test_snapshot(), [1200, 1000]);

        assert!(
            ((views[1].pose.position - views[0].pose.position).length() - XR_EMULATION_IPD_BLOCKS)
                .abs()
                < 1.0e-6
        );
        assert_eq!(views[0].fov, views[1].fov);
        assert_eq!(views[0].fov.angle_left, -views[0].fov.angle_right);
        assert_eq!(views[0].fov.angle_down, -views[0].fov.angle_up);
        assert!(views[0].fov.angle_right > views[0].fov.angle_up);
    }

    #[test]
    fn scene_tracking_maps_synthetic_head_to_engine_camera() {
        let snapshot = test_snapshot();
        let views = synthetic_stereo_views(snapshot, [1000, 1000]);
        let origin = mclone_scene::XrTrackingOrigin::from_initial_views(
            &views,
            mclone_scene::XrViewAlignmentMode::PlayerSpawn,
        )
        .unwrap();
        let transform =
            mclone_scene::XrStageToWorld::from_tracking_origin(origin, snapshot).unwrap();
        let left = transform.transform_position(views[0].pose.position);
        let right = transform.transform_position(views[1].pose.position);
        let center = (left + right) * 0.5;
        let expected_center = Vec3::new(
            snapshot.eye.x as f32,
            snapshot.eye.y as f32,
            snapshot.eye.z as f32,
        );
        assert!((center - expected_center).length() < 1.0e-5);
        let expected_eye_axis = Quat::from_rotation_y(snapshot.yaw_radians as f32) * Vec3::X;
        assert!(
            ((right - left).normalize() - expected_eye_axis).length() < 1.0e-5,
            "engine yaw should rotate the synthetic eye axis"
        );
    }

    #[test]
    fn emulation_keys_accept_only_held_locomotion_bindings() {
        let frame = held_xr_emulation_input(&[
            mclone_input::KeyboardKey::KeyW,
            mclone_input::KeyboardKey::ArrowLeft,
        ])
        .unwrap()
        .expect("held frame");
        assert!(frame.forward);
        assert_eq!(frame.keyboard_turn, 1.0);

        let error = held_xr_emulation_input(&[mclone_input::KeyboardKey::Escape]).unwrap_err();
        assert!(error.to_string().contains("not a held locomotion binding"));
    }

    fn test_snapshot() -> EngineCameraSnapshot {
        EngineCameraSnapshot {
            eye: Vec3d::new(12.0, 64.0, -7.0),
            yaw_radians: 1.2,
            pitch_radians: 0.25,
            speed_blocks_per_second: 4.0,
            chunk_pos: ChunkPos::new(0, 0),
        }
    }
}
