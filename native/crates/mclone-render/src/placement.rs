use anyhow::{Result, bail};
use glam::{Mat4, Vec3};
use mclone_core::{Aabb, ChunkPos, Vec3d};
use mclone_mesh::RenderSectionKey;

use crate::chunk::ChunkRenderView;

/// A finite, non-empty source-space AABB used to bound one presentation.
///
/// Bounds are minimum-inclusive and maximum-exclusive. They describe what a
/// presentation may draw; they do not describe runtime loading or simulation
/// interest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldSourceBounds {
    min: Vec3d,
    max: Vec3d,
}

impl WorldSourceBounds {
    pub fn new(min: Vec3d, max: Vec3d) -> Result<Self> {
        if !min.is_finite() || !max.is_finite() {
            bail!("world source bounds must be finite");
        }
        if min.x >= max.x || min.y >= max.y || min.z >= max.z {
            bail!("world source bounds must be non-empty on every axis");
        }
        Ok(Self { min, max })
    }

    pub fn min(self) -> Vec3d {
        self.min
    }

    pub fn max(self) -> Vec3d {
        self.max
    }

    pub fn contains(self, point: Vec3d) -> bool {
        point.x >= self.min.x
            && point.y >= self.min.y
            && point.z >= self.min.z
            && point.x < self.max.x
            && point.y < self.max.y
            && point.z < self.max.z
    }

    pub fn contains_render_section(self, key: RenderSectionKey) -> bool {
        let min = Vec3d::new(
            f64::from(key.chunk_x) * 16.0,
            f64::from(key.section_y) * 16.0,
            f64::from(key.chunk_z) * 16.0,
        );
        let max = min.add(Vec3d::new(16.0, 16.0, 16.0));
        min.x >= self.min.x
            && min.y >= self.min.y
            && min.z >= self.min.z
            && max.x <= self.max.x
            && max.y <= self.max.y
            && max.z <= self.max.z
    }

    pub fn source_to_composition_aabb(self, placement: WorldPlacement) -> Aabb {
        // WorldPlacement is a positive uniform scale plus translation, so it
        // preserves axis ordering and maps an AABB to an AABB exactly.
        let min = placement.source_to_composition(self.min);
        let max = placement.source_to_composition(self.max);
        Aabb::new(min.x, min.y, min.z, max.x, max.y, max.z)
    }
}

/// One normalized composition-space plane.
///
/// Points with `dot(normal, point) + offset >= 0` are retained. The plane
/// boundary is deliberately inclusive; complementary planes therefore share
/// the measure-zero seam and neither drops fragments because of the CPU-side
/// convention.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompositionHalfSpace {
    normal: Vec3,
    offset: f32,
}

impl CompositionHalfSpace {
    pub fn new(normal: Vec3, offset: f32) -> Result<Self> {
        if !normal.is_finite() || !offset.is_finite() {
            bail!("composition half-space plane must be finite");
        }
        let length = normal.length();
        if !length.is_finite() || length <= f32::EPSILON {
            bail!("composition half-space normal must be nonzero");
        }
        Ok(Self {
            normal: normal / length,
            offset: offset / length,
        })
    }

    pub fn normal(self) -> Vec3 {
        self.normal
    }

    pub fn offset(self) -> f32 {
        self.offset
    }

    pub fn signed_distance(self, composition_position: Vec3) -> f32 {
        self.normal.dot(composition_position) + self.offset
    }

    pub fn retains(self, composition_position: Vec3) -> bool {
        self.signed_distance(composition_position) >= 0.0
    }

    /// Return true only when every point in an axis-aligned box is outside
    /// this half-space. Intersecting boxes remain GPU work so the fragment
    /// clip can resolve the exact plane.
    pub fn rejects_aabb(self, min: Vec3, max: Vec3) -> bool {
        let farthest = Vec3::new(
            if self.normal.x >= 0.0 { max.x } else { min.x },
            if self.normal.y >= 0.0 { max.y } else { min.y },
            if self.normal.z >= 0.0 { max.z } else { min.z },
        );
        self.signed_distance(farthest) < 0.0
    }

