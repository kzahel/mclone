use mclone_assets::{BlockStateRecord, ResourceLocation};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BlockRenderFacts {
    pub(crate) occludes: bool,
    pub(crate) light_emission: u8,
    pub(crate) light_block: u8,
    pub(crate) view_blocking: bool,
    pub(crate) solid_render: bool,
    pub(crate) collision_shape_full_block: bool,
    pub(crate) shade_brightness: f32,
}

/// Java `BlockBehaviour.BlockStateBase.Cache` facts needed by block-model
/// AO and coarse native face/section occlusion.
pub(crate) fn block_render_facts(
    record: &BlockStateRecord,
    occlusion_shape_full_block: bool,
) -> BlockRenderFacts {
    let block = &record.block;
    let can_occlude = java_can_occlude(block);
    let collision_shape_full_block = java_has_collision(block)
        && java_collision_shape_full_block(block, occlusion_shape_full_block);
    let solid_render = can_occlude && occlusion_shape_full_block;
    let propagates_skylight_down = java_propagates_skylight_down(block, occlusion_shape_full_block);
    let light_block = java_light_block(
        block,
        solid_render,
        propagates_skylight_down,
        occlusion_shape_full_block,
    );
    let view_blocking = java_view_blocking(block, collision_shape_full_block);

    BlockRenderFacts {
        occludes: solid_render,
        light_emission: java_light_emission(record),
        light_block,
        view_blocking,
        solid_render,
        collision_shape_full_block,
        shade_brightness: java_shade_brightness(block, collision_shape_full_block),
    }
}

fn java_can_occlude(block: &ResourceLocation) -> bool {
    !java_no_collision(block) && !java_no_occlusion(block)
}

fn java_no_collision(block: &ResourceLocation) -> bool {
    let path = block.path();
    matches!(
        path,
        "air"
            | "cave_air"
            | "water"
            | "lava"
            | "grass"
            | "fern"
            | "large_fern"
            | "dandelion"
            | "poppy"
            | "dead_bush"
            | "vine"
            | "glow_lichen"
            | "sugar_cane"
            | "seagrass"
            | "tall_seagrass"
            | "kelp"
            | "kelp_plant"
            | "sea_pickle"
            | "torch"
            | "wall_torch"
    )
}

fn java_no_occlusion(block: &ResourceLocation) -> bool {
    let path = block.path();
    path.ends_with("_leaves")
        || path.ends_with("_stained_glass")
        || matches!(
            path,
            "glass" | "tinted_glass" | "ice" | "frosted_ice" | "pointed_dripstone"
        )
}

fn java_collision_shape_full_block(
    block: &ResourceLocation,
    occlusion_shape_full_block: bool,
) -> bool {
    if matches!(block.path(), "snow") {
        return false;
    }
    if matches!(block.path(), "pointed_dripstone") {
        return false;
    }
    occlusion_shape_full_block
}

fn java_propagates_skylight_down(
    block: &ResourceLocation,
    occlusion_shape_full_block: bool,
) -> bool {
    let path = block.path();
    if matches!(path, "water" | "lava" | "tinted_glass") {
        return false;
    }
    if java_abstract_glass_block(block)
        || java_bush_like_block(block)
        || matches!(path, "glow_lichen")
    {
        return true;
    }

    !occlusion_shape_full_block
}

fn java_light_block(
    block: &ResourceLocation,
    solid_render: bool,
    propagates_skylight_down: bool,
    _occlusion_shape_full_block: bool,
) -> u8 {
    let path = block.path();
    if path.ends_with("_leaves") {
        return 1;
    }
    if matches!(path, "tinted_glass") {
        return 15;
    }
    if solid_render {
        15
    } else if propagates_skylight_down {
        0
    } else {
        1
    }
}

fn java_light_emission(record: &BlockStateRecord) -> u8 {
    let path = record.block.path();
    if matches!(path, "lava") {
        return 15;
    }
    if matches!(path, "magma_block") {
        return 3;
    }
    if matches!(path, "glow_lichen") {
        return 7;
    }
    if matches!(path, "brown_mushroom") {
        return 1;
    }
    if matches!(path, "torch" | "wall_torch") {
        return 14;
    }
    if matches!(path, "sea_pickle")
        && record
            .properties
            .get("waterlogged")
            .is_some_and(|value| value == "true")
    {
        let pickles = record
            .properties
            .get("pickles")
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(1);
        return 3 + 3 * pickles.clamp(1, 4);
    }
    if matches!(path, "redstone_ore" | "deepslate_redstone_ore")
        && record
            .properties
            .get("lit")
            .is_some_and(|value| value == "true")
    {
        return 9;
    }
    0
}

