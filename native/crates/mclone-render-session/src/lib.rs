#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use mclone_client::ClientRuntime;
use mclone_core::{
    AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkPos, ChunkSnapshot,
    PackedLightSection, SECTION_HEIGHT,
};
use mclone_mesh::{
    RenderSectionKey, TexturedChunkMeshInput, TexturedChunkVertex, TexturedMeshCatalog,
    TexturedRenderSectionBuildReport, TexturedRenderSectionMesh, TexturedVisibleChunkMesh,
    VisibilityGraphBuildStats, VisibilitySet,
    build_textured_render_sections_for_section_set_with_stats,
    build_textured_render_sections_with_stats, quad_face_count_from_indices,
};

const PACKED_BUILD_REPORT_MAGIC: &[u8; 8] = b"MCRSBR1\0";

pub fn build_client_textured_sections(
    client: &ClientRuntime,
    catalog: &TexturedMeshCatalog,
) -> Result<TexturedRenderSectionBuildReport> {
    let chunks = mesh_chunks_from_client(client)?;
    let inputs = textured_mesh_inputs(&chunks);
    build_textured_render_sections_with_stats(&inputs, catalog)
        .context("failed to build textured sections")
}

pub fn build_render_sections_from_snapshots(
    snapshots: &[ChunkSnapshot],
    catalog: &TexturedMeshCatalog,
    target_sections: &BTreeSet<RenderSectionKey>,
) -> Result<TexturedRenderSectionBuildReport> {
    let chunks = snapshots
        .iter()
        .map(snapshot_mesh_block_state_ids)
        .collect::<Result<Vec<_>>>()?;
    let inputs = textured_mesh_inputs(&chunks);
    build_textured_render_sections_for_section_set_with_stats(&inputs, catalog, target_sections)
        .context("failed to build queued textured render sections")
}

pub fn mesh_chunks_from_client(client: &ClientRuntime) -> Result<Vec<MeshChunkBlocks>> {
    client
        .chunk_snapshots()
        .map(snapshot_mesh_block_state_ids)
        .collect::<Result<Vec<_>>>()
}

pub fn textured_mesh_inputs(chunks: &[MeshChunkBlocks]) -> Vec<TexturedChunkMeshInput<'_>> {
    chunks
        .iter()
        .map(|chunk| {
            TexturedChunkMeshInput::new(
                chunk.chunk_x,
                chunk.chunk_z,
                chunk.min_y,
                chunk.height,
                &chunk.blocks,
            )
            .with_light_sections(&chunk.light_sections)
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct MeshChunkBlocks {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub min_y: i32,
    pub height: i32,
    pub blocks: Vec<mclone_core::BlockStateId>,
    pub light_sections: Vec<PackedLightSection>,
}

pub fn snapshot_mesh_block_state_ids(snapshot: &ChunkSnapshot) -> Result<MeshChunkBlocks> {
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
        light_sections: snapshot.light_sections.clone(),
    })
}

#[derive(Clone, Debug, Default)]
pub struct CachedTexturedRenderSections {
    sections: BTreeMap<RenderSectionKey, TexturedRenderSectionMesh>,
}

#[derive(Clone, Debug, Default)]
pub struct RenderSectionCacheUpdate {
    pub rebuilt_sections: Vec<TexturedRenderSectionMesh>,
    pub removed_section_keys: BTreeSet<RenderSectionKey>,
    pub rebuilt_vertex_count: u32,
    pub rebuilt_index_count: u32,
    pub neighbor_ready_section_count: usize,
    pub near_exception_section_count: usize,
    pub deferred_section_count: usize,
    pub submitted_compile_section_count: usize,
    pub completed_compile_section_count: usize,
    pub stale_compile_section_count: usize,
    pub pending_compile_jobs: usize,
    pub visibility_graph_stats: VisibilityGraphBuildStats,
}

impl RenderSectionCacheUpdate {
    pub fn rebuilt_section_count(&self) -> usize {
        self.rebuilt_sections.len()
    }

