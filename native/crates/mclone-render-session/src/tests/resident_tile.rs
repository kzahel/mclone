use super::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TestTileKey {
    chunk: ChunkPos,
    level: u8,
}

impl ResidentTileKey for TestTileKey {
    fn chunk_pos(self) -> ChunkPos {
        self.chunk
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TestUpload {
    key: TestTileKey,
    bytes: usize,
}

impl ResidentTileUploadPayload<TestTileKey> for TestUpload {
    fn tile_key(&self) -> TestTileKey {
        self.key
    }

    fn estimated_owned_bytes(&self) -> usize {
        self.bytes
    }
}

fn key(chunk_x: i32, level: u8) -> TestTileKey {
    TestTileKey {
        chunk: ChunkPos::new(chunk_x, 0),
        level,
    }
}

#[test]
fn resident_tile_cache_shares_chunk_lookup_dirtying_and_eviction_across_levels() {
    let near = key(2, 1);
    let far = key(2, 2);
    let other = key(3, 2);
    let mut cache = ResidentTileCache::default();

    assert_eq!(cache.insert(near, "near"), None);
    assert_eq!(cache.insert(far, "far"), None);
    assert_eq!(cache.insert(other, "other"), None);
    assert_eq!(cache.generation(), 3);
    assert_eq!(cache.dirty_generation(), 0);
    assert_eq!(
        cache
            .tile_keys_for_chunk(ChunkPos::new(2, 0))
            .collect::<Vec<_>>(),
        vec![near, far]
    );

    assert!(cache.mark_tile_dirty(far));
    assert_eq!(cache.dirty_generation(), 1);
    assert!(cache.mark_tile_dirty(far));
    assert_eq!(cache.dirty_generation(), 1);
    assert_eq!(cache.dirty_tile_keys().collect::<Vec<_>>(), vec![far]);
    assert_eq!(cache.mark_all_tiles_dirty(), 2);
    assert_eq!(cache.dirty_generation(), 2);
    assert_eq!(cache.dirty_tile_count(), 3);
    assert!(cache.clear_tile_dirty(near));
    assert_eq!(cache.dirty_generation(), 3);

    assert_eq!(cache.remove(far), Some("far"));
    assert_eq!(cache.dirty_generation(), 4);
    assert_eq!(cache.generation(), 4);
    assert!(cache.contains_chunk(ChunkPos::new(2, 0)));
    assert_eq!(cache.remove(near), Some("near"));
    assert!(!cache.contains_chunk(ChunkPos::new(2, 0)));
}

#[test]
fn resident_tile_cache_applies_one_shared_ready_rebuild_and_removal_diff() {
    let retained = key(0, 1);
    let replaced = key(1, 1);
    let filtered = key(2, 2);
    let removed_chunk = key(3, 2);
    let mut cache = ResidentTileCache::default();
    cache.insert(retained, "old-retained");
    cache.insert(replaced, "old-replaced");
    cache.insert(removed_chunk, "remove");

    let update = cache.apply_build_report(
        &BTreeSet::from([retained, replaced]),
        vec![(replaced, "new-replaced"), (filtered, "filtered")],
        &BTreeSet::from([removed_chunk.chunk]),
        &BTreeSet::new(),
        |tile| tile.0,
        |tile| tile.1,
    );

    assert_eq!(update.rebuilt_tiles, vec![(replaced, "new-replaced")]);
    assert_eq!(
        update.removed_tile_keys,
        BTreeSet::from([retained, removed_chunk])
    );
    assert!(!cache.contains_tile(retained));
    assert!(cache.contains_tile(replaced));
    assert!(!cache.contains_tile(filtered));
    assert!(!cache.contains_tile(removed_chunk));
}

#[test]
fn resident_tile_upload_coordinator_preserves_supersession_budgets_and_job_release() {
    let first = key(0, 1);
    let second = key(1, 2);
    let mut uploads = ResidentTileUploadCoordinator::default();

    let removal = uploads.enqueue(Vec::new(), [first].into_iter().collect(), 1);
    assert_eq!(removal.queued_lifecycle_items, 1);
    assert_eq!(removal.released_producer_jobs, 0);

    let replacement = uploads.enqueue(
        vec![TestUpload {
            key: first,
            bytes: 64,
        }],
        BTreeSet::new(),
        1,
    );
    assert_eq!(replacement.superseded_lifecycle_items, 1);
    assert_eq!(replacement.released_producer_jobs, 1);
    uploads.enqueue(
        vec![TestUpload {
            key: second,
            bytes: 128,
        }],
        BTreeSet::new(),
        1,
    );
    assert_eq!(uploads.stats().queued_upload_owned_bytes, 192);

    let drain = uploads.drain_budgeted(Some(1), Some(1));
    assert_eq!(drain.uploads.len(), 1);
    assert_eq!(drain.uploads[0].key, first);
    assert!(drain.upload_limited);
    assert!(drain.accept_limited);
    assert_eq!(uploads.complete_applied_lifecycle_items(1), 1);

    let drain = uploads.drain_budgeted(Some(1), Some(1));
    assert_eq!(drain.uploads[0].key, second);
    assert!(!drain.upload_limited);
    assert!(!drain.accept_limited);
    assert_eq!(uploads.complete_applied_lifecycle_items(1), 1);
    assert!(!uploads.has_pending_work());
}
