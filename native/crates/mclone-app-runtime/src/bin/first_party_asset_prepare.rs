#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use anyhow::{Context, Result, bail};
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::prepared_assets::prepare_first_party_asset_set_from_files;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let json = args.iter().any(|arg| arg == "--json");
    args.retain(|arg| arg != "--json");
    let authored = args.first().map(PathBuf::from).context(
        "usage: first_party_asset_prepare [--json] <authored.pbp> <generated-fallback.pbp>",
    )?;
    let fallback = args.get(1).map(PathBuf::from).context(
        "usage: first_party_asset_prepare [--json] <authored.pbp> <generated-fallback.pbp>",
    )?;
    if args.len() != 2 {
        bail!("usage: first_party_asset_prepare [--json] <authored.pbp> <generated-fallback.pbp>");
    }

    let prepared = prepare_first_party_asset_set_from_files(1, authored, fallback)?;
    let coverage = prepared.coverage;
    if json {
        let summary = prepared.provenance.summary();
        let entries = prepared
            .provenance
            .entries()
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "path": entry.path.as_str(),
                    "pack_id": entry.source.pack_id.as_ref().map(|id| id.as_str()),
                    "origin": origin_label(entry.source.origin),
                    "outcome": outcome_label(entry.outcome),
                })
            })
            .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "schema": 1,
                "strict": true,
                "epoch": prepared.epoch,
                "active_selection": prepared.selection.enabled_ids().map(|id| id.as_str()).collect::<Vec<_>>(),
                "proprietary_free": prepared.provenance.allows_proprietary_free_claim(),
                "provenance": {
                    "first_party": summary.first_party,
                    "generated": summary.generated,
                    "minecraft_reference": summary.minecraft_reference,
                    "unknown": summary.unknown,
                    "suppressed": summary.suppressed,
                    "missing": summary.missing,
                },
                "coverage": {
                    "block_states": coverage.block_states,
                    "atlas_sprites": coverage.atlas_sprites,
                    "actor_figures": coverage.actor_figures,
                    "far_lod_colors": coverage.far_lod_colors,
                    "missing_registry_entries": coverage.missing_registry_entries,
                },
                "entries": entries,
            }))?
        );
        if !prepared.provenance.allows_proprietary_free_claim()
            || summary.minecraft_reference != 0
            || summary.unknown != 0
        {
            bail!("strict first-party provenance validation failed");
        }
        return Ok(());
    }
    println!(
        "prepared_epoch={} block_states={} atlas_sprites={} actor_figures={} far_lod_colors={} missing_registry_entries={}",
        prepared.epoch,
        coverage.block_states,
        coverage.atlas_sprites,
        coverage.actor_figures,
        coverage.far_lod_colors,
        coverage.missing_registry_entries,
    );
    println!(
        "provenance first_party={} generated={} minecraft_reference=0 unknown=0 suppressed={} missing_optional={}",
        coverage.first_party_resolutions,
        coverage.generated_resolutions,
        coverage.suppressed_audio,
        coverage.missing_optional,
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
const fn origin_label(origin: mclone_assets::AssetPackOrigin) -> &'static str {
    match origin {
        mclone_assets::AssetPackOrigin::FirstParty => "first_party",
        mclone_assets::AssetPackOrigin::Generated => "generated",
        mclone_assets::AssetPackOrigin::MinecraftReference => "minecraft_reference",
        mclone_assets::AssetPackOrigin::Unknown => "unknown",
    }
}

#[cfg(not(target_arch = "wasm32"))]
const fn outcome_label(outcome: mclone_assets::AssetResolutionOutcome) -> &'static str {
    match outcome {
        mclone_assets::AssetResolutionOutcome::Resolved => "resolved",
        mclone_assets::AssetResolutionOutcome::Suppressed => "suppressed",
        mclone_assets::AssetResolutionOutcome::Missing => "missing",
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
