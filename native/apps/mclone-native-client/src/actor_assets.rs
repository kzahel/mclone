use anyhow::Result;
pub(crate) use mclone_app_runtime::render_assets::ActorTextureAssets;

pub(crate) fn load_actor_texture_assets() -> Result<ActorTextureAssets> {
    mclone_app_runtime::render_assets::load_actor_texture_assets()
}
