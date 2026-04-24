import { Registry } from "../../../core/registry";
import { Heightmap } from "../heightmap";
import { ClampedNormalFloat } from "../../../util/valueproviders/clamped-normal-float";
import { UniformFloat } from "../../../util/valueproviders/uniform-float";
import { ResourceLocation } from "../../../core/resource-location";
import { UniformInt } from "../../../util/valueproviders/uniform-int";
import { absolute, bottom, top } from "../../carver/carver-config";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { DiskConfiguration } from "./configurations/disk-configuration";
import { DripstoneClusterConfiguration } from "./configurations/dripstone-cluster-configuration";
import { GlowLichenConfiguration } from "./configurations/glow-lichen-configuration";
import { HeightmapConfiguration } from "./configurations/heightmap-configuration";
import { OreConfiguration } from "./configurations/ore-configuration";
import { ReplaceBlockConfiguration } from "./configurations/replace-block-configuration";
import { SmallDripstoneConfiguration } from "./configurations/small-dripstone-configuration";
import { Features } from "./features";
import { FeatureDecorators } from "../placement/feature-decorators";

const DIRT_LOCATION = new ResourceLocation("minecraft:dirt");
const CLAY_LOCATION = new ResourceLocation("minecraft:clay");
const GRASS_BLOCK_LOCATION = new ResourceLocation("minecraft:grass_block");
const SAND_LOCATION = new ResourceLocation("minecraft:sand");
const GRAVEL_LOCATION = new ResourceLocation("minecraft:gravel");
const GRANITE_LOCATION = new ResourceLocation("minecraft:granite");
const DIORITE_LOCATION = new ResourceLocation("minecraft:diorite");
const ANDESITE_LOCATION = new ResourceLocation("minecraft:andesite");
const CALCITE_LOCATION = new ResourceLocation("minecraft:calcite");
const DRIPSTONE_BLOCK_LOCATION = new ResourceLocation("minecraft:dripstone_block");
const TUFF_LOCATION = new ResourceLocation("minecraft:tuff");
const DEEPSLATE_LOCATION = new ResourceLocation("minecraft:deepslate");
const COAL_ORE_LOCATION = new ResourceLocation("minecraft:coal_ore");
const DEEPSLATE_COAL_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_coal_ore");
const IRON_ORE_LOCATION = new ResourceLocation("minecraft:iron_ore");
const DEEPSLATE_IRON_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_iron_ore");
const GOLD_ORE_LOCATION = new ResourceLocation("minecraft:gold_ore");
const DEEPSLATE_GOLD_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_gold_ore");
const EMERALD_ORE_LOCATION = new ResourceLocation("minecraft:emerald_ore");
const DEEPSLATE_EMERALD_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_emerald_ore");
const REDSTONE_ORE_LOCATION = new ResourceLocation("minecraft:redstone_ore");
const DEEPSLATE_REDSTONE_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_redstone_ore");
const DIAMOND_ORE_LOCATION = new ResourceLocation("minecraft:diamond_ore");
const DEEPSLATE_DIAMOND_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_diamond_ore");
const LAPIS_ORE_LOCATION = new ResourceLocation("minecraft:lapis_ore");
const DEEPSLATE_LAPIS_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_lapis_ore");
const COPPER_ORE_LOCATION = new ResourceLocation("minecraft:copper_ore");
const DEEPSLATE_COPPER_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_copper_ore");
const INFESTED_STONE_LOCATION = new ResourceLocation("minecraft:infested_stone");
const INFESTED_DEEPSLATE_LOCATION = new ResourceLocation("minecraft:infested_deepslate");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function topSolidHeightmapSquare() {
  return FeatureDecorators.HEIGHTMAP.configured(new HeightmapConfiguration(Heightmap.Types.OCEAN_FLOOR_WG)).squared();
}

export class OreFeatures {
  public static get DISK_CLAY() {
    return Features.DISK.configured(
      new DiskConfiguration(getRequiredState(CLAY_LOCATION), UniformInt.of(2, 3), 1, [getRequiredState(DIRT_LOCATION), getRequiredState(CLAY_LOCATION)]),
    ).decorated(topSolidHeightmapSquare());
  }

