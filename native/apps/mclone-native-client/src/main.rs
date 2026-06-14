use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use mclone_client::ClientRuntime;
use mclone_core::{
    AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkPos, ChunkSnapshot, SECTION_HEIGHT,
};
use mclone_mesh::{ChunkMeshInput, VisibleChunkMesh, build_visible_chunk_area_mesh};
use mclone_net::LocalTransport;
use mclone_protocol::ChunkInterest;
use mclone_render::chunk::{ChunkCamera, ChunkDrawResources};
use mclone_render::headless::{
    HeadlessChunkOptions, HeadlessClearOptions, write_headless_chunk_png, write_headless_clear_png,
};
use mclone_render::native::{NativeSurfaceContext, SurfaceFrameStatus};
use mclone_server::IntegratedServer;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const DEFAULT_SEED: i64 = 12345;
const DEFAULT_CHUNK_X: i32 = 0;
const DEFAULT_CHUNK_Z: i32 = 0;
const DEFAULT_CHUNK_RADIUS: i32 = 1;
const MAX_CHUNK_RADIUS: i32 = 4;

fn main() -> Result<()> {
    env_logger::init();
    match Cli::parse(std::env::args().skip(1))? {
        Cli::HeadlessClear {
            path,
            width,
            height,
        } => {
            let report = write_headless_clear_png(HeadlessClearOptions {
                path,
                width,
                height,
                color: mclone_render::default_clear_color(),
            })?;
            println!(
                "headless clear saved to {} ({}x{}, {} bytes)",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len
            );
            Ok(())
        }
        Cli::HeadlessChunk {
            path,
            width,
            height,
            scene,
        } => {
            let mesh = build_scene_mesh(scene)?;
            let report = write_headless_chunk_png(
                HeadlessChunkOptions {
                    path,
                    width,
                    height,
                    color: mclone_render::default_clear_color(),
                    camera: ChunkCamera::overview_for_chunk_area(
                        scene.chunk_x,
                        scene.chunk_z,
                        scene.chunk_radius,
                    ),
                },
                &mesh,
            )?;
            println!(
                "headless chunk saved to {} ({}x{}, {} bytes, {} vertices, {} indices)",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len,
                report.vertex_count,
                report.index_count
            );
            Ok(())
        }
        Cli::Window { scene } => run_window(scene),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SceneOptions {
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    chunk_radius: i32,
}

impl Default for SceneOptions {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SEED,
            chunk_x: DEFAULT_CHUNK_X,
            chunk_z: DEFAULT_CHUNK_Z,
            chunk_radius: DEFAULT_CHUNK_RADIUS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Cli {
    Window {
        scene: SceneOptions,
    },
    HeadlessClear {
        path: PathBuf,
        width: u32,
        height: u32,
    },
    HeadlessChunk {
        path: PathBuf,
        width: u32,
        height: u32,
        scene: SceneOptions,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HeadlessMode {
    Clear(PathBuf),
    Chunk(PathBuf),
}

impl Cli {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut mode = None;
        let mut width = None;
        let mut height = None;
        let mut scene = SceneOptions::default();
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--headless-clear" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-clear requires an output PNG path")?;
                    set_headless_mode(&mut mode, HeadlessMode::Clear(path))?;
                }
                "--headless-chunk" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-chunk requires an output PNG path")?;
                    set_headless_mode(&mut mode, HeadlessMode::Chunk(path))?;
                }
                "--width" => width = Some(parse_u32_arg("--width", args.next())?),
                "--height" => height = Some(parse_u32_arg("--height", args.next())?),
                "--seed" => scene.seed = parse_i64_arg("--seed", args.next())?,
                "--chunk-x" => scene.chunk_x = parse_i32_arg("--chunk-x", args.next())?,
                "--chunk-z" => scene.chunk_z = parse_i32_arg("--chunk-z", args.next())?,
                "--chunk-radius" => {
                    scene.chunk_radius = parse_chunk_radius_arg("--chunk-radius", args.next())?
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => bail!("unknown argument `{arg}`; pass --help for usage"),
            }
        }

        match mode {
            Some(HeadlessMode::Clear(path)) => Ok(Self::HeadlessClear {
                path,
                width: width.unwrap_or(96),
                height: height.unwrap_or(64),
            }),
            Some(HeadlessMode::Chunk(path)) => Ok(Self::HeadlessChunk {
                path,
                width: width.unwrap_or(640),
                height: height.unwrap_or(480),
                scene,
            }),
            None => Ok(Self::Window { scene }),
        }
    }
}