    pub fn complementary(self) -> Self {
        Self {
            normal: -self.normal,
            offset: -self.offset,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CompositionClip {
    #[default]
    Unbounded,
    HalfSpace(CompositionHalfSpace),
}

impl CompositionClip {
    pub fn retains(self, composition_position: Vec3) -> bool {
        match self {
            Self::Unbounded => true,
            Self::HalfSpace(half_space) => half_space.retains(composition_position),
        }
    }

    pub fn rejects_aabb(self, min: Vec3, max: Vec3) -> bool {
        match self {
            Self::Unbounded => false,
            Self::HalfSpace(half_space) => half_space.rejects_aabb(min, max),
        }
    }
}

/// A finite, positive uniform placement of one world's coordinates in a
/// composition world's coordinates.
///
/// Anchors and mapping math stay in `f64`. Renderers upload the two anchors and
/// scale separately so source positions are rebased before scaling; ordinary
/// terrain does not acquire a model matrix or a second origin convention.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldPlacement {
    source_anchor: Vec3d,
    composition_anchor: Vec3d,
    uniform_scale: f64,
}

impl WorldPlacement {
    pub fn new(
        source_anchor: Vec3d,
        composition_anchor: Vec3d,
        uniform_scale: f64,
    ) -> Result<Self> {
        if !source_anchor.is_finite() {
            bail!("world placement source anchor must be finite");
        }
        if !composition_anchor.is_finite() {
            bail!("world placement composition anchor must be finite");
        }
        if !uniform_scale.is_finite() || uniform_scale <= 0.0 {
            bail!("world placement scale must be finite and positive");
        }
        Ok(Self {
            source_anchor,
            composition_anchor,
            uniform_scale,
        })
    }

    pub fn identity() -> Self {
        Self {
            source_anchor: Vec3d::ZERO,
            composition_anchor: Vec3d::ZERO,
            uniform_scale: 1.0,
        }
    }

    pub fn source_anchor(self) -> Vec3d {
        self.source_anchor
    }

    pub fn composition_anchor(self) -> Vec3d {
        self.composition_anchor
    }

    pub fn uniform_scale(self) -> f64 {
        self.uniform_scale
    }

    pub fn source_to_composition(self, source: Vec3d) -> Vec3d {
        self.composition_anchor.add(
            source
                .subtract(self.source_anchor)
                .scale(self.uniform_scale),
        )
    }

    pub fn composition_to_source(self, composition: Vec3d) -> Vec3d {
        self.source_anchor.add(
            composition
                .subtract(self.composition_anchor)
                .scale(1.0 / self.uniform_scale),
        )
    }

    /// Derive the source-local view used for section traversal and culling.
    /// The physical projection remains unchanged; positive uniform placement
    /// does not rotate the camera basis.
    pub fn source_render_view(self, physical: ChunkRenderView) -> ChunkRenderView {
        let model = self.source_to_composition_matrix_f32();
        let source_eye = self.composition_to_source(Vec3d::new(
            f64::from(physical.camera_position.x),
            f64::from(physical.camera_position.y),
            f64::from(physical.camera_position.z),
        ));
        let view = physical.view * model;
        ChunkRenderView {
            view,
            view_projection: physical.projection * view,
            camera_position: Vec3::new(
                source_eye.x as f32,
                source_eye.y as f32,
                source_eye.z as f32,
            ),
            ..physical
        }
    }

    /// The exact `f32` anchor values uploaded to the placed terrain shaders.
    pub(crate) fn shader_values(self) -> ([f32; 4], [f32; 4]) {
        (
            [
                self.source_anchor.x as f32,
                self.source_anchor.y as f32,
                self.source_anchor.z as f32,
                self.uniform_scale as f32,
            ],
            [
                self.composition_anchor.x as f32,
                self.composition_anchor.y as f32,
                self.composition_anchor.z as f32,
                0.0,
            ],
        )
    }

    /// Apply the exact rebased `f32` mapping used by placed shaders.
    ///
    /// Renderer CPU rejection uses this instead of casting the completed
    /// `f64` result so far-away source anchors agree with shader arithmetic.
    pub(crate) fn source_to_composition_f32(self, source: Vec3) -> Vec3 {
        let (source_anchor_scale, composition_anchor) = self.shader_values();
        Vec3::from_array([
            composition_anchor[0],
            composition_anchor[1],
            composition_anchor[2],
        ]) + (source
            - Vec3::from_array([
                source_anchor_scale[0],
                source_anchor_scale[1],
                source_anchor_scale[2],
            ]))
            * source_anchor_scale[3]
    }

    pub(crate) fn source_to_composition_matrix_f32(self) -> Mat4 {
        let (source, composition) = self.shader_values();
        let scale = source[3];
        let translation = Vec3::new(
            composition[0] - source[0] * scale,
            composition[1] - source[1] * scale,
            composition[2] - source[2] * scale,
        );
        Mat4::from_translation(translation) * Mat4::from_scale(Vec3::splat(scale))
    }
}

/// Complete renderer-facing context for one opt-in placed-world submission.
///
/// Runtime interest remains owned by the session/runtime. The renderer sees
/// only placement, an optional coarse source-space presentation bound, and an
/// exact composition-space clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldCompositionContext {
    placement: WorldPlacement,
    source_bounds: Option<WorldSourceBounds>,
    clip: CompositionClip,
}

impl WorldCompositionContext {
    pub fn new(
        placement: WorldPlacement,
        source_bounds: Option<WorldSourceBounds>,
        clip: CompositionClip,
    ) -> Self {
        Self {
            placement,
            source_bounds,
            clip,
        }
    }

