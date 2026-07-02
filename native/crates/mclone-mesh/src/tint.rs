use crate::catalog::{TexturedBlockTint, TexturedFluidKind, TexturedMeshCatalog};

pub(crate) fn liquid_color(kind: TexturedFluidKind, biome_id: i32, shade: f32) -> [f32; 4] {
    let color = match kind {
        TexturedFluidKind::Water => rgb8_alpha(biome_visual(biome_id).water_color, 0.72),
        TexturedFluidKind::Lava => [1.0, 1.0, 1.0, 1.0],
    };
    [
        color[0] * shade,
        color[1] * shade,
        color[2] * shade,
        color[3],
    ]
}

pub(crate) fn block_tint(
    catalog: &TexturedMeshCatalog,
    tint: TexturedBlockTint,
    biome_id: i32,
    world_x: i32,
    world_z: i32,
) -> [f32; 3] {
    let visual = biome_visual(biome_id);
    match tint {
        TexturedBlockTint::None => [1.0, 1.0, 1.0],
        TexturedBlockTint::Grass => rgb8(grass_color(catalog, visual, world_x, world_z)),
        TexturedBlockTint::Foliage => rgb8(foliage_color(catalog, visual)),
        TexturedBlockTint::BirchFoliage => rgb8(0x80_a7_55),
        TexturedBlockTint::EvergreenFoliage => rgb8(0x61_99_61),
    }
}