fn java_view_blocking(block: &ResourceLocation, collision_shape_full_block: bool) -> bool {
    if java_overrides_view_blocking_never(block) {
        return false;
    }
    java_material_blocks_motion(block) && collision_shape_full_block
}

fn java_material_blocks_motion(block: &ResourceLocation) -> bool {
    let path = block.path();
    !matches!(
        path,
        "air"
            | "cave_air"
            | "water"
            | "lava"
            | "snow"
            | "grass"
            | "fern"
            | "large_fern"
            | "dandelion"
            | "poppy"
            | "dead_bush"
            | "brown_mushroom"
            | "red_mushroom"
            | "vine"
            | "glow_lichen"
            | "sugar_cane"
            | "seagrass"
            | "tall_seagrass"
            | "kelp"
            | "kelp_plant"
            | "sea_pickle"
            | "torch"
            | "wall_torch"
    )
}

fn java_overrides_view_blocking_never(block: &ResourceLocation) -> bool {
    let path = block.path();
    path.ends_with("_leaves")
        || path.ends_with("_stained_glass")
        || matches!(path, "glass" | "tinted_glass")
}

fn java_shade_brightness(block: &ResourceLocation, collision_shape_full_block: bool) -> f32 {
    if java_abstract_glass_block(block) {
        1.0
    } else if collision_shape_full_block {
        0.2
    } else {
        1.0
    }
}

fn java_abstract_glass_block(block: &ResourceLocation) -> bool {
    let path = block.path();
    path.ends_with("_stained_glass") || matches!(path, "glass" | "tinted_glass")
}

fn java_bush_like_block(block: &ResourceLocation) -> bool {
    matches!(
        block.path(),
        "grass"
            | "fern"
            | "large_fern"
            | "dandelion"
            | "poppy"
            | "dead_bush"
            | "sugar_cane"
            | "seagrass"
            | "tall_seagrass"
            | "kelp"
            | "kelp_plant"
            | "sea_pickle"
    )
}