    pub fn unbounded(placement: WorldPlacement, source_bounds: Option<WorldSourceBounds>) -> Self {
        Self::new(placement, source_bounds, CompositionClip::Unbounded)
    }

    pub fn placement(self) -> WorldPlacement {
        self.placement
    }

    pub fn source_bounds(self) -> Option<WorldSourceBounds> {
        self.source_bounds
    }

    pub fn composition_bounds(self) -> Option<Aabb> {
        self.source_bounds
            .map(|bounds| bounds.source_to_composition_aabb(self.placement))
    }

    pub fn clip(self) -> CompositionClip {
        self.clip
    }

    pub fn source_to_composition(self, source: Vec3d) -> Vec3d {
        self.placement.source_to_composition(source)
    }

    pub fn composition_to_source(self, composition: Vec3d) -> Vec3d {
        self.placement.composition_to_source(composition)
    }

    pub fn source_render_view(self, physical: ChunkRenderView) -> ChunkRenderView {
        self.placement.source_render_view(physical)
    }
}

/// Inclusive, section-aligned source bounds for an embedded terrain draw.
///
/// Rectangular chunk bounds are the canonical representation. The
/// center-plus-radius constructor remains convenience sugar for older square
/// fixtures, but product previews may use even spans such as 2x2 or 4x4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedChunkRegion {
    min_chunk: ChunkPos,
    max_chunk: ChunkPos,
    min_section_y: i32,
    max_section_y: i32,
}

impl EmbeddedChunkRegion {
    pub fn new(
        center: ChunkPos,
        horizontal_radius: u32,
        min_section_y: i32,
        max_section_y: i32,
    ) -> Result<Self> {
        let radius = i64::from(horizontal_radius);
        let min_chunk_x = i64::from(center.x) - radius;
        let max_chunk_x = i64::from(center.x) + radius;
        let min_chunk_z = i64::from(center.z) - radius;
        let max_chunk_z = i64::from(center.z) + radius;
        let [min_chunk_x, max_chunk_x, min_chunk_z, max_chunk_z] =
            [min_chunk_x, max_chunk_x, min_chunk_z, max_chunk_z].map(|value| {
                i32::try_from(value)
                    .map_err(|_| anyhow::anyhow!("embedded chunk radius exceeds coordinate range"))
            });
        Self::from_chunk_bounds(
            ChunkPos::new(min_chunk_x?, min_chunk_z?),
            ChunkPos::new(max_chunk_x?, max_chunk_z?),
            min_section_y,
            max_section_y,
        )
    }

    pub fn from_chunk_bounds(
        min_chunk: ChunkPos,
        max_chunk: ChunkPos,
        min_section_y: i32,
        max_section_y: i32,
    ) -> Result<Self> {
        if min_chunk.x > max_chunk.x || min_chunk.z > max_chunk.z {
            bail!("embedded chunk region minimum chunk must not exceed maximum");
        }
        if min_section_y > max_section_y {
            bail!("embedded chunk region minimum section must not exceed maximum");
        }
        Ok(Self {
            min_chunk,
            max_chunk,
            min_section_y,
            max_section_y,
        })
    }

    pub fn center(self) -> ChunkPos {
        ChunkPos::new(
            midpoint_i32(self.min_chunk.x, self.max_chunk.x),
            midpoint_i32(self.min_chunk.z, self.max_chunk.z),
        )
    }

