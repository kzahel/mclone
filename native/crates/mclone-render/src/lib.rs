#![forbid(unsafe_code)]

#[cfg(not(target_arch = "wasm32"))]
mod gpu_util;

mod asset_lab_figure;
mod grass;
pub use grass::{
    GrassInteractor, GrassInteractorIdentity, GrassInteractorSet, GrassQuality,
    MAX_GRASS_INTERACTORS,
};
mod prepared_actor;
mod seasonal_appearance;
pub use seasonal_appearance::SeasonalAppearanceRenderState;

pub mod actor_assets;
pub mod actor_composition_fixture;
pub mod chunk;
pub mod color_profile;
pub mod composition_fixture;
pub mod entity;
pub mod fog;
pub mod gpu_timestamps;
pub mod gui;
pub mod light_texture;
pub mod opaque_world_gate;
pub mod placement;
pub mod prepared_figure;
pub mod screen_effect;
pub mod selection_outline;
pub mod sky;
pub mod sky_render;
pub mod target;
mod texture_mips;
pub mod uniform;
pub mod world_color_mesh;

pub use mclone_diagnostics::GpuPassId;

#[cfg(not(target_arch = "wasm32"))]
pub mod headless;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-surface"))]
pub mod native;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderBackend {
    NativeWgpu,
    WebWgpu,
}

pub fn default_clear_color() -> wgpu::Color {
    wgpu::Color {
        r: 0.035,
        g: 0.04,
        b: 0.05,
        a: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_native_and_web_backends() {
        assert_ne!(RenderBackend::NativeWgpu, RenderBackend::WebWgpu);
    }
}
