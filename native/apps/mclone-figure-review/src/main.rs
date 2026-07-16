use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_assets::{
    AssetPath, FilesystemAssetSource, PreparedFigure, PreparedFigurePoseSample,
    default_player_figure_path, evaluate_prepared_figure_clip_into, load_prepared_figure,
};
use mclone_render::chunk::{ChunkCamera, ChunkDepthTarget};
use mclone_render::headless::{
    HeadlessFrameLoopOptions, HeadlessMultiviewFrameOptions, HeadlessStereoFrameOptions,
    run_headless_capture_loop, save_rgba_png, write_headless_multiview_frame_png,
    write_headless_stereo_frame_png,
};
use mclone_render::prepared_figure::{
    PreparedFigureDrawResources, PreparedFigureGpuSnapshot, PreparedFigureRenderStats,
    clear_prepared_figure_target,
};
use mclone_render::target::RenderFrameTarget;
use mclone_render::uniform::{LEFT_EYE_VIEW_SLOT, RIGHT_EYE_VIEW_SLOT};
use serde::{Deserialize, Serialize};

const DEFAULT_PANEL_WIDTH: u32 = 360;
const DEFAULT_PANEL_HEIGHT: u32 = 480;
const REVIEW_FOV_DEGREES: f32 = 35.0;
const WALK_PROOF_TIMES_SECONDS: [f64; 8] = [0.045, 0.09, 0.123, 0.125, 0.45, 0.855, 0.899, 0.901];

fn main() -> Result<()> {
    let options = Options::parse(env::args().skip(1))?;
    fs::create_dir_all(&options.out_dir).with_context(|| {
        format!(
            "failed to create figure review output directory {}",
            options.out_dir.display()
        )
    })?;

    let source = FilesystemAssetSource::new(&options.asset_root);
    let preparation_start = Instant::now();
    let figure = load_prepared_figure(&source, &options.figure_path)
        .with_context(|| format!("failed to prepare {}", options.figure_path.as_str()))?;
    let preparation_ms = preparation_start.elapsed().as_secs_f64() * 1_000.0;
    let review = options
        .review_contract
        .as_deref()
        .map(|path| {
            let bytes = fs::read(path)
                .with_context(|| format!("failed to read review contract {}", path.display()))?;
            serde_json::from_slice::<ReviewContract>(&bytes)
                .with_context(|| format!("failed to parse review contract {}", path.display()))
        })
        .transpose()?
        .unwrap_or_else(|| ReviewContract::for_figure(&figure, options.width, options.height));
    review.validate()?;
    let views = review.views();
    let figure_for_gpu = figure.clone();
    let (loop_report, pixels, state) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: review.panel_width,
            height: review.panel_height,
            frame_count: views.len(),
            pace_frame_duration: None,
        },
        move |device, queue, format, _size| {
            Ok(ReviewState {
                draw: PreparedFigureDrawResources::new(device, queue, format, &figure_for_gpu)?,
                stats: Vec::with_capacity(views.len()),
            })
        },
        |index, frame, state| {
            let view = views
                .get(index)
                .context("prepared figure review view index out of range")?;
            let depth =
                ChunkDepthTarget::new(frame.device, frame.target.size[0], frame.target.size[1]);
            clear_prepared_figure_target(
                frame.encoder,
                frame.target,
                &depth.view,
                review.clear_color(),
            );
            let stats = state.draw.render(
                frame.queue,
                frame.encoder,
                frame.target.with_depth(&depth.view),
                view.camera
                    .render_view(frame.target.size[0], frame.target.size[1]),
            )?;
            state.stats.push(stats);
            Ok(())
        },
    )?;
    if pixels.len() != views.len() || state.stats.len() != views.len() {
        bail!(
            "prepared figure review produced {} images and {} reports for {} views",
            pixels.len(),
            state.stats.len(),
            views.len()
        );
    }
    for ((view, pixels), stats) in views.iter().zip(&pixels).zip(&state.stats) {
        if stats.vertex_count != figure.vertices.len() as u32
            || stats.index_count != figure.indices.len() as u32
            || stats.draw_count != 1
        {
            bail!(
                "prepared figure view '{}' reported unexpected draw counts",
                view.name
            );
        }
        save_rgba_png(
            &options.out_dir.join(format!("engine-{}.png", view.name)),
            review.panel_width,
            review.panel_height,
            pixels,
        )?;
    }
    let gpu = state.draw.snapshot();
    if gpu.immutable_upload_count != 4 || gpu.view_uniform_write_count != views.len() as u64 {
        bail!(
            "prepared figure residency reported {} immutable uploads and {} view writes",
            gpu.immutable_upload_count,
            gpu.view_uniform_write_count
        );
    }
    let receipt = ReviewReceipt::new(
        &options,
        &figure,
        &review,
        preparation_ms,
        loop_report.setup_ms,
        loop_report.total_frame_ms,
        gpu,
    );
    let receipt_path = options.out_dir.join("engine-receipt.json");
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)
        .with_context(|| format!("failed to write {}", receipt_path.display()))?;
    if options.portability {
        write_portability_review(&options, &figure, &review)?;
    }
    if options.animation_proof {
        write_animation_proof(&options, &figure, &review)?;
    }
    println!(
        "prepared figure review wrote {} views to {} ({} vertices, {} indices, {:.3} ms preparation)",
        views.len(),
        options.out_dir.display(),
        figure.vertices.len(),
        figure.indices.len(),
        preparation_ms
    );
    Ok(())
}