fn set_headless_mode(mode: &mut Option<HeadlessMode>, next: HeadlessMode) -> Result<()> {
    if mode.is_some() {
        bail!("only one headless output mode can be selected");
    }
    *mode = Some(next);
    Ok(())
}

fn parse_u32_arg(flag: &str, value: Option<String>) -> Result<u32> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    let parsed = value
        .parse::<u32>()
        .with_context(|| format!("{flag} requires an unsigned integer, got `{value}`"))?;
    if parsed == 0 {
        bail!("{flag} must be greater than zero");
    }
    Ok(parsed)
}

fn parse_i32_arg(flag: &str, value: Option<String>) -> Result<i32> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<i32>()
        .with_context(|| format!("{flag} requires a signed integer, got `{value}`"))
}

fn parse_i64_arg(flag: &str, value: Option<String>) -> Result<i64> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<i64>()
        .with_context(|| format!("{flag} requires a signed 64-bit integer, got `{value}`"))
}

fn parse_chunk_radius_arg(flag: &str, value: Option<String>) -> Result<i32> {
    let parsed = parse_i32_arg(flag, value)?;
    if !(0..=MAX_CHUNK_RADIUS).contains(&parsed) {
        bail!("{flag} must be between 0 and {MAX_CHUNK_RADIUS}");
    }
    Ok(parsed)
}

fn print_help() {
    println!(
        "mclone-native-client\n\n\
         Usage:\n\
           mclone-native-client [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--chunk-radius 1]\n\
           mclone-native-client --headless-clear /tmp/mclone-native-clear.png [--width 96] [--height 64]\n\
           mclone-native-client --headless-chunk /tmp/mclone-native-chunk.png [--width 640] [--height 480] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--chunk-radius 1]\n\n\
         Window mode renders generated chunks with WASD/QE, mouse drag, and wheel camera controls. Headless modes write PNGs for GPU validation."
    );
}

