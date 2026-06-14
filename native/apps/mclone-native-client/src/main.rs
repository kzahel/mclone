use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use mclone_assets::{
    AssetSource, BlockModelLibrary, BlockStateAssetIndex, BlockStateRegistry,
    FilesystemAssetSource, TextureAtlasPlan,
};
use mclone_client::{ClientHost, ClientRuntime};
use mclone_core::{
    AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkPos, ChunkSnapshot, SECTION_HEIGHT,
};
use mclone_mesh::{
    TexturedChunkMeshInput, TexturedMeshCatalog, TexturedRenderSectionMesh,
    build_textured_render_sections,
};
use mclone_net::{LocalTransport, request_server_updates};
use mclone_protocol::{ChunkInterest, ServerUpdate};
use mclone_render::chunk::{ChunkCamera, ChunkTextureAtlas, TexturedSectionDrawResources};
use mclone_render::headless::{
    HeadlessChunkOptions, HeadlessClearOptions, write_headless_clear_png,
    write_headless_textured_sections_png,
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
            let scene_mesh = build_scene_textured_sections(&scene)?;
            let report = write_headless_textured_sections_png(
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
                &scene_mesh.sections,
                scene_mesh.atlas.as_upload(),
            )?;
            println!(
                "headless textured sections saved to {} ({}x{}, {} bytes, {} vertices, {} indices)",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len,
                report.vertex_count,
                report.index_count
            );
            Ok(())
        }
        Cli::HeadlessChunkScenarios {
            directory,
            width,
            height,
            scene,
        } => {
            let scene_mesh = build_scene_textured_sections(&scene)?;
            let reports =
                write_headless_chunk_scenarios(&directory, width, height, &scene, &scene_mesh)?;
            for report in reports {
                println!(
                    "headless textured scenario saved to {} ({}x{}, {} bytes, {} vertices, {} indices)",
                    report.path.display(),
                    report.width,
                    report.height,
                    report.byte_len,
                    report.vertex_count,
                    report.index_count
                );
            }
            Ok(())
        }
        Cli::Window { scene } => run_window(scene),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SceneOptions {
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    chunk_radius: i32,
    remote_addr: Option<String>,
}

impl Default for SceneOptions {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SEED,
            chunk_x: DEFAULT_CHUNK_X,
            chunk_z: DEFAULT_CHUNK_Z,
            chunk_radius: DEFAULT_CHUNK_RADIUS,
            remote_addr: None,
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
    HeadlessChunkScenarios {
        directory: PathBuf,
        width: u32,
        height: u32,
        scene: SceneOptions,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HeadlessMode {
    Clear(PathBuf),
    Chunk(PathBuf),
    ChunkScenarios(PathBuf),
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
                "--headless-chunk-scenarios" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-chunk-scenarios requires an output directory")?;
                    set_headless_mode(&mut mode, HeadlessMode::ChunkScenarios(path))?;
                }
                "--width" => width = Some(parse_u32_arg("--width", args.next())?),
                "--height" => height = Some(parse_u32_arg("--height", args.next())?),
                "--seed" => scene.seed = parse_i64_arg("--seed", args.next())?,
                "--chunk-x" => scene.chunk_x = parse_i32_arg("--chunk-x", args.next())?,
                "--chunk-z" => scene.chunk_z = parse_i32_arg("--chunk-z", args.next())?,
                "--chunk-radius" => {
                    scene.chunk_radius = parse_chunk_radius_arg("--chunk-radius", args.next())?
                }
                "--remote-addr" => {
                    scene.remote_addr =
                        Some(args.next().context("--remote-addr requires HOST:PORT")?);
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
            Some(HeadlessMode::ChunkScenarios(directory)) => Ok(Self::HeadlessChunkScenarios {
                directory,
                width: width.unwrap_or(960),
                height: height.unwrap_or(640),
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
           mclone-native-client [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--chunk-radius 1] [--remote-addr 127.0.0.1:25565]\n\
           mclone-native-client --headless-clear /tmp/mclone-native-clear.png [--width 96] [--height 64]\n\
           mclone-native-client --headless-chunk /tmp/mclone-native-chunk.png [--width 640] [--height 480] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--chunk-radius 1] [--remote-addr 127.0.0.1:25565]\n\
           mclone-native-client --headless-chunk-scenarios /tmp/mclone-native-camera [--width 960] [--height 640] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--chunk-radius 1]\n\n\
         Window mode renders generated chunks with WASD/QE, mouse drag, and wheel camera controls. Headless modes write PNGs for GPU validation."
    );
}

fn run_window(scene: SceneOptions) -> Result<()> {
    let scene_mesh = build_scene_textured_sections(&scene)?;
    let chunk_count = scene.chunk_span() * scene.chunk_span();
    log::info!(
        "client-runtime textured chunk area seed={} center=({}, {}) radius={} remote={:?} chunks={} sections={} vertices={} indices={} atlas={}x{}",
        scene.seed,
        scene.chunk_x,
        scene.chunk_z,
        scene.chunk_radius,
        scene.remote_addr,
        chunk_count,
        scene_mesh.section_count(),
        scene_mesh.vertex_count(),
        scene_mesh.index_count(),
        scene_mesh.atlas.width,
        scene_mesh.atlas.height
    );

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ChunkApp::new(
        scene_mesh,
        ChunkCamera::overview_for_chunk_area(scene.chunk_x, scene.chunk_z, scene.chunk_radius),
    );
    event_loop.run_app(&mut app)?;
    Ok(())
}

#[derive(Clone, Debug)]
struct SceneTexturedSections {
    sections: Vec<TexturedRenderSectionMesh>,
    atlas: TextureAtlasImage,
}

impl SceneTexturedSections {
    fn section_count(&self) -> usize {
        self.sections.len()
    }

