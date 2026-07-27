#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod browser_exact;
#[cfg(target_arch = "wasm32")]
mod browser_vegetation;
mod canonical;
mod canonical_batch_codec;
mod canonical_mesh;
mod clipmap;
mod composition;
mod horizon_admission;
mod runtime_exact;
mod runtime_session;
mod source;
mod terrain_vegetation_coordinator;
mod tree_ownership;
mod viewport;
mod viewport_renderer;

use std::fmt::Write;
use std::num::NonZeroU64;
use std::sync::mpsc;

use mclone_worldgen::levelgen::{
    MCLONE_OVERWORLD_GROVE_DOMAIN, MCLONE_OVERWORLD_GROVE_SCALE_BLOCKS,
    MCLONE_OVERWORLD_LARGE_FIELD_SPEC, MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldLargeFieldBand,
    McloneOverworldSampler, VanillaOverworldLodSampler,
};
use mclone_worldgen::terrain_preview::{
    TERRAIN_PREVIEW_SAMPLE_FLOATS, TerrainPreviewComparison, TerrainPreviewReferenceGrid,
    TerrainPreviewSample, ValidatedTerrainPreviewRequest,
};

#[cfg(target_arch = "wasm32")]
pub use browser_exact::{BrowserCanonicalExactExecutor, canonical_mesh_batch_encoded_bytes};
#[cfg(target_arch = "wasm32")]
pub use browser_vegetation::{
    BrowserTerrainVegetationExecutor, TerrainVegetationWorkerActor, TerrainVegetationWorkerDispatch,
};
pub use canonical::{
    CANONICAL_TERRAIN_MAX_CHUNK_RADIUS, CanonicalTerrainChunk, CanonicalTerrainCompiler,
    CanonicalTerrainDependencyCacheReport, CanonicalTerrainStage, CanonicalTerrainVisibility,
    canonical_terrain_chunk_order, canonical_terrain_presentation_blocks,
};
pub use canonical_batch_codec::{
    CanonicalEncodedAdmission, CanonicalEncodedBatch, CanonicalEncodedNaturalTree,
    decode_canonical_batch, encode_canonical_batch, patch_canonical_batch_transfer_ms,
};
pub use canonical_mesh::{
    CanonicalMeshBatch, CanonicalMeshCoordinate, CanonicalMeshFrontier,
    CanonicalMeshRequestReceipt, CanonicalMeshSession, CanonicalNaturalTreePresentation,
    CanonicalPackedAdmission, CanonicalPackedNaturalTree, suppress_missing_footprint_walls,
};
pub use clipmap::{
    TERRAIN_CLIPMAP_DEFAULT_LEVEL_COUNT, TERRAIN_CLIPMAP_DEFAULT_TILES_PER_AXIS, TerrainClipmap,
    TerrainClipmapBounds, TerrainClipmapConfig, TerrainClipmapDiagnostics,
    TerrainClipmapLevelSnapshot, TerrainClipmapTile, TerrainClipmapUpdate,
};
pub use composition::{
    BoundedRepresentationBounds, BoundedRepresentationOwner,
    BoundedRepresentationOwnershipSnapshot, BoundedRepresentationReadiness,
    BoundedRepresentationUnit, ExactPaintedCoverageSnapshot, TERRAIN_EXACT_COVERAGE_MASK_BYTES,
    TERRAIN_EXACT_COVERAGE_MAX_CHUNKS_PER_AXIS, TERRAIN_EXACT_COVERAGE_WORD_COUNT,
    TERRAIN_EXACT_FRONTIER_COLLAR_BLOCKS, TerrainCompositionSourceIdentity,
    TerrainExactCoverageMask, TerrainExactCoverageMode,
};
pub use runtime_exact::{
    CanonicalExactExecutor, CanonicalExactRequest, CanonicalExactResult,
    TerrainRuntimeExactRenderer, TerrainRuntimeExactStats,
};
pub use runtime_session::{
    TerrainRuntimeCompositionMode, TerrainRuntimeConfig, TerrainRuntimeExactAnchor,
    TerrainRuntimeExactView, TerrainRuntimeSession,
};
pub use source::{TerrainPreparedExactFrame, TerrainViewSourceIdentity, TerrainViewTruthRole};
pub use terrain_vegetation_coordinator::{
    TerrainVegetationAdmission, TerrainVegetationCoordinator,
    TerrainVegetationCoordinatorDiagnostics, TerrainVegetationCoordinatorState,
    TerrainVegetationDesiredTile, TerrainVegetationExecutor, TerrainVegetationExecutorActor,
    TerrainVegetationExecutorDiagnostics, TerrainVegetationExecutorEvent,
    TerrainVegetationExecutorJob, TerrainVegetationExecutorKind, TerrainVegetationJobIdentity,
    TerrainVegetationSlotToken, TerrainVegetationSubmitError,
};
pub use tree_ownership::{
    McloneTreeOccurrenceId, McloneTreeOwnershipCandidate, mclone_tree_ownership_snapshot,
    mclone_tree_working_bounds,
};
pub use viewport::{
    TERRAIN_VIEWPORT_AUTO_PIXELS_PER_CELL, TERRAIN_VIEWPORT_MAX_BLOCKS_ACROSS,
    TERRAIN_VIEWPORT_MAX_DIAGNOSTIC_TILES_PER_AXIS, TERRAIN_VIEWPORT_MAX_VISIBLE_TILES_PER_AXIS,
    TERRAIN_VIEWPORT_MIN_BLOCKS_ACROSS, TERRAIN_VIEWPORT_PRELOAD_MARGIN_TILES,
    TerrainViewportDetail, TerrainViewportLevel, TerrainViewportPlan, TerrainViewportRequest,
    TerrainViewportTileId, plan_terrain_viewport,
};
pub use viewport_renderer::{
    TERRAIN_PREVIEW_MATERIAL_UV_COUNT, TerrainHorizonFrameStats, TerrainHorizonRenderTarget,
    TerrainHorizonRenderer, TerrainHorizonVegetationServiceStats, TerrainPreviewMaterialAtlas,
    TerrainViewportCompletedComparison, TerrainViewportExternalCpuRequest,
    TerrainViewportFrameStats, TerrainViewportRenderer,
};

pub const TERRAIN_PREVIEW_GPU_EVALUATOR_REVISION: &str = "mclone-overworld-v1-gpu-preview-a9";
pub const TERRAIN_PREVIEW_COMPUTE_WGSL_TEMPLATE: &str =
    include_str!("shaders/terrain_preview_compute.wgsl");
pub const TERRAIN_PREVIEW_RENDER_WGSL: &str = include_str!("shaders/terrain_preview_render.wgsl");
pub const TERRAIN_PREVIEW_TREE_WGSL: &str = include_str!("shaders/terrain_preview_tree.wgsl");

const TERRAIN_PREVIEW_UNIFORM_BYTES: u64 = 224;
const TERRAIN_PREVIEW_SAMPLE_BYTES: u64 =
    (TERRAIN_PREVIEW_SAMPLE_FLOATS * std::mem::size_of::<f32>()) as u64;
const TERRAIN_PREVIEW_WORKGROUP_AXIS: u32 = 8;
const TERRAIN_PREVIEW_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

const fn terrain_horizon_orbit_target_y() -> f32 {
    MCLONE_OVERWORLD_SEA_LEVEL as f32
}

