use std::collections::{BTreeMap, BTreeSet, VecDeque};

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
use wgpu::util::DeviceExt;

use crate::target::RenderFrameTarget;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

const VERTEX_FLOAT_COUNT: usize = 7;
const VERTEX_BYTE_SIZE: wgpu::BufferAddress =
    (VERTEX_FLOAT_COUNT * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const TEXTURED_VERTEX_BYTE_SIZE: wgpu::BufferAddress = 40;
const UNIFORM_BYTE_SIZE: wgpu::BufferAddress = 80;

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
}

impl ChunkRenderView {
    pub fn uniform_matrix(self) -> [[f32; 4]; 4] {
        self.view_projection.to_cols_array_2d()
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

    fn color_load_op(self) -> wgpu::LoadOp<wgpu::Color> {
        if self.load_color {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(self.clear_color)
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexturedSectionRenderOptions {
    pub section_occlusion_culling: bool,
    pub force_fullbright: bool,
    pub sky_darken: f32,
}

impl Default for TexturedSectionRenderOptions {
    fn default() -> Self {
        Self {
            section_occlusion_culling: true,
            force_fullbright: false,
            sky_darken: 1.0,
        }
    }
}

impl TexturedSectionRenderOptions {
    pub fn with_sky_darken(mut self, sky_darken: f32) -> Self {
        self.sky_darken = sky_darken.clamp(0.0, 1.0);
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
    cull_textured_sections(&records, render_view, options).stats
}

#[derive(Clone, Copy, Debug)]
struct TexturedSectionCullingRecord {
    index_count: u32,
    visibility: VisibilitySet,
    drawable: bool,
    traversal_ready: bool,
}

#[derive(Clone, Debug)]
struct TexturedSectionCullingResult {
    stats: TexturedSectionRenderStats,
    drawn_keys: BTreeSet<RenderSectionKey>,
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
    records: &BTreeMap<RenderSectionKey, TexturedSectionCullingRecord>,
    render_view: ChunkRenderView,
    options: TexturedSectionRenderOptions,
) -> TexturedSectionCullingResult {
    let frustum = ClipFrustum::from_render_view(render_view);
    let mut stats = TexturedSectionRenderStats {
        section_occlusion_culling: options.section_occlusion_culling,
        force_fullbright: options.force_fullbright,
        loaded_section_count: records.values().filter(|record| record.drawable).count(),
        loaded_index_count: records
            .values()
            .filter(|record| record.drawable)
            .map(|record| record.index_count)
            .sum(),
        ..TexturedSectionRenderStats::default()
    };

    let mut frustum_keys = BTreeSet::new();
    let mut ready_frustum_keys = BTreeSet::new();
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

    let start_keys = traversal_start_keys(records, &frustum_keys, render_view.camera_position);
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
            drawn_keys: ready_frustum_keys,
        };
    }

    let mut queue = VecDeque::new();
    let mut infos = BTreeMap::new();
    let mut drawn_keys = BTreeSet::new();
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

fn traversal_start_keys(
    records: &BTreeMap<RenderSectionKey, TexturedSectionCullingRecord>,
    frustum_keys: &BTreeSet<RenderSectionKey>,
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
    frustum_keys: &BTreeSet<RenderSectionKey>,
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
            visibility,
        })
    }

    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    pub fn visibility(&self) -> VisibilitySet {
        self.visibility
    }
}

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

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_chunk_texture_atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: Default::default(),
                aspect: Default::default(),
            },
            atlas.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mclone_chunk_texture_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
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

pub struct ChunkRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl ChunkRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_chunk_flat_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/chunk_flat.wgsl").into()),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_chunk_uniforms"),
            size: UNIFORM_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_chunk_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_chunk_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
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
            uniform_buffer,
            bind_group,
        }
    }
}

pub struct TexturedChunkRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
}