fn run_window(scene: SceneOptions) -> Result<()> {
    let mesh = build_scene_mesh(scene)?;
    let stats = mesh.stats();
    let chunk_count = scene.chunk_span() * scene.chunk_span();
    log::info!(
        "client-runtime chunk area seed={} center=({}, {}) radius={} chunks={} mesh={} vertices {} indices",
        scene.seed,
        scene.chunk_x,
        scene.chunk_z,
        scene.chunk_radius,
        chunk_count,
        stats.vertex_count,
        stats.index_count
    );

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ChunkApp::new(
        mesh,
        ChunkCamera::overview_for_chunk_area(scene.chunk_x, scene.chunk_z, scene.chunk_radius),
    );
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn build_scene_mesh(scene: SceneOptions) -> Result<VisibleChunkMesh> {
    let client = build_scene_client_runtime(scene)?;
    let chunks = client
        .chunk_snapshots()
        .map(snapshot_mesh_blocks)
        .collect::<Result<Vec<_>>>()?;
    let inputs = chunks
        .iter()
        .map(|chunk| {
            ChunkMeshInput::new(
                chunk.chunk_x,
                chunk.chunk_z,
                chunk.min_y,
                chunk.height,
                &chunk.blocks,
            )
        })
        .collect::<Vec<_>>();
    let mesh = build_visible_chunk_area_mesh(&inputs);
    if mesh.is_empty() {
        bail!(
            "generated chunk area seed={} center=({}, {}) radius={} produced an empty mesh",
            scene.seed,
            scene.chunk_x,
            scene.chunk_z,
            scene.chunk_radius
        );
    }
    Ok(mesh)
}

fn build_scene_client_runtime(scene: SceneOptions) -> Result<ClientRuntime> {
    let mut server = IntegratedServer::new(scene.seed);
    let mut client = ClientRuntime::local_integrated();
    let mut transport = LocalTransport::new();
    let radius_chunks =
        u32::try_from(scene.chunk_radius).context("chunk radius must be positive")?;

    let command = client.set_chunk_interest(ChunkInterest {
        center: ChunkPos::new(scene.chunk_x, scene.chunk_z),
        radius_chunks,
    });
    transport.send_client_command(command);

    for command in transport.drain_client_commands() {
        for update in server.handle_command(command) {
            transport.send_server_update(update);
        }
    }
    client.apply_updates(transport.drain_server_updates());
    Ok(client)
}

#[derive(Clone, Debug)]
struct MeshChunkBlocks {
    chunk_x: i32,
    chunk_z: i32,
    min_y: i32,
    height: i32,
    blocks: Vec<u8>,
}

fn snapshot_mesh_blocks(snapshot: &ChunkSnapshot) -> Result<MeshChunkBlocks> {
    if snapshot.height <= 0 || snapshot.height % SECTION_HEIGHT != 0 {
        bail!(
            "chunk snapshot {:?} has invalid height {}",
            snapshot.pos,
            snapshot.height
        );
    }
    let expected_len = snapshot.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
    let mut blocks = vec![AIR_BLOCK_STATE_ID.0 as u8; expected_len];
    let min_section_y = snapshot.min_y / SECTION_HEIGHT;
    let section_count = snapshot.height / SECTION_HEIGHT;

    for section in &snapshot.sections {
        let section_offset = section.section_y - min_section_y;
        if !(0..section_count).contains(&section_offset) {
            bail!(
                "chunk snapshot {:?} contains section {} outside {}..{}",
                snapshot.pos,
                section.section_y,
                min_section_y,
                min_section_y + section_count - 1
            );
        }
        let unpacked = section.unpack_block_state_ids();
        if unpacked.len() != CHUNK_SECTION_VOLUME {
            bail!(
                "chunk snapshot {:?} section {} unpacked to {} blocks",
                snapshot.pos,
                section.section_y,
                unpacked.len()
            );
        }
        let start = section_offset as usize * CHUNK_SECTION_VOLUME;
        for (index, state_id) in unpacked.into_iter().enumerate() {
            blocks[start + index] = u8::try_from(state_id.0).with_context(|| {
                format!(
                    "temporary mesh adapter cannot represent block state id {} as u8",
                    state_id.0
                )
            })?;
        }
    }

    Ok(MeshChunkBlocks {
        chunk_x: snapshot.pos.x,
        chunk_z: snapshot.pos.z,
        min_y: snapshot.min_y,
        height: snapshot.height,
        blocks,
    })
}

impl SceneOptions {
    fn chunk_span(self) -> i32 {
        self.chunk_radius * 2 + 1
    }

    #[cfg(test)]
    fn chunk_positions(self) -> impl Iterator<Item = (i32, i32)> {
        let min_x = self.chunk_x - self.chunk_radius;
        let max_x = self.chunk_x + self.chunk_radius;
        let min_z = self.chunk_z - self.chunk_radius;
        let max_z = self.chunk_z + self.chunk_radius;
        (min_x..=max_x)
            .flat_map(move |chunk_x| (min_z..=max_z).map(move |chunk_z| (chunk_x, chunk_z)))
    }
}

struct ChunkApp {
    mesh: VisibleChunkMesh,
    camera: ChunkCamera,
    window: Option<Arc<Window>>,
    surface: Option<NativeSurfaceContext>,
    draw: Option<ChunkDrawResources>,
    pressed_keys: std::collections::HashSet<KeyCode>,
    orbit_dragging: bool,
    last_cursor: Option<(f64, f64)>,
    last_frame: Instant,
}

impl ChunkApp {
    fn new(mesh: VisibleChunkMesh, camera: ChunkCamera) -> Self {
        Self {
            mesh,
            camera,
            window: None,
            surface: None,
            draw: None,
            pressed_keys: std::collections::HashSet::new(),
            orbit_dragging: false,
            last_cursor: None,
            last_frame: Instant::now(),
        }
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn update_camera_from_keys(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;

        let right = key_axis(&self.pressed_keys, KeyCode::KeyD, KeyCode::KeyA);
        let up = key_axis(&self.pressed_keys, KeyCode::KeyE, KeyCode::KeyQ);
        let forward = key_axis(&self.pressed_keys, KeyCode::KeyW, KeyCode::KeyS);
        let boosted = self.pressed_keys.contains(&KeyCode::ShiftLeft)
            || self.pressed_keys.contains(&KeyCode::ShiftRight);
        let speed = if boosted { 95.0 } else { 32.0 };
        self.camera.move_local(right, up, forward, speed * dt);
    }
}

impl ApplicationHandler for ChunkApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("mclone native")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 900));
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                log::error!("failed to create native window: {err}");
                event_loop.exit();
                return;
            }
        };
        let surface = match NativeSurfaceContext::new(window.clone()) {
            Ok(surface) => surface,
            Err(err) => {
                log::error!("failed to initialize native GPU: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let draw = match ChunkDrawResources::new(
            &surface.device,
            surface.config.format,
            surface.config.width,
            surface.config.height,
            &self.mesh,
        ) {
            Ok(draw) => draw,
            Err(err) => {
                log::error!("failed to initialize chunk draw resources: {err:#}");
                event_loop.exit();
                return;
            }
        };
        log::info!("uploaded chunk mesh with {} indices", draw.index_count());
        self.draw = Some(draw);
        self.surface = Some(surface);
        self.window = Some(window);
        self.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(surface) = &mut self.surface {
                    surface.resize(size);
                }
                if let (Some(surface), Some(draw)) = (&self.surface, &mut self.draw) {
                    draw.resize(&surface.device, surface.config.width, surface.config.height);
                }
                self.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key_code) = event.physical_key {
                    if key_code == KeyCode::Escape {
                        event_loop.exit();
                        return;
                    }
                    match event.state {
                        ElementState::Pressed => {
                            self.pressed_keys.insert(key_code);
                        }
                        ElementState::Released => {
                            self.pressed_keys.remove(&key_code);
                        }
                    }
                    self.request_redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left || button == MouseButton::Right {
                    self.orbit_dragging = state == ElementState::Pressed;
                    self.last_cursor = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let cursor = (position.x, position.y);
                if self.orbit_dragging {
                    if let Some(previous) = self.last_cursor {
                        let dx = (cursor.0 - previous.0) as f32;
                        let dy = (cursor.1 - previous.1) as f32;
                        self.camera.orbit(-dx * 0.006, -dy * 0.004);
                        self.request_redraw();
                    }
                    self.last_cursor = Some(cursor);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 0.12,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 * 0.001,
                };
                self.camera.zoom(amount);
                self.request_redraw();
            }
            WindowEvent::Focused(false) => {
                self.pressed_keys.clear();
                self.orbit_dragging = false;
                self.last_cursor = None;
            }
            WindowEvent::RedrawRequested => {
                self.update_camera_from_keys();
                let result = {
                    let (Some(surface), Some(draw)) = (&mut self.surface, &mut self.draw) else {
                        return;
                    };
                    surface.render_with(|_device, queue, encoder, view, size| {
                        draw.render(
                            queue,
                            encoder,
                            view,
                            size,
                            self.camera,
                            mclone_render::default_clear_color(),
                        )
                    })
                };
                match result {
                    Ok(SurfaceFrameStatus::Presented | SurfaceFrameStatus::Skipped) => {}
                    Ok(SurfaceFrameStatus::Reconfigured) => self.request_redraw(),
                    Err(err) => {
                        log::error!("render failed: {err:#}");
                        event_loop.exit();
                        return;
                    }
                }
                self.request_redraw();
            }
            _ => {}
        }
    }
}

