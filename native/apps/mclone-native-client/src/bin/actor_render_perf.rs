use std::{
    env,
    path::PathBuf,
    process::Command,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_app_runtime::render_assets::load_actor_texture_assets;
use mclone_assets::{
    ActorFigureId, chicken_figure_id, cow_figure_id, default_player_figure_id,
    upright_bear_figure_id,
};
use mclone_diagnostics::{PercentileMethod, percentile_sorted_ms};
use mclone_render::{
    chunk::{ChunkCamera, ChunkDepthTarget, REVERSED_Z_DEPTH_CLEAR, TexturedSectionRenderOptions},
    entity::{ActorDrawResourceSnapshot, ActorDrawResources, ActorInstance, ActorInstanceId},
    headless::{
        HEADLESS_FORMAT, create_headless_device, read_headless_rgba8_texture, save_rgba_png,
    },
    target::RenderFrameTarget,
};
use serde::Serialize;

const DEFAULT_ACTOR_COUNT: usize = 100;
const DEFAULT_FRAME_COUNT: usize = 120;
const DEFAULT_WARMUP_FRAME_COUNT: usize = 30;
const DEFAULT_WIDTH: u32 = 640;
const DEFAULT_HEIGHT: u32 = 360;
const MAX_ACTOR_COUNT: usize = 4_096;
const MAX_LEGACY_ACTOR_COUNT: usize = 256;
const MAX_FRAME_COUNT: usize = 4_096;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let config = Config::parse(env::args().skip(1))?;
    let assets = load_actor_texture_assets().context("failed to load actor assets")?;
    let (device, queue) = create_headless_device()?;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mclone_actor_render_perf_color"),
        size: wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HEADLESS_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&Default::default());
    let depth = ChunkDepthTarget::new(&device, config.width, config.height);
    let mut resources = ActorDrawResources::new(
        &device,
        &queue,
        HEADLESS_FORMAT,
        assets.atlas.as_upload(),
        Some(&assets.figures),
    )?;
    let mut actors = benchmark_actors(config.actor_count, config.figure, config.path);
    let camera = benchmark_camera(config.actor_count);
    let render_view = camera.render_view(config.width, config.height);
    let render_options = TexturedSectionRenderOptions {
        force_fullbright: true,
        ..TexturedSectionRenderOptions::default()
    };

    for frame_index in 0..config.warmup_frames {
        advance_actors(&mut actors, frame_index, config.motion);
        render_frame(
            &device,
            &queue,
            &target_view,
            &depth,
            &mut resources,
            render_view,
            render_options,
            &actors,
        )?;
    }
    let measured_start = resources.resource_snapshot();

    let mut timings = Vec::with_capacity(config.frames);
    let mut last_stats = None;
    for frame_offset in 0..config.frames {
        let frame_index = config.warmup_frames + frame_offset;
        advance_actors(&mut actors, frame_index, config.motion);
        let (timing, stats) = render_frame(
            &device,
            &queue,
            &target_view,
            &depth,
            &mut resources,
            render_view,
            render_options,
            &actors,
        )?;
        timings.push(timing);
        last_stats = Some(stats);
    }
    let measured_end = resources.resource_snapshot();
    let last_stats = last_stats.context("actor benchmark rendered no measured frames")?;

    let screenshot = if let Some(path) = &config.screenshot {
        let pixels =
            read_headless_rgba8_texture(&device, &queue, &target, config.width, config.height)?;
        let non_clear_rgb_pixels = count_non_clear_rgb_pixels(&pixels);
        if non_clear_rgb_pixels == 0 {
            bail!("actor benchmark screenshot contains no non-clear pixels");
        }
        save_rgba_png(path, config.width, config.height, &pixels)?;
        Some(ScreenshotReport {
            path: path.display().to_string(),
            non_clear_rgb_pixels,
        })
    } else {
        None
    };

    let report = Report {
        benchmark: "native_actor_render",
        recorded_unix_seconds: current_unix_seconds(),
        git_commit: git_commit(),
        git_dirty: git_dirty(),
        debug_assertions: cfg!(debug_assertions),
        config,
        timing: TimingReport::from_samples(&timings),
        actors: ActorReport {
            submitted: last_stats.submitted_actor_count,
            drawn: last_stats.drawn_actor_count,
            vertices_per_frame: last_stats.vertex_count,
            indices_per_frame: last_stats.index_count,
            prepared_per_frame: measured_end.prepared_world.prepared_actor_count,
            legacy_per_frame: measured_end.prepared_world.legacy_actor_count,
            instance_buckets_per_frame: measured_end.prepared_world.instance_bucket_count,
            unchanged_actor_reuses: measured_end
                .prepared_world
                .unchanged_actor_reuse_count
                .saturating_sub(measured_start.prepared_world.unchanged_actor_reuse_count),
            prepared_draws: measured_end
                .prepared_world
                .draw_count
                .saturating_sub(measured_start.prepared_world.draw_count),
            drawn_instances: measured_end
                .prepared_world
                .drawn_instance_count
                .saturating_sub(measured_start.prepared_world.drawn_instance_count),
            max_instances_per_draw: measured_end.prepared_world.max_instances_per_draw,
            pose_evaluations: measured_end
                .prepared_world
                .pose_evaluation_count
                .saturating_sub(measured_start.prepared_world.pose_evaluation_count),
            palette_writes: measured_end
                .prepared_world
                .palette_write_count
                .saturating_sub(measured_start.prepared_world.palette_write_count),
            palette_written_bytes: measured_end
                .prepared_world
                .palette_written_bytes
                .saturating_sub(measured_start.prepared_world.palette_written_bytes),
            actor_writes: measured_end
                .prepared_world
                .actor_write_count
                .saturating_sub(measured_start.prepared_world.actor_write_count),
            actor_written_bytes: measured_end
                .prepared_world
                .actor_written_bytes
                .saturating_sub(measured_start.prepared_world.actor_written_bytes),
            average_prepare_evaluation_ms: average_prepare_stage_ms(
                measured_start.prepared_world.prepare_count,
                measured_end.prepared_world.prepare_count,
                measured_start.prepared_world.prepare_evaluation_ns,
                measured_end.prepared_world.prepare_evaluation_ns,
            ),
            average_prepare_upload_ms: average_prepare_stage_ms(
                measured_start.prepared_world.prepare_count,
                measured_end.prepared_world.prepare_count,
                measured_start.prepared_world.prepare_upload_ns,
                measured_end.prepared_world.prepare_upload_ns,
            ),
            legacy_mesh_rebuilds: measured_end
                .mesh
                .rebuild_count
                .saturating_sub(measured_start.mesh.rebuild_count),
            legacy_mesh_uploads: measured_end
                .mesh
                .upload_count
                .saturating_sub(measured_start.mesh.upload_count),
            legacy_mesh_uploaded_bytes: measured_end
                .mesh
                .total_uploaded_bytes
                .saturating_sub(measured_start.mesh.total_uploaded_bytes),
            mutable_allocated_bytes: measured_end.mutable_state_allocated_bytes,
            immutable_prepared_figure_count: measured_end.prepared_shared.figure_count,
            immutable_prepared_bytes: prepared_immutable_bytes(measured_end),
        },
        screenshot,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
struct Config {
    actor_count: usize,
    frames: usize,
    warmup_frames: usize,
    width: u32,
    height: u32,
    figure: FigureSelection,
    path: RenderPath,
    motion: Motion,
    screenshot: Option<PathBuf>,
}

impl Config {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut config = Self {
            actor_count: DEFAULT_ACTOR_COUNT,
            frames: DEFAULT_FRAME_COUNT,
            warmup_frames: DEFAULT_WARMUP_FRAME_COUNT,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            figure: FigureSelection::Cow,
            path: RenderPath::Prepared,
            motion: Motion::Animated,
            screenshot: None,
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--actors" => config.actor_count = parse_next(&mut args, "--actors")?,
                "--frames" => config.frames = parse_next(&mut args, "--frames")?,
                "--warmup-frames" => {
                    config.warmup_frames = parse_next(&mut args, "--warmup-frames")?
                }
                "--width" => config.width = parse_next(&mut args, "--width")?,
                "--height" => config.height = parse_next(&mut args, "--height")?,
                "--figure" => config.figure = parse_next(&mut args, "--figure")?,
                "--path" => config.path = parse_next(&mut args, "--path")?,
                "--motion" => config.motion = parse_next(&mut args, "--motion")?,
                "--screenshot" => {
                    config.screenshot = Some(PathBuf::from(
                        args.next()
                            .context("--screenshot requires an output PNG path")?,
                    ));
                }
                "--help" | "-h" => bail!("{}", usage()),
                _ => bail!("unknown argument {arg}\n{}", usage()),
            }
        }
        if config.actor_count == 0 || config.actor_count > MAX_ACTOR_COUNT {
            bail!("--actors must be in 1..={MAX_ACTOR_COUNT}");
        }
        if config.path == RenderPath::Legacy && config.actor_count > MAX_LEGACY_ACTOR_COUNT {
            bail!(
                "--path legacy is limited to {MAX_LEGACY_ACTOR_COUNT} actors because the \
                 per-texel cow fallback intentionally creates a very large mesh"
            );
        }
        if config.frames == 0 || config.frames > MAX_FRAME_COUNT {
            bail!("--frames must be in 1..={MAX_FRAME_COUNT}");
        }
        if config.warmup_frames > MAX_FRAME_COUNT {
            bail!("--warmup-frames must be <= {MAX_FRAME_COUNT}");
        }
        if config.width == 0 || config.height == 0 {
            bail!("--width and --height must be greater than zero");
        }
        Ok(config)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum FigureSelection {
    Cow,
    Chicken,
    Mixed,
}

impl std::str::FromStr for FigureSelection {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "cow" => Ok(Self::Cow),
            "chicken" => Ok(Self::Chicken),
            "mixed" => Ok(Self::Mixed),
            _ => bail!("--figure must be cow, chicken, or mixed"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RenderPath {
    Prepared,
    Legacy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Motion {
    Animated,
    Stationary,
}

impl std::str::FromStr for Motion {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "animated" => Ok(Self::Animated),
            "stationary" => Ok(Self::Stationary),
            _ => bail!("--motion must be animated or stationary"),
        }
    }
}

impl std::str::FromStr for RenderPath {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "prepared" => Ok(Self::Prepared),
            "legacy" => Ok(Self::Legacy),
            _ => bail!("--path must be prepared or legacy"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct FrameTiming {
    frame_ms: f64,
    prepare_ms: f64,
    encode_ms: f64,
    submit_ms: f64,
    device_poll_ms: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct TimingReport {
    average_frame_ms: f64,
    p50_frame_ms: f64,
    p95_frame_ms: f64,
    max_frame_ms: f64,
    average_prepare_ms: f64,
    average_encode_ms: f64,
    average_submit_ms: f64,
    average_device_poll_ms: f64,
}

impl TimingReport {
    fn from_samples(samples: &[FrameTiming]) -> Self {
        let mut sorted = samples
            .iter()
            .map(|sample| sample.frame_ms)
            .collect::<Vec<_>>();
        sorted.sort_by(f64::total_cmp);
        let count = samples.len() as f64;
        Self {
            average_frame_ms: samples.iter().map(|sample| sample.frame_ms).sum::<f64>() / count,
            p50_frame_ms: percentile_sorted_ms(&sorted, 0.50, PercentileMethod::NearestRank),
            p95_frame_ms: percentile_sorted_ms(&sorted, 0.95, PercentileMethod::NearestRank),
            max_frame_ms: sorted.last().copied().unwrap_or(0.0),
            average_prepare_ms: samples.iter().map(|sample| sample.prepare_ms).sum::<f64>() / count,
            average_encode_ms: samples.iter().map(|sample| sample.encode_ms).sum::<f64>() / count,
            average_submit_ms: samples.iter().map(|sample| sample.submit_ms).sum::<f64>() / count,
            average_device_poll_ms: samples
                .iter()
                .map(|sample| sample.device_poll_ms)
                .sum::<f64>()
                / count,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ActorReport {
    submitted: usize,
    drawn: usize,
    vertices_per_frame: u32,
    indices_per_frame: u32,
    prepared_per_frame: usize,
    legacy_per_frame: usize,
    instance_buckets_per_frame: usize,
    unchanged_actor_reuses: u64,
    prepared_draws: u64,
    drawn_instances: u64,
    max_instances_per_draw: u32,
    pose_evaluations: u64,
    palette_writes: u64,
    palette_written_bytes: u64,
    actor_writes: u64,
    actor_written_bytes: u64,
    average_prepare_evaluation_ms: f64,
    average_prepare_upload_ms: f64,
    legacy_mesh_rebuilds: u64,
    legacy_mesh_uploads: u64,
    legacy_mesh_uploaded_bytes: u64,
    mutable_allocated_bytes: u64,
    immutable_prepared_figure_count: usize,
    immutable_prepared_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
struct ScreenshotReport {
    path: String,
    non_clear_rgb_pixels: usize,
}

#[derive(Clone, Debug, Serialize)]
struct Report {
    benchmark: &'static str,
    recorded_unix_seconds: u64,
    git_commit: String,
    git_dirty: bool,
    debug_assertions: bool,
    config: Config,
    timing: TimingReport,
    actors: ActorReport,
    screenshot: Option<ScreenshotReport>,
}

fn benchmark_actors(
    actor_count: usize,
    figure_selection: FigureSelection,
    path: RenderPath,
) -> Vec<ActorInstance> {
    let columns = (actor_count as f64).sqrt().ceil() as usize;
    let rows = actor_count.div_ceil(columns);
    let spacing = 1.25;
    let center_x = (columns.saturating_sub(1)) as f32 * spacing * 0.5;
    let center_z = (rows.saturating_sub(1)) as f32 * spacing * 0.5;
    (0..actor_count)
        .map(|index| {
            let figure = selected_figure(figure_selection, index);
            let x = (index % columns) as f32 * spacing - center_x;
            let z = (index / columns) as f32 * spacing - center_z;
            let (width, height) = figure_dimensions(figure);
            let mut actor = ActorInstance::remote_player_with_figure(
                Vec3::new(x, 0.0, z),
                (index % 31) as f32 - 15.0,
                figure,
            )
            .with_dimensions(width, height)
            .with_walk_animation_distance(index as f32 * 0.017);
            if path == RenderPath::Prepared {
                actor = actor.with_id(ActorInstanceId::Entity(index as u64 + 1));
            }
            if figure == chicken_figure_id() {
                actor = actor.with_chicken_wing_flap_radians(Some(0.45));
            }
            actor
        })
        .collect()
}

fn selected_figure(selection: FigureSelection, index: usize) -> ActorFigureId {
    match selection {
        FigureSelection::Cow => cow_figure_id(),
        FigureSelection::Chicken => chicken_figure_id(),
        FigureSelection::Mixed => [
            cow_figure_id(),
            chicken_figure_id(),
            default_player_figure_id(),
            upright_bear_figure_id(),
        ][index % 4],
    }
}

fn figure_dimensions(figure: ActorFigureId) -> (f32, f32) {
    if figure == cow_figure_id() {
        (0.9, 1.4)
    } else if figure == chicken_figure_id() {
        (0.4, 0.7)
    } else if figure == upright_bear_figure_id() {
        (0.8, 1.8)
    } else {
        (0.6, 1.8)
    }
}

fn benchmark_camera(actor_count: usize) -> ChunkCamera {
    let side = (actor_count as f32).sqrt().ceil() * 1.25;
    ChunkCamera {
        eye: [0.0, side * 0.85 + 3.0, side * 1.15 + 6.0],
        target: [0.0, 0.65, 0.0],
        up: [0.0, 1.0, 0.0],
        fov_y_radians: 55.0_f32.to_radians(),
        z_near: 0.05,
        z_far: 512.0,
    }
}

fn advance_actors(actors: &mut [ActorInstance], frame_index: usize, motion: Motion) {
    if motion == Motion::Stationary {
        return;
    }
    let direction = if frame_index % 240 < 120 { 1.0 } else { -1.0 };
    for actor in actors {
        actor.feet_position.x += direction * 0.0025;
        if let Some(animation) = actor.animation.as_mut() {
            animation.distance += 0.035;
        }
        if let Some(flap) = actor.chicken_wing_flap_radians.as_mut() {
            *flap = (*flap + 0.17).sin() * 0.8;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_frame(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color_view: &wgpu::TextureView,
    depth: &ChunkDepthTarget,
    resources: &mut ActorDrawResources,
    render_view: mclone_render::chunk::ChunkRenderView,
    render_options: TexturedSectionRenderOptions,
    actors: &[ActorInstance],
) -> Result<(FrameTiming, mclone_render::entity::ActorRenderStats)> {
    let frame_start = Instant::now();
    let prepare_start = Instant::now();
    resources.prepare_for_frame(device, queue, actors);
    let prepare_ms = elapsed_ms(prepare_start.elapsed());
    let encode_start = Instant::now();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_actor_render_perf_encoder"),
    });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_actor_render_perf_clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.62,
                        g: 0.66,
                        b: 0.70,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(REVERSED_Z_DEPTH_CLEAR),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
    }
    let stats = resources.render_reusing_prepared_in_slot(
        device,
        queue,
        &mut encoder,
        RenderFrameTarget::color(color_view, [depth.width, depth.height]).with_depth(&depth.view),
        render_view,
        render_options,
        actors,
        mclone_render::uniform::SINGLE_VIEW_SLOT,
    )?;
    let command = encoder.finish();
    let encode_ms = elapsed_ms(encode_start.elapsed());
    let submit_start = Instant::now();
    let submission = queue.submit(std::iter::once(command));
    let submit_ms = elapsed_ms(submit_start.elapsed());
    let poll_start = Instant::now();
    device
        .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
        .or_else(|_| device.poll(wgpu::PollType::Wait))?;
    let device_poll_ms = elapsed_ms(poll_start.elapsed());
    Ok((
        FrameTiming {
            frame_ms: elapsed_ms(frame_start.elapsed()),
            prepare_ms,
            encode_ms,
            submit_ms,
            device_poll_ms,
        },
        stats,
    ))
}

fn prepared_immutable_bytes(snapshot: ActorDrawResourceSnapshot) -> u64 {
    snapshot
        .prepared_shared
        .immutable_vertex_bytes
        .saturating_add(snapshot.prepared_shared.immutable_index_bytes)
        .saturating_add(snapshot.prepared_shared.immutable_atlas_bytes)
}

fn count_non_clear_rgb_pixels(pixels: &[u8]) -> usize {
    let Some(clear) = pixels.get(0..3) else {
        return 0;
    };
    pixels
        .chunks_exact(4)
        .filter(|pixel| &pixel[0..3] != clear)
        .count()
}

fn parse_next<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    args.next()
        .with_context(|| format!("{flag} requires a value"))?
        .parse()
        .map_err(|error| anyhow::anyhow!("invalid {flag}: {error}"))
}

fn usage() -> &'static str {
    "usage: actor_render_perf [--actors N] [--frames N] [--warmup-frames N] \
     [--width N] [--height N] [--figure cow|chicken|mixed] \
     [--path prepared|legacy] [--motion animated|stationary] \
     [--screenshot /tmp/actors.png]"
}

fn elapsed_ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn average_prepare_stage_ms(start_count: u64, end_count: u64, start_ns: u64, end_ns: u64) -> f64 {
    let count = end_count.saturating_sub(start_count);
    if count == 0 {
        return 0.0;
    }
    end_ns.saturating_sub(start_ns) as f64 / count as f64 / 1_000_000.0
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn git_commit() -> String {
    command_output("git", &["rev-parse", "--short=8", "HEAD"])
        .unwrap_or_else(|| "unknown".to_owned())
}

fn git_dirty() -> bool {
    command_output("git", &["status", "--porcelain"]).is_some_and(|output| !output.is_empty())
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
