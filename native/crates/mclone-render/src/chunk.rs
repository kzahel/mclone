use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::num::{NonZeroU32, NonZeroU64};
use std::ops::Range;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use anyhow::{Context, Result, bail};
use glam::{Mat4, Quat, Vec3, Vec4};
use mclone_core::{
    ChunkPos, HorizontalTopology, Vec3d, block_to_chunk_coord, block_to_section_coord,
    chunk_middle_block_coord, chunk_min_block_coord,
};
use mclone_diagnostics::GpuPassId;
use mclone_mesh::{
    BUSHY_LEAF_CARD_OVERHANG, CHUNK_WIDTH as MESH_CHUNK_WIDTH, RENDER_SECTION_HEIGHT,
    RenderSectionKey, SectionFace, TexturedRenderSectionMesh, TexturedRenderSectionMetadata,
    TexturedVisibleChunkMesh, VisibilitySet, VisibleChunkMesh, quad_face_count_from_indices,
};
use rustc_hash::{FxHashMap, FxHashSet};
use wgpu::util::DeviceExt;

use crate::color_profile::{RenderColorProfile, RenderConfig};
use crate::fog::RenderFog;
use crate::gpu_timestamps::GpuTimestampFrameEncoder;
use crate::placement::{
    CompositionClip, WorldCompositionContext, WorldPlacement, WorldSourceBounds,
};
use crate::target::RenderFrameTarget;
use crate::texture_mips::generate_rgba_mip_chain;
use crate::uniform::{
    PER_VIEW_UNIFORM_SLOT_COUNT, PerViewSlot, PerViewUniformBuffer, SINGLE_VIEW_SLOT, StereoEye,
};

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Reversed-Z clear value paired with [`DEPTH_FORMAT`]: the far plane is `0.0`,
/// so the depth buffer clears to `0.0` and the depth test is `GreaterEqual`.
pub const REVERSED_Z_DEPTH_CLEAR: f32 = 0.0;

/// Reversed-Z remap matrix. Transforms a standard wgpu/DX `[0,1]` depth
/// projection (near→0, far→1) into reversed-Z (near→1, far→0) by mapping
/// clip-space `z' = w - z`. Paired with [`DEPTH_FORMAT`] (`Depth32Float`), a
/// `GreaterEqual` depth test, and a [`REVERSED_Z_DEPTH_CLEAR`] clear, this gives
/// near-uniform depth precision and eliminates far-distance z-fighting.
/// See `docs/tactical/158-reversed-z-depth-precision.md`.
const REVERSE_Z: Mat4 = Mat4::from_cols(
    Vec4::new(1.0, 0.0, 0.0, 0.0),
    Vec4::new(0.0, 1.0, 0.0, 0.0),
    Vec4::new(0.0, 0.0, -1.0, 0.0),
    Vec4::new(0.0, 0.0, 1.0, 1.0),
);

/// Right-handed perspective projection with reversed-Z depth. Preserves the
/// finite near/far clip planes of [`Mat4::perspective_rh`] but flips the depth
/// mapping to near→1, far→0. Use this everywhere a depth-writing pass builds its
/// projection so the whole pipeline stays on one convention (see [`REVERSE_Z`]).
pub fn reversed_z_perspective_rh(fov_y_radians: f32, aspect: f32, z_near: f32, z_far: f32) -> Mat4 {
    REVERSE_Z * Mat4::perspective_rh(fov_y_radians, aspect, z_near, z_far)
}

const VERTEX_FLOAT_COUNT: usize = 7;
const VERTEX_BYTE_SIZE: wgpu::BufferAddress =
    (VERTEX_FLOAT_COUNT * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const TEXTURED_VERTEX_BYTE_SIZE: wgpu::BufferAddress = 40;
const UNIFORM_BYTE_LEN: usize = 128;
const UNIFORM_BYTE_SIZE: wgpu::BufferAddress = UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const MULTIVIEW_UNIFORM_BYTE_LEN: usize = UNIFORM_BYTE_LEN * 2;
const MULTIVIEW_UNIFORM_BYTE_SIZE: wgpu::BufferAddress =
    MULTIVIEW_UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const PLACED_UNIFORM_BYTE_LEN: usize = 160;
const PLACED_UNIFORM_BYTE_SIZE: wgpu::BufferAddress =
    PLACED_UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const PLACED_MULTIVIEW_UNIFORM_BYTE_LEN: usize = PLACED_UNIFORM_BYTE_LEN * 2;
const PLACED_MULTIVIEW_UNIFORM_BYTE_SIZE: wgpu::BufferAddress =
    PLACED_MULTIVIEW_UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const CLIPPED_PLACED_UNIFORM_BYTE_LEN: usize = PLACED_UNIFORM_BYTE_LEN + 16;
const CLIPPED_PLACED_UNIFORM_BYTE_SIZE: wgpu::BufferAddress =
    CLIPPED_PLACED_UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const CLIPPED_PLACED_MULTIVIEW_UNIFORM_BYTE_LEN: usize = CLIPPED_PLACED_UNIFORM_BYTE_LEN * 2;
const CLIPPED_PLACED_MULTIVIEW_UNIFORM_BYTE_SIZE: wgpu::BufferAddress =
    CLIPPED_PLACED_MULTIVIEW_UNIFORM_BYTE_LEN as wgpu::BufferAddress;
// Matches the default Java 1.17.1 video option: Options.mipmapLevels = 4.
// TextureUtil.prepareImage allocates levels 0..=4 for the block atlas.
const CHUNK_ATLAS_MAX_MIP_LEVEL: u32 = 4;
const CAMERA_BASIS_MIN_LENGTH_SQUARED: f32 = 1.0e-8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkCamera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov_y_radians: f32,
    pub z_near: f32,
    pub z_far: f32,
}

impl ChunkCamera {
    pub fn overview_for_chunk(chunk_x: i32, chunk_z: i32) -> Self {
        Self::overview_for_chunk_area(chunk_x, chunk_z, 0)
    }

    pub fn overview_for_chunk_area(
        center_chunk_x: i32,
        center_chunk_z: i32,
        chunk_radius: i32,
    ) -> Self {
        let radius = chunk_radius.max(0);
        let scale = 1.0 + radius as f32 * 1.1;
        let center_x = chunk_middle_block_coord(center_chunk_x) as f32;
        let center_z = chunk_middle_block_coord(center_chunk_z) as f32;
        Self {
            eye: [
                center_x + 54.0 * scale,
                116.0 + 12.0 * (scale - 1.0),
                center_z - 66.0 * scale,
            ],
            target: [center_x, 48.0, center_z],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 58.0_f32.to_radians(),
            z_near: 0.1,
            z_far: 600.0 * scale,
        }
    }

    pub fn view_projection(self, width: u32, height: u32) -> [[f32; 4]; 4] {
        self.render_view(width, height).uniform_matrix()
    }

    pub fn render_view(self, width: u32, height: u32) -> ChunkRenderView {
        let aspect = width.max(1) as f32 / height.max(1) as f32;
        let aspect = aspect.max(0.01);
        let eye = Vec3::from_array(self.eye);
        let target = Vec3::from_array(self.target);
        let world_up = Vec3::from_array(self.up);
        let view = Mat4::look_at_rh(eye, target, world_up);
        let projection =
            reversed_z_perspective_rh(self.fov_y_radians, aspect, self.z_near, self.z_far);
        let forward = (target - eye).normalize_or_zero();
        let right = forward.cross(world_up).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();

        let render_view = ChunkRenderView {
            view,
            projection,
            view_projection: projection * view,
            camera_position: eye,
            camera_forward: forward,
            camera_right: right,
            camera_up: up,
            aspect,
            fov_y_radians: self.fov_y_radians,
            z_near: self.z_near,
            z_far: self.z_far,
            projection_kind: ChunkProjectionKind::CameraPerspective,
        };
        debug_assert!(
            render_view.is_finite(),
            "ChunkCamera produced a non-finite render view: {render_view:?}"
        );
        render_view
    }

    pub fn orbit(&mut self, yaw_delta: f32, pitch_delta: f32) {
        let target = Vec3::from_array(self.target);
        let mut offset = Vec3::from_array(self.eye) - target;
        if offset.length_squared() <= f32::EPSILON {
            return;
        }

        offset = Mat4::from_rotation_y(yaw_delta).transform_vector3(offset);
        let forward = (-offset).normalize();
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        if right.length_squared() > f32::EPSILON {
            let pitched = Mat4::from_axis_angle(right, pitch_delta).transform_vector3(offset);
            let pitched_forward = (-pitched).normalize();
            if pitched_forward.dot(Vec3::Y).abs() < 0.96 {
                offset = pitched;
            }
        }

        self.eye = (target + offset).to_array();
    }

    pub fn move_local(&mut self, right_axis: f32, up_axis: f32, forward_axis: f32, distance: f32) {
        if distance <= 0.0 {
            return;
        }
        let (forward, right, up) = self.basis();
        let direction = right * right_axis + up * up_axis + forward * forward_axis;
        let Some(direction) = direction.try_normalize() else {
            return;
        };
        let delta = direction * distance;
        self.eye = (Vec3::from_array(self.eye) + delta).to_array();
        self.target = (Vec3::from_array(self.target) + delta).to_array();
    }

    pub fn zoom(&mut self, amount: f32) {
        if !amount.is_finite() {
            return;
        }
        let target = Vec3::from_array(self.target);
        let eye = Vec3::from_array(self.eye);
        let offset = eye - target;
        let distance = offset.length();
        if distance <= f32::EPSILON {
            return;
        }
        let new_distance = (distance * (1.0 - amount).clamp(0.2, 5.0)).clamp(8.0, 900.0);
        self.eye = (target + offset / distance * new_distance).to_array();
    }

