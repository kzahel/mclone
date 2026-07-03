use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::num::{NonZeroU32, NonZeroU64};
use std::ops::Range;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use anyhow::{Context, Result, bail};
use glam::{Mat4, Vec3, Vec4};
use mclone_core::{
    block_to_chunk_coord, block_to_section_coord, chunk_middle_block_coord, chunk_min_block_coord,
};
use mclone_mesh::{
    CHUNK_WIDTH as MESH_CHUNK_WIDTH, RENDER_SECTION_HEIGHT, RenderSectionKey, SectionFace,
    TexturedRenderSectionMesh, TexturedVisibleChunkMesh, VisibilitySet, VisibleChunkMesh,
    quad_face_count_from_indices,
};
use rustc_hash::{FxHashMap, FxHashSet};
use wgpu::util::DeviceExt;

use crate::color_profile::{RenderColorProfile, RenderConfig};
use crate::fog::RenderFog;
use crate::target::RenderFrameTarget;
use crate::texture_mips::generate_rgba_mip_chain;
use crate::uniform::{
    PER_VIEW_UNIFORM_SLOT_COUNT, PerViewSlot, PerViewUniformBuffer, SINGLE_VIEW_SLOT,
};

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

const VERTEX_FLOAT_COUNT: usize = 7;
const VERTEX_BYTE_SIZE: wgpu::BufferAddress =
    (VERTEX_FLOAT_COUNT * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const TEXTURED_VERTEX_BYTE_SIZE: wgpu::BufferAddress = 40;
const UNIFORM_BYTE_LEN: usize = 128;
const UNIFORM_BYTE_SIZE: wgpu::BufferAddress = UNIFORM_BYTE_LEN as wgpu::BufferAddress;
const MULTIVIEW_UNIFORM_BYTE_LEN: usize = UNIFORM_BYTE_LEN * 2;
const MULTIVIEW_UNIFORM_BYTE_SIZE: wgpu::BufferAddress =
    MULTIVIEW_UNIFORM_BYTE_LEN as wgpu::BufferAddress;
// Matches the default Java 1.17.1 video option: Options.mipmapLevels = 4.
// TextureUtil.prepareImage allocates levels 0..=4 for the block atlas.
const CHUNK_ATLAS_MAX_MIP_LEVEL: u32 = 4;

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
        let projection = Mat4::perspective_rh(self.fov_y_radians, aspect, self.z_near, self.z_far);
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
            fov_y_radians: self.fov_y_radians,
            z_near: self.z_near,
            z_far: self.z_far,
            projection_kind: ChunkProjectionKind::CameraPerspective,
        }
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
        let projection = Mat4::perspective_rh(fov_y_radians, self.aspect, self.z_near, self.z_far);
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
        let rotation_only_view = Mat4::look_at_rh(Vec3::ZERO, self.camera_forward, self.camera_up);
        self.projection * rotation_only_view
    }
}

