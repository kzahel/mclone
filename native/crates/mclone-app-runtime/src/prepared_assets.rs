use anyhow::{Context, Result, bail};
pub use mclone_assets::{
    AUTHORED_FIRST_PARTY_PACK_ID, DIAGNOSTIC_MISSING_PACK_ID, MINECRAFT_REFERENCE_PACK_ID,
};
use mclone_assets::{
    AssetPackCatalog, AssetPackDescriptor, AssetPackDiscovery, AssetPackId, AssetPackOrigin,
    AssetPackSelection, AssetProvenanceReport, AssetResolutionOutcome, AssetSource,
    AssetSourceChain, FIRST_PARTY_AUDIO_POLICY_PATH, FirstPartyAudioPolicy, MissingAssetRegistry,
    PROVISIONAL_FIRST_PARTY_PACK_ID, PackedAssetSource, ProvenanceTrackingAssetSource,
    TexturePresentation, TextureVisualProfile,
};
use mclone_assets::{AssetProvenanceEntry, AssetResolutionOrigin, SharedAssetSource};
use mclone_audio::PreparedAudioAssets;
use mclone_mesh::{
    TexturedTerrainAssets, load_first_party_textured_terrain_assets,
    load_first_party_textured_terrain_assets_with_presentation,
    load_textured_terrain_assets_with_presentation,
};
use mclone_render::actor_assets::{ActorTextureAssets, load_actor_texture_assets};
use mclone_render::screen_effect::{ScreenEffectTextureAssets, load_screen_effect_texture_assets};

use crate::far_lod::FarTerrainLodMaterialPalette;

use std::collections::BTreeMap;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;

use mclone_core::ChunkSnapshot;
use mclone_mesh::RenderSectionKey;
use mclone_mesh::TexturedRenderSectionBuildReport;
use mclone_render_session::{
    build_render_sections_from_snapshots_with_biome_zoom_seed, render_section_keys_for_snapshot,
};

use crate::render_asset_data::TexturedMeshAssets;

pub const GENERATED_FALLBACK_PACK_ID: &str = PROVISIONAL_FIRST_PARTY_PACK_ID;

pub fn reference_asset_pack_selection() -> AssetPackSelection {
    TextureVisualProfile::MinecraftReference.selection()
}

