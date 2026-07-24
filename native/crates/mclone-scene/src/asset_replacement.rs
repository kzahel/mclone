use super::*;

use mclone_app_runtime::asset_pack_ui::ClientAssetPackEffect;
use mclone_app_runtime::prepared_assets::{
    AssetPackSourceRegistry, AssetPreparePoll, PreparedAssetReplacement,
    PreparedAssetReplacementRequest, PreparedSceneAssets, PreparedSceneAssetsRequest,
};
use mclone_mesh::RenderSectionKey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetReplacementStatus {
    Active { epoch: u64 },
    PreparingAssets { epoch: u64 },
    PreparingMeshes { epoch: u64 },
    Failed { active_epoch: u64, message: String },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AssetReplacementCommitReport {
    pub epoch: u64,
    pub section_count: usize,
    pub session_preserved: bool,
    pub camera_preserved: bool,
    pub command_count_unchanged: bool,
    pub update_count_unchanged: bool,
    pub preparation_ms: f64,
    pub compile_ms: f64,
    pub upload_ms: f64,
    pub total_ms: f64,
    pub peak_retained_cpu_bytes: usize,
    pub peak_retained_gpu_bytes: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssetPackRuntimeDiagnostics {
    pub epoch: u64,
    pub active_ids: Vec<String>,
    pub preferred_ids: Vec<String>,
    pub provenance: mclone_assets::AssetProvenanceSummary,
    pub proprietary_free: bool,
    pub preference_error: Option<String>,
    pub last_commit: Option<AssetReplacementCommitReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalAssetPackSelection {
    /// Resource-compatibility generation for the prepared asset bundle. This
    /// is not platform-operation identity; the surrounding operation token is.
    pub content_generation: u64,
    pub selection: mclone_assets::AssetPackSelection,
}

pub(crate) enum SceneAssetReplacementPending {
    Assets(PreparedSceneAssetsRequest),
    Meshes(PreparedAssetReplacementRequest),
}

impl McloneSceneHost {
    pub fn asset_replacement_status(&self) -> &AssetReplacementStatus {
        &self.asset_replacement_status
    }

    pub fn asset_pack_ui_state(&self) -> mclone_ui::AssetPacksUiState {
        self.client_experience.asset_packs().ui_state()
    }

    pub const fn active_asset_epoch(&self) -> u64 {
        self.active_assets.epoch
    }

    pub fn active_asset_pack_selection(&self) -> &AssetPackSelection {
        &self.active_assets.selection
    }

    pub fn begin_asset_replacement(&mut self, request: PreparedSceneAssetsRequest) -> Result<()> {
        let epoch = request.epoch();
        self.validate_replacement_epoch(epoch)?;
        self.asset_replacement = Some(SceneAssetReplacementPending::Assets(request));
        self.asset_replacement_started_at = Some(self.services.clock.now());
        self.asset_replacement_assets_ready_at = None;
        self.asset_replacement_status = AssetReplacementStatus::PreparingAssets { epoch };
        Ok(())
    }

    pub fn configure_asset_pack_sources(
        &mut self,
        registry: AssetPackSourceRegistry,
        active_selection: mclone_assets::AssetPackSelection,
    ) -> Result<()> {
        registry
            .catalog()
            .source_order(&active_selection)
            .context("active asset selection is invalid for configured sources")?;
        self.active_assets.selection = active_selection.clone();
        self.active_assets.provenance.selection = active_selection.clone();
        self.client_experience.asset_packs_mut().configure(
            registry.catalog().clone(),
            active_selection.clone(),
            &self.active_assets.provenance,
            self.active_assets.coverage,
        )?;
        self.asset_pack_sources = Some(registry);
        self.asset_pack_preference = AssetPackPreference::from_selection(&active_selection);
        self.external_asset_pack_preparation = false;
        self.pending_external_asset_pack_selection = None;
        let _ = self.external_asset_pack_operations.teardown();
        self.pending_restored_asset_pack_selection = None;
        Ok(())
    }

    /// Configure a platform-owned asynchronous preparation backend.
    ///
    /// Discovery projects only catalog facts into the shared controller. The
    /// platform retains fetched/staged bytes and later returns a complete
    /// `PreparedSceneAssets` bundle for the exact requested epoch/selection.
    pub fn configure_external_asset_pack_catalog(
        &mut self,
        catalog: AssetPackCatalog,
        active_selection: AssetPackSelection,
    ) -> Result<()> {
        catalog
            .source_order(&active_selection)
            .context("active asset selection is invalid for external preparation")?;
        self.active_assets.selection = active_selection.clone();
        self.active_assets.provenance.selection = active_selection.clone();
        self.client_experience.asset_packs_mut().configure(
            catalog,
            active_selection.clone(),
            &self.active_assets.provenance,
            self.active_assets.coverage,
        )?;
        self.asset_pack_sources = None;
        self.asset_pack_preference = AssetPackPreference::from_selection(&active_selection);
        self.external_asset_pack_preparation = true;
        self.pending_external_asset_pack_selection = None;
        let _ = self.external_asset_pack_operations.teardown();
        self.pending_restored_asset_pack_selection = None;
        Ok(())
    }

    /// Take one platform delivery while leaving identity/currentness in the
    /// shared operation ledger until completion or epoch teardown.
    pub fn take_external_asset_pack_selection(
        &mut self,
    ) -> Option<PlatformOperation<ExternalAssetPackSelection>> {
        self.pending_external_asset_pack_selection.take()
    }

    pub fn pending_external_asset_pack_preparation_count(&self) -> usize {
        self.external_asset_pack_operations.pending_len()
    }

    pub fn configure_asset_pack_preference_storage(
        &mut self,
        storage: Box<dyn AssetPackPreferenceStorage>,
    ) -> Result<()> {
        match storage.load() {
            Ok(Some(preference)) => {
                let resolution =
                    preference.reconcile(self.client_experience.asset_packs().catalog());
                self.asset_pack_preference = preference;
                if resolution.selection != self.active_assets.selection {
                    self.client_experience
                        .asset_packs_mut()
                        .begin_preferred_selection(resolution.selection.clone())?;
                    if self.active_world.runtime.is_some() {
                        self.request_asset_pack_selection(resolution.selection)?;
                    } else {
                        self.pending_restored_asset_pack_selection = Some(resolution.selection);
                    }
                }
            }
            Ok(None) => {}
            Err(error) => {
                self.asset_pack_preference_error =
                    Some(format!("failed to load {}: {error:#}", storage.label()));
            }
        }
        self.asset_pack_preference_storage = Some(storage);
        Ok(())
    }

    pub fn configure_graphics_preference_storage(
        &mut self,
        storage: Box<dyn ClientGraphicsPreferenceStorage>,
    ) -> Result<()> {
        match storage.load() {
            Ok(Some(preferences)) => {
                let detail = engine_leaf_detail(preferences.leaf_detail);
                if detail != self.mesh_assets.catalog.leaf_detail() {
                    if self.asset_replacement.is_none() {
                        if let Err(error) = self.request_leaf_detail(detail) {
                            self.graphics_preference_error =
                                Some(format!("failed to restore {}: {error:#}", storage.label()));
                            self.pending_restored_leaf_detail = Some(detail);
                        }
                    } else {
                        self.pending_restored_leaf_detail = Some(detail);
                    }
                }
            }
            Ok(None) => {}
            Err(error) => {
                self.graphics_preference_error =
                    Some(format!("failed to load {}: {error:#}", storage.label()));
            }
        }
        self.graphics_preference_storage = Some(storage);
        Ok(())
    }

    pub fn graphics_preference_error(&self) -> Option<&str> {
        self.graphics_preference_error.as_deref()
    }

    pub fn preferred_asset_pack_ids(&self) -> impl Iterator<Item = &mclone_assets::AssetPackId> {
        self.asset_pack_preference.enabled_ids()
    }

    pub fn asset_pack_preference_error(&self) -> Option<&str> {
        self.asset_pack_preference_error.as_deref()
    }

    pub fn asset_pack_runtime_diagnostics(&self) -> AssetPackRuntimeDiagnostics {
        let provenance = self.active_assets.provenance.summary();
        AssetPackRuntimeDiagnostics {
            epoch: self.active_assets.epoch,
            active_ids: self
                .active_assets
                .selection
                .enabled_ids()
                .map(|id| id.as_str().to_owned())
                .collect(),
            preferred_ids: self
                .asset_pack_preference
                .enabled_ids()
                .map(|id| id.as_str().to_owned())
                .collect(),
            proprietary_free: provenance.minecraft_reference == 0 && provenance.unknown == 0,
            provenance,
            preference_error: self.asset_pack_preference_error.clone(),
            last_commit: self.last_asset_replacement_commit,
        }
    }

    /// Complete browser/other asynchronous CPU preparation and synchronously
    /// build the currently visible replacement sections. Native platforms use
    /// their background preparation/compiler request; this portable fallback
    /// keeps the same transactional frame-boundary commit contract.
    pub fn complete_external_asset_pack_preparation(
        &mut self,
        token: mclone_app_runtime::platform_operation::PlatformOperationToken,
        result: Result<PreparedSceneAssets, String>,
    ) -> Result<bool> {
        let resolution = self.external_asset_pack_operations.complete(
            mclone_app_runtime::platform_operation::PlatformOperationCompletion { token, result },
        );
        let (pending, mut assets) = match resolution {
            mclone_app_runtime::platform_operation::PlatformOperationResolution::Applied {
                kind,
                value,
                ..
            } => (kind, value),
            mclone_app_runtime::platform_operation::PlatformOperationResolution::Failed {
                error,
                ..
            } => {
                self.pending_external_asset_pack_selection = None;
                self.fail_asset_replacement(error);
                return Ok(true);
            }
            mclone_app_runtime::platform_operation::PlatformOperationResolution::Stale(_)
            | mclone_app_runtime::platform_operation::PlatformOperationResolution::Duplicate(_)
            | mclone_app_runtime::platform_operation::PlatformOperationResolution::Unknown(_) => {
                return Ok(false);
            }
        };
        self.pending_external_asset_pack_selection = None;
        if assets.epoch != pending.content_generation || assets.selection != pending.selection {
            let message = format!(
                "external asset preparation mismatch: expected epoch {} selection {:?}, got epoch {} selection {:?}",
                pending.content_generation, pending.selection, assets.epoch, assets.selection
            );
            self.fail_asset_replacement(message.clone());
            bail!(message);
        }
        assets.mesh.catalog = assets
            .mesh
            .catalog
            .with_leaf_detail(self.mesh_assets.catalog.leaf_detail());
        self.asset_replacement_assets_ready_at = Some(self.services.clock.now());
        let (snapshots, target_sections) = self.current_asset_compile_inputs()?;
        let replacement = PreparedAssetReplacement::build(
            assets,
            snapshots,
            target_sections,
            self.current_biome_zoom_seed(),
        )?;
        let epoch = replacement.assets.epoch;
        self.asset_replacement = Some(SceneAssetReplacementPending::Meshes(
            PreparedAssetReplacementRequest::ready(replacement),
        ));
        self.asset_replacement_status = AssetReplacementStatus::PreparingMeshes { epoch };
        self.client_experience
            .asset_packs_mut()
            .mark_preparing_meshes();
        Ok(true)
    }

    pub(crate) fn apply_asset_pack_effects(&mut self, effects: Vec<ClientAssetPackEffect>) {
        for effect in effects {
            match effect {
                ClientAssetPackEffect::ApplySelection(selection) => {
                    let result = self.request_asset_pack_selection(selection);
                    if let Err(error) = result {
                        self.client_experience
                            .asset_packs_mut()
                            .mark_failed(format!("{error:#}"));
                    }
                }
            }
        }
    }

    fn request_asset_pack_selection(&mut self, selection: AssetPackSelection) -> Result<()> {
        let epoch = self.active_assets.epoch.saturating_add(1);
        self.asset_replacement_started_at = Some(self.services.clock.now());
        self.asset_replacement_assets_ready_at = None;
        if self.external_asset_pack_preparation {
            if !self.external_asset_pack_operations.is_empty() {
                let _ = self.external_asset_pack_operations.teardown();
            }
            let operation = self.external_asset_pack_operations.issue(
                ExternalAssetPackSelection {
                    content_generation: epoch,
                    selection,
                },
                (),
            );
            self.pending_external_asset_pack_selection = Some(operation);
            self.asset_replacement_status = AssetReplacementStatus::PreparingAssets { epoch };
            return Ok(());
        }
        let registry = self
            .asset_pack_sources
            .clone()
            .context("asset pack sources are not configured on this platform")?;
        let request = PreparedSceneAssetsRequest::from_registry(epoch, registry, selection)?;
        self.begin_asset_replacement(request)
    }

    pub fn begin_prepared_asset_replacement(&mut self, assets: PreparedSceneAssets) -> Result<()> {
        let epoch = assets.epoch;
        let started_at = self.services.clock.now();
        self.validate_replacement_epoch(epoch)?;
        let (snapshots, target_sections) = self.current_asset_compile_inputs()?;
        #[cfg(not(target_arch = "wasm32"))]
        let request = PreparedAssetReplacementRequest::spawn(
            assets,
            snapshots,
            target_sections,
            self.current_biome_zoom_seed(),
        )?;
        #[cfg(target_arch = "wasm32")]
        let request = PreparedAssetReplacementRequest::ready(PreparedAssetReplacement::build(
            assets,
            snapshots,
            target_sections,
            self.current_biome_zoom_seed(),
        )?);
        self.asset_replacement = Some(SceneAssetReplacementPending::Meshes(request));
        self.asset_replacement_started_at = Some(started_at);
        self.asset_replacement_assets_ready_at = Some(started_at);
        self.asset_replacement_status = AssetReplacementStatus::PreparingMeshes { epoch };
        Ok(())
    }

    pub(crate) fn request_leaf_detail(&mut self, detail: mclone_mesh::LeafDetail) -> Result<()> {
        if self.mesh_assets.catalog.leaf_detail() == detail {
            return Ok(());
        }
        if self.active_world.runtime.is_none() {
            let mut mesh_assets = self.mesh_assets.clone();
            mesh_assets.catalog = mesh_assets.catalog.with_leaf_detail(detail);
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(startup) = self.active_world.local_startup.as_mut() {
                if startup.pump.poll_count() != 0 {
                    self.pending_restored_leaf_detail = Some(detail);
                    log::info!(
                        "leaf detail {detail:?} queued until the active startup pump completes"
                    );
                    return Ok(());
                }
                startup
                    .pump
                    .replace_mesh_assets_before_start(mesh_assets.clone())?;
            }
            self.mesh_assets = mesh_assets.clone();
            self.active_assets.mesh = mesh_assets;
            self.pending_restored_leaf_detail = None;
            self.persist_graphics_preference(detail);
            log::info!("leaf detail set to {detail:?} before active runtime startup");
            return Ok(());
        }
        let epoch = self.active_asset_epoch().saturating_add(1);
        let mut assets = self.active_asset_snapshot_for_epoch(epoch);
        assets.mesh.catalog = assets.mesh.catalog.with_leaf_detail(detail);
        self.begin_prepared_asset_replacement(assets)?;
        log::info!("leaf detail replacement epoch {epoch} requested as {detail:?}");
        Ok(())
    }

    fn persist_graphics_preference(&mut self, detail: mclone_mesh::LeafDetail) {
        let preferences = ClientGraphicsPreferences {
            leaf_detail: game_leaf_detail(detail),
        };
        if let Some(storage) = self.graphics_preference_storage.as_ref() {
            if let Err(error) = storage.store(&preferences) {
                self.graphics_preference_error =
                    Some(format!("failed to store {}: {error:#}", storage.label()));
            } else {
                self.graphics_preference_error = None;
            }
        }
    }

    /// Snapshot the current CPU resources for deterministic rollback/switchback
    /// diagnostics. Production Apply flows normally re-prepare from discovery.
    pub fn active_asset_snapshot_for_epoch(&self, epoch: u64) -> PreparedSceneAssets {
        let mut assets = self.active_assets.clone();
        assets.epoch = epoch;
        assets.provenance.epoch = epoch;
        assets
    }

    pub fn take_last_asset_replacement_commit(&mut self) -> Option<AssetReplacementCommitReport> {
        self.last_asset_replacement_commit.take()
    }

    pub(crate) fn poll_asset_replacement(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        if self.asset_replacement.is_none()
            && self.active_world.runtime.is_some()
            && let Some(detail) = self.pending_restored_leaf_detail.take()
            && let Err(error) = self.request_leaf_detail(detail)
        {
            self.graphics_preference_error =
                Some(format!("restore preferred leaf detail: {error:#}"));
            self.pending_restored_leaf_detail = Some(detail);
        }
        if self.asset_replacement.is_none()
            && self.active_world.runtime.is_some()
            && let Some(selection) = self.pending_restored_asset_pack_selection.take()
        {
            if let Err(error) = self.request_asset_pack_selection(selection) {
                self.fail_asset_replacement(format!("restore preferred asset packs: {error:#}"));
            }
        }
        let Some(pending) = self.asset_replacement.take() else {
            return Ok(());
        };
        match pending {
            SceneAssetReplacementPending::Assets(mut request) => match request.poll() {
                AssetPreparePoll::Pending => {
                    self.asset_replacement = Some(SceneAssetReplacementPending::Assets(request));
                }
                AssetPreparePoll::Ready(mut assets) => {
                    if assets.epoch != request.epoch() {
                        self.fail_asset_replacement(format!(
                            "asset worker returned epoch {} for request {}",
                            assets.epoch,
                            request.epoch()
                        ));
                        return Ok(());
                    }
                    assets.mesh.catalog = assets
                        .mesh
                        .catalog
                        .with_leaf_detail(self.mesh_assets.catalog.leaf_detail());
                    let epoch = assets.epoch;
                    self.asset_replacement_assets_ready_at = Some(self.services.clock.now());
                    let (snapshots, target_sections) = self.current_asset_compile_inputs()?;
                    let request = match PreparedAssetReplacementRequest::spawn(
                        assets,
                        snapshots,
                        target_sections,
                        self.current_biome_zoom_seed(),
                    ) {
                        Ok(request) => request,
                        Err(error) => {
                            self.fail_asset_replacement(format!("{error:#}"));
                            return Ok(());
                        }
                    };
                    self.asset_replacement = Some(SceneAssetReplacementPending::Meshes(request));
                    self.asset_replacement_status =
                        AssetReplacementStatus::PreparingMeshes { epoch };
                    self.client_experience
                        .asset_packs_mut()
                        .mark_preparing_meshes();
                }
                AssetPreparePoll::Failed(message) => self.fail_asset_replacement(message),
            },
            SceneAssetReplacementPending::Meshes(mut request) => match request.poll() {
                AssetPreparePoll::Pending => {
                    self.asset_replacement = Some(SceneAssetReplacementPending::Meshes(request));
                }
                AssetPreparePoll::Ready(replacement) => {
                    let (current, current_targets) = self.current_asset_compile_inputs()?;
                    if current != replacement.source_snapshots
                        || current_targets != replacement.target_sections
                    {
                        let epoch = replacement.assets.epoch;
                        let request = match PreparedAssetReplacementRequest::spawn(
                            replacement.assets,
                            current,
                            current_targets,
                            self.current_biome_zoom_seed(),
                        ) {
                            Ok(request) => request,
                            Err(error) => {
                                self.fail_asset_replacement(format!("{error:#}"));
                                return Ok(());
                            }
                        };
                        self.asset_replacement =
                            Some(SceneAssetReplacementPending::Meshes(request));
                        self.asset_replacement_status =
                            AssetReplacementStatus::PreparingMeshes { epoch };
                    } else if let Err(error) =
                        self.commit_asset_replacement(device, queue, replacement)
                    {
                        self.fail_asset_replacement(format!("{error:#}"));
                    }
                }
                AssetPreparePoll::Failed(message) => self.fail_asset_replacement(message),
            },
        }
        Ok(())
    }

    fn validate_replacement_epoch(&self, epoch: u64) -> Result<()> {
        if self.asset_replacement.is_some() {
            bail!("an asset replacement is already in progress");
        }
        if self.active_world.runtime.is_none() {
            bail!("asset replacement requires an active runtime");
        }
        if epoch <= self.active_assets.epoch {
            bail!(
                "replacement asset epoch {epoch} must be newer than active epoch {}",
                self.active_assets.epoch
            );
        }
        Ok(())
    }

    fn current_asset_compile_inputs(
        &self,
    ) -> Result<(
        Vec<mclone_core::ChunkSnapshot>,
        std::collections::BTreeSet<RenderSectionKey>,
    )> {
        let runtime = self
            .active_world
            .runtime
            .as_ref()
            .context("asset replacement requires an active runtime")?;
        let target_sections = runtime
            .resident_section_metadata()
            .into_iter()
            .map(|metadata| metadata.key)
            .collect::<std::collections::BTreeSet<_>>();
        let mut needed_chunks = std::collections::BTreeSet::new();
        for key in &target_sections {
            let chunk = mclone_render_session::render_section_chunk_pos(*key);
            needed_chunks.extend([
                chunk,
                ChunkPos::new(chunk.x - 1, chunk.z),
                ChunkPos::new(chunk.x + 1, chunk.z),
                ChunkPos::new(chunk.x, chunk.z - 1),
                ChunkPos::new(chunk.x, chunk.z + 1),
            ]);
        }
        let mut snapshots = runtime
            .client()
            .chunk_snapshots()
            .filter(|snapshot| target_sections.is_empty() || needed_chunks.contains(&snapshot.pos))
            .cloned()
            .collect::<Vec<_>>();
        snapshots.sort_by_key(|snapshot| snapshot.pos);
        Ok((snapshots, target_sections))
    }

    fn current_biome_zoom_seed(&self) -> Option<i64> {
        self.active_world
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.client().biome_zoom_seed())
    }

    fn fail_asset_replacement(&mut self, message: String) {
        self.asset_replacement = None;
        self.asset_replacement_started_at = None;
        self.asset_replacement_assets_ready_at = None;
        self.asset_replacement_status = AssetReplacementStatus::Failed {
            active_epoch: self.active_assets.epoch,
            message,
        };
        if let AssetReplacementStatus::Failed { message, .. } = &self.asset_replacement_status {
            self.client_experience
                .asset_packs_mut()
                .mark_failed(message.clone());
        }
    }

    fn commit_asset_replacement(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        replacement: PreparedAssetReplacement,
    ) -> Result<()> {
        let commit_started_at = self.services.clock.now();
        let replacement_started_at = self
            .asset_replacement_started_at
            .unwrap_or(commit_started_at);
        let assets_ready_at = self
            .asset_replacement_assets_ready_at
            .unwrap_or(replacement_started_at);
        let PreparedAssetReplacement {
            assets,
            source_snapshots: _,
            target_sections: _,
            sections,
        } = replacement;
        // A retained standby is bound to the active asset epoch. Tear it down
        // before committing the new epoch rather than allowing two incompatible
        // catalogs or atlases to coexist behind a later swap.
        self.cancel_warm_world_standby("asset replacement");
        let active_asset_bytes = estimated_prepared_asset_bytes(&self.active_assets);
        let candidate_asset_bytes = estimated_prepared_asset_bytes(&assets);
        let active_section_bytes = self.active_world.runtime.as_ref().map_or(0, |runtime| {
            runtime
                .resident_section_metadata()
                .into_iter()
                .map(|metadata| estimated_section_gpu_bytes(&metadata.stats()))
                .sum()
        });
        let candidate_section_bytes = sections
            .sections
            .iter()
            .map(mclone_mesh::TexturedRenderSectionMesh::estimated_owned_bytes)
            .sum::<usize>();
        let atlas_upload_copies = 2 + usize::from(self.mono_gui.is_some());
        let active_gpu_bytes = active_asset_bytes
            .saturating_add(active_section_bytes)
            .saturating_add(
                self.active_assets
                    .mesh
                    .atlas
                    .byte_len()
                    .saturating_mul(atlas_upload_copies.saturating_sub(1)),
            );
        let candidate_gpu_bytes = candidate_asset_bytes
            .saturating_add(candidate_section_bytes)
            .saturating_add(
                assets
                    .mesh
                    .atlas
                    .byte_len()
                    .saturating_mul(atlas_upload_copies.saturating_sub(1)),
            );
        let runtime = self
            .active_world
            .runtime
            .as_mut()
            .context("asset replacement requires an active runtime")?;
        let session_before = self.session.state().clone();
        let camera_before = self.active_world.local_participant.camera.snapshot();
        let command_count_before = runtime.core().command_count();
        let update_count_before = runtime.core().update_count();

        // Create every fallible GPU/audio replacement before mutating active
        // scene state. Until this block completes, the old set remains drawable.
        let draw = TexturedSectionDrawResources::new(
            device,
            queue,
            self.color_format,
            &sections.sections,
            assets.mesh.atlas.as_upload(),
        )
        .context("upload replacement terrain resources")?;
        let actors = ActorDrawResources::new(
            device,
            queue,
            self.color_format,
            assets.actors.atlas.as_upload(),
            Some(&assets.actors.figures),
        )
        .context("upload replacement actor resources")?;
        let screen_effects = ScreenEffectsRenderer::new_with_assets(
            device,
            queue,
            self.color_format,
            &assets.screen_effects,
        )
        .context("upload replacement screen-effect resources")?;
        let mut world_gui_renderer = WorldGuiRenderer::new(device, self.color_format);
        world_gui_renderer
            .upload_texture_atlas(device, queue, assets.mesh.atlas.as_upload())
            .context("upload replacement world-GUI atlas")?;
        let world_gui_overlay_renderer = WorldGuiRenderer::new(device, self.color_format);
        let mono_gui = if self.mono_gui.is_some() {
            let mut gui = GuiRenderer::new(device, self.color_format);
            gui.upload_texture_atlas(device, queue, assets.mesh.atlas.as_upload())
                .context("upload replacement mono-GUI atlas")?;
            Some(gui)
        } else {
            None
        };
        let far_lod = FarTerrainLodRenderer::new(device, self.color_format);
        let audio = self
            .services
            .audio
            .replacement(assets.audio.clone())
            .context("prepare replacement audio capability")?;

        // Compiler construction is the final fallible step. Replacing the
        // instance drops old queued/results and resets resident compile state
        // to the already prepared full-view report.
        runtime.replace_asset_epoch(assets.epoch, assets.mesh.clone(), sections.clone())?;
        let commit_finished_at = self.services.clock.now();

        self.mesh_assets = assets.mesh.clone();
        self.active_world.draw = draw;
        self.active_world.actors = Some(actors);
        self.screen_effects = screen_effects;
        self.world_gui_renderer = world_gui_renderer;
        self.world_gui_overlay_renderer = world_gui_overlay_renderer;
        self.mono_gui = mono_gui;
        self.active_world.far_lod = far_lod;
        self.services.audio = audio;
        self.active_assets = assets;
        self.active_world.asset_epoch = self.active_assets.epoch;
        self.active_world.traversal_ready_sections.clear();
        self.active_world.section_uploads.clear();
        self.prefetched_live_upload = None;

        let report = AssetReplacementCommitReport {
            epoch: self.active_assets.epoch,
            section_count: sections.sections.len(),
            session_preserved: self.session.state() == &session_before,
            camera_preserved: self.active_world.local_participant.camera.snapshot()
                == camera_before,
            command_count_unchanged: runtime.core().command_count() == command_count_before,
            update_count_unchanged: runtime.core().update_count() == update_count_before,
            preparation_ms: assets_ready_at
                .saturating_duration_since(replacement_started_at)
                .as_secs_f64()
                * 1_000.0,
            compile_ms: commit_started_at
                .saturating_duration_since(assets_ready_at)
                .as_secs_f64()
                * 1_000.0,
            upload_ms: commit_finished_at
                .saturating_duration_since(commit_started_at)
                .as_secs_f64()
                * 1_000.0,
            total_ms: commit_finished_at
                .saturating_duration_since(replacement_started_at)
                .as_secs_f64()
                * 1_000.0,
            peak_retained_cpu_bytes: active_asset_bytes
                .saturating_add(candidate_asset_bytes)
                .saturating_add(candidate_section_bytes),
            peak_retained_gpu_bytes: active_gpu_bytes.saturating_add(candidate_gpu_bytes),
        };
        self.asset_replacement_status = AssetReplacementStatus::Active {
            epoch: self.active_assets.epoch,
        };
        self.client_experience.asset_packs_mut().mark_active(
            self.active_assets.selection.clone(),
            &self.active_assets.provenance,
            self.active_assets.coverage,
        );
        let preference = self.asset_pack_preference.after_successful_apply(
            self.client_experience.asset_packs().catalog(),
            &self.active_assets.selection,
        );
        if let Some(storage) = self.asset_pack_preference_storage.as_ref() {
            if let Err(error) = storage.store(&preference) {
                self.asset_pack_preference_error =
                    Some(format!("failed to store {}: {error:#}", storage.label()));
            } else {
                self.asset_pack_preference_error = None;
            }
        }
        self.asset_pack_preference = preference;
        self.persist_graphics_preference(self.active_assets.mesh.catalog.leaf_detail());
        self.pending_restored_leaf_detail = None;
        self.asset_replacement_started_at = None;
        self.asset_replacement_assets_ready_at = None;
        self.last_asset_replacement_commit = Some(report);
        Ok(())
    }
}

fn estimated_prepared_asset_bytes(assets: &PreparedSceneAssets) -> usize {
    assets
        .mesh
        .atlas
        .byte_len()
        .saturating_add(assets.actors.atlas.byte_len())
        .saturating_add(assets.screen_effects.underwater_rgba.len())
}

fn estimated_section_gpu_bytes(stats: &mclone_mesh::SectionMeshStats) -> usize {
    (stats.vertex_count as usize)
        .saturating_mul(std::mem::size_of::<mclone_mesh::TexturedChunkVertex>())
        .saturating_add((stats.index_count as usize).saturating_mul(std::mem::size_of::<u32>()))
}
