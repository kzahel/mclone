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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssetReplacementCommitReport {
    pub epoch: u64,
    pub section_count: usize,
    pub session_preserved: bool,
    pub camera_preserved: bool,
    pub command_count_unchanged: bool,
    pub update_count_unchanged: bool,
}

pub(crate) enum SceneAssetReplacementPending {
    Assets(PreparedSceneAssetsRequest),
    Meshes(PreparedAssetReplacementRequest),
}

impl<S> McloneSceneHost<S>
where
    S: RemoteDedicatedServerSession,
{
    pub fn asset_replacement_status(&self) -> &AssetReplacementStatus {
        &self.asset_replacement_status
    }

    pub fn asset_pack_ui_state(&self) -> mclone_ui::AssetPacksUiState {
        self.client_experience.asset_packs().ui_state()
    }

    pub const fn active_asset_epoch(&self) -> u64 {
        self.active_assets.epoch
    }

    pub fn begin_asset_replacement(&mut self, request: PreparedSceneAssetsRequest) -> Result<()> {
        let epoch = request.epoch();
        self.validate_replacement_epoch(epoch)?;
        self.asset_replacement = Some(SceneAssetReplacementPending::Assets(request));
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
            active_selection,
            &self.active_assets.provenance,
            self.active_assets.coverage,
        )?;
        self.asset_pack_sources = Some(registry);
        Ok(())
    }

    pub(crate) fn apply_asset_pack_effects(&mut self, effects: Vec<ClientAssetPackEffect>) {
        for effect in effects {
            match effect {
                ClientAssetPackEffect::ApplySelection(selection) => {
                    let result = (|| {
                        let registry = self
                            .asset_pack_sources
                            .clone()
                            .context("asset pack sources are not configured on this platform")?;
                        let epoch = self.active_assets.epoch.saturating_add(1);
                        let request =
                            PreparedSceneAssetsRequest::from_registry(epoch, registry, selection)?;
                        self.begin_asset_replacement(request)
                    })();
                    if let Err(error) = result {
                        self.client_experience
                            .asset_packs_mut()
                            .mark_failed(format!("{error:#}"));
                    }
                }
            }
        }
    }

    pub fn begin_prepared_asset_replacement(&mut self, assets: PreparedSceneAssets) -> Result<()> {
        let epoch = assets.epoch;
        self.validate_replacement_epoch(epoch)?;
        let (snapshots, target_sections) = self.current_asset_compile_inputs()?;
        let request = PreparedAssetReplacementRequest::spawn(
            assets,
            snapshots,
            target_sections,
            self.current_biome_zoom_seed(),
        )?;
        self.asset_replacement = Some(SceneAssetReplacementPending::Meshes(request));
        self.asset_replacement_status = AssetReplacementStatus::PreparingMeshes { epoch };
        Ok(())
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
        let Some(pending) = self.asset_replacement.take() else {
            return Ok(());
        };
        match pending {
            SceneAssetReplacementPending::Assets(mut request) => match request.poll() {
                AssetPreparePoll::Pending => {
                    self.asset_replacement = Some(SceneAssetReplacementPending::Assets(request));
                }
                AssetPreparePoll::Ready(assets) => {
                    if assets.epoch != request.epoch() {
                        self.fail_asset_replacement(format!(
                            "asset worker returned epoch {} for request {}",
                            assets.epoch,
                            request.epoch()
                        ));
                        return Ok(());
                    }
                    let epoch = assets.epoch;
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
        if self.runtime.is_none() {
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
        self.runtime
            .as_ref()
            .and_then(|runtime| runtime.client().biome_zoom_seed())
    }

    fn fail_asset_replacement(&mut self, message: String) {
        self.asset_replacement = None;
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
        let PreparedAssetReplacement {
            assets,
            source_snapshots: _,
            target_sections: _,
            sections,
        } = replacement;
        let runtime = self
            .runtime
            .as_mut()
            .context("asset replacement requires an active runtime")?;
        let session_before = self.session.state().clone();
        let camera_before = self.camera.snapshot();
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
        let audio = if self.audio.is_some() {
            Some(
                AudioEngine::from_prepared(
                    assets.audio.clone(),
                    mclone_audio::AudioSettings::default(),
                )
                .context("initialize replacement audio resources")?,
            )
        } else {
            None
        };

        // Compiler construction is the final fallible step. Replacing the
        // instance drops old queued/results and resets resident compile state
        // to the already prepared full-view report.
        runtime.replace_asset_epoch(assets.epoch, assets.mesh.clone(), sections.clone())?;

        self.mesh_assets = assets.mesh.clone();
        self.draw = draw;
        self.actors = actors;
        self.screen_effects = screen_effects;
        self.world_gui_renderer = world_gui_renderer;
        self.world_gui_overlay_renderer = world_gui_overlay_renderer;
        self.mono_gui = mono_gui;
        self.far_lod = far_lod;
        self.audio = audio;
        self.active_assets = assets;
        self.traversal_ready_sections.clear();
        self.section_uploads.clear();
        self.prefetched_live_upload = None;

        let report = AssetReplacementCommitReport {
            epoch: self.active_assets.epoch,
            section_count: sections.sections.len(),
            session_preserved: self.session.state() == &session_before,
            camera_preserved: self.camera.snapshot() == camera_before,
            command_count_unchanged: runtime.core().command_count() == command_count_before,
            update_count_unchanged: runtime.core().update_count() == update_count_before,
        };
        self.asset_replacement_status = AssetReplacementStatus::Active {
            epoch: self.active_assets.epoch,
        };
        self.client_experience.asset_packs_mut().mark_active(
            self.active_assets.selection.clone(),
            &self.active_assets.provenance,
            self.active_assets.coverage,
        );
        self.last_asset_replacement_commit = Some(report);
        Ok(())
    }
}