fn write_portability_review(
    options: &Options,
    figure: &PreparedFigure,
    review: &ReviewContract,
) -> Result<()> {
    let stereo_cameras = review.stereo_cameras();
    let per_eye_path = options.out_dir.join("engine-stereo-per-eye.png");
    let per_eye_figure = figure.clone();
    let (per_eye_report, per_eye_gpu) = write_headless_stereo_frame_png(
        HeadlessStereoFrameOptions {
            path: per_eye_path.clone(),
            eye_width: review.panel_width,
            eye_height: review.panel_height,
        },
        move |device, queue, format, size, left_color, right_color| {
            let mut draw =
                PreparedFigureDrawResources::new(device, queue, format, &per_eye_figure)?;
            let left_depth = ChunkDepthTarget::new(device, size[0], size[1]);
            let right_depth = ChunkDepthTarget::new(device, size[0], size[1]);
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_prepared_figure_per_eye_encoder"),
            });
            let left_target =
                RenderFrameTarget::color(left_color, size).with_depth(&left_depth.view);
            clear_prepared_figure_target(
                &mut encoder,
                left_target,
                &left_depth.view,
                review.clear_color(),
            );
            draw.render_in_slot(
                queue,
                &mut encoder,
                left_target,
                stereo_cameras[0].render_view(size[0], size[1]),
                LEFT_EYE_VIEW_SLOT,
            )?;
            let right_target =
                RenderFrameTarget::color(right_color, size).with_depth(&right_depth.view);
            clear_prepared_figure_target(
                &mut encoder,
                right_target,
                &right_depth.view,
                review.clear_color(),
            );
            draw.render_in_slot(
                queue,
                &mut encoder,
                right_target,
                stereo_cameras[1].render_view(size[0], size[1]),
                RIGHT_EYE_VIEW_SLOT,
            )?;
            queue.submit(std::iter::once(encoder.finish()));
            Ok(draw.snapshot())
        },
    )?;
    if per_eye_gpu.immutable_upload_count != 4
        || per_eye_gpu.view_uniform_write_count != 2
        || per_eye_report.eye_pixel_difference_count == 0
    {
        bail!("prepared per-eye stereo did not retain resources or distinct eye pixels");
    }

    let multiview_path = options.out_dir.join("engine-stereo-multiview.png");
    let multiview_figure = figure.clone();
    let multiview_result = write_headless_multiview_frame_png(
        HeadlessMultiviewFrameOptions {
            path: multiview_path.clone(),
            eye_width: review.panel_width,
            eye_height: review.panel_height,
        },
        move |device, queue, format, size, color, depth| {
            let mut draw =
                PreparedFigureDrawResources::new(device, queue, format, &multiview_figure)?;
            if !draw.multiview_supported() {
                bail!("prepared figure resource omitted its multiview pipeline");
            }
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_prepared_figure_multiview_encoder"),
            });
            draw.render_multiview(
                queue,
                &mut encoder,
                RenderFrameTarget::color(color, size).with_depth(depth),
                [
                    stereo_cameras[0].render_view(size[0], size[1]),
                    stereo_cameras[1].render_view(size[0], size[1]),
                ],
                Some(review.clear_color()),
            )?;
            queue.submit(std::iter::once(encoder.finish()));
            Ok(draw.snapshot())
        },
    );
    let (multiview, multiview_unavailable_reason, pixel_identical) = match multiview_result {
        Ok((multiview_report, multiview_gpu)) => {
            if multiview_gpu.immutable_upload_count != 4
                || multiview_gpu.multiview_pipeline_count != 1
                || multiview_gpu.multiview_uniform_write_count != 1
                || multiview_report.eye_pixel_difference_count == 0
            {
                bail!("prepared multiview stereo did not retain resources or distinct eye pixels");
            }
            let pixel_identical = fs::read(&per_eye_path)? == fs::read(&multiview_path)?;
            if !pixel_identical {
                bail!("prepared per-eye and multiview stereo captures differ");
            }
            (
                Some(PortabilityPathReceipt::new(
                    &multiview_path,
                    multiview_report.eye_pixel_difference_count,
                    multiview_gpu,
                )),
                None,
                Some(pixel_identical),
            )
        }
        Err(error) if error.to_string().contains("does not expose wgpu MULTIVIEW") => {
            (None, Some(error.to_string()), None)
        }
        Err(error) => return Err(error),
    };
    let receipt = PortabilityReceipt {
        schema_version: 1,
        figure: &figure.name,
        eye_separation: ReviewContract::STEREO_EYE_SEPARATION,
        views: ["left", "right"],
        per_eye: PortabilityPathReceipt::new(
            &per_eye_path,
            per_eye_report.eye_pixel_difference_count,
            per_eye_gpu,
        ),
        multiview,
        multiview_unavailable_reason,
        pixel_identical,
    };
    let receipt_path = options.out_dir.join("portability-receipt.json");
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)
        .with_context(|| format!("failed to write {}", receipt_path.display()))?;
    Ok(())
}

