use anyhow::{Result, bail};
use glam::{Mat4, Vec3};
use mclone_core::{ChunkPos, Vec3d};
use mclone_mesh::RenderSectionKey;

use crate::chunk::ChunkRenderView;

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

/// Inclusive, section-aligned source bounds for an embedded terrain draw.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedChunkRegion {
    center: ChunkPos,
    horizontal_radius: u32,
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
        if min_section_y > max_section_y {
            bail!("embedded chunk region minimum section must not exceed maximum");
        }
        Ok(Self {
            center,
            horizontal_radius,
            min_section_y,
            max_section_y,
        })
    }

    pub fn center(self) -> ChunkPos {
        self.center
    }

    pub fn horizontal_radius(self) -> u32 {
        self.horizontal_radius
    }

    pub fn min_section_y(self) -> i32 {
        self.min_section_y
    }

    pub fn max_section_y(self) -> i32 {
        self.max_section_y
    }

    pub fn contains(self, key: RenderSectionKey) -> bool {
        let radius = i64::from(self.horizontal_radius);
        (i64::from(key.chunk_x) - i64::from(self.center.x)).abs() <= radius
            && (i64::from(key.chunk_z) - i64::from(self.center.z)).abs() <= radius
            && key.section_y >= self.min_section_y
            && key.section_y <= self.max_section_y
    }
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

        assert!(region.contains(RenderSectionKey::new(-6, -3, -9)));
        assert!(region.contains(RenderSectionKey::new(-2, 5, -5)));
        assert!(!region.contains(RenderSectionKey::new(-7, 0, -7)));
        assert!(!region.contains(RenderSectionKey::new(-4, -4, -7)));
        assert!(!region.contains(RenderSectionKey::new(-4, 6, -7)));
    }

    #[test]
    fn invalid_vertical_region_is_rejected() {
        assert!(EmbeddedChunkRegion::new(ChunkPos::new(0, 0), 0, 2, 1).is_err());
    }
}