pub fn original_asset_pack_selection() -> AssetPackSelection {
    TextureVisualProfile::McloneOriginal.selection()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedAssetCoverage {
    pub block_states: usize,
    pub atlas_sprites: usize,
    pub actor_figures: usize,
    pub far_lod_colors: usize,
    pub missing_registry_entries: usize,
    pub first_party_resolutions: usize,
    pub generated_resolutions: usize,
    pub suppressed_audio: usize,
    pub missing_optional: usize,
}

/// Complete CPU-side asset bundle prepared for a later transactional swap.
///
/// Slice 2 intentionally stops before GPU upload or live-scene replacement.
pub struct PreparedAssetSet {
    pub epoch: u64,
    pub selection: AssetPackSelection,
    pub presentation: TexturePresentation,
    pub source: AssetSourceChain,
    pub terrain: TexturedTerrainAssets,
    pub far_lod_materials: Option<FarTerrainLodMaterialPalette>,
    pub actors: ActorTextureAssets,
    pub screen_effects: ScreenEffectTextureAssets,
    pub audio_policy: FirstPartyAudioPolicy,
    pub missing_registry: Option<MissingAssetRegistry>,
    pub provenance: AssetProvenanceReport,
    pub coverage: PreparedAssetCoverage,
}

#[derive(Clone)]
pub struct PreparedSceneAssets {
    pub epoch: u64,
    pub selection: AssetPackSelection,
    pub presentation: TexturePresentation,
    pub mesh: TexturedMeshAssets,
    pub actors: ActorTextureAssets,
    pub screen_effects: ScreenEffectTextureAssets,
    pub audio: PreparedAudioAssets,
    pub audio_policy: FirstPartyAudioPolicy,
    pub provenance: AssetProvenanceReport,
    pub coverage: Option<PreparedAssetCoverage>,
}

impl PreparedSceneAssets {
    pub fn startup(
        epoch: u64,
        mesh: TexturedMeshAssets,
        actors: ActorTextureAssets,
        screen_effects: ScreenEffectTextureAssets,
        audio: PreparedAudioAssets,
    ) -> Self {
        let selection = reference_asset_pack_selection();
        let mut provenance = AssetProvenanceReport::new(epoch, selection.clone());
        provenance.record(AssetProvenanceEntry {
            path: mclone_assets::AssetPath::new("assets/mclone/runtime-startup-source"),
            source: AssetResolutionOrigin::named(
                AssetPackId::new(MINECRAFT_REFERENCE_PACK_ID),
                AssetPackOrigin::MinecraftReference,
            ),
            outcome: AssetResolutionOutcome::Resolved,
        });
        Self {
            epoch,
            selection: selection.clone(),
            presentation: TexturePresentation::Textured,
            mesh,
            actors,
            screen_effects,
            audio,
            audio_policy: FirstPartyAudioPolicy {
                suppressed: Default::default(),
            },
            provenance,
            coverage: None,
        }
    }
}

impl PreparedAssetSet {
    pub fn into_scene_assets(self) -> PreparedSceneAssets {
        PreparedSceneAssets {
            epoch: self.epoch,
            selection: self.selection,
            presentation: self.presentation,
            mesh: TexturedMeshAssets {
                catalog: self.terrain.catalog,
                atlas: self.terrain.atlas.into(),
                far_lod_materials: self.far_lod_materials,
            },
            actors: self.actors,
            screen_effects: self.screen_effects,
            audio: PreparedAudioAssets::silent(),
            audio_policy: self.audio_policy,
            provenance: self.provenance,
            coverage: Some(self.coverage),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AssetPackSourceRegistry {
    catalog: AssetPackCatalog,
    sources: BTreeMap<AssetPackId, SharedAssetSource>,
}

impl AssetPackSourceRegistry {
    pub fn new(
        catalog: AssetPackCatalog,
        sources: impl IntoIterator<Item = (AssetPackId, SharedAssetSource)>,
    ) -> Result<Self> {
        let sources = sources.into_iter().collect::<BTreeMap<_, _>>();
        for descriptor in catalog.descriptors() {
            if descriptor.availability.is_available() && !sources.contains_key(&descriptor.id) {
                bail!(
                    "available asset pack {} has no readable source",
                    descriptor.id
                );
            }
        }
        Ok(Self { catalog, sources })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_files_with_reference(
        authored_path: impl AsRef<Path>,
        generated_fallback_path: impl AsRef<Path>,
        diagnostic_path: Option<impl AsRef<Path>>,
        reference: SharedAssetSource,
    ) -> Result<Self> {
        let authored = PackedAssetSource::from_file(authored_path.as_ref()).with_context(|| {
            format!(
                "failed to open authored asset pack {}",
                authored_path.as_ref().display()
            )
        })?;
        let generated = PackedAssetSource::from_file(generated_fallback_path.as_ref())
            .with_context(|| {
                format!(
                    "failed to open generated fallback pack {}",
                    generated_fallback_path.as_ref().display()
                )
            })?;
        let diagnostic = diagnostic_path
            .map(|path| {
                PackedAssetSource::from_file(path.as_ref()).with_context(|| {
                    format!(
                        "failed to open diagnostic missing pack {}",
                        path.as_ref().display()
                    )
                })
            })
            .transpose()?;
        Self::from_packed_with_reference(Some(authored), generated, diagnostic, reference)
    }

    pub fn from_packed_with_reference(
        authored: Option<PackedAssetSource>,
        provisional: PackedAssetSource,
        diagnostic: Option<PackedAssetSource>,
        reference: SharedAssetSource,
    ) -> Result<Self> {
        let authored_descriptor = match authored.as_ref() {
            Some(authored) => first_party_descriptor(
                authored,
                AUTHORED_FIRST_PARTY_PACK_ID,
                AssetPackOrigin::FirstParty,
                10,
            )?,
            None => AssetPackDescriptor::new(
                AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID),
                "Mclone Original Assets",
                AssetPackOrigin::FirstParty,
                10,
            )?
            .unavailable("Not installed by this platform"),
        };
        let reference_descriptor = AssetPackDescriptor::new(
            AssetPackId::new(MINECRAFT_REFERENCE_PACK_ID),
            "Minecraft 1.17.1 Reference",
            AssetPackOrigin::MinecraftReference,
            20,
        )?;
        let provisional_descriptor = first_party_descriptor(
            &provisional,
            GENERATED_FALLBACK_PACK_ID,
            AssetPackOrigin::FirstPartyProvisional,
            30,
        )?;
        let diagnostic_descriptor = match diagnostic.as_ref() {
            Some(diagnostic) => first_party_descriptor(
                diagnostic,
                DIAGNOSTIC_MISSING_PACK_ID,
                AssetPackOrigin::Diagnostic,
                40,
            )?,
            None => AssetPackDescriptor::new(
                AssetPackId::new(DIAGNOSTIC_MISSING_PACK_ID),
                "Numbered Missing Diagnostics",
                AssetPackOrigin::Diagnostic,
                40,
            )?
            .unavailable("Diagnostic textures are not installed by this platform"),
        };
        let authored_id = authored_descriptor.id.clone();
        let reference_id = reference_descriptor.id.clone();
        let provisional_id = provisional_descriptor.id.clone();
        let diagnostic_id = diagnostic_descriptor.id.clone();
        let mut sources = vec![
            (reference_id, reference),
            (provisional_id, SharedAssetSource::new(provisional)),
        ];
        if let Some(authored) = authored {
            sources.push((authored_id, SharedAssetSource::new(authored)));
        }
        if let Some(diagnostic) = diagnostic {
            sources.push((diagnostic_id, SharedAssetSource::new(diagnostic)));
        }
        Self::new(
            AssetPackCatalog::new([
                authored_descriptor,
                reference_descriptor,
                provisional_descriptor,
                diagnostic_descriptor,
            ])?,
            sources,
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn discover_native_with_reference(reference: SharedAssetSource) -> Result<Option<Self>> {
        let authored = discover_native_first_party_pack(
            "MCLONE_ASSET_AUTHORED_PACK",
            crate::render_assets::DEFAULT_AUTHORED_FIRST_PARTY_PACK_FILE,
            false,
        )?;
        let Some(generated) = discover_native_first_party_pack(
            "MCLONE_ASSET_FALLBACK_PACK",
            crate::render_assets::DEFAULT_GENERATED_FALLBACK_PACK_FILE,
            true,
        )?
        else {
            return Ok(None);
        };
        let diagnostic = discover_native_first_party_pack(
            "MCLONE_ASSET_DIAGNOSTIC_PACK",
            crate::render_assets::DEFAULT_DIAGNOSTIC_MISSING_PACK_FILE,
            false,
        )?;
        Self::from_packed_with_reference(authored, generated, diagnostic, reference).map(Some)
    }

    pub fn catalog(&self) -> &AssetPackCatalog {
        &self.catalog
    }

    pub fn prepare(
        &self,
        epoch: u64,
        selection: AssetPackSelection,
    ) -> Result<PreparedSceneAssets> {
        self.prepare_with_presentation(epoch, selection, TexturePresentation::Textured)
    }

    pub fn prepare_with_presentation(
        &self,
        epoch: u64,
        selection: AssetPackSelection,
        presentation: TexturePresentation,
    ) -> Result<PreparedSceneAssets> {
        prepare_scene_asset_selection(
            epoch,
            &self.catalog,
            TextureVisualProfile::normalize_legacy_selection(&selection),
            presentation,
            &self.sources,
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn discover_native_first_party_pack(
    environment_name: &str,
    file_name: &str,
    required_for_discovery: bool,
) -> Result<Option<PackedAssetSource>> {
    let explicit = std::env::var_os(environment_name).map(std::path::PathBuf::from);
    let candidates = explicit.clone().map_or_else(
        || crate::render_assets::first_party_pack_candidates(file_name),
        |path| vec![path],
    );
    for path in &candidates {
        if !path.is_file() {
            continue;
        }
        let pack = PackedAssetSource::from_file(path)
            .with_context(|| format!("failed to open asset pack {}", path.display()))?;
        log::info!(
            "discovered logical asset pack {} at {}",
            pack.manifest()
                .pack_id
                .as_ref()
                .map_or(file_name, AssetPackId::as_str),
            path.display()
        );
        return Ok(Some(pack));
    }
    if explicit.is_some() {
        bail!(
            "{environment_name} does not name a readable file: {}",
            candidates
                .first()
                .map_or_else(|| "<empty>".to_owned(), |path| path.display().to_string())
        );
    }
    if required_for_discovery {
        log::info!("provisional texture pack is not installed; visual profiles are unavailable");
    }
    Ok(None)
}

pub enum AssetPreparePoll<T> {
    Pending,
    Ready(T),
    Failed(String),
}

pub struct PreparedSceneAssetsRequest {
    epoch: u64,
    backend: PreparedSceneAssetsRequestBackend,
}

enum PreparedSceneAssetsRequestBackend {
    #[cfg(not(target_arch = "wasm32"))]
    Native(mpsc::Receiver<Result<PreparedSceneAssets>>),
    Ready(Option<Result<PreparedSceneAssets>>),
}

impl PreparedSceneAssetsRequest {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn(
        epoch: u64,
        prepare: impl FnOnce() -> Result<PreparedSceneAssets> + Send + 'static,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name(format!("mclone-asset-prepare-{epoch}"))
            .spawn(move || {
                let _ = sender.send(prepare());
            })
            .context("failed to spawn asset preparation worker")?;
        Ok(Self {
            epoch,
            backend: PreparedSceneAssetsRequestBackend::Native(receiver),
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn first_party_from_files(
        epoch: u64,
        authored_path: impl Into<std::path::PathBuf>,
        fallback_path: impl Into<std::path::PathBuf>,
    ) -> Result<Self> {
        let authored_path = authored_path.into();
        let fallback_path = fallback_path.into();
        Self::spawn(epoch, move || {
            prepare_first_party_asset_set_from_files(epoch, authored_path, fallback_path)
                .map(PreparedAssetSet::into_scene_assets)
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_registry(
        epoch: u64,
        registry: AssetPackSourceRegistry,
        selection: AssetPackSelection,
        presentation: TexturePresentation,
    ) -> Result<Self> {
        Self::spawn(epoch, move || {
            registry.prepare_with_presentation(epoch, selection, presentation)
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn from_registry(
        _epoch: u64,
        _registry: AssetPackSourceRegistry,
        _selection: AssetPackSelection,
        _presentation: TexturePresentation,
    ) -> Result<Self> {
        bail!("browser asset preparation must complete through the injected worker/promise adapter")
    }

    /// Wrap assets prepared by a platform-owned worker or promise. The request
    /// envelope stays target-neutral; only the adapter owns the asynchronous
    /// resource that produced the bundle.
    pub fn ready(assets: PreparedSceneAssets) -> Self {
        Self {
            epoch: assets.epoch,
            backend: PreparedSceneAssetsRequestBackend::Ready(Some(Ok(assets))),
        }
    }

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn poll(&mut self) -> AssetPreparePoll<PreparedSceneAssets> {
        match &mut self.backend {
            #[cfg(not(target_arch = "wasm32"))]
            PreparedSceneAssetsRequestBackend::Native(receiver) => match receiver.try_recv() {
                Ok(Ok(assets)) => AssetPreparePoll::Ready(assets),
                Ok(Err(error)) => AssetPreparePoll::Failed(format!("{error:#}")),
                Err(mpsc::TryRecvError::Empty) => AssetPreparePoll::Pending,
                Err(mpsc::TryRecvError::Disconnected) => {
                    AssetPreparePoll::Failed("asset preparation worker disconnected".to_owned())
                }
            },
            PreparedSceneAssetsRequestBackend::Ready(result) => match result.take() {
                Some(Ok(assets)) => AssetPreparePoll::Ready(assets),
                Some(Err(error)) => AssetPreparePoll::Failed(format!("{error:#}")),
                None => AssetPreparePoll::Pending,
            },
        }
    }
}

fn prepare_scene_asset_selection(
    epoch: u64,
    catalog: &AssetPackCatalog,
    selection: AssetPackSelection,
    presentation: TexturePresentation,
    registered_sources: &BTreeMap<AssetPackId, SharedAssetSource>,
) -> Result<PreparedSceneAssets> {
    let source_order = catalog.source_order(&selection)?;
    let reference_enabled = source_order
        .iter()
        .any(|descriptor| descriptor.origin == AssetPackOrigin::MinecraftReference);
    let source = AssetSourceChain::from_selection(
        catalog,
        &selection,
        registered_sources
            .iter()
            .map(|(id, source)| (id.clone(), Box::new(source.clone()) as Box<dyn AssetSource>)),
    )?;
    let tracker = ProvenanceTrackingAssetSource::new(&source);
    let effective_presentation = if TextureVisualProfile::from_selection(&selection)
        == Some(TextureVisualProfile::FirstPartyCoverage)
    {
        TexturePresentation::Textured
    } else {
        presentation
    };
    let terrain = if reference_enabled {
        load_textured_terrain_assets_with_presentation(&tracker, effective_presentation)
            .context("failed to prepare Minecraft-reference terrain assets")?
    } else {
        load_first_party_textured_terrain_assets_with_presentation(&tracker, effective_presentation)
            .context("failed to prepare first-party terrain assets")?
    };
    let far_lod_materials = Some(FarTerrainLodMaterialPalette::from_textured_terrain_assets(
        &terrain,
    ));
    let actors = load_actor_texture_assets(&tracker)
        .context("failed to prepare selected actor and figure assets")?;
    let screen_effects = load_screen_effect_texture_assets(&tracker)
        .context("failed to prepare selected screen-effect assets")?;

    let (audio, audio_policy, missing_registry) = if reference_enabled {
        (
            PreparedAudioAssets::load(&tracker)
                .context("failed to prepare selected reference audio")?,
            FirstPartyAudioPolicy {
                suppressed: Default::default(),
            },
            None,
        )
    } else {
        let policy = FirstPartyAudioPolicy::load(&tracker)
            .context("failed to prepare first-party audio policy")?;
        let registry = MissingAssetRegistry::load_optional(&tracker)
            .context("failed to prepare optional diagnostic missing-resource registry")?;
        let policy_origin = tracker
            .resolved_origin(&mclone_assets::AssetPath::new(
                FIRST_PARTY_AUDIO_POLICY_PATH,
            ))
            .context("audio policy resolution did not retain a named source")?;
        for path in &policy.suppressed {
            tracker.record_suppressed(path.clone(), policy_origin.clone());
        }
        (PreparedAudioAssets::silent(), policy, registry)
    };

    let provenance = tracker.report(epoch, selection.clone());
    if !reference_enabled && !provenance.allows_proprietary_free_claim() {
        bail!("reference-disabled preparation resolved reference or unknown content");
    }
    for entry in provenance.entries() {
        if entry.outcome == AssetResolutionOutcome::Resolved
            && entry.source.origin == AssetPackOrigin::Diagnostic
            && entry.path.as_str().ends_with(".png")
            && missing_registry
                .as_ref()
                .and_then(|registry| registry.get(&entry.path))
                .is_none()
        {
            bail!(
                "diagnostic resource {} has no missing-resource registry id",
                entry.path.as_str()
            );
        }
    }
    let summary = provenance.summary();
    let coverage = PreparedAssetCoverage {
        block_states: terrain.catalog.len(),
        atlas_sprites: terrain.atlas_sprite_count,
        actor_figures: actors.figures.len(),
        far_lod_colors: far_lod_materials
            .as_ref()
            .map_or(0, FarTerrainLodMaterialPalette::color_count),
        missing_registry_entries: missing_registry
            .as_ref()
            .map_or(0, MissingAssetRegistry::len),
        first_party_resolutions: summary.first_party,
        generated_resolutions: summary.generated + summary.provisional + summary.diagnostic,
        suppressed_audio: summary.suppressed,
        missing_optional: summary.missing,
    };
    drop(tracker);
    Ok(PreparedSceneAssets {
        epoch,
        selection,
        presentation: effective_presentation,
        mesh: TexturedMeshAssets {
            catalog: terrain.catalog,
            atlas: terrain.atlas.into(),
            far_lod_materials,
        },
        actors,
        screen_effects,
        audio,
        audio_policy,
        provenance,
        coverage: Some(coverage),
    })
}

pub struct PreparedAssetReplacement {
    pub assets: PreparedSceneAssets,
    pub source_snapshots: Vec<ChunkSnapshot>,
    pub target_sections: std::collections::BTreeSet<RenderSectionKey>,
    pub sections: TexturedRenderSectionBuildReport,
}

impl PreparedAssetReplacement {
    pub fn build(
        assets: PreparedSceneAssets,
        mut snapshots: Vec<ChunkSnapshot>,
        target_sections: std::collections::BTreeSet<RenderSectionKey>,
        biome_zoom_seed: Option<i64>,
    ) -> Result<Self> {
        snapshots.sort_by_key(|snapshot| snapshot.pos);
        let compile_target_sections = if target_sections.is_empty() {
            snapshots
                .iter()
                .flat_map(render_section_keys_for_snapshot)
                .collect()
        } else {
            target_sections.clone()
        };
        let sections = build_render_sections_from_snapshots_with_biome_zoom_seed(
            &snapshots,
            &assets.mesh.catalog,
            &compile_target_sections,
            biome_zoom_seed,
        )
        .context("failed to compile replacement render sections")?;
        Ok(Self {
            assets,
            source_snapshots: snapshots,
            target_sections,
            sections,
        })
    }
}

pub struct PreparedAssetReplacementRequest {
    epoch: u64,
    backend: PreparedAssetReplacementRequestBackend,
}

enum PreparedAssetReplacementRequestBackend {
    #[cfg(not(target_arch = "wasm32"))]
    Native(mpsc::Receiver<Result<PreparedAssetReplacement>>),
    Ready(Option<Result<PreparedAssetReplacement>>),
}

impl PreparedAssetReplacementRequest {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn(
        assets: PreparedSceneAssets,
        mut snapshots: Vec<ChunkSnapshot>,
        target_sections: std::collections::BTreeSet<RenderSectionKey>,
        biome_zoom_seed: Option<i64>,
    ) -> Result<Self> {
        let epoch = assets.epoch;
        snapshots.sort_by_key(|snapshot| snapshot.pos);
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name(format!("mclone-asset-mesh-prepare-{epoch}"))
            .spawn(move || {
                let result = PreparedAssetReplacement::build(
                    assets,
                    snapshots,
                    target_sections,
                    biome_zoom_seed,
                );
                let _ = sender.send(result);
            })
            .context("failed to spawn replacement mesh preparation worker")?;
        Ok(Self {
            epoch,
            backend: PreparedAssetReplacementRequestBackend::Native(receiver),
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn spawn(
        _assets: PreparedSceneAssets,
        _snapshots: Vec<ChunkSnapshot>,
        _target_sections: std::collections::BTreeSet<RenderSectionKey>,
        _biome_zoom_seed: Option<i64>,
    ) -> Result<Self> {
        bail!(
            "browser replacement meshes must complete through the injected render-compiler adapter"
        )
    }

    /// Wrap replacement meshes produced by a platform-owned compiler worker.
    pub fn ready(replacement: PreparedAssetReplacement) -> Self {
        Self {
            epoch: replacement.assets.epoch,
            backend: PreparedAssetReplacementRequestBackend::Ready(Some(Ok(replacement))),
        }
    }

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn poll(&mut self) -> AssetPreparePoll<PreparedAssetReplacement> {
        match &mut self.backend {
            #[cfg(not(target_arch = "wasm32"))]
            PreparedAssetReplacementRequestBackend::Native(receiver) => match receiver.try_recv() {
                Ok(Ok(replacement)) => AssetPreparePoll::Ready(replacement),
                Ok(Err(error)) => AssetPreparePoll::Failed(format!("{error:#}")),
                Err(mpsc::TryRecvError::Empty) => AssetPreparePoll::Pending,
                Err(mpsc::TryRecvError::Disconnected) => AssetPreparePoll::Failed(
                    "replacement mesh preparation worker disconnected".to_owned(),
                ),
            },
            PreparedAssetReplacementRequestBackend::Ready(result) => match result.take() {
                Some(Ok(replacement)) => AssetPreparePoll::Ready(replacement),
                Some(Err(error)) => AssetPreparePoll::Failed(format!("{error:#}")),
                None => AssetPreparePoll::Pending,
            },
        }
    }
}

pub fn prepare_first_party_asset_set(
    epoch: u64,
    authored: PackedAssetSource,
    generated_fallback: PackedAssetSource,
) -> Result<PreparedAssetSet> {
    let authored_descriptor = first_party_descriptor(
        &authored,
        AUTHORED_FIRST_PARTY_PACK_ID,
        AssetPackOrigin::FirstParty,
        10,
    )?;
    let fallback_descriptor = first_party_descriptor(
        &generated_fallback,
        GENERATED_FALLBACK_PACK_ID,
        AssetPackOrigin::FirstPartyProvisional,
        30,
    )?;
    let authored_id = authored_descriptor.id.clone();
    let fallback_id = fallback_descriptor.id.clone();
    let catalog = AssetPackCatalog::new([authored_descriptor, fallback_descriptor])?;
    let selection = TextureVisualProfile::McloneOriginal.selection();
    let source = AssetSourceChain::from_selection(
        &catalog,
        &selection,
        [
            (authored_id, Box::new(authored) as Box<dyn AssetSource>),
            (
                fallback_id,
                Box::new(generated_fallback) as Box<dyn AssetSource>,
            ),
        ],
    )?;
    let tracker = ProvenanceTrackingAssetSource::new(&source);

    let terrain = load_first_party_textured_terrain_assets(&tracker)
        .context("failed to prepare first-party terrain assets")?;
    let far_lod_materials = Some(FarTerrainLodMaterialPalette::from_textured_terrain_assets(
        &terrain,
    ));
    let actors = load_actor_texture_assets(&tracker)
        .context("failed to prepare first-party actor and figure assets")?;
    let screen_effects = load_screen_effect_texture_assets(&tracker)
        .context("failed to prepare first-party screen-effect assets")?;
    let audio_policy = FirstPartyAudioPolicy::load(&tracker)
        .context("failed to prepare first-party audio policy")?;
    let missing_registry = MissingAssetRegistry::load_optional(&tracker)
        .context("failed to prepare optional diagnostic missing-resource registry")?;

    let audio_policy_origin = tracker
        .resolved_origin(&mclone_assets::AssetPath::new(
            FIRST_PARTY_AUDIO_POLICY_PATH,
        ))
        .context("audio policy resolution did not retain a named source")?;
    for path in &audio_policy.suppressed {
        tracker.record_suppressed(path.clone(), audio_policy_origin.clone());
    }
    let provenance = tracker.report(epoch, selection.clone());
    if !provenance.allows_proprietary_free_claim() {
        bail!("first-party preparation resolved Minecraft-reference or unknown content");
    }
    let summary = provenance.summary();
    let coverage = PreparedAssetCoverage {
        block_states: terrain.catalog.len(),
        atlas_sprites: terrain.atlas_sprite_count,
        actor_figures: actors.figures.len(),
        far_lod_colors: far_lod_materials
            .as_ref()
            .map_or(0, FarTerrainLodMaterialPalette::color_count),
        missing_registry_entries: missing_registry
            .as_ref()
            .map_or(0, MissingAssetRegistry::len),
        first_party_resolutions: summary.first_party,
        generated_resolutions: summary.generated + summary.provisional + summary.diagnostic,
        suppressed_audio: summary.suppressed,
        missing_optional: summary.missing,
    };

    drop(tracker);
    Ok(PreparedAssetSet {
        epoch,
        selection,
        presentation: TexturePresentation::Textured,
        source,
        terrain,
        far_lod_materials,
        actors,
        screen_effects,
        audio_policy,
        missing_registry,
        provenance,
        coverage,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn prepare_first_party_asset_set_from_files(
    epoch: u64,
    authored_path: impl AsRef<std::path::Path>,
    generated_fallback_path: impl AsRef<std::path::Path>,
) -> Result<PreparedAssetSet> {
    let authored_path = authored_path.as_ref();
    let fallback_path = generated_fallback_path.as_ref();
    let authored = PackedAssetSource::from_file(authored_path)
        .with_context(|| format!("failed to open {}", authored_path.display()))?;
    let fallback = PackedAssetSource::from_file(fallback_path)
        .with_context(|| format!("failed to open {}", fallback_path.display()))?;
    prepare_first_party_asset_set(epoch, authored, fallback)
}

fn first_party_descriptor(
    source: &PackedAssetSource,
    expected_id: &str,
    expected_origin: AssetPackOrigin,
    priority: u16,
) -> Result<AssetPackDescriptor> {
    let descriptor = source.manifest().descriptor(
        AssetPackDiscovery::untrusted(AssetPackId::new(expected_id), expected_id),
        priority,
    )?;
    if descriptor.id.as_str() != expected_id
        || descriptor.origin != expected_origin
        || descriptor.asset_schema.as_deref() != Some(mclone_assets::FIRST_PARTY_VISUAL_SCHEMA)
    {
        bail!(
            "pack `{}` does not declare the expected id/origin/schema",
            descriptor.id
        );
    }
    Ok(descriptor)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn background_prepare_surfaces_failure_without_blocking_caller() {
        let mut request =
            PreparedSceneAssetsRequest::spawn(7, || bail!("synthetic prepare failure")).unwrap();
        assert_eq!(request.epoch(), 7);

        for _ in 0..10_000 {
            match request.poll() {
                AssetPreparePoll::Pending => std::thread::yield_now(),
                AssetPreparePoll::Ready(_) => panic!("failed prepare unexpectedly succeeded"),
                AssetPreparePoll::Failed(message) => {
                    assert!(message.contains("synthetic prepare failure"));
                    return;
                }
            }
        }
        panic!("background prepare did not finish");
    }
}
