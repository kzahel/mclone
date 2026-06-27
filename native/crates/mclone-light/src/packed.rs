use mclone_core::{
    CHUNK_WIDTH, LIGHT_DATA_LAYER_BYTE_COUNT, PackedLightSection, block_to_section_coord,
    chunk_section_index, local_section_block_coord,
};

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

pub fn packed_light_at_local_block_or_fullbright(
    light_sections: &[PackedLightSection],
    min_y: i32,
    height: i32,
    local_x: i32,
    y: i32,
    local_z: i32,
) -> u32 {
    if !(0..CHUNK_WIDTH).contains(&local_x)
        || !(min_y..min_y + height).contains(&y)
        || !(0..CHUNK_WIDTH).contains(&local_z)
    {
        return FULL_BRIGHT;
    }
    if light_sections.is_empty() {
        return FULL_BRIGHT;
    }

    let index = chunk_section_index(local_x, local_section_block_coord(y), local_z);
    let section_y = block_to_section_coord(y);
    pack_light(
        block_light_at(light_sections, section_y, index),
        sky_light_at(light_sections, section_y, index),
    )
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

fn block_light_at(light_sections: &[PackedLightSection], section_y: i32, index: usize) -> u8 {
    light_sections
        .iter()
        .find(|section| section.section_y == section_y)
        .and_then(|section| section.block.as_deref())
        .map_or(0, |layer| data_layer_value(layer, index))
}

fn sky_light_at(light_sections: &[PackedLightSection], section_y: i32, index: usize) -> u8 {
    let mut next_sky_layer = None;
    for section in light_sections {
        if section.section_y < section_y {
            continue;
        }
        let Some(layer) = section.sky.as_deref() else {
            continue;
        };
        if section.section_y == section_y {
            return data_layer_value(layer, index);
        }
        if next_sky_layer.is_none_or(|(next_section_y, _)| section.section_y < next_section_y) {
            next_sky_layer = Some((section.section_y, layer));
        }
    }

    if light_sections.iter().any(|section| section.sky.is_some()) {
        next_sky_layer
            .map(|(_, layer)| data_layer_value(layer, index))
            .unwrap_or(15)
    } else {
        0
    }
}

fn data_layer_value(layer: &[u8], index: usize) -> u8 {
    debug_assert_eq!(layer.len(), LIGHT_DATA_LAYER_BYTE_COUNT);
    let byte = layer[index >> 1];
    let shift = 4 * (index & 1);
    (byte >> shift) & 15
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

    #[test]
    fn packed_light_sampling_reads_exact_section_layers() {
        let index = chunk_section_index(2, 4, 3);
        let sections = [PackedLightSection::new(
            0,
            Some(light_layer_with_value(index, 12)),
            Some(light_layer_with_value(index, 5)),
        )];

        assert_eq!(
            packed_light_at_local_block_or_fullbright(&sections, 0, 16, 2, 4, 3),
            pack_light(5, 12)
        );
    }

    #[test]
    fn packed_light_sampling_uses_mesh_sky_fallbacks() {
        let index = chunk_section_index(2, 4, 3);
        let next_layer_sections = [PackedLightSection::new(
            2,
            Some(light_layer_with_value(index, 9)),
            None,
        )];

        assert_eq!(
            packed_light_at_local_block_or_fullbright(&next_layer_sections, 0, 48, 2, 4, 3),
            pack_light(0, 9)
        );
        assert_eq!(
            packed_light_at_local_block_or_fullbright(&next_layer_sections, 0, 64, 2, 52, 3),
            pack_light(0, 15)
        );
    }

    #[test]
    fn packed_light_sampling_respects_explicit_empty_sky_layers() {
        let index = chunk_section_index(2, 4, 3);
        let sections = [
            PackedLightSection::new(0, Some(vec![0; LIGHT_DATA_LAYER_BYTE_COUNT]), None),
            PackedLightSection::new(1, Some(light_layer_with_value(index, 9)), None),
        ];

        assert_eq!(
            packed_light_at_local_block_or_fullbright(&sections, 0, 32, 2, 4, 3),
            pack_light(0, 0)
        );
    }

    #[test]
    fn packed_light_sampling_falls_back_to_fullbright_without_light_payload() {
        assert_eq!(
            packed_light_at_local_block_or_fullbright(&[], 0, 16, 2, 4, 3),
            FULL_BRIGHT
        );

        let sections = [PackedLightSection::new(
            0,
            Some(light_layer_with_value(0, 1)),
            None,
        )];
        assert_eq!(
            packed_light_at_local_block_or_fullbright(&sections, 0, 16, -1, 4, 3),
            FULL_BRIGHT
        );
        assert_eq!(
            packed_light_at_local_block_or_fullbright(&sections, 0, 16, 2, 16, 3),
            FULL_BRIGHT
        );
    }

    fn light_layer_with_value(index: usize, value: u8) -> Vec<u8> {
        let mut layer = vec![0; LIGHT_DATA_LAYER_BYTE_COUNT];
        let byte_index = index >> 1;
        let shift = 4 * (index & 1);
        layer[byte_index] |= (value & 15) << shift;
        layer
    }
}
