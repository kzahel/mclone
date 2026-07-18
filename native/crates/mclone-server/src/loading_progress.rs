use std::collections::BTreeMap;

use mclone_core::{ChunkPos, ChunkStatus, HorizontalTopology, LiftedChunkPos};
use mclone_protocol::ChunkView;

use crate::ChunkStatusStep;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkLoadingProgress {
    topology: HorizontalTopology,
    view: Option<ChunkLoadingProgressView>,
    target_status: ChunkStatus,
    latest_statuses: BTreeMap<ChunkPos, ChunkStatus>,
    ready_statuses: BTreeMap<ChunkPos, ChunkStatus>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ChunkLoadingProgressView {
    center: ChunkPos,
    target_radius: u32,
    target_lifts: BTreeMap<ChunkPos, LiftedChunkPos>,
    playable_lifts: BTreeMap<ChunkPos, LiftedChunkPos>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkLoadingProgressStats {
    pub center: ChunkPos,
    pub target_radius: u32,
    pub target_status: ChunkStatus,
    pub target_chunk_count: usize,
    pub target_ready_chunks: usize,
    pub playable_chunk: ChunkPos,
    pub playable_gate_radius: u32,
    pub playable_gate_chunk_count: usize,
    pub playable_gate_ready_chunks: usize,
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
        Self::with_topology(target_status, HorizontalTopology::UNBOUNDED)
    }

    pub fn with_topology(target_status: ChunkStatus, topology: HorizontalTopology) -> Self {
        topology
            .validate()
            .expect("chunk loading progress requires validated topology");
        Self {
            topology,
            view: None,
            target_status,
            latest_statuses: BTreeMap::new(),
            ready_statuses: BTreeMap::new(),
        }
    }

    pub fn set_target_status(&mut self, target_status: ChunkStatus) {
        self.target_status = target_status;
    }

    pub fn set_view(&mut self, view: &ChunkView) {
        let target_lifts = self
            .topology
            .chunk_view(view.center, view.chunk_tracking_radius)
            .expect("accepted loading view must satisfy the topology contract")
            .into_iter()
            .map(|entry| (entry.canonical, entry.lifted))
            .collect();
        let playable_lifts = self
            .topology
            .chunk_view(view.center, PLAYABLE_GATE_RADIUS)
            .expect("playable loading gate must satisfy the topology contract")
            .into_iter()
            .map(|entry| (entry.canonical, entry.lifted))
            .collect();
        self.view = Some(ChunkLoadingProgressView {
            center: view.center,
            target_radius: view.chunk_tracking_radius,
            target_lifts,
            playable_lifts,
        });
    }

    pub fn record_status_change(
        &mut self,
        pos: ChunkPos,
        status: ChunkStatus,
        step: ChunkStatusStep,
    ) {
        self.latest_statuses
            .entry(pos)
            .and_modify(|latest_status| {
                *latest_status = (*latest_status).max(status);
            })
            .or_insert(status);

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
        self.latest_statuses.remove(&pos);
        self.ready_statuses.remove(&pos);
    }

    pub fn stats(&self) -> Option<ChunkLoadingProgressStats> {
        self.snapshot().map(|snapshot| snapshot.stats)
    }

    pub fn snapshot(&self) -> Option<ChunkLoadingProgressSnapshot> {
        let view = self.view.as_ref()?;
        let target_ready_chunks = self
            .ready_statuses
            .iter()
            .filter(|(pos, status)| {
                view.target_lifts.contains_key(pos) && **status >= self.target_status
            })
            .count();
        let playable_gate_ready_chunks = view
            .playable_lifts
            .keys()
            .filter(|pos| {
                self.ready_statuses
                    .get(pos)
                    .is_some_and(|status| *status >= self.target_status)
            })
            .count();
        let playable_gate_chunk_count = view.playable_lifts.len();
        let playable_chunk_ready = playable_gate_ready_chunks == playable_gate_chunk_count;
        let stats = ChunkLoadingProgressStats {
            center: view.center,
            target_radius: view.target_radius,
            target_status: self.target_status,
            target_chunk_count: view.target_lifts.len(),
            target_ready_chunks,
            playable_chunk: view.center,
            playable_gate_radius: PLAYABLE_GATE_RADIUS,
            playable_gate_chunk_count,
            playable_gate_ready_chunks,
            playable_chunk_ready,
        };

        let mut cells = self
            .latest_statuses
            .iter()
            .filter_map(|(pos, status)| {
                let ready_status = self.ready_statuses.get(pos).copied();
                let target_ready =
                    ready_status.is_some_and(|ready_status| ready_status >= self.target_status);
                view.target_lifts
                    .get(pos)
                    .map(|lifted| ChunkLoadingProgressCell {
                        relative_x: i32::try_from(lifted.x - i64::from(view.center.x))
                            .expect("accepted loading view relative X must fit i32"),
                        relative_z: i32::try_from(lifted.z - i64::from(view.center.z))
                            .expect("accepted loading view relative Z must fit i32"),
                        status: Some(*status),
                        target_ready,
                        playable: *pos == stats.playable_chunk,
                    })
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

pub(crate) const PLAYABLE_GATE_RADIUS: u32 = 1;

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

    fn mark_playable_gate_ready(
        progress: &mut ChunkLoadingProgress,
        center: ChunkPos,
        status: ChunkStatus,
    ) {
        for relative_z in -1..=1 {
            for relative_x in -1..=1 {
                progress.record_status_change(
                    ChunkPos::new(center.x + relative_x, center.z + relative_z),
                    status,
                    ChunkStatusStep::Ready,
                );
            }
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
                playable_gate_radius: PLAYABLE_GATE_RADIUS,
                playable_gate_chunk_count: 9,
                playable_gate_ready_chunks: 0,
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
        assert_eq!(stats.playable_gate_ready_chunks, 2);
        assert!(!stats.playable_chunk_ready);

        mark_playable_gate_ready(&mut progress, ChunkPos::new(0, 0), ChunkStatus::Light);

        let stats = progress.stats().unwrap();
        assert_eq!(stats.target_ready_chunks, 9);
        assert_eq!(stats.playable_gate_ready_chunks, 9);
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

        assert_eq!(
            progress.snapshot().unwrap().cells,
            vec![ChunkLoadingProgressCell {
                relative_x: 0,
                relative_z: 0,
                status: Some(ChunkStatus::Features),
                target_ready: false,
                playable: true,
            }]
        );
    }

    #[test]
    fn scheduled_status_can_color_cell_after_lower_status_is_ready() {
        let mut progress = ChunkLoadingProgress::new(ChunkStatus::Light);
        progress.set_view(&view(ChunkPos::new(0, 0), 0));

        progress.record_status_change(
            ChunkPos::new(0, 0),
            ChunkStatus::Features,
            ChunkStatusStep::Ready,
        );
        progress.record_status_change(
            ChunkPos::new(0, 0),
            ChunkStatus::Light,
            ChunkStatusStep::Scheduled,
        );

        let snapshot = progress.snapshot().unwrap();
        assert_eq!(snapshot.stats.target_ready_chunks, 0);
        assert_eq!(
            snapshot.cells,
            vec![ChunkLoadingProgressCell {
                relative_x: 0,
                relative_z: 0,
                status: Some(ChunkStatus::Light),
                target_ready: false,
                playable: true,
            }]
        );
    }

    #[test]
    fn clear_chunk_resets_playable_readiness() {
        let mut progress = ChunkLoadingProgress::new(ChunkStatus::Features);
        let center = ChunkPos::new(4, -2);
        progress.set_view(&view(center, 0));
        mark_playable_gate_ready(&mut progress, center, ChunkStatus::Features);
        assert!(progress.stats().unwrap().playable_chunk_ready);

        progress.clear_chunk(center);

        let stats = progress.stats().unwrap();
        assert_eq!(stats.target_ready_chunks, 0);
        assert_eq!(stats.playable_gate_ready_chunks, 8);
        assert!(!stats.playable_chunk_ready);
        assert_eq!(
            progress.snapshot().unwrap().cells,
            vec![ChunkLoadingProgressCell {
                relative_x: 0,
                relative_z: 0,
                status: None,
                target_ready: false,
                playable: true,
            }]
        );
    }

    #[test]
    fn target_status_can_follow_lighting_mode() {
        let mut progress = ChunkLoadingProgress::new(ChunkStatus::Light);
        let center = ChunkPos::new(0, 0);
        progress.set_view(&view(center, 0));
        mark_playable_gate_ready(&mut progress, center, ChunkStatus::Features);
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

    #[test]
    fn finite_topology_counts_only_authoritative_boundary_chunks() {
        let topology = HorizontalTopology {
            x: mclone_core::AxisTopology::Finite {
                minimum_chunk: 0,
                maximum_chunk_exclusive: 2,
            },
            z: mclone_core::AxisTopology::Finite {
                minimum_chunk: 0,
                maximum_chunk_exclusive: 2,
            },
        };
        let mut progress = ChunkLoadingProgress::with_topology(ChunkStatus::Features, topology);
        progress.set_view(&view(ChunkPos::new(0, 0), 2));

        for z in 0..2 {
            for x in 0..2 {
                progress.record_status_change(
                    ChunkPos::new(x, z),
                    ChunkStatus::Features,
                    ChunkStatusStep::Ready,
                );
            }
        }

        let snapshot = progress.snapshot().unwrap();
        assert_eq!(snapshot.stats.target_chunk_count, 4);
        assert_eq!(snapshot.stats.target_ready_chunks, 4);
        assert_eq!(snapshot.stats.playable_gate_chunk_count, 4);
        assert_eq!(snapshot.stats.playable_gate_ready_chunks, 4);
        assert!(snapshot.stats.playable_chunk_ready);
        assert_eq!(snapshot.cells.len(), 4);
    }
}