#[derive(Clone, Copy, Debug)]
struct BiomeVisual {
    temperature: f32,
    downfall: f32,
    grass_color: u32,
    grass_color_override: Option<u32>,
    foliage_color: u32,
    foliage_color_override: Option<u32>,
    water_color: u32,
    grass_modifier: GrassColorModifier,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GrassColorModifier {
    None,
    DarkForest,
    Swamp,
}

const DEFAULT_WATER_COLOR: u32 = 0x3f_76_e4;
const COLD_WATER_COLOR: u32 = 0x3d_57_d6;
const FROZEN_WATER_COLOR: u32 = 0x39_38_c9;
const WARM_OCEAN_WATER_COLOR: u32 = 0x43_d5_ee;
const LUKEWARM_OCEAN_WATER_COLOR: u32 = 0x45_ad_f2;

fn biome_visual(biome_id: i32) -> BiomeVisual {
    let plains = BiomeVisual {
        temperature: 0.8,
        downfall: 0.4,
        grass_color: 0x91_bd_59,
        grass_color_override: None,
        foliage_color: 0x77_ab_2f,
        foliage_color_override: None,
        water_color: DEFAULT_WATER_COLOR,
        grass_modifier: GrassColorModifier::None,
    };
    match biome_id {
        0 | 24 => BiomeVisual {
            temperature: 0.5,
            downfall: 0.5,
            grass_color: 0x8e_b9_71,
            foliage_color: 0x71_a7_4d,
            ..plains
        },
        44 | 47 => BiomeVisual {
            temperature: 0.5,
            downfall: 0.5,
            grass_color: 0x8e_b9_71,
            foliage_color: 0x71_a7_4d,
            water_color: WARM_OCEAN_WATER_COLOR,
            ..plains
        },
        45 | 48 => BiomeVisual {
            temperature: 0.5,
            downfall: 0.5,
            grass_color: 0x8e_b9_71,
            foliage_color: 0x71_a7_4d,
            water_color: LUKEWARM_OCEAN_WATER_COLOR,
            ..plains
        },
        46 | 49 => BiomeVisual {
            temperature: 0.5,
            downfall: 0.5,
            grass_color: 0x8e_b9_71,
            foliage_color: 0x71_a7_4d,
            water_color: COLD_WATER_COLOR,
            ..plains
        },
        50 => BiomeVisual {
            temperature: 0.5,
            downfall: 0.5,
            grass_color: 0x8e_b9_71,
            foliage_color: 0x71_a7_4d,
            water_color: FROZEN_WATER_COLOR,
            ..plains
        },
        1 | 16 | 129 => plains,
        2 | 17 | 130 => BiomeVisual {
            temperature: 2.0,
            downfall: 0.0,
            grass_color: 0xb5_b7_55,
            foliage_color: 0xae_b4_55,
            ..plains
        },
        3 | 20 | 25 | 34 | 131 | 162 => BiomeVisual {
            temperature: 0.2,
            downfall: 0.3,
            grass_color: 0x8a_b6_89,
            foliage_color: 0x6f_a0_78,
            ..plains
        },
        4 | 18 | 132 => BiomeVisual {
            temperature: 0.7,
            downfall: 0.8,
            grass_color: 0x79_c0_5a,
            foliage_color: 0x59_9b_35,
            ..plains
        },
        5 | 19 | 133 => BiomeVisual {
            temperature: 0.25,
            downfall: 0.8,
            grass_color: 0x86_b7_83,
            foliage_color: 0x68_9b_68,
            ..plains
        },
        7 => BiomeVisual {
            temperature: 0.5,
            downfall: 0.5,
            grass_color: 0x8e_b9_71,
            foliage_color: 0x71_a7_4d,
            ..plains
        },
        30 | 31 | 158 => BiomeVisual {
            temperature: -0.5,
            downfall: 0.4,
            grass_color: 0x86_b7_83,
            foliage_color: 0x68_9b_68,
            water_color: COLD_WATER_COLOR,
            ..plains
        },
        32 | 33 => BiomeVisual {
            temperature: 0.3,
            downfall: 0.8,
            grass_color: 0x86_b7_83,
            foliage_color: 0x68_9b_68,
            ..plains
        },
        160 | 161 => BiomeVisual {
            temperature: 0.25,
            downfall: 0.8,
            grass_color: 0x86_b7_83,
            foliage_color: 0x68_9b_68,
            ..plains
        },
        6 | 134 => BiomeVisual {
            temperature: 0.8,
            downfall: 0.9,
            grass_color: 0x6a_70_39,
            grass_color_override: None,
            foliage_color: 0x6a_70_39,
            foliage_color_override: Some(0x6a_70_39),
            water_color: 0x61_7b_64,
            grass_modifier: GrassColorModifier::Swamp,
            ..plains
        },
        10 | 11 => BiomeVisual {
            temperature: 0.0,
            downfall: 0.5,
            grass_color: 0x80_b4_97,
            foliage_color: 0x60_93_80,
            water_color: FROZEN_WATER_COLOR,
            ..plains
        },
        12 | 13 | 140 => BiomeVisual {
            temperature: 0.0,
            downfall: 0.5,
            grass_color: 0x80_b4_97,
            foliage_color: 0x60_93_80,
            ..plains
        },
        26 => BiomeVisual {
            temperature: 0.05,
            downfall: 0.3,
            grass_color: 0x8a_b6_89,
            foliage_color: 0x6f_a0_78,
            water_color: COLD_WATER_COLOR,
            ..plains
        },
        14 | 15 => BiomeVisual {
            temperature: 0.9,
            downfall: 1.0,
            grass_color: 0x55_c9_3f,
            foliage_color: 0x2f_b2_33,
            ..plains
        },
        21 | 22 | 149 | 168 | 169 => BiomeVisual {
            temperature: 0.95,
            downfall: 0.9,
            grass_color: 0x59_c9_3c,
            foliage_color: 0x30_bb_0b,
            ..plains
        },
        23 | 151 => BiomeVisual {
            temperature: 0.95,
            downfall: 0.8,
            grass_color: 0x59_c9_3c,
            foliage_color: 0x30_bb_0b,
            ..plains
        },
        27 | 28 | 155 | 156 => BiomeVisual {
            temperature: 0.6,
            downfall: 0.6,
            grass_color: 0x88_bb_67,
            foliage_color: 0x80_a7_55,
            ..plains
        },
        29 | 157 => BiomeVisual {
            temperature: 0.7,
            downfall: 0.8,
            grass_color: 0x79_c0_5a,
            foliage_color: 0x59_9b_35,
            grass_modifier: GrassColorModifier::DarkForest,
            ..plains
        },
        35 => BiomeVisual {
            temperature: 1.2,
            downfall: 0.0,
            grass_color: 0xb5_b7_55,
            foliage_color: 0xae_b4_55,
            ..plains
        },
        36 | 164 => BiomeVisual {
            temperature: 1.0,
            downfall: 0.0,
            grass_color: 0xb5_b7_55,
            foliage_color: 0xae_b4_55,
            ..plains
        },
        163 => BiomeVisual {
            temperature: 1.1,
            downfall: 0.0,
            grass_color: 0xb5_b7_55,
            foliage_color: 0xae_b4_55,
            ..plains
        },
        37 | 38 | 39 | 165 | 166 | 167 => BiomeVisual {
            temperature: 2.0,
            downfall: 0.0,
            grass_color: 0x90_81_4d,
            grass_color_override: Some(0x90_81_4d),
            foliage_color: 0x9e_81_4d,
            foliage_color_override: Some(0x9e_81_4d),
            water_color: DEFAULT_WATER_COLOR,
            ..plains
        },
        _ => plains,
    }
}

fn grass_color(
    catalog: &TexturedMeshCatalog,
    visual: BiomeVisual,
    world_x: i32,
    world_z: i32,
) -> u32 {
    let base_color = visual.grass_color_override.unwrap_or_else(|| {
        catalog
            .grass_color_from_colormap(visual.temperature, visual.downfall)
            .unwrap_or(visual.grass_color)
    });
    match visual.grass_modifier {
        GrassColorModifier::None => base_color,
        GrassColorModifier::DarkForest => ((base_color & 0xfe_fe_fe) + 0x28_34_0a) >> 1,
        GrassColorModifier::Swamp => {
            if coarse_position_noise(world_x, world_z) < 0 {
                0x4c_76_3c
            } else {
                0x6a_70_39
            }
        }
    }
}

fn foliage_color(catalog: &TexturedMeshCatalog, visual: BiomeVisual) -> u32 {
    visual.foliage_color_override.unwrap_or_else(|| {
        catalog
            .foliage_color_from_colormap(visual.temperature, visual.downfall)
            .unwrap_or(visual.foliage_color)
    })
}

fn coarse_position_noise(world_x: i32, world_z: i32) -> i32 {
    let mut value = world_x as i64 * 341_873_128_712 + world_z as i64 * 132_897_987_541;
    value ^= value >> 13;
    value = value.wrapping_mul(0x5deece66d);
    ((value >> 24) & 1) as i32 * 2 - 1
}

fn rgb8(color: u32) -> [f32; 3] {
    [
        ((color >> 16) & 0xff) as f32 / 255.0,
        ((color >> 8) & 0xff) as f32 / 255.0,
        (color & 0xff) as f32 / 255.0,
    ]
}

fn rgb8_alpha(color: u32, alpha: f32) -> [f32; 4] {
    let [r, g, b] = rgb8(color);
    [r, g, b, alpha]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_forest_grass_modifier_matches_java_constant() {
        let color = grass_color(&TexturedMeshCatalog::default(), biome_visual(29), -94, 348);

        assert_eq!(color, 0x50_7a_32);
    }

    #[test]
    fn biome_visual_water_colors_match_vanilla_special_effects() {
        let cases = [
            (0, DEFAULT_WATER_COLOR),
            (7, DEFAULT_WATER_COLOR),
            (10, FROZEN_WATER_COLOR),
            (11, FROZEN_WATER_COLOR),
            (12, DEFAULT_WATER_COLOR),
            (13, DEFAULT_WATER_COLOR),
            (26, COLD_WATER_COLOR),
            (30, COLD_WATER_COLOR),
            (31, COLD_WATER_COLOR),
            (44, WARM_OCEAN_WATER_COLOR),
            (45, LUKEWARM_OCEAN_WATER_COLOR),
            (46, COLD_WATER_COLOR),
            (47, WARM_OCEAN_WATER_COLOR),
            (48, LUKEWARM_OCEAN_WATER_COLOR),
            (49, COLD_WATER_COLOR),
            (50, FROZEN_WATER_COLOR),
            (134, 0x61_7b_64),
            (140, DEFAULT_WATER_COLOR),
            (158, COLD_WATER_COLOR),
        ];

        for (biome_id, expected) in cases {
            assert_eq!(
                biome_visual(biome_id).water_color,
                expected,
                "biome id {biome_id} water color"
            );
        }
    }

    #[test]
    fn biome_visual_climate_inputs_match_vanilla_colormap_inputs() {
        let cases = [
            (7, 0.5, 0.5),
            (10, 0.0, 0.5),
            (11, 0.0, 0.5),
            (12, 0.0, 0.5),
            (13, 0.0, 0.5),
            (21, 0.95, 0.9),
            (23, 0.95, 0.8),
            (26, 0.05, 0.3),
            (30, -0.5, 0.4),
            (32, 0.3, 0.8),
            (36, 1.0, 0.0),
            (50, 0.5, 0.5),
            (140, 0.0, 0.5),
            (151, 0.95, 0.8),
            (160, 0.25, 0.8),
            (161, 0.25, 0.8),
            (163, 1.1, 0.0),
            (164, 1.0, 0.0),
        ];

        for (biome_id, expected_temperature, expected_downfall) in cases {
            let visual = biome_visual(biome_id);
            assert_eq!(
                visual.temperature.to_bits(),
                f32::to_bits(expected_temperature),
                "biome id {biome_id} temperature"
            );
            assert_eq!(
                visual.downfall.to_bits(),
                f32::to_bits(expected_downfall),
                "biome id {biome_id} downfall"
            );
        }
    }

    #[test]
    fn special_biome_visual_overrides_match_vanilla_constants() {
        let swamp = biome_visual(6);
        assert_eq!(swamp.foliage_color_override, Some(0x6a_70_39));
        assert_eq!(swamp.grass_modifier, GrassColorModifier::Swamp);

        let badlands = biome_visual(37);
        assert_eq!(badlands.grass_color_override, Some(0x90_81_4d));
        assert_eq!(badlands.foliage_color_override, Some(0x9e_81_4d));

        assert_eq!(
            block_tint(
                &TexturedMeshCatalog::default(),
                TexturedBlockTint::BirchFoliage,
                1,
                0,
                0
            ),
            rgb8(0x80_a7_55)
        );
        assert_eq!(
            block_tint(
                &TexturedMeshCatalog::default(),
                TexturedBlockTint::EvergreenFoliage,
                1,
                0,
                0
            ),
            rgb8(0x61_99_61)
        );
    }
}