fn write_animation_proof(
    options: &Options,
    figure: &PreparedFigure,
    review: &ReviewContract,
) -> Result<()> {
    let figure_for_gpu = figure.clone();
    let camera = review.views()[0].camera;
    let (loop_report, pixels, state) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: review.panel_width,
            height: review.panel_height,
            frame_count: WALK_PROOF_TIMES_SECONDS.len(),
            pace_frame_duration: None,
        },
        move |device, queue, format, _size| {
            Ok(AnimationReviewState {
                draw: PreparedFigureDrawResources::new(device, queue, format, &figure_for_gpu)?,
                figure: figure_for_gpu,
                palette: Vec::new(),
                samples: Vec::with_capacity(WALK_PROOF_TIMES_SECONDS.len()),
                stats: Vec::with_capacity(WALK_PROOF_TIMES_SECONDS.len()),
            })
        },
        |index, frame, state| {
            let time_seconds = *WALK_PROOF_TIMES_SECONDS
                .get(index)
                .context("prepared animation review time index out of range")?;
            let sample = evaluate_prepared_figure_clip_into(
                &state.figure,
                "walk",
                time_seconds,
                &mut state.palette,
            )?;
            state.draw.write_palette(frame.queue, &state.palette)?;
            let depth =
                ChunkDepthTarget::new(frame.device, frame.target.size[0], frame.target.size[1]);
            clear_prepared_figure_target(
                frame.encoder,
                frame.target,
                &depth.view,
                review.clear_color(),
            );
            let stats = state.draw.render(
                frame.queue,
                frame.encoder,
                frame.target.with_depth(&depth.view),
                camera.render_view(frame.target.size[0], frame.target.size[1]),
            )?;
            state.samples.push(sample);
            state.stats.push(stats);
            Ok(())
        },
    )?;
    if pixels.len() != WALK_PROOF_TIMES_SECONDS.len()
        || state.samples.len() != WALK_PROOF_TIMES_SECONDS.len()
        || state.stats.len() != WALK_PROOF_TIMES_SECONDS.len()
    {
        bail!("prepared animation review did not produce every requested frame");
    }

    let mut frames = Vec::with_capacity(pixels.len());
    let mut previous_pixels: Option<&[u8]> = None;
    for (((time_seconds, sample), stats), pixels) in WALK_PROOF_TIMES_SECONDS
        .iter()
        .zip(&state.samples)
        .zip(&state.stats)
        .zip(&pixels)
    {
        if stats.vertex_count != figure.vertices.len() as u32
            || stats.index_count != figure.indices.len() as u32
            || stats.draw_count != 1
        {
            bail!("prepared animation frame reported unexpected draw counts");
        }
        let image = format!(
            "engine-walk-{:06}us.png",
            (time_seconds * 1_000_000.0).round() as u64
        );
        save_rgba_png(
            &options.out_dir.join(&image),
            review.panel_width,
            review.panel_height,
            pixels,
        )?;
        let different_from_previous = previous_pixels
            .map(|previous| differing_pixel_count(previous, pixels))
            .transpose()?;
        frames.push(AnimationFrameReceipt {
            requested_time_seconds: *time_seconds,
            local_time_seconds: sample.local_time_seconds,
            image,
            different_from_previous,
        });
        previous_pixels = Some(pixels);
    }
    if frames
        .iter()
        .skip(1)
        .all(|frame| frame.different_from_previous == Some(0))
    {
        bail!("prepared animation proof pixels did not change across poses");
    }
    let gpu = state.draw.snapshot();
    let expected_palette_bytes = figure.parts.len() as u64 * 16 * 4;
    if gpu.immutable_upload_count != 4
        || gpu.palette_write_count != WALK_PROOF_TIMES_SECONDS.len() as u64
        || gpu.palette_written_bytes
            != expected_palette_bytes * WALK_PROOF_TIMES_SECONDS.len() as u64
        || gpu.view_uniform_write_count != WALK_PROOF_TIMES_SECONDS.len() as u64
    {
        bail!(
            "prepared animation residency reported unexpected uploads/writes: {:?}",
            gpu
        );
    }
    let receipt = AnimationReceipt {
        schema_version: 1,
        figure: &figure.name,
        clip: "walk",
        duration_seconds: state.samples[0].duration_seconds,
        frame_count: frames.len(),
        render_total_ms: loop_report.total_frame_ms,
        immutable_upload_count: gpu.immutable_upload_count,
        immutable_vertex_bytes: gpu.immutable_vertex_bytes,
        immutable_index_bytes: gpu.immutable_index_bytes,
        immutable_atlas_bytes: gpu.immutable_atlas_bytes,
        initial_palette_bytes: gpu.immutable_palette_bytes,
        palette_write_count: gpu.palette_write_count,
        palette_written_bytes: gpu.palette_written_bytes,
        palette_bytes_per_write: expected_palette_bytes,
        view_uniform_write_count: gpu.view_uniform_write_count,
        frames,
    };
    let receipt_path = options.out_dir.join("animation-receipt.json");
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)
        .with_context(|| format!("failed to write {}", receipt_path.display()))?;
    Ok(())
}