    fn vertex_count(&self) -> u32 {
        self.sections
            .iter()
            .map(|section| section.stats().vertex_count)
            .sum()
    }

    fn index_count(&self) -> u32 {
        self.sections
            .iter()
            .map(|section| section.stats().index_count)
            .sum()
    }
}

#[derive(Clone, Debug)]
struct TextureAtlasImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl TextureAtlasImage {
    fn as_upload(&self) -> ChunkTextureAtlas<'_> {
        ChunkTextureAtlas {
            width: self.width,
            height: self.height,
            rgba: &self.rgba,
        }
    }
}

fn build_scene_textured_sections(scene: &SceneOptions) -> Result<SceneTexturedSections> {
    let client = build_scene_client_runtime(scene)?;
    let chunks = client
        .chunk_snapshots()
        .map(snapshot_mesh_block_state_ids)
        .collect::<Result<Vec<_>>>()?;
    let mesh_assets = load_textured_mesh_assets()?;
    let inputs = chunks
        .iter()
        .map(|chunk| {
            TexturedChunkMeshInput::new(
                chunk.chunk_x,
                chunk.chunk_z,
                chunk.min_y,
                chunk.height,
                &chunk.blocks,
            )
        })
        .collect::<Vec<_>>();
    let sections = build_textured_render_sections(&inputs, &mesh_assets.catalog)?;
    if sections.is_empty() {
        bail!(
            "generated chunk area seed={} center=({}, {}) radius={} produced no textured render sections",
            scene.seed,
            scene.chunk_x,
            scene.chunk_z,
            scene.chunk_radius
        );
    }
    Ok(SceneTexturedSections {
        sections,
        atlas: mesh_assets.atlas,
    })
}

fn write_headless_chunk_scenarios(
    directory: &Path,
    width: u32,
    height: u32,
    scene: &SceneOptions,
    scene_mesh: &SceneTexturedSections,
) -> Result<Vec<mclone_render::headless::HeadlessChunkReport>> {
    let mut reports = Vec::new();
    for (name, camera) in chunk_capture_scenarios(scene) {
        let path = directory.join(format!("{name}.png"));
        reports.push(write_headless_textured_sections_png(
            HeadlessChunkOptions {
                path,
                width,
                height,
                color: mclone_render::default_clear_color(),
                camera,
            },
            &scene_mesh.sections,
            scene_mesh.atlas.as_upload(),
        )?);
    }
    Ok(reports)
}

