use super::*;

#[test]
fn cli_parses_actor_review_sheet_options() {
    let cli = Cli::parse([
        "--actor-review-sheet".to_owned(),
        "/tmp/mclone-actor-review.png".to_owned(),
        "--width".to_owned(),
        "900".to_owned(),
        "--height".to_owned(),
        "420".to_owned(),
        "--fullbright".to_owned(),
        "false".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::HeadlessActorReviewSheet {
            options: HeadlessActorReviewSheetOptions {
                path: PathBuf::from("/tmp/mclone-actor-review.png"),
                width: 900,
                height: 420,
                render_options: TexturedSectionRenderOptions {
                    force_fullbright: false,
                    ..TexturedSectionRenderOptions::default()
                },
            },
        }
    );
}

#[test]
fn cli_parses_actor_walk_review_options() {
    let cli = Cli::parse([
        "--actor-walk-review".to_owned(),
        "/tmp/mclone-actor-walk-review.png".to_owned(),
        "--actor-walk-review-video".to_owned(),
        "/tmp/mclone-actor-walk-review.mp4".to_owned(),
        "--width".to_owned(),
        "320".to_owned(),
        "--height".to_owned(),
        "240".to_owned(),
        "--walk-review-frames".to_owned(),
        "16".to_owned(),
        "--walk-review-fps".to_owned(),
        "8".to_owned(),
        "--walk-review-cycles".to_owned(),
        "3.5".to_owned(),
        "--fullbright".to_owned(),
        "false".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli,
        Cli::HeadlessActorWalkReview {
            options: HeadlessActorWalkReviewOptions {
                sheet_path: PathBuf::from("/tmp/mclone-actor-walk-review.png"),
                video_path: Some(PathBuf::from("/tmp/mclone-actor-walk-review.mp4")),
                width: 320,
                height: 240,
                frames: 16,
                fps: 8,
                cycles: 3.5,
                render_options: TexturedSectionRenderOptions {
                    force_fullbright: false,
                    ..TexturedSectionRenderOptions::default()
                },
            },
        }
    );
}

#[test]
fn cli_rejects_actor_walk_review_video_without_sheet() {
    let error = Cli::parse([
        "--actor-walk-review-video".to_owned(),
        "/tmp/mclone-actor-walk-review.mp4".to_owned(),
    ])
    .unwrap_err()
    .to_string();

    assert!(error.contains("actor walk review options require --actor-walk-review"));
}