    pub fn removed_section_count(&self) -> usize {
        self.removed_section_keys.len()
    }

    pub fn rebuilt_face_count(&self) -> u32 {
        quad_face_count_from_indices(self.rebuilt_index_count)
    }

    pub fn merge(&mut self, other: Self) {
        self.rebuilt_sections.extend(other.rebuilt_sections);
        self.removed_section_keys.extend(other.removed_section_keys);
        self.rebuilt_vertex_count += other.rebuilt_vertex_count;
        self.rebuilt_index_count += other.rebuilt_index_count;
        self.neighbor_ready_section_count += other.neighbor_ready_section_count;
        self.near_exception_section_count += other.near_exception_section_count;
        self.deferred_section_count += other.deferred_section_count;
        self.submitted_compile_section_count += other.submitted_compile_section_count;
        self.completed_compile_section_count += other.completed_compile_section_count;
        self.stale_compile_section_count += other.stale_compile_section_count;
        self.pending_compile_jobs = other.pending_compile_jobs;
        self.visibility_graph_stats.build_count += other.visibility_graph_stats.build_count;
        self.visibility_graph_stats.total_ms += other.visibility_graph_stats.total_ms;
        self.visibility_graph_stats.worst_ms = self
            .visibility_graph_stats
            .worst_ms
            .max(other.visibility_graph_stats.worst_ms);
    }
}

#[derive(Clone, Debug)]
pub struct RenderSectionCompileRequest {
    pub target_sections: BTreeSet<RenderSectionKey>,
    pub section_revisions: BTreeMap<RenderSectionKey, u64>,
    pub snapshots: Vec<ChunkSnapshot>,
}

#[derive(Debug)]
pub struct RenderSectionCompileResult {
    pub target_sections: BTreeSet<RenderSectionKey>,
    pub section_revisions: BTreeMap<RenderSectionKey, u64>,
    pub result: std::result::Result<TexturedRenderSectionBuildReport, String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PackedRenderSectionBuildReportSummary {
    pub section_count: usize,
    pub non_empty_section_count: usize,
    pub vertex_count: u32,
    pub index_count: u32,
    pub visibility_graph_stats: VisibilityGraphBuildStats,
}

impl PackedRenderSectionBuildReportSummary {
    pub fn face_count(self) -> u32 {
        quad_face_count_from_indices(self.index_count)
    }
}

pub fn summarize_textured_render_section_build_report(
    report: &TexturedRenderSectionBuildReport,
) -> PackedRenderSectionBuildReportSummary {
    let mut summary = PackedRenderSectionBuildReportSummary {
        section_count: report.sections.len(),
        visibility_graph_stats: report.visibility_graph,
        ..PackedRenderSectionBuildReportSummary::default()
    };
    for section in &report.sections {
        if section.is_empty() {
            continue;
        }
        let stats = section.stats();
        summary.non_empty_section_count += 1;
        summary.vertex_count += stats.vertex_count;
        summary.index_count += stats.index_count;
    }
    summary
}

pub fn encode_textured_render_section_build_report(
    report: &TexturedRenderSectionBuildReport,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(PACKED_BUILD_REPORT_MAGIC);
    write_u64(&mut out, report.visibility_graph.build_count as u64);
    write_f64(&mut out, report.visibility_graph.total_ms);
    write_f64(&mut out, report.visibility_graph.worst_ms);
    write_u32(&mut out, report.sections.len() as u32);
    for section in &report.sections {
        write_i32(&mut out, section.key.chunk_x);
        write_i32(&mut out, section.key.section_y);
        write_i32(&mut out, section.key.chunk_z);
        write_u64(&mut out, section.visibility.bits());
        write_u32(&mut out, section.mesh.vertices.len() as u32);
        write_u32(&mut out, section.mesh.indices.len() as u32);
        for vertex in &section.mesh.vertices {
            for value in vertex.position {
                write_f32(&mut out, value);
            }
            for value in vertex.uv {
                write_f32(&mut out, value);
            }
            for value in vertex.color {
                write_f32(&mut out, value);
            }
            write_u32(&mut out, vertex.packed_light);
        }
        for index in &section.mesh.indices {
            write_u32(&mut out, *index);
        }
    }
    out
}

pub fn decode_textured_render_section_build_report(
    bytes: &[u8],
) -> Result<TexturedRenderSectionBuildReport> {
    let mut reader = PackedReportReader::new(bytes)?;
    let build_count =
        usize::try_from(reader.read_u64()?).context("visibility graph build count is too large")?;
    let total_ms = reader.read_f64()?;
    let worst_ms = reader.read_f64()?;
    let section_count = reader.read_u32()? as usize;
    let mut sections = Vec::with_capacity(section_count);
    for _ in 0..section_count {
        let key = RenderSectionKey::new(reader.read_i32()?, reader.read_i32()?, reader.read_i32()?);
        let visibility = VisibilitySet::from_bits(reader.read_u64()?);
        let vertex_count = reader.read_u32()? as usize;
        let index_count = reader.read_u32()? as usize;
        let mut vertices = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            let position = [reader.read_f32()?, reader.read_f32()?, reader.read_f32()?];
            let uv = [reader.read_f32()?, reader.read_f32()?];
            let color = [
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
            ];
            let packed_light = reader.read_u32()?;
            vertices.push(TexturedChunkVertex {
                position,
                uv,
                color,
                packed_light,
            });
        }
        let mut indices = Vec::with_capacity(index_count);
        for _ in 0..index_count {
            indices.push(reader.read_u32()?);
        }
        sections.push(TexturedRenderSectionMesh {
            key,
            mesh: TexturedVisibleChunkMesh { vertices, indices },
            visibility,
        });
    }
    reader.finish()?;

