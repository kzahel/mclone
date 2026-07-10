#![forbid(unsafe_code)]

#[cfg(not(target_arch = "wasm32"))]
mod gpu_util;

mod asset_lab_figure;

pub mod actor_assets;
pub mod chunk;
pub mod color_profile;
pub mod entity;
pub mod far_lod;
pub mod fog;
pub mod gpu_timestamps;
pub mod gui;
pub mod light_texture;
pub mod screen_effect;
pub mod selection_outline;
pub mod sky;
pub mod sky_render;
pub mod target;
mod texture_mips;
pub mod uniform;

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