    pub fn min_chunk(self) -> ChunkPos {
        self.min_chunk
    }

    pub fn max_chunk(self) -> ChunkPos {
        self.max_chunk
    }

    pub fn chunk_width(self) -> u32 {
        inclusive_i32_span(self.min_chunk.x, self.max_chunk.x)
    }

    pub fn chunk_depth(self) -> u32 {
        inclusive_i32_span(self.min_chunk.z, self.max_chunk.z)
    }

    pub fn symmetric_horizontal_radius(self) -> Option<u32> {
        let center = self.center();
        let x_min = i64::from(center.x) - i64::from(self.min_chunk.x);
        let x_max = i64::from(self.max_chunk.x) - i64::from(center.x);
        let z_min = i64::from(center.z) - i64::from(self.min_chunk.z);
        let z_max = i64::from(self.max_chunk.z) - i64::from(center.z);
        (x_min == x_max && x_min == z_min && x_min == z_max)
            .then(|| u32::try_from(x_min).expect("nonnegative i32 radius fits u32"))
    }

    pub fn min_section_y(self) -> i32 {
        self.min_section_y
    }

    pub fn max_section_y(self) -> i32 {
        self.max_section_y
    }

    pub fn contains(self, key: RenderSectionKey) -> bool {
        key.chunk_x >= self.min_chunk.x
            && key.chunk_x <= self.max_chunk.x
            && key.chunk_z >= self.min_chunk.z
            && key.chunk_z <= self.max_chunk.z
            && key.section_y >= self.min_section_y
            && key.section_y <= self.max_section_y
    }

    /// Derive the section-aligned presentation AABB. This conversion does not
    /// transfer runtime interest ownership into renderer vocabulary.
    pub fn source_bounds(self) -> WorldSourceBounds {
        let min_chunk_x = i64::from(self.min_chunk.x);
        let max_chunk_x_exclusive = i64::from(self.max_chunk.x) + 1;
        let min_chunk_z = i64::from(self.min_chunk.z);
        let max_chunk_z_exclusive = i64::from(self.max_chunk.z) + 1;
        let min_section_y = i64::from(self.min_section_y);
        let max_section_y_exclusive = i64::from(self.max_section_y) + 1;
        WorldSourceBounds::new(
            Vec3d::new(
                (min_chunk_x * 16) as f64,
                (min_section_y * 16) as f64,
                (min_chunk_z * 16) as f64,
            ),
            Vec3d::new(
                (max_chunk_x_exclusive * 16) as f64,
                (max_section_y_exclusive * 16) as f64,
                (max_chunk_z_exclusive * 16) as f64,
            ),
        )
        .expect("a valid section region always produces finite non-empty bounds")
    }
}

fn midpoint_i32(min: i32, max: i32) -> i32 {
    let midpoint = i64::from(min) + (i64::from(max) - i64::from(min)) / 2;
    i32::try_from(midpoint).expect("midpoint of two i32 values fits i32")
}

