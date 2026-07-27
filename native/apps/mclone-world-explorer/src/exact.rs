use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
#[cfg(not(target_arch = "wasm32"))]
use std::thread;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use anyhow::bail;
use anyhow::{Context, Result};
use mclone_core::ChunkPos;
#[cfg(not(target_arch = "wasm32"))]
use mclone_mesh::TexturedMeshCatalog;
use mclone_mesh::{
    RenderSectionKey, TexturedRenderSectionMesh, merge_textured_render_section_meshes,
    unpack_textured_render_sections,
};
use mclone_render::chunk::{
    ChunkDepthTarget, ChunkRenderTarget, ChunkRenderView, ChunkTextureAtlas, ChunkTextureSampling,
    TexturedSectionDrawResources, TexturedSectionRenderOptions, TexturedSectionRenderStats,
};
use mclone_render_color::{RenderColorProfile, RenderTargetColorTransform, color_transform_wgpu};
use mclone_terrain_view::{
    BoundedRepresentationOwnershipSnapshot, CanonicalMeshBatch, CanonicalMeshCoordinate,
    CanonicalPackedAdmission, ExactPaintedCoverageSnapshot, McloneTreeOccurrenceId,
    McloneTreeOwnershipCandidate, TERRAIN_EXACT_FRONTIER_COLLAR_BLOCKS,
    TerrainCompositionSourceIdentity, canonical_terrain_chunk_order,
    mclone_tree_ownership_snapshot,
};
#[cfg(not(target_arch = "wasm32"))]
use mclone_terrain_view::{
    CanonicalMeshFrontier, CanonicalMeshSession, CanonicalNaturalTreePresentation,
    CanonicalTerrainStage, CanonicalTerrainVisibility,
};
use mclone_worldgen::levelgen::McloneTreeOccurrence;
use mclone_worldgen::terrain_preview::TerrainPreviewProfile;

const EXACT_COMPILE_BATCH_CHUNKS: usize = 4;
#[cfg(not(target_arch = "wasm32"))]
const EXACT_COMMAND_CAPACITY: usize = 1;
#[cfg(not(target_arch = "wasm32"))]
const EXACT_RESULT_CAPACITY: usize = 2;

pub(crate) struct CanonicalExactRequest {
    pub generation: u64,
    pub desired: Vec<CanonicalMeshCoordinate>,
    pub requested: Vec<CanonicalMeshCoordinate>,
}

pub(crate) struct CanonicalExactResult {
    pub generation: u64,
    pub batch: Result<CanonicalMeshBatch, String>,
}

pub(crate) trait CanonicalExactExecutor {
    fn try_submit(&mut self, request: CanonicalExactRequest) -> Result<bool>;
    fn poll(&mut self) -> Result<Option<CanonicalExactResult>>;
}

#[cfg(not(target_arch = "wasm32"))]
enum NativeExactCommand {
    Compile(CanonicalExactRequest),
    Shutdown,
}