pub fn terrain_preview_compute_wgsl() -> String {
    let spec = MCLONE_OVERWORLD_LARGE_FIELD_SPEC;
    let mut constants = String::new();
    for (name, band) in [
        ("CONTINENT_LARGE", spec.continent[0]),
        ("CONTINENT_MEDIUM", spec.continent[1]),
        ("CONTINENT_DETAIL", spec.continent[2]),
        ("COAST", spec.coast),
        ("RELIEF_LARGE", spec.relief[0]),
        ("RELIEF_DETAIL", spec.relief[1]),
        ("RELIEF_FINE", spec.relief[2]),
        ("RUGGEDNESS_LARGE", spec.ruggedness[0]),
        ("RUGGEDNESS_DETAIL", spec.ruggedness[1]),
        ("RIDGE_LARGE", spec.ridge[0]),
        ("RIDGE_DETAIL", spec.ridge[1]),
        ("MOUNTAIN_DETAIL_LARGE", spec.mountain_detail[0]),
        ("MOUNTAIN_DETAIL_FINE", spec.mountain_detail[1]),
        ("RIVER_LARGE", spec.river[0]),
        ("RIVER_DETAIL", spec.river[1]),
        ("RIVER_WIDTH", spec.river[2]),
        ("RIVER_REACH", spec.river[3]),
        ("RIVER_MORPHOLOGY_DETAIL", spec.river[4]),
        ("WETLAND_POOL", spec.wetland_pool),
        ("OCEAN_BASIN", spec.ocean_basin),
        ("SEABED_LARGE", spec.seabed[0]),
        ("SEABED_DETAIL", spec.seabed[1]),
        ("TEMPERATURE_LARGE", spec.temperature[0]),
        ("TEMPERATURE_DETAIL", spec.temperature[1]),
        ("MOISTURE_LARGE", spec.moisture[0]),
        ("MOISTURE_DETAIL", spec.moisture[1]),
    ] {
        write_field_constants(&mut constants, name, band);
    }
    let grove_low = MCLONE_OVERWORLD_GROVE_DOMAIN as u32;
    let grove_high = (MCLONE_OVERWORLD_GROVE_DOMAIN >> 32) as u32;
    writeln!(
        constants,
        "const GROVE_DOMAIN: U64 = U64(0x{grove_low:08x}u, 0x{grove_high:08x}u);"
    )
    .expect("writing Terrain Lab grove WGSL domain to String cannot fail");
    writeln!(
        constants,
        "const GROVE_SCALE: i32 = {MCLONE_OVERWORLD_GROVE_SCALE_BLOCKS};"
    )
    .expect("writing Terrain Lab grove WGSL scale to String cannot fail");
    TERRAIN_PREVIEW_COMPUTE_WGSL_TEMPLATE
        .replace("// __MCLONE_PRODUCTION_FIELD_CONSTANTS__", &constants)
}

pub fn terrain_preview_render_wgsl(
    transform: mclone_render_color::RenderTargetColorTransform,
) -> String {
    mclone_render_color::inject_target_color_transform_wgsl(TERRAIN_PREVIEW_RENDER_WGSL, transform)
        .expect("terrain preview render WGSL has one color transfer and transform marker")
}

pub fn terrain_preview_tree_wgsl(
    transform: mclone_render_color::RenderTargetColorTransform,
) -> String {
    mclone_render_color::inject_target_color_transform_wgsl(TERRAIN_PREVIEW_TREE_WGSL, transform)
        .expect("terrain preview tree WGSL has one color transfer and transform marker")
}

fn write_field_constants(
    destination: &mut String,
    name: &str,
    band: McloneOverworldLargeFieldBand,
) {
    let low = band.domain as u32;
    let high = (band.domain >> 32) as u32;
    writeln!(
        destination,
        "const {name}_DOMAIN: U64 = U64(0x{low:08x}u, 0x{high:08x}u);"
    )
    .expect("writing terrain preview WGSL constants to String cannot fail");
    writeln!(destination, "const {name}_SCALE: i32 = {};", band.scale)
        .expect("writing terrain preview WGSL constants to String cannot fail");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TerrainPreviewSource {
    Gpu = 0,
    Reference = 1,
    Split = 2,
    Macro = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TerrainPreviewView {
    Map = 0,
    ThreeDimensional = 1,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u32)]
pub enum TerrainPreviewProjectionKind {
    #[default]
    Orthographic = 0,
    Perspective = 1,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u32)]
pub enum TerrainPreviewSplitLayout {
    #[default]
    Columns = 0,
    Rows = 1,
}

impl TerrainPreviewProjectionKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Orthographic => "orthographic",
            Self::Perspective => "perspective",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TerrainPreviewLayer {
    Terrain = 0,
    Height = 1,
    Error = 2,
    Continentalness = 3,
    Climate = 4,
    Rivers = 5,
    Wetlands = 6,
    Biomes = 7,
    SurfaceRecipe = 8,
    PlannedStreams = 9,
    Landforms = 10,
    Forests = 11,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainPreviewDrawOptions {
    pub source: TerrainPreviewSource,
    pub view: TerrainPreviewView,
    pub layer: TerrainPreviewLayer,
    pub split_layout: TerrainPreviewSplitLayout,
}

impl Default for TerrainPreviewDrawOptions {
    fn default() -> Self {
        Self {
            source: TerrainPreviewSource::Reference,
            view: TerrainPreviewView::ThreeDimensional,
            layer: TerrainPreviewLayer::Terrain,
            split_layout: TerrainPreviewSplitLayout::Columns,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainPreviewCamera {
    pub yaw_radians: f32,
    pub pitch_radians: f32,
    pub projection: TerrainPreviewProjectionKind,
}

impl TerrainPreviewCamera {
    pub fn new(
        yaw_radians: f32,
        pitch_radians: f32,
        projection: TerrainPreviewProjectionKind,
    ) -> Result<Self, String> {
        if !yaw_radians.is_finite() || !pitch_radians.is_finite() {
            return Err("terrain preview camera angles must be finite".to_owned());
        }
        Ok(Self {
            yaw_radians,
            pitch_radians: pitch_radians.clamp(0.12, 1.25),
            projection,
        })
    }
}

impl Default for TerrainPreviewCamera {
    fn default() -> Self {
        Self {
            yaw_radians: std::f32::consts::FRAC_PI_4,
            pitch_radians: 0.48,
            projection: TerrainPreviewProjectionKind::Orthographic,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainHorizonPresentation {
    pub center_x: f64,
    pub center_z: f64,
    pub width_blocks: f64,
    pub height_blocks: f64,
    pub view: TerrainPreviewView,
    pub camera: TerrainPreviewCamera,
    pub target_y: f32,
}

impl TerrainHorizonPresentation {
    pub fn new(
        center_x: f64,
        center_z: f64,
        width_blocks: f64,
        height_blocks: f64,
        view: TerrainPreviewView,
        camera: TerrainPreviewCamera,
    ) -> Result<Self, String> {
        let presentation = Self {
            center_x,
            center_z,
            width_blocks,
            height_blocks,
            view,
            camera,
            target_y: terrain_horizon_orbit_target_y(),
        };
        presentation.uniform_facts()?;
        Ok(presentation)
    }

    pub fn with_target_y(mut self, target_y: f32) -> Result<Self, String> {
        if !target_y.is_finite() {
            return Err("terrain horizon camera target height must be finite".to_owned());
        }
        self.target_y = target_y;
        Ok(self)
    }

    fn uniform_facts(self) -> Result<TerrainPreviewUniformPresentation, String> {
        TerrainPreviewUniformPresentation::continuous(
            self.center_x,
            self.center_z,
            self.width_blocks,
            self.height_blocks,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TerrainPreviewUniformPresentation {
    anchor_x: i32,
    anchor_z: i32,
    fraction_x: f32,
    fraction_z: f32,
    width_blocks: f32,
    height_blocks: f32,
}

impl TerrainPreviewUniformPresentation {
    fn integer(center_x: i32, center_z: i32, width_blocks: u32, height_blocks: u32) -> Self {
        Self {
            anchor_x: center_x,
            anchor_z: center_z,
            fraction_x: 0.0,
            fraction_z: 0.0,
            width_blocks: width_blocks.max(1) as f32,
            height_blocks: height_blocks.max(1) as f32,
        }
    }

    fn continuous(
        center_x: f64,
        center_z: f64,
        width_blocks: f64,
        height_blocks: f64,
    ) -> Result<Self, String> {
        if !center_x.is_finite() || !center_z.is_finite() {
            return Err("terrain horizon presentation center must be finite".to_owned());
        }
        if !width_blocks.is_finite()
            || !height_blocks.is_finite()
            || width_blocks <= 0.0
            || height_blocks <= 0.0
            || width_blocks > f64::from(f32::MAX)
            || height_blocks > f64::from(f32::MAX)
        {
            return Err(
                "terrain horizon presentation extent must be finite, positive, and \
                        representable as f32"
                    .to_owned(),
            );
        }
        if center_x < f64::from(i32::MIN)
            || center_x > f64::from(i32::MAX)
            || center_z < f64::from(i32::MIN)
            || center_z > f64::from(i32::MAX)
        {
            return Err("terrain horizon presentation center exceeds i32 world bounds".to_owned());
        }
        let anchor_x = center_x.round() as i32;
        let anchor_z = center_z.round() as i32;
        Ok(Self {
            anchor_x,
            anchor_z,
            fraction_x: (center_x - f64::from(anchor_x)) as f32,
            fraction_z: (center_z - f64::from(anchor_z)) as f32,
            width_blocks: width_blocks as f32,
            height_blocks: height_blocks as f32,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainPreviewProjection {
    pub kind: TerrainPreviewProjectionKind,
    pub eye_offset: [f32; 3],
    pub target_y: f32,
    pub up: [f32; 3],
    pub fov_y_radians: f32,
    pub vertical_half_extent: f32,
    pub z_near: f32,
    pub z_far: f32,
    pub aspect: f32,
}

pub fn terrain_preview_focus_y(seed: i64, world_x: i32, world_z: i32) -> f32 {
    terrain_preview_focus_y_for_profile(
        mclone_worldgen::terrain_preview::TerrainPreviewProfile::McloneOverworldV1,
        seed,
        world_x,
        world_z,
    )
}

pub fn terrain_preview_focus_y_for_profile(
    profile: mclone_worldgen::terrain_preview::TerrainPreviewProfile,
    seed: i64,
    world_x: i32,
    world_z: i32,
) -> f32 {
    if profile == mclone_worldgen::terrain_preview::TerrainPreviewProfile::VanillaOverworld {
        return VanillaOverworldLodSampler::new(seed)
            .sample(world_x, world_z)
            .display_y as f32;
    }
    let terrain = McloneOverworldSampler::new(seed).sample(world_x, world_z);
    let display_y =
        if terrain.watercourse.is_water() || terrain.surface_y < MCLONE_OVERWORLD_SEA_LEVEL {
            terrain.surface_y.max(terrain.watercourse.water_surface_y)
        } else {
            terrain.surface_y
        };
    display_y as f32 + 1.0
}

pub fn terrain_preview_projection(
    blocks_across: u32,
    view: TerrainPreviewView,
    camera: TerrainPreviewCamera,
    panel_width: u32,
    panel_height: u32,
    target_y: f32,
) -> TerrainPreviewProjection {
    terrain_preview_projection_for_extent(
        blocks_across.max(1) as f32,
        view,
        camera,
        panel_width,
        panel_height,
        target_y,
    )
}

pub fn terrain_horizon_chunk_render_view(
    presentation: TerrainHorizonPresentation,
    width: u32,
    height: u32,
) -> Result<mclone_render::chunk::ChunkRenderView, String> {
    let projection = terrain_preview_projection_for_extent(
        presentation.width_blocks as f32,
        presentation.view,
        presentation.camera,
        width,
        height,
        presentation.target_y,
    );
    Ok(terrain_preview_chunk_render_view(
        projection,
        presentation.center_x as f32,
        presentation.center_z as f32,
        width,
        height,
    ))
}

fn terrain_preview_chunk_render_view(
    projection: TerrainPreviewProjection,
    center_x: f32,
    center_z: f32,
    width: u32,
    height: u32,
) -> mclone_render::chunk::ChunkRenderView {
    let camera = mclone_render::chunk::ChunkCamera {
        eye: [
            center_x + projection.eye_offset[0],
            projection.target_y + projection.eye_offset[1],
            center_z + projection.eye_offset[2],
        ],
        target: [center_x, projection.target_y, center_z],
        up: projection.up,
        fov_y_radians: projection.fov_y_radians,
        z_near: projection.z_near,
        z_far: projection.z_far,
    };
    match projection.kind {
        TerrainPreviewProjectionKind::Orthographic => {
            camera.render_orthographic_view(width, height, projection.vertical_half_extent * 2.0)
        }
        TerrainPreviewProjectionKind::Perspective => camera.render_view(width, height),
    }
}

fn terrain_preview_projection_for_extent(
    blocks_across: f32,
    view: TerrainPreviewView,
    camera: TerrainPreviewCamera,
    panel_width: u32,
    panel_height: u32,
    target_y: f32,
) -> TerrainPreviewProjection {
    const OVERVIEW_FOV_Y: f32 = 58.0_f32.to_radians();
    const MAP_CAMERA_CLEARANCE: f32 = 192.0;
    const CLOSE_3D_THRESHOLD_BLOCKS: f32 = 96.0;

    let blocks_across = blocks_across.max(f32::MIN_POSITIVE);
    let aspect = panel_width.max(1) as f32 / panel_height.max(1) as f32;
    let (kind, eye_offset, up, fov_y_radians, vertical_half_extent) = match view {
        TerrainPreviewView::Map => {
            let vertical_blocks = blocks_across / aspect.max(0.2);
            let half_height = vertical_blocks * 0.5;
            let distance = half_height / (OVERVIEW_FOV_Y * 0.5).tan() + MAP_CAMERA_CLEARANCE;
            let fov_y_radians = 2.0 * (half_height / distance).atan();
            (
                TerrainPreviewProjectionKind::Orthographic,
                [0.0, distance, 0.0],
                [0.0, 0.0, -1.0],
                fov_y_radians,
                half_height,
            )
        }
        TerrainPreviewView::ThreeDimensional => {
            let overview_distance = CLOSE_3D_THRESHOLD_BLOCKS * 0.9 + 64.0;
            let (distance, fov_y_radians) = if blocks_across < CLOSE_3D_THRESHOLD_BLOCKS {
                (
                    overview_distance,
                    2.0 * ((OVERVIEW_FOV_Y * 0.5).tan() * blocks_across
                        / CLOSE_3D_THRESHOLD_BLOCKS)
                        .atan(),
                )
            } else {
                (blocks_across * 0.9 + 64.0, OVERVIEW_FOV_Y)
            };
            let horizontal = camera.pitch_radians.cos() * distance;
            (
                camera.projection,
                [
                    camera.yaw_radians.cos() * horizontal,
                    camera.pitch_radians.sin() * distance,
                    -camera.yaw_radians.sin() * horizontal,
                ],
                [0.0, 1.0, 0.0],
                fov_y_radians,
                distance * (fov_y_radians * 0.5).tan(),
            )
        }
    };
    TerrainPreviewProjection {
        kind,
        eye_offset,
        target_y,
        up,
        fov_y_radians,
        vertical_half_extent,
        z_near: 0.1,
        z_far: eye_offset[0].hypot(eye_offset[1]).hypot(eye_offset[2])
            + blocks_across.max(128.0) * 6.0
            + 512.0,
        aspect,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainPreviewFrameStats {
    pub revision: u64,
    pub cells_per_axis: u32,
    pub samples_per_axis: u32,
    pub sample_count: u32,
    pub vertex_count: u32,
    pub footprint_blocks: u32,
    pub reference_bytes: u64,
    pub gpu_sample_bytes: u64,
    pub readback_bytes: u64,
    pub resident_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainPreviewCompletedComparison {
    pub revision: u64,
    pub comparison: TerrainPreviewComparison,
}

pub struct EncodedTerrainPreviewReadback {
    revision: u64,
    readback_buffer: wgpu::Buffer,
    byte_len: u64,
    reference: TerrainPreviewReferenceGrid,
}

struct PendingTerrainPreviewReadback {
    revision: u64,
    readback_buffer: wgpu::Buffer,
    byte_len: u64,
    reference: TerrainPreviewReferenceGrid,
    receiver: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
}

struct TerrainPreviewDepthTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl TerrainPreviewDepthTarget {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_terrain_preview_depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TERRAIN_PREVIEW_DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            _texture: texture,
            view,
            width,
            height,
        }
    }
}

pub struct TerrainPreviewRenderer {
    cells_per_axis: u32,
    samples_per_axis: u32,
    sample_count: u32,
    sample_byte_len: u64,
    uniform_buffer: wgpu::Buffer,
    gpu_sample_buffer: wgpu::Buffer,
    reference_sample_buffer: wgpu::Buffer,
    compute_bind_group: wgpu::BindGroup,
    render_bind_group: wgpu::BindGroup,
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,
    depth: TerrainPreviewDepthTarget,
    pending: Vec<PendingTerrainPreviewReadback>,
    latest_requested_revision: u64,
    stale_result_count: u64,
}

impl TerrainPreviewRenderer {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        cells_per_axis: u32,
    ) -> Result<Self, String> {
        if cells_per_axis == 0 || !cells_per_axis.is_power_of_two() {
            return Err(format!(
                "terrain preview renderer cells per axis must be a non-zero power of two, got \
                 {cells_per_axis}"
            ));
        }
        let samples_per_axis = cells_per_axis
            .checked_add(1)
            .ok_or("terrain preview renderer sample axis overflow")?;
        let sample_count = samples_per_axis
            .checked_mul(samples_per_axis)
            .ok_or("terrain preview renderer sample count overflow")?;
        let sample_byte_len = u64::from(sample_count)
            .checked_mul(TERRAIN_PREVIEW_SAMPLE_BYTES)
            .ok_or("terrain preview renderer buffer size overflow")?;

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_preview_uniforms"),
            size: TERRAIN_PREVIEW_UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let gpu_sample_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_preview_gpu_samples"),
            size: sample_byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let reference_sample_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_preview_reference_samples"),
            size: sample_byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_terrain_preview_compute_layout"),
            entries: &[
                uniform_layout_entry(
                    0,
                    wgpu::ShaderStages::COMPUTE,
                    TERRAIN_PREVIEW_UNIFORM_BYTES,
                ),
                storage_layout_entry(1, wgpu::ShaderStages::COMPUTE, false, sample_byte_len),
            ],
        });
        let render_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_terrain_preview_render_layout"),
            entries: &[
                uniform_layout_entry(
                    0,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    TERRAIN_PREVIEW_UNIFORM_BYTES,
                ),
                storage_layout_entry(
                    1,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    true,
                    sample_byte_len,
                ),
                storage_layout_entry(
                    2,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    true,
                    sample_byte_len,
                ),
            ],
        });
        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_terrain_preview_compute_bind_group"),
            layout: &compute_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu_sample_buffer.as_entire_binding(),
                },
            ],
        });
        let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_terrain_preview_render_bind_group"),
            layout: &render_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu_sample_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: reference_sample_buffer.as_entire_binding(),
                },
            ],
        });

        let compute_shader_source = terrain_preview_compute_wgsl();
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_terrain_preview_compute_shader"),
            source: wgpu::ShaderSource::Wgsl(compute_shader_source.into()),
        });
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_terrain_preview_render_shader"),
            source: wgpu::ShaderSource::Wgsl(
                terrain_preview_render_wgsl(
                    mclone_render_color::RenderTargetColorTransform::Identity,
                )
                .into(),
            ),
        });
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mclone_terrain_preview_compute_pipeline_layout"),
                bind_group_layouts: &[&compute_layout],
                push_constant_ranges: &[],
            });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mclone_terrain_preview_render_pipeline_layout"),
                bind_group_layouts: &[&render_layout],
                push_constant_ranges: &[],
            });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("mclone_terrain_preview_compute_pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("compute_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_terrain_preview_render_pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: Some("vertex_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: TERRAIN_PREVIEW_DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            cells_per_axis,
            samples_per_axis,
            sample_count,
            sample_byte_len,
            uniform_buffer,
            gpu_sample_buffer,
            reference_sample_buffer,
            compute_bind_group,
            render_bind_group,
            compute_pipeline,
            render_pipeline,
            depth: TerrainPreviewDepthTarget::new(device, width, height),
            pending: Vec::new(),
            latest_requested_revision: 0,
            stale_result_count: 0,
        })
    }

    pub const fn stale_result_count(&self) -> u64 {
        self.stale_result_count
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.depth.width == width && self.depth.height == height {
            return;
        }
        self.depth = TerrainPreviewDepthTarget::new(device, width, height);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        revision: u64,
        reference: &TerrainPreviewReferenceGrid,
        options: TerrainPreviewDrawOptions,
        camera: TerrainPreviewCamera,
    ) -> Result<(TerrainPreviewFrameStats, EncodedTerrainPreviewReadback), String> {
        let request = reference.request();
        if request.request().cells_per_axis != self.cells_per_axis
            || request.samples_per_axis() != self.samples_per_axis
            || request.sample_count() != self.sample_count
        {
            return Err(format!(
                "terrain preview grid is {} cells/{} samples but renderer is {} cells/{} samples",
                request.request().cells_per_axis,
                request.sample_count(),
                self.cells_per_axis,
                self.sample_count
            ));
        }
        self.resize(device, width, height);
        self.latest_requested_revision = revision;

        let reference_bytes = reference.packed_bytes();
        if reference_bytes.len() as u64 != self.sample_byte_len {
            return Err(format!(
                "terrain preview reference upload is {} bytes, expected {}",
                reference_bytes.len(),
                self.sample_byte_len
            ));
        }
        queue.write_buffer(&self.reference_sample_buffer, 0, &reference_bytes);
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            &uniform_bytes(reference, width, height, options, camera),
        );

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("mclone_terrain_preview_compute_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &self.compute_bind_group, &[]);
            let workgroups = self
                .samples_per_axis
                .div_ceil(TERRAIN_PREVIEW_WORKGROUP_AXIS);
            pass.dispatch_workgroups(workgroups, workgroups, 1);
        }

        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_preview_readback"),
            size: self.sample_byte_len,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(
            &self.gpu_sample_buffer,
            0,
            &readback_buffer,
            0,
            self.sample_byte_len,
        );

        let instance_count = preview_instance_count(options.source);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mclone_terrain_preview_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.025,
                            g: 0.035,
                            b: 0.055,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, &self.render_bind_group, &[]);
            pass.draw(
                0..self.cells_per_axis * self.cells_per_axis * 6,
                0..instance_count,
            );
        }

        let stats = TerrainPreviewFrameStats {
            revision,
            cells_per_axis: self.cells_per_axis,
            samples_per_axis: self.samples_per_axis,
            sample_count: self.sample_count,
            vertex_count: self.cells_per_axis * self.cells_per_axis * 6 * instance_count,
            footprint_blocks: request.footprint_blocks(),
            reference_bytes: self.sample_byte_len,
            gpu_sample_bytes: self.sample_byte_len,
            readback_bytes: self.sample_byte_len,
            resident_bytes: self.sample_byte_len * 2 + TERRAIN_PREVIEW_UNIFORM_BYTES,
        };
        Ok((
            stats,
            EncodedTerrainPreviewReadback {
                revision,
                readback_buffer,
                byte_len: self.sample_byte_len,
                reference: reference.clone(),
            },
        ))
    }

    pub fn mark_submitted(&mut self, encoded: EncodedTerrainPreviewReadback) {
        let (sender, receiver) = mpsc::channel();
        encoded.readback_buffer.slice(..encoded.byte_len).map_async(
            wgpu::MapMode::Read,
            move |result| {
                let _ = sender.send(result);
            },
        );
        self.pending.push(PendingTerrainPreviewReadback {
            revision: encoded.revision,
            readback_buffer: encoded.readback_buffer,
            byte_len: encoded.byte_len,
            reference: encoded.reference,
            receiver,
        });
    }

    pub fn poll_completed(
        &mut self,
        device: &wgpu::Device,
    ) -> Vec<Result<TerrainPreviewCompletedComparison, String>> {
        let _ = device.poll(wgpu::PollType::Poll);
        let mut completed = Vec::new();
        let mut index = 0;
        while index < self.pending.len() {
            match self.pending[index].receiver.try_recv() {
                Ok(Ok(())) => {
                    let pending = self.pending.remove(index);
                    let result = self.read_completed(&pending);
                    pending.readback_buffer.unmap();
                    if pending.revision == self.latest_requested_revision {
                        completed.push(result.map(|comparison| {
                            TerrainPreviewCompletedComparison {
                                revision: pending.revision,
                                comparison,
                            }
                        }));
                    } else {
                        self.stale_result_count = self.stale_result_count.saturating_add(1);
                    }
                }
                Ok(Err(error)) => {
                    let pending = self.pending.remove(index);
                    completed.push(Err(format!(
                        "terrain preview revision {} readback failed: {error}",
                        pending.revision
                    )));
                }
                Err(mpsc::TryRecvError::Empty) => {
                    index += 1;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let pending = self.pending.remove(index);
                    completed.push(Err(format!(
                        "terrain preview revision {} readback callback disconnected",
                        pending.revision
                    )));
                }
            }
        }
        completed
    }

    fn read_completed(
        &self,
        pending: &PendingTerrainPreviewReadback,
    ) -> Result<TerrainPreviewComparison, String> {
        let mapped = pending
            .readback_buffer
            .slice(..pending.byte_len)
            .get_mapped_range();
        let result = parse_samples(&mapped)
            .and_then(|samples| TerrainPreviewComparison::compare(&pending.reference, &samples));
        drop(mapped);
        result
    }
}

