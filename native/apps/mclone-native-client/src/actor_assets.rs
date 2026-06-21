use anyhow::Result;
use mclone_render::actor_assets::load_actor_texture_assets as load_actor_texture_assets_from_source;

pub(crate) use mclone_render::actor_assets::ActorTextureAssets;

use crate::render_cache::load_asset_source;

pub(crate) fn load_actor_texture_assets() -> Result<ActorTextureAssets> {
    let source = load_asset_source()?;
    Ok(load_actor_texture_assets_from_source(&source)?)
}
