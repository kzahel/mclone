use anyhow::{Context, Result, bail};
use mclone_assets::{
    AssetPackCatalog, AssetPackDescriptor, AssetPackDiscovery, AssetPackId, AssetPackOrigin,
    AssetPackSelection, AssetProvenanceReport, AssetResolutionOutcome, AssetSource,
    AssetSourceChain, FIRST_PARTY_AUDIO_POLICY_PATH, FirstPartyAudioPolicy, MissingAssetRegistry,
    PackedAssetSource, ProvenanceTrackingAssetSource,
};
use mclone_mesh::{TexturedTerrainAssets, load_first_party_textured_terrain_assets};
use mclone_render::actor_assets::{ActorTextureAssets, load_actor_texture_assets};
use mclone_render::screen_effect::{ScreenEffectTextureAssets, load_screen_effect_texture_assets};

use crate::far_lod::FarTerrainLodMaterialPalette;

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