#[cfg(not(target_arch = "wasm32"))]
struct NativeCanonicalExactExecutor {
    commands: SyncSender<NativeExactCommand>,
    results: Receiver<CanonicalExactResult>,
    worker: Option<thread::JoinHandle<()>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeCanonicalExactExecutor {
    fn new(seed: i64, catalog: TexturedMeshCatalog, compile_delay: Duration) -> Result<Self> {
        let (commands, command_receiver) = sync_channel(EXACT_COMMAND_CAPACITY);
        let (result_sender, results) = sync_channel(EXACT_RESULT_CAPACITY);
        let worker = thread::Builder::new()
            .name("mclone-world-explorer-exact".to_owned())
            .spawn(move || {
                let mut session = CanonicalMeshSession::new(
                    TerrainPreviewProfile::McloneOverworldV1,
                    seed,
                    CanonicalTerrainStage::FinalFeatures,
                    catalog,
                );
                session.set_frontier(CanonicalMeshFrontier::SuppressFootprintWalls);
                session.set_natural_tree_presentation(CanonicalNaturalTreePresentation::Separated);
                while let Ok(command) = command_receiver.recv() {
                    match command {
                        NativeExactCommand::Compile(request) => {
                            session.begin(
                                request.desired,
                                CanonicalTerrainVisibility::default(),
                                true,
                            );
                            if !compile_delay.is_zero() {
                                thread::sleep(compile_delay);
                            }
                            let batch = session.compile_batch(&request.requested);
                            if result_sender
                                .send(CanonicalExactResult {
                                    generation: request.generation,
                                    batch,
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                        NativeExactCommand::Shutdown => break,
                    }
                }
            })
            .context("failed to spawn World Explorer exact terrain thread")?;
        Ok(Self {
            commands,
            results,
            worker: Some(worker),
        })
    }

    fn try_submit_inner(&self, request: CanonicalExactRequest) -> Result<bool> {
        match self.commands.try_send(NativeExactCommand::Compile(request)) {
            Ok(()) => Ok(true),
            Err(TrySendError::Full(_)) => Ok(false),
            Err(TrySendError::Disconnected(_)) => {
                bail!("World Explorer exact terrain thread disconnected")
            }
        }
    }

    fn poll_inner(&self) -> Result<Option<CanonicalExactResult>> {
        match self.results.try_recv() {
            Ok(result) => Ok(Some(result)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                bail!("World Explorer exact terrain result channel disconnected")
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl CanonicalExactExecutor for NativeCanonicalExactExecutor {
    fn try_submit(&mut self, request: CanonicalExactRequest) -> Result<bool> {
        self.try_submit_inner(request)
    }

    fn poll(&mut self) -> Result<Option<CanonicalExactResult>> {
        self.poll_inner()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for NativeCanonicalExactExecutor {
    fn drop(&mut self) {
        let _ = self.commands.try_send(NativeExactCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct PendingExactAdmission {
    generation: u64,
    admission: CanonicalPackedAdmission,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ExplorerExactStats {
    pub desired_chunks: u32,
    pub painted_chunks: u32,
    pub queued_chunks: u32,
    pub pending_admissions: u32,
    pub in_flight: bool,
    pub coverage_generation: u64,
    pub admitted_chunks_total: u64,
    pub stale_chunks_total: u64,
    pub generation_ms: f64,
    pub presentation_ms: f64,
    pub mesh_ms: f64,
    pub pack_ms: f64,
    pub resident_mesh_bytes: u64,
    pub vertex_count: u32,
    pub index_count: u32,
    pub drawn_sections: u32,
    pub drawn_indices: u32,
    pub natural_tree_records: u32,
    pub exact_owned_tree_records: u32,
    pub proxy_owned_tree_records: u32,
    pub exact_tree_sections: u32,
    pub exact_tree_indices: u32,
    pub complete: bool,
}

pub struct ExplorerExactTerrain {
    draw: TexturedSectionDrawResources,
    tree_draw: TexturedSectionDrawResources,
    depth: ChunkDepthTarget,
    executor: Box<dyn CanonicalExactExecutor>,
    source: TerrainCompositionSourceIdentity,
    radius: u32,
    generation: u64,
    coverage_generation: u64,
    desired: BTreeSet<ChunkPos>,
    painted: BTreeSet<ChunkPos>,
    queued: VecDeque<CanonicalMeshCoordinate>,
    pending: VecDeque<PendingExactAdmission>,
    in_flight: bool,
    sections_by_chunk: BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>>,
    tree_occurrences: BTreeMap<McloneTreeOccurrenceId, McloneTreeOccurrence>,
    tree_sections:
        BTreeMap<McloneTreeOccurrenceId, BTreeMap<RenderSectionKey, TexturedRenderSectionMesh>>,
    tree_gpu_sections: BTreeSet<RenderSectionKey>,
    tree_ownership: BoundedRepresentationOwnershipSnapshot<McloneTreeOccurrenceId>,
    clear_color: wgpu::Color,
    admitted_chunks_total: u64,
    stale_chunks_total: u64,
    generation_ms: f64,
    presentation_ms: f64,
    mesh_ms: f64,
    pack_ms: f64,
    last_render: TexturedSectionRenderStats,
    last_tree_render: TexturedSectionRenderStats,
}

impl ExplorerExactTerrain {
    #[allow(clippy::too_many_arguments)]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        seed: i64,
        radius: u32,
        compile_delay: Duration,
        catalog: TexturedMeshCatalog,
        atlas: ChunkTextureAtlas<'_>,
        target_color_transform: RenderTargetColorTransform,
    ) -> Result<Self> {
        let executor = NativeCanonicalExactExecutor::new(seed, catalog.clone(), compile_delay)?;
        Self::new_with_executor(
            device,
            queue,
            color_format,
            width,
            height,
            seed,
            radius,
            Box::new(executor),
            atlas,
            target_color_transform,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_executor(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        seed: i64,
        radius: u32,
        executor: Box<dyn CanonicalExactExecutor>,
        atlas: ChunkTextureAtlas<'_>,
        target_color_transform: RenderTargetColorTransform,
    ) -> Result<Self> {
        let tree_atlas = ChunkTextureAtlas {
            width: atlas.width,
            height: atlas.height,
            rgba: atlas.rgba,
        };
        let draw = TexturedSectionDrawResources::new_with_texture_sampling(
            device,
            queue,
            color_format,
            &[],
            atlas,
            ChunkTextureSampling::TerrainOverview,
        )
        .context("failed to initialize World Explorer exact terrain renderer")?;
        let tree_draw = TexturedSectionDrawResources::new_with_texture_sampling(
            device,
            queue,
            color_format,
            &[],
            tree_atlas,
            ChunkTextureSampling::TerrainOverview,
        )
        .context("failed to initialize World Explorer exact natural-tree renderer")?;
        let source =
            TerrainCompositionSourceIdentity::new(TerrainPreviewProfile::McloneOverworldV1, seed);
        Ok(Self {
            draw,
            tree_draw,
            depth: ChunkDepthTarget::new(device, width, height),
            executor,
            source,
            radius,
            generation: 0,
            coverage_generation: 1,
            desired: BTreeSet::new(),
            painted: BTreeSet::new(),
            queued: VecDeque::new(),
            pending: VecDeque::new(),
            in_flight: false,
            sections_by_chunk: BTreeMap::new(),
            tree_occurrences: BTreeMap::new(),
            tree_sections: BTreeMap::new(),
            tree_gpu_sections: BTreeSet::new(),
            tree_ownership: BoundedRepresentationOwnershipSnapshot::new(source, 1, [])
                .expect("an empty initial tree ownership snapshot is valid"),
            clear_color: color_transform_wgpu(
                wgpu::Color {
                    r: 0.025,
                    g: 0.035,
                    b: 0.055,
                    a: 1.0,
                },
                target_color_transform,
            ),
            admitted_chunks_total: 0,
            stale_chunks_total: 0,
            generation_ms: 0.0,
            presentation_ms: 0.0,
            mesh_ms: 0.0,
            pack_ms: 0.0,
            last_render: TexturedSectionRenderStats::default(),
            last_tree_render: TexturedSectionRenderStats::default(),
        })
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.depth.resize(device, width, height);
    }

    pub fn depth(&self) -> &ChunkDepthTarget {
        &self.depth
    }

    pub const fn clear_color(&self) -> wgpu::Color {
        self.clear_color
    }

    pub fn update_and_pump(
        &mut self,
        device: &wgpu::Device,
        center_x: i32,
        center_z: i32,
    ) -> Result<()> {
        self.update_desired(device, center_x, center_z)?;
        self.poll_worker()?;
        self.admit_one(device)?;
        self.submit_next()?;
        Ok(())
    }

    pub fn coverage_snapshot(&self) -> Result<ExactPaintedCoverageSnapshot, String> {
        ExactPaintedCoverageSnapshot::new(
            self.source,
            self.coverage_generation,
            self.painted.iter().copied(),
        )
    }

    pub fn tree_ownership(
        &self,
    ) -> &BoundedRepresentationOwnershipSnapshot<McloneTreeOccurrenceId> {
        &self.tree_ownership
    }

    pub fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        render_view: ChunkRenderView,
        size: [u32; 2],
        load_existing: bool,
    ) -> Result<TexturedSectionRenderStats> {
        let mut target =
            ChunkRenderTarget::new(color_view, &self.depth.view, size, self.clear_color);
        if load_existing {
            target = target.with_loaded_color().with_loaded_depth();
        }
        let stats = self
            .draw
            .render_with_options(
                queue,
                encoder,
                target,
                render_view,
                TexturedSectionRenderOptions {
                    section_occlusion_culling: false,
                    force_fullbright: true,
                    color_profile: RenderColorProfile::Vanilla,
                    ..TexturedSectionRenderOptions::default()
                },
            )
            .context("failed to render World Explorer exact terrain")?;
        self.last_render = stats;
        let tree_target =
            ChunkRenderTarget::new(color_view, &self.depth.view, size, self.clear_color)
                .with_loaded_color()
                .with_loaded_depth();
        self.last_tree_render = self
            .tree_draw
            .render_with_options(
                queue,
                encoder,
                tree_target,
                render_view,
                TexturedSectionRenderOptions {
                    section_occlusion_culling: false,
                    force_fullbright: true,
                    color_profile: RenderColorProfile::Vanilla,
                    ..TexturedSectionRenderOptions::default()
                },
            )
            .context("failed to render World Explorer exact natural trees")?;
        Ok(stats)
    }

    pub fn stats(&self) -> ExplorerExactStats {
        ExplorerExactStats {
            desired_chunks: self.desired.len().try_into().unwrap_or(u32::MAX),
            painted_chunks: self.painted.len().try_into().unwrap_or(u32::MAX),
            queued_chunks: self.queued.len().try_into().unwrap_or(u32::MAX),
            pending_admissions: self.pending.len().try_into().unwrap_or(u32::MAX),
            in_flight: self.in_flight,
            coverage_generation: self.coverage_generation,
            admitted_chunks_total: self.admitted_chunks_total,
            stale_chunks_total: self.stale_chunks_total,
            generation_ms: self.generation_ms,
            presentation_ms: self.presentation_ms,
            mesh_ms: self.mesh_ms,
            pack_ms: self.pack_ms,
            resident_mesh_bytes: self
                .draw
                .resident_mesh_used_bytes()
                .saturating_add(self.tree_draw.resident_mesh_used_bytes()),
            vertex_count: self
                .draw
                .vertex_count()
                .saturating_add(self.tree_draw.vertex_count()),
            index_count: self
                .draw
                .index_count()
                .saturating_add(self.tree_draw.index_count()),
            drawn_sections: self
                .last_render
                .drawn_section_count
                .saturating_add(self.last_tree_render.drawn_section_count)
                .try_into()
                .unwrap_or(u32::MAX),
            drawn_indices: self
                .last_render
                .drawn_index_count
                .saturating_add(self.last_tree_render.drawn_index_count),
            natural_tree_records: self
                .tree_ownership
                .units()
                .len()
                .try_into()
                .unwrap_or(u32::MAX),
            exact_owned_tree_records: self
                .tree_ownership
                .exact_owned_ids()
                .count()
                .try_into()
                .unwrap_or(u32::MAX),
            proxy_owned_tree_records: self
                .tree_ownership
                .approximate_owned_ids()
                .count()
                .try_into()
                .unwrap_or(u32::MAX),
            exact_tree_sections: self
                .last_tree_render
                .drawn_section_count
                .try_into()
                .unwrap_or(u32::MAX),
            exact_tree_indices: self.last_tree_render.drawn_index_count,
            complete: self.is_complete(),
        }
    }

    fn update_desired(
        &mut self,
        device: &wgpu::Device,
        center_x: i32,
        center_z: i32,
    ) -> Result<()> {
        let ordered = canonical_terrain_chunk_order(center_x, center_z, self.radius);
        let desired = ordered.iter().copied().collect::<BTreeSet<_>>();
        if desired == self.desired {
            return Ok(());
        }
        self.generation = self.generation.wrapping_add(1).max(1);
        self.pending.clear();
        self.queued.clear();

        let departed = self
            .painted
            .difference(&desired)
            .copied()
            .collect::<Vec<_>>();
        let mut removed = BTreeSet::new();
        for chunk in &departed {
            self.painted.remove(chunk);
            if let Some(keys) = self.sections_by_chunk.remove(chunk) {
                removed.extend(keys);
            }
        }
        if !removed.is_empty() {
            self.draw
                .apply_section_updates(device, &[], &removed)
                .context("failed to evict departed World Explorer exact chunks")?;
        }
        for sections in self.tree_sections.values_mut() {
            sections.retain(|key, _| !departed.contains(&ChunkPos::new(key.chunk_x, key.chunk_z)));
        }
        self.tree_sections
            .retain(|_, sections| !sections.is_empty());
        self.tree_occurrences
            .retain(|id, _| self.tree_sections.contains_key(id));
        self.desired = desired;
        self.queued.extend(
            ordered
                .into_iter()
                .map(|position| CanonicalMeshCoordinate::new(position.x, position.z)),
        );
        self.refresh_readiness();
        self.coverage_generation = self.coverage_generation.wrapping_add(1).max(1);
        self.rebuild_tree_ownership(device)?;
        Ok(())
    }

    fn poll_worker(&mut self) -> Result<()> {
        let Some(result) = self.executor.poll()? else {
            return Ok(());
        };
        self.in_flight = false;
        let batch = result
            .batch
            .map_err(anyhow::Error::msg)
            .context("World Explorer exact terrain compilation failed")?;
        if result.generation != self.generation {
            self.stale_chunks_total = self
                .stale_chunks_total
                .saturating_add(batch.admissions.len() as u64);
            return Ok(());
        }
        self.generation_ms += batch.generation_ms;
        self.presentation_ms += batch.presentation_ms;
        self.mesh_ms += batch.mesh_ms;
        self.pack_ms += batch.pack_ms;
        self.pending.extend(
            batch
                .admissions
                .into_iter()
                .map(|admission| PendingExactAdmission {
                    generation: result.generation,
                    admission,
                }),
        );
        Ok(())
    }

    fn admit_one(&mut self, device: &wgpu::Device) -> Result<()> {
        let Some(pending) = self.pending.pop_front() else {
            return Ok(());
        };
        let coordinate = pending.admission.requested.coordinate;
        let requested = ChunkPos::new(coordinate.chunk_x, coordinate.chunk_z);
        if pending.generation != self.generation || !self.desired.contains(&requested) {
            self.stale_chunks_total = self.stale_chunks_total.saturating_add(1);
            return Ok(());
        }
        let sections = unpack_textured_render_sections(&pending.admission.packed_sections)
            .context("invalid World Explorer canonical packed mesh")?;
        let mut target_chunks = sections
            .iter()
            .map(|section| ChunkPos::new(section.key.chunk_x, section.key.chunk_z))
            .collect::<BTreeSet<_>>();
        target_chunks.insert(requested);
        let mut removed = BTreeSet::new();
        for target in target_chunks {
            let next = sections
                .iter()
                .filter(|section| {
                    section.key.chunk_x == target.x && section.key.chunk_z == target.z
                })
                .map(|section| section.key)
                .collect::<BTreeSet<_>>();
            if let Some(previous) = self.sections_by_chunk.insert(target, next.clone()) {
                removed.extend(previous.difference(&next).copied());
            }
        }
        self.draw
            .apply_section_updates(device, &sections, &removed)
            .context("failed to upload World Explorer canonical packed mesh")?;
        for tree in pending.admission.natural_trees {
            let id = McloneTreeOccurrenceId::from(tree.occurrence);
            let sections = unpack_textured_render_sections(&tree.packed_sections)
                .context("invalid World Explorer canonical natural-tree mesh")?;
            self.tree_occurrences.insert(id, tree.occurrence);
            let resident = self.tree_sections.entry(id).or_default();
            for section in sections {
                resident.insert(section.key, section);
            }
        }
        if self.painted.insert(requested) {
            self.coverage_generation = self.coverage_generation.wrapping_add(1).max(1);
        }
        self.admitted_chunks_total = self.admitted_chunks_total.saturating_add(1);
        self.refresh_readiness();
        self.rebuild_tree_ownership(device)?;
        Ok(())
    }

    fn submit_next(&mut self) -> Result<()> {
        if self.in_flight || self.queued.is_empty() {
            return Ok(());
        }
        let mut requested = Vec::with_capacity(EXACT_COMPILE_BATCH_CHUNKS);
        while requested.len() < EXACT_COMPILE_BATCH_CHUNKS {
            let Some(coordinate) = self.queued.pop_front() else {
                break;
            };
            requested.push(coordinate);
        }
        let desired = self
            .desired
            .iter()
            .map(|position| CanonicalMeshCoordinate::new(position.x, position.z))
            .collect();
        let request = CanonicalExactRequest {
            generation: self.generation,
            desired,
            requested,
        };
        match self.executor.try_submit(request)? {
            true => self.in_flight = true,
            false => {
                // The executor only admits one request at a time. A full command
                // queue is transient; rebuild the stable center-first work list
                // on the next frame instead of blocking presentation.
                self.queued = self
                    .desired
                    .iter()
                    .map(|position| CanonicalMeshCoordinate::new(position.x, position.z))
                    .collect();
            }
        }
        Ok(())
    }

    fn refresh_readiness(&mut self) {
        self.draw
            .set_traversal_ready_columns_with_context(&self.painted, false);
        self.tree_draw
            .set_traversal_ready_columns_with_context(&self.painted, false);
    }

    fn rebuild_tree_ownership(&mut self, device: &wgpu::Device) -> Result<()> {
        let coverage = self.coverage_snapshot().map_err(anyhow::Error::msg)?;
        self.tree_ownership = mclone_tree_ownership_snapshot(
            self.source,
            self.coverage_generation,
            &coverage,
            TERRAIN_EXACT_FRONTIER_COLLAR_BLOCKS,
            self.tree_occurrences.values().copied().map(|occurrence| {
                let id = McloneTreeOccurrenceId::from(occurrence);
                McloneTreeOwnershipCandidate::new(
                    occurrence,
                    self.tree_sections
                        .get(&id)
                        .is_some_and(|sections| !sections.is_empty()),
                    true,
                )
            }),
        )
        .map_err(anyhow::Error::msg)?;

        let mut grouped = BTreeMap::<RenderSectionKey, Vec<&TexturedRenderSectionMesh>>::new();
        for id in self.tree_ownership.exact_owned_ids() {
            if let Some(sections) = self.tree_sections.get(id) {
                for section in sections.values() {
                    grouped.entry(section.key).or_default().push(section);
                }
            }
        }
        let merged = grouped
            .into_iter()
            .map(|(key, sections)| merge_textured_render_section_meshes(key, sections))
            .collect::<Vec<_>>();
        let next = merged
            .iter()
            .map(|section| section.key)
            .collect::<BTreeSet<_>>();
        let removed = self
            .tree_gpu_sections
            .difference(&next)
            .copied()
            .collect::<BTreeSet<_>>();
        self.tree_draw
            .apply_section_updates(device, &merged, &removed)
            .context("failed to upload World Explorer exact natural-tree ownership")?;
        self.tree_gpu_sections = next;
        Ok(())
    }

    fn is_complete(&self) -> bool {
        self.painted == self.desired
            && self.queued.is_empty()
            && self.pending.is_empty()
            && !self.in_flight
    }
}