    fn basis(self) -> (Vec3, Vec3, Vec3) {
        let forward = (Vec3::from_array(self.target) - Vec3::from_array(self.eye)).normalize();
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        (forward, right, up)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerspectiveRenderPose {
    pub eye: Vec3,
    pub orientation: Quat,
    pub fov_y_radians: f32,
    pub z_near: f32,
    pub z_far: f32,
}

impl PerspectiveRenderPose {
    pub const fn new(
        eye: Vec3,
        orientation: Quat,
        fov_y_radians: f32,
        z_near: f32,
        z_far: f32,
    ) -> Self {
        Self {
            eye,
            orientation,
            fov_y_radians,
            z_near,
            z_far,
        }
    }

    pub fn render_view(self, width: u32, height: u32) -> Result<ChunkRenderView> {
        if !self.eye.is_finite() {
            bail!("perspective render pose has a non-finite eye");
        }
        if !finite_quat(self.orientation)
            || self.orientation.length_squared() <= CAMERA_BASIS_MIN_LENGTH_SQUARED
        {
            bail!("perspective render pose has an invalid orientation");
        }
        if !self.fov_y_radians.is_finite()
            || self.fov_y_radians <= 0.0
            || !self.z_near.is_finite()
            || !self.z_far.is_finite()
            || self.z_near <= 0.0
            || self.z_far <= self.z_near
        {
            bail!("perspective render pose has invalid projection parameters");
        }

        let aspect = width.max(1) as f32 / height.max(1) as f32;
        let aspect = aspect.max(0.01);
        let orientation = self.orientation.normalize();
        let view = Mat4::from_rotation_translation(orientation, self.eye).inverse();
        let projection =
            reversed_z_perspective_rh(self.fov_y_radians, aspect, self.z_near, self.z_far);
        let camera_forward = (orientation * Vec3::NEG_Z).normalize_or_zero();
        let camera_right = (orientation * Vec3::X).normalize_or_zero();
        let camera_up = (orientation * Vec3::Y).normalize_or_zero();
        if camera_forward.length_squared() <= CAMERA_BASIS_MIN_LENGTH_SQUARED
            || camera_right.length_squared() <= CAMERA_BASIS_MIN_LENGTH_SQUARED
            || camera_up.length_squared() <= CAMERA_BASIS_MIN_LENGTH_SQUARED
        {
            bail!("perspective render pose produced a degenerate camera basis");
        }

        let render_view = ChunkRenderView {
            view,
            projection,
            view_projection: projection * view,
            camera_position: self.eye,
            camera_forward,
            camera_right,
            camera_up,
            aspect,
            fov_y_radians: self.fov_y_radians,
            z_near: self.z_near,
            z_far: self.z_far,
            projection_kind: ChunkProjectionKind::CameraPerspective,
        };
        if !render_view.is_finite() {
            bail!("perspective render pose produced a non-finite render view");
        }
        Ok(render_view)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkProjectionKind {
    CameraPerspective,
    External,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkRenderView {
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
    pub projection_kind: ChunkProjectionKind,
}

impl ChunkRenderView {
    pub fn is_finite(self) -> bool {
        self.view.is_finite()
            && self.projection.is_finite()
            && self.view_projection.is_finite()
            && self.camera_position.is_finite()
            && self.camera_forward.is_finite()
            && self.camera_right.is_finite()
            && self.camera_up.is_finite()
            && self.aspect.is_finite()
            && self.fov_y_radians.is_finite()
            && self.z_near.is_finite()
            && self.z_far.is_finite()
    }

    pub fn uniform_matrix(self) -> [[f32; 4]; 4] {
        self.view_projection.to_cols_array_2d()
    }

    pub fn with_fov_multiplier(self, multiplier: f32) -> Self {
        if self.projection_kind != ChunkProjectionKind::CameraPerspective
            || !multiplier.is_finite()
            || multiplier <= 0.0
        {
            return self;
        }
        let fov_y_radians = self.fov_y_radians * multiplier;
        let projection =
            reversed_z_perspective_rh(fov_y_radians, self.aspect, self.z_near, self.z_far);
        Self {
            projection,
            view_projection: projection * self.view,
            fov_y_radians,
            ..self
        }
    }

    /// View-projection with the camera translation dropped, so geometry rendered
    /// with it (the sky dome, celestial bodies) is anchored at infinity. Mirrors
    /// Minecraft's `renderSky`, which draws into a pose stack carrying only the
    /// camera rotation.
    pub fn sky_view_projection(self) -> Mat4 {
        debug_assert!(
            self.is_finite(),
            "sky view-projection requires a finite render view: {self:?}"
        );
        let rotation_only_view = Mat4::from_cols(
            self.view.x_axis,
            self.view.y_axis,
            self.view.z_axis,
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        );
        self.projection * rotation_only_view
    }
}

fn finite_quat(value: Quat) -> bool {
    let [x, y, z, w] = value.to_array();
    x.is_finite() && y.is_finite() && z.is_finite() && w.is_finite()
}

#[derive(Clone, Copy)]
pub struct ChunkRenderTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub depth_view: &'a wgpu::TextureView,
    pub size: [u32; 2],
    pub clear_color: wgpu::Color,
    pub clear_depth: f32,
    pub gpu_timestamps: Option<&'a GpuTimestampFrameEncoder>,
    /// When `true`, the color attachment is loaded instead of cleared — used when
    /// an earlier pass (the sky dome) has already drawn the background.
    pub load_color: bool,
    /// When `true`, the depth attachment is loaded instead of cleared — used by
    /// late translucent terrain passes after actors have written depth.
    pub load_depth: bool,
}

impl<'a> ChunkRenderTarget<'a> {
    pub fn new(
        color_view: &'a wgpu::TextureView,
        depth_view: &'a wgpu::TextureView,
        size: [u32; 2],
        clear_color: wgpu::Color,
    ) -> Self {
        Self {
            color_view,
            depth_view,
            size,
            clear_color,
            clear_depth: REVERSED_Z_DEPTH_CLEAR,
            gpu_timestamps: None,
            load_color: false,
            load_depth: false,
        }
    }

    pub fn from_frame_target(
        target: RenderFrameTarget<'a>,
        clear_color: wgpu::Color,
    ) -> Result<Self> {
        let depth_view = target
            .depth_view
            .context("chunk render target requires a depth attachment")?;
        Ok(
            Self::new(target.color_view, depth_view, target.size, clear_color)
                .with_gpu_timestamps_option(target.gpu_timestamps),
        )
    }

    /// Preserve the color attachment's existing contents instead of clearing,
    /// so a previously drawn sky shows through where no chunk geometry covers it.
    pub fn with_loaded_color(mut self) -> Self {
        self.load_color = true;
        self
    }

    pub fn with_loaded_depth(mut self) -> Self {
        self.load_depth = true;
        self
    }

    pub fn with_gpu_timestamps(mut self, gpu_timestamps: &'a GpuTimestampFrameEncoder) -> Self {
        self.gpu_timestamps = Some(gpu_timestamps);
        self
    }

    pub fn with_gpu_timestamps_option(
        mut self,
        gpu_timestamps: Option<&'a GpuTimestampFrameEncoder>,
    ) -> Self {
        self.gpu_timestamps = gpu_timestamps;
        self
    }

    fn gpu_timestamp_writes(self, pass: GpuPassId) -> Option<wgpu::RenderPassTimestampWrites<'a>> {
        self.gpu_timestamps
            .and_then(|timestamps| timestamps.render_pass_timestamp_writes(pass))
    }

    fn color_load_op(self) -> wgpu::LoadOp<wgpu::Color> {
        if self.load_color {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(self.clear_color)
        }
    }

    fn depth_load_op(self) -> wgpu::LoadOp<f32> {
        if self.load_depth {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(self.clear_depth)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TexturedSectionRenderStats {
    pub section_occlusion_culling: bool,
    pub force_fullbright: bool,
    pub loaded_section_count: usize,
    pub drawn_section_count: usize,
    pub frustum_section_count: usize,
    pub readiness_culled_section_count: usize,
    pub graph_cull_enabled: bool,
    pub graph_culled_section_count: usize,
    pub loaded_index_count: u32,
    pub drawn_index_count: u32,
    pub frustum_index_count: u32,
    pub readiness_culled_index_count: u32,
    pub graph_culled_index_count: u32,
}

/// Pull-only exact section sets for one render view. `paintable_frustum_keys`
/// excludes non-drawable and not-yet-ready records; a column present there but
/// absent from `drawn_keys` was graph-culled rather than merely off-screen.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TexturedSectionViewSetSnapshot {
    pub paintable_frustum_keys: BTreeSet<RenderSectionKey>,
    pub drawn_keys: BTreeSet<RenderSectionKey>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TexturedSectionRenderPhase {
    #[default]
    All,
    Opaque,
    Translucent,
}

impl TexturedSectionRenderPhase {
    const fn draws_opaque(self) -> bool {
        matches!(self, Self::All | Self::Opaque)
    }

    const fn draws_translucent(self) -> bool {
        matches!(self, Self::All | Self::Translucent)
    }

    fn gpu_pass_id(self) -> GpuPassId {
        match self {
            Self::All => GpuPassId::Terrain,
            Self::Opaque => GpuPassId::TerrainOpaque,
            Self::Translucent => GpuPassId::TerrainTranslucent,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TexturedSectionRenderTiming {
    pub records_ms: f64,
    pub cull_ms: f64,
    pub cull_cache_lookup: bool,
    pub cull_cache_hit: bool,
    pub uniform_write_ms: f64,
    pub translucent_collect_ms: f64,
    pub translucent_sort_ms: f64,
    pub prepare_ms: f64,
    pub encode_ms: f64,
    pub direct_draw_calls: usize,
    pub multi_draw_calls: usize,
    pub indirect_draw_count: usize,
    pub arena_vertex_used_bytes: u64,
    pub arena_vertex_capacity_bytes: u64,
    pub arena_index_used_bytes: u64,
    pub arena_index_capacity_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexturedSectionRenderOptions {
    pub section_occlusion_culling: bool,
    pub force_fullbright: bool,
    pub sky_darken: f32,
    pub fog: RenderFog,
    pub color_profile: RenderColorProfile,
    /// Active dimension topology used only for observer-local presentation.
    /// Canonical mesh/upload identity remains unchanged.
    pub topology: HorizontalTopology,
}

impl Default for TexturedSectionRenderOptions {
    fn default() -> Self {
        Self {
            section_occlusion_culling: true,
            force_fullbright: false,
            sky_darken: 1.0,
            fog: RenderFog::none(),
            color_profile: RenderColorProfile::default(),
            topology: HorizontalTopology::UNBOUNDED,
        }
    }
}

impl TexturedSectionRenderOptions {
    pub fn with_sky_darken(mut self, sky_darken: f32) -> Self {
        self.sky_darken = sky_darken.clamp(0.0, 1.0);
        self
    }

    pub fn with_fog(mut self, fog: RenderFog) -> Self {
        self.fog = fog;
        self
    }

    pub fn with_color_profile(mut self, color_profile: RenderColorProfile) -> Self {
        self.color_profile = color_profile;
        self
    }

    pub fn with_topology(mut self, topology: HorizontalTopology) -> Self {
        self.topology = topology;
        self
    }
}

impl TexturedSectionRenderStats {
    pub fn loaded_face_count(&self) -> u32 {
        quad_face_count_from_indices(self.loaded_index_count)
    }

    pub fn drawn_face_count(&self) -> u32 {
        quad_face_count_from_indices(self.drawn_index_count)
    }

    pub fn frustum_face_count(&self) -> u32 {
        quad_face_count_from_indices(self.frustum_index_count)
    }

    pub fn graph_culled_face_count(&self) -> u32 {
        quad_face_count_from_indices(self.graph_culled_index_count)
    }

    pub fn readiness_culled_face_count(&self) -> u32 {
        quad_face_count_from_indices(self.readiness_culled_index_count)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TexturedSectionUploadReport {
    pub uploaded_section_count: usize,
    pub removed_section_count: usize,
    pub uploaded_vertex_count: u32,
    pub uploaded_index_count: u32,
}

impl TexturedSectionUploadReport {
    pub fn uploaded_face_count(&self) -> u32 {
        quad_face_count_from_indices(self.uploaded_index_count)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TexturedSectionUploadTiming {
    pub total_ms: f64,
    pub dirty_mark_ms: f64,
    pub remove_ms: f64,
    pub section_state_ms: f64,
    pub vertex_bytes_ms: f64,
    pub vertex_buffer_ms: f64,
    pub index_bytes_ms: f64,
    pub index_buffer_ms: f64,
    pub mesh_insert_ms: f64,
    pub mesh_upload_worst_ms: f64,
}

impl TexturedSectionUploadTiming {
    fn absorb_mesh_upload(&mut self, timing: GpuTexturedChunkMeshUploadTiming) {
        self.vertex_bytes_ms += timing.vertex_bytes_ms;
        self.vertex_buffer_ms += timing.vertex_buffer_ms;
        self.index_bytes_ms += timing.index_bytes_ms;
        self.index_buffer_ms += timing.index_buffer_ms;
        self.mesh_upload_worst_ms = self.mesh_upload_worst_ms.max(timing.total_ms);
    }
}

pub fn textured_section_visibility_stats(
    sections: &[TexturedRenderSectionMetadata],
    render_view: ChunkRenderView,
) -> TexturedSectionRenderStats {
    textured_section_visibility_stats_with_options(
        sections,
        render_view,
        TexturedSectionRenderOptions::default(),
    )
}

pub fn textured_section_visibility_stats_with_options(
    sections: &[TexturedRenderSectionMetadata],
    render_view: ChunkRenderView,
    options: TexturedSectionRenderOptions,
) -> TexturedSectionRenderStats {
    textured_section_visibility_stats_with_options_and_ready_sections(
        sections,
        render_view,
        options,
        None,
    )
}

pub fn textured_section_visibility_stats_with_options_and_ready_sections(
    sections: &[TexturedRenderSectionMetadata],
    render_view: ChunkRenderView,
    options: TexturedSectionRenderOptions,
    ready_sections: Option<&BTreeSet<RenderSectionKey>>,
) -> TexturedSectionRenderStats {
    let records = sections
        .iter()
        .map(|section| {
            (
                section.key,
                TexturedSectionCullingRecord {
                    index_count: section.stats.index_count,
                    visibility: section.visibility,
                    drawable: section.drawable,
                    traversal_ready: ready_sections
                        .is_none_or(|ready| ready.contains(&section.key)),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let loaded_section_count = records.values().filter(|record| record.drawable).count();
    let loaded_index_count = records
        .values()
        .filter(|record| record.drawable)
        .map(|record| record.index_count)
        .sum();
    let prepared = PreparedTexturedSectionRecords {
        records,
        loaded_section_count,
        loaded_index_count,
    };
    let mut scratch = CullScratch::default();
    cull_textured_sections(&prepared, render_view, options, &mut scratch).stats
}

#[derive(Clone, Copy, Debug)]
struct TexturedSectionCullingRecord {
    index_count: u32,
    visibility: VisibilitySet,
    drawable: bool,
    traversal_ready: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PreparedTexturedSectionRecords {
    records: BTreeMap<RenderSectionKey, TexturedSectionCullingRecord>,
    // Slice F (docs/tactical/106): view-independent loaded totals, computed once
    // when the record set is built rather than re-summed per eye per frame in
    // `cull_textured_sections`.
    loaded_section_count: usize,
    loaded_index_count: u32,
}

impl PreparedTexturedSectionRecords {
    /// Build the stable, bounded record set consumed by an embedded terrain
    /// presentation. The source store remains untouched and may continue to
    /// retain neighboring sections for lighting or later interest changes.
    pub fn for_source_bounds(&self, bounds: WorldSourceBounds) -> Self {
        let records = self
            .records
            .iter()
            .filter(|(key, _)| bounds.contains_render_section(**key))
            .map(|(key, record)| (*key, *record))
            .collect::<BTreeMap<_, _>>();
        let loaded_section_count = records.values().filter(|record| record.drawable).count();
        let loaded_index_count = records
            .values()
            .filter(|record| record.drawable)
            .map(|record| record.index_count)
            .sum();
        Self {
            records,
            loaded_section_count,
            loaded_index_count,
        }
    }

    pub fn section_keys(&self) -> impl ExactSizeIterator<Item = RenderSectionKey> + '_ {
        self.records.keys().copied()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TexturedSectionRecordCacheStats {
    pub ready_set_calls: u64,
    pub ready_set_changed_calls: u64,
    pub ready_set_unchanged_calls: u64,
    pub ready_set_upload_backpressured_calls: u64,
    pub ready_set_upload_backpressured_changed_calls: u64,
    pub ready_set_upload_backpressured_unchanged_calls: u64,
    pub ready_set_skipped_calls: u64,
    pub ready_set_upload_backpressured_skipped_calls: u64,
    pub prepared_record_rebuilds: u64,
    pub prepared_record_rebuild_total_ms: f64,
    pub prepared_record_rebuild_max_ms: f64,
    pub prepared_record_rebuild_initial_dirty: u64,
    pub prepared_record_rebuild_section_upload_dirty: u64,
    pub prepared_record_rebuild_section_remove_dirty: u64,
    pub prepared_record_rebuild_ready_set_dirty: u64,
    pub prepared_record_rebuild_upload_backpressured_dirty: u64,
    pub prepared_record_rebuild_multi_dirty: u64,
    pub prepared_record_rebuild_visibility_section_max: u64,
    pub prepared_record_rebuild_loaded_section_max: u64,
    pub prepared_record_rebuild_ready_section_max: u64,
    pub prepared_record_rebuild_loaded_index_max: u64,
}

impl TexturedSectionRecordCacheStats {
    pub fn record_ready_set_call(&mut self, upload_backpressured: bool, changed: bool) {
        self.ready_set_calls += 1;
        if changed {
            self.ready_set_changed_calls += 1;
        } else {
            self.ready_set_unchanged_calls += 1;
        }
        if upload_backpressured {
            self.ready_set_upload_backpressured_calls += 1;
            if changed {
                self.ready_set_upload_backpressured_changed_calls += 1;
            } else {
                self.ready_set_upload_backpressured_unchanged_calls += 1;
            }
        }
    }

    pub fn record_ready_set_skip(&mut self, upload_backpressured: bool) {
        self.ready_set_skipped_calls += 1;
        if upload_backpressured {
            self.ready_set_upload_backpressured_skipped_calls += 1;
        }
    }

    pub fn record_prepared_record_rebuild(
        &mut self,
        rebuild_ms: f64,
        dirty_causes: TexturedSectionRecordDirtyCauses,
        visibility_section_count: usize,
        loaded_section_count: usize,
        ready_section_count: usize,
        loaded_index_count: u32,
    ) {
        self.prepared_record_rebuilds += 1;
        self.prepared_record_rebuild_total_ms += rebuild_ms;
        self.prepared_record_rebuild_max_ms = self.prepared_record_rebuild_max_ms.max(rebuild_ms);
        if dirty_causes.contains(TexturedSectionRecordDirtyCauses::INITIAL) {
            self.prepared_record_rebuild_initial_dirty += 1;
        }
        if dirty_causes.contains(TexturedSectionRecordDirtyCauses::SECTION_UPLOAD) {
            self.prepared_record_rebuild_section_upload_dirty += 1;
        }
        if dirty_causes.contains(TexturedSectionRecordDirtyCauses::SECTION_REMOVE) {
            self.prepared_record_rebuild_section_remove_dirty += 1;
        }
        if dirty_causes.contains(TexturedSectionRecordDirtyCauses::TRAVERSAL_READY) {
            self.prepared_record_rebuild_ready_set_dirty += 1;
        }
        if dirty_causes.contains(TexturedSectionRecordDirtyCauses::UPLOAD_BACKPRESSURED) {
            self.prepared_record_rebuild_upload_backpressured_dirty += 1;
        }
        if dirty_causes.cause_count() > 1 {
            self.prepared_record_rebuild_multi_dirty += 1;
        }
        self.prepared_record_rebuild_visibility_section_max = self
            .prepared_record_rebuild_visibility_section_max
            .max(visibility_section_count as u64);
        self.prepared_record_rebuild_loaded_section_max = self
            .prepared_record_rebuild_loaded_section_max
            .max(loaded_section_count as u64);
        self.prepared_record_rebuild_ready_section_max = self
            .prepared_record_rebuild_ready_section_max
            .max(ready_section_count as u64);
        self.prepared_record_rebuild_loaded_index_max = self
            .prepared_record_rebuild_loaded_index_max
            .max(u64::from(loaded_index_count));
    }

    pub fn prepared_record_rebuild_avg_ms(self) -> f64 {
        if self.prepared_record_rebuilds == 0 {
            0.0
        } else {
            self.prepared_record_rebuild_total_ms / self.prepared_record_rebuilds as f64
        }
    }

    pub fn sample_delta(self, baseline: Self) -> Self {
        Self {
            ready_set_calls: self
                .ready_set_calls
                .saturating_sub(baseline.ready_set_calls),
            ready_set_changed_calls: self
                .ready_set_changed_calls
                .saturating_sub(baseline.ready_set_changed_calls),
            ready_set_unchanged_calls: self
                .ready_set_unchanged_calls
                .saturating_sub(baseline.ready_set_unchanged_calls),
            ready_set_upload_backpressured_calls: self
                .ready_set_upload_backpressured_calls
                .saturating_sub(baseline.ready_set_upload_backpressured_calls),
            ready_set_upload_backpressured_changed_calls: self
                .ready_set_upload_backpressured_changed_calls
                .saturating_sub(baseline.ready_set_upload_backpressured_changed_calls),
            ready_set_upload_backpressured_unchanged_calls: self
                .ready_set_upload_backpressured_unchanged_calls
                .saturating_sub(baseline.ready_set_upload_backpressured_unchanged_calls),
            ready_set_skipped_calls: self
                .ready_set_skipped_calls
                .saturating_sub(baseline.ready_set_skipped_calls),
            ready_set_upload_backpressured_skipped_calls: self
                .ready_set_upload_backpressured_skipped_calls
                .saturating_sub(baseline.ready_set_upload_backpressured_skipped_calls),
            prepared_record_rebuilds: self
                .prepared_record_rebuilds
                .saturating_sub(baseline.prepared_record_rebuilds),
            prepared_record_rebuild_total_ms: (self.prepared_record_rebuild_total_ms
                - baseline.prepared_record_rebuild_total_ms)
                .max(0.0),
            // A max cannot be derived from cumulative counters; callers that
            // need sample-local max should track per-prepare samples.
            prepared_record_rebuild_max_ms: 0.0,
            prepared_record_rebuild_initial_dirty: self
                .prepared_record_rebuild_initial_dirty
                .saturating_sub(baseline.prepared_record_rebuild_initial_dirty),
            prepared_record_rebuild_section_upload_dirty: self
                .prepared_record_rebuild_section_upload_dirty
                .saturating_sub(baseline.prepared_record_rebuild_section_upload_dirty),
            prepared_record_rebuild_section_remove_dirty: self
                .prepared_record_rebuild_section_remove_dirty
                .saturating_sub(baseline.prepared_record_rebuild_section_remove_dirty),
            prepared_record_rebuild_ready_set_dirty: self
                .prepared_record_rebuild_ready_set_dirty
                .saturating_sub(baseline.prepared_record_rebuild_ready_set_dirty),
            prepared_record_rebuild_upload_backpressured_dirty: self
                .prepared_record_rebuild_upload_backpressured_dirty
                .saturating_sub(baseline.prepared_record_rebuild_upload_backpressured_dirty),
            prepared_record_rebuild_multi_dirty: self
                .prepared_record_rebuild_multi_dirty
                .saturating_sub(baseline.prepared_record_rebuild_multi_dirty),
            prepared_record_rebuild_visibility_section_max: 0,
            prepared_record_rebuild_loaded_section_max: 0,
            prepared_record_rebuild_ready_section_max: 0,
            prepared_record_rebuild_loaded_index_max: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TexturedSectionRecordDirtyCauses {
    bits: u8,
}

impl TexturedSectionRecordDirtyCauses {
    pub const INITIAL: Self = Self { bits: 1 << 0 };
    pub const SECTION_UPLOAD: Self = Self { bits: 1 << 1 };
    pub const SECTION_REMOVE: Self = Self { bits: 1 << 2 };
    pub const TRAVERSAL_READY: Self = Self { bits: 1 << 3 };
    pub const UPLOAD_BACKPRESSURED: Self = Self { bits: 1 << 4 };

    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub const fn with(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    pub const fn contains(self, other: Self) -> bool {
        self.bits & other.bits != 0
    }

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    pub fn cause_count(self) -> u32 {
        self.bits.count_ones()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TexturedSectionRecordPrepareStats {
    pub rebuilt: bool,
    pub rebuild_ms: f64,
    pub dirty_causes: TexturedSectionRecordDirtyCauses,
    pub visibility_section_count: usize,
    pub loaded_section_count: usize,
    pub ready_section_count: usize,
    pub loaded_index_count: u32,
    pub cache: TexturedSectionRecordCacheStats,
}

#[derive(Clone, Debug)]
struct TexturedSectionCullingResult {
    stats: TexturedSectionRenderStats,
    drawn_keys: FxHashSet<RenderSectionKey>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TexturedSectionMonoCullingCacheKey {
    render_view: ChunkRenderView,
    section_occlusion_culling: bool,
    force_fullbright: bool,
    topology: HorizontalTopology,
}

#[derive(Clone, Debug)]
struct TexturedSectionMonoCullingCache {
    key: TexturedSectionMonoCullingCacheKey,
    records: Arc<PreparedTexturedSectionRecords>,
    result: Arc<TexturedSectionCullingResult>,
}

impl TexturedSectionMonoCullingCache {
    fn matches(
        &self,
        key: TexturedSectionMonoCullingCacheKey,
        records: &Arc<PreparedTexturedSectionRecords>,
    ) -> bool {
        self.key == key && Arc::ptr_eq(&self.records, records)
    }
}

#[derive(Clone, Debug)]
struct TexturedSectionStereoCullingResult {
    stats: TexturedSectionRenderStats,
    eye_stats: [TexturedSectionRenderStats; 2],
    drawn_keys: FxHashSet<RenderSectionKey>,
    draw_masks: FxHashMap<RenderSectionKey, StereoDrawMask>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct StereoDrawMask(u8);

impl StereoDrawMask {
    const LEFT: Self = Self(0b01);
    const RIGHT: Self = Self(0b10);

    fn for_eye(eye: StereoEye) -> Self {
        match eye {
            StereoEye::Left => Self::LEFT,
            StereoEye::Right => Self::RIGHT,
        }
    }

    fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Debug)]
pub struct PreparedTexturedSectionStereoDraw {
    union_stats: [TexturedSectionRenderStats; 2],
    eye_stats: [TexturedSectionRenderStats; 2],
    drawn_keys: FxHashSet<RenderSectionKey>,
    draw_masks: FxHashMap<RenderSectionKey, StereoDrawMask>,
    translucent_keys: Vec<RenderSectionKey>,
}

impl PreparedTexturedSectionStereoDraw {
    pub fn stats(&self) -> [TexturedSectionRenderStats; 2] {
        self.union_stats
    }

    fn stats_for_eye(&self, eye: StereoEye) -> TexturedSectionRenderStats {
        match eye {
            StereoEye::Left => self.eye_stats[0],
            StereoEye::Right => self.eye_stats[1],
        }
    }

    fn draws_in_eye(&self, key: RenderSectionKey, eye: StereoEye) -> bool {
        self.draw_masks
            .get(&key)
            .is_some_and(|mask| mask.contains(StereoDrawMask::for_eye(eye)))
    }

    /// Visible translucent sections from the stereo union, expressed in the
    /// physical composition coordinate system. Scene composition qualifies
    /// these neutral records with its own world-instance identity before
    /// sorting multiple draw stores together.
    pub fn translucent_records(
        &self,
        placement: WorldPlacement,
    ) -> Vec<TexturedSectionTranslucentRecord> {
        self.translucent_keys
            .iter()
            .copied()
            .map(|key| textured_section_translucent_record(key, placement))
            .collect()
    }
}

/// Convert the uniform slot chosen by an explicitly stereo render API into its
/// typed eye. Generic flat-view code never calls this conversion.
fn stereo_eye_for_slot(view_slot: PerViewSlot) -> StereoEye {
    match view_slot.view().get() {
        0 => StereoEye::Left,
        1 => StereoEye::Right,
        index => panic!("stereo draw received presentation view index {index}"),
    }
}

/// Renderer-neutral input for scene-level translucent composition.
///
/// The renderer owns only the plain per-store section key and its physical
/// center. World identity and source selection stay at the scene/app-runtime
/// boundary rather than turning a draw store into a world registry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexturedSectionTranslucentRecord {
    pub key: RenderSectionKey,
    pub composition_center: Vec3,
}

// Slice G (docs/tactical/106): the per-eye cull previously allocated three
// `BTreeSet`s + a `BTreeMap` + a `VecDeque` every eye every frame, and every
// membership test/insert paid `O(log n)` plus pointer chasing. `RenderSectionKey`
// is a 12-byte `Copy` key, so a flat fast-hash set/map is a much better fit. We
// also keep these scratch containers alive across calls (cleared, capacity
// retained) so the steady-state frozen frame does no cull-side allocation at all.
// `drawn_keys` is returned to the encode loop, so it is built fresh per call.
#[derive(Default)]
struct CullScratch {
    frustum_keys: FxHashSet<RenderSectionKey>,
    frustum_masks: FxHashMap<RenderSectionKey, StereoDrawMask>,
    ready_frustum_keys: FxHashSet<RenderSectionKey>,
    queue: VecDeque<RenderSectionKey>,
    infos: FxHashMap<RenderSectionKey, RenderSectionTraversalInfo>,
}

impl CullScratch {
    fn reset(&mut self) {
        self.frustum_keys.clear();
        self.frustum_masks.clear();
        self.ready_frustum_keys.clear();
        self.queue.clear();
        self.infos.clear();
    }
}

#[derive(Clone, Copy, Debug)]
struct RenderSectionTraversalInfo {
    source_directions: u8,
    directions: u8,
}

impl RenderSectionTraversalInfo {
    fn start() -> Self {
        Self {
            source_directions: 0,
            directions: 0,
        }
    }

    fn from_parent(parent_directions: u8, direction: SectionFace) -> Self {
        Self {
            source_directions: direction.mask(),
            directions: parent_directions | direction.mask(),
        }
    }

    fn has_direction(self, direction: SectionFace) -> bool {
        self.directions & direction.mask() != 0
    }

    fn add_source_direction(&mut self, direction: SectionFace) {
        self.source_directions |= direction.mask();
    }

    fn has_source_directions(self) -> bool {
        self.source_directions != 0
    }
}

fn cull_textured_sections(
    prepared: &PreparedTexturedSectionRecords,
    render_view: ChunkRenderView,
    options: TexturedSectionRenderOptions,
    scratch: &mut CullScratch,
) -> TexturedSectionCullingResult {
    let frustum = ClipFrustum::from_render_view(render_view, options.topology);
    cull_textured_sections_with_frustum(prepared, render_view, options, scratch, &frustum)
}

fn cull_textured_sections_with_frustum(
    prepared: &PreparedTexturedSectionRecords,
    render_view: ChunkRenderView,
    options: TexturedSectionRenderOptions,
    scratch: &mut CullScratch,
    frustum: &impl RenderSectionFrustum,
) -> TexturedSectionCullingResult {
    let records = &prepared.records;
    let mut stats = TexturedSectionRenderStats {
        section_occlusion_culling: options.section_occlusion_culling,
        force_fullbright: options.force_fullbright,
        // Slice F: precomputed once at record-build time, not re-summed per eye.
        loaded_section_count: prepared.loaded_section_count,
        loaded_index_count: prepared.loaded_index_count,
        ..TexturedSectionRenderStats::default()
    };

    // Slice G: reuse scratch capacity (cleared) instead of allocating three sets
    // + a map + a queue per eye per frame; fast-hash membership instead of BTree.
    scratch.reset();
    let CullScratch {
        frustum_keys,
        frustum_masks: _,
        ready_frustum_keys,
        queue,
        infos,
    } = scratch;
    for (key, record) in records {
        if frustum.is_render_section_visible(*key) {
            frustum_keys.insert(*key);
            if record.traversal_ready {
                ready_frustum_keys.insert(*key);
            }
            if record.drawable {
                stats.frustum_section_count += 1;
                stats.frustum_index_count += record.index_count;
                if !record.traversal_ready {
                    stats.readiness_culled_section_count += 1;
                    stats.readiness_culled_index_count += record.index_count;
                }
            }
        }
    }

    let start_keys = traversal_start_keys(
        records,
        frustum_keys,
        render_view.camera_position,
        options.topology,
    );
    if start_keys.is_empty() {
        stats.drawn_section_count = ready_frustum_keys
            .iter()
            .filter_map(|key| records.get(key))
            .filter(|record| record.drawable)
            .count();
        stats.drawn_index_count = ready_frustum_keys
            .iter()
            .filter_map(|key| records.get(key))
            .filter(|record| record.drawable)
            .map(|record| record.index_count)
            .sum();
        return TexturedSectionCullingResult {
            stats,
            // `ready_frustum_keys` is reused scratch, so hand back an owned copy.
            drawn_keys: ready_frustum_keys.iter().copied().collect(),
        };
    }

    let mut drawn_keys = FxHashSet::default();
    for start_key in start_keys {
        infos.insert(start_key, RenderSectionTraversalInfo::start());
        queue.push_back(start_key);
    }

    while let Some(key) = queue.pop_front() {
        let Some(record) = records.get(&key) else {
            continue;
        };
        let Some(info) = infos.get(&key).copied() else {
            continue;
        };
        if frustum_keys.contains(&key) {
            drawn_keys.insert(key);
        }

        for direction in SectionFace::ALL {
            if info.has_direction(direction.opposite()) {
                continue;
            }
            if options.section_occlusion_culling
                && info.has_source_directions()
                && !can_see_through_source(record.visibility, info.source_directions, direction)
            {
                continue;
            }

            let Some(neighbor_key) = section_neighbor_key_in(options.topology, key, direction)
            else {
                continue;
            };
            let Some(neighbor_record) = records.get(&neighbor_key) else {
                continue;
            };
            if !neighbor_record.traversal_ready || !frustum_keys.contains(&neighbor_key) {
                continue;
            }

            if let Some(existing) = infos.get_mut(&neighbor_key) {
                existing.add_source_direction(direction);
            } else {
                infos.insert(
                    neighbor_key,
                    RenderSectionTraversalInfo::from_parent(info.directions, direction),
                );
                queue.push_back(neighbor_key);
            }
        }
    }

    stats.graph_cull_enabled = options.section_occlusion_culling;
    stats.drawn_section_count = drawn_keys
        .iter()
        .filter_map(|key| records.get(key))
        .filter(|record| record.drawable)
        .count();
    stats.drawn_index_count = drawn_keys
        .iter()
        .filter_map(|key| records.get(key))
        .filter(|record| record.drawable)
        .map(|record| record.index_count)
        .sum();
    stats.graph_culled_section_count = stats
        .frustum_section_count
        .saturating_sub(stats.readiness_culled_section_count)
        .saturating_sub(stats.drawn_section_count);
    stats.graph_culled_index_count = stats
        .frustum_index_count
        .saturating_sub(stats.readiness_culled_index_count)
        .saturating_sub(stats.drawn_index_count);

    TexturedSectionCullingResult { stats, drawn_keys }
}

fn cull_textured_sections_stereo_union(
    prepared: &PreparedTexturedSectionRecords,
    render_views: [ChunkRenderView; 2],
    options: [TexturedSectionRenderOptions; 2],
    scratch: &mut CullScratch,
) -> TexturedSectionStereoCullingResult {
    let frustums =
        render_views.map(|view| ClipFrustum::from_render_view(view, options[0].topology));
    cull_textured_sections_stereo_union_with_frustums(
        prepared,
        render_views,
        options,
        scratch,
        &frustums,
    )
}

fn cull_textured_sections_stereo_union_with_frustums(
    prepared: &PreparedTexturedSectionRecords,
    render_views: [ChunkRenderView; 2],
    options: [TexturedSectionRenderOptions; 2],
    scratch: &mut CullScratch,
    frustums: &[impl RenderSectionFrustum; 2],
) -> TexturedSectionStereoCullingResult {
    let records = &prepared.records;
    let section_occlusion_culling =
        options[0].section_occlusion_culling && options[1].section_occlusion_culling;
    let force_fullbright = options[0].force_fullbright || options[1].force_fullbright;
    let mut stats = TexturedSectionRenderStats {
        section_occlusion_culling,
        force_fullbright,
        loaded_section_count: prepared.loaded_section_count,
        loaded_index_count: prepared.loaded_index_count,
        ..TexturedSectionRenderStats::default()
    };
    let mut eye_stats = std::array::from_fn(|eye| TexturedSectionRenderStats {
        section_occlusion_culling: options[eye].section_occlusion_culling,
        force_fullbright: options[eye].force_fullbright,
        loaded_section_count: prepared.loaded_section_count,
        loaded_index_count: prepared.loaded_index_count,
        ..TexturedSectionRenderStats::default()
    });

    scratch.reset();
    let CullScratch {
        frustum_keys,
        frustum_masks,
        ready_frustum_keys,
        queue,
        infos,
    } = scratch;
    for (key, record) in records {
        let mut mask = StereoDrawMask::default();
        for eye in 0..2 {
            if frustums[eye].is_render_section_visible(*key) {
                mask.insert(if eye == 0 {
                    StereoDrawMask::LEFT
                } else {
                    StereoDrawMask::RIGHT
                });
                if record.drawable {
                    eye_stats[eye].frustum_section_count += 1;
                    eye_stats[eye].frustum_index_count += record.index_count;
                    if !record.traversal_ready {
                        eye_stats[eye].readiness_culled_section_count += 1;
                        eye_stats[eye].readiness_culled_index_count += record.index_count;
                    }
                }
            }
        }
        if !mask.is_empty() {
            frustum_keys.insert(*key);
            frustum_masks.insert(*key, mask);
            if record.traversal_ready {
                ready_frustum_keys.insert(*key);
            }
            if record.drawable {
                stats.frustum_section_count += 1;
                stats.frustum_index_count += record.index_count;
                if !record.traversal_ready {
                    stats.readiness_culled_section_count += 1;
                    stats.readiness_culled_index_count += record.index_count;
                }
            }
        }
    }

    let center_position = stereo_center_position(render_views);
    let start_keys =
        traversal_start_keys(records, frustum_keys, center_position, options[0].topology);
    if start_keys.is_empty() {
        stats.drawn_section_count = ready_frustum_keys
            .iter()
            .filter_map(|key| records.get(key))
            .filter(|record| record.drawable)
            .count();
        stats.drawn_index_count = ready_frustum_keys
            .iter()
            .filter_map(|key| records.get(key))
            .filter(|record| record.drawable)
            .map(|record| record.index_count)
            .sum();
        let drawn_keys = ready_frustum_keys.iter().copied().collect::<FxHashSet<_>>();
        let draw_masks = stereo_draw_masks_from_frustum_masks(&drawn_keys, frustum_masks);
        finalize_stereo_eye_stats(&mut eye_stats, records, &draw_masks, false);
        return TexturedSectionStereoCullingResult {
            stats,
            eye_stats,
            drawn_keys,
            draw_masks,
        };
    }

    let mut drawn_keys = FxHashSet::default();
    for start_key in start_keys {
        infos.insert(start_key, RenderSectionTraversalInfo::start());
        queue.push_back(start_key);
    }

    while let Some(key) = queue.pop_front() {
        let Some(record) = records.get(&key) else {
            continue;
        };
        let Some(info) = infos.get(&key).copied() else {
            continue;
        };
        if frustum_keys.contains(&key) {
            drawn_keys.insert(key);
        }

        for direction in SectionFace::ALL {
            if info.has_direction(direction.opposite()) {
                continue;
            }
            if section_occlusion_culling
                && info.has_source_directions()
                && !can_see_through_source(record.visibility, info.source_directions, direction)
            {
                continue;
            }

            let Some(neighbor_key) = section_neighbor_key_in(options[0].topology, key, direction)
            else {
                continue;
            };
            let Some(neighbor_record) = records.get(&neighbor_key) else {
                continue;
            };
            if !neighbor_record.traversal_ready || !frustum_keys.contains(&neighbor_key) {
                continue;
            }

            if let Some(existing) = infos.get_mut(&neighbor_key) {
                existing.add_source_direction(direction);
            } else {
                infos.insert(
                    neighbor_key,
                    RenderSectionTraversalInfo::from_parent(info.directions, direction),
                );
                queue.push_back(neighbor_key);
            }
        }
    }

    stats.graph_cull_enabled = section_occlusion_culling;
    for stats in &mut eye_stats {
        stats.graph_cull_enabled = section_occlusion_culling;
    }
    stats.drawn_section_count = drawn_keys
        .iter()
        .filter_map(|key| records.get(key))
        .filter(|record| record.drawable)
        .count();
    stats.drawn_index_count = drawn_keys
        .iter()
        .filter_map(|key| records.get(key))
        .filter(|record| record.drawable)
        .map(|record| record.index_count)
        .sum();
    stats.graph_culled_section_count = stats
        .frustum_section_count
        .saturating_sub(stats.readiness_culled_section_count)
        .saturating_sub(stats.drawn_section_count);
    stats.graph_culled_index_count = stats
        .frustum_index_count
        .saturating_sub(stats.readiness_culled_index_count)
        .saturating_sub(stats.drawn_index_count);

    let draw_masks = stereo_draw_masks_from_frustum_masks(&drawn_keys, frustum_masks);
    finalize_stereo_eye_stats(
        &mut eye_stats,
        records,
        &draw_masks,
        section_occlusion_culling,
    );
    TexturedSectionStereoCullingResult {
        stats,
        eye_stats,
        drawn_keys,
        draw_masks,
    }
}

fn stereo_draw_masks_from_frustum_masks(
    drawn_keys: &FxHashSet<RenderSectionKey>,
    frustum_masks: &FxHashMap<RenderSectionKey, StereoDrawMask>,
) -> FxHashMap<RenderSectionKey, StereoDrawMask> {
    let mut masks = FxHashMap::default();
    for key in drawn_keys {
        if let Some(mask) = frustum_masks.get(key).copied() {
            masks.insert(*key, mask);
        }
    }
    masks
}

fn finalize_stereo_eye_stats(
    eye_stats: &mut [TexturedSectionRenderStats; 2],
    records: &BTreeMap<RenderSectionKey, TexturedSectionCullingRecord>,
    draw_masks: &FxHashMap<RenderSectionKey, StereoDrawMask>,
    graph_cull_enabled: bool,
) {
    for stats in eye_stats.iter_mut() {
        stats.graph_cull_enabled = graph_cull_enabled;
        stats.drawn_section_count = 0;
        stats.drawn_index_count = 0;
    }

    for (key, mask) in draw_masks {
        let Some(record) = records.get(key) else {
            continue;
        };
        if !record.drawable {
            continue;
        }
        for (eye, eye_mask) in [StereoDrawMask::LEFT, StereoDrawMask::RIGHT]
            .into_iter()
            .enumerate()
        {
            if mask.contains(eye_mask) {
                eye_stats[eye].drawn_section_count += 1;
                eye_stats[eye].drawn_index_count += record.index_count;
            }
        }
    }

    for stats in eye_stats.iter_mut() {
        stats.graph_culled_section_count = stats
            .frustum_section_count
            .saturating_sub(stats.readiness_culled_section_count)
            .saturating_sub(stats.drawn_section_count);
        stats.graph_culled_index_count = stats
            .frustum_index_count
            .saturating_sub(stats.readiness_culled_index_count)
            .saturating_sub(stats.drawn_index_count);
    }
}

fn traversal_start_keys(
    records: &BTreeMap<RenderSectionKey, TexturedSectionCullingRecord>,
    frustum_keys: &FxHashSet<RenderSectionKey>,
    camera_position: Vec3,
    topology: HorizontalTopology,
) -> Vec<RenderSectionKey> {
    let Some(camera_key) = render_section_key_containing_in(topology, camera_position) else {
        return Vec::new();
    };
    if records
        .get(&camera_key)
        .is_some_and(|record| record.traversal_ready && record.drawable)
    {
        return vec![camera_key];
    }

    // Vanilla's full-height ViewArea can traverse from an empty camera section
    // through retained empty neighbors to the visible terrain. Native keeps a
    // sparse resident graph, so an empty camera record may have no continuous
    // path to the surface and cannot safely be the sole traversal seed.
    outside_retained_section_start_keys(records, frustum_keys, camera_position, topology)
}

fn outside_retained_section_start_keys(
    records: &BTreeMap<RenderSectionKey, TexturedSectionCullingRecord>,
    frustum_keys: &FxHashSet<RenderSectionKey>,
    camera_position: Vec3,
    topology: HorizontalTopology,
) -> Vec<RenderSectionKey> {
    if !camera_position.is_finite() {
        return Vec::new();
    }
    let Some(min_section_y) = records.keys().map(|key| key.section_y).min() else {
        return Vec::new();
    };
    let min_build_y = min_section_y * RENDER_SECTION_HEIGHT;
    let seed_from_above = camera_position.y.floor() as i32 > min_build_y;

    // Minecraft seeds every visible column at the world's top or bottom render
    // layer when the camera has no containing RenderChunk. Native retains only
    // resident sections rather than a full-height ViewArea, so a single global
    // section layer can omit shorter columns entirely. Use each column's
    // outermost visible, drawable, traversal-ready resident section as its
    // equivalent entry point. An empty resident section is not a sufficient
    // seed: sparse native retention can leave the visible surface separated
    // from it by non-frustum records, so the graph could never reach the first
    // section that actually paints the column.
    let mut starts_by_column = BTreeMap::<(i32, i32), RenderSectionKey>::new();
    for key in records.keys().copied().filter(|key| {
        frustum_keys.contains(key)
            && records
                .get(key)
                .is_some_and(|record| record.traversal_ready && record.drawable)
    }) {
        starts_by_column
            .entry((key.chunk_x, key.chunk_z))
            .and_modify(|current| {
                let is_better = if seed_from_above {
                    key.section_y > current.section_y
                } else {
                    key.section_y < current.section_y
                };
                if is_better {
                    *current = key;
                }
            })
            .or_insert(key);
    }
    let mut starts = starts_by_column.into_values().collect::<Vec<_>>();
    starts.sort_by(|a, b| {
        let a_distance = render_section_center_in(*a, camera_position, topology)
            .distance_squared(camera_position);
        let b_distance = render_section_center_in(*b, camera_position, topology)
            .distance_squared(camera_position);
        a_distance.total_cmp(&b_distance)
    });
    starts
}

fn can_see_through_source(
    visibility: VisibilitySet,
    source_directions: u8,
    exit_direction: SectionFace,
) -> bool {
    SectionFace::ALL.into_iter().any(|source| {
        source_directions & source.mask() != 0
            && visibility.visibility_between(source.opposite(), exit_direction)
    })
}

fn render_section_key_containing_in(
    topology: HorizontalTopology,
    position: Vec3,
) -> Option<RenderSectionKey> {
    if !position.is_finite() {
        return None;
    }
    let block_x = position.x.floor() as i32;
    let block_y = position.y.floor() as i32;
    let block_z = position.z.floor() as i32;
    let chunk = topology.canonicalize_chunk(mclone_core::ChunkPos::new(
        block_to_chunk_coord(block_x),
        block_to_chunk_coord(block_z),
    ))?;
    Some(RenderSectionKey::new(
        chunk.x,
        block_to_section_coord(block_y),
        chunk.z,
    ))
}

fn section_neighbor_key_in(
    topology: HorizontalTopology,
    key: RenderSectionKey,
    face: SectionFace,
) -> Option<RenderSectionKey> {
    let [dx, dy, dz] = face.section_delta();
    let chunk =
        topology.neighbor_chunk(mclone_core::ChunkPos::new(key.chunk_x, key.chunk_z), dx, dz)?;
    Some(RenderSectionKey::new(chunk.x, key.section_y + dy, chunk.z))
}

fn render_section_center_in(
    key: RenderSectionKey,
    observer: Vec3,
    topology: HorizontalTopology,
) -> Vec3 {
    let lifted = topology.nearest_chunk_lift(
        mclone_core::ChunkPos::new(key.chunk_x, key.chunk_z),
        Vec3d::new(
            f64::from(observer.x),
            f64::from(observer.y),
            f64::from(observer.z),
        ),
    );
    Vec3::new(
        lifted.x as f32 * MESH_CHUNK_WIDTH as f32 + MESH_CHUNK_WIDTH as f32 * 0.5,
        key.min_y() as f32 + RENDER_SECTION_HEIGHT as f32 * 0.5,
        lifted.z as f32 * MESH_CHUNK_WIDTH as f32 + MESH_CHUNK_WIDTH as f32 * 0.5,
    )
}

#[derive(Clone, Copy, Debug)]
struct ClipFrustum {
    view_projection: Mat4,
    camera_position: Vec3,
    topology: HorizontalTopology,
}

trait RenderSectionFrustum {
    fn is_render_section_visible(&self, key: RenderSectionKey) -> bool;
}

impl ClipFrustum {
    fn from_render_view(render_view: ChunkRenderView, topology: HorizontalTopology) -> Self {
        Self {
            view_projection: render_view.view_projection,
            camera_position: render_view.camera_position,
            topology,
        }
    }

    fn is_aabb_visible(&self, min: Vec3, max: Vec3) -> bool {
        clip_aabb_visible(min, max, |corner| {
            self.view_projection * Vec4::new(corner.x, corner.y, corner.z, 1.0)
        })
    }
}

impl RenderSectionFrustum for ClipFrustum {
    fn is_render_section_visible(&self, key: RenderSectionKey) -> bool {
        let center = render_section_center_in(key, self.camera_position, self.topology);
        let half = Vec3::new(
            MESH_CHUNK_WIDTH as f32 * 0.5,
            RENDER_SECTION_HEIGHT as f32 * 0.5,
            MESH_CHUNK_WIDTH as f32 * 0.5,
        );
        let margin = Vec3::splat(BUSHY_LEAF_CARD_OVERHANG);
        let min = center - half - margin;
        let max = center - half
            + Vec3::new(
                MESH_CHUNK_WIDTH as f32,
                RENDER_SECTION_HEIGHT as f32,
                MESH_CHUNK_WIDTH as f32,
            )
            + margin;
        self.is_aabb_visible(min, max)
    }
}

#[derive(Clone, Copy, Debug)]
struct PlacedClipFrustum {
    physical_view_projection: Mat4,
    placement: WorldPlacement,
    clip: CompositionClip,
}

impl PlacedClipFrustum {
    fn new(physical_render_view: ChunkRenderView, context: WorldCompositionContext) -> Self {
        Self {
            physical_view_projection: physical_render_view.view_projection,
            placement: context.placement(),
            clip: context.clip(),
        }
    }

    fn source_to_composition(self, source: Vec3) -> Vec3 {
        self.placement.source_to_composition_f32(source)
    }
}

impl RenderSectionFrustum for PlacedClipFrustum {
    fn is_render_section_visible(&self, key: RenderSectionKey) -> bool {
        let margin = Vec3::splat(BUSHY_LEAF_CARD_OVERHANG);
        let min = Vec3::new(
            chunk_min_block_coord(key.chunk_x) as f32,
            key.min_y() as f32,
            chunk_min_block_coord(key.chunk_z) as f32,
        ) - margin;
        let max =
            min + Vec3::new(
                MESH_CHUNK_WIDTH as f32,
                RENDER_SECTION_HEIGHT as f32,
                MESH_CHUNK_WIDTH as f32,
            ) + margin * 2.0;
        let composition_min = self.source_to_composition(min);
        let composition_max = self.source_to_composition(max);
        if self.clip.rejects_aabb(composition_min, composition_max) {
            return false;
        }
        clip_aabb_visible(min, max, |source_corner| {
            let composition_corner = self.source_to_composition(source_corner);
            self.physical_view_projection
                * Vec4::new(
                    composition_corner.x,
                    composition_corner.y,
                    composition_corner.z,
                    1.0,
                )
        })
    }
}

fn clip_aabb_visible(min: Vec3, max: Vec3, mut clip_position: impl FnMut(Vec3) -> Vec4) -> bool {
    let corners = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(min.x, max.y, max.z),
        Vec3::new(max.x, max.y, max.z),
    ];
    let mut outside_left = true;
    let mut outside_right = true;
    let mut outside_bottom = true;
    let mut outside_top = true;
    let mut outside_near = true;
    let mut outside_far = true;

    for corner in corners {
        let clip = clip_position(corner);
        outside_left &= clip.x < -clip.w;
        outside_right &= clip.x > clip.w;
        outside_bottom &= clip.y < -clip.w;
        outside_top &= clip.y > clip.w;
        outside_near &= clip.z < -clip.w;
        outside_far &= clip.z > clip.w;
    }

    !(outside_left || outside_right || outside_bottom || outside_top || outside_near || outside_far)
}

pub(crate) fn render_view_aabb_visible(render_view: ChunkRenderView, min: Vec3, max: Vec3) -> bool {
    clip_aabb_visible(min, max, |corner| {
        render_view.view_projection * corner.extend(1.0)
    })
}

pub struct GpuChunkMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl GpuChunkMesh {
    pub fn new(device: &wgpu::Device, mesh: &VisibleChunkMesh) -> Result<Self> {
        if mesh.is_empty() {
            bail!("cannot upload an empty chunk mesh");
        }
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_chunk_vertices"),
            contents: &vertex_bytes(mesh),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_chunk_indices"),
            contents: &index_bytes(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Ok(Self {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
        })
    }

    pub fn index_count(&self) -> u32 {
        self.index_count
    }
}

pub struct GpuTexturedChunkMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_count: u32,
    index_count: u32,
    solid_index_count: u32,
    opaque_index_count: u32,
    visibility: VisibilitySet,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct GpuTexturedChunkMeshUploadTiming {
    total_ms: f64,
    vertex_bytes_ms: f64,
    vertex_buffer_ms: f64,
    index_bytes_ms: f64,
    index_buffer_ms: f64,
}

impl GpuTexturedChunkMesh {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &TexturedVisibleChunkMesh,
    ) -> Result<Self> {
        Self::with_visibility(device, queue, mesh, VisibilitySet::all_visible())
    }

    pub fn with_visibility(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &TexturedVisibleChunkMesh,
        visibility: VisibilitySet,
    ) -> Result<Self> {
        Self::with_visibility_timed(device, queue, mesh, visibility).map(|(mesh, _timing)| mesh)
    }

    fn with_visibility_timed(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &TexturedVisibleChunkMesh,
        visibility: VisibilitySet,
    ) -> Result<(Self, GpuTexturedChunkMeshUploadTiming)> {
        if mesh.is_empty() {
            bail!("cannot upload an empty textured chunk mesh");
        }
        let total_start = timing_now();
        let mut timing = GpuTexturedChunkMeshUploadTiming::default();
        let vertex_bytes_start = timing_now();
        let vertex_bytes = textured_vertex_bytes(mesh);
        timing.vertex_bytes_ms = timing_elapsed_ms(vertex_bytes_start);
        let vertex_buffer_start = timing_now();
        let vertex_buffer = create_uploaded_buffer(
            device,
            queue,
            "mclone_textured_chunk_vertices",
            &vertex_bytes,
            wgpu::BufferUsages::VERTEX,
        );
        timing.vertex_buffer_ms = timing_elapsed_ms(vertex_buffer_start);
        let index_bytes_start = timing_now();
        let indices = index_bytes(&mesh.indices);
        timing.index_bytes_ms = timing_elapsed_ms(index_bytes_start);
        let index_buffer_start = timing_now();
        let index_buffer = create_uploaded_buffer(
            device,
            queue,
            "mclone_textured_chunk_indices",
            &indices,
            wgpu::BufferUsages::INDEX,
        );
        timing.index_buffer_ms = timing_elapsed_ms(index_buffer_start);
        timing.total_ms = timing_elapsed_ms(total_start);
        Ok((
            Self {
                vertex_buffer,
                index_buffer,
                vertex_count: mesh.vertices.len() as u32,
                index_count: mesh.indices.len() as u32,
                solid_index_count: mesh.solid_index_count().min(mesh.indices.len() as u32),
                opaque_index_count: mesh.opaque_index_count().min(mesh.indices.len() as u32),
                visibility,
            },
            timing,
        ))
    }

    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    pub fn opaque_index_range(&self) -> Range<u32> {
        0..self.opaque_index_count.min(self.index_count)
    }

    pub fn solid_index_range(&self) -> Range<u32> {
        0..self.solid_index_count.min(self.index_count)
    }

    pub fn cutout_index_range(&self) -> Range<u32> {
        self.solid_index_count.min(self.index_count)..self.opaque_index_count.min(self.index_count)
    }

    pub fn translucent_index_range(&self) -> Range<u32> {
        self.opaque_index_count.min(self.index_count)..self.index_count
    }

    pub fn visibility(&self) -> VisibilitySet {
        self.visibility
    }
}

fn create_uploaded_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    contents: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: contents.len().max(4) as wgpu::BufferAddress,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, contents);
    buffer
}

fn draw_textured_mesh_range(
    pass: &mut wgpu::RenderPass<'_>,
    mesh: &GpuTexturedChunkMesh,
    index_range: Range<u32>,
) {
    if index_range.is_empty() {
        return;
    }
    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
    pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    pass.draw_indexed(index_range, 0, 0..1);
}

const TERRAIN_ARENA_MIN_VERTEX_CAPACITY: u32 = 262_144;
const TERRAIN_ARENA_MIN_INDEX_CAPACITY: u32 = 393_216;
const TERRAIN_INDIRECT_PHASE_SLOT_COUNT: u32 = 3;
const TERRAIN_INDIRECT_SLOT_COUNT: u32 =
    PER_VIEW_UNIFORM_SLOT_COUNT * TERRAIN_INDIRECT_PHASE_SLOT_COUNT;
const DRAW_INDEXED_INDIRECT_ARG_BYTES: wgpu::BufferAddress =
    std::mem::size_of::<wgpu::util::DrawIndexedIndirectArgs>() as wgpu::BufferAddress;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ElementRangeAllocator {
    capacity: u32,
    free: BTreeMap<u32, u32>,
}

impl ElementRangeAllocator {
    fn new(capacity: u32) -> Self {
        let mut free = BTreeMap::new();
        if capacity > 0 {
            free.insert(0, capacity);
        }
        Self { capacity, free }
    }

    fn allocate(&mut self, count: u32) -> Option<Range<u32>> {
        if count == 0 {
            return None;
        }
        let (start, available) = self
            .free
            .iter()
            .filter(|(_, available)| **available >= count)
            .min_by_key(|(start, available)| (**available, **start))
            .map(|(start, available)| (*start, *available))?;
        self.free.remove(&start);
        if available > count {
            self.free.insert(start + count, available - count);
        }
        Some(start..start + count)
    }

    fn release(&mut self, range: Range<u32>) {
        if range.is_empty() {
            return;
        }
        debug_assert!(range.end <= self.capacity);
        let mut start = range.start;
        let mut end = range.end;
        if let Some((previous_start, previous_count)) = self
            .free
            .range(..=start)
            .next_back()
            .map(|(start, count)| (*start, *count))
            && previous_start + previous_count == start
        {
            self.free.remove(&previous_start);
            start = previous_start;
        }
        if let Some((next_start, next_count)) = self
            .free
            .range(start..)
            .next()
            .map(|(start, count)| (*start, *count))
            && end == next_start
        {
            self.free.remove(&next_start);
            end = next_start + next_count;
        }
        self.free.insert(start, end - start);
    }

    fn extend(&mut self, capacity: u32) {
        assert!(capacity >= self.capacity);
        let previous = self.capacity;
        self.capacity = capacity;
        self.release(previous..capacity);
    }

    fn used(&self) -> u32 {
        self.capacity
            .saturating_sub(self.free.values().copied().sum())
    }
}

#[derive(Clone, Debug)]
struct GpuTexturedSectionMesh {
    vertex_page: usize,
    vertex_range: Range<u32>,
    index_range: Range<u32>,
    solid_index_count: u32,
    opaque_index_count: u32,
}

impl GpuTexturedSectionMesh {
    fn vertex_count(&self) -> u32 {
        self.vertex_range.len() as u32
    }

    fn index_count(&self) -> u32 {
        self.index_range.len() as u32
    }

    fn solid_index_range(&self) -> Range<u32> {
        0..self.solid_index_count.min(self.index_count())
    }

    fn cutout_index_range(&self) -> Range<u32> {
        self.solid_index_count.min(self.index_count())
            ..self.opaque_index_count.min(self.index_count())
    }

    fn translucent_index_range(&self) -> Range<u32> {
        self.opaque_index_count.min(self.index_count())..self.index_count()
    }

    fn indirect_args(&self, range: Range<u32>) -> wgpu::util::DrawIndexedIndirectArgs {
        wgpu::util::DrawIndexedIndirectArgs {
            index_count: range.len() as u32,
            instance_count: 1,
            first_index: self.index_range.start + range.start,
            base_vertex: self.vertex_range.start as i32,
            first_instance: 0,
        }
    }
}

struct GpuTexturedSectionArena {
    vertex_pages: Vec<GpuTexturedSectionVertexArena>,
    index_buffer: wgpu::Buffer,
    index_ranges: ElementRangeAllocator,
    indirect_buffer: wgpu::Buffer,
    indirect_draw_capacity_per_slot: u32,
    max_vertex_capacity: u32,
    max_index_capacity: u32,
    multi_draw_indirect: bool,
}

struct GpuTexturedSectionVertexArena {
    buffer: wgpu::Buffer,
    ranges: ElementRangeAllocator,
}

impl GpuTexturedSectionArena {
    fn new(
        device: &wgpu::Device,
        initial_vertex_count: u32,
        initial_index_count: u32,
        initial_section_count: usize,
    ) -> Result<Self> {
        let max_buffer_size = device.limits().max_buffer_size;
        let max_vertex_capacity = u32::try_from(max_buffer_size / TEXTURED_VERTEX_BYTE_SIZE)
            .unwrap_or(u32::MAX)
            .min(i32::MAX as u32);
        let max_index_capacity =
            u32::try_from(max_buffer_size / std::mem::size_of::<u32>() as u64).unwrap_or(u32::MAX);
        let vertex_capacity = initial_arena_capacity(
            initial_vertex_count.min(max_vertex_capacity),
            TERRAIN_ARENA_MIN_VERTEX_CAPACITY,
            max_vertex_capacity,
        )?;
        let index_capacity = initial_arena_capacity(
            initial_index_count,
            TERRAIN_ARENA_MIN_INDEX_CAPACITY,
            max_index_capacity,
        )?;
        let required_indirect_draws =
            u32::try_from(initial_section_count.saturating_mul(3).max(1)).unwrap_or(u32::MAX);
        let indirect_draw_capacity_per_slot = required_indirect_draws
            .checked_next_power_of_two()
            .unwrap_or(u32::MAX);
        let indirect_size = indirect_buffer_size(indirect_draw_capacity_per_slot)?;
        if indirect_size > max_buffer_size {
            bail!(
                "terrain indirect buffer request {indirect_size} exceeds adapter limit {max_buffer_size}"
            );
        }
        Ok(Self {
            vertex_pages: vec![GpuTexturedSectionVertexArena {
                buffer: create_textured_section_vertex_arena(device, vertex_capacity),
                ranges: ElementRangeAllocator::new(vertex_capacity),
            }],
            index_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_textured_section_index_arena"),
                size: u64::from(index_capacity) * std::mem::size_of::<u32>() as u64,
                usage: wgpu::BufferUsages::INDEX
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            index_ranges: ElementRangeAllocator::new(index_capacity),
            indirect_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_textured_section_indirect_draws"),
                size: indirect_size,
                usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            indirect_draw_capacity_per_slot,
            max_vertex_capacity,
            max_index_capacity,
            multi_draw_indirect: device
                .features()
                .contains(wgpu::Features::MULTI_DRAW_INDIRECT),
        })
    }

    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &TexturedVisibleChunkMesh,
    ) -> Result<(GpuTexturedSectionMesh, GpuTexturedChunkMeshUploadTiming)> {
        if mesh.is_empty() {
            bail!("cannot upload an empty textured section mesh");
        }
        let total_start = timing_now();
        let mut timing = GpuTexturedChunkMeshUploadTiming::default();
        let vertex_bytes_start = timing_now();
        let vertex_bytes = textured_vertex_bytes(mesh);
        timing.vertex_bytes_ms = timing_elapsed_ms(vertex_bytes_start);
        let vertex_buffer_start = timing_now();
        let (vertex_page, vertex_range) =
            self.allocate_vertices(device, queue, mesh.vertices.len() as u32)?;
        queue.write_buffer(
            &self.vertex_pages[vertex_page].buffer,
            u64::from(vertex_range.start) * TEXTURED_VERTEX_BYTE_SIZE,
            &vertex_bytes,
        );
        timing.vertex_buffer_ms = timing_elapsed_ms(vertex_buffer_start);

        let index_bytes_start = timing_now();
        let indices = index_bytes(&mesh.indices);
        timing.index_bytes_ms = timing_elapsed_ms(index_bytes_start);
        let index_buffer_start = timing_now();
        let index_range = match self.allocate_indices(device, queue, mesh.indices.len() as u32) {
            Ok(range) => range,
            Err(error) => {
                self.vertex_pages[vertex_page].ranges.release(vertex_range);
                return Err(error);
            }
        };
        queue.write_buffer(
            &self.index_buffer,
            u64::from(index_range.start) * std::mem::size_of::<u32>() as u64,
            &indices,
        );
        timing.index_buffer_ms = timing_elapsed_ms(index_buffer_start);
        timing.total_ms = timing_elapsed_ms(total_start);
        Ok((
            GpuTexturedSectionMesh {
                vertex_page,
                vertex_range,
                index_range,
                solid_index_count: mesh.solid_index_count().min(mesh.indices.len() as u32),
                opaque_index_count: mesh.opaque_index_count().min(mesh.indices.len() as u32),
            },
            timing,
        ))
    }

    fn release(&mut self, mesh: GpuTexturedSectionMesh) {
        self.vertex_pages[mesh.vertex_page]
            .ranges
            .release(mesh.vertex_range);
        self.index_ranges.release(mesh.index_range);
    }

    fn ensure_indirect_capacity(
        &mut self,
        device: &wgpu::Device,
        section_count: usize,
    ) -> Result<()> {
        let required = u32::try_from(section_count.saturating_mul(3).max(1)).unwrap_or(u32::MAX);
        if required <= self.indirect_draw_capacity_per_slot {
            return Ok(());
        }
        let capacity = required.checked_next_power_of_two().unwrap_or(u32::MAX);
        let size = indirect_buffer_size(capacity)?;
        if size > device.limits().max_buffer_size {
            bail!(
                "terrain indirect buffer request {size} exceeds adapter limit {}",
                device.limits().max_buffer_size
            );
        }
        self.indirect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_textured_section_indirect_draws"),
            size,
            usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.indirect_draw_capacity_per_slot = capacity;
        Ok(())
    }

    fn write_indirect_args(
        &self,
        queue: &wgpu::Queue,
        slot: u32,
        args: &[wgpu::util::DrawIndexedIndirectArgs],
    ) -> wgpu::BufferAddress {
        debug_assert!(slot < TERRAIN_INDIRECT_SLOT_COUNT);
        debug_assert!(args.len() <= self.indirect_draw_capacity_per_slot as usize);
        let offset = u64::from(slot)
            * u64::from(self.indirect_draw_capacity_per_slot)
            * DRAW_INDEXED_INDIRECT_ARG_BYTES;
        if !args.is_empty() {
            let mut bytes =
                Vec::with_capacity(args.len() * DRAW_INDEXED_INDIRECT_ARG_BYTES as usize);
            for arg in args {
                bytes.extend_from_slice(arg.as_bytes());
            }
            queue.write_buffer(&self.indirect_buffer, offset, &bytes);
        }
        offset
    }

    fn vertex_used_bytes(&self) -> u64 {
        self.vertex_pages
            .iter()
            .map(|page| u64::from(page.ranges.used()) * TEXTURED_VERTEX_BYTE_SIZE)
            .sum()
    }

    fn vertex_capacity_bytes(&self) -> u64 {
        self.vertex_pages
            .iter()
            .map(|page| u64::from(page.ranges.capacity) * TEXTURED_VERTEX_BYTE_SIZE)
            .sum()
    }

    fn index_used_bytes(&self) -> u64 {
        u64::from(self.index_ranges.used()) * std::mem::size_of::<u32>() as u64
    }

    fn index_capacity_bytes(&self) -> u64 {
        u64::from(self.index_ranges.capacity) * std::mem::size_of::<u32>() as u64
    }

    fn allocate_vertices(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        count: u32,
    ) -> Result<(usize, Range<u32>)> {
        for (page_index, page) in self.vertex_pages.iter_mut().enumerate() {
            if let Some(range) = page.ranges.allocate(count) {
                return Ok((page_index, range));
            }
        }
        for (page_index, page) in self.vertex_pages.iter_mut().enumerate() {
            if page.ranges.capacity >= self.max_vertex_capacity {
                continue;
            }
            let capacity =
                grown_arena_capacity(page.ranges.capacity, count, self.max_vertex_capacity)?;
            page.buffer = grow_arena_buffer(
                device,
                queue,
                &page.buffer,
                "mclone_textured_section_vertex_arena",
                u64::from(page.ranges.capacity) * TEXTURED_VERTEX_BYTE_SIZE,
                u64::from(capacity) * TEXTURED_VERTEX_BYTE_SIZE,
                wgpu::BufferUsages::VERTEX,
            );
            page.ranges.extend(capacity);
            let range = page
                .ranges
                .allocate(count)
                .context("grown terrain vertex arena must fit requested range")?;
            return Ok((page_index, range));
        }
        let capacity = initial_arena_capacity(
            count,
            TERRAIN_ARENA_MIN_VERTEX_CAPACITY,
            self.max_vertex_capacity,
        )?;
        let mut ranges = ElementRangeAllocator::new(capacity);
        let range = ranges
            .allocate(count)
            .context("new terrain vertex arena page must fit requested range")?;
        self.vertex_pages.push(GpuTexturedSectionVertexArena {
            buffer: create_textured_section_vertex_arena(device, capacity),
            ranges,
        });
        Ok((self.vertex_pages.len() - 1, range))
    }

    fn allocate_indices(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        count: u32,
    ) -> Result<Range<u32>> {
        if let Some(range) = self.index_ranges.allocate(count) {
            return Ok(range);
        }
        let capacity =
            grown_arena_capacity(self.index_ranges.capacity, count, self.max_index_capacity)?;
        let index_size = std::mem::size_of::<u32>() as u64;
        self.index_buffer = grow_arena_buffer(
            device,
            queue,
            &self.index_buffer,
            "mclone_textured_section_index_arena",
            u64::from(self.index_ranges.capacity) * index_size,
            u64::from(capacity) * index_size,
            wgpu::BufferUsages::INDEX,
        );
        self.index_ranges.extend(capacity);
        self.index_ranges
            .allocate(count)
            .context("grown terrain index arena must fit requested range")
    }
}

fn initial_arena_capacity(initial: u32, minimum: u32, maximum: u32) -> Result<u32> {
    let requested = initial.max(minimum);
    let capacity = requested.checked_next_power_of_two().unwrap_or(maximum);
    if capacity > maximum || requested > maximum {
        bail!("terrain GPU arena request {requested} exceeds adapter capacity {maximum}");
    }
    Ok(capacity)
}

fn grown_arena_capacity(current: u32, requested: u32, maximum: u32) -> Result<u32> {
    let minimum = current
        .checked_add(requested)
        .context("terrain GPU arena element count overflow")?;
    if minimum > maximum {
        bail!("terrain GPU arena exhausted at {current} elements (requested {requested})");
    }
    let doubled = current.saturating_mul(2).max(minimum);
    Ok(doubled
        .checked_next_power_of_two()
        .unwrap_or(maximum)
        .min(maximum))
}

fn indirect_buffer_size(draw_capacity_per_slot: u32) -> Result<wgpu::BufferAddress> {
    u64::from(draw_capacity_per_slot)
        .checked_mul(u64::from(TERRAIN_INDIRECT_SLOT_COUNT))
        .and_then(|size| size.checked_mul(DRAW_INDEXED_INDIRECT_ARG_BYTES))
        .context("terrain indirect buffer size overflow")
}

fn create_textured_section_vertex_arena(device: &wgpu::Device, capacity: u32) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mclone_textured_section_vertex_arena"),
        size: u64::from(capacity) * TEXTURED_VERTEX_BYTE_SIZE,
        usage: wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn grow_arena_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    old: &wgpu::Buffer,
    label: &'static str,
    old_size: wgpu::BufferAddress,
    new_size: wgpu::BufferAddress,
    primary_usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    let new = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: new_size,
        usage: primary_usage | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_textured_section_arena_grow"),
    });
    encoder.copy_buffer_to_buffer(old, 0, &new, 0, old_size);
    queue.submit(Some(encoder.finish()));
    new
}

fn bind_textured_section_arena<'pass>(
    pass: &mut wgpu::RenderPass<'pass>,
    arena: &'pass GpuTexturedSectionArena,
) {
    pass.set_index_buffer(arena.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
}

fn bind_textured_section_vertex_page<'pass>(
    pass: &mut wgpu::RenderPass<'pass>,
    arena: &'pass GpuTexturedSectionArena,
    page: usize,
) {
    pass.set_vertex_buffer(0, arena.vertex_pages[page].buffer.slice(..));
}

fn draw_textured_section_mesh_range<'pass>(
    pass: &mut wgpu::RenderPass<'pass>,
    arena: &'pass GpuTexturedSectionArena,
    mesh: &GpuTexturedSectionMesh,
    range: Range<u32>,
) {
    if range.is_empty() {
        return;
    }
    bind_textured_section_vertex_page(pass, arena, mesh.vertex_page);
    pass.draw_indexed(
        mesh.index_range.start + range.start..mesh.index_range.start + range.end,
        mesh.vertex_range.start as i32,
        0..1,
    );
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TerrainIndirectDrawSpan {
    first: u32,
    count: u32,
}

impl TerrainIndirectDrawSpan {
    fn byte_offset(self, base: wgpu::BufferAddress) -> wgpu::BufferAddress {
        base + u64::from(self.first) * DRAW_INDEXED_INDIRECT_ARG_BYTES
    }
}

#[derive(Clone, Debug, Default)]
struct TerrainIndirectDrawBatch {
    args: Vec<wgpu::util::DrawIndexedIndirectArgs>,
    pages: Vec<TerrainIndirectDrawPage>,
}

#[derive(Clone, Copy, Debug, Default)]
struct TerrainIndirectDrawPage {
    vertex_page: usize,
    solid: TerrainIndirectDrawSpan,
    cutout: TerrainIndirectDrawSpan,
}

impl TerrainIndirectDrawBatch {
    fn begin_span(&self) -> u32 {
        self.args.len() as u32
    }

    fn finish_span(&self, first: u32) -> TerrainIndirectDrawSpan {
        TerrainIndirectDrawSpan {
            first,
            count: self.args.len() as u32 - first,
        }
    }

    fn push(&mut self, mesh: &GpuTexturedSectionMesh, range: Range<u32>) {
        if !range.is_empty() {
            self.args.push(mesh.indirect_args(range));
        }
    }
}

fn terrain_indirect_slot(view_slot: PerViewSlot, phase: TexturedSectionRenderPhase) -> u32 {
    let phase_slot = match phase {
        TexturedSectionRenderPhase::All => 0,
        TexturedSectionRenderPhase::Opaque => 1,
        TexturedSectionRenderPhase::Translucent => 2,
    };
    view_slot.index() * TERRAIN_INDIRECT_PHASE_SLOT_COUNT + phase_slot
}

#[derive(Clone, Copy, Debug)]
pub struct ChunkTextureAtlas<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChunkTextureSampling {
    #[default]
    Vanilla,
    TerrainOverview,
}

fn chunk_texture_filter_modes(
    sampling: ChunkTextureSampling,
) -> (wgpu::FilterMode, wgpu::FilterMode) {
    match sampling {
        ChunkTextureSampling::Vanilla => (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest),
        ChunkTextureSampling::TerrainOverview => {
            (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear)
        }
    }
}

struct GpuChunkTextureAtlas {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
}

impl GpuChunkTextureAtlas {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        atlas: ChunkTextureAtlas<'_>,
    ) -> Result<Self> {
        Self::new_with_sampling(device, queue, layout, atlas, ChunkTextureSampling::Vanilla)
    }

    fn new_with_sampling(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        atlas: ChunkTextureAtlas<'_>,
        sampling: ChunkTextureSampling,
    ) -> Result<Self> {
        let width = atlas.width.max(1);
        let height = atlas.height.max(1);
        let expected_len = width as usize * height as usize * 4;
        if atlas.rgba.len() != expected_len {
            bail!(
                "texture atlas has {} bytes; expected {expected_len} for {}x{} RGBA",
                atlas.rgba.len(),
                width,
                height
            );
        }

        let mip_levels =
            generate_rgba_mip_chain(width, height, atlas.rgba, CHUNK_ATLAS_MAX_MIP_LEVEL + 1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_chunk_texture_atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (mip_level, mip) in mip_levels.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: mip_level as u32,
                    origin: Default::default(),
                    aspect: Default::default(),
                },
                &mip.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(mip.width * 4),
                    rows_per_image: Some(mip.height),
                },
                wgpu::Extent3d {
                    width: mip.width,
                    height: mip.height,
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = texture.create_view(&Default::default());
        let lod_max_clamp = mip_levels.len().saturating_sub(1) as f32;
        let (min_filter, mipmap_filter) = chunk_texture_filter_modes(sampling);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mclone_chunk_texture_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter,
            // Java's AbstractTexture.setFilter(false, true) uses
            // GL_NEAREST_MIPMAP_NEAREST, preserving pixelated blocks while
            // still selecting a lower-detail mip for distant terrain. The
            // Terrain Lab opts into trilinear minification for macro views.
            mipmap_filter,
            lod_max_clamp,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_chunk_texture_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Ok(Self {
            _texture: texture,
            _view: view,
            _sampler: sampler,
            bind_group,
        })
    }
}

pub struct ChunkDepthTarget {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

impl ChunkDepthTarget {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_chunk_depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            texture,
            view,
            width,
            height,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width.max(1) && self.height == height.max(1) {
            return;
        }
        *self = Self::new(device, width, height);
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
}

pub struct ChunkMultiviewDepthTarget {
    _texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

impl ChunkMultiviewDepthTarget {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_chunk_multiview_depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 2,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("mclone_chunk_multiview_depth_view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            base_array_layer: 0,
            array_layer_count: Some(2),
            ..Default::default()
        });
        Self {
            _texture: texture,
            view,
            width,
            height,
        }
    }
}

pub struct ChunkRenderer {
    pipeline: wgpu::RenderPipeline,
    uniforms: PerViewUniformBuffer,
    bind_group: wgpu::BindGroup,
}

impl ChunkRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_chunk_flat_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/chunk_flat.wgsl").into()),
        });
        let uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_chunk_uniforms",
            UNIFORM_BYTE_SIZE,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_chunk_bind_group_layout"),
            entries: &[uniforms.layout_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_chunk_bind_group"),
            layout: &bind_group_layout,
            entries: &[uniforms.bind_group_entry(0)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_chunk_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_chunk_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: VERTEX_BYTE_SIZE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        Self {
            pipeline,
            uniforms,
            bind_group,
        }
    }
}

pub struct TexturedChunkRenderer {
    solid_pipeline: wgpu::RenderPipeline,
    cutout_pipeline: wgpu::RenderPipeline,
    translucent_pipeline: wgpu::RenderPipeline,
    uniforms: PerViewUniformBuffer,
    bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    color_format: wgpu::TextureFormat,
    multiview: RefCell<Option<TexturedChunkMultiviewRenderer>>,
}

impl TexturedChunkRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_chunk_textured_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/chunk_textured.wgsl").into()),
        });
        let uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_textured_chunk_uniforms",
            UNIFORM_BYTE_SIZE,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_textured_chunk_uniform_bind_group_layout"),
                entries: &[uniforms.layout_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_textured_chunk_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[uniforms.bind_group_entry(0)],
        });
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_textured_chunk_texture_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_textured_chunk_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let solid_pipeline = create_textured_chunk_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_textured_chunk_solid_pipeline",
            "fs_main_solid",
            None,
            true,
            None,
        );
        let cutout_pipeline = create_textured_chunk_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_textured_chunk_cutout_pipeline",
            "fs_main_cutout",
            None,
            true,
            None,
        );
        let translucent_pipeline = create_textured_chunk_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_textured_chunk_translucent_pipeline",
            "fs_main_cutout",
            Some(translucent_blend_state()),
            false,
            None,
        );
        Self {
            solid_pipeline,
            cutout_pipeline,
            translucent_pipeline,
            uniforms,
            bind_group,
            texture_bind_group_layout,
            color_format,
            multiview: RefCell::new(None),
        }
    }

    fn multiview_renderer(
        &self,
        device: &wgpu::Device,
    ) -> Result<std::cell::Ref<'_, TexturedChunkMultiviewRenderer>> {
        if !device.features().contains(wgpu::Features::MULTIVIEW) {
            bail!("textured chunk multiview render requires wgpu MULTIVIEW");
        }
        if self.multiview.borrow().is_none() {
            let renderer = TexturedChunkMultiviewRenderer::new(
                device,
                self.color_format,
                &self.texture_bind_group_layout,
            );
            *self.multiview.borrow_mut() = Some(renderer);
        }
        Ok(std::cell::Ref::map(self.multiview.borrow(), |renderer| {
            renderer
                .as_ref()
                .expect("multiview renderer initialized above")
        }))
    }
}

struct TexturedChunkMultiviewRenderer {
    solid_pipeline: wgpu::RenderPipeline,
    cutout_pipeline: wgpu::RenderPipeline,
    translucent_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl TexturedChunkMultiviewRenderer {
    fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_chunk_textured_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/chunk_textured_multiview.wgsl").into(),
            ),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_textured_chunk_multiview_uniforms"),
            size: MULTIVIEW_UNIFORM_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_textured_chunk_multiview_uniform_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(MULTIVIEW_UNIFORM_BYTE_SIZE),
                    },
                    count: None,
                }],
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_textured_chunk_multiview_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_textured_chunk_multiview_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let solid_pipeline = create_textured_chunk_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_textured_chunk_multiview_solid_pipeline",
            "fs_main_solid",
            None,
            true,
            NonZeroU32::new(2),
        );
        let cutout_pipeline = create_textured_chunk_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_textured_chunk_multiview_cutout_pipeline",
            "fs_main_cutout",
            None,
            true,
            NonZeroU32::new(2),
        );
        let translucent_pipeline = create_textured_chunk_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_textured_chunk_multiview_translucent_pipeline",
            "fs_main_cutout",
            Some(translucent_blend_state()),
            false,
            NonZeroU32::new(2),
        );
        Self {
            solid_pipeline,
            cutout_pipeline,
            translucent_pipeline,
            uniform_buffer,
            bind_group,
        }
    }

    fn write_uniforms(
        &self,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
        color_format: wgpu::TextureFormat,
    ) {
        let bytes = multiview_uniform_bytes(render_views, options, color_format);
        queue.write_buffer(&self.uniform_buffer, 0, &bytes);
    }
}

