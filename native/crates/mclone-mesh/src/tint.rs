use crate::catalog::{TexturedBlockTint, TexturedFluidKind, TexturedMeshCatalog};

pub(crate) fn blended_liquid_color<F>(
    kind: TexturedFluidKind,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    shade: f32,
    biome_id_at: F,
) -> [f32; 4]
where
    F: Fn(i32, i32, i32) -> i32,
{
    let color = match kind {
        TexturedFluidKind::Water => rgb8_alpha(
            blended_biome_color(world_x, world_y, world_z, biome_id_at, |biome_id, _, _| {
                biome_visual(biome_id).water_color
            }),
            0.72,
        ),
        TexturedFluidKind::Lava => [1.0, 1.0, 1.0, 1.0],
    };
    [
        color[0] * shade,
        color[1] * shade,
        color[2] * shade,
        color[3],
    ]
}

pub(crate) fn block_tint<F>(
    catalog: &TexturedMeshCatalog,
    tint: TexturedBlockTint,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    biome_id_at: F,
) -> [f32; 3]
where
    F: Fn(i32, i32, i32) -> i32,
{
    match tint {
        TexturedBlockTint::None => [1.0, 1.0, 1.0],
        TexturedBlockTint::Grass => rgb8(blended_biome_color(
            world_x,
            world_y,
            world_z,
            biome_id_at,
            |biome_id, sample_x, sample_z| {
                grass_color(catalog, biome_visual(biome_id), sample_x, sample_z)
            },
        )),
        TexturedBlockTint::Foliage => rgb8(blended_biome_color(
            world_x,
            world_y,
            world_z,
            biome_id_at,
            |biome_id, _, _| foliage_color(catalog, biome_visual(biome_id)),
        )),
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
const BIOME_BLEND_RADIUS: i32 = 2;
const SWAMP_GRASS_COLOR_DARK: u32 = 0x4c_76_3c;
const SWAMP_GRASS_COLOR_LIGHT: u32 = 0x6a_70_39;
const SWAMP_GRASS_NOISE_SCALE: f64 = 0.0225;
const SWAMP_GRASS_NOISE_THRESHOLD: f64 = -0.1;
const SIMPLEX_F2: f64 = 0.366_025_403_784_438_6;
const SIMPLEX_G2: f64 = 0.211_324_865_405_187_13;

const SIMPLEX_GRADIENTS: [[i32; 3]; 16] = [
    [1, 1, 0],
    [-1, 1, 0],
    [1, -1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [-1, 0, 1],
    [1, 0, -1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, -1, 1],
    [0, 1, -1],
    [0, -1, -1],
    [1, 1, 0],
    [0, -1, 1],
    [-1, 1, 0],
    [0, -1, -1],
];

// Fixed permutation for Biome.BIOME_INFO_NOISE: PerlinSimplexNoise(new
// WorldgenRandom(2345L), [0]). The constructor consumes SimplexNoise xo/yo/zo,
// but getValue(..., false) does not use those offsets.
const BIOME_INFO_NOISE_PERMUTATION: [u8; 256] = [
    64, 175, 124, 148, 10, 239, 244, 91, 138, 73, 228, 171, 27, 134, 77, 122, 238, 196, 202, 181,
    211, 7, 49, 173, 48, 165, 120, 217, 129, 56, 153, 8, 140, 141, 21, 130, 71, 100, 132, 23, 176,
    250, 29, 104, 149, 159, 180, 237, 247, 11, 252, 241, 14, 2, 219, 75, 178, 151, 233, 251, 103,
    45, 52, 201, 222, 18, 223, 88, 136, 34, 227, 235, 35, 160, 0, 131, 51, 214, 39, 216, 207, 26,
    137, 185, 41, 13, 249, 54, 112, 5, 66, 242, 157, 158, 28, 89, 86, 192, 172, 17, 69, 204, 38,
    221, 65, 166, 9, 226, 33, 30, 84, 240, 59, 224, 127, 108, 92, 146, 99, 195, 255, 98, 126, 4,
    133, 236, 189, 121, 144, 183, 80, 109, 191, 218, 161, 53, 25, 93, 72, 150, 163, 234, 205, 152,
    61, 37, 197, 78, 81, 32, 85, 70, 187, 63, 96, 115, 117, 184, 139, 79, 74, 46, 188, 182, 76, 31,
    174, 57, 68, 198, 90, 245, 230, 106, 94, 212, 190, 16, 200, 213, 206, 44, 43, 215, 231, 12,
    177, 203, 220, 24, 170, 19, 209, 82, 95, 125, 194, 248, 208, 55, 67, 1, 87, 110, 135, 162, 128,
    3, 60, 225, 15, 186, 232, 145, 119, 142, 113, 154, 102, 164, 42, 156, 210, 22, 253, 147, 169,
    193, 83, 143, 118, 123, 254, 167, 111, 114, 6, 50, 40, 199, 179, 246, 20, 107, 168, 97, 229,
    101, 155, 62, 47, 58, 116, 243, 105, 36,
];

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
        GrassColorModifier::Swamp => swamp_grass_color(world_x, world_z),
    }
}

fn foliage_color(catalog: &TexturedMeshCatalog, visual: BiomeVisual) -> u32 {
    visual.foliage_color_override.unwrap_or_else(|| {
        catalog
            .foliage_color_from_colormap(visual.temperature, visual.downfall)
            .unwrap_or(visual.foliage_color)
    })
}

fn blended_biome_color<B, C>(
    world_x: i32,
    world_y: i32,
    world_z: i32,
    biome_id_at: B,
    color_for_biome: C,
) -> u32
where
    B: Fn(i32, i32, i32) -> i32,
    C: Fn(i32, i32, i32) -> u32,
{
    if BIOME_BLEND_RADIUS == 0 {
        return color_for_biome(biome_id_at(world_x, world_y, world_z), world_x, world_z);
    }

    let mut red = 0;
    let mut green = 0;
    let mut blue = 0;
    for sample_x in world_x - BIOME_BLEND_RADIUS..=world_x + BIOME_BLEND_RADIUS {
        for sample_z in world_z - BIOME_BLEND_RADIUS..=world_z + BIOME_BLEND_RADIUS {
            let biome_id = biome_id_at(sample_x, world_y, sample_z);
            let color = color_for_biome(biome_id, sample_x, sample_z);
            red += (color >> 16) & 0xff;
            green += (color >> 8) & 0xff;
            blue += color & 0xff;
        }
    }

    let sample_count = ((BIOME_BLEND_RADIUS * 2 + 1) * (BIOME_BLEND_RADIUS * 2 + 1)) as u32;
    ((red / sample_count) & 0xff) << 16
        | ((green / sample_count) & 0xff) << 8
        | ((blue / sample_count) & 0xff)
}

fn swamp_grass_color(world_x: i32, world_z: i32) -> u32 {
    if biome_info_noise_value(world_x, world_z) < SWAMP_GRASS_NOISE_THRESHOLD {
        SWAMP_GRASS_COLOR_DARK
    } else {
        SWAMP_GRASS_COLOR_LIGHT
    }
}

fn biome_info_noise_value(world_x: i32, world_z: i32) -> f64 {
    biome_info_simplex_noise_2d(
        world_x as f64 * SWAMP_GRASS_NOISE_SCALE,
        world_z as f64 * SWAMP_GRASS_NOISE_SCALE,
    )
}

fn biome_info_simplex_noise_2d(x: f64, z: f64) -> f64 {
    let skew = (x + z) * SIMPLEX_F2;
    let cell_x = (x + skew).floor() as i32;
    let cell_z = (z + skew).floor() as i32;
    let unskew = (cell_x + cell_z) as f64 * SIMPLEX_G2;
    let cell_origin_x = cell_x as f64 - unskew;
    let cell_origin_z = cell_z as f64 - unskew;
    let local_x = x - cell_origin_x;
    let local_z = z - cell_origin_z;
    let (offset_x, offset_z) = if local_x > local_z { (1, 0) } else { (0, 1) };
    let second_corner_x = local_x - offset_x as f64 + SIMPLEX_G2;
    let second_corner_z = local_z - offset_z as f64 + SIMPLEX_G2;
    let third_corner_x = local_x - 1.0 + 2.0 * SIMPLEX_G2;
    let third_corner_z = local_z - 1.0 + 2.0 * SIMPLEX_G2;
    let perm_x = cell_x & 0xff;
    let perm_z = cell_z & 0xff;
    let gradient0 = biome_info_permutation(perm_x + biome_info_permutation(perm_z)) % 12;
    let gradient1 =
        biome_info_permutation(perm_x + offset_x + biome_info_permutation(perm_z + offset_z)) % 12;
    let gradient2 = biome_info_permutation(perm_x + 1 + biome_info_permutation(perm_z + 1)) % 12;
    let corner0 = simplex_corner_noise(gradient0, local_x, local_z, 0.0, 0.5);
    let corner1 = simplex_corner_noise(gradient1, second_corner_x, second_corner_z, 0.0, 0.5);
    let corner2 = simplex_corner_noise(gradient2, third_corner_x, third_corner_z, 0.0, 0.5);
    70.0 * (corner0 + corner1 + corner2)
}

fn biome_info_permutation(index: i32) -> i32 {
    BIOME_INFO_NOISE_PERMUTATION[(index & 0xff) as usize] as i32
}

fn simplex_corner_noise(gradient_index: i32, x: f64, y: f64, z: f64, offset: f64) -> f64 {
    let mut value = offset - x * x - y * y - z * z;
    if value < 0.0 {
        return 0.0;
    }

    value *= value;
    value * value * simplex_dot(gradient_index as usize, x, y, z)
}

fn simplex_dot(gradient_index: usize, x: f64, y: f64, z: f64) -> f64 {
    let gradient = SIMPLEX_GRADIENTS[gradient_index & 15];
    gradient[0] as f64 * x + gradient[1] as f64 * y + gradient[2] as f64 * z
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
    fn swamp_grass_modifier_matches_java_biome_info_noise() {
        let catalog = TexturedMeshCatalog::default();
        let visual = biome_visual(6);
        let cases = [
            (-100, -100, 0xbfcf_6d0e_e96b_8ad7, SWAMP_GRASS_COLOR_DARK),
            (-51, 193, 0xbfc3_f7a5_1f45_17b8, SWAMP_GRASS_COLOR_DARK),
            (0, 0, 0x0000_0000_0000_0000, SWAMP_GRASS_COLOR_LIGHT),
            (100, 0, 0xbfe0_7e43_e139_0f53, SWAMP_GRASS_COLOR_DARK),
            (193, 193, 0x3fdf_eeab_0f8e_d6aa, SWAMP_GRASS_COLOR_LIGHT),
        ];

        for (world_x, world_z, expected_noise_bits, expected_color) in cases {
            assert_eq!(
                biome_info_noise_value(world_x, world_z).to_bits(),
                expected_noise_bits,
                "Biome.BIOME_INFO_NOISE mismatch at ({world_x}, {world_z})"
            );
            assert_eq!(
                grass_color(&catalog, visual, world_x, world_z),
                expected_color,
                "swamp grass color mismatch at ({world_x}, {world_z})"
            );
        }
    }

    #[test]
    fn biome_blend_averages_default_five_by_five_block_window() {
        let color = blended_biome_color(
            0,
            64,
            0,
            |sample_x, sample_y, _sample_z| {
                assert_eq!(sample_y, 64);
                if sample_x < 0 { 6 } else { 1 }
            },
            |biome_id, _sample_x, _sample_z| {
                if biome_id == 6 {
                    0x00_00_00
                } else {
                    0x19_32_4b
                }
            },
        );

        assert_eq!(color, 0x0f_1e_2d);
    }

    #[test]
    fn water_tint_uses_biome_blend_average() {
        let color = blended_liquid_color(
            TexturedFluidKind::Water,
            0,
            64,
            0,
            1.0,
            |sample_x, _sample_y, _sample_z| if sample_x < 0 { 6 } else { 1 },
        );
        let expected = rgb8_alpha(0x4c_78_b0, 0.72);

        assert_eq!(color, expected);
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
                0,
                64,
                0,
                |_, _, _| 1
            ),
            rgb8(0x80_a7_55)
        );
        assert_eq!(
            block_tint(
                &TexturedMeshCatalog::default(),
                TexturedBlockTint::EvergreenFoliage,
                0,
                64,
                0,
                |_, _, _| 1
            ),
            rgb8(0x61_99_61)
        );
    }
}