fn differing_pixel_count(left: &[u8], right: &[u8]) -> Result<usize> {
    if left.len() != right.len() || left.len() % 4 != 0 {
        bail!("prepared animation pixel buffers have incompatible lengths");
    }
    Ok(left
        .chunks_exact(4)
        .zip(right.chunks_exact(4))
        .filter(|(left, right)| left != right)
        .count())
}

struct AnimationReviewState {
    draw: PreparedFigureDrawResources,
    figure: PreparedFigure,
    palette: Vec<[[f32; 4]; 4]>,
    samples: Vec<PreparedFigurePoseSample>,
    stats: Vec<PreparedFigureRenderStats>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnimationReceipt<'a> {
    schema_version: u32,
    figure: &'a str,
    clip: &'static str,
    duration_seconds: f64,
    frame_count: usize,
    render_total_ms: f64,
    immutable_upload_count: u64,
    immutable_vertex_bytes: u64,
    immutable_index_bytes: u64,
    immutable_atlas_bytes: u64,
    initial_palette_bytes: u64,
    palette_write_count: u64,
    palette_written_bytes: u64,
    palette_bytes_per_write: u64,
    view_uniform_write_count: u64,
    frames: Vec<AnimationFrameReceipt>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnimationFrameReceipt {
    requested_time_seconds: f64,
    local_time_seconds: f64,
    image: String,
    different_from_previous: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortabilityReceipt<'a> {
    schema_version: u32,
    figure: &'a str,
    eye_separation: f32,
    views: [&'static str; 2],
    per_eye: PortabilityPathReceipt,
    multiview: Option<PortabilityPathReceipt>,
    multiview_unavailable_reason: Option<String>,
    pixel_identical: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortabilityPathReceipt {
    image: String,
    eye_pixel_difference_count: usize,
    immutable_upload_count: u64,
    view_uniform_write_count: u64,
    multiview_uniform_write_count: u64,
    multiview_pipeline_count: u64,
}

impl PortabilityPathReceipt {
    fn new(
        image: &std::path::Path,
        eye_pixel_difference_count: usize,
        gpu: PreparedFigureGpuSnapshot,
    ) -> Self {
        Self {
            image: image
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown.png")
                .to_owned(),
            eye_pixel_difference_count,
            immutable_upload_count: gpu.immutable_upload_count,
            view_uniform_write_count: gpu.view_uniform_write_count,
            multiview_uniform_write_count: gpu.multiview_uniform_write_count,
            multiview_pipeline_count: gpu.multiview_pipeline_count,
        }
    }
}

struct ReviewState {
    draw: PreparedFigureDrawResources,
    stats: Vec<PreparedFigureRenderStats>,
}

#[derive(Clone, Debug)]
struct Options {
    asset_root: PathBuf,
    figure_path: AssetPath,
    out_dir: PathBuf,
    width: u32,
    height: u32,
    review_contract: Option<PathBuf>,
    portability: bool,
    animation_proof: bool,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut asset_root = PathBuf::from(".");
        let mut figure_path = default_player_figure_path();
        let mut out_dir = PathBuf::from("/tmp/mclone-figure-review/player");
        let mut width = DEFAULT_PANEL_WIDTH;
        let mut height = DEFAULT_PANEL_HEIGHT;
        let mut review_contract = None;
        let mut portability = false;
        let mut animation_proof = false;
        let mut args = args.into_iter();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--asset-root" => {
                    asset_root = PathBuf::from(required_value("--asset-root", args.next())?);
                }
                "--figure" => {
                    figure_path = AssetPath::try_new(required_value("--figure", args.next())?)?;
                }
                "--out-dir" => {
                    out_dir = PathBuf::from(required_value("--out-dir", args.next())?);
                }
                "--width" => {
                    width = parse_dimension("--width", args.next())?;
                }
                "--height" => {
                    height = parse_dimension("--height", args.next())?;
                }
                "--review-contract" => {
                    review_contract = Some(PathBuf::from(required_value(
                        "--review-contract",
                        args.next(),
                    )?));
                }
                "--portability" => portability = true,
                "--animation-proof" => animation_proof = true,
                "--help" | "-h" => {
                    println!(
                        "Usage: mclone-figure-review [--asset-root PATH] [--figure ASSET_PATH] [--out-dir PATH] [--width PIXELS] [--height PIXELS] [--review-contract JSON] [--portability] [--animation-proof]"
                    );
                    std::process::exit(0);
                }
                _ => bail!("unknown argument '{}'", argument),
            }
        }
        Ok(Self {
            asset_root,
            figure_path,
            out_dir,
            width,
            height,
            review_contract,
            portability,
            animation_proof,
        })
    }
}