/// Opt-in terrain renderer for a uniformly scaled/translated world. This is
/// intentionally separate from `TexturedChunkRenderer`: constructing an
/// ordinary world does not allocate placed uniforms or compile placed
/// pipelines.
pub struct PlacedTexturedSectionRenderer {
    solid_pipeline: wgpu::RenderPipeline,
    cutout_pipeline: wgpu::RenderPipeline,
    translucent_pipeline: wgpu::RenderPipeline,
    uniforms: PerViewUniformBuffer,
    bind_group: wgpu::BindGroup,
    color_format: wgpu::TextureFormat,
    multiview: RefCell<Option<PlacedTexturedSectionMultiviewRenderer>>,
    clipped: RefCell<Option<ClippedPlacedTexturedSectionRenderer>>,
    clipped_multiview: RefCell<Option<ClippedPlacedTexturedSectionMultiviewRenderer>>,
}

impl PlacedTexturedSectionRenderer {
    fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_placed_textured_chunk_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/chunk_textured_placed.wgsl").into(),
            ),
        });
        let uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_placed_textured_chunk_uniforms",
            PLACED_UNIFORM_BYTE_SIZE,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_placed_textured_chunk_uniform_bind_group_layout"),
                entries: &[uniforms.layout_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_placed_textured_chunk_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[uniforms.bind_group_entry(0)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_placed_textured_chunk_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let solid_pipeline = create_textured_chunk_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_placed_textured_chunk_solid_pipeline",
            "fs_main_solid",
            None,
            true,
            None,
        );
        let cutout_pipeline = create_textured_chunk_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_placed_textured_chunk_cutout_pipeline",
            "fs_main_cutout",
            None,
            true,
            None,
        );
        let translucent_pipeline = create_textured_chunk_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_placed_textured_chunk_translucent_pipeline",
            "fs_main_cutout",
            Some(translucent_blend_state()),
            false,
            None,
        );
        Self {
            solid_pipeline,
            cutout_pipeline,
            translucent_pipeline,
            uniforms,
            bind_group,
            color_format,
            multiview: RefCell::new(None),
            clipped: RefCell::new(None),
            clipped_multiview: RefCell::new(None),
        }
    }

    fn clipped_renderer(
        &self,
        device: &wgpu::Device,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> std::cell::Ref<'_, ClippedPlacedTexturedSectionRenderer> {
        if self.clipped.borrow().is_none() {
            *self.clipped.borrow_mut() = Some(ClippedPlacedTexturedSectionRenderer::new(
                device,
                self.color_format,
                texture_bind_group_layout,
            ));
        }
        std::cell::Ref::map(self.clipped.borrow(), |renderer| {
            renderer
                .as_ref()
                .expect("clipped placed renderer initialized above")
        })
    }

    fn selected_mono_renderer(
        &self,
        device: &wgpu::Device,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        clip: CompositionClip,
    ) -> SelectedPlacedMonoRenderer<'_> {
        match clip {
            CompositionClip::Unbounded => SelectedPlacedMonoRenderer::Unbounded(self),
            CompositionClip::HalfSpace(_) => SelectedPlacedMonoRenderer::Clipped(
                self.clipped_renderer(device, texture_bind_group_layout),
            ),
        }
    }

    fn multiview_renderer(
        &self,
        device: &wgpu::Device,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Result<std::cell::Ref<'_, PlacedTexturedSectionMultiviewRenderer>> {
        if !device.features().contains(wgpu::Features::MULTIVIEW) {
            bail!("placed textured terrain multiview render requires wgpu MULTIVIEW");
        }
        if self.multiview.borrow().is_none() {
            *self.multiview.borrow_mut() = Some(PlacedTexturedSectionMultiviewRenderer::new(
                device,
                self.color_format,
                texture_bind_group_layout,
            ));
        }
        Ok(std::cell::Ref::map(self.multiview.borrow(), |renderer| {
            renderer
                .as_ref()
                .expect("placed multiview renderer initialized above")
        }))
    }

    pub fn multiview_renderer_materialized(&self) -> bool {
        self.multiview.borrow().is_some()
    }

    fn clipped_multiview_renderer(
        &self,
        device: &wgpu::Device,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Result<std::cell::Ref<'_, ClippedPlacedTexturedSectionMultiviewRenderer>> {
        if !device.features().contains(wgpu::Features::MULTIVIEW) {
            bail!("clipped placed terrain multiview render requires wgpu MULTIVIEW");
        }
        if self.clipped_multiview.borrow().is_none() {
            *self.clipped_multiview.borrow_mut() =
                Some(ClippedPlacedTexturedSectionMultiviewRenderer::new(
                    device,
                    self.color_format,
                    texture_bind_group_layout,
                ));
        }
        Ok(std::cell::Ref::map(
            self.clipped_multiview.borrow(),
            |renderer| {
                renderer
                    .as_ref()
                    .expect("clipped placed multiview renderer initialized above")
            },
        ))
    }

    fn selected_multiview_renderer(
        &self,
        device: &wgpu::Device,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        clip: CompositionClip,
    ) -> Result<SelectedPlacedMultiviewRenderer<'_>> {
        match clip {
            CompositionClip::Unbounded => Ok(SelectedPlacedMultiviewRenderer::Unbounded(
                self.multiview_renderer(device, texture_bind_group_layout)?,
            )),
            CompositionClip::HalfSpace(_) => Ok(SelectedPlacedMultiviewRenderer::Clipped(
                self.clipped_multiview_renderer(device, texture_bind_group_layout)?,
            )),
        }
    }

    pub fn clipped_renderer_materialized(&self) -> bool {
        self.clipped.borrow().is_some()
    }

    pub fn clipped_multiview_renderer_materialized(&self) -> bool {
        self.clipped_multiview.borrow().is_some()
    }
}