impl TexturedChunkRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_chunk_textured_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/chunk_textured.wgsl").into()),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_textured_chunk_uniforms"),
            size: UNIFORM_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_textured_chunk_uniform_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_textured_chunk_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
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
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_textured_chunk_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
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
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState {
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
                    }),
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
            uniform_buffer,
            bind_group,
            texture_bind_group_layout,
        }
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
        queue.write_buffer(
            &self.renderer.uniform_buffer,
            0,
            &uniform_bytes(
                render_view.uniform_matrix(),
                TexturedSectionRenderOptions::default(),
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
                    load: wgpu::LoadOp::Clear(target.clear_depth),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.renderer.pipeline);
        pass.set_bind_group(0, &self.renderer.bind_group, &[]);
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
        queue.write_buffer(
            &self.renderer.uniform_buffer,
            0,
            &uniform_bytes(
                render_view.uniform_matrix(),
                TexturedSectionRenderOptions::default(),
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
                    load: wgpu::LoadOp::Clear(target.clear_depth),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.renderer.pipeline);
        pass.set_bind_group(0, &self.renderer.bind_group, &[]);
        pass.set_bind_group(1, &self.atlas.bind_group, &[]);
        pass.set_vertex_buffer(0, self.mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(self.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.mesh.index_count, 0, 0..1);
        Ok(())
    }
}

pub struct TexturedSectionDrawResources {
    renderer: TexturedChunkRenderer,
    sections: BTreeMap<RenderSectionKey, GpuTexturedChunkMesh>,
    visibility_sections: BTreeMap<RenderSectionKey, VisibilitySet>,
    traversal_ready_sections: BTreeSet<RenderSectionKey>,
    atlas: GpuChunkTextureAtlas,
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
            atlas,
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
        self.traversal_ready_sections = self
            .visibility_sections
            .keys()
            .copied()
            .filter(|key| ready_sections.contains(key))
            .collect();
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
        self.render_with_options(
            queue,
            encoder,
            target,
            render_view,
            TexturedSectionRenderOptions::default(),
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
        let records = self
            .visibility_sections
            .iter()
            .map(|(key, visibility)| {
                let mesh = self.sections.get(key);
                (
                    *key,
                    TexturedSectionCullingRecord {
                        index_count: mesh.map_or(0, GpuTexturedChunkMesh::index_count),
                        visibility: *visibility,
                        drawable: mesh.is_some(),
                        traversal_ready: self.traversal_ready_sections.contains(key),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let culling = cull_textured_sections(&records, render_view, options);
        queue.write_buffer(
            &self.renderer.uniform_buffer,
            0,
            &uniform_bytes(render_view.uniform_matrix(), options),
        );

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
                    load: wgpu::LoadOp::Clear(target.clear_depth),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.renderer.pipeline);
        pass.set_bind_group(0, &self.renderer.bind_group, &[]);
        pass.set_bind_group(1, &self.atlas.bind_group, &[]);
        for (key, mesh) in &self.sections {
            if !culling.drawn_keys.contains(key) {
                continue;
            }
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
        Ok(culling.stats)
    }
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

fn uniform_bytes(matrix: [[f32; 4]; 4], options: TexturedSectionRenderOptions) -> [u8; 80] {
    let mut bytes = [0; 80];
    bytes[..64].copy_from_slice(&matrix_bytes(matrix));
    let render_options = [
        if options.force_fullbright {
            1.0_f32
        } else {
            0.0
        },
        options.sky_darken.clamp(0.0, 1.0),
        0.0,
        0.0,
    ];
    for (index, value) in render_options.into_iter().enumerate() {
        let start = 64 + index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
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
            uniform_bytes(matrix, TexturedSectionRenderOptions::default()).len()
                as wgpu::BufferAddress,
            UNIFORM_BYTE_SIZE
        );
        assert!(matrix.into_iter().flatten().all(f32::is_finite));
        assert!(render_view.view_projection.is_finite());
    }

    #[test]
    fn textured_render_options_serialize_fullbright_and_sky_darken() {
        let bytes = uniform_bytes(
            [[0.0; 4]; 4],
            TexturedSectionRenderOptions {
                force_fullbright: true,
                sky_darken: 0.25,
                ..TexturedSectionRenderOptions::default()
            },
        );

        assert_eq!(f32::from_ne_bytes(bytes[64..68].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_ne_bytes(bytes[68..72].try_into().unwrap()), 0.25);
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
            },
            visibility,
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
        };

        assert_eq!(
            textured_vertex_bytes(&mesh).len() as wgpu::BufferAddress,
            TEXTURED_VERTEX_BYTE_SIZE
        );
    }
}