#[derive(Clone, Copy)]
pub struct ChunkRenderTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub depth_view: &'a wgpu::TextureView,
    pub size: [u32; 2],
    pub clear_color: wgpu::Color,
    pub clear_depth: f32,
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
            clear_depth: 1.0,
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
        Ok(Self::new(
            target.color_view,
            depth_view,
            target.size,
            clear_color,
        ))
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
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TexturedSectionRenderTiming {
    pub records_ms: f64,
    pub cull_ms: f64,
    pub uniform_write_ms: f64,
    pub translucent_collect_ms: f64,
    pub translucent_sort_ms: f64,
    pub prepare_ms: f64,
    pub encode_ms: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexturedSectionRenderOptions {
    pub section_occlusion_culling: bool,
    pub force_fullbright: bool,
    pub sky_darken: f32,
    pub fog: RenderFog,
    pub color_profile: RenderColorProfile,
}

impl Default for TexturedSectionRenderOptions {
    fn default() -> Self {
        Self {
            section_occlusion_culling: true,
            force_fullbright: false,
            sky_darken: 1.0,
            fog: RenderFog::none(),
            color_profile: RenderColorProfile::default(),
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

pub fn textured_section_visibility_stats(
    sections: &[TexturedRenderSectionMesh],
    render_view: ChunkRenderView,
) -> TexturedSectionRenderStats {
    textured_section_visibility_stats_with_options(
        sections,
        render_view,
        TexturedSectionRenderOptions::default(),
    )
}

pub fn textured_section_visibility_stats_with_options(
    sections: &[TexturedRenderSectionMesh],
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
    sections: &[TexturedRenderSectionMesh],
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
                    index_count: section.stats().index_count,
                    visibility: section.visibility,
                    drawable: !section.is_empty(),
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
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TexturedSectionRecordPrepareStats {
    pub rebuilt: bool,
    pub rebuild_ms: f64,
    pub cache: TexturedSectionRecordCacheStats,
}

#[derive(Clone, Debug)]
struct TexturedSectionCullingResult {
    stats: TexturedSectionRenderStats,
    drawn_keys: FxHashSet<RenderSectionKey>,
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

    fn for_slot(view_slot: PerViewSlot) -> Self {
        if view_slot.is_right_eye() {
            Self::RIGHT
        } else {
            Self::LEFT
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

    fn stats_for_slot(&self, view_slot: PerViewSlot) -> TexturedSectionRenderStats {
        if view_slot.is_right_eye() {
            self.eye_stats[1]
        } else {
            self.eye_stats[0]
        }
    }

    fn draws_in_slot(&self, key: RenderSectionKey, view_slot: PerViewSlot) -> bool {
        self.draw_masks
            .get(&key)
            .is_some_and(|mask| mask.contains(StereoDrawMask::for_slot(view_slot)))
    }
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
    let records = &prepared.records;
    let frustum = ClipFrustum::from_render_view(render_view);
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

    let start_keys = traversal_start_keys(records, frustum_keys, render_view.camera_position);
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

            let neighbor_key = section_neighbor_key(key, direction);
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
    let records = &prepared.records;
    let frustums = render_views.map(ClipFrustum::from_render_view);
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
    let start_keys = traversal_start_keys(records, frustum_keys, center_position);
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

            let neighbor_key = section_neighbor_key(key, direction);
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
) -> Vec<RenderSectionKey> {
    let Some(camera_key) = render_section_key_containing(camera_position) else {
        return Vec::new();
    };
    if records
        .get(&camera_key)
        .is_some_and(|record| record.traversal_ready)
    {
        return vec![camera_key];
    }

    outside_retained_section_start_keys(records, frustum_keys, camera_position)
}

fn outside_retained_section_start_keys(
    records: &BTreeMap<RenderSectionKey, TexturedSectionCullingRecord>,
    frustum_keys: &FxHashSet<RenderSectionKey>,
    camera_position: Vec3,
) -> Vec<RenderSectionKey> {
    if !camera_position.is_finite() {
        return Vec::new();
    }
    let Some(min_section_y) = records.keys().map(|key| key.section_y).min() else {
        return Vec::new();
    };
    let Some(max_section_y) = records.keys().map(|key| key.section_y).max() else {
        return Vec::new();
    };

    let min_build_y = min_section_y * RENDER_SECTION_HEIGHT;
    let target_section_y = if camera_position.y.floor() as i32 > min_build_y {
        max_section_y
    } else {
        min_section_y
    };

    // Mirrors LevelRenderer.updateRenderChunks when the camera has no containing RenderChunk.
    let mut starts = records
        .keys()
        .copied()
        .filter(|key| {
            key.section_y == target_section_y
                && frustum_keys.contains(key)
                && records
                    .get(key)
                    .is_some_and(|record| record.traversal_ready)
        })
        .collect::<Vec<_>>();
    starts.sort_by(|a, b| {
        let a_distance = render_section_center(*a).distance_squared(camera_position);
        let b_distance = render_section_center(*b).distance_squared(camera_position);
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

fn render_section_key_containing(position: Vec3) -> Option<RenderSectionKey> {
    if !position.is_finite() {
        return None;
    }
    let block_x = position.x.floor() as i32;
    let block_y = position.y.floor() as i32;
    let block_z = position.z.floor() as i32;
    Some(RenderSectionKey::new(
        block_to_chunk_coord(block_x),
        block_to_section_coord(block_y),
        block_to_chunk_coord(block_z),
    ))
}

fn section_neighbor_key(key: RenderSectionKey, face: SectionFace) -> RenderSectionKey {
    let [dx, dy, dz] = face.section_delta();
    RenderSectionKey::new(key.chunk_x + dx, key.section_y + dy, key.chunk_z + dz)
}

fn render_section_center(key: RenderSectionKey) -> Vec3 {
    Vec3::new(
        chunk_middle_block_coord(key.chunk_x) as f32,
        key.min_y() as f32 + RENDER_SECTION_HEIGHT as f32 * 0.5,
        chunk_middle_block_coord(key.chunk_z) as f32,
    )
}

#[derive(Clone, Copy, Debug)]
struct ClipFrustum {
    view_projection: Mat4,
}

impl ClipFrustum {
    fn from_render_view(render_view: ChunkRenderView) -> Self {
        Self {
            view_projection: render_view.view_projection,
        }
    }

    fn is_render_section_visible(&self, key: RenderSectionKey) -> bool {
        let min = Vec3::new(
            chunk_min_block_coord(key.chunk_x) as f32,
            key.min_y() as f32,
            chunk_min_block_coord(key.chunk_z) as f32,
        );
        let max = min
            + Vec3::new(
                MESH_CHUNK_WIDTH as f32,
                RENDER_SECTION_HEIGHT as f32,
                MESH_CHUNK_WIDTH as f32,
            );
        self.is_aabb_visible(min, max)
    }

    fn is_aabb_visible(&self, min: Vec3, max: Vec3) -> bool {
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
            let clip = self.view_projection * Vec4::new(corner.x, corner.y, corner.z, 1.0);
            outside_left &= clip.x < -clip.w;
            outside_right &= clip.x > clip.w;
            outside_bottom &= clip.y < -clip.w;
            outside_top &= clip.y > clip.w;
            outside_near &= clip.z < -clip.w;
            outside_far &= clip.z > clip.w;
        }

        !(outside_left
            || outside_right
            || outside_bottom
            || outside_top
            || outside_near
            || outside_far)
    }
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
    index_count: u32,
    solid_index_count: u32,
    opaque_index_count: u32,
    visibility: VisibilitySet,
}

impl GpuTexturedChunkMesh {
    pub fn new(device: &wgpu::Device, mesh: &TexturedVisibleChunkMesh) -> Result<Self> {
        Self::with_visibility(device, mesh, VisibilitySet::all_visible())
    }

    pub fn with_visibility(
        device: &wgpu::Device,
        mesh: &TexturedVisibleChunkMesh,
        visibility: VisibilitySet,
    ) -> Result<Self> {
        if mesh.is_empty() {
            bail!("cannot upload an empty textured chunk mesh");
        }
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_textured_chunk_vertices"),
            contents: &textured_vertex_bytes(mesh),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_textured_chunk_indices"),
            contents: &index_bytes(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Ok(Self {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
            solid_index_count: mesh.solid_index_count().min(mesh.indices.len() as u32),
            opaque_index_count: mesh.opaque_index_count().min(mesh.indices.len() as u32),
            visibility,
        })
    }

    pub fn index_count(&self) -> u32 {
        self.index_count
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

#[derive(Clone, Copy, Debug)]
pub struct ChunkTextureAtlas<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mclone_chunk_texture_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            // Java's AbstractTexture.setFilter(false, true) uses
            // GL_NEAREST_MIPMAP_NEAREST, preserving pixelated blocks while
            // still selecting a lower-detail mip for distant terrain.
            mipmap_filter: wgpu::FilterMode::Nearest,
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
    _texture: wgpu::Texture,
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            _texture: texture,
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
                depth_compare: wgpu::CompareFunction::LessEqual,
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
            depth_compare: wgpu::CompareFunction::LessEqual,
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
            mesh: GpuTexturedChunkMesh::new(device, mesh)
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
        mesh: &TexturedVisibleChunkMesh,
    ) -> Result<()> {
        self.mesh = GpuTexturedChunkMesh::new(device, mesh)
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
            clear_depth: 1.0,
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

pub struct TexturedSectionDrawResources {
    renderer: TexturedChunkRenderer,
    sections: BTreeMap<RenderSectionKey, GpuTexturedChunkMesh>,
    visibility_sections: BTreeMap<RenderSectionKey, VisibilitySet>,
    traversal_ready_sections: BTreeSet<RenderSectionKey>,
    section_set_generation: u64,
    atlas: GpuChunkTextureAtlas,
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
    record_cache_stats: Cell<TexturedSectionRecordCacheStats>,
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
        let renderer = TexturedChunkRenderer::new(device, color_format);
        let atlas =
            GpuChunkTextureAtlas::new(device, queue, &renderer.texture_bind_group_layout, atlas)
                .context("failed to upload chunk texture atlas")?;
        let mut resources = Self {
            renderer,
            sections: BTreeMap::new(),
            visibility_sections: BTreeMap::new(),
            traversal_ready_sections: BTreeSet::new(),
            section_set_generation: 0,
            atlas,
            cached_records: RefCell::new(None),
            records_dirty: Cell::new(true),
            record_cache_stats: Cell::new(TexturedSectionRecordCacheStats::default()),
            cull_scratch: RefCell::new(CullScratch::default()),
        };
        let _ = resources.update_sections(device, sections)?;
        Ok(resources)
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
        if sections.is_empty() && removed.is_empty() {
            return Ok(TexturedSectionUploadReport::default());
        }
        self.section_set_generation = self.section_set_generation.wrapping_add(1);
        // Slice F: the section set / meshes change here, so the cached culling
        // records must be rebuilt on the next prepare.
        self.records_dirty.set(true);
        let mut report = TexturedSectionUploadReport::default();
        for key in removed {
            if self.sections.remove(key).is_some() {
                report.removed_section_count += 1;
            }
            self.visibility_sections.remove(key);
            self.traversal_ready_sections.remove(key);
        }

        for section in sections {
            self.visibility_sections
                .insert(section.key, section.visibility);
            self.traversal_ready_sections.insert(section.key);
            if section.is_empty() {
                if self.sections.remove(&section.key).is_some() {
                    report.removed_section_count += 1;
                }
                continue;
            }
            let stats = section.stats();
            report.uploaded_section_count += 1;
            report.uploaded_vertex_count += stats.vertex_count;
            report.uploaded_index_count += stats.index_count;
            self.sections.insert(
                section.key,
                GpuTexturedChunkMesh::with_visibility(device, &section.mesh, section.visibility)
                    .with_context(|| {
                        format!("failed to upload render section {:?}", section.key)
                    })?,
            );
        }
        Ok(report)
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
        let filtered_ready_sections = self
            .visibility_sections
            .keys()
            .copied()
            .filter(|key| ready_sections.contains(key))
            .collect();
        let mut stats = self.record_cache_stats.get();
        let changed = filtered_ready_sections != self.traversal_ready_sections;
        stats.record_ready_set_call(upload_backpressured, changed);
        if !changed {
            self.record_cache_stats.set(stats);
            return;
        }
        self.record_cache_stats.set(stats);
        // Slice F follow-up: readiness flips change cached `traversal_ready`.
        // Reasserting the same ready set should not rebuild prepared records.
        self.records_dirty.set(true);
        self.traversal_ready_sections = filtered_ready_sections;
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

    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    pub fn index_count(&self) -> u32 {
        self.sections.values().map(|mesh| mesh.index_count()).sum()
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

    pub fn prepare_render_records_with_stats(
        &self,
    ) -> (
        Arc<PreparedTexturedSectionRecords>,
        TexturedSectionRecordPrepareStats,
    ) {
        let mut prepare_stats = TexturedSectionRecordPrepareStats::default();
        if self.records_dirty.get() || self.cached_records.borrow().is_none() {
            let rebuild_start = timing_now();
            let rebuilt = Arc::new(self.build_prepared_records());
            let rebuild_ms = timing_elapsed_ms(rebuild_start);
            *self.cached_records.borrow_mut() = Some(rebuilt);
            self.records_dirty.set(false);
            let mut cache_stats = self.record_cache_stats.get();
            cache_stats.prepared_record_rebuilds += 1;
            cache_stats.prepared_record_rebuild_total_ms += rebuild_ms;
            cache_stats.prepared_record_rebuild_max_ms =
                cache_stats.prepared_record_rebuild_max_ms.max(rebuild_ms);
            self.record_cache_stats.set(cache_stats);
            prepare_stats.rebuilt = true;
            prepare_stats.rebuild_ms = rebuild_ms;
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
        let renderer = self.renderer.multiview_renderer(device)?;
        renderer.write_uniforms(queue, render_views, options, self.renderer.color_format);
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
                ..Default::default()
            });
            pass.set_bind_group(0, &renderer.bind_group, &[]);
            pass.set_bind_group(1, &self.atlas.bind_group, &[]);
            if phase.draws_opaque() {
                pass.set_pipeline(&renderer.solid_pipeline);
                for (key, mesh) in &self.sections {
                    if !prepared_draw.drawn_keys.contains(key) {
                        continue;
                    }
                    draw_textured_mesh_range(&mut pass, mesh, mesh.solid_index_range());
                }
                pass.set_pipeline(&renderer.cutout_pipeline);
                for (key, mesh) in &self.sections {
                    if !prepared_draw.drawn_keys.contains(key) {
                        continue;
                    }
                    draw_textured_mesh_range(&mut pass, mesh, mesh.cutout_index_range());
                }
            }
            if phase.draws_translucent() {
                pass.set_pipeline(&renderer.translucent_pipeline);
                for key in &prepared_draw.translucent_keys {
                    if let Some(mesh) = self.sections.get(key) {
                        draw_textured_mesh_range(&mut pass, mesh, mesh.translucent_index_range());
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
        let records_storage;
        let records: &PreparedTexturedSectionRecords = match prepared_records {
            Some(records) => records,
            None => {
                // Slice F shared with all single-view clients: go through the
                // cross-frame cache instead of rebuilding the records every frame.
                let records_start = timing.as_ref().map(|_| timing_now());
                records_storage = self.prepare_render_records();
                if let (Some(timing), Some(records_start)) = (&mut timing, records_start) {
                    timing.records_ms = timing_elapsed_ms(records_start);
                }
                &records_storage
            }
        };
        let cull_start = timing.as_ref().map(|_| timing_now());
        let culling = {
            let mut scratch = self.cull_scratch.borrow_mut();
            cull_textured_sections(records, render_view, options, &mut scratch)
        };
        if let (Some(timing), Some(cull_start)) = (&mut timing, cull_start) {
            timing.cull_ms = timing_elapsed_ms(cull_start);
        }
        let uniform_start = timing.as_ref().map(|_| timing_now());
        let uniform_offset = self.renderer.uniforms.write_slot(
            queue,
            view_slot,
            &uniform_bytes(render_view, options, self.renderer.color_format),
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
                compare_translucent_sections(**left_key, **right_key, render_view)
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
                ..Default::default()
            });
            pass.set_bind_group(0, &self.renderer.bind_group, &[uniform_offset]);
            pass.set_bind_group(1, &self.atlas.bind_group, &[]);
            if phase.draws_opaque() {
                pass.set_pipeline(&self.renderer.solid_pipeline);
                for (key, mesh) in &self.sections {
                    if !culling.drawn_keys.contains(key) {
                        continue;
                    }
                    draw_textured_mesh_range(&mut pass, mesh, mesh.solid_index_range());
                }
                pass.set_pipeline(&self.renderer.cutout_pipeline);
                for (key, mesh) in &self.sections {
                    if !culling.drawn_keys.contains(key) {
                        continue;
                    }
                    draw_textured_mesh_range(&mut pass, mesh, mesh.cutout_index_range());
                }
            }
            if phase.draws_translucent() {
                pass.set_pipeline(&self.renderer.translucent_pipeline);
                for (_, mesh) in translucent_sections {
                    draw_textured_mesh_range(&mut pass, mesh, mesh.translucent_index_range());
                }
            }
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
        translucent_keys
            .sort_by(|left, right| compare_translucent_sections(*left, *right, sort_view));
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
        let uniform_offset = self.renderer.uniforms.write_slot(
            queue,
            view_slot,
            &uniform_bytes(render_view, options, self.renderer.color_format),
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
                ..Default::default()
            });
            pass.set_bind_group(0, &self.renderer.bind_group, &[uniform_offset]);
            pass.set_bind_group(1, &self.atlas.bind_group, &[]);
            if phase.draws_opaque() {
                pass.set_pipeline(&self.renderer.solid_pipeline);
                for (key, mesh) in &self.sections {
                    if !prepared_draw.draws_in_slot(*key, view_slot) {
                        continue;
                    }
                    draw_textured_mesh_range(&mut pass, mesh, mesh.solid_index_range());
                }
                pass.set_pipeline(&self.renderer.cutout_pipeline);
                for (key, mesh) in &self.sections {
                    if !prepared_draw.draws_in_slot(*key, view_slot) {
                        continue;
                    }
                    draw_textured_mesh_range(&mut pass, mesh, mesh.cutout_index_range());
                }
            }
            if phase.draws_translucent() {
                pass.set_pipeline(&self.renderer.translucent_pipeline);
                for key in &prepared_draw.translucent_keys {
                    if !prepared_draw.draws_in_slot(*key, view_slot) {
                        continue;
                    }
                    if let Some(mesh) = self.sections.get(key) {
                        draw_textured_mesh_range(&mut pass, mesh, mesh.translucent_index_range());
                    }
                }
            }
        }
        if let (Some(timing), Some(encode_start)) = (&mut timing, encode_start) {
            timing.encode_ms = timing_elapsed_ms(encode_start);
        }
        Ok(prepared_draw.stats_for_slot(view_slot))
    }

    fn build_prepared_records(&self) -> PreparedTexturedSectionRecords {
        let mut records = BTreeMap::new();
        let mut loaded_section_count = 0usize;
        let mut loaded_index_count = 0u32;
        for (key, visibility) in &self.visibility_sections {
            let mesh = self.sections.get(key);
            let index_count = mesh.map_or(0, GpuTexturedChunkMesh::index_count);
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
) -> Ordering {
    let left_depth = section_depth_along_view(left, render_view);
    let right_depth = section_depth_along_view(right, render_view);
    right_depth
        .partial_cmp(&left_depth)
        .unwrap_or(Ordering::Equal)
        .then_with(|| right.cmp(&left))
}

fn section_depth_along_view(key: RenderSectionKey, render_view: ChunkRenderView) -> f32 {
    let center = Vec3::new(
        chunk_min_block_coord(key.chunk_x) as f32 + MESH_CHUNK_WIDTH as f32 * 0.5,
        key.min_y() as f32 + RENDER_SECTION_HEIGHT as f32 * 0.5,
        chunk_min_block_coord(key.chunk_z) as f32 + MESH_CHUNK_WIDTH as f32 * 0.5,
    );
    (center - render_view.camera_position).dot(render_view.camera_forward)
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
    for (index, value) in [options.fog.start, options.fog.end, 0.0, 0.0]
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

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_mesh::{
        ChunkMeshInput, TexturedChunkVertex, TexturedVisibleChunkMesh, build_visible_chunk_mesh,
    };

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
    ) -> TexturedRenderSectionMesh {
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
    }

    fn prepared_records_for_sections(
        sections: &[TexturedRenderSectionMesh],
    ) -> PreparedTexturedSectionRecords {
        let records = sections
            .iter()
            .map(|section| {
                (
                    section.key,
                    TexturedSectionCullingRecord {
                        index_count: section.stats().index_count,
                        visibility: section.visibility,
                        drawable: !section.is_empty(),
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
        let frustum = ClipFrustum::from_render_view(camera.render_view(1280, 720));

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
        let frustum = ClipFrustum::from_render_view(camera.render_view(1280, 720));

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
        let projection = Mat4::perspective_rh(fov_y_radians, aspect, z_near, z_far);
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