fn inclusive_i32_span(min: i32, max: i32) -> u32 {
    u32::try_from(i64::from(max) - i64::from(min) + 1)
        .expect("validated inclusive i32 span fits u32")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_vec3d_close(actual: Vec3d, expected: Vec3d) {
        assert!((actual.x - expected.x).abs() < 1.0e-9, "x: {actual:?}");
        assert!((actual.y - expected.y).abs() < 1.0e-9, "y: {actual:?}");
        assert!((actual.z - expected.z).abs() < 1.0e-9, "z: {actual:?}");
    }

    #[test]
    fn placement_round_trips_with_distant_source_anchor() {
        let placement = WorldPlacement::new(
            Vec3d::new(30_000_008.0, -2_048.0, -29_999_992.0),
            Vec3d::new(12.5, 65.25, -3.75),
            1.0 / 16.0,
        )
        .unwrap();
        let source = Vec3d::new(30_000_024.0, -2_016.0, -30_000_008.0);
        let composition = placement.source_to_composition(source);

        assert_vec3d_close(composition, Vec3d::new(13.5, 67.25, -4.75));
        assert_vec3d_close(placement.composition_to_source(composition), source);
        let (source_uniform, composition_uniform) = placement.shader_values();
        assert_eq!(source_uniform[3], 1.0 / 16.0);
        assert_eq!(composition_uniform, [12.5, 65.25, -3.75, 0.0]);
    }

    #[test]
    fn placement_rejects_invalid_values() {
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(WorldPlacement::new(Vec3d::ZERO, Vec3d::ZERO, scale).is_err());
        }
        assert!(WorldPlacement::new(Vec3d::new(f64::NAN, 0.0, 0.0), Vec3d::ZERO, 1.0).is_err());
        assert!(
            WorldPlacement::new(Vec3d::ZERO, Vec3d::new(0.0, f64::INFINITY, 0.0), 1.0).is_err()
        );
    }

    #[test]
    fn region_handles_negative_chunks_and_inclusive_vertical_bounds() {
        let region = EmbeddedChunkRegion::new(ChunkPos::new(-4, -7), 2, -3, 5).unwrap();

        assert_eq!(region.min_chunk(), ChunkPos::new(-6, -9));
        assert_eq!(region.max_chunk(), ChunkPos::new(-2, -5));
        assert_eq!(region.chunk_width(), 5);
        assert_eq!(region.chunk_depth(), 5);
        assert_eq!(region.symmetric_horizontal_radius(), Some(2));
        assert!(region.contains(RenderSectionKey::new(-6, -3, -9)));
        assert!(region.contains(RenderSectionKey::new(-2, 5, -5)));
        assert!(!region.contains(RenderSectionKey::new(-7, 0, -7)));
        assert!(!region.contains(RenderSectionKey::new(-4, -4, -7)));
        assert!(!region.contains(RenderSectionKey::new(-4, 6, -7)));
    }

    #[test]
    fn invalid_vertical_region_is_rejected() {
        assert!(EmbeddedChunkRegion::new(ChunkPos::new(0, 0), 0, 2, 1).is_err());
        assert!(
            EmbeddedChunkRegion::from_chunk_bounds(ChunkPos::new(1, 0), ChunkPos::new(0, 1), 0, 1,)
                .is_err()
        );
    }

    #[test]
    fn exact_even_rectangles_are_not_coerced_to_radius_squares() {
        let two =
            EmbeddedChunkRegion::from_chunk_bounds(ChunkPos::new(-1, 4), ChunkPos::new(0, 5), 3, 6)
                .unwrap();
        let four = EmbeddedChunkRegion::from_chunk_bounds(
            ChunkPos::new(8, -5),
            ChunkPos::new(11, -2),
            2,
            7,
        )
        .unwrap();

        assert_eq!((two.chunk_width(), two.chunk_depth()), (2, 2));
        assert_eq!((four.chunk_width(), four.chunk_depth()), (4, 4));
        assert_eq!(two.symmetric_horizontal_radius(), None);
        assert_eq!(four.symmetric_horizontal_radius(), None);
        assert!(two.contains(RenderSectionKey::new(-1, 3, 4)));
        assert!(two.contains(RenderSectionKey::new(0, 6, 5)));
        assert!(!two.contains(RenderSectionKey::new(1, 4, 5)));
        assert_eq!(two.source_bounds().min(), Vec3d::new(-16.0, 48.0, 64.0));
        assert_eq!(two.source_bounds().max(), Vec3d::new(16.0, 112.0, 96.0));
    }

    #[test]
    fn source_bounds_are_finite_non_empty_and_min_inclusive_max_exclusive() {
        assert!(WorldSourceBounds::new(Vec3d::ZERO, Vec3d::ZERO).is_err());
        assert!(WorldSourceBounds::new(Vec3d::ZERO, Vec3d::new(f64::INFINITY, 1.0, 1.0)).is_err());
        let bounds =
            WorldSourceBounds::new(Vec3d::new(-16.0, -32.0, 48.0), Vec3d::new(0.0, 16.0, 80.0))
                .unwrap();
        assert!(bounds.contains(bounds.min()));
        assert!(bounds.contains(Vec3d::new(-0.001, 15.999, 79.999)));
        assert!(!bounds.contains(bounds.max()));
        assert!(!bounds.contains(Vec3d::new(0.0, 0.0, 64.0)));
    }

    #[test]
    fn half_space_normalizes_and_uses_an_inclusive_boundary() {
        for normal in [Vec3::ZERO, Vec3::splat(f32::NAN)] {
            assert!(CompositionHalfSpace::new(normal, 0.0).is_err());
        }
        assert!(CompositionHalfSpace::new(Vec3::X, f32::INFINITY).is_err());
        let half_space = CompositionHalfSpace::new(Vec3::new(2.0, 0.0, 0.0), -4.0).unwrap();
        assert_eq!(half_space.normal(), Vec3::X);
        assert_eq!(half_space.offset(), -2.0);
        assert!(half_space.retains(Vec3::new(2.0, 100.0, -100.0)));
        assert!(half_space.retains(Vec3::new(3.0, 0.0, 0.0)));
        assert!(!half_space.retains(Vec3::new(1.0, 0.0, 0.0)));
        let complement = half_space.complementary();
        assert!(complement.retains(Vec3::new(2.0, 0.0, 0.0)));
        assert!(!complement.retains(Vec3::new(3.0, 0.0, 0.0)));
        assert!(complement.retains(Vec3::new(1.0, 0.0, 0.0)));
        assert!(!half_space.rejects_aabb(Vec3::new(1.0, -1.0, -1.0), Vec3::new(2.0, 1.0, 1.0),));
        assert!(half_space.rejects_aabb(Vec3::new(-4.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0),));
    }

    #[test]
    fn region_bounds_match_section_membership_at_large_coordinates() {
        let region = EmbeddedChunkRegion::from_chunk_bounds(
            ChunkPos::new(i32::MAX - 8, i32::MIN),
            ChunkPos::new(i32::MAX, i32::MIN + 8),
            i32::MIN + 2,
            i32::MIN + 5,
        )
        .unwrap();
        let bounds = region.source_bounds();
        for key in [
            RenderSectionKey::new(i32::MAX - 8, i32::MIN + 2, i32::MIN),
            RenderSectionKey::new(i32::MAX, i32::MIN + 5, i32::MIN + 8),
            RenderSectionKey::new(i32::MAX - 9, i32::MIN + 3, i32::MIN + 4),
        ] {
            assert_eq!(bounds.contains_render_section(key), region.contains(key));
        }
    }

    #[test]
    fn composition_context_transforms_bounds_and_preserves_camera_facts_per_view() {
        let placement = WorldPlacement::new(
            Vec3d::new(30_000_000.0, -4_096.0, -30_000_000.0),
            Vec3d::new(8.0, 64.0, -12.0),
            0.25,
        )
        .unwrap();
        let bounds = WorldSourceBounds::new(
            Vec3d::new(29_999_984.0, -4_112.0, -30_000_032.0),
            Vec3d::new(30_000_032.0, -4_048.0, -29_999_984.0),
        )
        .unwrap();
        let context = WorldCompositionContext::unbounded(placement, Some(bounds));
        let transformed = context.composition_bounds().unwrap();
        assert_vec3d_close(
            Vec3d::new(transformed.min_x, transformed.min_y, transformed.min_z),
            Vec3d::new(4.0, 60.0, -20.0),
        );
        assert_vec3d_close(
            Vec3d::new(transformed.max_x, transformed.max_y, transformed.max_z),
            Vec3d::new(16.0, 76.0, -8.0),
        );
        assert_vec3d_close(
            context.composition_to_source(context.source_to_composition(bounds.min())),
            bounds.min(),
        );

        let make_view = |camera_position, projection| ChunkRenderView {
            view: Mat4::IDENTITY,
            projection,
            view_projection: projection,
            camera_position,
            camera_forward: -Vec3::Z,
            camera_right: Vec3::X,
            camera_up: Vec3::Y,
            aspect: 1.0,
            fov_y_radians: 1.0,
            z_near: 0.1,
            z_far: 1_000.0,
            projection_kind: crate::chunk::ChunkProjectionKind::External,
        };
        let views = [
            make_view(Vec3::new(8.0, 64.0, -12.0), Mat4::IDENTITY),
            make_view(
                Vec3::new(8.25, 64.0, -12.0),
                Mat4::from_scale(Vec3::new(1.1, 1.2, 1.0)),
            ),
        ];
        for view in views {
            let source_view = context.source_render_view(view);
            assert_eq!(source_view.projection, view.projection);
            let expected = placement.composition_to_source(Vec3d::new(
                f64::from(view.camera_position.x),
                f64::from(view.camera_position.y),
                f64::from(view.camera_position.z),
            ));
            assert_eq!(
                source_view.camera_position,
                Vec3::new(expected.x as f32, expected.y as f32, expected.z as f32),
            );
        }
    }
}