fn java_has_collision(block: &ResourceLocation) -> bool {
    !java_no_collision(block)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::BlockStateId;

    fn record(path: &str) -> BlockStateRecord {
        BlockStateRecord::new(
            BlockStateId(0),
            ResourceLocation::parse(path).unwrap(),
            [] as [(&str, &str); 0],
        )
    }

    fn record_with_props(path: &str, props: &[(&str, &str)]) -> BlockStateRecord {
        BlockStateRecord::new(
            BlockStateId(0),
            ResourceLocation::parse(path).unwrap(),
            props.iter().copied(),
        )
    }

    #[test]
    fn full_solid_block_matches_block_state_base_cache_defaults() {
        let facts = block_render_facts(&record("minecraft:stone"), true);

        assert!(facts.occludes);
        assert_eq!(facts.light_emission, 0);
        assert_eq!(facts.light_block, 15);
        assert!(facts.view_blocking);
        assert!(facts.solid_render);
        assert!(facts.collision_shape_full_block);
        assert_eq!(facts.shade_brightness, 0.2);
    }

    #[test]
    fn leaves_use_java_no_occlusion_but_keep_leaf_light_block() {
        let facts = block_render_facts(&record("minecraft:oak_leaves"), true);

        assert!(!facts.occludes);
        assert_eq!(facts.light_block, 1);
        assert!(!facts.view_blocking);
        assert!(!facts.solid_render);
        assert!(facts.collision_shape_full_block);
        assert_eq!(facts.shade_brightness, 0.2);
    }

    #[test]
    fn liquids_are_non_occluding_and_attenuate_like_liquid_block() {
        let water = block_render_facts(&record("minecraft:water"), false);
        let lava = block_render_facts(&record("minecraft:lava"), false);

        assert!(!water.occludes);
        assert_eq!(water.light_block, 1);
        assert_eq!(water.light_emission, 0);
        assert!(!water.view_blocking);
        assert!(!water.collision_shape_full_block);
        assert_eq!(water.shade_brightness, 1.0);

        assert_eq!(lava.light_block, 1);
        assert_eq!(lava.light_emission, 15);
    }

    #[test]
    fn ice_and_packed_ice_split_no_occlusion_from_solid_ice() {
        let ice = block_render_facts(&record("minecraft:ice"), true);
        let packed_ice = block_render_facts(&record("minecraft:packed_ice"), true);

        assert!(!ice.occludes);
        assert_eq!(ice.light_block, 1);
        assert!(ice.view_blocking);
        assert!(!ice.solid_render);
        assert!(ice.collision_shape_full_block);
        assert_eq!(ice.shade_brightness, 0.2);

        assert!(packed_ice.occludes);
        assert_eq!(packed_ice.light_block, 15);
        assert!(packed_ice.view_blocking);
        assert!(packed_ice.solid_render);
    }

    #[test]
    fn pointed_dripstone_is_non_occluding_with_partial_collision() {
        let facts = block_render_facts(&record("minecraft:pointed_dripstone"), false);

        assert!(!facts.occludes);
        assert_eq!(facts.light_block, 0);
        assert!(!facts.view_blocking);
        assert!(!facts.solid_render);
        assert!(!facts.collision_shape_full_block);
        assert_eq!(facts.shade_brightness, 1.0);
    }

    #[test]
    fn glass_and_tinted_glass_use_abstract_glass_brightness() {
        let glass = block_render_facts(&record("minecraft:glass"), true);
        let tinted = block_render_facts(&record("minecraft:tinted_glass"), true);

        assert!(!glass.occludes);
        assert_eq!(glass.light_block, 0);
        assert!(!glass.view_blocking);
        assert_eq!(glass.shade_brightness, 1.0);

        assert!(!tinted.occludes);
        assert_eq!(tinted.light_block, 15);
        assert!(!tinted.view_blocking);
        assert_eq!(tinted.shade_brightness, 1.0);
    }

    #[test]
    fn bush_like_blocks_propagate_skylight() {
        for block in [
            "minecraft:grass",
            "minecraft:fern",
            "minecraft:large_fern",
            "minecraft:dandelion",
            "minecraft:poppy",
            "minecraft:dead_bush",
            "minecraft:brown_mushroom",
            "minecraft:red_mushroom",
            "minecraft:sugar_cane",
            "minecraft:seagrass",
            "minecraft:tall_seagrass",
            "minecraft:kelp",
            "minecraft:kelp_plant",
            "minecraft:sea_pickle",
            "minecraft:glow_lichen",
        ] {
            let facts = block_render_facts(&record(block), false);
            assert!(!facts.occludes, "{block}");
            assert_eq!(facts.light_block, 0, "{block}");
            assert!(!facts.view_blocking, "{block}");
            assert!(!facts.collision_shape_full_block, "{block}");
            assert_eq!(facts.shade_brightness, 1.0, "{block}");
        }
    }

    #[test]
    fn snow_layer_uses_partial_shape_light_facts() {
        let facts = block_render_facts(
            &record_with_props("minecraft:snow", &[("layers", "1")]),
            false,
        );

        assert!(!facts.occludes);
        assert_eq!(facts.light_block, 0);
        assert!(!facts.view_blocking);
        assert!(!facts.collision_shape_full_block);
        assert_eq!(facts.shade_brightness, 1.0);
    }

    #[test]
    fn lit_redstone_ore_emission_is_state_dependent() {
        let dark = block_render_facts(&record("minecraft:redstone_ore"), true);
        let lit = block_render_facts(
            &record_with_props("minecraft:redstone_ore", &[("lit", "true")]),
            true,
        );

        assert_eq!(dark.light_emission, 0);
        assert_eq!(lit.light_emission, 9);
    }

    #[test]
    fn live_sea_pickle_emission_scales_with_pickle_count() {
        let one = block_render_facts(
            &record_with_props(
                "minecraft:sea_pickle",
                &[("pickles", "1"), ("waterlogged", "true")],
            ),
            false,
        );
        let four = block_render_facts(
            &record_with_props(
                "minecraft:sea_pickle",
                &[("pickles", "4"), ("waterlogged", "true")],
            ),
            false,
        );
        let dry = block_render_facts(
            &record_with_props(
                "minecraft:sea_pickle",
                &[("pickles", "4"), ("waterlogged", "false")],
            ),
            false,
        );

        assert_eq!(one.light_emission, 6);
        assert_eq!(four.light_emission, 15);
        assert_eq!(dry.light_emission, 0);
    }

    #[test]
    fn small_mushroom_emission_matches_java_block_properties() {
        let brown = block_render_facts(&record("minecraft:brown_mushroom"), false);
        let red = block_render_facts(&record("minecraft:red_mushroom"), false);

        assert_eq!(brown.light_emission, 1);
        assert_eq!(red.light_emission, 0);
    }

    #[test]
    fn torches_are_non_colliding_emitters() {
        for block in ["minecraft:torch", "minecraft:wall_torch"] {
            let facts = block_render_facts(&record(block), false);
            assert!(!facts.occludes, "{block}");
            assert_eq!(facts.light_block, 0, "{block}");
            assert_eq!(facts.light_emission, 14, "{block}");
            assert!(!facts.view_blocking, "{block}");
            assert!(!facts.collision_shape_full_block, "{block}");
            assert_eq!(facts.shade_brightness, 1.0, "{block}");
        }
    }
}