  public static get DISK_GRAVEL() {
    return Features.DISK.configured(
      new DiskConfiguration(getRequiredState(GRAVEL_LOCATION), UniformInt.of(2, 5), 2, [getRequiredState(DIRT_LOCATION), getRequiredState(GRASS_BLOCK_LOCATION)]),
    ).decorated(topSolidHeightmapSquare());
  }

  public static get DISK_SAND() {
    return Features.DISK.configured(
      new DiskConfiguration(getRequiredState(SAND_LOCATION), UniformInt.of(2, 6), 2, [getRequiredState(DIRT_LOCATION), getRequiredState(GRASS_BLOCK_LOCATION)]),
    )
      .decorated(topSolidHeightmapSquare())
      .count(3);
  }

  public static get ORE_DIRT() {
    return Features.ORE.configured(new OreConfiguration(OreConfiguration.Predicates.NATURAL_STONE, getRequiredState(DIRT_LOCATION), 33))
      .rangeUniform(absolute(0), top())
      .squared()
      .count(10);
  }

  public static get ORE_GRAVEL() {
    return Features.ORE.configured(new OreConfiguration(OreConfiguration.Predicates.NATURAL_STONE, getRequiredState(GRAVEL_LOCATION), 33))
      .rangeUniform(absolute(0), top())
      .squared()
      .count(8);
  }

  public static get ORE_GRANITE() {
    return Features.ORE.configured(new OreConfiguration(OreConfiguration.Predicates.NATURAL_STONE, getRequiredState(GRANITE_LOCATION), 33))
      .rangeUniform(absolute(0), absolute(79))
      .squared()
      .count(10);
  }

  public static get ORE_DIORITE() {
    return Features.ORE.configured(new OreConfiguration(OreConfiguration.Predicates.NATURAL_STONE, getRequiredState(DIORITE_LOCATION), 33))
      .rangeUniform(absolute(0), absolute(79))
      .squared()
      .count(10);
  }

  public static get ORE_ANDESITE() {
    return Features.ORE.configured(new OreConfiguration(OreConfiguration.Predicates.NATURAL_STONE, getRequiredState(ANDESITE_LOCATION), 33))
      .rangeUniform(absolute(0), absolute(79))
      .squared()
      .count(10);
  }

  public static get ORE_TUFF() {
    return Features.ORE.configured(new OreConfiguration(OreConfiguration.Predicates.NATURAL_STONE, getRequiredState(TUFF_LOCATION), 33))
      .rangeUniform(absolute(0), absolute(16))
      .squared()
      .count(1);
  }

  public static get ORE_DEEPSLATE() {
    return Features.ORE.configured(new OreConfiguration(OreConfiguration.Predicates.NATURAL_STONE, getRequiredState(DEEPSLATE_LOCATION), 64))
      .rangeUniform(absolute(0), absolute(16))
      .squared()
      .count(2);
  }

  public static get ORE_IRON_TARGET_LIST(): readonly OreConfiguration.TargetBlockState[] {
    return [
      OreConfiguration.target(OreConfiguration.Predicates.STONE_ORE_REPLACEABLES, getRequiredState(IRON_ORE_LOCATION)),
      OreConfiguration.target(OreConfiguration.Predicates.DEEPSLATE_ORE_REPLACEABLES, getRequiredState(DEEPSLATE_IRON_ORE_LOCATION)),
    ];
  }

  public static get ORE_REDSTONE_TARGET_LIST(): readonly OreConfiguration.TargetBlockState[] {
    return [
      OreConfiguration.target(OreConfiguration.Predicates.STONE_ORE_REPLACEABLES, getRequiredState(REDSTONE_ORE_LOCATION)),
      OreConfiguration.target(OreConfiguration.Predicates.DEEPSLATE_ORE_REPLACEABLES, getRequiredState(DEEPSLATE_REDSTONE_ORE_LOCATION)),
    ];
  }

