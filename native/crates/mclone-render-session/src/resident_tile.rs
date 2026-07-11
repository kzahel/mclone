use super::*;

/// Host-neutral identity required by the shared resident-tile substrate.
///
/// Producers keep their concrete key and payload types. The substrate needs
/// only a stable ordering plus the chunk footprint used for residency diffs
/// and cross-producer precedence. Level zero is reserved for real sections.
pub trait ResidentTileKey: Copy + Ord {
    fn chunk_pos(self) -> ChunkPos;

    fn lod_level(self) -> u8;
}

impl ResidentTileKey for RenderSectionKey {
    fn chunk_pos(self) -> ChunkPos {
        render_section_chunk_pos(self)
    }

    fn lod_level(self) -> u8 {
        0
    }
}

impl ResidentTileKey for mclone_core::LodTileKey {
    fn chunk_pos(self) -> ChunkPos {
        self.chunk
    }

    fn lod_level(self) -> u8 {
        self.level
    }
}

#[derive(Clone, Debug)]
struct ResidentTileSlot<M> {
    metadata: M,
    dirty: bool,
}

/// Shared resident metadata, dirtying, chunk lookup, and eviction mechanism.
///
/// The cache deliberately knows nothing about mesh construction, shaders, or
/// GPU allocation. Real render sections use it with compact section metadata;
/// later LOD producers can use their own keys and metadata without duplicating
/// the lifecycle algorithm.
#[derive(Clone, Debug)]
pub struct ResidentTileCache<K, M> {
    tiles: BTreeMap<K, ResidentTileSlot<M>>,
    tile_keys_by_chunk: BTreeMap<ChunkPos, BTreeSet<K>>,
    generation: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResidentTileCacheUpdate<K, P> {
    pub rebuilt_tiles: Vec<P>,
    pub removed_tile_keys: BTreeSet<K>,
}

impl<K, M> Default for ResidentTileCache<K, M> {
    fn default() -> Self {
        Self {
            tiles: BTreeMap::new(),
            tile_keys_by_chunk: BTreeMap::new(),
            generation: 0,
        }
    }
}

impl<K: ResidentTileKey, M> ResidentTileCache<K, M> {
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    pub fn contains_chunk(&self, pos: ChunkPos) -> bool {
        self.tile_keys_by_chunk.contains_key(&pos)
    }

    pub fn contains_tile(&self, key: K) -> bool {
        self.tiles.contains_key(&key)
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn metadata(&self) -> impl Iterator<Item = &M> {
        self.tiles.values().map(|slot| &slot.metadata)
    }

    pub fn tile_keys(&self) -> impl Iterator<Item = K> + '_ {
        self.tiles.keys().copied()
    }

    pub fn tile_keys_for_chunk(&self, pos: ChunkPos) -> impl Iterator<Item = K> + '_ {
        self.tile_keys_by_chunk
            .get(&pos)
            .into_iter()
            .flat_map(|keys| keys.iter().copied())
    }

    pub fn has_dirty_tiles(&self) -> bool {
        self.tiles.values().any(|slot| slot.dirty)
    }

    pub fn dirty_tile_count(&self) -> usize {
        self.tiles.values().filter(|slot| slot.dirty).count()
    }

