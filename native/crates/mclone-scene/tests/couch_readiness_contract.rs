const MONO: &str = include_str!("../src/mono.rs");
const UNIFORM: &str = include_str!("../../mclone-render/src/uniform.rs");
const FRAME_RENDER: &str = include_str!("../../mclone-app-runtime/src/frame_render.rs");
const DESKTOP_DRIVER: &str =
    include_str!("../../../apps/mclone-native-client/src/winit_frame_driver.rs");

fn braced_item<'a>(source: &'a str, marker: &str) -> &'a str {
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("missing source marker `{marker}`"));
    let brace = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("missing opening brace after `{marker}`"));
    let mut depth = 0usize;
    for (offset, byte) in source[brace..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[start..=brace + offset];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated source item `{marker}`");
}

#[test]
fn flat_multi_view_frame_prepares_shared_work_once() {
    let function = braced_item(MONO, "fn render_flat_presentation_frame_inner(");
    assert_eq!(function.matches("live_upload_for_frame(").count(), 1);
    assert_eq!(function.matches("advance_local_startup(").count(), 1);
    assert_eq!(function.matches("sync_player_lifecycle_ui();").count(), 1);
    assert_eq!(function.matches("prepare_render_records();").count(), 1);
    assert_eq!(function.matches("render_mono_frame_inner(").count(), 1);
    assert!(function.contains("FrameActorPreparation::Refresh"));
    assert!(function.contains("FrameActorPreparation::ReusePrepared"));
    assert!(!function.contains("XrView"));
    assert!(!function.contains("LEFT_EYE"));
    assert!(!function.contains("RIGHT_EYE"));
}

#[test]
fn generic_view_identity_and_stereo_eye_identity_stay_separate() {
    assert!(UNIFORM.contains("pub struct PresentationViewIndex"));
    assert!(UNIFORM.contains("pub enum StereoEye"));
    assert!(UNIFORM.contains("MAX_PRESENTATION_VIEW_COUNT: u32 = 4"));
    assert!(!UNIFORM.contains("is_right_eye"));
    assert!(FRAME_RENDER.contains("FrameActorPreparation::ReusePrepared"));
}

#[test]
fn platform_driver_does_not_own_couch_policy() {
    for forbidden in [
        "LocalParticipant",
        "SplitScreenLayout",
        "ParticipantAssignment",
        "render_flat_presentation_frame",
    ] {
        assert!(
            !DESKTOP_DRIVER.contains(forbidden),
            "desktop platform driver must not own `{forbidden}`"
        );
    }
    assert!(MONO.contains("pub struct FlatPresentationFrameSummary"));
}