fn key_axis(
    keys: &std::collections::HashSet<KeyCode>,
    positive: KeyCode,
    negative: KeyCode,
) -> f32 {
    let positive = keys.contains(&positive) as i32;
    let negative = keys.contains(&negative) as i32;
    (positive - negative) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{BlockStateId, ChunkRevision, ChunkStatus};

    #[test]
    fn cli_defaults_to_window() {
        assert_eq!(
            Cli::parse([]).unwrap(),
            Cli::Window {
                scene: SceneOptions::default()
            }
        );
    }

    #[test]
    fn cli_parses_headless_clear_dimensions_after_path() {
        let cli = Cli::parse([
            "--headless-clear".to_owned(),
            "/tmp/mclone.png".to_owned(),
            "--width".to_owned(),
            "32".to_owned(),
            "--height".to_owned(),
            "16".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessClear {
                path: PathBuf::from("/tmp/mclone.png"),
                width: 32,
                height: 16,
            }
        );
    }

    #[test]
    fn cli_parses_dimensions_before_headless_path() {
        let cli = Cli::parse([
            "--width".to_owned(),
            "32".to_owned(),
            "--height".to_owned(),
            "16".to_owned(),
            "--headless-clear".to_owned(),
            "/tmp/mclone.png".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessClear {
                path: PathBuf::from("/tmp/mclone.png"),
                width: 32,
                height: 16,
            }
        );
    }

    #[test]
    fn cli_parses_headless_chunk_scene_options() {
        let cli = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
            "--seed".to_owned(),
            "-9".to_owned(),
            "--chunk-x".to_owned(),
            "2".to_owned(),
            "--chunk-z".to_owned(),
            "-3".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunk {
                path: PathBuf::from("/tmp/mclone-chunk.png"),
                width: 640,
                height: 480,
                scene: SceneOptions {
                    seed: -9,
                    chunk_x: 2,
                    chunk_z: -3,
                    chunk_radius: DEFAULT_CHUNK_RADIUS,
                },
            }
        );
    }

    #[test]
    fn cli_parses_chunk_radius() {
        let cli = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
            "--chunk-radius".to_owned(),
            "2".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunk {
                path: PathBuf::from("/tmp/mclone-chunk.png"),
                width: 640,
                height: 480,
                scene: SceneOptions {
                    chunk_radius: 2,
                    ..SceneOptions::default()
                },
            }
        );
    }

    #[test]
    fn scene_chunk_positions_cover_square_radius() {
        let positions = SceneOptions {
            seed: 0,
            chunk_x: -2,
            chunk_z: 3,
            chunk_radius: 1,
        }
        .chunk_positions()
        .collect::<Vec<_>>();

        assert_eq!(positions.len(), 9);
        assert!(positions.contains(&(-3, 2)));
        assert!(positions.contains(&(-2, 3)));
        assert!(positions.contains(&(-1, 4)));
    }

    #[test]
    fn scene_client_runtime_loads_center_chunk_from_integrated_server() {
        let scene = SceneOptions {
            chunk_radius: 0,
            ..SceneOptions::default()
        };
        let client = build_scene_client_runtime(scene).unwrap();

        assert_eq!(client.loaded_chunk_count(), 1);
        assert!(
            client
                .chunk_snapshot(ChunkPos::new(scene.chunk_x, scene.chunk_z))
                .is_some()
        );
    }

    #[test]
    fn build_scene_mesh_uses_client_runtime_snapshots() {
        let mesh = build_scene_mesh(SceneOptions {
            chunk_radius: 0,
            ..SceneOptions::default()
        })
        .unwrap();

        assert!(!mesh.is_empty());
    }

    #[test]
    fn snapshot_mesh_blocks_rehydrates_omitted_air_sections() {
        let mut block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2];
        block_state_ids[CHUNK_SECTION_VOLUME] = BlockStateId(1);
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            32,
            &block_state_ids,
        );

        let mesh_blocks = snapshot_mesh_blocks(&snapshot).unwrap();

        assert_eq!(mesh_blocks.blocks.len(), CHUNK_SECTION_VOLUME * 2);
        assert_eq!(mesh_blocks.blocks[0], 0);
        assert_eq!(mesh_blocks.blocks[CHUNK_SECTION_VOLUME], 1);
    }
}