  public static get ORE_GOLD_TARGET_LIST(): readonly OreConfiguration.TargetBlockState[] {
    return [
      OreConfiguration.target(OreConfiguration.Predicates.STONE_ORE_REPLACEABLES, getRequiredState(GOLD_ORE_LOCATION)),
      OreConfiguration.target(OreConfiguration.Predicates.DEEPSLATE_ORE_REPLACEABLES, getRequiredState(DEEPSLATE_GOLD_ORE_LOCATION)),
    ];
  }

  public static get ORE_DIAMOND_TARGET_LIST(): readonly OreConfiguration.TargetBlockState[] {
    return [
      OreConfiguration.target(OreConfiguration.Predicates.STONE_ORE_REPLACEABLES, getRequiredState(DIAMOND_ORE_LOCATION)),
      OreConfiguration.target(OreConfiguration.Predicates.DEEPSLATE_ORE_REPLACEABLES, getRequiredState(DEEPSLATE_DIAMOND_ORE_LOCATION)),
    ];
  }

  public static get ORE_LAPIS_TARGET_LIST(): readonly OreConfiguration.TargetBlockState[] {
    return [
      OreConfiguration.target(OreConfiguration.Predicates.STONE_ORE_REPLACEABLES, getRequiredState(LAPIS_ORE_LOCATION)),
      OreConfiguration.target(OreConfiguration.Predicates.DEEPSLATE_ORE_REPLACEABLES, getRequiredState(DEEPSLATE_LAPIS_ORE_LOCATION)),
    ];
  }

  public static get ORE_EMERALD_TARGET_LIST(): readonly OreConfiguration.TargetBlockState[] {
    return [
      OreConfiguration.target(OreConfiguration.Predicates.STONE_ORE_REPLACEABLES, getRequiredState(EMERALD_ORE_LOCATION)),
      OreConfiguration.target(OreConfiguration.Predicates.DEEPSLATE_ORE_REPLACEABLES, getRequiredState(DEEPSLATE_EMERALD_ORE_LOCATION)),
    ];
  }

  public static get ORE_COPPER_TARGET_LIST(): readonly OreConfiguration.TargetBlockState[] {
    return [
      OreConfiguration.target(OreConfiguration.Predicates.STONE_ORE_REPLACEABLES, getRequiredState(COPPER_ORE_LOCATION)),
      OreConfiguration.target(OreConfiguration.Predicates.DEEPSLATE_ORE_REPLACEABLES, getRequiredState(DEEPSLATE_COPPER_ORE_LOCATION)),
    ];
  }

  public static get ORE_COAL_TARGET_LIST(): readonly OreConfiguration.TargetBlockState[] {
    return [
      OreConfiguration.target(OreConfiguration.Predicates.STONE_ORE_REPLACEABLES, getRequiredState(COAL_ORE_LOCATION)),
      OreConfiguration.target(OreConfiguration.Predicates.DEEPSLATE_ORE_REPLACEABLES, getRequiredState(DEEPSLATE_COAL_ORE_LOCATION)),
    ];
  }

  public static get ORE_INFESTED_TARGET_LIST(): readonly OreConfiguration.TargetBlockState[] {
    return [
      OreConfiguration.target(OreConfiguration.Predicates.STONE_ORE_REPLACEABLES, getRequiredState(INFESTED_STONE_LOCATION)),
      OreConfiguration.target(OreConfiguration.Predicates.DEEPSLATE_ORE_REPLACEABLES, getRequiredState(INFESTED_DEEPSLATE_LOCATION)),
    ];
  }

  public static get ORE_IRON_CONFIG(): OreConfiguration {
    return new OreConfiguration(OreFeatures.ORE_IRON_TARGET_LIST, 9);
  }

  public static get ORE_REDSTONE_CONFIG(): OreConfiguration {
    return new OreConfiguration(OreFeatures.ORE_REDSTONE_TARGET_LIST, 8);
  }

  public static get ORE_COAL() {
    return Features.ORE.configured(new OreConfiguration(OreFeatures.ORE_COAL_TARGET_LIST, 17))
      .rangeUniform(bottom(), absolute(127))
      .squared()
      .count(20);
  }