    Ok(TexturedRenderSectionBuildReport {
        sections,
        visibility_graph: VisibilityGraphBuildStats {
            build_count,
            total_ms,
            worst_ms,
        },
    })
}

pub trait RenderSectionCompiler {
    fn submit(&mut self, request: RenderSectionCompileRequest) -> Result<()>;

    fn try_recv_completed(&mut self) -> Result<Vec<RenderSectionCompileResult>>;

    fn pending_job_count(&self) -> usize;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RenderSectionCompileAcceptance {
    pub accepted_sections: BTreeSet<RenderSectionKey>,
    pub stale_sections: BTreeSet<RenderSectionKey>,
}

impl RenderSectionCompileAcceptance {
    pub fn accepted_section_count(&self) -> usize {
        self.accepted_sections.len()
    }

    pub fn stale_section_count(&self) -> usize {
        self.stale_sections.len()
    }
}

impl RenderSectionCompileResult {
    pub fn partition_by_revision(
        &self,
        mut current_revision: impl FnMut(RenderSectionKey) -> u64,
    ) -> RenderSectionCompileAcceptance {
        let mut accepted_sections = BTreeSet::new();
        let mut stale_sections = BTreeSet::new();
        for key in &self.target_sections {
            let submitted_revision = self.section_revisions.get(key).copied().unwrap_or_default();
            if current_revision(*key) == submitted_revision {
                accepted_sections.insert(*key);
            } else {
                stale_sections.insert(*key);
            }
        }

        RenderSectionCompileAcceptance {
            accepted_sections,
            stale_sections,
        }
    }
}

fn write_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_f64(out: &mut Vec<u8>, value: f64) {
    out.extend_from_slice(&value.to_le_bytes());
}

struct PackedReportReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> PackedReportReader<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self> {
        if !bytes.starts_with(PACKED_BUILD_REPORT_MAGIC) {
            bail!("packed render section build report has invalid magic/version");
        }
        Ok(Self {
            bytes,
            offset: PACKED_BUILD_REPORT_MAGIC.len(),
        })
    }

    fn finish(self) -> Result<()> {
        if self.offset != self.bytes.len() {
            bail!(
                "packed render section build report has {} trailing bytes",
                self.bytes.len() - self.offset
            );
        }
        Ok(())
    }

