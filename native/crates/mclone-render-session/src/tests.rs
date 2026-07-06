use std::collections::{BTreeMap, BTreeSet};

use mclone_assets::default_player_figure_id;
use mclone_client::{ActorAppearance, ActorPresentationId};
use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockPos, BlockStateId, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkStatus,
    HitResultType, chunk_section_index,
};
use mclone_mesh::{
    TexturedChunkVertex, TexturedRenderSectionBuildReport, TexturedVisibleChunkMesh,
    VisibilityGraphBuildStats, VisibilitySet,
};
use mclone_protocol::{SectionBlockUpdate, ServerUpdate};

use super::*;

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-9,
        "expected {actual} to be approximately {expected}"
    );
}

fn assert_vec3_close(actual: Vec3, expected: Vec3) {
    assert!(
        (actual - expected).length() < 1.0e-6,
        "expected {actual:?} to be approximately {expected:?}"
    );
}

fn test_build_report(
    keys: impl IntoIterator<Item = RenderSectionKey>,
) -> TexturedRenderSectionBuildReport {
    TexturedRenderSectionBuildReport {
        sections: keys
            .into_iter()
            .map(|key| TexturedRenderSectionMesh {
                key,
                mesh: TexturedVisibleChunkMesh::default(),
                visibility: VisibilitySet::all_visible(),
            })
            .collect(),
        visibility_graph: VisibilityGraphBuildStats::default(),
    }
}

fn test_section_mesh(key: RenderSectionKey) -> TexturedRenderSectionMesh {
    test_build_report([key])
        .sections
        .pop()
        .expect("test build report should contain one section")
}

fn empty_test_snapshot(pos: ChunkPos, min_y: i32, height: i32) -> ChunkSnapshot {
    let section_count =
        usize::try_from(height / SECTION_HEIGHT).expect("test height must fit usize");
    let block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * section_count];
    ChunkSnapshot::from_block_state_ids(
        pos,
        ChunkStatus::Surface,
        ChunkRevision(1),
        min_y,
        height,
        &block_state_ids,
    )
}

#[derive(Debug)]
struct CapacityTestCompiler {
    pending_jobs: usize,
    max_pending_jobs: usize,
    completed_results: Vec<RenderSectionCompileResult>,
    submitted_requests: Vec<RenderSectionCompileRequest>,
}

impl CapacityTestCompiler {
    fn new(pending_jobs: usize, max_pending_jobs: usize) -> Self {
        Self {
            pending_jobs,
            max_pending_jobs,
            completed_results: Vec::new(),
            submitted_requests: Vec::new(),
        }
    }

    fn with_completed_results(completed_results: Vec<RenderSectionCompileResult>) -> Self {
        Self {
            pending_jobs: 0,
            max_pending_jobs: 1,
            completed_results,
            submitted_requests: Vec::new(),
        }
    }
}

impl RenderSectionCompiler for CapacityTestCompiler {
    fn submit(&mut self, request: RenderSectionCompileRequest) -> Result<()> {
        self.pending_jobs += 1;
        self.submitted_requests.push(request);
        Ok(())
    }

    fn try_recv_completed(&mut self) -> Result<Vec<RenderSectionCompileResult>> {
        Ok(std::mem::take(&mut self.completed_results))
    }

    fn pending_job_count(&self) -> usize {
        self.pending_jobs
    }

    fn max_pending_job_count(&self) -> usize {
        self.max_pending_jobs
    }
}

fn test_snapshot_with_block(pos: ChunkPos, block: BlockPos, state: BlockStateId) -> ChunkSnapshot {
    assert_eq!(block.chunk_pos(), pos);
    assert!((0..SECTION_HEIGHT).contains(&block.y));
    let mut block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
    block_state_ids[chunk_section_index(block.x, block.y, block.z)] = state;
    ChunkSnapshot::from_block_state_ids(
        pos,
        ChunkStatus::Surface,
        ChunkRevision(1),
        0,
        SECTION_HEIGHT,
        &block_state_ids,
    )
}

mod actor_pose;
mod cache_session;
mod camera_controls;
mod codec_reports;
mod dirty_sync;
mod engine_session;
mod snapshot_dirty;
mod upload;
