use anyhow::{Context, Result, bail};
use mclone_assets::{
    AssetPackCatalog, AssetPackDescriptor, AssetPackDiscovery, AssetPackId, AssetPackOrigin,
    AssetPackSelection, AssetProvenanceReport, AssetResolutionOutcome, AssetSource,
    AssetSourceChain, FIRST_PARTY_AUDIO_POLICY_PATH, FirstPartyAudioPolicy, MissingAssetRegistry,
    PackedAssetSource, ProvenanceTrackingAssetSource,
};
#[cfg(not(target_arch = "wasm32"))]
use mclone_assets::{AssetProvenanceEntry, AssetResolutionOrigin};
#[cfg(not(target_arch = "wasm32"))]
use mclone_audio::PreparedAudioAssets;
use mclone_mesh::{TexturedTerrainAssets, load_first_party_textured_terrain_assets};
use mclone_render::actor_assets::{ActorTextureAssets, load_actor_texture_assets};
use mclone_render::screen_effect::{ScreenEffectTextureAssets, load_screen_effect_texture_assets};

use crate::far_lod::FarTerrainLodMaterialPalette;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;

#[cfg(not(target_arch = "wasm32"))]
use mclone_core::ChunkSnapshot;
#[cfg(not(target_arch = "wasm32"))]
use mclone_mesh::RenderSectionKey;
#[cfg(not(target_arch = "wasm32"))]
use mclone_mesh::TexturedRenderSectionBuildReport;
#[cfg(not(target_arch = "wasm32"))]
use mclone_render_session::{
    build_render_sections_from_snapshots_with_biome_zoom_seed, render_section_keys_for_snapshot,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::render_assets::TexturedMeshAssets;

pub const AUTHORED_FIRST_PARTY_PACK_ID: &str = "mclone-authored";
pub const GENERATED_FALLBACK_PACK_ID: &str = "mclone-generated-fallback";

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
    pub source: AssetSourceChain,
    pub terrain: TexturedTerrainAssets,
    pub far_lod_materials: Option<FarTerrainLodMaterialPalette>,
    pub actors: ActorTextureAssets,
    pub screen_effects: ScreenEffectTextureAssets,
    pub audio_policy: FirstPartyAudioPolicy,
    pub missing_registry: MissingAssetRegistry,
    pub provenance: AssetProvenanceReport,
    pub coverage: PreparedAssetCoverage,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub struct PreparedSceneAssets {
    pub epoch: u64,
    pub selection: AssetPackSelection,
    pub mesh: TexturedMeshAssets,
    pub actors: ActorTextureAssets,
    pub screen_effects: ScreenEffectTextureAssets,
    pub audio: PreparedAudioAssets,
    pub audio_policy: FirstPartyAudioPolicy,
    pub provenance: AssetProvenanceReport,
    pub coverage: Option<PreparedAssetCoverage>,
}

#[cfg(not(target_arch = "wasm32"))]
impl PreparedSceneAssets {
    pub fn startup(
        epoch: u64,
        mesh: TexturedMeshAssets,
        actors: ActorTextureAssets,
        screen_effects: ScreenEffectTextureAssets,
        audio: PreparedAudioAssets,
    ) -> Self {
        let selection = AssetPackSelection::default();
        let mut provenance = AssetProvenanceReport::new(epoch, selection.clone());
        provenance.record(AssetProvenanceEntry {
            path: mclone_assets::AssetPath::new("assets/mclone/runtime-startup-source"),
            source: AssetResolutionOrigin::anonymous(),
            outcome: AssetResolutionOutcome::Resolved,
        });
        Self {
            epoch,
            selection: selection.clone(),
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

#[cfg(not(target_arch = "wasm32"))]
impl PreparedAssetSet {
    pub fn into_scene_assets(self) -> PreparedSceneAssets {
        PreparedSceneAssets {
            epoch: self.epoch,
            selection: self.selection,
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

#[cfg(not(target_arch = "wasm32"))]
pub enum AssetPreparePoll<T> {
    Pending,
    Ready(T),
    Failed(String),
}

#[cfg(not(target_arch = "wasm32"))]
pub struct PreparedSceneAssetsRequest {
    epoch: u64,
    receiver: mpsc::Receiver<Result<PreparedSceneAssets>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl PreparedSceneAssetsRequest {
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
        Ok(Self { epoch, receiver })
    }

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

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn poll(&mut self) -> AssetPreparePoll<PreparedSceneAssets> {
        match self.receiver.try_recv() {
            Ok(Ok(assets)) => AssetPreparePoll::Ready(assets),
            Ok(Err(error)) => AssetPreparePoll::Failed(format!("{error:#}")),
            Err(mpsc::TryRecvError::Empty) => AssetPreparePoll::Pending,
            Err(mpsc::TryRecvError::Disconnected) => {
                AssetPreparePoll::Failed("asset preparation worker disconnected".to_owned())
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct PreparedAssetReplacement {
    pub assets: PreparedSceneAssets,
    pub source_snapshots: Vec<ChunkSnapshot>,
    pub target_sections: std::collections::BTreeSet<RenderSectionKey>,
    pub sections: TexturedRenderSectionBuildReport,
}

#[cfg(not(target_arch = "wasm32"))]
pub struct PreparedAssetReplacementRequest {
    epoch: u64,
    receiver: mpsc::Receiver<Result<PreparedAssetReplacement>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl PreparedAssetReplacementRequest {
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
                let result = (|| {
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
                    Ok(PreparedAssetReplacement {
                        assets,
                        source_snapshots: snapshots,
                        target_sections,
                        sections,
                    })
                })();
                let _ = sender.send(result);
            })
            .context("failed to spawn replacement mesh preparation worker")?;
        Ok(Self { epoch, receiver })
    }

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn poll(&mut self) -> AssetPreparePoll<PreparedAssetReplacement> {
        match self.receiver.try_recv() {
            Ok(Ok(replacement)) => AssetPreparePoll::Ready(replacement),
            Ok(Err(error)) => AssetPreparePoll::Failed(format!("{error:#}")),
            Err(mpsc::TryRecvError::Empty) => AssetPreparePoll::Pending,
            Err(mpsc::TryRecvError::Disconnected) => AssetPreparePoll::Failed(
                "replacement mesh preparation worker disconnected".to_owned(),
            ),
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
        AssetPackOrigin::Generated,
        30,
    )?
    .required();
    let authored_id = authored_descriptor.id.clone();
    let fallback_id = fallback_descriptor.id.clone();
    let catalog = AssetPackCatalog::new([authored_descriptor, fallback_descriptor])?;
    let selection = AssetPackSelection::new([authored_id.clone()]);
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
    let far_lod_materials = FarTerrainLodMaterialPalette::load_from_asset_source(&tracker)
        .context("failed to prepare first-party Far LOD metadata")?;
    let actors = load_actor_texture_assets(&tracker)
        .context("failed to prepare first-party actor and figure assets")?;
    let screen_effects = load_screen_effect_texture_assets(&tracker)
        .context("failed to prepare first-party screen-effect assets")?;
    let audio_policy = FirstPartyAudioPolicy::load(&tracker)
        .context("failed to prepare first-party audio policy")?;
    let missing_registry = MissingAssetRegistry::load(&tracker)
        .context("failed to prepare generated missing-resource registry")?;

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
    for entry in provenance.entries() {
        if entry.outcome == AssetResolutionOutcome::Resolved
            && entry.source.origin == AssetPackOrigin::Generated
            && entry.path.as_str().ends_with(".png")
            && missing_registry.get(&entry.path).is_none()
        {
            bail!(
                "generated fallback resource {} has no missing-resource registry id",
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
        missing_registry_entries: missing_registry.len(),
        first_party_resolutions: summary.first_party,
        generated_resolutions: summary.generated,
        suppressed_audio: summary.suppressed,
        missing_optional: summary.missing,
    };

    drop(tracker);
    Ok(PreparedAssetSet {
        epoch,
        selection,
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
