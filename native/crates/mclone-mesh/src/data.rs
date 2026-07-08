use std::ops::Range;

use mclone_core::SECTION_HEIGHT as RENDER_SECTION_HEIGHT;

use crate::QUAD_FACE_INDEX_COUNT;
use crate::visibility::VisibilitySet;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SectionMeshStats {
    pub vertex_count: u32,
    pub index_count: u32,
}

impl SectionMeshStats {
    pub fn face_count(&self) -> u32 {
        quad_face_count_from_indices(self.index_count)
    }
}

pub fn quad_face_count_from_indices(index_count: u32) -> u32 {
    debug_assert_eq!(
        index_count % QUAD_FACE_INDEX_COUNT,
        0,
        "chunk meshes should emit whole quad faces"
    );
    index_count / QUAD_FACE_INDEX_COUNT
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VisibleChunkMesh {
    pub vertices: Vec<ChunkVertex>,
    pub indices: Vec<u32>,
}

impl VisibleChunkMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn stats(&self) -> SectionMeshStats {
        SectionMeshStats {
            vertex_count: self.vertices.len() as u32,
            index_count: self.indices.len() as u32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexturedChunkVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub packed_light: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TexturedVisibleChunkMesh {
    pub vertices: Vec<TexturedChunkVertex>,
    pub indices: Vec<u32>,
    pub solid_index_count: u32,
    pub opaque_index_count: u32,
}

impl TexturedVisibleChunkMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn estimated_owned_bytes(&self) -> usize {
        self.vertices
            .capacity()
            .saturating_mul(std::mem::size_of::<TexturedChunkVertex>())
            .saturating_add(
                self.indices
                    .capacity()
                    .saturating_mul(std::mem::size_of::<u32>()),
            )
    }

    pub fn stats(&self) -> SectionMeshStats {
        SectionMeshStats {
            vertex_count: self.vertices.len() as u32,
            index_count: self.indices.len() as u32,
        }
    }

    pub fn mark_all_indices_solid(&mut self) {
        let index_count = self.indices.len() as u32;
        self.solid_index_count = index_count;
        self.opaque_index_count = index_count;
    }

    pub fn mark_all_indices_cutout(&mut self) {
        self.solid_index_count = 0;
        self.opaque_index_count = self.indices.len() as u32;
    }

    pub fn solid_index_range(&self) -> Range<u32> {
        0..self.solid_index_count.min(self.indices.len() as u32)
    }

    pub fn cutout_index_range(&self) -> Range<u32> {
        self.solid_index_count.min(self.indices.len() as u32)
            ..self.opaque_index_count.min(self.indices.len() as u32)
    }

    pub fn opaque_index_range(&self) -> Range<u32> {
        0..self.opaque_index_count.min(self.indices.len() as u32)
    }

    pub fn translucent_index_range(&self) -> Range<u32> {
        self.opaque_index_count.min(self.indices.len() as u32)..self.indices.len() as u32
    }

    pub fn opaque_index_count(&self) -> u32 {
        self.opaque_index_range().len() as u32
    }

    pub fn solid_index_count(&self) -> u32 {
        self.solid_index_range().len() as u32
    }

    pub fn cutout_index_count(&self) -> u32 {
        self.cutout_index_range().len() as u32
    }

    pub fn translucent_index_count(&self) -> u32 {
        self.translucent_index_range().len() as u32
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RenderSectionKey {
    pub chunk_x: i32,
    pub section_y: i32,
    pub chunk_z: i32,
}

impl RenderSectionKey {
    pub fn new(chunk_x: i32, section_y: i32, chunk_z: i32) -> Self {
        Self {
            chunk_x,
            section_y,
            chunk_z,
        }
    }

    pub fn min_y(self) -> i32 {
        self.section_y * RENDER_SECTION_HEIGHT
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TexturedRenderSectionMesh {
    pub key: RenderSectionKey,
    pub mesh: TexturedVisibleChunkMesh,
    pub visibility: VisibilitySet,
}

impl TexturedRenderSectionMesh {
    pub fn is_empty(&self) -> bool {
        self.mesh.is_empty()
    }

    pub fn estimated_owned_bytes(&self) -> usize {
        self.mesh.estimated_owned_bytes()
    }

    pub fn stats(&self) -> SectionMeshStats {
        self.mesh.stats()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VisibilityGraphBuildStats {
    pub build_count: usize,
    pub total_ms: f64,
    pub worst_ms: f64,
}

impl VisibilityGraphBuildStats {
    pub fn average_ms(self) -> f64 {
        if self.build_count == 0 {
            0.0
        } else {
            self.total_ms / self.build_count as f64
        }
    }

    pub(crate) fn record_ms(&mut self, elapsed_ms: f64) {
        self.build_count += 1;
        self.total_ms += elapsed_ms;
        self.worst_ms = self.worst_ms.max(elapsed_ms);
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TexturedRenderSectionBuildReport {
    pub sections: Vec<TexturedRenderSectionMesh>,
    pub visibility_graph: VisibilityGraphBuildStats,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textured_mesh_layer_ranges_split_opaque_and_translucent_indices() {
        let mesh = TexturedVisibleChunkMesh {
            vertices: Vec::new(),
            indices: vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
            solid_index_count: 6,
            opaque_index_count: 6,
        };

        assert_eq!(mesh.solid_index_range(), 0..6);
        assert_eq!(mesh.cutout_index_range(), 6..6);
        assert_eq!(mesh.opaque_index_range(), 0..6);
        assert_eq!(mesh.translucent_index_range(), 6..12);
        assert_eq!(mesh.solid_index_count(), 6);
        assert_eq!(mesh.cutout_index_count(), 0);
        assert_eq!(mesh.opaque_index_count(), 6);
        assert_eq!(mesh.translucent_index_count(), 6);
    }

    #[test]
    fn textured_mesh_layer_ranges_clamp_invalid_opaque_count() {
        let mesh = TexturedVisibleChunkMesh {
            vertices: Vec::new(),
            indices: vec![0, 1, 2],
            solid_index_count: 99,
            opaque_index_count: 99,
        };

        assert_eq!(mesh.solid_index_range(), 0..3);
        assert_eq!(mesh.cutout_index_range(), 3..3);
        assert_eq!(mesh.opaque_index_range(), 0..3);
        assert_eq!(mesh.translucent_index_range(), 3..3);
    }

    #[test]
    fn textured_mesh_layer_ranges_split_solid_cutout_and_translucent_indices() {
        let mesh = TexturedVisibleChunkMesh {
            vertices: Vec::new(),
            indices: vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7, 8, 9, 10, 8, 10, 11],
            solid_index_count: 6,
            opaque_index_count: 12,
        };

        assert_eq!(mesh.solid_index_range(), 0..6);
        assert_eq!(mesh.cutout_index_range(), 6..12);
        assert_eq!(mesh.opaque_index_range(), 0..12);
        assert_eq!(mesh.translucent_index_range(), 12..18);
        assert_eq!(mesh.solid_index_count(), 6);
        assert_eq!(mesh.cutout_index_count(), 6);
        assert_eq!(mesh.opaque_index_count(), 12);
        assert_eq!(mesh.translucent_index_count(), 6);
    }
}
