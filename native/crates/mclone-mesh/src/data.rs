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
}

impl TexturedVisibleChunkMesh {
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