struct ClippedPlacedTexturedSectionRenderer {
    solid_pipeline: wgpu::RenderPipeline,
    cutout_pipeline: wgpu::RenderPipeline,
    translucent_pipeline: wgpu::RenderPipeline,
    uniforms: PerViewUniformBuffer,
    bind_group: wgpu::BindGroup,
}

impl ClippedPlacedTexturedSectionRenderer {
    fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_clipped_placed_textured_chunk_shader"),
            source: wgpu::ShaderSource::Wgsl(clipped_placed_shader_source().into()),
        });
        let uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_clipped_placed_textured_chunk_uniforms",
            CLIPPED_PLACED_UNIFORM_BYTE_SIZE,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_clipped_placed_textured_chunk_uniform_layout"),
                entries: &[uniforms.layout_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_clipped_placed_textured_chunk_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[uniforms.bind_group_entry(0)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_clipped_placed_textured_chunk_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        Self {
            solid_pipeline: create_textured_chunk_pipeline(
                device,
                &pipeline_layout,
                &shader,
                color_format,
                "mclone_clipped_placed_textured_chunk_solid_pipeline",
                "fs_main_solid",
                None,
                true,
                None,
            ),
            cutout_pipeline: create_textured_chunk_pipeline(
                device,
                &pipeline_layout,
                &shader,
                color_format,
                "mclone_clipped_placed_textured_chunk_cutout_pipeline",
                "fs_main_cutout",
                None,
                true,
                None,
            ),
            translucent_pipeline: create_textured_chunk_pipeline(
                device,
                &pipeline_layout,
                &shader,
                color_format,
                "mclone_clipped_placed_textured_chunk_translucent_pipeline",
                "fs_main_cutout",
                Some(translucent_blend_state()),
                false,
                None,
            ),
            uniforms,
            bind_group,
        }
    }
}

enum SelectedPlacedMonoRenderer<'a> {
    Unbounded(&'a PlacedTexturedSectionRenderer),
    Clipped(std::cell::Ref<'a, ClippedPlacedTexturedSectionRenderer>),
}

impl SelectedPlacedMonoRenderer<'_> {
    fn write_uniforms(
        &self,
        queue: &wgpu::Queue,
        view_slot: PerViewSlot,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        context: WorldCompositionContext,
        color_format: wgpu::TextureFormat,
    ) -> u32 {
        match self {
            Self::Unbounded(renderer) => renderer.uniforms.write_slot(
                queue,
                view_slot,
                &placed_uniform_bytes(render_view, options, context.placement(), color_format),
            ),
            Self::Clipped(renderer) => renderer.uniforms.write_slot(
                queue,
                view_slot,
                &clipped_placed_uniform_bytes(render_view, options, context, color_format),
            ),
        }
    }

    fn bind_group(&self) -> &wgpu::BindGroup {
        match self {
            Self::Unbounded(renderer) => &renderer.bind_group,
            Self::Clipped(renderer) => &renderer.bind_group,
        }
    }

    fn solid_pipeline(&self) -> &wgpu::RenderPipeline {
        match self {
            Self::Unbounded(renderer) => &renderer.solid_pipeline,
            Self::Clipped(renderer) => &renderer.solid_pipeline,
        }
    }

    fn cutout_pipeline(&self) -> &wgpu::RenderPipeline {
        match self {
            Self::Unbounded(renderer) => &renderer.cutout_pipeline,
            Self::Clipped(renderer) => &renderer.cutout_pipeline,
        }
    }

    fn translucent_pipeline(&self) -> &wgpu::RenderPipeline {
        match self {
            Self::Unbounded(renderer) => &renderer.translucent_pipeline,
            Self::Clipped(renderer) => &renderer.translucent_pipeline,
        }
    }
}

struct PlacedTexturedSectionMultiviewRenderer {
    solid_pipeline: wgpu::RenderPipeline,
    cutout_pipeline: wgpu::RenderPipeline,
    translucent_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl PlacedTexturedSectionMultiviewRenderer {
    fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_placed_textured_chunk_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/chunk_textured_placed_multiview.wgsl").into(),
            ),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_placed_textured_chunk_multiview_uniforms"),
            size: PLACED_MULTIVIEW_UNIFORM_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_placed_textured_chunk_multiview_uniform_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(PLACED_MULTIVIEW_UNIFORM_BYTE_SIZE),
                    },
                    count: None,
                }],
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_placed_textured_chunk_multiview_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_placed_textured_chunk_multiview_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let solid_pipeline = create_textured_chunk_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_placed_textured_chunk_multiview_solid_pipeline",
            "fs_main_solid",
            None,
            true,
            NonZeroU32::new(2),
        );
        let cutout_pipeline = create_textured_chunk_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_placed_textured_chunk_multiview_cutout_pipeline",
            "fs_main_cutout",
            None,
            true,
            NonZeroU32::new(2),
        );
        let translucent_pipeline = create_textured_chunk_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "mclone_placed_textured_chunk_multiview_translucent_pipeline",
            "fs_main_cutout",
            Some(translucent_blend_state()),
            false,
            NonZeroU32::new(2),
        );
        Self {
            solid_pipeline,
            cutout_pipeline,
            translucent_pipeline,
            uniform_buffer,
            bind_group,
        }
    }

    fn write_uniforms(
        &self,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
        placement: WorldPlacement,
        color_format: wgpu::TextureFormat,
    ) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            &placed_multiview_uniform_bytes(render_views, options, placement, color_format),
        );
    }
}

struct ClippedPlacedTexturedSectionMultiviewRenderer {
    solid_pipeline: wgpu::RenderPipeline,
    cutout_pipeline: wgpu::RenderPipeline,
    translucent_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl ClippedPlacedTexturedSectionMultiviewRenderer {
    fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_clipped_placed_textured_chunk_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(clipped_placed_multiview_shader_source().into()),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_clipped_placed_textured_chunk_multiview_uniforms"),
            size: CLIPPED_PLACED_MULTIVIEW_UNIFORM_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_clipped_placed_textured_chunk_multiview_uniform_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(
                            CLIPPED_PLACED_MULTIVIEW_UNIFORM_BYTE_SIZE,
                        ),
                    },
                    count: None,
                }],
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_clipped_placed_textured_chunk_multiview_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_clipped_placed_textured_chunk_multiview_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        Self {
            solid_pipeline: create_textured_chunk_pipeline(
                device,
                &pipeline_layout,
                &shader,
                color_format,
                "mclone_clipped_placed_textured_chunk_multiview_solid_pipeline",
                "fs_main_solid",
                None,
                true,
                NonZeroU32::new(2),
            ),
            cutout_pipeline: create_textured_chunk_pipeline(
                device,
                &pipeline_layout,
                &shader,
                color_format,
                "mclone_clipped_placed_textured_chunk_multiview_cutout_pipeline",
                "fs_main_cutout",
                None,
                true,
                NonZeroU32::new(2),
            ),
            translucent_pipeline: create_textured_chunk_pipeline(
                device,
                &pipeline_layout,
                &shader,
                color_format,
                "mclone_clipped_placed_textured_chunk_multiview_translucent_pipeline",
                "fs_main_cutout",
                Some(translucent_blend_state()),
                false,
                NonZeroU32::new(2),
            ),
            uniform_buffer,
            bind_group,
        }
    }

    fn write_uniforms(
        &self,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
        context: WorldCompositionContext,
        color_format: wgpu::TextureFormat,
    ) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            &clipped_placed_multiview_uniform_bytes(render_views, options, context, color_format),
        );
    }
}

enum SelectedPlacedMultiviewRenderer<'a> {
    Unbounded(std::cell::Ref<'a, PlacedTexturedSectionMultiviewRenderer>),
    Clipped(std::cell::Ref<'a, ClippedPlacedTexturedSectionMultiviewRenderer>),
}

impl SelectedPlacedMultiviewRenderer<'_> {
    fn write_uniforms(
        &self,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
        context: WorldCompositionContext,
        color_format: wgpu::TextureFormat,
    ) {
        match self {
            Self::Unbounded(renderer) => renderer.write_uniforms(
                queue,
                render_views,
                options,
                context.placement(),
                color_format,
            ),
            Self::Clipped(renderer) => {
                renderer.write_uniforms(queue, render_views, options, context, color_format)
            }
        }
    }

    fn bind_group(&self) -> &wgpu::BindGroup {
        match self {
            Self::Unbounded(renderer) => &renderer.bind_group,
            Self::Clipped(renderer) => &renderer.bind_group,
        }
    }

    fn solid_pipeline(&self) -> &wgpu::RenderPipeline {
        match self {
            Self::Unbounded(renderer) => &renderer.solid_pipeline,
            Self::Clipped(renderer) => &renderer.solid_pipeline,
        }
    }

    fn cutout_pipeline(&self) -> &wgpu::RenderPipeline {
        match self {
            Self::Unbounded(renderer) => &renderer.cutout_pipeline,
            Self::Clipped(renderer) => &renderer.cutout_pipeline,
        }
    }

    fn translucent_pipeline(&self) -> &wgpu::RenderPipeline {
        match self {
            Self::Unbounded(renderer) => &renderer.translucent_pipeline,
            Self::Clipped(renderer) => &renderer.translucent_pipeline,
        }
    }
}

fn clipped_placed_shader_source() -> String {
    let source = include_str!("shaders/chunk_textured_placed.wgsl");
    let source = source.replacen(
        "    composition_anchor: vec4<f32>,\n};",
        "    composition_anchor: vec4<f32>,\n    clip_plane: vec4<f32>,\n};",
        1,
    );
    let clip = concat!(
        "    if (dot(uniforms.clip_plane.xyz, input.composition_position) ",
        "+ uniforms.clip_plane.w < 0.0) {\n",
        "        discard;\n",
        "    }\n",
    );
    let source = source.replacen(
        "fn fs_main_solid(input: VertexOutput) -> @location(0) vec4<f32> {\n",
        &format!("fn fs_main_solid(input: VertexOutput) -> @location(0) vec4<f32> {{\n{clip}"),
        1,
    );
    let source = source.replacen(
        "fn fs_main_cutout(input: VertexOutput) -> @location(0) vec4<f32> {\n",
        &format!("fn fs_main_cutout(input: VertexOutput) -> @location(0) vec4<f32> {{\n{clip}"),
        1,
    );
    assert_eq!(source.matches("clip_plane: vec4<f32>").count(), 1);
    assert_eq!(source.matches("uniforms.clip_plane").count(), 4);
    source
}

fn clipped_placed_multiview_shader_source() -> String {
    let source = include_str!("shaders/chunk_textured_placed_multiview.wgsl");
    let source = source.replacen(
        "    composition_anchor: vec4<f32>,\n};",
        "    composition_anchor: vec4<f32>,\n    clip_plane: vec4<f32>,\n};",
        1,
    );
    let uniforms_and_texel = concat!(
        "    let uniforms = stereo_uniforms.views[u32(input.view_index)];\n",
        "    let texel",
    );
    let clipped_uniforms_and_texel = concat!(
        "    let uniforms = stereo_uniforms.views[u32(input.view_index)];\n",
        "    if (dot(uniforms.clip_plane.xyz, input.composition_position) ",
        "+ uniforms.clip_plane.w < 0.0) {\n",
        "        discard;\n",
        "    }\n",
        "    let texel",
    );
    let source = source.replace(uniforms_and_texel, clipped_uniforms_and_texel);
    assert_eq!(source.matches("clip_plane: vec4<f32>").count(), 1);
    assert_eq!(source.matches("uniforms.clip_plane").count(), 4);
    source
}

fn create_textured_chunk_pipeline(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    label: &'static str,
    fragment_entry_point: &'static str,
    blend: Option<wgpu::BlendState>,
    depth_write_enabled: bool,
    multiview: Option<NonZeroU32>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: TEXTURED_VERTEX_BYTE_SIZE,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x3,
                    },
                    wgpu::VertexAttribute {
                        offset: 12,
                        shader_location: 1,
                        format: wgpu::VertexFormat::Float32x2,
                    },
                    wgpu::VertexAttribute {
                        offset: 20,
                        shader_location: 2,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                    wgpu::VertexAttribute {
                        offset: 36,
                        shader_location: 3,
                        format: wgpu::VertexFormat::Uint32,
                    },
                ],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment_entry_point),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled,
            depth_compare: wgpu::CompareFunction::GreaterEqual,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview,
        cache: None,
    })
}

fn translucent_blend_state() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

pub struct ChunkDrawResources {
    renderer: ChunkRenderer,
    mesh: GpuChunkMesh,
}

impl ChunkDrawResources {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        mesh: &VisibleChunkMesh,
    ) -> Result<Self> {
        Ok(Self {
            renderer: ChunkRenderer::new(device, color_format),
            mesh: GpuChunkMesh::new(device, mesh).context("failed to upload chunk mesh")?,
        })
    }

    pub fn index_count(&self) -> u32 {
        self.mesh.index_count()
    }

    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
    ) -> Result<()> {
        self.render_in_slot(queue, encoder, target, render_view, SINGLE_VIEW_SLOT)
    }

    pub fn render_in_slot(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        view_slot: PerViewSlot,
    ) -> Result<()> {
        let uniform_offset = self.renderer.uniforms.write_slot(
            queue,
            view_slot,
            &uniform_bytes(
                render_view,
                TexturedSectionRenderOptions::default(),
                wgpu::TextureFormat::Rgba8Unorm,
            ),
        );

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_chunk_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: target.color_load_op(),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: target.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: target.depth_load_op(),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.renderer.pipeline);
        pass.set_bind_group(0, &self.renderer.bind_group, &[uniform_offset]);
        pass.set_vertex_buffer(0, self.mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(self.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.mesh.index_count, 0, 0..1);
        Ok(())
    }
}

pub struct TexturedChunkDrawResources {
    renderer: TexturedChunkRenderer,
    mesh: GpuTexturedChunkMesh,
    atlas: GpuChunkTextureAtlas,
}

impl TexturedChunkDrawResources {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        mesh: &TexturedVisibleChunkMesh,
        atlas: ChunkTextureAtlas<'_>,
    ) -> Result<Self> {
        let renderer = TexturedChunkRenderer::new(device, color_format);
        let atlas =
            GpuChunkTextureAtlas::new(device, queue, &renderer.texture_bind_group_layout, atlas)
                .context("failed to upload chunk texture atlas")?;
        Ok(Self {
            renderer,
            mesh: GpuTexturedChunkMesh::new(device, queue, mesh)
                .context("failed to upload textured chunk mesh")?,
            atlas,
        })
    }

    pub fn index_count(&self) -> u32 {
        self.mesh.index_count()
    }

    pub fn update_mesh(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &TexturedVisibleChunkMesh,
    ) -> Result<()> {
        self.mesh = GpuTexturedChunkMesh::new(device, queue, mesh)
            .context("failed to upload textured chunk mesh")?;
        Ok(())
    }

    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
    ) -> Result<()> {
        self.render_in_slot(queue, encoder, target, render_view, SINGLE_VIEW_SLOT)
    }

    pub fn render_in_slot(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        view_slot: PerViewSlot,
    ) -> Result<()> {
        let uniform_offset = self.renderer.uniforms.write_slot(
            queue,
            view_slot,
            &uniform_bytes(
                render_view,
                TexturedSectionRenderOptions::default(),
                self.renderer.color_format,
            ),
        );

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_textured_chunk_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: target.color_load_op(),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: target.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: target.depth_load_op(),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, &self.renderer.bind_group, &[uniform_offset]);
        pass.set_bind_group(1, &self.atlas.bind_group, &[]);
        pass.set_pipeline(&self.renderer.solid_pipeline);
        draw_textured_mesh_range(&mut pass, &self.mesh, self.mesh.solid_index_range());
        pass.set_pipeline(&self.renderer.cutout_pipeline);
        draw_textured_mesh_range(&mut pass, &self.mesh, self.mesh.cutout_index_range());
        pass.set_pipeline(&self.renderer.translucent_pipeline);
        draw_textured_mesh_range(&mut pass, &self.mesh, self.mesh.translucent_index_range());
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct ChunkMultiviewRenderTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub depth_view: &'a wgpu::TextureView,
    pub size: [u32; 2],
    pub clear_color: wgpu::Color,
    pub clear_depth: f32,
    pub gpu_timestamps: Option<&'a GpuTimestampFrameEncoder>,
    pub load_color: bool,
    pub load_depth: bool,
}

impl<'a> ChunkMultiviewRenderTarget<'a> {
    pub fn new(
        color_view: &'a wgpu::TextureView,
        depth_view: &'a wgpu::TextureView,
        size: [u32; 2],
        clear_color: wgpu::Color,
    ) -> Self {
        Self {
            color_view,
            depth_view,
            size,
            clear_color,
            clear_depth: REVERSED_Z_DEPTH_CLEAR,
            gpu_timestamps: None,
            load_color: false,
            load_depth: false,
        }
    }

    pub fn with_loaded_color(mut self) -> Self {
        self.load_color = true;
        self
    }

    pub fn with_loaded_depth(mut self) -> Self {
        self.load_depth = true;
        self
    }

    pub fn with_gpu_timestamps(mut self, gpu_timestamps: &'a GpuTimestampFrameEncoder) -> Self {
        self.gpu_timestamps = Some(gpu_timestamps);
        self
    }

    fn gpu_timestamp_writes(self, pass: GpuPassId) -> Option<wgpu::RenderPassTimestampWrites<'a>> {
        self.gpu_timestamps
            .and_then(|timestamps| timestamps.render_pass_timestamp_writes(pass))
    }

    fn color_load_op(self) -> wgpu::LoadOp<wgpu::Color> {
        if self.load_color {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(self.clear_color)
        }
    }

    fn depth_load_op(self) -> wgpu::LoadOp<f32> {
        if self.load_depth {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(self.clear_depth)
        }
    }
}

/// Immutable device/asset terrain resources shared by compatible world slots.
///
/// Section residency and GPU mesh buffers deliberately do not live here. One
/// host may retain multiple mutable worlds while paying for one atlas and one
/// compatible direct-terrain pipeline/uniform topology.
pub struct TexturedSectionSharedResources {
    renderer: TexturedChunkRenderer,
    atlas: GpuChunkTextureAtlas,
}

impl TexturedSectionSharedResources {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        atlas: ChunkTextureAtlas<'_>,
    ) -> Result<Arc<Self>> {
        Self::new_with_texture_sampling(
            device,
            queue,
            color_format,
            atlas,
            ChunkTextureSampling::Vanilla,
        )
    }

    pub fn new_with_texture_sampling(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        atlas: ChunkTextureAtlas<'_>,
        sampling: ChunkTextureSampling,
    ) -> Result<Arc<Self>> {
        let renderer = TexturedChunkRenderer::new(device, color_format);
        let atlas = GpuChunkTextureAtlas::new_with_sampling(
            device,
            queue,
            &renderer.texture_bind_group_layout,
            atlas,
            sampling,
        )
        .context("failed to upload chunk texture atlas")?;
        Ok(Arc::new(Self { renderer, atlas }))
    }

    pub const fn color_format(&self) -> wgpu::TextureFormat {
        self.renderer.color_format
    }
}

pub struct TexturedSectionDrawResources {
    shared: Arc<TexturedSectionSharedResources>,
    sections: BTreeMap<RenderSectionKey, GpuTexturedSectionMesh>,
    section_arena: GpuTexturedSectionArena,
    visibility_sections: BTreeMap<RenderSectionKey, VisibilitySet>,
    visibility_section_keys_by_chunk: BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>>,
    traversal_ready_sections: BTreeSet<RenderSectionKey>,
    traversal_ready_columns: Option<BTreeSet<ChunkPos>>,
    section_set_generation: u64,
    queue: wgpu::Queue,
    // Slice F (docs/tactical/106): the prepared culling records only change when
    // the section set / readiness changes (upload, removal, traversal refresh),
    // not on camera movement. Cache them across frames and rebuild lazily on the
    // next `prepare_render_records` after a mutation, instead of rebuilding the
    // whole `BTreeMap` every frame. `Arc` keeps the type `Send` and makes the
    // per-frame handout an O(1) refcount bump rather than a deep clone.
    //
    // Behind interior mutability so the cache lives on the shared `&self` render
    // path (`render_with_options_inner`), not just the XR-only
    // `prepare_render_records` entry point: every client (flat, web, headless,
    // XR) that culls through this draw-state store reuses the same cache.
    cached_records: RefCell<Option<Arc<PreparedTexturedSectionRecords>>>,
    records_dirty: Cell<bool>,
    record_dirty_causes: Cell<TexturedSectionRecordDirtyCauses>,
    record_cache_stats: Cell<TexturedSectionRecordCacheStats>,
    // Retain the mono visibility/traversal result while both the exact view
    // and immutable prepared-record generation stay unchanged. This also
    // shares one cull between the opaque and translucent phases of a frame.
    mono_culling_cache: RefCell<Option<TexturedSectionMonoCullingCache>>,
    // Slice G: reusable per-eye cull scratch (cleared each call). `RefCell`
    // because the render path borrows `&self`; the scratch is only ever touched
    // synchronously inside one cull call, never aliased.
    cull_scratch: RefCell<CullScratch>,
}

