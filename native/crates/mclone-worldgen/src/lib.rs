#![forbid(unsafe_code)]

pub mod biome;
pub mod block;
pub mod carver;
pub mod feature;
pub mod levelgen;
pub mod noise;
pub mod placement;
pub mod prng;
pub mod procedural_structure;
pub mod structure_json;
pub mod structure_template;
pub mod surface;
pub mod terrain_analysis;

pub fn target_minecraft_version() -> &'static str {
    mclone_core::TARGET_MINECRAFT_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_core_target_version() {
        assert_eq!(target_minecraft_version(), "1.17.1");
    }
}