fn chunk_capture_scenarios(scene: &SceneOptions) -> [(&'static str, ChunkCamera); 3] {
    let overview =
        ChunkCamera::overview_for_chunk_area(scene.chunk_x, scene.chunk_z, scene.chunk_radius);
    let mut orbit = overview;
    orbit.orbit(0.7, -0.16);
    let mut close = overview;
    close.zoom(0.45);
    close.orbit(-0.32, 0.08);
    [
        ("overview", overview),
        ("orbit-east", orbit),
        ("close", close),
    ]
}

fn build_scene_client_runtime(scene: &SceneOptions) -> Result<ClientRuntime> {
    let mut client = if scene.remote_addr.is_some() {
        ClientRuntime::new(ClientHost::RemoteDedicated)
    } else {
        ClientRuntime::local_integrated()
    };
    let radius_chunks =
        u32::try_from(scene.chunk_radius).context("chunk radius must be positive")?;

    let command = client.set_chunk_interest(ChunkInterest {
        center: ChunkPos::new(scene.chunk_x, scene.chunk_z),
        radius_chunks,
    });

    if let Some(remote_addr) = &scene.remote_addr {
        let updates = request_server_updates(remote_addr.as_str(), &command)
            .with_context(|| format!("failed to request chunk updates from {remote_addr}"))?;
        client.apply_updates(updates);
    } else {
        let mut server = IntegratedServer::new(scene.seed);
        let mut transport = LocalTransport::new();
        transport.send_client_command(command);

        for command in transport.drain_client_commands() {
            for update in server.handle_command(command) {
                transport.send_server_update(update);
            }
        }
        for update in poll_integrated_server_until_idle(&mut server)? {
            transport.send_server_update(update);
        }
        client.apply_updates(transport.drain_server_updates());
    }
    Ok(client)
}

fn poll_integrated_server_until_idle(server: &mut IntegratedServer) -> Result<Vec<ServerUpdate>> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut updates = Vec::new();

    loop {
        updates.extend(
            server
                .try_poll()
                .context("failed to poll integrated server worldgen jobs")?,
        );
        if server.pending_job_count() == 0 {
            return Ok(updates);
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for integrated server worldgen jobs");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[derive(Clone, Debug)]
struct TexturedMeshAssets {
    catalog: TexturedMeshCatalog,
    atlas: TextureAtlasImage,
}

fn load_textured_mesh_assets() -> Result<TexturedMeshAssets> {
    let root = extracted_asset_root();
    if !root.exists() {
        bail!(
            "missing extracted Minecraft assets at {}; run ./scripts/decompile-mc.sh from the repository root",
            root.display()
        );
    }

    let source = FilesystemAssetSource::new(root);
    let registry = BlockStateRegistry::terrain_mvp();
    let blockstates = BlockStateAssetIndex::load_namespace(&source, "minecraft")
        .context("failed to load vanilla blockstate assets")?;
    registry
        .validate_blockstate_assets(&blockstates)
        .context("terrain MVP registry is not covered by vanilla blockstates")?;
    let selected_model_refs = selected_model_refs(&registry, &blockstates)?;
    let models = BlockModelLibrary::load_model_tree(&source, selected_model_refs.iter().cloned())
        .context("failed to load vanilla block model tree")?;
    let materials = models
        .collect_materials_for_models(selected_model_refs.iter().cloned())
        .context("failed to collect selected block model materials")?;
    let atlas_plan = TextureAtlasPlan::build(&source, materials)
        .context("failed to plan block texture atlas")?;
    let atlas = stitch_texture_atlas(&source, &atlas_plan)?;
    let catalog = TexturedMeshCatalog::from_assets(&registry, &blockstates, &models, &atlas_plan)
        .context("failed to build textured mesh catalog")?;

    Ok(TexturedMeshAssets { catalog, atlas })
}

fn selected_model_refs(
    registry: &BlockStateRegistry,
    blockstates: &BlockStateAssetIndex,
) -> Result<BTreeSet<mclone_assets::ResourceLocation>> {
    let mut refs = BTreeSet::new();
    for record in registry.records() {
        let asset = blockstates
            .get(&record.block)
            .with_context(|| format!("missing blockstate asset for {}", record.block))?;
        let variant_key = record.variant_key();
        if let Some(variants) = asset.variants_for_key(&variant_key) {
            if let Some(variant) = variants.first() {
                refs.insert(variant.model.clone());
            }
        } else if variant_key.is_empty() && !asset.model_refs.is_empty() {
            refs.extend(asset.model_refs.iter().cloned());
        } else {
            bail!(
                "missing blockstate variant `{variant_key}` for {}",
                record.block
            );
        }
    }
    Ok(refs)
}

fn stitch_texture_atlas(
    source: &impl AssetSource,
    plan: &TextureAtlasPlan,
) -> Result<TextureAtlasImage> {
    let width = plan.width();
    let height = plan.height();
    if width == 0 || height == 0 {
        bail!("cannot stitch an empty texture atlas");
    }
    let mut atlas = vec![0; width as usize * height as usize * 4];

    for sprite in plan.sprites() {
        let bytes = source
            .read(&sprite.info.path)?
            .with_context(|| format!("missing texture {}", sprite.info.path))?;
        let image = image::load_from_memory(&bytes)
            .with_context(|| format!("failed to decode texture {}", sprite.info.path))?
            .to_rgba8();
        let (sprite_width, sprite_height) = image.dimensions();
        if sprite_width != sprite.info.width || sprite_height != sprite.info.height {
            bail!(
                "texture {} decoded as {}x{} but atlas plan expected {}x{}",
                sprite.info.path,
                sprite_width,
                sprite_height,
                sprite.info.width,
                sprite.info.height
            );
        }

        let image = image.as_raw();
        for row in 0..sprite.info.height {
            let source_start = (row * sprite.info.width * 4) as usize;
            let source_end = source_start + (sprite.info.width * 4) as usize;
            let dest_start = (((sprite.y + row) * width + sprite.x) * 4) as usize;
            let dest_end = dest_start + (sprite.info.width * 4) as usize;
            atlas[dest_start..dest_end].copy_from_slice(&image[source_start..source_end]);
        }
    }

    Ok(TextureAtlasImage {
        width,
        height,
        rgba: atlas,
    })
}

fn extracted_asset_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../reference/minecraft-1.17.1/extracted")
}

#[derive(Clone, Debug)]
struct MeshChunkBlocks {
    chunk_x: i32,
    chunk_z: i32,
    min_y: i32,
    height: i32,
    blocks: Vec<mclone_core::BlockStateId>,
}

fn snapshot_mesh_block_state_ids(snapshot: &ChunkSnapshot) -> Result<MeshChunkBlocks> {
    if snapshot.height <= 0 || snapshot.height % SECTION_HEIGHT != 0 {
        bail!(
            "chunk snapshot {:?} has invalid height {}",
            snapshot.pos,
            snapshot.height
        );
    }
    let expected_len = snapshot.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
    let mut blocks = vec![AIR_BLOCK_STATE_ID; expected_len];
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
            blocks[start + index] = state_id;
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
    fn chunk_span(&self) -> i32 {
        self.chunk_radius * 2 + 1
    }

    #[cfg(test)]
    fn chunk_positions(&self) -> impl Iterator<Item = (i32, i32)> {
        let min_x = self.chunk_x - self.chunk_radius;
        let max_x = self.chunk_x + self.chunk_radius;
        let min_z = self.chunk_z - self.chunk_radius;
        let max_z = self.chunk_z + self.chunk_radius;
        (min_x..=max_x)
            .flat_map(move |chunk_x| (min_z..=max_z).map(move |chunk_z| (chunk_x, chunk_z)))
    }
}

struct ChunkApp {
    scene_mesh: SceneTexturedSections,
    camera: ChunkCamera,
    window: Option<Arc<Window>>,
    surface: Option<NativeSurfaceContext>,
    draw: Option<TexturedSectionDrawResources>,
    pressed_keys: std::collections::HashSet<KeyCode>,
    orbit_dragging: bool,
    last_cursor: Option<(f64, f64)>,
    last_frame: Instant,
}

impl ChunkApp {
    fn new(scene_mesh: SceneTexturedSections, camera: ChunkCamera) -> Self {
        Self {
            scene_mesh,
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
        let draw = match TexturedSectionDrawResources::new(
            &surface.device,
            &surface.queue,
            surface.config.format,
            surface.config.width,
            surface.config.height,
            &self.scene_mesh.sections,
            self.scene_mesh.atlas.as_upload(),
        ) {
            Ok(draw) => draw,
            Err(err) => {
                log::error!("failed to initialize chunk draw resources: {err:#}");
                event_loop.exit();
                return;
            }
        };
        log::info!(
            "uploaded {} render sections with {} indices",
            draw.section_count(),
            draw.index_count()
        );
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
                    remote_addr: None,
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
    fn cli_parses_headless_chunk_scenarios() {
        let cli = Cli::parse([
            "--headless-chunk-scenarios".to_owned(),
            "/tmp/mclone-camera".to_owned(),
            "--width".to_owned(),
            "320".to_owned(),
            "--height".to_owned(),
            "180".to_owned(),
            "--chunk-radius".to_owned(),
            "2".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunkScenarios {
                directory: PathBuf::from("/tmp/mclone-camera"),
                width: 320,
                height: 180,
                scene: SceneOptions {
                    chunk_radius: 2,
                    ..SceneOptions::default()
                },
            }
        );
    }

    #[test]
    fn cli_parses_remote_addr() {
        let cli = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
            "--remote-addr".to_owned(),
            "127.0.0.1:25565".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunk {
                path: PathBuf::from("/tmp/mclone-chunk.png"),
                width: 640,
                height: 480,
                scene: SceneOptions {
                    remote_addr: Some("127.0.0.1:25565".to_owned()),
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
            remote_addr: None,
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
        let client = build_scene_client_runtime(&scene).unwrap();

        assert_eq!(client.loaded_chunk_count(), 1);
        assert!(
            client
                .chunk_snapshot(ChunkPos::new(scene.chunk_x, scene.chunk_z))
                .is_some()
        );
    }

    #[test]
    fn scene_client_runtime_loads_center_chunk_from_remote_server() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let command = mclone_net::read_client_command_frame(&mut stream).unwrap();
            let mut server = IntegratedServer::new(DEFAULT_SEED);
            let mut updates = server.try_handle_command(command).unwrap();
            updates.extend(poll_integrated_server_until_idle(&mut server).unwrap());
            mclone_net::write_server_update_batch(&mut stream, &updates).unwrap();
        });
        let scene = SceneOptions {
            chunk_radius: 0,
            remote_addr: Some(addr.to_string()),
            ..SceneOptions::default()
        };

        let client = build_scene_client_runtime(&scene).unwrap();
        server.join().unwrap();

        assert_eq!(client.host(), ClientHost::RemoteDedicated);
        assert_eq!(client.loaded_chunk_count(), 1);
        assert!(
            client
                .chunk_snapshot(ChunkPos::new(scene.chunk_x, scene.chunk_z))
                .is_some()
        );
    }

    #[test]
    fn build_scene_textured_sections_uses_client_runtime_snapshots() {
        if !extracted_asset_root().exists() {
            return;
        }
        let scene_mesh = build_scene_textured_sections(&SceneOptions {
            chunk_radius: 0,
            ..SceneOptions::default()
        })
        .unwrap();

        assert!(scene_mesh.section_count() > 0);
        assert!(scene_mesh.index_count() > 0);
        assert!(scene_mesh.atlas.width > 0);
        assert!(scene_mesh.atlas.height > 0);
    }

    #[test]
    fn snapshot_mesh_block_state_ids_rehydrates_omitted_air_sections() {
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

        let mesh_blocks = snapshot_mesh_block_state_ids(&snapshot).unwrap();

        assert_eq!(mesh_blocks.blocks.len(), CHUNK_SECTION_VOLUME * 2);
        assert_eq!(mesh_blocks.blocks[0], AIR_BLOCK_STATE_ID);
        assert_eq!(mesh_blocks.blocks[CHUNK_SECTION_VOLUME], BlockStateId(1));
    }
}