const fn preview_instance_count(source: TerrainPreviewSource) -> u32 {
    match source {
        TerrainPreviewSource::Split => 2,
        TerrainPreviewSource::Gpu
        | TerrainPreviewSource::Reference
        | TerrainPreviewSource::Macro => 1,
    }
}

fn uniform_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    byte_len: u64,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(byte_len),
        },
        count: None,
    }
}

fn storage_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    read_only: bool,
    byte_len: u64,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(byte_len),
        },
        count: None,
    }
}

fn uniform_bytes(
    reference: &TerrainPreviewReferenceGrid,
    width: u32,
    height: u32,
    options: TerrainPreviewDrawOptions,
    camera: TerrainPreviewCamera,
) -> Vec<u8> {
    let request = reference.request();
    viewport_uniform_bytes(
        reference,
        width,
        height,
        options,
        camera,
        request.request().center_x,
        request.request().center_z,
        request.footprint_blocks(),
        request.footprint_blocks(),
    )
}

#[allow(clippy::too_many_arguments)]
fn viewport_uniform_bytes(
    reference: &TerrainPreviewReferenceGrid,
    width: u32,
    height: u32,
    options: TerrainPreviewDrawOptions,
    camera: TerrainPreviewCamera,
    viewport_center_x: i32,
    viewport_center_z: i32,
    viewport_width_blocks: u32,
    viewport_height_blocks: u32,
) -> Vec<u8> {
    let focus_y = terrain_preview_focus_y_for_profile(
        reference.request().request().profile,
        reference.request().request().seed,
        viewport_center_x,
        viewport_center_z,
    );
    viewport_uniform_bytes_for_request(
        reference.request(),
        width,
        height,
        options,
        camera,
        viewport_center_x,
        viewport_center_z,
        viewport_width_blocks,
        viewport_height_blocks,
        focus_y,
    )
}

