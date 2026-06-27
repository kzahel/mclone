use std::collections::BTreeMap;

use mclone_core::{ChunkPos, ChunkStatus};
use mclone_protocol::ChunkView;

use crate::ChunkStatusStep;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkLoadingProgress {
    view: Option<ChunkLoadingProgressView>,
    target_status: ChunkStatus,
    ready_statuses: BTreeMap<ChunkPos, ChunkStatus>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChunkLoadingProgressView {
    center: ChunkPos,
    target_radius: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkLoadingProgressStats {
    pub center: ChunkPos,
    pub target_radius: u32,
    pub target_status: ChunkStatus,
    pub target_chunk_count: usize,
    pub target_ready_chunks: usize,
    pub playable_chunk: ChunkPos,
    pub playable_chunk_ready: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkLoadingProgressCell {
    pub relative_x: i32,
    pub relative_z: i32,
    pub status: Option<ChunkStatus>,
    pub target_ready: bool,
    pub playable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkLoadingProgressSnapshot {
    pub stats: ChunkLoadingProgressStats,
    pub cells: Vec<ChunkLoadingProgressCell>,
}

impl Default for ChunkLoadingProgress {
    fn default() -> Self {
        Self::new(ChunkStatus::Light)
    }
}

impl ChunkLoadingProgress {
    pub fn new(target_status: ChunkStatus) -> Self {
        Self {
            view: None,
            target_status,
            ready_statuses: BTreeMap::new(),
        }
    }

    pub fn set_target_status(&mut self, target_status: ChunkStatus) {
        self.target_status = target_status;
    }

    pub fn set_view(&mut self, view: &ChunkView) {
        self.view = Some(ChunkLoadingProgressView {
            center: view.center,
            target_radius: view.chunk_tracking_radius,
        });
    }

    pub fn record_status_change(
        &mut self,
        pos: ChunkPos,
        status: ChunkStatus,
        step: ChunkStatusStep,
    ) {
        if step == ChunkStatusStep::Ready {
            self.ready_statuses
                .entry(pos)
                .and_modify(|ready_status| {
                    *ready_status = (*ready_status).max(status);
                })
                .or_insert(status);
        }
    }

    pub fn clear_chunk(&mut self, pos: ChunkPos) {
        self.ready_statuses.remove(&pos);
    }

    pub fn stats(&self) -> Option<ChunkLoadingProgressStats> {
        self.snapshot().map(|snapshot| snapshot.stats)
    }

    pub fn snapshot(&self) -> Option<ChunkLoadingProgressSnapshot> {
        let view = self.view?;
        let target_ready_chunks = self
            .ready_statuses
            .iter()
            .filter(|(pos, status)| {
                chunk_within_radius(**pos, view.center, view.target_radius)
                    && **status >= self.target_status
            })
            .count();
        let playable_chunk_ready = self
            .ready_statuses
            .get(&view.center)
            .is_some_and(|status| *status >= self.target_status);
        let stats = ChunkLoadingProgressStats {
            center: view.center,
            target_radius: view.target_radius,
            target_status: self.target_status,
            target_chunk_count: square_chunk_count(view.target_radius),
            target_ready_chunks,
            playable_chunk: view.center,
            playable_chunk_ready,
        };

        let mut cells = self
            .ready_statuses
            .iter()
            .filter_map(|(pos, status)| {
                chunk_within_radius(*pos, view.center, view.target_radius).then_some(
                    ChunkLoadingProgressCell {
                        relative_x: pos.x - view.center.x,
                        relative_z: pos.z - view.center.z,
                        status: Some(*status),
                        target_ready: *status >= self.target_status,
                        playable: *pos == stats.playable_chunk,
                    },
                )
            })
            .collect::<Vec<_>>();

        if !cells.iter().any(|cell| cell.playable) {
            cells.push(ChunkLoadingProgressCell {
                relative_x: stats.playable_chunk.x - view.center.x,
                relative_z: stats.playable_chunk.z - view.center.z,
                status: None,
                target_ready: false,
                playable: true,
            });
        }

        Some(ChunkLoadingProgressSnapshot { stats, cells })
    }
}

fn chunk_within_radius(pos: ChunkPos, center: ChunkPos, radius: u32) -> bool {
    pos.x.abs_diff(center.x).max(pos.z.abs_diff(center.z)) <= radius
}

fn square_chunk_count(radius: u32) -> usize {
    let side = radius as usize * 2 + 1;
    side * side
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(center: ChunkPos, radius: u32) -> ChunkView {
        ChunkView {
            center,
            render_distance: radius,
            chunk_tracking_radius: radius,
        }
    }

    #[test]
    fn counts_target_ready_chunks_inside_view_radius() {
        let mut progress = ChunkLoadingProgress::new(ChunkStatus::Light);
        progress.set_view(&view(ChunkPos::new(0, 0), 1));

        progress.record_status_change(
            ChunkPos::new(0, 0),
            ChunkStatus::Features,
            ChunkStatusStep::Ready,
        );
        assert_eq!(
            progress.stats().unwrap(),
            ChunkLoadingProgressStats {
                center: ChunkPos::new(0, 0),
                target_radius: 1,
                target_status: ChunkStatus::Light,
                target_chunk_count: 9,
                target_ready_chunks: 0,
                playable_chunk: ChunkPos::new(0, 0),
                playable_chunk_ready: false,
            }
        );

        progress.record_status_change(
            ChunkPos::new(0, 0),
            ChunkStatus::Light,
            ChunkStatusStep::Ready,
        );
        progress.record_status_change(
            ChunkPos::new(1, 0),
            ChunkStatus::Light,
            ChunkStatusStep::Ready,
        );
        progress.record_status_change(
            ChunkPos::new(3, 0),
            ChunkStatus::Light,
            ChunkStatusStep::Ready,
        );

        let stats = progress.stats().unwrap();
        assert_eq!(stats.target_ready_chunks, 2);
        assert!(stats.playable_chunk_ready);
    }

    #[test]
    fn scheduled_statuses_do_not_mark_chunks_ready() {
        let mut progress = ChunkLoadingProgress::new(ChunkStatus::Features);
        progress.set_view(&view(ChunkPos::new(0, 0), 0));

        progress.record_status_change(
            ChunkPos::new(0, 0),
            ChunkStatus::Features,
            ChunkStatusStep::Scheduled,
        );

        let stats = progress.stats().unwrap();
        assert_eq!(stats.target_ready_chunks, 0);
        assert!(!stats.playable_chunk_ready);
    }

    #[test]
    fn clear_chunk_resets_playable_readiness() {
        let mut progress = ChunkLoadingProgress::new(ChunkStatus::Features);
        let center = ChunkPos::new(4, -2);
        progress.set_view(&view(center, 0));
        progress.record_status_change(center, ChunkStatus::Features, ChunkStatusStep::Ready);
        assert!(progress.stats().unwrap().playable_chunk_ready);

        progress.clear_chunk(center);

        let stats = progress.stats().unwrap();
        assert_eq!(stats.target_ready_chunks, 0);
        assert!(!stats.playable_chunk_ready);
    }

    #[test]
    fn target_status_can_follow_lighting_mode() {
        let mut progress = ChunkLoadingProgress::new(ChunkStatus::Light);
        progress.set_view(&view(ChunkPos::new(0, 0), 0));
        progress.record_status_change(
            ChunkPos::new(0, 0),
            ChunkStatus::Features,
            ChunkStatusStep::Ready,
        );
        assert!(!progress.stats().unwrap().playable_chunk_ready);

        progress.set_target_status(ChunkStatus::Features);

        assert!(progress.stats().unwrap().playable_chunk_ready);
    }

    #[test]
    fn snapshot_includes_relative_cells_inside_radius() {
        let mut progress = ChunkLoadingProgress::new(ChunkStatus::Light);
        progress.set_view(&view(ChunkPos::new(4, -3), 1));
        progress.record_status_change(
            ChunkPos::new(4, -3),
            ChunkStatus::Light,
            ChunkStatusStep::Ready,
        );
        progress.record_status_change(
            ChunkPos::new(5, -3),
            ChunkStatus::Features,
            ChunkStatusStep::Ready,
        );
        progress.record_status_change(
            ChunkPos::new(7, -3),
            ChunkStatus::Light,
            ChunkStatusStep::Ready,
        );

        let snapshot = progress.snapshot().unwrap();
        assert_eq!(snapshot.stats.target_ready_chunks, 1);
        assert_eq!(
            snapshot.cells,
            vec![
                ChunkLoadingProgressCell {
                    relative_x: 0,
                    relative_z: 0,
                    status: Some(ChunkStatus::Light),
                    target_ready: true,
                    playable: true,
                },
                ChunkLoadingProgressCell {
                    relative_x: 1,
                    relative_z: 0,
                    status: Some(ChunkStatus::Features),
                    target_ready: false,
                    playable: false,
                },
            ]
        );
    }

    #[test]
    fn snapshot_marks_playable_cell_even_before_status_arrives() {
        let mut progress = ChunkLoadingProgress::new(ChunkStatus::Features);
        progress.set_view(&view(ChunkPos::new(2, 3), 0));

        let snapshot = progress.snapshot().unwrap();

        assert_eq!(
            snapshot.cells,
            vec![ChunkLoadingProgressCell {
                relative_x: 0,
                relative_z: 0,
                status: None,
                target_ready: false,
                playable: true,
            }]
        );
    }
}