    pub fn dirty_tile_keys(&self) -> impl Iterator<Item = K> + '_ {
        self.tiles
            .iter()
            .filter_map(|(key, slot)| slot.dirty.then_some(*key))
    }

    pub fn mark_tile_dirty(&mut self, key: K) -> bool {
        let Some(slot) = self.tiles.get_mut(&key) else {
            return false;
        };
        slot.dirty = true;
        true
    }

    pub fn mark_all_tiles_dirty(&mut self) -> usize {
        let mut marked = 0;
        for slot in self.tiles.values_mut() {
            if !slot.dirty {
                marked += 1;
            }
            slot.dirty = true;
        }
        marked
    }

    pub fn clear_tile_dirty(&mut self, key: K) -> bool {
        let Some(slot) = self.tiles.get_mut(&key) else {
            return false;
        };
        slot.dirty = false;
        true
    }

    /// Apply one producer build/removal report through the shared residency
    /// diff. Payloads remain producer-owned and are returned for upload; only
    /// compact metadata enters the resident cache.
    pub fn apply_build_report<P>(
        &mut self,
        ready_tile_keys: &BTreeSet<K>,
        rebuilt_tiles: Vec<P>,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_tile_keys: &BTreeSet<K>,
        mut tile_key: impl FnMut(&P) -> K,
        mut tile_metadata: impl FnMut(&P) -> M,
    ) -> ResidentTileCacheUpdate<K, P> {
        let rebuilt_keys = rebuilt_tiles
            .iter()
            .map(&mut tile_key)
            .collect::<BTreeSet<_>>();
        let rebuilt_tiles = rebuilt_tiles
            .into_iter()
            .filter(|tile| ready_tile_keys.contains(&tile_key(tile)))
            .collect::<Vec<_>>();
        let old_ready_keys = self
            .tile_keys()
            .filter(|key| ready_tile_keys.contains(key))
            .collect::<BTreeSet<_>>();
        let removal_keys = self
            .tile_keys()
            .filter(|key| removal_chunks.contains(&key.chunk_pos()))
            .collect::<BTreeSet<_>>();
        let mut removed_tile_keys = old_ready_keys
            .difference(&rebuilt_keys)
            .copied()
            .collect::<BTreeSet<_>>();
        removed_tile_keys.extend(removal_keys);
        removed_tile_keys.extend(removal_tile_keys.iter().copied());

        for key in &removed_tile_keys {
            self.remove(*key);
        }
        for tile in &rebuilt_tiles {
            self.insert(tile_key(tile), tile_metadata(tile));
        }

        ResidentTileCacheUpdate {
            rebuilt_tiles,
            removed_tile_keys,
        }
    }

    pub fn insert(&mut self, key: K, metadata: M) -> Option<M> {
        let replaced = self
            .tiles
            .insert(
                key,
                ResidentTileSlot {
                    metadata,
                    dirty: false,
                },
            )
            .map(|slot| slot.metadata);
        if replaced.is_none() {
            self.generation = self.generation.wrapping_add(1);
            self.tile_keys_by_chunk
                .entry(key.chunk_pos())
                .or_default()
                .insert(key);
        }
        replaced
    }

    pub fn remove(&mut self, key: K) -> Option<M> {
        let metadata = self.tiles.remove(&key)?.metadata;
        self.generation = self.generation.wrapping_add(1);
        let pos = key.chunk_pos();
        if let Some(keys) = self.tile_keys_by_chunk.get_mut(&pos) {
            keys.remove(&key);
            if keys.is_empty() {
                self.tile_keys_by_chunk.remove(&pos);
            }
        }
        Some(metadata)
    }
}

/// Payload facts required by the neutral upload queue.
pub trait ResidentTileUploadPayload<K> {
    fn tile_key(&self) -> K;

    fn estimated_owned_bytes(&self) -> usize;
}

impl ResidentTileUploadPayload<RenderSectionKey> for TexturedRenderSectionMesh {
    fn tile_key(&self) -> RenderSectionKey {
        self.key
    }

    fn estimated_owned_bytes(&self) -> usize {
        TexturedRenderSectionMesh::estimated_owned_bytes(self)
    }
}