#[allow(clippy::too_many_arguments)]
fn viewport_uniform_bytes_for_request(
    request: ValidatedTerrainPreviewRequest,
    width: u32,
    height: u32,
    options: TerrainPreviewDrawOptions,
    camera: TerrainPreviewCamera,
    viewport_center_x: i32,
    viewport_center_z: i32,
    viewport_width_blocks: u32,
    viewport_height_blocks: u32,
    focus_y: f32,
) -> Vec<u8> {
    viewport_uniform_bytes_for_request_with_presentation(
        request,
        width,
        height,
        options,
        camera,
        TerrainPreviewUniformPresentation::integer(
            viewport_center_x,
            viewport_center_z,
            viewport_width_blocks,
            viewport_height_blocks,
        ),
        focus_y,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn viewport_uniform_bytes_for_request_with_presentation(
    request: ValidatedTerrainPreviewRequest,
    width: u32,
    height: u32,
    options: TerrainPreviewDrawOptions,
    camera: TerrainPreviewCamera,
    presentation: TerrainPreviewUniformPresentation,
    focus_y: f32,
    inner_hole: Option<clipmap::TerrainClipmapBounds>,
) -> Vec<u8> {
    let source = request.request();
    let seed = source.seed as u64;
    let words = [
        request.min_x() as u32,
        request.min_z() as u32,
        source.sample_spacing,
        source.cells_per_axis,
        seed as u32,
        (seed >> 32) as u32,
        options.source as u32,
        options.view as u32,
        options.layer as u32,
        request.samples_per_axis(),
        width.max(1),
        height.max(1),
    ];
    let (panel_width, panel_height) =
        terrain_preview_panel_size(width, height, options.source, options.split_layout);
    let projection = terrain_preview_projection_for_extent(
        presentation.width_blocks,
        options.view,
        camera,
        panel_width,
        panel_height,
        focus_y,
    );
    let mut bytes = Vec::with_capacity(TERRAIN_PREVIEW_UNIFORM_BYTES as usize);
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    for value in [
        projection.eye_offset[0],
        projection.eye_offset[1],
        projection.eye_offset[2],
        projection.target_y,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in [
        projection.up[0],
        projection.up[1],
        projection.up[2],
        projection.fov_y_radians,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in [
        projection.z_near,
        projection.z_far,
        projection.aspect,
        projection.vertical_half_extent,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in [
        presentation.anchor_x,
        presentation.anchor_z,
        presentation.width_blocks.ceil().min(i32::MAX as f32) as i32,
        presentation.height_blocks.ceil().min(i32::MAX as f32) as i32,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in [
        presentation.fraction_x,
        presentation.fraction_z,
        presentation.width_blocks,
        presentation.height_blocks,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for word in [
        source.content_stage as u32,
        projection.kind as u32,
        options.split_layout as u32,
        match source.profile {
            mclone_worldgen::terrain_preview::TerrainPreviewProfile::McloneOverworldV1 => 0,
            mclone_worldgen::terrain_preview::TerrainPreviewProfile::VanillaOverworld => 1,
        } | ((source.surface_quality as u32) << 1),
    ] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    let hole = inner_hole.unwrap_or(clipmap::TerrainClipmapBounds {
        min_x: 0,
        min_z: 0,
        max_x: 0,
        max_z: 0,
    });
    for value in [hole.min_x, hole.min_z, hole.max_x, hole.max_z] {
        let value = i32::try_from(value).unwrap_or_else(|_| {
            if value.is_negative() {
                i32::MIN
            } else {
                i32::MAX
            }
        });
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let render_view =
        terrain_preview_chunk_render_view(projection, 0.0, 0.0, panel_width, panel_height);
    for value in render_view.view_projection.to_cols_array() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    debug_assert_eq!(bytes.len(), TERRAIN_PREVIEW_UNIFORM_BYTES as usize);
    bytes
}

fn terrain_preview_panel_size(
    width: u32,
    height: u32,
    source: TerrainPreviewSource,
    split_layout: TerrainPreviewSplitLayout,
) -> (u32, u32) {
    if source != TerrainPreviewSource::Split {
        return (width.max(1), height.max(1));
    }
    match split_layout {
        TerrainPreviewSplitLayout::Columns => ((width / 2).max(1), height.max(1)),
        TerrainPreviewSplitLayout::Rows => (width.max(1), (height / 2).max(1)),
    }
}

fn parse_samples(bytes: &[u8]) -> Result<Vec<TerrainPreviewSample>, String> {
    let sample_bytes = TERRAIN_PREVIEW_SAMPLE_BYTES as usize;
    if bytes.is_empty() || bytes.len() % sample_bytes != 0 {
        return Err(format!(
            "terrain preview readback byte length {} is not a positive multiple of {}",
            bytes.len(),
            sample_bytes
        ));
    }
    let mut samples = Vec::with_capacity(bytes.len() / sample_bytes);
    for packed_sample in bytes.chunks_exact(sample_bytes) {
        let mut values = [0.0_f32; TERRAIN_PREVIEW_SAMPLE_FLOATS];
        for (index, value) in values.iter_mut().enumerate() {
            let offset = index * std::mem::size_of::<f32>();
            let value_bytes: [u8; 4] = packed_sample[offset..offset + 4]
                .try_into()
                .expect("terrain preview readback chunks are exact");
            *value = f32::from_le_bytes(value_bytes);
        }
        samples.push(TerrainPreviewSample::from_packed(values));
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::terrain_preview::TerrainPreviewRequest;

    fn validate_shader(source: &str, entry_point: &str) {
        let module = naga::front::wgsl::parse_str(source).expect("terrain preview WGSL parses");
        assert!(
            module
                .entry_points
                .iter()
                .any(|entry| entry.name == entry_point)
        );
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("terrain preview WGSL validates");
    }

    #[test]
    fn compute_and_render_shaders_validate() {
        validate_shader(&terrain_preview_compute_wgsl(), "compute_main");
        let render =
            terrain_preview_render_wgsl(mclone_render_color::RenderTargetColorTransform::Identity);
        let tree =
            terrain_preview_tree_wgsl(mclone_render_color::RenderTargetColorTransform::Identity);
        validate_shader(&render, "vertex_main");
        validate_shader(&render, "fragment_main");
        validate_shader(&tree, "vertex_main");
        validate_shader(&tree, "fragment_main");
    }

    #[test]
    #[ignore = "requires a native WGPU adapter"]
    fn native_gpu_preview_agrees_with_coast_reference_site() {
        use mclone_worldgen::terrain_preview::{
            TerrainPreviewContentStage, TerrainPreviewProfile, TerrainPreviewSurfaceQuality,
        };

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("native coast preview conformance requires a WGPU adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("mclone_coast_preview_conformance_device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .expect("native coast preview conformance device");

        let mut request = TerrainPreviewRequest::new(-98_765, -304, 336, 8)
            .with_profile(TerrainPreviewProfile::McloneOverworldV1)
            .with_content_stage(TerrainPreviewContentStage::Hydrology)
            .with_surface_quality(TerrainPreviewSurfaceQuality::Inferred);
        request.cells_per_axis = 64;
        let reference =
            TerrainPreviewReferenceGrid::compile(request).expect("coast preview reference grid");
        let sample_count = reference.request().sample_count();
        let sample_byte_len = u64::from(sample_count) * TERRAIN_PREVIEW_SAMPLE_BYTES;
        let normal_height_byte_len = u64::from(sample_count) * std::mem::size_of::<f32>() as u64;
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_coast_preview_conformance_uniforms"),
            size: TERRAIN_PREVIEW_UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let gpu_sample_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_coast_preview_conformance_samples"),
            size: sample_byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let normal_height_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_coast_preview_conformance_normal_heights"),
            size: normal_height_byte_len,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_coast_preview_conformance_readback"),
            size: sample_byte_len,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_coast_preview_conformance_layout"),
            entries: &[
                uniform_layout_entry(
                    0,
                    wgpu::ShaderStages::COMPUTE,
                    TERRAIN_PREVIEW_UNIFORM_BYTES,
                ),
                storage_layout_entry(1, wgpu::ShaderStages::COMPUTE, false, sample_byte_len),
                storage_layout_entry(
                    2,
                    wgpu::ShaderStages::COMPUTE,
                    false,
                    normal_height_byte_len,
                ),
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_coast_preview_conformance_bind_group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu_sample_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: normal_height_buffer.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_coast_preview_conformance_shader"),
            source: wgpu::ShaderSource::Wgsl(terrain_preview_compute_wgsl().into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_coast_preview_conformance_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("mclone_coast_preview_conformance_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("compute_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let options = TerrainPreviewDrawOptions {
            source: TerrainPreviewSource::Gpu,
            ..Default::default()
        };
        queue.write_buffer(
            &uniform_buffer,
            0,
            &uniform_bytes(
                &reference,
                512,
                512,
                options,
                TerrainPreviewCamera::default(),
            ),
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_coast_preview_conformance_encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("mclone_coast_preview_conformance_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let workgroups = reference
                .request()
                .samples_per_axis()
                .div_ceil(TERRAIN_PREVIEW_WORKGROUP_AXIS);
            pass.dispatch_workgroups(workgroups, workgroups, 1);
        }
        encoder.copy_buffer_to_buffer(&gpu_sample_buffer, 0, &readback_buffer, 0, sample_byte_len);
        queue.submit(std::iter::once(encoder.finish()));
        let (sender, receiver) = std::sync::mpsc::channel();
        readback_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        device
            .poll(wgpu::PollType::Wait)
            .expect("wait for coast preview conformance readback");
        receiver
            .recv()
            .expect("coast preview conformance callback")
            .expect("coast preview conformance map");
        let mapped = readback_buffer.slice(..).get_mapped_range();
        let gpu_samples = parse_samples(&mapped).expect("coast preview conformance samples");
        let comparison = TerrainPreviewComparison::compare(&reference, &gpu_samples)
            .expect("coast preview comparison");

        eprintln!("coast preview comparison: {comparison:#?}");
        assert_eq!(comparison.max_absolute_base_surface_error, 0.0);
        assert_eq!(comparison.macro_surface_material_agreement, 1.0);
        assert_eq!(comparison.landform_kind_agreement, 1.0);
        assert_eq!(comparison.biome_recipe_agreement, 1.0);
        assert_eq!(comparison.surface_recipe_agreement, 1.0);
    }

    #[test]
    fn close_projection_frames_one_block_at_the_local_surface() {
        let camera = TerrainPreviewCamera::default();
        let focus_y = terrain_preview_focus_y(-98_765, -304, 336);
        let map = terrain_preview_projection(1, TerrainPreviewView::Map, camera, 800, 400, focus_y);
        let projected_map_height = map.eye_offset[1] * (map.fov_y_radians * 0.5).tan() * 2.0;
        assert!((projected_map_height - 0.5).abs() < 1.0e-5);
        assert_eq!(map.kind, TerrainPreviewProjectionKind::Orthographic);
        assert!((map.vertical_half_extent - 0.25).abs() < 1.0e-5);
        assert_eq!(map.target_y, focus_y);
        assert!(map.fov_y_radians < 1.0_f32.to_radians());
        assert!(map.z_near > 0.0 && map.z_far > map.z_near);

        let close = terrain_preview_projection(
            1,
            TerrainPreviewView::ThreeDimensional,
            camera,
            800,
            400,
            focus_y,
        );
        let threshold = terrain_preview_projection(
            96,
            TerrainPreviewView::ThreeDimensional,
            camera,
            800,
            400,
            focus_y,
        );
        let overview = terrain_preview_projection(
            512,
            TerrainPreviewView::ThreeDimensional,
            camera,
            800,
            400,
            focus_y,
        );
        assert_eq!(close.eye_offset, threshold.eye_offset);
        assert_eq!(close.kind, TerrainPreviewProjectionKind::Orthographic);
        assert!(close.fov_y_radians < 1.0_f32.to_radians());
        assert!((threshold.fov_y_radians - 58.0_f32.to_radians()).abs() < f32::EPSILON);
        assert!((overview.fov_y_radians - 58.0_f32.to_radians()).abs() < f32::EPSILON);
        assert!(overview.eye_offset[1] > threshold.eye_offset[1]);

        let perspective = terrain_preview_projection(
            512,
            TerrainPreviewView::ThreeDimensional,
            TerrainPreviewCamera {
                projection: TerrainPreviewProjectionKind::Perspective,
                ..camera
            },
            800,
            400,
            focus_y,
        );
        assert_eq!(perspective.kind, TerrainPreviewProjectionKind::Perspective);
        assert_eq!(
            perspective.vertical_half_extent,
            overview.vertical_half_extent
        );
    }

    #[test]
    fn horizon_orbit_target_uses_the_profile_sea_level() {
        assert_eq!(
            terrain_horizon_orbit_target_y(),
            MCLONE_OVERWORLD_SEA_LEVEL as f32
        );
    }

    #[test]
    fn horizon_target_override_moves_one_shared_camera_without_xz_drift() {
        let presentation = TerrainHorizonPresentation::new(
            -32.0,
            48.0,
            96.0,
            54.0,
            TerrainPreviewView::ThreeDimensional,
            TerrainPreviewCamera::new(0.75, 0.12, TerrainPreviewProjectionKind::Perspective)
                .unwrap(),
        )
        .unwrap();
        let default_view = terrain_horizon_chunk_render_view(presentation, 1280, 720).unwrap();
        let raised_view =
            terrain_horizon_chunk_render_view(presentation.with_target_y(91.0).unwrap(), 1280, 720)
                .unwrap();
        let target_delta = 91.0 - terrain_horizon_orbit_target_y();

        assert_eq!(
            [
                default_view.camera_position.x,
                default_view.camera_position.z
            ],
            [raised_view.camera_position.x, raised_view.camera_position.z]
        );
        assert!(
            (raised_view.camera_position.y - default_view.camera_position.y - target_delta).abs()
                < 1.0e-5
        );
    }

    #[test]
    fn uniform_packing_matches_wgsl_layout() {
        let reference =
            TerrainPreviewReferenceGrid::compile(TerrainPreviewRequest::new(12_345, -64, 96, 16))
                .unwrap();
        let camera =
            TerrainPreviewCamera::new(-0.75, 0.65, TerrainPreviewProjectionKind::Perspective)
                .unwrap();
        let bytes = uniform_bytes(
            &reference,
            1280,
            720,
            TerrainPreviewDrawOptions::default(),
            camera,
        );
        assert_eq!(bytes.len(), TERRAIN_PREVIEW_UNIFORM_BYTES as usize);
        assert_eq!(
            i32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            reference.request().min_x()
        );
        assert_eq!(
            i32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            reference.request().min_z()
        );
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 16);
        assert_eq!(u32::from_le_bytes(bytes[36..40].try_into().unwrap()), 65);
        let focus_y = terrain_preview_focus_y(
            reference.request().request().seed,
            reference.request().request().center_x,
            reference.request().request().center_z,
        );
        let projection = terrain_preview_projection(
            1_024,
            TerrainPreviewView::ThreeDimensional,
            camera,
            1_280,
            720,
            focus_y,
        );
        assert_eq!(
            f32::from_le_bytes(bytes[48..52].try_into().unwrap()),
            projection.eye_offset[0]
        );
        assert_eq!(
            f32::from_le_bytes(bytes[60..64].try_into().unwrap()),
            projection.target_y
        );
        assert_eq!(
            i32::from_le_bytes(bytes[96..100].try_into().unwrap()),
            reference.request().request().center_x
        );
        assert_eq!(
            i32::from_le_bytes(bytes[104..108].try_into().unwrap()),
            1_024
        );
        assert_eq!(f32::from_le_bytes(bytes[112..116].try_into().unwrap()), 0.0);
        assert_eq!(
            f32::from_le_bytes(bytes[120..124].try_into().unwrap()),
            1_024.0
        );
        assert_eq!(
            u32::from_le_bytes(bytes[128..132].try_into().unwrap()),
            mclone_worldgen::terrain_preview::TerrainPreviewContentStage::Base as u32
        );
        assert_eq!(u32::from_le_bytes(bytes[132..136].try_into().unwrap()), 1);
        let packed_view_projection = std::array::from_fn::<_, 16, _>(|index| {
            let offset = 160 + index * std::mem::size_of::<f32>();
            f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
        });
        let expected_view_projection =
            terrain_preview_chunk_render_view(projection, 0.0, 0.0, 1_280, 720)
                .view_projection
                .to_cols_array();
        assert_eq!(packed_view_projection, expected_view_projection);
    }

    #[test]
    fn continuous_presentation_splits_large_centers_from_fractional_motion() {
        let presentation = TerrainPreviewUniformPresentation::continuous(
            1_000_000.25,
            -1_000_000.375,
            4_096.5,
            2_304.28125,
        )
        .unwrap();

        assert_eq!(presentation.anchor_x, 1_000_000);
        assert_eq!(presentation.anchor_z, -1_000_000);
        assert_eq!(presentation.fraction_x, 0.25);
        assert_eq!(presentation.fraction_z, -0.375);
        assert_eq!(presentation.width_blocks, 4_096.5);
        assert_eq!(presentation.height_blocks, 2_304.28125);
    }

    #[test]
    fn continuous_presentation_stays_smooth_across_anchor_changes() {
        let left =
            TerrainPreviewUniformPresentation::continuous(0.49, -0.49, 512.0, 288.0).unwrap();
        let right =
            TerrainPreviewUniformPresentation::continuous(0.51, -0.51, 512.0, 288.0).unwrap();
        let world_x = 32;
        let world_z = -24;
        let left_relative_x = (world_x - left.anchor_x) as f32 - left.fraction_x;
        let right_relative_x = (world_x - right.anchor_x) as f32 - right.fraction_x;
        let left_relative_z = (world_z - left.anchor_z) as f32 - left.fraction_z;
        let right_relative_z = (world_z - right.anchor_z) as f32 - right.fraction_z;

        assert!((left_relative_x - right_relative_x - 0.02).abs() < 0.000_01);
        assert!((left_relative_z - right_relative_z + 0.02).abs() < 0.000_01);
    }

    #[test]
    fn continuous_presentation_rejects_unrepresentable_values() {
        let camera = TerrainPreviewCamera::default();
        assert!(
            TerrainHorizonPresentation::new(
                f64::NAN,
                0.0,
                512.0,
                288.0,
                TerrainPreviewView::Map,
                camera,
            )
            .is_err()
        );
        assert!(
            TerrainHorizonPresentation::new(
                0.0,
                0.0,
                f64::from(f32::MAX) * 2.0,
                288.0,
                TerrainPreviewView::Map,
                camera,
            )
            .is_err()
        );
    }

    #[test]
    fn parses_packed_samples_and_rejects_misalignment() {
        let sample = TerrainPreviewSample {
            surface_y: 64.0,
            display_y: 64.0,
            continentalness: 0.2,
            relief: -0.1,
            temperature: 0.4,
            moisture: 0.6,
            water: 0.0,
            ruggedness: 0.8,
            base_surface_y: 63.0,
            base_display_y: 64.0,
            ocean_water: 0.0,
            macro_surface_material: 4.0,
            river_signed_distance: -2.0,
            channel_influence: 0.8,
            bank_influence: 1.0,
            river_half_width: 6.0,
            wetland_influence: 0.2,
            wetland_pool_influence: 0.0,
            submerged_outlet_influence: 0.0,
            visible_surface_material: 2.0,
            planned_stream_influence: 0.0,
            biome_recipe: 2.0,
            landform_kind: 2.0,
            surface_recipe: 2.0,
            forest_coverage: 0.72,
            forest_density: 0.64,
            forest_family: 1.0,
            forest_family_mix: 0.0,
            mean_canopy_height: 8.0,
            canopy_height_variation: 1.8,
            grove_or_opening_influence: 0.5,
            forest_summary_available: 1.0,
        };
        let mut bytes = Vec::new();
        for value in sample.packed() {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        assert_eq!(parse_samples(&bytes).unwrap(), vec![sample]);
        bytes.push(0);
        assert!(parse_samples(&bytes).is_err());
    }

    #[test]
    fn evaluator_revision_and_production_spec_are_explicit() {
        assert_eq!(
            TERRAIN_PREVIEW_GPU_EVALUATOR_REVISION,
            "mclone-overworld-v1-gpu-preview-a9"
        );
        let shader = terrain_preview_compute_wgsl();
        assert!(!shader.contains("__MCLONE_PRODUCTION_FIELD_CONSTANTS__"));
        assert!(
            shader.contains("const CONTINENT_LARGE_DOMAIN: U64 = U64(0x636f6e31u, 0x6d636f76u);")
        );
        assert!(shader.contains("const MOUNTAIN_DETAIL_FINE_SCALE: i32 = 8;"));
        assert!(shader.contains("const RIVER_LARGE_SCALE: i32 = 768;"));
        assert!(shader.contains("fn river_geometry"));
        assert!(shader.contains("fn complete_hydrology"));
        assert!(shader.contains("fn splitmix64"));
        assert!(shader.contains("fn gradient_noise"));
        assert!(!shader.contains("band_weight"));
        assert!(TERRAIN_PREVIEW_RENDER_WGSL.contains("error_color"));
        assert!(TERRAIN_PREVIEW_RENDER_WGSL.contains("@builtin(instance_index)"));
        assert!(TERRAIN_PREVIEW_RENDER_WGSL.contains("fn base_sample"));
        assert!(TERRAIN_PREVIEW_RENDER_WGSL.contains("visible_half_width"));
        assert!(TERRAIN_PREVIEW_RENDER_WGSL.contains("params.view_projection * vec4<f32>"));
        assert!(TERRAIN_PREVIEW_TREE_WGSL.contains("params.view_projection * vec4<f32>"));
        assert!(!TERRAIN_PREVIEW_RENDER_WGSL.contains("clip_z = 1.0 - clamp"));
        assert!(!TERRAIN_PREVIEW_TREE_WGSL.contains("clip_z = 1.0 - clamp"));
        assert!(TERRAIN_PREVIEW_RENDER_WGSL.contains("sample.terrain.y + 1.0"));
        assert!(
            TERRAIN_PREVIEW_RENDER_WGSL
                .contains("let stacked_compare = compare && params.content_stage_flags.z == 1u")
        );
        assert_eq!(preview_instance_count(TerrainPreviewSource::Split), 2);
        assert_eq!(preview_instance_count(TerrainPreviewSource::Reference), 1);
        assert_eq!(preview_instance_count(TerrainPreviewSource::Macro), 1);
    }
}
