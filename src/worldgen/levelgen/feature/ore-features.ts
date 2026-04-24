import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import { absolute, bottom, top } from "../../carver/carver-config";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { OreConfiguration } from "./configurations/ore-configuration";
import { Features } from "./features";

const DIRT_LOCATION = new ResourceLocation("minecraft:dirt");
const GRAVEL_LOCATION = new ResourceLocation("minecraft:gravel");
const GRANITE_LOCATION = new ResourceLocation("minecraft:granite");
const DIORITE_LOCATION = new ResourceLocation("minecraft:diorite");
const ANDESITE_LOCATION = new ResourceLocation("minecraft:andesite");
const TUFF_LOCATION = new ResourceLocation("minecraft:tuff");
const DEEPSLATE_LOCATION = new ResourceLocation("minecraft:deepslate");
const COAL_ORE_LOCATION = new ResourceLocation("minecraft:coal_ore");
const DEEPSLATE_COAL_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_coal_ore");
const IRON_ORE_LOCATION = new ResourceLocation("minecraft:iron_ore");
const DEEPSLATE_IRON_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_iron_ore");
const GOLD_ORE_LOCATION = new ResourceLocation("minecraft:gold_ore");
const DEEPSLATE_GOLD_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_gold_ore");
const REDSTONE_ORE_LOCATION = new ResourceLocation("minecraft:redstone_ore");
const DEEPSLATE_REDSTONE_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_redstone_ore");
const DIAMOND_ORE_LOCATION = new ResourceLocation("minecraft:diamond_ore");
const DEEPSLATE_DIAMOND_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_diamond_ore");
const LAPIS_ORE_LOCATION = new ResourceLocation("minecraft:lapis_ore");
const DEEPSLATE_LAPIS_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_lapis_ore");
const COPPER_ORE_LOCATION = new ResourceLocation("minecraft:copper_ore");
const DEEPSLATE_COPPER_ORE_LOCATION = new ResourceLocation("minecraft:deepslate_copper_ore");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

export class OreFeatures {
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

  public static get ORE_COPPER() {
    return Features.ORE.configured(new OreConfiguration(OreFeatures.ORE_COPPER_TARGET_LIST, 10))
      .rangeTriangle(absolute(0), absolute(96))
      .squared()
      .count(6);
  }
}
