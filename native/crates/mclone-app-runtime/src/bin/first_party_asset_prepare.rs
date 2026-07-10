#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use anyhow::{Context, Result, bail};
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::prepared_assets::prepare_first_party_asset_set_from_files;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let authored = args
        .next()
        .map(PathBuf::from)
        .context("usage: first_party_asset_prepare <authored.pbp> <generated-fallback.pbp>")?;
    let fallback = args
        .next()
        .map(PathBuf::from)
        .context("usage: first_party_asset_prepare <authored.pbp> <generated-fallback.pbp>")?;
    if args.next().is_some() {
        bail!("usage: first_party_asset_prepare <authored.pbp> <generated-fallback.pbp>");
    }

    let prepared = prepare_first_party_asset_set_from_files(1, authored, fallback)?;
    let coverage = prepared.coverage;
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

#[cfg(target_arch = "wasm32")]
fn main() {}