    fn read_i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.read_array()?))
    }

    fn read_u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }

    fn read_u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read_array()?))
    }

    fn read_f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.read_array()?))
    }

    fn read_f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.read_array()?))
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.offset + N;
        if end > self.bytes.len() {
            bail!("packed render section build report ended unexpectedly");
        }
        let mut out = [0_u8; N];
        out.copy_from_slice(&self.bytes[self.offset..end]);
        self.offset = end;
        Ok(out)
    }
}

impl CachedTexturedRenderSections {
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    pub fn contains_chunk(&self, pos: ChunkPos) -> bool {
        self.sections
            .keys()
            .any(|key| key.chunk_x == pos.x && key.chunk_z == pos.z)
    }

    pub fn contains_section(&self, key: RenderSectionKey) -> bool {
        self.sections.contains_key(&key)
    }

    pub fn sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.sections.values().cloned().collect()
    }

    pub fn section_keys(&self) -> impl Iterator<Item = RenderSectionKey> + '_ {
        self.sections.keys().copied()
    }

    pub fn apply_build_report(
        &mut self,
        ready_section_keys: &BTreeSet<RenderSectionKey>,
        rebuilt_report: TexturedRenderSectionBuildReport,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_section_keys: &BTreeSet<RenderSectionKey>,
    ) -> RenderSectionCacheUpdate {
        let rebuilt_keys = rebuilt_report
            .sections
            .iter()
            .map(|section| section.key)
            .collect::<BTreeSet<_>>();
        let rebuilt_sections = rebuilt_report
            .sections
            .into_iter()
            .filter(|section| ready_section_keys.contains(&section.key))
            .collect::<Vec<_>>();
        let old_ready_keys = self
            .sections
            .keys()
            .copied()
            .filter(|key| ready_section_keys.contains(key))
            .collect::<BTreeSet<_>>();
        let removal_keys = self
            .sections
            .keys()
            .copied()
            .filter(|key| removal_chunks.contains(&ChunkPos::new(key.chunk_x, key.chunk_z)))
            .collect::<BTreeSet<_>>();
        let mut removed_section_keys = old_ready_keys
            .difference(&rebuilt_keys)
            .copied()
            .collect::<BTreeSet<_>>();
        removed_section_keys.extend(removal_keys);
        removed_section_keys.extend(removal_section_keys.iter().copied());

        for key in &removed_section_keys {
            self.sections.remove(key);
        }

        let mut report = RenderSectionCacheUpdate {
            rebuilt_sections,
            removed_section_keys,
            rebuilt_vertex_count: 0,
            rebuilt_index_count: 0,
            visibility_graph_stats: rebuilt_report.visibility_graph,
            neighbor_ready_section_count: ready_section_keys.len(),
            completed_compile_section_count: ready_section_keys.len(),
            ..RenderSectionCacheUpdate::default()
        };
        for section in &report.rebuilt_sections {
            let stats = section.stats();
            report.rebuilt_vertex_count += stats.vertex_count;
            report.rebuilt_index_count += stats.index_count;
            self.sections.insert(section.key, section.clone());
        }
        report
    }

    pub fn remove_sections(
        &mut self,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_section_keys: &BTreeSet<RenderSectionKey>,
    ) -> RenderSectionCacheUpdate {
        self.apply_build_report(
            &BTreeSet::new(),
            TexturedRenderSectionBuildReport::default(),
            removal_chunks,
            removal_section_keys,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use mclone_core::{
        AIR_BLOCK_STATE_ID, BlockStateId, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkStatus,
    };
    use mclone_mesh::{
        TexturedChunkVertex, TexturedRenderSectionBuildReport, TexturedVisibleChunkMesh,
        VisibilityGraphBuildStats, VisibilitySet,
    };

    use super::*;

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

    #[test]
    fn cached_sections_apply_build_report_and_remove_unloaded_chunks() {
        let mut cache = CachedTexturedRenderSections::default();
        let key = RenderSectionKey::new(2, 4, -1);
        let report = TexturedRenderSectionBuildReport {
            sections: vec![TexturedRenderSectionMesh {
                key,
                mesh: TexturedVisibleChunkMesh::default(),
                visibility: VisibilitySet::all_visible(),
            }],
            visibility_graph: VisibilityGraphBuildStats::default(),
        };

        let update = cache.apply_build_report(
            &BTreeSet::from([key]),
            report,
            &BTreeSet::new(),
            &BTreeSet::new(),
        );

        assert_eq!(update.rebuilt_section_count(), 1);
        assert!(cache.contains_section(key));
        assert!(cache.contains_chunk(ChunkPos::new(2, -1)));

        let removal =
            cache.remove_sections(&BTreeSet::from([ChunkPos::new(2, -1)]), &BTreeSet::new());

        assert_eq!(removal.removed_section_count(), 1);
        assert!(!cache.contains_section(key));
        assert!(!cache.contains_chunk(ChunkPos::new(2, -1)));
    }

    #[test]
    fn compile_result_partitions_accepted_and_stale_revisions() {
        let unchanged = RenderSectionKey::new(0, 4, 0);
        let changed = RenderSectionKey::new(0, 5, 0);
        let default_revision = RenderSectionKey::new(0, 6, 0);
        let result = RenderSectionCompileResult {
            target_sections: BTreeSet::from([unchanged, changed, default_revision]),
            section_revisions: BTreeMap::from([(unchanged, 7), (changed, 3)]),
            result: Ok(TexturedRenderSectionBuildReport::default()),
        };

        let acceptance = result.partition_by_revision(|key| {
            if key == changed {
                4
            } else if key == unchanged {
                7
            } else {
                0
            }
        });

        assert_eq!(acceptance.accepted_section_count(), 2);
        assert_eq!(acceptance.stale_section_count(), 1);
        assert_eq!(
            acceptance.accepted_sections,
            BTreeSet::from([unchanged, default_revision])
        );
        assert_eq!(acceptance.stale_sections, BTreeSet::from([changed]));
    }

    #[test]
    fn packed_build_report_roundtrips_section_mesh_payload() {
        let key = RenderSectionKey::new(-2, 5, 7);
        let visibility = VisibilitySet::from_bits(0b101010);
        let report = TexturedRenderSectionBuildReport {
            sections: vec![TexturedRenderSectionMesh {
                key,
                mesh: TexturedVisibleChunkMesh {
                    vertices: vec![
                        TexturedChunkVertex {
                            position: [1.0, 2.0, 3.0],
                            uv: [0.25, 0.75],
                            color: [1.0, 0.5, 0.25, 1.0],
                            packed_light: 0x00f0_00f0,
                        },
                        TexturedChunkVertex {
                            position: [4.0, 5.0, 6.0],
                            uv: [0.5, 0.125],
                            color: [0.25, 0.5, 1.0, 1.0],
                            packed_light: 0x000f_000f,
                        },
                    ],
                    indices: vec![0, 1, 0],
                },
                visibility,
            }],
            visibility_graph: VisibilityGraphBuildStats {
                build_count: 1,
                total_ms: 2.5,
                worst_ms: 2.5,
            },
        };

        let encoded = encode_textured_render_section_build_report(&report);
        let decoded = decode_textured_render_section_build_report(&encoded).unwrap();

        assert_eq!(decoded, report);
        let summary = summarize_textured_render_section_build_report(&decoded);
        assert_eq!(summary.section_count, 1);
        assert_eq!(summary.non_empty_section_count, 1);
        assert_eq!(summary.vertex_count, 2);
        assert_eq!(summary.index_count, 3);
    }

    #[test]
    fn packed_build_report_rejects_invalid_bytes() {
        assert!(decode_textured_render_section_build_report(b"bad").is_err());

        let report = TexturedRenderSectionBuildReport::default();
        let mut encoded = encode_textured_render_section_build_report(&report);
        encoded.push(1);

        assert!(decode_textured_render_section_build_report(&encoded).is_err());
    }
}