impl TexturedSectionDrawResources {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        sections: &[TexturedRenderSectionMesh],
        atlas: ChunkTextureAtlas<'_>,
    ) -> Result<Self> {
        let shared = TexturedSectionSharedResources::new(device, queue, color_format, atlas)?;
        Self::new_with_shared_resources(device, queue, sections, shared)
    }

    pub fn new_with_texture_sampling(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        sections: &[TexturedRenderSectionMesh],
        atlas: ChunkTextureAtlas<'_>,
        sampling: ChunkTextureSampling,
    ) -> Result<Self> {
        let shared = TexturedSectionSharedResources::new_with_texture_sampling(
            device,
            queue,
            color_format,
            atlas,
            sampling,
        )?;
        Self::new_with_shared_resources(device, queue, sections, shared)
    }

    pub fn new_with_shared_resources(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sections: &[TexturedRenderSectionMesh],
        shared: Arc<TexturedSectionSharedResources>,
    ) -> Result<Self> {
        let initial_vertex_count = sections
            .iter()
            .map(|section| section.mesh.vertices.len() as u64)
            .sum::<u64>()
            .min(u64::from(u32::MAX)) as u32;
        let initial_index_count = sections
            .iter()
            .map(|section| section.mesh.indices.len() as u64)
            .sum::<u64>()
            .min(u64::from(u32::MAX)) as u32;
        let mut resources = Self {
            shared,
            sections: BTreeMap::new(),
            section_arena: GpuTexturedSectionArena::new(
                device,
                initial_vertex_count,
                initial_index_count,
                sections.len(),
            )?,
            visibility_sections: BTreeMap::new(),
            visibility_section_keys_by_chunk: BTreeMap::new(),
            traversal_ready_sections: BTreeSet::new(),
            traversal_ready_columns: None,
            section_set_generation: 0,
            queue: queue.clone(),
            cached_records: RefCell::new(None),
            records_dirty: Cell::new(true),
            record_dirty_causes: Cell::new(TexturedSectionRecordDirtyCauses::INITIAL),
            record_cache_stats: Cell::new(TexturedSectionRecordCacheStats::default()),
            mono_culling_cache: RefCell::new(None),
            cull_scratch: RefCell::new(CullScratch::default()),
        };
        let _ = resources.update_sections(device, sections)?;
        Ok(resources)
    }

    pub fn shared_resources(&self) -> Arc<TexturedSectionSharedResources> {
        Arc::clone(&self.shared)
    }

    pub fn shares_immutable_resources_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }

    pub fn shared_resource_owner_count(&self) -> usize {
        Arc::strong_count(&self.shared)
    }

    /// Allocate the opt-in placed pipeline shell for this draw store. Nothing
    /// in `new` calls this, preserving the ordinary single-world allocation
    /// and shader-compilation path.
    pub fn create_placed_renderer(&self, device: &wgpu::Device) -> PlacedTexturedSectionRenderer {
        PlacedTexturedSectionRenderer::new(
            device,
            self.shared.renderer.color_format,
            &self.shared.renderer.texture_bind_group_layout,
        )
    }

    /// Eagerly materialize the placed full-frame multiview pipelines. Preview
    /// readiness calls this before publication on adapters that expose
    /// `MULTIVIEW`; it is never reached by the no-preview branch.
    pub fn materialize_placed_multiview_renderer(
        &self,
        device: &wgpu::Device,
        renderer: &PlacedTexturedSectionRenderer,
    ) -> Result<()> {
        drop(renderer.multiview_renderer(device, &self.shared.renderer.texture_bind_group_layout)?);
        Ok(())
    }

    pub fn prepare_render_records_for_context(
        &self,
        context: WorldCompositionContext,
    ) -> Arc<PreparedTexturedSectionRecords> {
        let records = self.prepare_render_records();
        match context.source_bounds() {
            Some(bounds) => Arc::new(records.for_source_bounds(bounds)),
            None => records,
        }
    }

    pub fn update_sections(
        &mut self,
        device: &wgpu::Device,
        sections: &[TexturedRenderSectionMesh],
    ) -> Result<TexturedSectionUploadReport> {
        let wanted = sections
            .iter()
            .map(|section| section.key)
            .collect::<BTreeSet<_>>();
        let removed = self
            .visibility_sections
            .keys()
            .copied()
            .filter(|key| !wanted.contains(key))
            .collect::<BTreeSet<_>>();
        self.apply_section_updates(device, sections, &removed)
    }

    pub fn apply_section_updates(
        &mut self,
        device: &wgpu::Device,
        sections: &[TexturedRenderSectionMesh],
        removed: &BTreeSet<RenderSectionKey>,
    ) -> Result<TexturedSectionUploadReport> {
        self.apply_section_updates_with_context(device, sections, removed, false)
    }

    pub fn apply_section_updates_with_context(
        &mut self,
        device: &wgpu::Device,
        sections: &[TexturedRenderSectionMesh],
        removed: &BTreeSet<RenderSectionKey>,
        upload_backpressured: bool,
    ) -> Result<TexturedSectionUploadReport> {
        self.apply_section_updates_with_context_timed(
            device,
            sections,
            removed,
            upload_backpressured,
        )
        .map(|(report, _timing)| report)
    }

    pub fn apply_section_updates_with_context_timed(
        &mut self,
        device: &wgpu::Device,
        sections: &[TexturedRenderSectionMesh],
        removed: &BTreeSet<RenderSectionKey>,
        upload_backpressured: bool,
    ) -> Result<(TexturedSectionUploadReport, TexturedSectionUploadTiming)> {
        let total_start = timing_now();
        let mut timing = TexturedSectionUploadTiming::default();
        if sections.is_empty() && removed.is_empty() {
            timing.total_ms = timing_elapsed_ms(total_start);
            return Ok((TexturedSectionUploadReport::default(), timing));
        }
        let dirty_start = timing_now();
        let membership_changed = removed
            .iter()
            .any(|key| self.visibility_sections.contains_key(key))
            || sections
                .iter()
                .any(|section| !self.visibility_sections.contains_key(&section.key));
        if membership_changed {
            self.section_set_generation = self.section_set_generation.wrapping_add(1);
        }
        // Keep failure handling conservative by marking the cache dirty before
        // GPU mutation. A successful bounded update patches only the affected
        // records below; an early error leaves the full lazy rebuild armed.
        let can_patch_cached_records =
            !self.records_dirty.get() && self.cached_records.borrow().is_some();
        let mut dirty_causes = TexturedSectionRecordDirtyCauses::empty();
        if !sections.is_empty() {
            dirty_causes = dirty_causes.with(TexturedSectionRecordDirtyCauses::SECTION_UPLOAD);
        }
        if !removed.is_empty() {
            dirty_causes = dirty_causes.with(TexturedSectionRecordDirtyCauses::SECTION_REMOVE);
        }
        if upload_backpressured {
            dirty_causes =
                dirty_causes.with(TexturedSectionRecordDirtyCauses::UPLOAD_BACKPRESSURED);
        }
        self.mark_records_dirty(dirty_causes);
        timing.dirty_mark_ms = timing_elapsed_ms(dirty_start);
        let changed_record_keys = removed
            .iter()
            .copied()
            .chain(sections.iter().map(|section| section.key))
            .collect::<BTreeSet<_>>();
        let mut report = TexturedSectionUploadReport::default();
        let remove_start = timing_now();
        for key in removed {
            if let Some(mesh) = self.sections.remove(key) {
                self.section_arena.release(mesh);
                report.removed_section_count += 1;
            }
            if self.visibility_sections.remove(key).is_some() {
                let pos = ChunkPos::new(key.chunk_x, key.chunk_z);
                if let Some(keys) = self.visibility_section_keys_by_chunk.get_mut(&pos) {
                    keys.remove(key);
                    if keys.is_empty() {
                        self.visibility_section_keys_by_chunk.remove(&pos);
                    }
                }
            }
            self.traversal_ready_sections.remove(key);
        }
        timing.remove_ms = timing_elapsed_ms(remove_start);

        for section in sections {
            let section_state_start = timing_now();
            let pos = ChunkPos::new(section.key.chunk_x, section.key.chunk_z);
            if self
                .visibility_sections
                .insert(section.key, section.visibility)
                .is_none()
            {
                self.visibility_section_keys_by_chunk
                    .entry(pos)
                    .or_default()
                    .insert(section.key);
            }
            let section_is_ready = self
                .traversal_ready_columns
                .as_ref()
                .is_none_or(|columns| columns.contains(&pos));
            if section_is_ready {
                self.traversal_ready_sections.insert(section.key);
            } else {
                self.traversal_ready_sections.remove(&section.key);
            }
            if section.is_empty() {
                if let Some(mesh) = self.sections.remove(&section.key) {
                    self.section_arena.release(mesh);
                    report.removed_section_count += 1;
                }
                timing.section_state_ms += timing_elapsed_ms(section_state_start);
                continue;
            }
            timing.section_state_ms += timing_elapsed_ms(section_state_start);
            let stats = section.stats();
            report.uploaded_section_count += 1;
            report.uploaded_vertex_count += stats.vertex_count;
            report.uploaded_index_count += stats.index_count;
            if let Some(previous) = self.sections.remove(&section.key) {
                self.section_arena.release(previous);
            }
            let (gpu_mesh, mesh_timing) = self
                .section_arena
                .upload(device, &self.queue, &section.mesh)
                .with_context(|| format!("failed to upload render section {:?}", section.key))?;
            timing.absorb_mesh_upload(mesh_timing);
            let mesh_insert_start = timing_now();
            self.sections.insert(section.key, gpu_mesh);
            timing.mesh_insert_ms += timing_elapsed_ms(mesh_insert_start);
        }
        self.section_arena
            .ensure_indirect_capacity(device, self.sections.len())?;
        if can_patch_cached_records && self.patch_cached_records(&changed_record_keys) {
            self.records_dirty.set(false);
            self.record_dirty_causes
                .set(TexturedSectionRecordDirtyCauses::empty());
        }
        timing.total_ms = timing_elapsed_ms(total_start);
        Ok((report, timing))
    }

    pub fn set_traversal_ready_sections(&mut self, ready_sections: &BTreeSet<RenderSectionKey>) {
        self.set_traversal_ready_sections_with_context(ready_sections, false);
    }

    pub fn record_traversal_ready_sections_skipped(&self, upload_backpressured: bool) {
        let mut stats = self.record_cache_stats.get();
        stats.record_ready_set_skip(upload_backpressured);
        self.record_cache_stats.set(stats);
    }

    pub fn set_traversal_ready_sections_with_context(
        &mut self,
        ready_sections: &BTreeSet<RenderSectionKey>,
        upload_backpressured: bool,
    ) {
        let filtered_ready_sections: BTreeSet<_> = self
            .visibility_sections
            .keys()
            .copied()
            .filter(|key| ready_sections.contains(key))
            .collect();
        let mut stats = self.record_cache_stats.get();
        let changed = filtered_ready_sections != self.traversal_ready_sections;
        stats.record_ready_set_call(upload_backpressured, changed);
        // The section-oriented API may represent a partial column. Leave
        // column mode even when the effective section set is unchanged so
        // later section insertions retain the legacy caller's semantics.
        self.traversal_ready_columns = None;
        if !changed {
            self.record_cache_stats.set(stats);
            return;
        }
        self.record_cache_stats.set(stats);
        // Slice F follow-up: readiness flips change cached `traversal_ready`.
        // Reasserting the same ready set should not rebuild prepared records.
        let changed_record_keys = filtered_ready_sections
            .symmetric_difference(&self.traversal_ready_sections)
            .copied()
            .collect::<BTreeSet<_>>();
        let can_patch_cached_records =
            !self.records_dirty.get() && self.cached_records.borrow().is_some();
        let mut dirty_causes = TexturedSectionRecordDirtyCauses::TRAVERSAL_READY;
        if upload_backpressured {
            dirty_causes =
                dirty_causes.with(TexturedSectionRecordDirtyCauses::UPLOAD_BACKPRESSURED);
        }
        self.mark_records_dirty(dirty_causes);
        self.traversal_ready_sections = filtered_ready_sections;
        if can_patch_cached_records && self.patch_cached_records(&changed_record_keys) {
            self.records_dirty.set(false);
            self.record_dirty_causes
                .set(TexturedSectionRecordDirtyCauses::empty());
        }
    }

    pub fn set_traversal_ready_columns_with_context(
        &mut self,
        ready_columns: &BTreeSet<ChunkPos>,
        upload_backpressured: bool,
    ) {
        let changed_record_keys =
            if let Some(previous_columns) = self.traversal_ready_columns.as_ref() {
                previous_columns
                    .symmetric_difference(ready_columns)
                    .filter_map(|pos| self.visibility_section_keys_by_chunk.get(pos))
                    .flat_map(|keys| keys.iter().copied())
                    .collect::<BTreeSet<_>>()
            } else {
                let desired_ready_sections = ready_columns
                    .iter()
                    .filter_map(|pos| self.visibility_section_keys_by_chunk.get(pos))
                    .flat_map(|keys| keys.iter().copied())
                    .collect::<BTreeSet<_>>();
                desired_ready_sections
                    .symmetric_difference(&self.traversal_ready_sections)
                    .copied()
                    .collect()
            };
        let changed = !changed_record_keys.is_empty();
        let mut stats = self.record_cache_stats.get();
        stats.record_ready_set_call(upload_backpressured, changed);
        self.record_cache_stats.set(stats);
        self.traversal_ready_columns = Some(ready_columns.clone());
        if !changed {
            return;
        }

        let can_patch_cached_records =
            !self.records_dirty.get() && self.cached_records.borrow().is_some();
        let mut dirty_causes = TexturedSectionRecordDirtyCauses::TRAVERSAL_READY;
        if upload_backpressured {
            dirty_causes =
                dirty_causes.with(TexturedSectionRecordDirtyCauses::UPLOAD_BACKPRESSURED);
        }
        self.mark_records_dirty(dirty_causes);
        for key in &changed_record_keys {
            let pos = ChunkPos::new(key.chunk_x, key.chunk_z);
            if ready_columns.contains(&pos) {
                self.traversal_ready_sections.insert(*key);
            } else {
                self.traversal_ready_sections.remove(key);
            }
        }
        if can_patch_cached_records && self.patch_cached_records(&changed_record_keys) {
            self.records_dirty.set(false);
            self.record_dirty_causes
                .set(TexturedSectionRecordDirtyCauses::empty());
        }
    }

    pub fn traversal_ready_section_count(&self) -> usize {
        self.traversal_ready_sections.len()
    }

    pub const fn traversal_ready_source_generation(&self) -> u64 {
        self.section_set_generation
    }

    pub fn record_cache_stats(&self) -> TexturedSectionRecordCacheStats {
        self.record_cache_stats.get()
    }

    fn mark_records_dirty(&self, causes: TexturedSectionRecordDirtyCauses) {
        let mut dirty_causes = self.record_dirty_causes.get();
        dirty_causes = dirty_causes.with(causes);
        self.record_dirty_causes.set(dirty_causes);
        self.records_dirty.set(true);
    }

    fn patch_cached_records(&self, changed_keys: &BTreeSet<RenderSectionKey>) -> bool {
        // The mono cull cache retains the previous records Arc so pointer
        // identity can safely identify its generation. Drop that internal
        // reference before Arc::make_mut so bounded mesh/readiness updates
        // normally patch the map in place instead of cloning every record.
        *self.mono_culling_cache.borrow_mut() = None;
        let mut cached = self.cached_records.borrow_mut();
        let Some(cached) = cached.as_mut() else {
            return false;
        };
        let prepared = Arc::make_mut(cached);
        for key in changed_keys {
            if let Some(previous) = prepared.records.remove(key)
                && previous.drawable
            {
                prepared.loaded_section_count = prepared.loaded_section_count.saturating_sub(1);
                prepared.loaded_index_count = prepared
                    .loaded_index_count
                    .saturating_sub(previous.index_count);
            }
            let Some(visibility) = self.visibility_sections.get(key).copied() else {
                continue;
            };
            let mesh = self.sections.get(key);
            let next = TexturedSectionCullingRecord {
                index_count: mesh.map_or(0, GpuTexturedSectionMesh::index_count),
                visibility,
                drawable: mesh.is_some(),
                traversal_ready: self.traversal_ready_sections.contains(key),
            };
            if next.drawable {
                prepared.loaded_section_count = prepared.loaded_section_count.saturating_add(1);
                prepared.loaded_index_count =
                    prepared.loaded_index_count.saturating_add(next.index_count);
            }
            prepared.records.insert(*key, next);
        }
        true
    }

    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    pub fn contains_section(&self, key: RenderSectionKey) -> bool {
        self.sections.contains_key(&key)
    }

    pub fn traversal_ready_contains_section(&self, key: RenderSectionKey) -> bool {
        self.traversal_ready_sections.contains(&key)
    }

    /// Materialize the terrain multiview shader/pipelines without submitting a
    /// draw. Warm-world admission uses this before publishing switchability so
    /// first-use pipeline creation can never land on the switch frame.
    pub fn materialize_multiview_renderer(&self, device: &wgpu::Device) -> Result<bool> {
        let already_materialized = self.shared.renderer.multiview.borrow().is_some();
        drop(self.shared.renderer.multiview_renderer(device)?);
        Ok(!already_materialized)
    }

    pub fn multiview_renderer_materialized(&self) -> bool {
        self.shared.renderer.multiview.borrow().is_some()
    }

    pub fn index_count(&self) -> u32 {
        self.sections.values().map(|mesh| mesh.index_count()).sum()
    }

    pub fn vertex_count(&self) -> u32 {
        self.sections
            .values()
            .map(GpuTexturedSectionMesh::vertex_count)
            .sum()
    }

    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
    ) -> Result<TexturedSectionRenderStats> {
        self.render_with_options_in_slot(
            queue,
            encoder,
            target,
            render_view,
            TexturedSectionRenderOptions::default(),
            SINGLE_VIEW_SLOT,
        )
    }

    pub fn render_with_options(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
    ) -> Result<TexturedSectionRenderStats> {
        self.render_with_options_in_slot(
            queue,
            encoder,
            target,
            render_view,
            options,
            SINGLE_VIEW_SLOT,
        )
    }

    pub fn render_with_options_in_slot(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
    ) -> Result<TexturedSectionRenderStats> {
        self.render_with_options_phase_in_slot(
            queue,
            encoder,
            target,
            render_view,
            options,
            view_slot,
            TexturedSectionRenderPhase::All,
        )
    }

    pub fn render_with_options_phase_in_slot(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
        phase: TexturedSectionRenderPhase,
    ) -> Result<TexturedSectionRenderStats> {
        self.render_with_options_inner(
            queue,
            encoder,
            target,
            render_view,
            options,
            view_slot,
            None,
            None,
            phase,
        )
    }

    pub fn render_with_options_timed(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
    ) -> Result<(TexturedSectionRenderStats, TexturedSectionRenderTiming)> {
        self.render_with_options_timed_in_slot(
            queue,
            encoder,
            target,
            render_view,
            options,
            SINGLE_VIEW_SLOT,
        )
    }

    pub fn render_with_options_timed_in_slot(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
    ) -> Result<(TexturedSectionRenderStats, TexturedSectionRenderTiming)> {
        self.render_with_options_phase_timed_in_slot(
            queue,
            encoder,
            target,
            render_view,
            options,
            view_slot,
            TexturedSectionRenderPhase::All,
        )
    }

    pub fn render_with_options_phase_timed_in_slot(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
        phase: TexturedSectionRenderPhase,
    ) -> Result<(TexturedSectionRenderStats, TexturedSectionRenderTiming)> {
        let mut timing = TexturedSectionRenderTiming::default();
        let stats = self.render_with_options_inner(
            queue,
            encoder,
            target,
            render_view,
            options,
            view_slot,
            None,
            Some(&mut timing),
            phase,
        )?;
        Ok((stats, timing))
    }

    /// Shared cache accessor (Slice F): rebuild only when the section set /
    /// readiness changed since the last build; otherwise hand back the cached
    /// records (an O(1) `Arc` clone). Takes `&self` via interior mutability so
    /// both the XR stereo path and the single-view `render_with_options_inner`
    /// path reuse one cache.
    pub fn prepare_render_records(&self) -> Arc<PreparedTexturedSectionRecords> {
        self.prepare_render_records_with_stats().0
    }

    /// Recompute the exact drawable section keys for one view from the same
    /// cached records and culling routine used by rendering.
    ///
    /// This is a pull-only settle diagnostic. Normal frames retain count-only
    /// summaries and do not clone or store the culling set after encoding.
    pub fn section_view_set_snapshot(
        &self,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
    ) -> TexturedSectionViewSetSnapshot {
        let records = self.prepare_render_records();
        let (culling, paintable_frustum_keys) = {
            let mut scratch = self.cull_scratch.borrow_mut();
            let culling = cull_textured_sections(&records, render_view, options, &mut scratch);
            let paintable_frustum_keys = scratch
                .frustum_keys
                .iter()
                .copied()
                .filter(|key| {
                    records
                        .records
                        .get(key)
                        .is_some_and(|record| record.drawable && record.traversal_ready)
                })
                .collect();
            (culling, paintable_frustum_keys)
        };
        let drawn_keys = culling
            .drawn_keys
            .into_iter()
            .filter(|key| self.sections.contains_key(key))
            .collect();
        TexturedSectionViewSetSnapshot {
            paintable_frustum_keys,
            drawn_keys,
        }
    }

    pub fn prepare_render_records_with_stats(
        &self,
    ) -> (
        Arc<PreparedTexturedSectionRecords>,
        TexturedSectionRecordPrepareStats,
    ) {
        let mut prepare_stats = TexturedSectionRecordPrepareStats::default();
        if self.records_dirty.get() || self.cached_records.borrow().is_none() {
            let rebuild_start = timing_now();
            let rebuilt_records = self.build_prepared_records();
            let rebuild_ms = timing_elapsed_ms(rebuild_start);
            let mut dirty_causes = self.record_dirty_causes.get();
            if dirty_causes.is_empty() {
                dirty_causes = TexturedSectionRecordDirtyCauses::INITIAL;
            }
            let visibility_section_count = rebuilt_records.records.len();
            let loaded_section_count = rebuilt_records.loaded_section_count;
            let ready_section_count = self.traversal_ready_sections.len();
            let loaded_index_count = rebuilt_records.loaded_index_count;
            let rebuilt = Arc::new(rebuilt_records);
            *self.cached_records.borrow_mut() = Some(rebuilt);
            self.records_dirty.set(false);
            self.record_dirty_causes
                .set(TexturedSectionRecordDirtyCauses::empty());
            let mut cache_stats = self.record_cache_stats.get();
            cache_stats.record_prepared_record_rebuild(
                rebuild_ms,
                dirty_causes,
                visibility_section_count,
                loaded_section_count,
                ready_section_count,
                loaded_index_count,
            );
            self.record_cache_stats.set(cache_stats);
            prepare_stats.rebuilt = true;
            prepare_stats.rebuild_ms = rebuild_ms;
            prepare_stats.dirty_causes = dirty_causes;
            prepare_stats.visibility_section_count = visibility_section_count;
            prepare_stats.loaded_section_count = loaded_section_count;
            prepare_stats.ready_section_count = ready_section_count;
            prepare_stats.loaded_index_count = loaded_index_count;
        }
        prepare_stats.cache = self.record_cache_stats.get();
        (
            Arc::clone(
                self.cached_records
                    .borrow()
                    .as_ref()
                    .expect("records cached above"),
            ),
            prepare_stats,
        )
    }

    pub fn prepare_stereo_draw(
        &self,
        records: &PreparedTexturedSectionRecords,
        render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
    ) -> PreparedTexturedSectionStereoDraw {
        self.prepare_stereo_draw_inner(records, render_views, options, None)
    }

    pub fn prepare_stereo_draw_timed(
        &self,
        records: &PreparedTexturedSectionRecords,
        render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
    ) -> (
        PreparedTexturedSectionStereoDraw,
        TexturedSectionRenderTiming,
    ) {
        let mut timing = TexturedSectionRenderTiming::default();
        let prepared_draw =
            self.prepare_stereo_draw_inner(records, render_views, options, Some(&mut timing));
        (prepared_draw, timing)
    }

    pub fn render_prepared_with_options(
        &self,
        records: &PreparedTexturedSectionRecords,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
    ) -> Result<TexturedSectionRenderStats> {
        self.render_prepared_with_options_in_slot(
            records,
            queue,
            encoder,
            target,
            render_view,
            options,
            SINGLE_VIEW_SLOT,
        )
    }

    pub fn render_prepared_with_options_in_slot(
        &self,
        records: &PreparedTexturedSectionRecords,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
    ) -> Result<TexturedSectionRenderStats> {
        self.render_prepared_phase_with_options_in_slot(
            records,
            queue,
            encoder,
            target,
            render_view,
            options,
            view_slot,
            TexturedSectionRenderPhase::All,
        )
    }

    pub fn render_prepared_phase_with_options_in_slot(
        &self,
        records: &PreparedTexturedSectionRecords,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
        phase: TexturedSectionRenderPhase,
    ) -> Result<TexturedSectionRenderStats> {
        self.render_with_options_inner(
            queue,
            encoder,
            target,
            render_view,
            options,
            view_slot,
            Some(records),
            None,
            phase,
        )
    }

    pub fn render_prepared_with_options_timed(
        &self,
        records: &PreparedTexturedSectionRecords,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
    ) -> Result<(TexturedSectionRenderStats, TexturedSectionRenderTiming)> {
        self.render_prepared_with_options_timed_in_slot(
            records,
            queue,
            encoder,
            target,
            render_view,
            options,
            SINGLE_VIEW_SLOT,
        )
    }

    pub fn render_prepared_with_options_timed_in_slot(
        &self,
        records: &PreparedTexturedSectionRecords,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
    ) -> Result<(TexturedSectionRenderStats, TexturedSectionRenderTiming)> {
        self.render_prepared_phase_with_options_timed_in_slot(
            records,
            queue,
            encoder,
            target,
            render_view,
            options,
            view_slot,
            TexturedSectionRenderPhase::All,
        )
    }

    pub fn render_prepared_phase_with_options_timed_in_slot(
        &self,
        records: &PreparedTexturedSectionRecords,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
        phase: TexturedSectionRenderPhase,
    ) -> Result<(TexturedSectionRenderStats, TexturedSectionRenderTiming)> {
        let mut timing = TexturedSectionRenderTiming::default();
        let stats = self.render_with_options_inner(
            queue,
            encoder,
            target,
            render_view,
            options,
            view_slot,
            Some(records),
            Some(&mut timing),
            phase,
        )?;
        Ok((stats, timing))
    }

    pub fn render_prepared_stereo_draw_with_options_in_slot(
        &self,
        prepared_draw: &PreparedTexturedSectionStereoDraw,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
    ) -> Result<TexturedSectionRenderStats> {
        self.render_prepared_stereo_draw_phase_with_options_in_slot(
            prepared_draw,
            queue,
            encoder,
            target,
            render_view,
            options,
            view_slot,
            TexturedSectionRenderPhase::All,
        )
    }

    pub fn render_prepared_stereo_draw_phase_with_options_in_slot(
        &self,
        prepared_draw: &PreparedTexturedSectionStereoDraw,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
        phase: TexturedSectionRenderPhase,
    ) -> Result<TexturedSectionRenderStats> {
        self.render_prepared_stereo_draw_inner(
            prepared_draw,
            queue,
            encoder,
            target,
            render_view,
            options,
            view_slot,
            None,
            phase,
        )
    }

    pub fn render_prepared_stereo_draw_timed_in_slot(
        &self,
        prepared_draw: &PreparedTexturedSectionStereoDraw,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
    ) -> Result<(TexturedSectionRenderStats, TexturedSectionRenderTiming)> {
        self.render_prepared_stereo_draw_phase_timed_in_slot(
            prepared_draw,
            queue,
            encoder,
            target,
            render_view,
            options,
            view_slot,
            TexturedSectionRenderPhase::All,
        )
    }

    pub fn render_prepared_stereo_draw_phase_timed_in_slot(
        &self,
        prepared_draw: &PreparedTexturedSectionStereoDraw,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
        phase: TexturedSectionRenderPhase,
    ) -> Result<(TexturedSectionRenderStats, TexturedSectionRenderTiming)> {
        let mut timing = TexturedSectionRenderTiming::default();
        let stats = self.render_prepared_stereo_draw_inner(
            prepared_draw,
            queue,
            encoder,
            target,
            render_view,
            options,
            view_slot,
            Some(&mut timing),
            phase,
        )?;
        Ok((stats, timing))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_placed_prepared_with_options_in_slot(
        &self,
        renderer: &PlacedTexturedSectionRenderer,
        device: &wgpu::Device,
        records: &PreparedTexturedSectionRecords,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        physical_render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        context: WorldCompositionContext,
        view_slot: PerViewSlot,
    ) -> Result<TexturedSectionRenderStats> {
        self.render_placed_prepared_with_options_inner(
            renderer,
            device,
            records,
            queue,
            encoder,
            target,
            physical_render_view,
            options,
            context,
            view_slot,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_placed_prepared_with_options_timed_in_slot(
        &self,
        renderer: &PlacedTexturedSectionRenderer,
        device: &wgpu::Device,
        records: &PreparedTexturedSectionRecords,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        physical_render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        context: WorldCompositionContext,
        view_slot: PerViewSlot,
    ) -> Result<(TexturedSectionRenderStats, TexturedSectionRenderTiming)> {
        let mut timing = TexturedSectionRenderTiming::default();
        let stats = self.render_placed_prepared_with_options_inner(
            renderer,
            device,
            records,
            queue,
            encoder,
            target,
            physical_render_view,
            options,
            context,
            view_slot,
            Some(&mut timing),
        )?;
        Ok((stats, timing))
    }

    #[allow(clippy::too_many_arguments)]
    fn render_placed_prepared_with_options_inner(
        &self,
        renderer: &PlacedTexturedSectionRenderer,
        device: &wgpu::Device,
        records: &PreparedTexturedSectionRecords,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        physical_render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        context: WorldCompositionContext,
        view_slot: PerViewSlot,
        mut timing: Option<&mut TexturedSectionRenderTiming>,
    ) -> Result<TexturedSectionRenderStats> {
        let placement = context.placement();
        let cull_start = timing.is_some().then(timing_now);
        let source_render_view = placement.source_render_view(physical_render_view);
        let placed_frustum = PlacedClipFrustum::new(physical_render_view, context);
        let culling = {
            let mut scratch = self.cull_scratch.borrow_mut();
            cull_textured_sections_with_frustum(
                records,
                source_render_view,
                options,
                &mut scratch,
                &placed_frustum,
            )
        };
        if let (Some(timing), Some(start)) = (timing.as_deref_mut(), cull_start) {
            timing.cull_ms += timing_elapsed_ms(start);
        }
        let uniform_start = timing.is_some().then(timing_now);
        let selected = renderer.selected_mono_renderer(
            device,
            &self.shared.renderer.texture_bind_group_layout,
            context.clip(),
        );
        let uniform_offset = selected.write_uniforms(
            queue,
            view_slot,
            physical_render_view,
            options,
            context,
            renderer.color_format,
        );
        if let (Some(timing), Some(start)) = (timing.as_deref_mut(), uniform_start) {
            timing.uniform_write_ms += timing_elapsed_ms(start);
        }
        let encode_start = timing.is_some().then(timing_now);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mclone_placed_textured_section_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: target.color_load_op(),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: target.depth_load_op(),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, selected.bind_group(), &[uniform_offset]);
            pass.set_bind_group(1, &self.shared.atlas.bind_group, &[]);
            bind_textured_section_arena(&mut pass, &self.section_arena);
            pass.set_pipeline(selected.solid_pipeline());
            for (key, mesh) in &self.sections {
                if culling.drawn_keys.contains(key) {
                    draw_textured_section_mesh_range(
                        &mut pass,
                        &self.section_arena,
                        mesh,
                        mesh.solid_index_range(),
                    );
                }
            }
            pass.set_pipeline(selected.cutout_pipeline());
            for (key, mesh) in &self.sections {
                if culling.drawn_keys.contains(key) {
                    draw_textured_section_mesh_range(
                        &mut pass,
                        &self.section_arena,
                        mesh,
                        mesh.cutout_index_range(),
                    );
                }
            }
        }
        if let (Some(timing), Some(start)) = (timing.as_deref_mut(), encode_start) {
            timing.encode_ms += timing_elapsed_ms(start);
        }
        Ok(culling.stats)
    }

    pub fn prepare_placed_stereo_draw(
        &self,
        records: &PreparedTexturedSectionRecords,
        physical_render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
        context: WorldCompositionContext,
    ) -> PreparedTexturedSectionStereoDraw {
        let placement = context.placement();
        let source_render_views =
            physical_render_views.map(|view| placement.source_render_view(view));
        let frustums = physical_render_views.map(|view| PlacedClipFrustum::new(view, context));
        let culling = {
            let mut scratch = self.cull_scratch.borrow_mut();
            cull_textured_sections_stereo_union_with_frustums(
                records,
                source_render_views,
                options,
                &mut scratch,
                &frustums,
            )
        };
        let sort_view = stereo_translucent_sort_view(physical_render_views);
        let mut translucent_keys = self
            .sections
            .iter()
            .filter(|(key, mesh)| {
                culling.drawn_keys.contains(key) && !mesh.translucent_index_range().is_empty()
            })
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        translucent_keys.sort_by(|left, right| {
            compare_placed_translucent_sections(*left, *right, sort_view, placement)
        });
        PreparedTexturedSectionStereoDraw {
            union_stats: stereo_union_stats_for_options(culling.stats, options),
            eye_stats: culling.eye_stats,
            drawn_keys: culling.drawn_keys,
            draw_masks: culling.draw_masks,
            translucent_keys,
        }
    }

    /// Cull one placed source for a mono composition view and return only its
    /// visible translucent sections. The ordinary direct path never calls this;
    /// it exists for opt-in cross-world composition.
    pub fn prepare_placed_translucent_records(
        &self,
        records: &PreparedTexturedSectionRecords,
        physical_render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        context: WorldCompositionContext,
    ) -> Vec<TexturedSectionTranslucentRecord> {
        let placement = context.placement();
        let source_render_view = placement.source_render_view(physical_render_view);
        let placed_frustum = PlacedClipFrustum::new(physical_render_view, context);
        let culling = {
            let mut scratch = self.cull_scratch.borrow_mut();
            cull_textured_sections_with_frustum(
                records,
                source_render_view,
                options,
                &mut scratch,
                &placed_frustum,
            )
        };
        self.sections
            .iter()
            .filter(|(key, mesh)| {
                culling.drawn_keys.contains(key) && !mesh.translucent_index_range().is_empty()
            })
            .map(|(key, _)| textured_section_translucent_record(*key, placement))
            .collect()
    }

    pub fn prepare_placed_stereo_draw_timed(
        &self,
        records: &PreparedTexturedSectionRecords,
        physical_render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
        context: WorldCompositionContext,
    ) -> (
        PreparedTexturedSectionStereoDraw,
        TexturedSectionRenderTiming,
    ) {
        let started_at = timing_now();
        let prepared =
            self.prepare_placed_stereo_draw(records, physical_render_views, options, context);
        let elapsed_ms = timing_elapsed_ms(started_at);
        (
            prepared,
            TexturedSectionRenderTiming {
                cull_ms: elapsed_ms,
                prepare_ms: elapsed_ms,
                ..TexturedSectionRenderTiming::default()
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_placed_prepared_stereo_draw_with_options_in_slot(
        &self,
        renderer: &PlacedTexturedSectionRenderer,
        device: &wgpu::Device,
        prepared_draw: &PreparedTexturedSectionStereoDraw,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        physical_render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        context: WorldCompositionContext,
        view_slot: PerViewSlot,
    ) -> Result<TexturedSectionRenderStats> {
        let selected = renderer.selected_mono_renderer(
            device,
            &self.shared.renderer.texture_bind_group_layout,
            context.clip(),
        );
        let uniform_offset = selected.write_uniforms(
            queue,
            view_slot,
            physical_render_view,
            options,
            context,
            renderer.color_format,
        );
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mclone_placed_textured_section_stereo_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: target.color_load_op(),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: target.depth_load_op(),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, selected.bind_group(), &[uniform_offset]);
            pass.set_bind_group(1, &self.shared.atlas.bind_group, &[]);
            bind_textured_section_arena(&mut pass, &self.section_arena);
            pass.set_pipeline(selected.solid_pipeline());
            for (key, mesh) in &self.sections {
                if prepared_draw.draws_in_eye(*key, stereo_eye_for_slot(view_slot)) {
                    draw_textured_section_mesh_range(
                        &mut pass,
                        &self.section_arena,
                        mesh,
                        mesh.solid_index_range(),
                    );
                }
            }
            pass.set_pipeline(selected.cutout_pipeline());
            for (key, mesh) in &self.sections {
                if prepared_draw.draws_in_eye(*key, stereo_eye_for_slot(view_slot)) {
                    draw_textured_section_mesh_range(
                        &mut pass,
                        &self.section_arena,
                        mesh,
                        mesh.cutout_index_range(),
                    );
                }
            }
        }
        Ok(prepared_draw.stats_for_eye(stereo_eye_for_slot(view_slot)))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_placed_prepared_stereo_draw_with_options_timed_in_slot(
        &self,
        renderer: &PlacedTexturedSectionRenderer,
        device: &wgpu::Device,
        prepared_draw: &PreparedTexturedSectionStereoDraw,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        physical_render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        context: WorldCompositionContext,
        view_slot: PerViewSlot,
    ) -> Result<(TexturedSectionRenderStats, TexturedSectionRenderTiming)> {
        let started_at = timing_now();
        let stats = self.render_placed_prepared_stereo_draw_with_options_in_slot(
            renderer,
            device,
            prepared_draw,
            queue,
            encoder,
            target,
            physical_render_view,
            options,
            context,
            view_slot,
        )?;
        Ok((
            stats,
            TexturedSectionRenderTiming {
                encode_ms: timing_elapsed_ms(started_at),
                ..TexturedSectionRenderTiming::default()
            },
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_placed_prepared_multiview_stereo_draw_with_options(
        &self,
        renderer: &PlacedTexturedSectionRenderer,
        prepared_draw: &PreparedTexturedSectionStereoDraw,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkMultiviewRenderTarget<'_>,
        physical_render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
        context: WorldCompositionContext,
    ) -> Result<[TexturedSectionRenderStats; 2]> {
        let multiview = renderer.selected_multiview_renderer(
            device,
            &self.shared.renderer.texture_bind_group_layout,
            context.clip(),
        )?;
        multiview.write_uniforms(
            queue,
            physical_render_views,
            options,
            context,
            renderer.color_format,
        );
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mclone_placed_textured_section_multiview_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: target.color_load_op(),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: target.depth_load_op(),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, multiview.bind_group(), &[]);
            pass.set_bind_group(1, &self.shared.atlas.bind_group, &[]);
            bind_textured_section_arena(&mut pass, &self.section_arena);
            pass.set_pipeline(multiview.solid_pipeline());
            for (key, mesh) in &self.sections {
                if prepared_draw.drawn_keys.contains(key) {
                    draw_textured_section_mesh_range(
                        &mut pass,
                        &self.section_arena,
                        mesh,
                        mesh.solid_index_range(),
                    );
                }
            }
            pass.set_pipeline(multiview.cutout_pipeline());
            for (key, mesh) in &self.sections {
                if prepared_draw.drawn_keys.contains(key) {
                    draw_textured_section_mesh_range(
                        &mut pass,
                        &self.section_arena,
                        mesh,
                        mesh.cutout_index_range(),
                    );
                }
            }
        }
        Ok(prepared_draw.stats())
    }

    /// Draw an already composition-sorted run of direct-world translucent
    /// sections. Callers may pass a stereo preparation to suppress sections
    /// outside the current eye while preserving one midpoint-derived order for
    /// both eyes.
    #[allow(clippy::too_many_arguments)]
    pub fn render_ordered_translucent_sections_in_slot(
        &self,
        keys: &[RenderSectionKey],
        prepared_stereo_draw: Option<&PreparedTexturedSectionStereoDraw>,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
    ) {
        if keys.is_empty() {
            return;
        }
        let uniform_offset = self.shared.renderer.uniforms.write_slot(
            queue,
            view_slot,
            &uniform_bytes(render_view, options, self.shared.renderer.color_format),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_ordered_translucent_section_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: target.color_load_op(),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: target.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: target.depth_load_op(),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, &self.shared.renderer.bind_group, &[uniform_offset]);
        pass.set_bind_group(1, &self.shared.atlas.bind_group, &[]);
        bind_textured_section_arena(&mut pass, &self.section_arena);
        pass.set_pipeline(&self.shared.renderer.translucent_pipeline);
        for key in keys {
            if prepared_stereo_draw.is_some_and(|prepared| {
                !prepared.draws_in_eye(*key, stereo_eye_for_slot(view_slot))
            }) {
                continue;
            }
            if let Some(mesh) = self.sections.get(key) {
                draw_textured_section_mesh_range(
                    &mut pass,
                    &self.section_arena,
                    mesh,
                    mesh.translucent_index_range(),
                );
            }
        }
    }

    /// Draw an already composition-sorted run of placed translucent sections.
    #[allow(clippy::too_many_arguments)]
    pub fn render_ordered_placed_translucent_sections_in_slot(
        &self,
        renderer: &PlacedTexturedSectionRenderer,
        device: &wgpu::Device,
        keys: &[RenderSectionKey],
        prepared_stereo_draw: Option<&PreparedTexturedSectionStereoDraw>,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        physical_render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        context: WorldCompositionContext,
        view_slot: PerViewSlot,
    ) {
        if keys.is_empty() {
            return;
        }
        let selected = renderer.selected_mono_renderer(
            device,
            &self.shared.renderer.texture_bind_group_layout,
            context.clip(),
        );
        let uniform_offset = selected.write_uniforms(
            queue,
            view_slot,
            physical_render_view,
            options,
            context,
            renderer.color_format,
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_ordered_placed_translucent_section_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: target.color_load_op(),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: target.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: target.depth_load_op(),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, selected.bind_group(), &[uniform_offset]);
        pass.set_bind_group(1, &self.shared.atlas.bind_group, &[]);
        bind_textured_section_arena(&mut pass, &self.section_arena);
        pass.set_pipeline(selected.translucent_pipeline());
        for key in keys {
            if prepared_stereo_draw.is_some_and(|prepared| {
                !prepared.draws_in_eye(*key, stereo_eye_for_slot(view_slot))
            }) {
                continue;
            }
            if let Some(mesh) = self.sections.get(key) {
                draw_textured_section_mesh_range(
                    &mut pass,
                    &self.section_arena,
                    mesh,
                    mesh.translucent_index_range(),
                );
            }
        }
    }

    pub fn render_ordered_translucent_sections_multiview(
        &self,
        keys: &[RenderSectionKey],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkMultiviewRenderTarget<'_>,
        render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
    ) -> Result<()> {
        if keys.is_empty() {
            return Ok(());
        }
        let renderer = self.shared.renderer.multiview_renderer(device)?;
        renderer.write_uniforms(
            queue,
            render_views,
            options,
            self.shared.renderer.color_format,
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_ordered_translucent_section_multiview_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: target.color_load_op(),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: target.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: target.depth_load_op(),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, &renderer.bind_group, &[]);
        pass.set_bind_group(1, &self.shared.atlas.bind_group, &[]);
        bind_textured_section_arena(&mut pass, &self.section_arena);
        pass.set_pipeline(&renderer.translucent_pipeline);
        for key in keys {
            if let Some(mesh) = self.sections.get(key) {
                draw_textured_section_mesh_range(
                    &mut pass,
                    &self.section_arena,
                    mesh,
                    mesh.translucent_index_range(),
                );
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_ordered_placed_translucent_sections_multiview(
        &self,
        renderer: &PlacedTexturedSectionRenderer,
        keys: &[RenderSectionKey],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkMultiviewRenderTarget<'_>,
        physical_render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
        context: WorldCompositionContext,
    ) -> Result<()> {
        if keys.is_empty() {
            return Ok(());
        }
        let multiview = renderer.selected_multiview_renderer(
            device,
            &self.shared.renderer.texture_bind_group_layout,
            context.clip(),
        )?;
        multiview.write_uniforms(
            queue,
            physical_render_views,
            options,
            context,
            renderer.color_format,
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_ordered_placed_translucent_multiview_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: target.color_load_op(),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: target.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: target.depth_load_op(),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, multiview.bind_group(), &[]);
        pass.set_bind_group(1, &self.shared.atlas.bind_group, &[]);
        bind_textured_section_arena(&mut pass, &self.section_arena);
        pass.set_pipeline(multiview.translucent_pipeline());
        for key in keys {
            if let Some(mesh) = self.sections.get(key) {
                draw_textured_section_mesh_range(
                    &mut pass,
                    &self.section_arena,
                    mesh,
                    mesh.translucent_index_range(),
                );
            }
        }
        Ok(())
    }

    pub fn render_prepared_multiview_with_options(
        &self,
        records: &PreparedTexturedSectionRecords,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkMultiviewRenderTarget<'_>,
        render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
    ) -> Result<[TexturedSectionRenderStats; 2]> {
        let prepared_draw = self.prepare_stereo_draw(records, render_views, options);
        self.render_prepared_multiview_stereo_draw_with_options(
            &prepared_draw,
            device,
            queue,
            encoder,
            target,
            render_views,
            options,
        )
    }

    pub fn render_prepared_multiview_stereo_draw_with_options(
        &self,
        prepared_draw: &PreparedTexturedSectionStereoDraw,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkMultiviewRenderTarget<'_>,
        render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
    ) -> Result<[TexturedSectionRenderStats; 2]> {
        self.render_prepared_multiview_stereo_draw_phase_with_options(
            prepared_draw,
            device,
            queue,
            encoder,
            target,
            render_views,
            options,
            TexturedSectionRenderPhase::All,
        )
    }

    pub fn render_prepared_multiview_stereo_draw_phase_with_options(
        &self,
        prepared_draw: &PreparedTexturedSectionStereoDraw,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkMultiviewRenderTarget<'_>,
        render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
        phase: TexturedSectionRenderPhase,
    ) -> Result<[TexturedSectionRenderStats; 2]> {
        let renderer = self.shared.renderer.multiview_renderer(device)?;
        renderer.write_uniforms(
            queue,
            render_views,
            options,
            self.shared.renderer.color_format,
        );
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mclone_textured_section_multiview_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: target.color_load_op(),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: target.depth_load_op(),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: target.gpu_timestamp_writes(phase.gpu_pass_id()),
                ..Default::default()
            });
            pass.set_bind_group(0, &renderer.bind_group, &[]);
            pass.set_bind_group(1, &self.shared.atlas.bind_group, &[]);
            bind_textured_section_arena(&mut pass, &self.section_arena);
            if phase.draws_opaque() {
                pass.set_pipeline(&renderer.solid_pipeline);
                for (key, mesh) in &self.sections {
                    if !prepared_draw.drawn_keys.contains(key) {
                        continue;
                    }
                    draw_textured_section_mesh_range(
                        &mut pass,
                        &self.section_arena,
                        mesh,
                        mesh.solid_index_range(),
                    );
                }
                pass.set_pipeline(&renderer.cutout_pipeline);
                for (key, mesh) in &self.sections {
                    if !prepared_draw.drawn_keys.contains(key) {
                        continue;
                    }
                    draw_textured_section_mesh_range(
                        &mut pass,
                        &self.section_arena,
                        mesh,
                        mesh.cutout_index_range(),
                    );
                }
            }
            if phase.draws_translucent() {
                pass.set_pipeline(&renderer.translucent_pipeline);
                for key in &prepared_draw.translucent_keys {
                    if let Some(mesh) = self.sections.get(key) {
                        draw_textured_section_mesh_range(
                            &mut pass,
                            &self.section_arena,
                            mesh,
                            mesh.translucent_index_range(),
                        );
                    }
                }
            }
        }
        Ok(prepared_draw.stats())
    }

    fn render_with_options_inner(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
        prepared_records: Option<&PreparedTexturedSectionRecords>,
        mut timing: Option<&mut TexturedSectionRenderTiming>,
        phase: TexturedSectionRenderPhase,
    ) -> Result<TexturedSectionRenderStats> {
        let prepare_start = timing.as_ref().map(|_| timing_now());
        let records_storage = if prepared_records.is_none() {
            let records_start = timing.as_ref().map(|_| timing_now());
            let records = self.prepare_render_records();
            if let (Some(timing), Some(records_start)) = (&mut timing, records_start) {
                timing.records_ms = timing_elapsed_ms(records_start);
            }
            Some(records)
        } else {
            None
        };
        let records: &PreparedTexturedSectionRecords = match prepared_records {
            Some(records) => records,
            None => records_storage
                .as_deref()
                .expect("owned prepared records created for the direct render path"),
        };
        let cull_start = timing.as_ref().map(|_| timing_now());
        let (culling, cull_cache_lookup, cull_cache_hit) =
            if let Some(records_storage) = records_storage.as_ref() {
                let key = TexturedSectionMonoCullingCacheKey {
                    render_view,
                    section_occlusion_culling: options.section_occlusion_culling,
                    force_fullbright: options.force_fullbright,
                    topology: options.topology,
                };
                let mut cache = self.mono_culling_cache.borrow_mut();
                if let Some(cached) = cache.as_ref()
                    && cached.matches(key, records_storage)
                {
                    (Arc::clone(&cached.result), true, true)
                } else {
                    let result = {
                        let mut scratch = self.cull_scratch.borrow_mut();
                        Arc::new(cull_textured_sections(
                            records,
                            render_view,
                            options,
                            &mut scratch,
                        ))
                    };
                    *cache = Some(TexturedSectionMonoCullingCache {
                        key,
                        records: Arc::clone(records_storage),
                        result: Arc::clone(&result),
                    });
                    (result, true, false)
                }
            } else {
                let mut scratch = self.cull_scratch.borrow_mut();
                (
                    Arc::new(cull_textured_sections(
                        records,
                        render_view,
                        options,
                        &mut scratch,
                    )),
                    false,
                    false,
                )
            };
        if let Some(timing) = &mut timing {
            timing.cull_cache_lookup = cull_cache_lookup;
            timing.cull_cache_hit = cull_cache_hit;
        }
        if let (Some(timing), Some(cull_start)) = (&mut timing, cull_start) {
            timing.cull_ms = timing_elapsed_ms(cull_start);
        }
        let uniform_start = timing.as_ref().map(|_| timing_now());
        let uniform_offset = self.shared.renderer.uniforms.write_slot(
            queue,
            view_slot,
            &uniform_bytes(render_view, options, self.shared.renderer.color_format),
        );
        if let (Some(timing), Some(uniform_start)) = (&mut timing, uniform_start) {
            timing.uniform_write_ms = timing_elapsed_ms(uniform_start);
        }
        let mut translucent_sections = Vec::new();
        if phase.draws_translucent() {
            let translucent_collect_start = timing.as_ref().map(|_| timing_now());
            translucent_sections = self
                .sections
                .iter()
                .filter(|(key, mesh)| {
                    culling.drawn_keys.contains(key) && !mesh.translucent_index_range().is_empty()
                })
                .collect::<Vec<_>>();
            if let (Some(timing), Some(translucent_collect_start)) =
                (&mut timing, translucent_collect_start)
            {
                timing.translucent_collect_ms = timing_elapsed_ms(translucent_collect_start);
            }
            let translucent_sort_start = timing.as_ref().map(|_| timing_now());
            translucent_sections.sort_by(|(left_key, _), (right_key, _)| {
                compare_translucent_sections(**left_key, **right_key, render_view, options.topology)
            });
            if let (Some(timing), Some(translucent_sort_start)) =
                (&mut timing, translucent_sort_start)
            {
                timing.translucent_sort_ms = timing_elapsed_ms(translucent_sort_start);
            }
        }
        if let (Some(timing), Some(prepare_start)) = (&mut timing, prepare_start) {
            timing.prepare_ms = timing_elapsed_ms(prepare_start);
        }

        let encode_start = timing.as_ref().map(|_| timing_now());
        let indirect_batch = if self.section_arena.multi_draw_indirect {
            let mut batch = TerrainIndirectDrawBatch::default();
            if phase.draws_opaque() {
                for vertex_page in 0..self.section_arena.vertex_pages.len() {
                    let first = batch.begin_span();
                    for (key, mesh) in &self.sections {
                        if mesh.vertex_page == vertex_page && culling.drawn_keys.contains(key) {
                            batch.push(mesh, mesh.solid_index_range());
                        }
                    }
                    let solid = batch.finish_span(first);
                    let first = batch.begin_span();
                    for (key, mesh) in &self.sections {
                        if mesh.vertex_page == vertex_page && culling.drawn_keys.contains(key) {
                            batch.push(mesh, mesh.cutout_index_range());
                        }
                    }
                    let cutout = batch.finish_span(first);
                    if solid.count > 0 || cutout.count > 0 {
                        batch.pages.push(TerrainIndirectDrawPage {
                            vertex_page,
                            solid,
                            cutout,
                        });
                    }
                }
            }
            let offset = self.section_arena.write_indirect_args(
                queue,
                terrain_indirect_slot(view_slot, phase),
                &batch.args,
            );
            Some((batch, offset))
        } else {
            None
        };
        let direct_opaque_draw_calls = if indirect_batch.is_none() {
            let opaque = if phase.draws_opaque() {
                self.sections
                    .iter()
                    .filter(|(key, mesh)| {
                        culling.drawn_keys.contains(key) && !mesh.solid_index_range().is_empty()
                    })
                    .count()
                    + self
                        .sections
                        .iter()
                        .filter(|(key, mesh)| {
                            culling.drawn_keys.contains(key)
                                && !mesh.cutout_index_range().is_empty()
                        })
                        .count()
            } else {
                0
            };
            opaque
        } else {
            0
        };
        let direct_draw_calls = direct_opaque_draw_calls + translucent_sections.len();
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mclone_textured_section_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: target.color_load_op(),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: target.depth_load_op(),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: target.gpu_timestamp_writes(phase.gpu_pass_id()),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.shared.renderer.bind_group, &[uniform_offset]);
            pass.set_bind_group(1, &self.shared.atlas.bind_group, &[]);
            bind_textured_section_arena(&mut pass, &self.section_arena);
            if let Some((batch, offset)) = &indirect_batch {
                for page in &batch.pages {
                    bind_textured_section_vertex_page(
                        &mut pass,
                        &self.section_arena,
                        page.vertex_page,
                    );
                    if page.solid.count > 0 {
                        pass.set_pipeline(&self.shared.renderer.solid_pipeline);
                        pass.multi_draw_indexed_indirect(
                            &self.section_arena.indirect_buffer,
                            page.solid.byte_offset(*offset),
                            page.solid.count,
                        );
                    }
                    if page.cutout.count > 0 {
                        pass.set_pipeline(&self.shared.renderer.cutout_pipeline);
                        pass.multi_draw_indexed_indirect(
                            &self.section_arena.indirect_buffer,
                            page.cutout.byte_offset(*offset),
                            page.cutout.count,
                        );
                    }
                }
                if phase.draws_translucent() {
                    pass.set_pipeline(&self.shared.renderer.translucent_pipeline);
                    for (_, mesh) in &translucent_sections {
                        draw_textured_section_mesh_range(
                            &mut pass,
                            &self.section_arena,
                            mesh,
                            mesh.translucent_index_range(),
                        );
                    }
                }
            } else {
                if phase.draws_opaque() {
                    pass.set_pipeline(&self.shared.renderer.solid_pipeline);
                    for (key, mesh) in &self.sections {
                        if !culling.drawn_keys.contains(key) {
                            continue;
                        }
                        draw_textured_section_mesh_range(
                            &mut pass,
                            &self.section_arena,
                            mesh,
                            mesh.solid_index_range(),
                        );
                    }
                    pass.set_pipeline(&self.shared.renderer.cutout_pipeline);
                    for (key, mesh) in &self.sections {
                        if !culling.drawn_keys.contains(key) {
                            continue;
                        }
                        draw_textured_section_mesh_range(
                            &mut pass,
                            &self.section_arena,
                            mesh,
                            mesh.cutout_index_range(),
                        );
                    }
                }
                if phase.draws_translucent() {
                    pass.set_pipeline(&self.shared.renderer.translucent_pipeline);
                    for (_, mesh) in &translucent_sections {
                        draw_textured_section_mesh_range(
                            &mut pass,
                            &self.section_arena,
                            mesh,
                            mesh.translucent_index_range(),
                        );
                    }
                }
            }
        }
        if let Some(timing) = &mut timing {
            timing.direct_draw_calls = direct_draw_calls;
            if let Some((batch, _)) = &indirect_batch {
                timing.multi_draw_calls = batch
                    .pages
                    .iter()
                    .map(|page| {
                        usize::from(page.solid.count > 0) + usize::from(page.cutout.count > 0)
                    })
                    .sum();
                timing.indirect_draw_count = batch.args.len();
            }
            timing.arena_vertex_used_bytes = self.section_arena.vertex_used_bytes();
            timing.arena_vertex_capacity_bytes = self.section_arena.vertex_capacity_bytes();
            timing.arena_index_used_bytes = self.section_arena.index_used_bytes();
            timing.arena_index_capacity_bytes = self.section_arena.index_capacity_bytes();
        }
        if let (Some(timing), Some(encode_start)) = (&mut timing, encode_start) {
            timing.encode_ms = timing_elapsed_ms(encode_start);
        }
        Ok(culling.stats)
    }

    fn prepare_stereo_draw_inner(
        &self,
        records: &PreparedTexturedSectionRecords,
        render_views: [ChunkRenderView; 2],
        options: [TexturedSectionRenderOptions; 2],
        mut timing: Option<&mut TexturedSectionRenderTiming>,
    ) -> PreparedTexturedSectionStereoDraw {
        let prepare_start = timing.as_ref().map(|_| timing_now());
        let cull_start = timing.as_ref().map(|_| timing_now());
        let culling = {
            let mut scratch = self.cull_scratch.borrow_mut();
            cull_textured_sections_stereo_union(records, render_views, options, &mut scratch)
        };
        if let (Some(timing), Some(cull_start)) = (&mut timing, cull_start) {
            timing.cull_ms = timing_elapsed_ms(cull_start);
        }
        let translucent_collect_start = timing.as_ref().map(|_| timing_now());
        let mut translucent_keys = self
            .sections
            .iter()
            .filter(|(key, mesh)| {
                culling.drawn_keys.contains(key) && !mesh.translucent_index_range().is_empty()
            })
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        if let (Some(timing), Some(translucent_collect_start)) =
            (&mut timing, translucent_collect_start)
        {
            timing.translucent_collect_ms = timing_elapsed_ms(translucent_collect_start);
        }
        let translucent_sort_start = timing.as_ref().map(|_| timing_now());
        let sort_view = stereo_translucent_sort_view(render_views);
        translucent_keys.sort_by(|left, right| {
            compare_translucent_sections(*left, *right, sort_view, options[0].topology)
        });
        if let (Some(timing), Some(translucent_sort_start)) = (&mut timing, translucent_sort_start)
        {
            timing.translucent_sort_ms = timing_elapsed_ms(translucent_sort_start);
        }
        if let (Some(timing), Some(prepare_start)) = (&mut timing, prepare_start) {
            timing.prepare_ms = timing_elapsed_ms(prepare_start);
        }
        let union_stats = stereo_union_stats_for_options(culling.stats, options);
        PreparedTexturedSectionStereoDraw {
            union_stats,
            eye_stats: culling.eye_stats,
            drawn_keys: culling.drawn_keys,
            draw_masks: culling.draw_masks,
            translucent_keys,
        }
    }

    fn render_prepared_stereo_draw_inner(
        &self,
        prepared_draw: &PreparedTexturedSectionStereoDraw,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: ChunkRenderTarget<'_>,
        render_view: ChunkRenderView,
        options: TexturedSectionRenderOptions,
        view_slot: PerViewSlot,
        mut timing: Option<&mut TexturedSectionRenderTiming>,
        phase: TexturedSectionRenderPhase,
    ) -> Result<TexturedSectionRenderStats> {
        let prepare_start = timing.as_ref().map(|_| timing_now());
        let uniform_start = timing.as_ref().map(|_| timing_now());
        let uniform_offset = self.shared.renderer.uniforms.write_slot(
            queue,
            view_slot,
            &uniform_bytes(render_view, options, self.shared.renderer.color_format),
        );
        if let (Some(timing), Some(uniform_start)) = (&mut timing, uniform_start) {
            timing.uniform_write_ms = timing_elapsed_ms(uniform_start);
        }
        if let (Some(timing), Some(prepare_start)) = (&mut timing, prepare_start) {
            timing.prepare_ms = timing_elapsed_ms(prepare_start);
        }

        let encode_start = timing.as_ref().map(|_| timing_now());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mclone_textured_section_stereo_prepared_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: target.color_load_op(),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: target.depth_load_op(),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: target.gpu_timestamp_writes(phase.gpu_pass_id()),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.shared.renderer.bind_group, &[uniform_offset]);
            pass.set_bind_group(1, &self.shared.atlas.bind_group, &[]);
            bind_textured_section_arena(&mut pass, &self.section_arena);
            if phase.draws_opaque() {
                pass.set_pipeline(&self.shared.renderer.solid_pipeline);
                for (key, mesh) in &self.sections {
                    if !prepared_draw.draws_in_eye(*key, stereo_eye_for_slot(view_slot)) {
                        continue;
                    }
                    draw_textured_section_mesh_range(
                        &mut pass,
                        &self.section_arena,
                        mesh,
                        mesh.solid_index_range(),
                    );
                }
                pass.set_pipeline(&self.shared.renderer.cutout_pipeline);
                for (key, mesh) in &self.sections {
                    if !prepared_draw.draws_in_eye(*key, stereo_eye_for_slot(view_slot)) {
                        continue;
                    }
                    draw_textured_section_mesh_range(
                        &mut pass,
                        &self.section_arena,
                        mesh,
                        mesh.cutout_index_range(),
                    );
                }
            }
            if phase.draws_translucent() {
                pass.set_pipeline(&self.shared.renderer.translucent_pipeline);
                for key in &prepared_draw.translucent_keys {
                    if !prepared_draw.draws_in_eye(*key, stereo_eye_for_slot(view_slot)) {
                        continue;
                    }
                    if let Some(mesh) = self.sections.get(key) {
                        draw_textured_section_mesh_range(
                            &mut pass,
                            &self.section_arena,
                            mesh,
                            mesh.translucent_index_range(),
                        );
                    }
                }
            }
        }
        if let (Some(timing), Some(encode_start)) = (&mut timing, encode_start) {
            timing.encode_ms = timing_elapsed_ms(encode_start);
        }
        Ok(prepared_draw.stats_for_eye(stereo_eye_for_slot(view_slot)))
    }

    fn build_prepared_records(&self) -> PreparedTexturedSectionRecords {
        let mut records = BTreeMap::new();
        let mut loaded_section_count = 0usize;
        let mut loaded_index_count = 0u32;
        for (key, visibility) in &self.visibility_sections {
            let mesh = self.sections.get(key);
            let index_count = mesh.map_or(0, GpuTexturedSectionMesh::index_count);
            let drawable = mesh.is_some();
            if drawable {
                loaded_section_count += 1;
                loaded_index_count += index_count;
            }
            records.insert(
                *key,
                TexturedSectionCullingRecord {
                    index_count,
                    visibility: *visibility,
                    drawable,
                    traversal_ready: self.traversal_ready_sections.contains(key),
                },
            );
        }
        PreparedTexturedSectionRecords {
            records,
            loaded_section_count,
            loaded_index_count,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
type RenderTimingSample = Instant;

#[cfg(target_arch = "wasm32")]
type RenderTimingSample = f64;

#[cfg(not(target_arch = "wasm32"))]
fn timing_now() -> RenderTimingSample {
    Instant::now()
}

#[cfg(target_arch = "wasm32")]
fn timing_now() -> RenderTimingSample {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn timing_elapsed_ms(start: RenderTimingSample) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

#[cfg(target_arch = "wasm32")]
fn timing_elapsed_ms(start: RenderTimingSample) -> f64 {
    (js_sys::Date::now() - start).max(0.0)
}

fn stereo_union_stats_for_options(
    stats: TexturedSectionRenderStats,
    options: [TexturedSectionRenderOptions; 2],
) -> [TexturedSectionRenderStats; 2] {
    let mut stats = [stats; 2];
    stats[0].section_occlusion_culling = options[0].section_occlusion_culling;
    stats[0].force_fullbright = options[0].force_fullbright;
    stats[1].section_occlusion_culling = options[1].section_occlusion_culling;
    stats[1].force_fullbright = options[1].force_fullbright;
    stats
}

fn stereo_center_position(render_views: [ChunkRenderView; 2]) -> Vec3 {
    (render_views[0].camera_position + render_views[1].camera_position) * 0.5
}

fn stereo_translucent_sort_view(render_views: [ChunkRenderView; 2]) -> ChunkRenderView {
    let mut sort_view = render_views[0];
    sort_view.camera_position = stereo_center_position(render_views);
    let forward =
        (render_views[0].camera_forward + render_views[1].camera_forward).normalize_or_zero();
    if forward.length_squared() > 0.0 {
        sort_view.camera_forward = forward;
    }
    sort_view
}

fn compare_translucent_sections(
    left: RenderSectionKey,
    right: RenderSectionKey,
    render_view: ChunkRenderView,
    topology: HorizontalTopology,
) -> Ordering {
    let left_depth = section_depth_along_view(left, render_view, topology);
    let right_depth = section_depth_along_view(right, render_view, topology);
    right_depth
        .partial_cmp(&left_depth)
        .unwrap_or(Ordering::Equal)
        .then_with(|| right.cmp(&left))
}

fn compare_placed_translucent_sections(
    left: RenderSectionKey,
    right: RenderSectionKey,
    physical_render_view: ChunkRenderView,
    placement: WorldPlacement,
) -> Ordering {
    let left_depth = placed_section_depth_along_view(left, physical_render_view, placement);
    let right_depth = placed_section_depth_along_view(right, physical_render_view, placement);
    right_depth
        .partial_cmp(&left_depth)
        .unwrap_or(Ordering::Equal)
        .then_with(|| right.cmp(&left))
}

fn section_depth_along_view(
    key: RenderSectionKey,
    render_view: ChunkRenderView,
    topology: HorizontalTopology,
) -> f32 {
    let center = render_section_center_in(key, render_view.camera_position, topology);
    (center - render_view.camera_position).dot(render_view.camera_forward)
}

fn placed_section_depth_along_view(
    key: RenderSectionKey,
    render_view: ChunkRenderView,
    placement: WorldPlacement,
) -> f32 {
    let center = textured_section_translucent_record(key, placement).composition_center;
    (center - render_view.camera_position).dot(render_view.camera_forward)
}

fn textured_section_translucent_record(
    key: RenderSectionKey,
    placement: WorldPlacement,
) -> TexturedSectionTranslucentRecord {
    let center = section_center(key);
    let center = placement.source_to_composition(Vec3d::new(
        f64::from(center.x),
        f64::from(center.y),
        f64::from(center.z),
    ));
    TexturedSectionTranslucentRecord {
        key,
        composition_center: Vec3::new(center.x as f32, center.y as f32, center.z as f32),
    }
}

fn section_center(key: RenderSectionKey) -> Vec3 {
    Vec3::new(
        chunk_min_block_coord(key.chunk_x) as f32 + MESH_CHUNK_WIDTH as f32 * 0.5,
        key.min_y() as f32 + RENDER_SECTION_HEIGHT as f32 * 0.5,
        chunk_min_block_coord(key.chunk_z) as f32 + MESH_CHUNK_WIDTH as f32 * 0.5,
    )
}

fn vertex_bytes(mesh: &VisibleChunkMesh) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(mesh.vertices.len() * VERTEX_FLOAT_COUNT * 4);
    for vertex in &mesh.vertices {
        for value in vertex.position.into_iter().chain(vertex.color) {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
    bytes
}

fn textured_vertex_bytes(mesh: &TexturedVisibleChunkMesh) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(mesh.vertices.len() * TEXTURED_VERTEX_BYTE_SIZE as usize);
    for vertex in &mesh.vertices {
        for value in vertex
            .position
            .into_iter()
            .chain(vertex.uv)
            .chain(vertex.color)
        {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        bytes.extend_from_slice(&vertex.packed_light.to_ne_bytes());
    }
    bytes
}

fn index_bytes(indices: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(indices));
    for index in indices {
        bytes.extend_from_slice(&index.to_ne_bytes());
    }
    bytes
}

fn matrix_bytes(matrix: [[f32; 4]; 4]) -> [u8; 64] {
    let mut bytes = [0; 64];
    for (index, value) in matrix.into_iter().flatten().enumerate() {
        let start = index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn uniform_bytes(
    render_view: ChunkRenderView,
    options: TexturedSectionRenderOptions,
    color_format: wgpu::TextureFormat,
) -> [u8; 128] {
    let mut bytes = [0; UNIFORM_BYTE_LEN];
    bytes[..64].copy_from_slice(&matrix_bytes(render_view.uniform_matrix()));
    let color_transform = RenderConfig::for_color_target(options.color_profile, color_format)
        .target_color_transform()
        .shader_code();
    let render_options = [
        if options.force_fullbright {
            1.0_f32
        } else {
            0.0
        },
        options.sky_darken.clamp(0.0, 1.0),
        if options.fog.enabled { 1.0 } else { 0.0 },
        color_transform,
    ];
    for (index, value) in render_options.into_iter().enumerate() {
        let start = 64 + index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    let camera_position = render_view.camera_position.to_array();
    for (index, value) in [
        camera_position[0],
        camera_position[1],
        camera_position[2],
        0.0,
    ]
    .into_iter()
    .enumerate()
    {
        let start = 80 + index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    for (index, value) in [
        options.fog.color[0],
        options.fog.color[1],
        options.fog.color[2],
        1.0,
    ]
    .into_iter()
    .enumerate()
    {
        let start = 96 + index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    let topology_period_blocks = [
        options
            .topology
            .x
            .period_chunks()
            .map_or(0.0, |period| period as f32 * MESH_CHUNK_WIDTH as f32),
        options
            .topology
            .z
            .period_chunks()
            .map_or(0.0, |period| period as f32 * MESH_CHUNK_WIDTH as f32),
    ];
    for (index, value) in [
        options.fog.start,
        options.fog.end,
        topology_period_blocks[0],
        topology_period_blocks[1],
    ]
    .into_iter()
    .enumerate()
    {
        let start = 112 + index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn multiview_uniform_bytes(
    render_views: [ChunkRenderView; 2],
    options: [TexturedSectionRenderOptions; 2],
    color_format: wgpu::TextureFormat,
) -> [u8; MULTIVIEW_UNIFORM_BYTE_LEN] {
    let mut bytes = [0u8; MULTIVIEW_UNIFORM_BYTE_LEN];
    bytes[..UNIFORM_BYTE_LEN].copy_from_slice(&uniform_bytes(
        render_views[0],
        options[0],
        color_format,
    ));
    bytes[UNIFORM_BYTE_LEN..].copy_from_slice(&uniform_bytes(
        render_views[1],
        options[1],
        color_format,
    ));
    bytes
}

fn placed_uniform_bytes(
    render_view: ChunkRenderView,
    options: TexturedSectionRenderOptions,
    placement: WorldPlacement,
    color_format: wgpu::TextureFormat,
) -> [u8; PLACED_UNIFORM_BYTE_LEN] {
    let mut bytes = [0; PLACED_UNIFORM_BYTE_LEN];
    bytes[..UNIFORM_BYTE_LEN].copy_from_slice(&uniform_bytes(render_view, options, color_format));
    let (source_anchor_scale, composition_anchor) = placement.shader_values();
    for (index, value) in source_anchor_scale.into_iter().enumerate() {
        let start = UNIFORM_BYTE_LEN + index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    for (index, value) in composition_anchor.into_iter().enumerate() {
        let start = UNIFORM_BYTE_LEN + 16 + index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn placed_multiview_uniform_bytes(
    render_views: [ChunkRenderView; 2],
    options: [TexturedSectionRenderOptions; 2],
    placement: WorldPlacement,
    color_format: wgpu::TextureFormat,
) -> [u8; PLACED_MULTIVIEW_UNIFORM_BYTE_LEN] {
    let mut bytes = [0u8; PLACED_MULTIVIEW_UNIFORM_BYTE_LEN];
    bytes[..PLACED_UNIFORM_BYTE_LEN].copy_from_slice(&placed_uniform_bytes(
        render_views[0],
        options[0],
        placement,
        color_format,
    ));
    bytes[PLACED_UNIFORM_BYTE_LEN..].copy_from_slice(&placed_uniform_bytes(
        render_views[1],
        options[1],
        placement,
        color_format,
    ));
    bytes
}

fn clipped_placed_uniform_bytes(
    render_view: ChunkRenderView,
    options: TexturedSectionRenderOptions,
    context: WorldCompositionContext,
    color_format: wgpu::TextureFormat,
) -> [u8; CLIPPED_PLACED_UNIFORM_BYTE_LEN] {
    let mut bytes = [0; CLIPPED_PLACED_UNIFORM_BYTE_LEN];
    bytes[..PLACED_UNIFORM_BYTE_LEN].copy_from_slice(&placed_uniform_bytes(
        render_view,
        options,
        context.placement(),
        color_format,
    ));
    let CompositionClip::HalfSpace(half_space) = context.clip() else {
        panic!("clipped placed uniforms require a half-space context");
    };
    for (index, value) in [
        half_space.normal().x,
        half_space.normal().y,
        half_space.normal().z,
        half_space.offset(),
    ]
    .into_iter()
    .enumerate()
    {
        let start = PLACED_UNIFORM_BYTE_LEN + index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn clipped_placed_multiview_uniform_bytes(
    render_views: [ChunkRenderView; 2],
    options: [TexturedSectionRenderOptions; 2],
    context: WorldCompositionContext,
    color_format: wgpu::TextureFormat,
) -> [u8; CLIPPED_PLACED_MULTIVIEW_UNIFORM_BYTE_LEN] {
    let mut bytes = [0u8; CLIPPED_PLACED_MULTIVIEW_UNIFORM_BYTE_LEN];
    bytes[..CLIPPED_PLACED_UNIFORM_BYTE_LEN].copy_from_slice(&clipped_placed_uniform_bytes(
        render_views[0],
        options[0],
        context,
        color_format,
    ));
    bytes[CLIPPED_PLACED_UNIFORM_BYTE_LEN..].copy_from_slice(&clipped_placed_uniform_bytes(
        render_views[1],
        options[1],
        context,
        color_format,
    ));
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::Vec3d;
    use mclone_mesh::{
        ChunkMeshInput, TexturedChunkVertex, TexturedVisibleChunkMesh, build_visible_chunk_mesh,
    };

    #[test]
    fn terrain_overview_sampling_does_not_change_vanilla_defaults() {
        assert_eq!(
            chunk_texture_filter_modes(ChunkTextureSampling::Vanilla),
            (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest)
        );
        assert_eq!(
            chunk_texture_filter_modes(ChunkTextureSampling::TerrainOverview),
            (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear)
        );
    }

    #[test]
    fn camera_matrix_serializes_to_uniform_size() {
        let render_view = ChunkCamera::overview_for_chunk(0, 0).render_view(640, 480);
        let matrix = render_view.uniform_matrix();
        assert_eq!(
            uniform_bytes(
                render_view,
                TexturedSectionRenderOptions::default(),
                wgpu::TextureFormat::Rgba8Unorm,
            )
            .len() as wgpu::BufferAddress,
            UNIFORM_BYTE_SIZE
        );
        assert!(matrix.into_iter().flatten().all(f32::is_finite));
        assert!(render_view.view_projection.is_finite());
    }

    #[test]
    fn terrain_uniform_serializes_periods_without_growing_the_direct_path() {
        let render_view = ChunkCamera::overview_for_chunk(0, 0).render_view(640, 480);
        let plane = uniform_bytes(
            render_view,
            TexturedSectionRenderOptions::default(),
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let cylinder = uniform_bytes(
            render_view,
            TexturedSectionRenderOptions::default()
                .with_topology(HorizontalTopology::cylinder_x(0, 32)),
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let read_f32 = |bytes: &[u8], offset: usize| {
            f32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap())
        };

        assert_eq!(read_f32(&plane, 120), 0.0);
        assert_eq!(read_f32(&plane, 124), 0.0);
        assert_eq!(read_f32(&cylinder, 120), 512.0);
        assert_eq!(read_f32(&cylinder, 124), 0.0);
    }

    #[test]
    fn direct_terrain_uniforms_and_shaders_remain_unplaced() {
        assert_eq!(UNIFORM_BYTE_LEN, 128);
        assert_eq!(MULTIVIEW_UNIFORM_BYTE_LEN, 256);

        let mono = include_str!("shaders/chunk_textured.wgsl");
        assert!(mono.contains("let world_position = observer_local_position(input.position);"));
        assert!(mono.contains("output.world_position = world_position;"));

        let multiview = include_str!("shaders/chunk_textured_multiview.wgsl");
        assert!(
            multiview.contains(
                "let world_position = observer_local_position(input.position, uniforms);"
            )
        );
        assert!(multiview.contains("output.world_position = world_position;"));

        for source in [mono, multiview] {
            for placed_uniform in [
                "source_anchor",
                "composition_anchor",
                "uniform_scale",
                "model_matrix",
            ] {
                assert!(
                    !source.contains(placed_uniform),
                    "ordinary terrain shader unexpectedly contains `{placed_uniform}`"
                );
            }
        }
    }

    #[test]
    fn placed_uniforms_rebase_source_and_keep_distinct_eye_facts() {
        let left = ChunkCamera {
            eye: [-0.032, 8.0, 20.0],
            target: [-0.032, 8.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 65.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 200.0,
        }
        .render_view(640, 640);
        let right = ChunkCamera {
            eye: [0.032, 8.0, 20.0],
            target: [0.032, 8.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 65.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 200.0,
        }
        .render_view(640, 640);
        let placement = WorldPlacement::new(
            Vec3d::new(1_000_008.0, 64.0, -999_992.0),
            Vec3d::new(0.0, 2.0, 0.0),
            1.0 / 16.0,
        )
        .unwrap();
        let options = TexturedSectionRenderOptions::default();
        let left_bytes =
            placed_uniform_bytes(left, options, placement, wgpu::TextureFormat::Rgba8Unorm);
        let right_bytes =
            placed_uniform_bytes(right, options, placement, wgpu::TextureFormat::Rgba8Unorm);

        assert_eq!(left_bytes.len(), PLACED_UNIFORM_BYTE_LEN);
        assert_ne!(
            &left_bytes[..UNIFORM_BYTE_LEN],
            &right_bytes[..UNIFORM_BYTE_LEN]
        );
        assert_eq!(
            &left_bytes[UNIFORM_BYTE_LEN..],
            &right_bytes[UNIFORM_BYTE_LEN..]
        );
        assert_eq!(
            placed_multiview_uniform_bytes(
                [left, right],
                [options; 2],
                placement,
                wgpu::TextureFormat::Rgba8Unorm,
            )
            .len(),
            PLACED_MULTIVIEW_UNIFORM_BYTE_LEN
        );

        let mono = include_str!("shaders/chunk_textured_placed.wgsl");
        let multiview = include_str!("shaders/chunk_textured_placed_multiview.wgsl");
        for source in [mono, multiview] {
            assert!(source.contains("input.position - uniforms.source_anchor_scale.xyz"));
            assert!(source.contains("output.composition_position = composition_position;"));
            assert!(source.contains("uniforms.camera_position.xyz"));
            assert!(!source.contains("clip_plane"));
        }
    }

    #[test]
    fn clipped_placed_uniforms_and_shaders_are_separate_from_unbounded_topology() {
        let view = ChunkCamera::overview_for_chunk(0, 0).render_view(640, 480);
        let options = TexturedSectionRenderOptions::default();
        let placement = WorldPlacement::identity();
        let half_space =
            crate::placement::CompositionHalfSpace::new(Vec3::new(2.0, 0.0, 0.0), -6.0).unwrap();
        let context =
            WorldCompositionContext::new(placement, None, CompositionClip::HalfSpace(half_space));
        let ordinary =
            placed_uniform_bytes(view, options, placement, wgpu::TextureFormat::Rgba8Unorm);
        let clipped =
            clipped_placed_uniform_bytes(view, options, context, wgpu::TextureFormat::Rgba8Unorm);

        assert_eq!(&clipped[..PLACED_UNIFORM_BYTE_LEN], &ordinary);
        assert_eq!(clipped.len(), CLIPPED_PLACED_UNIFORM_BYTE_LEN);
        assert_eq!(
            (0..4)
                .map(|index| {
                    let start = PLACED_UNIFORM_BYTE_LEN + index * 4;
                    f32::from_ne_bytes(clipped[start..start + 4].try_into().unwrap())
                })
                .collect::<Vec<_>>(),
            vec![1.0, 0.0, 0.0, -3.0],
        );
        assert_eq!(
            clipped_placed_multiview_uniform_bytes(
                [view; 2],
                [options; 2],
                context,
                wgpu::TextureFormat::Rgba8Unorm,
            )
            .len(),
            CLIPPED_PLACED_MULTIVIEW_UNIFORM_BYTE_LEN,
        );

        let mono = clipped_placed_shader_source();
        let multiview = clipped_placed_multiview_shader_source();
        for source in [&mono, &multiview] {
            assert!(source.contains("clip_plane: vec4<f32>"));
            assert_eq!(source.matches("uniforms.clip_plane").count(), 4);
            assert!(source.contains("< 0.0)"));
        }
    }

    #[test]
    fn identity_placed_uniform_retains_direct_view_bytes() {
        let view = ChunkCamera::overview_for_chunk(0, 0).render_view(640, 480);
        let options = TexturedSectionRenderOptions::default();
        let ordinary = uniform_bytes(view, options, wgpu::TextureFormat::Rgba8Unorm);
        let placed = placed_uniform_bytes(
            view,
            options,
            WorldPlacement::identity(),
            wgpu::TextureFormat::Rgba8Unorm,
        );

        assert_eq!(&placed[..UNIFORM_BYTE_LEN], &ordinary);
        assert_eq!(
            f32::from_ne_bytes(placed[140..144].try_into().unwrap()),
            1.0
        );
        assert!(placed[128..140].iter().all(|byte| *byte == 0));
        assert!(placed[144..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn translucent_record_center_uses_physical_world_placement() {
        let placement = WorldPlacement::new(
            Vec3d::new(8.0, 65.0, 8.0),
            Vec3d::new(20.0, 70.0, -4.0),
            0.125,
        )
        .unwrap();
        let record = textured_section_translucent_record(RenderSectionKey::new(0, 4, 0), placement);

        assert_eq!(record.key, RenderSectionKey::new(0, 4, 0));
        assert_eq!(record.composition_center, Vec3::new(20.0, 70.875, -4.0));
    }

    #[test]
    fn placed_local_view_culls_in_source_coordinates_at_miniature_scale() {
        let physical = ChunkCamera {
            eye: [0.0, 8.0, 20.0],
            target: [0.0, 8.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 60.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 200.0,
        }
        .render_view(800, 600);
        let placement =
            WorldPlacement::new(Vec3d::new(1_000.0, 0.0, 1_000.0), Vec3d::ZERO, 0.5).unwrap();
        let source_view = placement.source_render_view(physical);
        let context = WorldCompositionContext::unbounded(placement, None);
        let frustum = PlacedClipFrustum::new(physical, context);

        assert_eq!(
            source_view.camera_position,
            Vec3::new(1_000.0, 16.0, 1_040.0)
        );
        assert!(frustum.is_render_section_visible(RenderSectionKey::new(62, 0, 62)));
        assert!(!frustum.is_render_section_visible(RenderSectionKey::new(62, 0, 80)));
    }

    #[test]
    fn placed_frustum_uses_shader_rebase_order_at_far_source_coordinates() {
        let physical = ChunkCamera {
            eye: [0.0, 8.0, 20.0],
            target: [0.0, 8.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 60.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 200.0,
        }
        .render_view(800, 600);
        let placement = WorldPlacement::new(
            Vec3d::new(30_000_000.0, 0.0, -30_000_000.0),
            Vec3d::ZERO,
            1.0 / 16.0,
        )
        .unwrap();
        let context = WorldCompositionContext::unbounded(placement, None);
        let frustum = PlacedClipFrustum::new(physical, context);
        let source = Vec3::new(30_000_016.0, 16.0, -30_000_016.0);
        let composition = frustum.source_to_composition(source);

        assert_eq!(composition, Vec3::new(1.0, 1.0, -1.0));
        let expected_clip = physical.view_projection * composition.extend(1.0);
        let recomputed_clip =
            physical.view_projection * frustum.source_to_composition(source).extend(1.0);
        assert_eq!(recomputed_clip, expected_clip);
    }

    #[test]
    fn placed_visibility_rejects_only_sections_wholly_outside_the_half_space() {
        let physical = ChunkCamera {
            eye: [0.0, 8.0, 40.0],
            target: [0.0, 8.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 90.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 200.0,
        }
        .render_view(800, 600);
        let clip = CompositionClip::HalfSpace(
            crate::placement::CompositionHalfSpace::new(Vec3::X, 0.0).unwrap(),
        );
        let context = WorldCompositionContext::new(WorldPlacement::identity(), None, clip);
        let frustum = PlacedClipFrustum::new(physical, context);

        assert!(frustum.is_render_section_visible(RenderSectionKey::new(0, 0, 0)));
        assert!(frustum.is_render_section_visible(RenderSectionKey::new(-1, 0, 0)));
        assert!(!frustum.is_render_section_visible(RenderSectionKey::new(-2, 0, 0)));
    }

    #[test]
    fn embedded_record_filter_is_bounded_and_stably_ordered() {
        let sections = [
            fake_section(
                RenderSectionKey::new(-2, 3, -1),
                6,
                VisibilitySet::all_visible(),
            ),
            fake_section(
                RenderSectionKey::new(-1, 4, -1),
                12,
                VisibilitySet::all_visible(),
            ),
            fake_section(
                RenderSectionKey::new(-1, 2, 0),
                18,
                VisibilitySet::all_visible(),
            ),
            fake_section(
                RenderSectionKey::new(0, 3, -1),
                24,
                VisibilitySet::all_visible(),
            ),
        ];
        let records = prepared_records_for_sections(&sections);
        let region =
            crate::placement::EmbeddedChunkRegion::new(mclone_core::ChunkPos::new(-1, -1), 1, 3, 4)
                .unwrap();
        let bounded = records.for_source_bounds(region.source_bounds());

        assert_eq!(
            bounded.section_keys().collect::<Vec<_>>(),
            vec![
                RenderSectionKey::new(-2, 3, -1),
                RenderSectionKey::new(-1, 4, -1),
                RenderSectionKey::new(0, 3, -1),
            ]
        );
        assert_eq!(bounded.loaded_section_count, 3);
        assert_eq!(bounded.loaded_index_count, 42);
    }

    #[test]
    fn perspective_render_pose_builds_finite_normal_view() {
        let orientation = Quat::IDENTITY;
        let render_view = test_perspective_pose(Vec3::new(0.0, 64.0, 0.0), orientation)
            .render_view(1280, 720)
            .expect("normal render pose should build");

        assert_finite_render_view(render_view);
        assert_vec3_near(render_view.camera_forward, Vec3::NEG_Z, 1.0e-6);
        assert_vec3_near(render_view.camera_right, Vec3::X, 1.0e-6);
        assert_vec3_near(render_view.camera_up, Vec3::Y, 1.0e-6);
    }

    #[test]
    fn perspective_render_pose_builds_finite_upward_vertical_view() {
        let orientation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let render_view = test_perspective_pose(Vec3::new(0.0, 64.0, 0.0), orientation)
            .render_view(1280, 720)
            .expect("upward vertical render pose should build");

        assert_finite_render_view(render_view);
        assert_vec3_near(render_view.camera_forward, Vec3::Y, 1.0e-6);
        assert_basis_is_orthonormal(render_view);
    }

    #[test]
    fn perspective_render_pose_builds_finite_downward_vertical_view() {
        let orientation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        let render_view = test_perspective_pose(Vec3::new(0.0, 64.0, 0.0), orientation)
            .render_view(1280, 720)
            .expect("downward vertical render pose should build");

        assert_finite_render_view(render_view);
        assert_vec3_near(render_view.camera_forward, Vec3::NEG_Y, 1.0e-6);
        assert_basis_is_orthonormal(render_view);
    }

    #[test]
    fn perspective_render_pose_keeps_third_person_vertical_view_finite() {
        let player_eye = Vec3::new(8.0, 70.0, 8.0);
        let orientation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        let forward = orientation * Vec3::NEG_Z;
        let render_eye = player_eye - forward * 4.0;
        let render_view = test_perspective_pose(render_eye, orientation)
            .render_view(1280, 720)
            .expect("third-person vertical render pose should build");

        assert_finite_render_view(render_view);
        assert_vec3_near(
            render_view.camera_position,
            Vec3::new(8.0, 74.0, 8.0),
            1.0e-5,
        );
        assert_vec3_near(render_view.camera_forward, Vec3::NEG_Y, 1.0e-6);
        assert_basis_is_orthonormal(render_view);
    }

    #[test]
    fn perspective_render_pose_rejects_invalid_orientation() {
        let pose = test_perspective_pose(
            Vec3::new(0.0, 64.0, 0.0),
            Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
        );

        assert!(pose.render_view(1280, 720).is_err());
    }

    #[test]
    fn multiview_uniform_serializes_distinct_left_right_views() {
        let left = ChunkCamera {
            eye: [0.0, 70.0, -4.0],
            target: [0.0, 70.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 70.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 700.0,
        }
        .render_view(1280, 1024);
        let right = ChunkCamera {
            eye: [0.063, 70.0, -4.0],
            target: [0.063, 70.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 72.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 700.0,
        }
        .render_view(1280, 1024);
        let bytes = multiview_uniform_bytes(
            [left, right],
            [
                TexturedSectionRenderOptions::default(),
                TexturedSectionRenderOptions {
                    force_fullbright: true,
                    ..TexturedSectionRenderOptions::default()
                },
            ],
            wgpu::TextureFormat::Rgba8Unorm,
        );

        assert_eq!(bytes.len(), MULTIVIEW_UNIFORM_BYTE_LEN);
        assert_ne!(&bytes[..UNIFORM_BYTE_LEN], &bytes[UNIFORM_BYTE_LEN..]);
        assert_eq!(
            &bytes[..UNIFORM_BYTE_LEN],
            &uniform_bytes(
                left,
                TexturedSectionRenderOptions::default(),
                wgpu::TextureFormat::Rgba8Unorm,
            )
        );
        assert_eq!(
            &bytes[UNIFORM_BYTE_LEN..],
            &uniform_bytes(
                right,
                TexturedSectionRenderOptions {
                    force_fullbright: true,
                    ..TexturedSectionRenderOptions::default()
                },
                wgpu::TextureFormat::Rgba8Unorm,
            )
        );
    }

    #[test]
    fn sky_view_projection_drops_camera_translation() {
        let camera = ChunkCamera {
            eye: [12.0, 72.0, -30.0],
            target: [18.0, 64.0, 4.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 64.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 700.0,
        };
        let mut shifted = camera;
        let offset = Vec3::new(128.0, -11.0, 47.0);
        shifted.eye = (Vec3::from_array(shifted.eye) + offset).to_array();
        shifted.target = (Vec3::from_array(shifted.target) + offset).to_array();

        let base = camera.render_view(1280, 720);
        let shifted = shifted.render_view(1280, 720);

        assert!(
            !mat4_near(base.view_projection, shifted.view_projection, 0.0001),
            "regular view projection should retain camera translation"
        );
        assert_mat4_near(
            base.sky_view_projection(),
            shifted.sky_view_projection(),
            0.0001,
        );
    }

    #[test]
    fn sky_view_projection_is_finite_for_vertical_render_pose() {
        for orientation in [
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        ] {
            let render_view = test_perspective_pose(Vec3::new(0.0, 64.0, 0.0), orientation)
                .render_view(1280, 720)
                .expect("vertical render pose should build");
            assert_finite_render_view(render_view);
            assert!(
                render_view.sky_view_projection().is_finite(),
                "vertical sky view-projection should be finite"
            );
        }
    }

    #[test]
    fn sky_view_projection_drops_render_pose_translation() {
        let orientation = Quat::from_rotation_y(0.37) * Quat::from_rotation_x(-0.41);
        let base = test_perspective_pose(Vec3::new(8.0, 70.0, 8.0), orientation)
            .render_view(1280, 720)
            .expect("base render pose should build");
        let shifted = test_perspective_pose(Vec3::new(136.0, 59.0, 55.0), orientation)
            .render_view(1280, 720)
            .expect("shifted render pose should build");

        assert!(
            !mat4_near(base.view_projection, shifted.view_projection, 0.0001),
            "regular view projection should retain render-pose translation"
        );
        assert_mat4_near(
            base.sky_view_projection(),
            shifted.sky_view_projection(),
            0.0001,
        );
    }

    #[test]
    fn render_view_can_apply_underwater_fov_multiplier() {
        let camera = ChunkCamera {
            eye: [0.0, 64.0, 0.0],
            target: [0.0, 64.0, -1.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 70.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 256.0,
        };
        let base = camera.render_view(1280, 720);
        let narrowed = base.with_fov_multiplier(0.85714287);

        assert!((narrowed.fov_y_radians - 60.0_f32.to_radians()).abs() < 1.0e-5);
        assert_eq!(narrowed.view, base.view);
        assert_ne!(narrowed.projection, base.projection);
        assert_ne!(narrowed.view_projection, base.view_projection);
    }

    #[test]
    fn external_render_view_ignores_underwater_fov_multiplier() {
        let camera = ChunkCamera {
            eye: [0.0, 64.0, 0.0],
            target: [0.0, 64.0, -1.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 70.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 256.0,
        };
        let external = ChunkRenderView {
            projection_kind: ChunkProjectionKind::External,
            ..camera.render_view(1280, 720)
        };

        assert_eq!(external.with_fov_multiplier(0.85714287), external);
    }

    #[test]
    fn mono_culling_cache_requires_identical_view_options_and_record_generation() {
        let records = Arc::new(PreparedTexturedSectionRecords {
            records: BTreeMap::new(),
            loaded_section_count: 0,
            loaded_index_count: 0,
        });
        let render_view = ChunkCamera::overview_for_chunk(0, 0).render_view(640, 480);
        let key = TexturedSectionMonoCullingCacheKey {
            render_view,
            section_occlusion_culling: true,
            force_fullbright: false,
            topology: HorizontalTopology::UNBOUNDED,
        };
        let cache = TexturedSectionMonoCullingCache {
            key,
            records: Arc::clone(&records),
            result: Arc::new(TexturedSectionCullingResult {
                stats: TexturedSectionRenderStats::default(),
                drawn_keys: FxHashSet::default(),
            }),
        };

        assert!(cache.matches(key, &records));

        let moved_key = TexturedSectionMonoCullingCacheKey {
            render_view: ChunkCamera::overview_for_chunk(1, 0).render_view(640, 480),
            ..key
        };
        assert!(!cache.matches(moved_key, &records));

        let replacement = Arc::new(PreparedTexturedSectionRecords {
            records: BTreeMap::new(),
            loaded_section_count: 0,
            loaded_index_count: 0,
        });
        assert!(!cache.matches(key, &replacement));
    }

    #[test]
    fn textured_render_options_serialize_fullbright_and_sky_darken() {
        let render_view = ChunkCamera::overview_for_chunk(0, 0).render_view(640, 480);
        let bytes = uniform_bytes(
            render_view,
            TexturedSectionRenderOptions {
                force_fullbright: true,
                sky_darken: 0.25,
                ..TexturedSectionRenderOptions::default()
            },
            wgpu::TextureFormat::Rgba8Unorm,
        );

        assert_eq!(f32::from_ne_bytes(bytes[64..68].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_ne_bytes(bytes[68..72].try_into().unwrap()), 0.25);
        assert_eq!(f32::from_ne_bytes(bytes[76..80].try_into().unwrap()), 0.0);
    }

    #[test]
    fn textured_render_options_serialize_color_profile_transform() {
        let render_view = ChunkCamera::overview_for_chunk(0, 0).render_view(640, 480);
        let bytes = uniform_bytes(
            render_view,
            TexturedSectionRenderOptions::default(),
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );
        assert_eq!(f32::from_ne_bytes(bytes[76..80].try_into().unwrap()), 1.0);

        let bytes = uniform_bytes(
            render_view,
            TexturedSectionRenderOptions::default()
                .with_color_profile(RenderColorProfile::StylizedBright),
            wgpu::TextureFormat::Rgba8Unorm,
        );
        assert_eq!(f32::from_ne_bytes(bytes[76..80].try_into().unwrap()), 2.0);
    }

    #[test]
    fn textured_render_options_serialize_underwater_fog() {
        let render_view = ChunkCamera {
            eye: [1.0, 2.0, 3.0],
            target: [1.0, 2.0, 2.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 70.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 256.0,
        }
        .render_view(640, 480);
        let bytes = uniform_bytes(
            render_view,
            TexturedSectionRenderOptions::default().with_fog(RenderFog::underwater()),
            wgpu::TextureFormat::Rgba8Unorm,
        );

        assert_eq!(f32::from_ne_bytes(bytes[72..76].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_ne_bytes(bytes[80..84].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_ne_bytes(bytes[84..88].try_into().unwrap()), 2.0);
        assert_eq!(f32::from_ne_bytes(bytes[88..92].try_into().unwrap()), 3.0);
        assert_eq!(
            f32::from_ne_bytes(bytes[96..100].try_into().unwrap()),
            5.0 / 255.0
        );
        assert_eq!(
            f32::from_ne_bytes(bytes[104..108].try_into().unwrap()),
            51.0 / 255.0
        );
        assert_eq!(
            f32::from_ne_bytes(bytes[112..116].try_into().unwrap()),
            -8.0
        );
        assert_eq!(
            f32::from_ne_bytes(bytes[116..120].try_into().unwrap()),
            96.0
        );
    }

    #[test]
    fn textured_render_options_clamp_sky_darken() {
        let options = TexturedSectionRenderOptions::default().with_sky_darken(2.0);

        assert_eq!(options.sky_darken, 1.0);
    }

    #[test]
    fn area_overview_camera_targets_center_chunk() {
        let camera = ChunkCamera::overview_for_chunk_area(2, -3, 1);

        assert_eq!(camera.target, [40.0, 48.0, -40.0]);
        assert!(camera.eye[0] > camera.target[0]);
        assert!(camera.eye[2] < camera.target[2]);
    }

    #[test]
    fn orbit_preserves_target_and_distance() {
        let mut camera = ChunkCamera::overview_for_chunk(0, 0);
        let target = camera.target;
        let before = (Vec3::from_array(camera.eye) - Vec3::from_array(camera.target)).length();

        camera.orbit(0.25, -0.1);

        let after = (Vec3::from_array(camera.eye) - Vec3::from_array(camera.target)).length();
        assert_eq!(camera.target, target);
        assert!((before - after).abs() < 0.001);
    }

    #[test]
    fn local_move_translates_eye_and_target_together() {
        let mut camera = ChunkCamera::overview_for_chunk(0, 0);
        let eye = Vec3::from_array(camera.eye);
        let target = Vec3::from_array(camera.target);

        camera.move_local(1.0, 0.0, 0.0, 3.0);

        let eye_delta = Vec3::from_array(camera.eye) - eye;
        let target_delta = Vec3::from_array(camera.target) - target;
        assert!((eye_delta - target_delta).length() < 0.001);
        assert!((eye_delta.length() - 3.0).abs() < 0.001);
    }

    #[test]
    fn textured_section_render_stats_report_quad_faces() {
        let stats = TexturedSectionRenderStats {
            loaded_section_count: 2,
            drawn_section_count: 1,
            frustum_section_count: 2,
            graph_culled_section_count: 1,
            loaded_index_count: 60,
            drawn_index_count: 36,
            frustum_index_count: 60,
            graph_culled_index_count: 24,
            ..TexturedSectionRenderStats::default()
        };

        assert_eq!(stats.loaded_face_count(), 10);
        assert_eq!(stats.drawn_face_count(), 6);
        assert_eq!(stats.frustum_face_count(), 10);
        assert_eq!(stats.graph_culled_face_count(), 4);

        let upload = TexturedSectionUploadReport {
            uploaded_section_count: 1,
            removed_section_count: 0,
            uploaded_vertex_count: 40,
            uploaded_index_count: 60,
        };
        assert_eq!(upload.uploaded_face_count(), 10);
    }

    fn fake_section(
        key: RenderSectionKey,
        index_count: usize,
        visibility: VisibilitySet,
    ) -> TexturedRenderSectionMetadata {
        TexturedRenderSectionMesh {
            key,
            mesh: TexturedVisibleChunkMesh {
                vertices: Vec::new(),
                indices: vec![0; index_count],
                solid_index_count: index_count as u32,
                opaque_index_count: index_count as u32,
            },
            visibility,
        }
        .metadata()
    }

    fn prepared_records_for_sections(
        sections: &[TexturedRenderSectionMetadata],
    ) -> PreparedTexturedSectionRecords {
        let records = sections
            .iter()
            .map(|section| {
                (
                    section.key,
                    TexturedSectionCullingRecord {
                        index_count: section.stats.index_count,
                        visibility: section.visibility,
                        drawable: section.drawable,
                        traversal_ready: true,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let loaded_section_count = records.values().filter(|record| record.drawable).count();
        let loaded_index_count = records
            .values()
            .filter(|record| record.drawable)
            .map(|record| record.index_count)
            .sum();
        PreparedTexturedSectionRecords {
            records,
            loaded_section_count,
            loaded_index_count,
        }
    }

    #[test]
    fn textured_section_visibility_stats_seeds_visible_edge_when_camera_section_is_missing() {
        let sections = vec![
            fake_section(
                RenderSectionKey::new(0, 0, 0),
                6,
                VisibilitySet::all_visible(),
            ),
            fake_section(
                RenderSectionKey::new(1, 0, 0),
                6,
                VisibilitySet::all_visible(),
            ),
        ];
        let camera = ChunkCamera {
            eye: [8.0, 8.0, -40.0],
            target: [16.0, 8.0, 8.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 80.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 200.0,
        };

        let stats = textured_section_visibility_stats(&sections, camera.render_view(800, 600));

        assert!(stats.graph_cull_enabled);
        assert_eq!(stats.frustum_section_count, stats.drawn_section_count);
        assert_eq!(stats.frustum_index_count, stats.drawn_index_count);
    }

    #[test]
    fn textured_section_visibility_stats_outside_seed_still_uses_section_graph() {
        let mut blocked_middle_visibility = VisibilitySet::all_visible();
        blocked_middle_visibility.set(SectionFace::Up, SectionFace::Down, false);
        let sections = vec![
            fake_section(
                RenderSectionKey::new(0, 0, 0),
                6,
                VisibilitySet::all_visible(),
            ),
            fake_section(RenderSectionKey::new(0, 1, 0), 6, blocked_middle_visibility),
            fake_section(
                RenderSectionKey::new(0, 2, 0),
                6,
                VisibilitySet::all_visible(),
            ),
        ];
        let camera = ChunkCamera {
            eye: [8.0, 64.0, 8.0],
            target: [8.0, 0.0, 8.0],
            up: [0.0, 0.0, 1.0],
            fov_y_radians: 80.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 120.0,
        };

        let stats = textured_section_visibility_stats(&sections, camera.render_view(800, 600));

        assert!(stats.graph_cull_enabled);
        assert_eq!(stats.frustum_section_count, 3);
        assert_eq!(stats.drawn_section_count, 2);
        assert_eq!(stats.graph_culled_section_count, 1);
        assert_eq!(stats.graph_culled_index_count, 6);
    }

    #[test]
    fn outside_seed_enters_each_visible_column_at_its_own_top_section() {
        let sections = vec![
            fake_section(
                RenderSectionKey::new(-1, 1, 0),
                6,
                VisibilitySet::all_visible(),
            ),
            fake_section(
                RenderSectionKey::new(0, 3, 0),
                6,
                VisibilitySet::all_visible(),
            ),
            fake_section(
                RenderSectionKey::new(1, 0, 0),
                6,
                VisibilitySet::all_visible(),
            ),
        ];
        let prepared = prepared_records_for_sections(&sections);
        let empty_above_short_column = RenderSectionKey::new(1, 2, 0);
        let mut prepared = prepared;
        prepared.records.insert(
            empty_above_short_column,
            TexturedSectionCullingRecord {
                index_count: 0,
                visibility: VisibilitySet::all_visible(),
                drawable: false,
                traversal_ready: true,
            },
        );
        let frustum_keys = prepared.records.keys().copied().collect();

        let starts = outside_retained_section_start_keys(
            &prepared.records,
            &frustum_keys,
            Vec3::new(8.0, 128.0, 8.0),
            HorizontalTopology::UNBOUNDED,
        );

        assert_eq!(starts.len(), 3);
        assert!(!starts.contains(&empty_above_short_column));
        assert_eq!(
            starts.into_iter().collect::<BTreeSet<_>>(),
            sections
                .into_iter()
                .map(|section| section.key)
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn empty_sparse_camera_section_falls_back_to_visible_column_surfaces() {
        let camera_key = RenderSectionKey::new(0, 12, 0);
        let surface_keys = [
            RenderSectionKey::new(-1, 4, 0),
            RenderSectionKey::new(0, 4, 0),
            RenderSectionKey::new(1, 3, 0),
        ];
        let mut records = surface_keys
            .into_iter()
            .map(|key| {
                (
                    key,
                    TexturedSectionCullingRecord {
                        index_count: 6,
                        visibility: VisibilitySet::all_visible(),
                        drawable: true,
                        traversal_ready: true,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        records.insert(
            camera_key,
            TexturedSectionCullingRecord {
                index_count: 0,
                visibility: VisibilitySet::all_visible(),
                drawable: false,
                traversal_ready: true,
            },
        );
        let frustum_keys = records.keys().copied().collect();

        let starts = traversal_start_keys(
            &records,
            &frustum_keys,
            Vec3::new(8.0, 200.0, 8.0),
            HorizontalTopology::UNBOUNDED,
        );

        assert_eq!(
            starts.into_iter().collect::<BTreeSet<_>>(),
            surface_keys.into_iter().collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn textured_section_visibility_stats_prunes_through_section_graph() {
        let mut wall_visibility = VisibilitySet::all_visible();
        wall_visibility.set(SectionFace::West, SectionFace::East, false);
        let sections = vec![
            fake_section(
                RenderSectionKey::new(0, 0, 0),
                6,
                VisibilitySet::all_visible(),
            ),
            fake_section(RenderSectionKey::new(1, 0, 0), 6, wall_visibility),
            fake_section(
                RenderSectionKey::new(2, 0, 0),
                6,
                VisibilitySet::all_visible(),
            ),
        ];
        let camera = ChunkCamera {
            eye: [8.0, 8.0, 8.0],
            target: [48.0, 8.0, 8.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 90.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 200.0,
        };

        let stats = textured_section_visibility_stats(&sections, camera.render_view(800, 600));

        assert!(stats.graph_cull_enabled);
        assert_eq!(stats.frustum_section_count, 3);
        assert_eq!(stats.drawn_section_count, 2);
        assert_eq!(stats.graph_culled_section_count, 1);
        assert_eq!(stats.graph_culled_index_count, 6);
    }

    #[test]
    fn periodic_culling_draws_canonical_last_chunk_beside_zero() {
        let sections = vec![
            fake_section(
                RenderSectionKey::new(0, 0, 0),
                6,
                VisibilitySet::all_visible(),
            ),
            fake_section(
                RenderSectionKey::new(31, 0, 0),
                6,
                VisibilitySet::all_visible(),
            ),
        ];
        let camera = ChunkCamera {
            eye: [8.0, 8.0, 8.0],
            target: [-16.0, 8.0, 8.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 90.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 80.0,
        };
        let options = TexturedSectionRenderOptions::default()
            .with_topology(HorizontalTopology::cylinder_x(0, 32));

        let stats = textured_section_visibility_stats_with_options(
            &sections,
            camera.render_view(800, 600),
            options,
        );

        assert_eq!(stats.frustum_section_count, 2);
        assert_eq!(stats.drawn_section_count, 2);
        assert_eq!(stats.graph_culled_section_count, 0);
    }

    #[test]
    fn textured_section_visibility_stats_can_disable_section_occlusion() {
        let mut wall_visibility = VisibilitySet::all_visible();
        wall_visibility.set(SectionFace::West, SectionFace::East, false);
        let sections = vec![
            fake_section(
                RenderSectionKey::new(0, 0, 0),
                6,
                VisibilitySet::all_visible(),
            ),
            fake_section(RenderSectionKey::new(1, 0, 0), 6, wall_visibility),
            fake_section(
                RenderSectionKey::new(2, 0, 0),
                6,
                VisibilitySet::all_visible(),
            ),
        ];
        let camera = ChunkCamera {
            eye: [8.0, 8.0, 8.0],
            target: [48.0, 8.0, 8.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 90.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 200.0,
        };

        let stats = textured_section_visibility_stats_with_options(
            &sections,
            camera.render_view(800, 600),
            TexturedSectionRenderOptions {
                section_occlusion_culling: false,
                ..TexturedSectionRenderOptions::default()
            },
        );

        assert!(!stats.section_occlusion_culling);
        assert!(!stats.force_fullbright);
        assert!(!stats.graph_cull_enabled);
        assert_eq!(stats.frustum_section_count, 3);
        assert_eq!(stats.drawn_section_count, stats.frustum_section_count);
        assert_eq!(stats.graph_culled_section_count, 0);
        assert_eq!(stats.drawn_index_count, stats.frustum_index_count);
        assert_eq!(stats.graph_culled_index_count, 0);
    }

    #[test]
    fn textured_section_visibility_stats_culls_not_ready_sections_even_without_graph() {
        let sections = vec![
            fake_section(
                RenderSectionKey::new(0, 0, 0),
                6,
                VisibilitySet::all_visible(),
            ),
            fake_section(
                RenderSectionKey::new(1, 0, 0),
                6,
                VisibilitySet::all_visible(),
            ),
            fake_section(
                RenderSectionKey::new(2, 0, 0),
                6,
                VisibilitySet::all_visible(),
            ),
        ];
        let ready = BTreeSet::from([
            RenderSectionKey::new(0, 0, 0),
            RenderSectionKey::new(1, 0, 0),
        ]);
        let camera = ChunkCamera {
            eye: [8.0, 8.0, 8.0],
            target: [48.0, 8.0, 8.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 90.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 200.0,
        };

        let stats = textured_section_visibility_stats_with_options_and_ready_sections(
            &sections,
            camera.render_view(800, 600),
            TexturedSectionRenderOptions {
                section_occlusion_culling: false,
                ..TexturedSectionRenderOptions::default()
            },
            Some(&ready),
        );

        assert!(!stats.graph_cull_enabled);
        assert_eq!(stats.frustum_section_count, 3);
        assert_eq!(stats.drawn_section_count, 2);
        assert_eq!(stats.readiness_culled_section_count, 1);
        assert_eq!(stats.readiness_culled_index_count, 6);
        assert_eq!(stats.graph_culled_section_count, 0);
    }

    #[test]
    fn record_cache_stats_track_upload_backpressured_ready_set_context() {
        let baseline = TexturedSectionRecordCacheStats::default();
        let mut stats = baseline;

        stats.record_ready_set_call(false, true);
        stats.record_ready_set_call(true, false);
        stats.record_ready_set_call(true, true);
        stats.record_ready_set_skip(false);
        stats.record_ready_set_skip(true);

        assert_eq!(stats.ready_set_calls, 3);
        assert_eq!(stats.ready_set_changed_calls, 2);
        assert_eq!(stats.ready_set_unchanged_calls, 1);
        assert_eq!(stats.ready_set_upload_backpressured_calls, 2);
        assert_eq!(stats.ready_set_upload_backpressured_changed_calls, 1);
        assert_eq!(stats.ready_set_upload_backpressured_unchanged_calls, 1);
        assert_eq!(stats.ready_set_skipped_calls, 2);
        assert_eq!(stats.ready_set_upload_backpressured_skipped_calls, 1);

        let delta = stats.sample_delta(baseline);
        assert_eq!(delta.ready_set_calls, 3);
        assert_eq!(delta.ready_set_upload_backpressured_calls, 2);
        assert_eq!(delta.ready_set_upload_backpressured_changed_calls, 1);
        assert_eq!(delta.ready_set_upload_backpressured_unchanged_calls, 1);
        assert_eq!(delta.ready_set_skipped_calls, 2);
        assert_eq!(delta.ready_set_upload_backpressured_skipped_calls, 1);

        let mut next = stats;
        next.record_ready_set_call(true, false);
        next.record_ready_set_skip(true);
        let next_delta = next.sample_delta(stats);
        assert_eq!(next_delta.ready_set_calls, 1);
        assert_eq!(next_delta.ready_set_changed_calls, 0);
        assert_eq!(next_delta.ready_set_unchanged_calls, 1);
        assert_eq!(next_delta.ready_set_upload_backpressured_calls, 1);
        assert_eq!(next_delta.ready_set_upload_backpressured_changed_calls, 0);
        assert_eq!(next_delta.ready_set_upload_backpressured_unchanged_calls, 1);
        assert_eq!(next_delta.ready_set_skipped_calls, 1);
        assert_eq!(next_delta.ready_set_upload_backpressured_skipped_calls, 1);
    }

    #[test]
    fn record_cache_stats_track_prepared_rebuild_dirty_causes() {
        let baseline = TexturedSectionRecordCacheStats::default();
        let mut stats = baseline;

        stats.record_prepared_record_rebuild(
            1.5,
            TexturedSectionRecordDirtyCauses::INITIAL
                .with(TexturedSectionRecordDirtyCauses::SECTION_UPLOAD),
            16,
            12,
            10,
            600,
        );
        stats.record_prepared_record_rebuild(
            0.75,
            TexturedSectionRecordDirtyCauses::TRAVERSAL_READY
                .with(TexturedSectionRecordDirtyCauses::UPLOAD_BACKPRESSURED),
            18,
            14,
            8,
            720,
        );

        assert_eq!(stats.prepared_record_rebuilds, 2);
        assert_eq!(stats.prepared_record_rebuild_initial_dirty, 1);
        assert_eq!(stats.prepared_record_rebuild_section_upload_dirty, 1);
        assert_eq!(stats.prepared_record_rebuild_section_remove_dirty, 0);
        assert_eq!(stats.prepared_record_rebuild_ready_set_dirty, 1);
        assert_eq!(stats.prepared_record_rebuild_upload_backpressured_dirty, 1);
        assert_eq!(stats.prepared_record_rebuild_multi_dirty, 2);
        assert_eq!(stats.prepared_record_rebuild_visibility_section_max, 18);
        assert_eq!(stats.prepared_record_rebuild_loaded_section_max, 14);
        assert_eq!(stats.prepared_record_rebuild_ready_section_max, 10);
        assert_eq!(stats.prepared_record_rebuild_loaded_index_max, 720);

        let delta = stats.sample_delta(baseline);
        assert_eq!(delta.prepared_record_rebuilds, 2);
        assert_eq!(delta.prepared_record_rebuild_initial_dirty, 1);
        assert_eq!(delta.prepared_record_rebuild_section_upload_dirty, 1);
        assert_eq!(delta.prepared_record_rebuild_ready_set_dirty, 1);
        assert_eq!(delta.prepared_record_rebuild_upload_backpressured_dirty, 1);
        assert_eq!(delta.prepared_record_rebuild_multi_dirty, 2);
        assert_eq!(delta.prepared_record_rebuild_visibility_section_max, 0);
        assert_eq!(delta.prepared_record_rebuild_loaded_section_max, 0);
        assert_eq!(delta.prepared_record_rebuild_ready_section_max, 0);
        assert_eq!(delta.prepared_record_rebuild_loaded_index_max, 0);
    }

    #[test]
    fn textured_section_visibility_stats_uses_host_supplied_render_view() {
        let west = RenderSectionKey::new(-2, 0, 0);
        let east = RenderSectionKey::new(2, 0, 0);
        let sections = vec![
            fake_section(west, 6, VisibilitySet::all_visible()),
            fake_section(east, 6, VisibilitySet::all_visible()),
        ];
        let eye = Vec3::new(8.0, 8.0, 8.0);
        let east_view = test_render_view(eye, Vec3::new(48.0, 8.0, 8.0), Vec3::Y, 800, 800);
        let west_view = test_render_view(eye, Vec3::new(-32.0, 8.0, 8.0), Vec3::Y, 800, 800);
        let options = TexturedSectionRenderOptions {
            section_occlusion_culling: false,
            ..TexturedSectionRenderOptions::default()
        };

        let east_stats =
            textured_section_visibility_stats_with_options(&sections, east_view, options);
        let west_stats =
            textured_section_visibility_stats_with_options(&sections, west_view, options);

        assert_eq!(east_stats.frustum_section_count, 1);
        assert_eq!(east_stats.frustum_index_count, 6);
        assert_eq!(east_stats.drawn_section_count, 1);
        assert_eq!(west_stats.frustum_section_count, 1);
        assert_eq!(west_stats.frustum_index_count, 6);
        assert_eq!(west_stats.drawn_section_count, 1);
    }

    #[test]
    fn stereo_union_cull_matches_union_of_per_eye_host_views() {
        let west = RenderSectionKey::new(-2, 0, 0);
        let east = RenderSectionKey::new(2, 0, 0);
        let sections = vec![
            fake_section(west, 6, VisibilitySet::all_visible()),
            fake_section(east, 6, VisibilitySet::all_visible()),
        ];
        let records = prepared_records_for_sections(&sections);
        let eye = Vec3::new(8.0, 8.0, 8.0);
        let left_view = test_render_view(eye, Vec3::new(-32.0, 8.0, 8.0), Vec3::Y, 800, 800);
        let right_view = test_render_view(eye, Vec3::new(48.0, 8.0, 8.0), Vec3::Y, 800, 800);
        let options = TexturedSectionRenderOptions {
            section_occlusion_culling: false,
            ..TexturedSectionRenderOptions::default()
        };
        let mut scratch = CullScratch::default();
        let left = cull_textured_sections(&records, left_view, options, &mut scratch);
        let right = cull_textured_sections(&records, right_view, options, &mut scratch);
        let stereo = cull_textured_sections_stereo_union(
            &records,
            [left_view, right_view],
            [options, options],
            &mut scratch,
        );
        let mut expected = left.drawn_keys;
        expected.extend(right.drawn_keys);

        assert!(expected.contains(&west));
        assert!(expected.contains(&east));
        assert_eq!(stereo.drawn_keys, expected);
        assert_eq!(stereo.draw_masks.get(&west), Some(&StereoDrawMask::LEFT));
        assert_eq!(stereo.draw_masks.get(&east), Some(&StereoDrawMask::RIGHT));
        assert_eq!(stereo.stats.drawn_section_count, expected.len());
        assert_eq!(stereo.stats.drawn_index_count, 12);

        assert_eq!(stereo.eye_stats[0].drawn_section_count, 1);
        assert_eq!(stereo.eye_stats[0].drawn_index_count, 6);
        assert_eq!(stereo.eye_stats[1].drawn_section_count, 1);
        assert_eq!(stereo.eye_stats[1].drawn_index_count, 6);
    }

    #[test]
    fn frustum_marks_target_section_visible() {
        let camera = ChunkCamera {
            eye: [8.0, 80.0, -40.0],
            target: [8.0, 48.0, 8.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 64.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 700.0,
        };
        let frustum = ClipFrustum::from_render_view(
            camera.render_view(1280, 720),
            HorizontalTopology::UNBOUNDED,
        );

        assert!(frustum.is_render_section_visible(RenderSectionKey::new(0, 3, 0)));
    }

    #[test]
    fn frustum_rejects_section_behind_camera() {
        let camera = ChunkCamera {
            eye: [8.0, 80.0, -40.0],
            target: [8.0, 48.0, 8.0],
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 64.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 700.0,
        };
        let frustum = ClipFrustum::from_render_view(
            camera.render_view(1280, 720),
            HorizontalTopology::UNBOUNDED,
        );

        assert!(!frustum.is_render_section_visible(RenderSectionKey::new(0, 3, -8)));
    }

    #[test]
    fn vertex_and_index_bytes_match_gpu_layout() {
        let mut blocks = vec![0; 16 * 16 * 16];
        blocks[0] = 1;
        let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

        assert_eq!(
            vertex_bytes(&mesh).len() as wgpu::BufferAddress,
            mesh.vertices.len() as wgpu::BufferAddress * VERTEX_BYTE_SIZE
        );
        assert_eq!(index_bytes(&mesh.indices).len(), mesh.indices.len() * 4);
    }

    #[test]
    fn terrain_arena_range_allocator_reuses_and_coalesces_space() {
        let mut allocator = ElementRangeAllocator::new(16);
        let first = allocator.allocate(4).expect("first range");
        let second = allocator.allocate(6).expect("second range");
        assert_eq!(first, 0..4);
        assert_eq!(second, 4..10);
        assert_eq!(allocator.used(), 10);

        allocator.release(first);
        allocator.release(second);
        assert_eq!(allocator.used(), 0);
        assert_eq!(allocator.allocate(16), Some(0..16));
        assert_eq!(allocator.allocate(1), None);
    }

    #[test]
    fn terrain_arena_growth_can_use_a_non_power_of_two_adapter_limit() {
        assert_eq!(grown_arena_capacity(4, 1, 6).expect("bounded growth"), 6);
        assert!(grown_arena_capacity(6, 1, 6).is_err());
    }

    #[test]
    fn terrain_arena_indirect_args_apply_global_mesh_offsets() {
        let mesh = GpuTexturedSectionMesh {
            vertex_page: 0,
            vertex_range: 100..116,
            index_range: 200..224,
            solid_index_count: 12,
            opaque_index_count: 18,
        };

        let solid = mesh.indirect_args(mesh.solid_index_range());
        assert_eq!(solid.index_count, 12);
        assert_eq!(solid.first_index, 200);
        assert_eq!(solid.base_vertex, 100);
        assert_eq!(solid.instance_count, 1);
        assert_eq!(solid.first_instance, 0);

        let cutout = mesh.indirect_args(mesh.cutout_index_range());
        assert_eq!(cutout.index_count, 6);
        assert_eq!(cutout.first_index, 212);
        assert_eq!(cutout.base_vertex, 100);
    }

    #[test]
    fn textured_vertex_bytes_match_gpu_layout() {
        let mesh = TexturedVisibleChunkMesh {
            vertices: vec![TexturedChunkVertex {
                position: [1.0, 2.0, 3.0],
                uv: [0.25, 0.75],
                color: [1.0, 0.5, 0.25, 1.0],
                packed_light: 15_728_880,
            }],
            indices: vec![0],
            solid_index_count: 1,
            opaque_index_count: 1,
        };

        assert_eq!(
            textured_vertex_bytes(&mesh).len() as wgpu::BufferAddress,
            TEXTURED_VERTEX_BYTE_SIZE
        );
    }

    fn test_render_view(
        eye: Vec3,
        target: Vec3,
        world_up: Vec3,
        width: u32,
        height: u32,
    ) -> ChunkRenderView {
        let aspect = width.max(1) as f32 / height.max(1) as f32;
        let fov_y_radians = 70.0_f32.to_radians();
        let z_near = 0.05;
        let z_far = 200.0;
        let view = Mat4::look_at_rh(eye, target, world_up);
        let projection = reversed_z_perspective_rh(fov_y_radians, aspect, z_near, z_far);
        let forward = (target - eye).normalize_or_zero();
        let right = forward.cross(world_up).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();

        ChunkRenderView {
            view,
            projection,
            view_projection: projection * view,
            camera_position: eye,
            camera_forward: forward,
            camera_right: right,
            camera_up: up,
            aspect,
            fov_y_radians,
            z_near,
            z_far,
            projection_kind: ChunkProjectionKind::CameraPerspective,
        }
    }

    fn test_perspective_pose(eye: Vec3, orientation: Quat) -> PerspectiveRenderPose {
        PerspectiveRenderPose::new(eye, orientation, 70.0_f32.to_radians(), 0.05, 200.0)
    }

    fn assert_finite_render_view(render_view: ChunkRenderView) {
        assert!(
            render_view.is_finite(),
            "render view should be finite: {render_view:?}"
        );
    }

    fn assert_basis_is_orthonormal(render_view: ChunkRenderView) {
        assert!((render_view.camera_forward.length() - 1.0).abs() < 1.0e-6);
        assert!((render_view.camera_right.length() - 1.0).abs() < 1.0e-6);
        assert!((render_view.camera_up.length() - 1.0).abs() < 1.0e-6);
        assert!(
            render_view
                .camera_forward
                .dot(render_view.camera_right)
                .abs()
                < 1.0e-6
        );
        assert!(render_view.camera_forward.dot(render_view.camera_up).abs() < 1.0e-6);
        assert!(render_view.camera_right.dot(render_view.camera_up).abs() < 1.0e-6);
    }

    fn assert_vec3_near(left: Vec3, right: Vec3, epsilon: f32) {
        assert!(
            (left - right).length() <= epsilon,
            "vectors differed beyond {epsilon}: left={left:?} right={right:?}"
        );
    }

    fn assert_mat4_near(left: Mat4, right: Mat4, epsilon: f32) {
        assert!(
            mat4_near(left, right, epsilon),
            "matrices differed beyond {epsilon}: left={left:?} right={right:?}"
        );
    }

    fn mat4_near(left: Mat4, right: Mat4, epsilon: f32) -> bool {
        left.to_cols_array()
            .into_iter()
            .zip(right.to_cols_array())
            .all(|(left, right)| (left - right).abs() <= epsilon)
    }
}