impl ResidentTileUploadPayload<mclone_core::LodTileKey>
    for mclone_render::far_lod::FarTerrainLodTileMesh
{
    fn tile_key(&self) -> mclone_core::LodTileKey {
        self.key()
    }

    fn estimated_owned_bytes(&self) -> usize {
        self.estimated_owned_bytes()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ResidentTileReleaseBatch {
    remaining_lifecycle_items: usize,
    producer_jobs: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResidentTileUploadQueueStats {
    pub queued_uploads: usize,
    pub queued_removals: usize,
    pub queued_lifecycle_items: usize,
    pub queued_upload_owned_bytes: usize,
    pub held_release_batches: usize,
    pub held_release_lifecycle_items: usize,
    pub held_producer_jobs: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResidentTileUploadEnqueueReport {
    pub queued_lifecycle_items: usize,
    pub superseded_lifecycle_items: usize,
    pub released_producer_jobs: usize,
}

#[derive(Clone, Debug)]
pub struct ResidentTileUploadDrain<K, P> {
    pub uploads: Vec<P>,
    pub removed_tile_keys: BTreeSet<K>,
    pub lifecycle_item_count: usize,
    pub upload_limited: bool,
    pub accept_limited: bool,
}

#[derive(Clone, Debug)]
pub struct ResidentTileUploadCoordinator<K, P> {
    pending_uploads: VecDeque<P>,
    pending_removals: BTreeSet<K>,
    pending_release_batches: VecDeque<ResidentTileReleaseBatch>,
}

impl<K, P> Default for ResidentTileUploadCoordinator<K, P> {
    fn default() -> Self {
        Self {
            pending_uploads: VecDeque::new(),
            pending_removals: BTreeSet::new(),
            pending_release_batches: VecDeque::new(),
        }
    }
}

impl<K, P> ResidentTileUploadCoordinator<K, P>
where
    K: Copy + Ord,
    P: ResidentTileUploadPayload<K>,
{
    pub fn clear(&mut self) {
        self.pending_uploads.clear();
        self.pending_removals.clear();
        self.pending_release_batches.clear();
    }

    pub fn has_pending_work(&self) -> bool {
        !self.pending_uploads.is_empty() || !self.pending_removals.is_empty()
    }

    pub fn queued_upload_count(&self) -> usize {
        self.pending_uploads.len()
    }

    pub fn queued_removal_count(&self) -> usize {
        self.pending_removals.len()
    }

    pub fn stats(&self) -> ResidentTileUploadQueueStats {
        ResidentTileUploadQueueStats {
            queued_uploads: self.pending_uploads.len(),
            queued_removals: self.pending_removals.len(),
            queued_lifecycle_items: self.pending_uploads.len() + self.pending_removals.len(),
            queued_upload_owned_bytes: self.pending_uploads.iter().fold(0usize, |bytes, tile| {
                bytes.saturating_add(tile.estimated_owned_bytes())
            }),
            held_release_batches: self.pending_release_batches.len(),
            held_release_lifecycle_items: self
                .pending_release_batches
                .iter()
                .map(|batch| batch.remaining_lifecycle_items)
                .sum(),
            held_producer_jobs: self
                .pending_release_batches
                .iter()
                .map(|batch| batch.producer_jobs)
                .sum(),
        }
    }

    pub fn enqueue(
        &mut self,
        uploads: Vec<P>,
        removals: BTreeSet<K>,
        producer_jobs: usize,
    ) -> ResidentTileUploadEnqueueReport {
        let mut report = ResidentTileUploadEnqueueReport::default();
        for key in removals {
            let before_uploads = self.pending_uploads.len();
            self.pending_uploads
                .retain(|payload| payload.tile_key() != key);
            report.superseded_lifecycle_items += before_uploads - self.pending_uploads.len();
            if self.pending_removals.insert(key) {
                report.queued_lifecycle_items += 1;
            }
        }
        for payload in uploads {
            let key = payload.tile_key();
            let before_uploads = self.pending_uploads.len();
            self.pending_uploads
                .retain(|pending| pending.tile_key() != key);
            report.superseded_lifecycle_items += before_uploads - self.pending_uploads.len();
            if self.pending_removals.remove(&key) {
                report.superseded_lifecycle_items += 1;
            }
            self.pending_uploads.push_back(payload);
            report.queued_lifecycle_items += 1;
        }
        report.released_producer_jobs =
            self.complete_release_work(report.superseded_lifecycle_items);
        report.released_producer_jobs +=
            self.queue_release_batch(producer_jobs, report.queued_lifecycle_items);
        report
    }

    pub fn drain_budgeted(
        &mut self,
        upload_budget: Option<usize>,
        accept_budget: Option<usize>,
    ) -> ResidentTileUploadDrain<K, P> {
        let accept_limit = accept_budget.unwrap_or(usize::MAX);
        let upload_limit = upload_budget.unwrap_or(usize::MAX);
        let upload_count = upload_limit
            .min(accept_limit)
            .min(self.pending_uploads.len());
        let mut uploads = Vec::with_capacity(upload_count);
        for _ in 0..upload_count {
            if let Some(payload) = self.pending_uploads.pop_front() {
                uploads.push(payload);
            }
        }
        let removed_tile_keys = if accept_budget.is_some() {
            let remaining_accept_budget = accept_limit.saturating_sub(uploads.len());
            self.take_budgeted_removals(remaining_accept_budget)
        } else {
            std::mem::take(&mut self.pending_removals)
        };
        let lifecycle_item_count = uploads.len() + removed_tile_keys.len();
        let queue_after = self.stats();
        let upload_limited = upload_budget.is_some()
            && upload_count == upload_limit
            && queue_after.queued_uploads > 0;
        let accept_limited = accept_budget.is_some()
            && lifecycle_item_count == accept_limit
            && queue_after.queued_lifecycle_items > 0;
        ResidentTileUploadDrain {
            uploads,
            removed_tile_keys,
            lifecycle_item_count,
            upload_limited,
            accept_limited,
        }
    }

    pub fn complete_applied_lifecycle_items(&mut self, lifecycle_items: usize) -> usize {
        self.complete_release_work(lifecycle_items)
    }

    fn take_budgeted_removals(&mut self, budget: usize) -> BTreeSet<K> {
        if budget == 0 || self.pending_removals.is_empty() {
            return BTreeSet::new();
        }
        if budget >= self.pending_removals.len() {
            return std::mem::take(&mut self.pending_removals);
        }
        let keys: Vec<_> = self.pending_removals.iter().copied().take(budget).collect();
        for key in &keys {
            self.pending_removals.remove(key);
        }
        keys.into_iter().collect()
    }

    fn queue_release_batch(&mut self, producer_jobs: usize, lifecycle_items: usize) -> usize {
        if producer_jobs == 0 {
            return 0;
        }
        if lifecycle_items == 0 {
            return producer_jobs;
        }
        self.pending_release_batches
            .push_back(ResidentTileReleaseBatch {
                remaining_lifecycle_items: lifecycle_items,
                producer_jobs,
            });
        0
    }

    fn complete_release_work(&mut self, mut lifecycle_items: usize) -> usize {
        let mut released_producer_jobs = 0;
        while lifecycle_items > 0 {
            let Some(front) = self.pending_release_batches.front_mut() else {
                break;
            };
            if lifecycle_items < front.remaining_lifecycle_items {
                front.remaining_lifecycle_items -= lifecycle_items;
                break;
            }
            lifecycle_items -= front.remaining_lifecycle_items;
            released_producer_jobs += front.producer_jobs;
            self.pending_release_batches.pop_front();
        }
        released_producer_jobs
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResidentTileUploadFramePolicy {
    pub upload_budget: Option<usize>,
    pub accept_budget: Option<usize>,
}

impl ResidentTileUploadFramePolicy {
    pub const fn new(upload_budget: Option<usize>, accept_budget: Option<usize>) -> Self {
        Self {
            upload_budget,
            accept_budget,
        }
    }

    pub const fn is_budgeted(self) -> bool {
        self.upload_budget.is_some() || self.accept_budget.is_some()
    }
}
