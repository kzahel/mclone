use mclone_core::PackedLightSection;

use crate::{DataLayer, DataLayerError, LightLayer};

pub const FULL_BLOCK: u32 = 240;
pub const FULL_SKY: u32 = 15_728_640;
pub const FULL_BRIGHT: u32 = 15_728_880;

pub fn pack_light(block: u8, sky: u8) -> u32 {
    ((block as u32 & 15) << 4) | ((sky as u32 & 15) << 20)
}

pub fn packed_block_light(packed: u32) -> u8 {
    ((packed >> 4) & 15) as u8
}

pub fn packed_sky_light(packed: u32) -> u8 {
    ((packed >> 20) & 15) as u8
}

pub fn packed_light_section_layer(
    section: &PackedLightSection,
    layer: LightLayer,
) -> Result<Option<DataLayer>, DataLayerError> {
    match layer {
        LightLayer::Sky => section.sky.clone().map(DataLayer::from_vec).transpose(),
        LightLayer::Block => section.block.clone().map(DataLayer::from_vec).transpose(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_light_matches_java_light_texture_layout() {
        assert_eq!(pack_light(15, 15), FULL_BRIGHT);
        assert_eq!(pack_light(15, 0), FULL_BLOCK);
        assert_eq!(pack_light(0, 15), FULL_SKY);
        assert_eq!(packed_block_light(FULL_BRIGHT), 15);
        assert_eq!(packed_sky_light(FULL_BRIGHT), 15);
    }
}
