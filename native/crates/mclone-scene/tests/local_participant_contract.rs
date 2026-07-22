use std::path::{Path, PathBuf};

fn scene_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(scene_root().join(relative)).expect("source file readable")
}

#[test]
fn participant_identity_stays_distinct_from_neighboring_identities() {
    let participant = read("../mclone-app-runtime/src/local_participant.rs");
    assert!(participant.contains("pub struct LocalParticipantId(NonZeroU64)"));
    assert!(!participant.contains("PlayerProfileId"));
    assert!(!participant.contains("ServerPlayerId"));
    assert!(!participant.contains("PresentationViewIndex"));
    assert!(!participant.contains("pub struct LocalParticipantId(pub"));

    let input = read("../mclone-input/src/lib.rs");
    assert!(!input.contains("LocalParticipantId"));
}

#[test]
fn physical_collectors_do_not_own_participant_or_join_policy() {
    for source in [
        read("../../apps/mclone-native-client/src/desktop_gamepad.rs"),
        read("../../apps/mclone-web-client/src/web_gamepad.rs"),
        read("../mclone-android-platform/src/android_controller.rs"),
    ] {
        assert!(!source.contains("LocalParticipantGroup"));
        assert!(!source.contains("LocalGamepadAssignmentReducer"));
        assert!(!source.contains("SourceJoinOutcome"));
    }
}
