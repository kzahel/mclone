use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EngineServerUpdateReport {
    pub changed: bool,
    pub updates: usize,
    pub snapshot_updates: usize,
    pub section_block_updates: usize,
    pub unload_updates: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EngineServerUpdateDirtyPolicy {
    pub dirty_chunk_snapshots: bool,
    pub dirty_chunk_unloads: bool,
    pub dirty_section_block_updates: bool,
}

impl EngineServerUpdateDirtyPolicy {
    pub const ALL: Self = Self {
        dirty_chunk_snapshots: true,
        dirty_chunk_unloads: true,
        dirty_section_block_updates: true,
    };

    pub const SECTION_BLOCK_UPDATES_ONLY: Self = Self {
        dirty_chunk_snapshots: false,
        dirty_chunk_unloads: false,
        dirty_section_block_updates: true,
    };
}

impl EngineServerUpdateReport {
    pub fn classify(updates: &[ServerUpdate]) -> Self {
        let mut report = Self {
            updates: updates.len(),
            ..Self::default()
        };
        for update in updates {
            match update {
                ServerUpdate::ChunkSnapshot(_) => {
                    report.changed = true;
                    report.snapshot_updates += 1;
                }
                ServerUpdate::ChunkUnload { .. } => {
                    report.changed = true;
                    report.unload_updates += 1;
                }
                ServerUpdate::SectionBlockUpdates { .. } => {
                    report.changed = true;
                    report.section_block_updates += 1;
                }
                ServerUpdate::PlayerPosition(_) | ServerUpdate::PlayerLife(_) => {
                    report.changed = true;
                }
                ServerUpdate::DimensionChange { .. } => {
                    report.changed = true;
                }
                ServerUpdate::SessionConfiguration(_)
                | ServerUpdate::SessionReady
                | ServerUpdate::KeepAlive { .. }
                | ServerUpdate::Disconnect(_)
                | ServerUpdate::WorldInfo { .. }
                | ServerUpdate::TimeUpdate { .. }
                | ServerUpdate::PlayerExperience { .. }
                | ServerUpdate::PlayerStatistics { .. }
                | ServerUpdate::RemotePlayerAdd(_)
                | ServerUpdate::RemotePlayerUpdate(_)
                | ServerUpdate::RemotePlayerRemove { .. }
                | ServerUpdate::EphemeralFallback(_)
                | ServerUpdate::EntitySnapshot(_)
                | ServerUpdate::EntityUpdate(_)
                | ServerUpdate::EntityRemove { .. } => {}
            }
        }
        report
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct EngineServerUpdateDirtyBatch {
    pub(crate) dirty_chunks: BTreeSet<ChunkPos>,
    pub(crate) force_dirty_chunks: BTreeSet<ChunkPos>,
    pub(crate) dirty_sections: BTreeSet<RenderSectionKey>,
}

impl EngineServerUpdateDirtyBatch {
    pub(crate) fn collect(
        updates: &[ServerUpdate],
        policy: EngineServerUpdateDirtyPolicy,
        topology: HorizontalTopology,
    ) -> Self {
        let mut batch = Self::default();
        for update in updates {
            match update {
                ServerUpdate::ChunkSnapshot(snapshot) => {
                    if policy.dirty_chunk_snapshots {
                        batch.force_dirty_chunks.insert(snapshot.pos);
                        batch.dirty_chunks.extend(
                            render_dirty_chunk_neighborhood(snapshot.pos)
                                .into_iter()
                                .filter_map(|pos| topology.canonicalize_chunk(pos)),
                        );
                    }
                }
                ServerUpdate::ChunkUnload { pos } => {
                    if policy.dirty_chunk_unloads {
                        batch.dirty_chunks.extend(
                            render_dirty_chunk_neighborhood(*pos)
                                .into_iter()
                                .filter_map(|pos| topology.canonicalize_chunk(pos)),
                        );
                    }
                }
                ServerUpdate::SectionBlockUpdates {
                    pos,
                    section_y,
                    updates,
                } => {
                    if policy.dirty_section_block_updates {
                        for update in updates {
                            batch.dirty_sections.extend(
                                render_dirty_section_keys_for_block_update(
                                    *pos, *section_y, update,
                                )
                                .into_iter()
                                .filter_map(|key| {
                                    topology
                                        .canonicalize_chunk(ChunkPos::new(key.chunk_x, key.chunk_z))
                                        .map(|chunk| {
                                            RenderSectionKey::new(chunk.x, key.section_y, chunk.z)
                                        })
                                }),
                            );
                        }
                    }
                }
                ServerUpdate::SessionConfiguration(_)
                | ServerUpdate::SessionReady
                | ServerUpdate::KeepAlive { .. }
                | ServerUpdate::Disconnect(_)
                | ServerUpdate::WorldInfo { .. }
                | ServerUpdate::DimensionChange { .. } => {}
                ServerUpdate::TimeUpdate { .. } => {}
                ServerUpdate::PlayerExperience { .. } => {}
                ServerUpdate::PlayerStatistics { .. } => {}
                ServerUpdate::PlayerLife(_) => {}
                ServerUpdate::PlayerPosition(_) => {}
                ServerUpdate::RemotePlayerAdd(_)
                | ServerUpdate::RemotePlayerUpdate(_)
                | ServerUpdate::RemotePlayerRemove { .. }
                | ServerUpdate::EphemeralFallback(_)
                | ServerUpdate::EntitySnapshot(_)
                | ServerUpdate::EntityUpdate(_)
                | ServerUpdate::EntityRemove { .. } => {}
            }
        }
        batch
    }
}

pub fn render_dirty_section_keys_for_block_update(
    pos: ChunkPos,
    section_y: i32,
    update: &SectionBlockUpdate,
) -> BTreeSet<RenderSectionKey> {
    let world_x = chunk_block_coord(pos.x, update.local_x as i32);
    let world_y = section_y * SECTION_HEIGHT + update.local_y as i32;
    let world_z = chunk_block_coord(pos.z, update.local_z as i32);
    let mut keys = BTreeSet::new();
    for z in world_z - 1..=world_z + 1 {
        for x in world_x - 1..=world_x + 1 {
            for y in world_y - 1..=world_y + 1 {
                keys.insert(RenderSectionKey::new(
                    block_to_chunk_coord(x),
                    block_to_section_coord(y),
                    block_to_chunk_coord(z),
                ));
            }
        }
    }
    keys
}

#[cfg(test)]
mod topology_tests {
    use super::*;

    #[test]
    fn periodic_snapshot_dirtying_wraps_and_deduplicates_the_seam_neighbor() {
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            mclone_core::ChunkStatus::Full,
            mclone_core::ChunkRevision(1),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        );
        let batch = EngineServerUpdateDirtyBatch::collect(
            &[ServerUpdate::ChunkSnapshot(snapshot)],
            EngineServerUpdateDirtyPolicy::ALL,
            HorizontalTopology::cylinder_x(0, 32),
        );

        assert!(batch.dirty_chunks.contains(&ChunkPos::new(31, 0)));
        assert!(!batch.dirty_chunks.contains(&ChunkPos::new(-1, 0)));
        assert_eq!(batch.dirty_chunks.len(), 5);
    }
}
