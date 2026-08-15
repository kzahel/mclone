use std::collections::BTreeSet;

use serde::Deserialize;

use crate::{AssetPath, BlockStateRecord, BlockStateRegistry, ResourceLocation};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FirstPartyVisualClass {
    Empty,
    Solid,
    Farmland,
    Slab,
    Stair,
    Fence,
    FenceGate,
    CrossedPlane,
    Flat,
    Fluid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirstPartyBlockVisual {
    pub state: BlockStateRecord,
    pub class: FirstPartyVisualClass,
    pub material: Option<ResourceLocation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetConsumerKind {
    ActorTexture,
    ActorFigure,
    SemanticProp,
    ScreenEffect,
    TerrainColorMap,
    Audio,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetRequirementPolicy {
    Required,
    Optional,
    Suppressible,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAssetRequirement {
    pub path: AssetPath,
    pub consumer: AssetConsumerKind,
    pub policy: AssetRequirementPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalFirstPartyAssetInventory {
    pub block_visuals: Vec<FirstPartyBlockVisual>,
    pub materials: BTreeSet<ResourceLocation>,
    pub direct_assets: Vec<CanonicalAssetRequirement>,
}

/// Repo-owned first-party visual/material inventory.
///
/// This intentionally derives only from the checked-in block-state registry and
/// explicit engine consumer paths. It never opens Minecraft blockstate/model
/// JSON, so pack construction can use it when the reference tree is absent.
pub fn canonical_first_party_asset_inventory() -> CanonicalFirstPartyAssetInventory {
    let block_visuals = BlockStateRegistry::terrain_mvp()
        .records()
        .cloned()
        .map(|state| {
            let class = visual_class(state.block.path());
            let material = (class != FirstPartyVisualClass::Empty).then(|| {
                ResourceLocation::new("mclone", format!("block/{}", material_path(&state)))
                    .expect("registry block paths produce valid first-party material ids")
            });
            FirstPartyBlockVisual {
                state,
                class,
                material,
            }
        })
        .collect::<Vec<_>>();
    let materials = block_visuals
        .iter()
        .filter_map(|visual| visual.material.clone())
        .collect();

    CanonicalFirstPartyAssetInventory {
        block_visuals,
        materials,
        direct_assets: direct_asset_requirements(),
    }
}

fn direct_asset_requirements() -> Vec<CanonicalAssetRequirement> {
    use AssetConsumerKind::{
        ActorFigure, ActorTexture, Audio, ScreenEffect, SemanticProp, TerrainColorMap,
    };
    use AssetRequirementPolicy::{Optional, Required, Suppressible};

    [
        (
            "assets/minecraft/textures/entity/cow/cow.png",
            ActorTexture,
            Required,
        ),
        (
            "assets/mclone/figures/player.figure.json",
            ActorFigure,
            Required,
        ),
        (
            "assets/mclone/figures/upright_bear.figure.json",
            ActorFigure,
            Required,
        ),
        (
            "assets/mclone/figures/cow.figure.json",
            ActorFigure,
            Required,
        ),
        (
            "assets/mclone/figures/chicken.figure.json",
            ActorFigure,
            Required,
        ),
        (
            "assets/mclone/figures/mallard_duck.figure.json",
            ActorFigure,
            Required,
        ),
        (
            "assets/mclone/figures/deer.figure.json",
            ActorFigure,
            Required,
        ),
        (
            "assets/mclone/figures/bee.figure.json",
            ActorFigure,
            Required,
        ),
        (
            "assets/mclone/figures/rabbit.figure.json",
            ActorFigure,
            Required,
        ),
        (
            "assets/mclone/figures/mallard_nest.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/mallard_feather.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/hunting_spear.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/venison.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/deer_hide.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/shed_antler.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/deer_bed.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/wildlife_remains.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/bee_nest.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/bee_hotel.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/bee_hotel_item.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/beeswax.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/wheat_bundle.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/wheat_seeds_item.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/carrot.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/oak_fence_item.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/oak_fence_gate_item.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/mclone/figures/rabbit_burrow.figure.json",
            SemanticProp,
            Required,
        ),
        (
            "assets/minecraft/textures/misc/underwater.png",
            ScreenEffect,
            Required,
        ),
        (
            "assets/minecraft/textures/colormap/grass.png",
            TerrainColorMap,
            Optional,
        ),
        (
            "assets/minecraft/textures/colormap/foliage.png",
            TerrainColorMap,
            Optional,
        ),
        (
            "assets/minecraft/sounds/damage/fallsmall.ogg",
            Audio,
            Suppressible,
        ),
        (
            "assets/minecraft/sounds/damage/fallbig.ogg",
            Audio,
            Suppressible,
        ),
    ]
    .into_iter()
    .map(|(path, consumer, policy)| CanonicalAssetRequirement {
        path: AssetPath::new(path),
        consumer,
        policy,
    })
    .collect()
}

fn visual_class(block: &str) -> FirstPartyVisualClass {
    match block {
        "air" | "cave_air" => FirstPartyVisualClass::Empty,
        "water" | "lava" => FirstPartyVisualClass::Fluid,
        "farmland" => FirstPartyVisualClass::Farmland,
        "spruce_slab" => FirstPartyVisualClass::Slab,
        "spruce_stairs" => FirstPartyVisualClass::Stair,
        "oak_fence" => FirstPartyVisualClass::Fence,
        "oak_fence_gate" => FirstPartyVisualClass::FenceGate,
        "lily_pad" => FirstPartyVisualClass::Flat,
        "grass"
        | "tall_grass"
        | "fern"
        | "large_fern"
        | "dead_bush"
        | "dandelion"
        | "poppy"
        | "allium"
        | "azure_bluet"
        | "red_tulip"
        | "orange_tulip"
        | "white_tulip"
        | "pink_tulip"
        | "oxeye_daisy"
        | "cornflower"
        | "lily_of_the_valley"
        | "lilac"
        | "rose_bush"
        | "peony"
        | "sunflower"
        | "blue_orchid"
        | "brown_mushroom"
        | "red_mushroom"
        | "sugar_cane"
        | "seagrass"
        | "tall_seagrass"
        | "kelp"
        | "kelp_plant"
        | "vine"
        | "tube_coral"
        | "brain_coral"
        | "bubble_coral"
        | "fire_coral"
        | "horn_coral"
        | "tube_coral_fan"
        | "brain_coral_fan"
        | "bubble_coral_fan"
        | "fire_coral_fan"
        | "horn_coral_fan"
        | "tube_coral_wall_fan"
        | "brain_coral_wall_fan"
        | "bubble_coral_wall_fan"
        | "fire_coral_wall_fan"
        | "horn_coral_wall_fan"
        | "wheat"
        | "carrots" => FirstPartyVisualClass::CrossedPlane,
        _ => FirstPartyVisualClass::Solid,
    }
}

fn material_path(state: &BlockStateRecord) -> String {
    match state.block.path() {
        "spruce_slab" | "spruce_stairs" => "spruce_planks".to_owned(),
        "farmland" => {
            if state
                .properties
                .get("moisture")
                .is_some_and(|value| value == "7")
            {
                "farmland_moist".to_owned()
            } else {
                "farmland".to_owned()
            }
        }
        "wheat" => format!(
            "wheat_stage{}",
            state.properties.get("age").map_or("0", String::as_str)
        ),
        "carrots" => {
            let stage = match state.properties.get("age").map(String::as_str) {
                Some("0" | "1") => 0,
                Some("2" | "3") => 1,
                Some("4" | "5" | "6") => 2,
                _ => 3,
            };
            format!("carrots_stage{stage}")
        }
        "oak_fence" | "oak_fence_gate" => "oak_planks".to_owned(),
        block => block.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_inventory_covers_every_repo_owned_runtime_block_state() {
        let inventory = canonical_first_party_asset_inventory();

        assert_eq!(inventory.block_visuals.len(), 309);
        assert_eq!(
            inventory
                .block_visuals
                .iter()
                .map(|visual| visual.state.canonical_key())
                .collect::<BTreeSet<_>>()
                .len(),
            309
        );
        assert!(inventory.block_visuals.iter().any(|visual| {
            visual.state.block.to_string() == "minecraft:water"
                && visual.class == FirstPartyVisualClass::Fluid
        }));
        assert!(inventory.block_visuals.iter().any(|visual| {
            visual.state.block.to_string() == "minecraft:spruce_stairs"
                && visual.class == FirstPartyVisualClass::Stair
                && visual
                    .material
                    .as_ref()
                    .is_some_and(|material| material.to_string() == "mclone:block/spruce_planks")
        }));
        assert!(inventory.block_visuals.iter().any(|visual| {
            visual.state.block.to_string() == "minecraft:oak_fence"
                && visual.class == FirstPartyVisualClass::Fence
                && visual
                    .material
                    .as_ref()
                    .is_some_and(|material| material.to_string() == "mclone:block/oak_planks")
        }));
        assert!(inventory.block_visuals.iter().any(|visual| {
            visual.state.canonical_key() == "minecraft:carrots[age=7]"
                && visual.class == FirstPartyVisualClass::CrossedPlane
                && visual
                    .material
                    .as_ref()
                    .is_some_and(|material| material.to_string() == "mclone:block/carrots_stage3")
        }));
        assert!(
            inventory
                .materials
                .contains(&ResourceLocation::parse("mclone:block/stone").unwrap())
        );
    }

    #[test]
    fn direct_inventory_captures_non_terrain_asset_consumers() {
        let inventory = canonical_first_party_asset_inventory();
        let paths = inventory
            .direct_assets
            .iter()
            .map(|asset| asset.path.as_str())
            .collect::<BTreeSet<_>>();

        for required in [
            "assets/minecraft/textures/entity/cow/cow.png",
            "assets/mclone/figures/player.figure.json",
            "assets/mclone/figures/deer.figure.json",
            "assets/mclone/figures/bee.figure.json",
            "assets/mclone/figures/rabbit.figure.json",
            "assets/mclone/figures/mallard_nest.figure.json",
            "assets/mclone/figures/mallard_feather.figure.json",
            "assets/mclone/figures/hunting_spear.figure.json",
            "assets/mclone/figures/venison.figure.json",
            "assets/mclone/figures/deer_hide.figure.json",
            "assets/mclone/figures/shed_antler.figure.json",
            "assets/mclone/figures/deer_bed.figure.json",
            "assets/mclone/figures/wildlife_remains.figure.json",
            "assets/mclone/figures/bee_nest.figure.json",
            "assets/mclone/figures/bee_hotel.figure.json",
            "assets/mclone/figures/bee_hotel_item.figure.json",
            "assets/mclone/figures/beeswax.figure.json",
            "assets/mclone/figures/wheat_bundle.figure.json",
            "assets/mclone/figures/wheat_seeds_item.figure.json",
            "assets/mclone/figures/carrot.figure.json",
            "assets/mclone/figures/oak_fence_item.figure.json",
            "assets/mclone/figures/oak_fence_gate_item.figure.json",
            "assets/mclone/figures/rabbit_burrow.figure.json",
            "assets/minecraft/textures/misc/underwater.png",
            "assets/minecraft/sounds/damage/fallsmall.ogg",
        ] {
            assert!(
                paths.contains(required),
                "missing inventory path {required}"
            );
        }
    }
}