  public static get ORE_IRON() {
    return Features.ORE.configured(new OreConfiguration(OreFeatures.ORE_IRON_TARGET_LIST, 9))
      .rangeUniform(bottom(), absolute(63))
      .squared()
      .count(20);
  }

  public static get ORE_GOLD() {
    return Features.ORE.configured(new OreConfiguration(OreFeatures.ORE_GOLD_TARGET_LIST, 9))
      .rangeUniform(bottom(), absolute(31))
      .squared()
      .count(2);
  }

  public static get ORE_GOLD_EXTRA() {
    return Features.ORE.configured(new OreConfiguration(OreFeatures.ORE_GOLD_TARGET_LIST, 9))
      .rangeUniform(absolute(32), absolute(79))
      .squared()
      .count(20);
  }

  public static get ORE_REDSTONE() {
    return Features.ORE.configured(OreFeatures.ORE_REDSTONE_CONFIG)
      .rangeUniform(bottom(), absolute(15))
      .squared()
      .count(8);
  }

  public static get ORE_DIAMOND() {
    return Features.ORE.configured(new OreConfiguration(OreFeatures.ORE_DIAMOND_TARGET_LIST, 8))
      .rangeUniform(bottom(), absolute(15))
      .squared();
  }

  public static get ORE_LAPIS() {
    return Features.ORE.configured(new OreConfiguration(OreFeatures.ORE_LAPIS_TARGET_LIST, 7))
      .rangeTriangle(absolute(0), absolute(30))
      .squared();
  }

  public static get ORE_INFESTED() {
    return Features.ORE.configured(new OreConfiguration(OreFeatures.ORE_INFESTED_TARGET_LIST, 9))
      .rangeUniform(bottom(), absolute(63))
      .squared()
      .count(7);
  }

  public static get ORE_EMERALD() {
    return Features.REPLACE_SINGLE_BLOCK.configured(new ReplaceBlockConfiguration(OreFeatures.ORE_EMERALD_TARGET_LIST))
      .rangeUniform(absolute(4), absolute(31))
      .squared()
      .count(UniformInt.of(3, 8));
  }

  public static get ORE_COPPER() {
    return Features.ORE.configured(new OreConfiguration(OreFeatures.ORE_COPPER_TARGET_LIST, 10))
      .rangeTriangle(absolute(0), absolute(96))
      .squared()
      .count(6);
  }

  public static get RARE_DRIPSTONE_CLUSTER_FEATURE() {
    return Features.DRIPSTONE_CLUSTER.configured(
      new DripstoneClusterConfiguration(
        12,
        UniformInt.of(3, 3),
        UniformInt.of(2, 6),
        1,
        3,
        UniformInt.of(2, 2),
        UniformFloat.of(0.3, 0.4),
        ClampedNormalFloat.of(0.1, 0.3, 0.1, 0.9),
        0.1,
        3,
        8,
      ),
    )
      .rangeUniform(bottom(), absolute(59))
      .squared()
      .count(UniformInt.of(10, 10))
      .rarity(25);
  }

  public static get RARE_SMALL_DRIPSTONE_FEATURE() {
    return Features.SMALL_DRIPSTONE.configured(new SmallDripstoneConfiguration(5, 10, 2, 0.2))
      .rangeUniform(bottom(), absolute(59))
      .squared()
      .count(UniformInt.of(40, 80))
      .rarity(30);
  }

  public static get GLOW_LICHEN() {
    return Features.GLOW_LICHEN.configured(
      new GlowLichenConfiguration(20, false, true, true, 0.5, [
        getRequiredState(new ResourceLocation("minecraft:stone")),
        getRequiredState(ANDESITE_LOCATION),
        getRequiredState(DIORITE_LOCATION),
        getRequiredState(GRANITE_LOCATION),
        getRequiredState(DRIPSTONE_BLOCK_LOCATION),
        getRequiredState(CALCITE_LOCATION),
        getRequiredState(TUFF_LOCATION),
        getRequiredState(DEEPSLATE_LOCATION),
      ]),
    )
      .squared()
      .rangeUniform(bottom(), absolute(54))
      .count(UniformInt.of(20, 30));
  }
}
