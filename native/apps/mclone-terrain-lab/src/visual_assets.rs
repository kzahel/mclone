use mclone_assets::{
    AUTHORED_FIRST_PARTY_PACK_ID, AssetPackCatalog, AssetPackDescriptor, AssetPackId,
    AssetPackOrigin, AssetSource, AssetSourceChain, DIAGNOSTIC_MISSING_PACK_ID,
    MINECRAFT_REFERENCE_PACK_ID, PROVISIONAL_FIRST_PARTY_PACK_ID, PackedAssetSource,
    TexturePresentation, TextureVisualProfile,
};
use mclone_mesh::{
    TexturedTerrainAssets, load_first_party_textured_terrain_assets_with_presentation,
    load_textured_terrain_assets_with_presentation,
};

pub(crate) struct TerrainLabVisualAssets {
    pub terrain: TexturedTerrainAssets,
    pub profile: TextureVisualProfile,
    pub presentation: TexturePresentation,
}

pub(crate) fn load_terrain_lab_visual_assets(
    authored_bytes: Vec<u8>,
    reference_bytes: Vec<u8>,
    provisional_bytes: Vec<u8>,
    diagnostic_bytes: Vec<u8>,
    profile: &str,
    presentation: &str,
) -> Result<TerrainLabVisualAssets, String> {
    let profile = TextureVisualProfile::parse(profile)
        .ok_or_else(|| format!("unsupported Terrain Lab visual profile {profile:?}"))?;
    let requested_presentation = TexturePresentation::parse(presentation)
        .ok_or_else(|| format!("unsupported Terrain Lab texture presentation {presentation:?}"))?;
    let presentation = if profile == TextureVisualProfile::FirstPartyCoverage {
        TexturePresentation::Textured
    } else {
        requested_presentation
    };

    let authored = parse_pack(authored_bytes, "curated first-party")?;
    let provisional = parse_pack(provisional_bytes, "provisional first-party")?;
    let diagnostic = parse_pack(diagnostic_bytes, "numbered diagnostic")?;
    let reference = if reference_bytes.is_empty() {
        None
    } else {
        Some(parse_pack(reference_bytes, "Minecraft reference")?)
    };

    let reference_descriptor = AssetPackDescriptor::new(
        AssetPackId::new(MINECRAFT_REFERENCE_PACK_ID),
        "Minecraft 1.17.1 Reference",
        AssetPackOrigin::MinecraftReference,
        20,
    )
    .map_err(|error| error.to_string())?;
    let reference_descriptor = if reference.is_some() {
        reference_descriptor
    } else {
        reference_descriptor.unavailable("local Minecraft reference assets are not installed")
    };
    let catalog = AssetPackCatalog::new([
        AssetPackDescriptor::new(
            AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID),
            "Mclone Curated Textures",
            AssetPackOrigin::FirstParty,
            10,
        )
        .map_err(|error| error.to_string())?,
        reference_descriptor,
        AssetPackDescriptor::new(
            AssetPackId::new(PROVISIONAL_FIRST_PARTY_PACK_ID),
            "Mclone Provisional Textures",
            AssetPackOrigin::FirstPartyProvisional,
            30,
        )
        .map_err(|error| error.to_string())?,
        AssetPackDescriptor::new(
            AssetPackId::new(DIAGNOSTIC_MISSING_PACK_ID),
            "Numbered Missing Diagnostics",
            AssetPackOrigin::Diagnostic,
            40,
        )
        .map_err(|error| error.to_string())?,
    ])
    .map_err(|error| error.to_string())?;

    let mut sources = vec![
        (
            AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID),
            Box::new(authored) as Box<dyn AssetSource>,
        ),
        (
            AssetPackId::new(PROVISIONAL_FIRST_PARTY_PACK_ID),
            Box::new(provisional) as Box<dyn AssetSource>,
        ),
        (
            AssetPackId::new(DIAGNOSTIC_MISSING_PACK_ID),
            Box::new(diagnostic) as Box<dyn AssetSource>,
        ),
    ];
    if let Some(reference) = reference {
        sources.push((
            AssetPackId::new(MINECRAFT_REFERENCE_PACK_ID),
            Box::new(reference) as Box<dyn AssetSource>,
        ));
    }
    let selection = profile.selection();
    let source = AssetSourceChain::from_selection(&catalog, &selection, sources)
        .map_err(|error| format!("visual profile {} is unavailable: {error}", profile.label()))?;
    let reference_enabled = selection.is_enabled(&AssetPackId::new(MINECRAFT_REFERENCE_PACK_ID));
    let terrain = if reference_enabled {
        load_textured_terrain_assets_with_presentation(&source, presentation)
    } else {
        load_first_party_textured_terrain_assets_with_presentation(&source, presentation)
    }
    .map_err(|error| {
        format!(
            "failed to prepare Terrain Lab {} / {} materials: {error}",
            profile.label(),
            presentation.label()
        )
    })?;

    Ok(TerrainLabVisualAssets {
        terrain,
        profile,
        presentation,
    })
}

fn parse_pack(bytes: Vec<u8>, label: &str) -> Result<PackedAssetSource, String> {
    PackedAssetSource::from_bytes(bytes)
        .map_err(|error| format!("failed to parse {label} asset pack: {error}"))
}