fn required_value(flag: &str, value: Option<String>) -> Result<String> {
    value.with_context(|| format!("{} requires a value", flag))
}

fn parse_dimension(flag: &str, value: Option<String>) -> Result<u32> {
    let value = required_value(flag, value)?;
    let value = value
        .parse::<u32>()
        .with_context(|| format!("{} requires an integer", flag))?;
    if value == 0 || value > 4096 {
        bail!("{} must be between 1 and 4096", flag);
    }
    Ok(value)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewContract {
    panel_width: u32,
    panel_height: u32,
    fov_degrees: f32,
    distance: f32,
    target: [f32; 3],
    background: String,
}

impl ReviewContract {
    const STEREO_EYE_SEPARATION: f32 = 0.036;

    fn for_figure(figure: &PreparedFigure, panel_width: u32, panel_height: u32) -> Self {
        let min = Vec3::from_array(figure.bounds.min);
        let max = Vec3::from_array(figure.bounds.max);
        let center = (min + max) * 0.5;
        let radius = ((max - min).length() * 0.5).max(0.75);
        let fov_radians = REVIEW_FOV_DEGREES.to_radians();
        let distance = 2.2_f32.max((radius / (fov_radians * 0.5).sin()) * 1.12);
        Self {
            panel_width,
            panel_height,
            fov_degrees: REVIEW_FOV_DEGREES,
            distance,
            target: center.to_array(),
            background: "#edf1f4".to_owned(),
        }
    }

    fn validate(&self) -> Result<()> {
        if self.panel_width == 0
            || self.panel_height == 0
            || self.panel_width > 4096
            || self.panel_height > 4096
        {
            bail!("review contract panel dimensions must be between 1 and 4096");
        }
        if !self.fov_degrees.is_finite()
            || self.fov_degrees <= 0.0
            || self.fov_degrees >= 170.0
            || !self.distance.is_finite()
            || self.distance <= 0.0
            || self.target.iter().any(|value| !value.is_finite())
        {
            bail!("review contract contains invalid camera values");
        }
        if self.background != "#edf1f4" {
            bail!("review contract background must be #edf1f4");
        }
        Ok(())
    }

    fn views(&self) -> [ReviewView; 3] {
        [
            self.view("front", Vec3::new(0.0, 0.2, 1.0)),
            self.view("right", Vec3::new(1.0, 0.2, 0.0)),
            self.view("three-quarter", Vec3::new(0.78, 0.34, 1.0)),
        ]
    }

    fn view(&self, name: &'static str, direction: Vec3) -> ReviewView {
        let target = Vec3::from_array(self.target);
        let eye = target + direction.normalize() * self.distance;
        ReviewView {
            name,
            camera: ChunkCamera {
                eye: eye.to_array(),
                target: self.target,
                up: [0.0, 1.0, 0.0],
                fov_y_radians: self.fov_degrees.to_radians(),
                z_near: 0.01,
                z_far: 100.0,
            },
        }
    }

    fn stereo_cameras(&self) -> [ChunkCamera; 2] {
        let target = Vec3::from_array(self.target);
        let direction = Vec3::new(0.0, 0.2, 1.0).normalize();
        let center_eye = target + direction * self.distance;
        let half_separation = Self::STEREO_EYE_SEPARATION * 0.5;
        [-half_separation, half_separation].map(|offset| ChunkCamera {
            eye: (center_eye + Vec3::X * offset).to_array(),
            target: (target + Vec3::X * offset).to_array(),
            up: [0.0, 1.0, 0.0],
            fov_y_radians: self.fov_degrees.to_radians(),
            z_near: 0.01,
            z_far: 100.0,
        })
    }

    fn clear_color(&self) -> wgpu::Color {
        wgpu::Color {
            r: 0xed as f64 / 255.0,
            g: 0xf1 as f64 / 255.0,
            b: 0xf4 as f64 / 255.0,
            a: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ReviewView {
    name: &'static str,
    camera: ChunkCamera,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewReceipt<'a> {
    schema_version: u32,
    figure: &'a str,
    asset_path: &'a str,
    compiler_id: &'a str,
    semantic_crc32: Option<String>,
    preparation_ms: f64,
    gpu_setup_ms: f64,
    render_total_ms: f64,
    part_count: usize,
    vertex_count: usize,
    index_count: usize,
    draw_range_count: usize,
    prepared_cpu_bytes: usize,
    atlas_width: u32,
    atlas_height: u32,
    atlas_bytes: usize,
    review: &'a ReviewContract,
    immutable_upload_count: u64,
    immutable_vertex_bytes: u64,
    immutable_index_bytes: u64,
    immutable_atlas_bytes: u64,
    immutable_palette_bytes: u64,
    view_uniform_write_count: u64,
    multiview_uniform_write_count: u64,
    multiview_pipeline_count: u64,
}

impl<'a> ReviewReceipt<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        options: &'a Options,
        figure: &'a PreparedFigure,
        review: &'a ReviewContract,
        preparation_ms: f64,
        gpu_setup_ms: f64,
        render_total_ms: f64,
        gpu: PreparedFigureGpuSnapshot,
    ) -> Self {
        Self {
            schema_version: 1,
            figure: &figure.name,
            asset_path: options.figure_path.as_str(),
            compiler_id: figure.diagnostics.compiler_id,
            semantic_crc32: figure
                .diagnostics
                .semantic_crc32
                .map(|value| format!("{value:08x}")),
            preparation_ms,
            gpu_setup_ms,
            render_total_ms,
            part_count: figure.parts.len(),
            vertex_count: figure.vertices.len(),
            index_count: figure.indices.len(),
            draw_range_count: figure.draw_ranges.len(),
            prepared_cpu_bytes: figure.diagnostics.prepared_cpu_bytes,
            atlas_width: figure.atlas.width,
            atlas_height: figure.atlas.height,
            atlas_bytes: figure.atlas.rgba.len(),
            review,
            immutable_upload_count: gpu.immutable_upload_count,
            immutable_vertex_bytes: gpu.immutable_vertex_bytes,
            immutable_index_bytes: gpu.immutable_index_bytes,
            immutable_atlas_bytes: gpu.immutable_atlas_bytes,
            immutable_palette_bytes: gpu.immutable_palette_bytes,
            view_uniform_write_count: gpu.view_uniform_write_count,
            multiview_uniform_write_count: gpu.multiview_uniform_write_count,
            multiview_pipeline_count: gpu.multiview_pipeline_count,
        }
    }
}
