import { ResourceLocation } from "../../../core/resource-location";
import type { BlockPos } from "../../../core/block-pos";
import type { WorldGenLevel } from "../../level/world-gen-level";
import { LightLayer } from "../../level/light-layer";

const GRASS_BLOCK_LOCATION = new ResourceLocation("minecraft:grass_block");
const SAND_LOCATION = new ResourceLocation("minecraft:sand");
const SNOW_LOCATION = new ResourceLocation("minecraft:snow");
const ICE_LOCATION = new ResourceLocation("minecraft:ice");
const MYCELIUM_LOCATION = new ResourceLocation("minecraft:mycelium");

export function checkAnimalSpawnRules(level: WorldGenLevel, pos: BlockPos): boolean {
  return isBlock(level, pos.below(), GRASS_BLOCK_LOCATION) && getRawBrightness(level, pos) > 8;
}

export function checkRabbitSpawnRules(level: WorldGenLevel, pos: BlockPos): boolean {
  const below = pos.below();
  return (
    isBlock(level, below, GRASS_BLOCK_LOCATION) ||
    isBlock(level, below, SNOW_LOCATION) ||
    isBlock(level, below, SAND_LOCATION)
  ) && getRawBrightness(level, pos) > 8;
}

export function checkMushroomSpawnRules(level: WorldGenLevel, pos: BlockPos): boolean {
  return isBlock(level, pos.below(), MYCELIUM_LOCATION) && getRawBrightness(level, pos) > 8;
}

export function checkPolarBearSpawnRules(level: WorldGenLevel, pos: BlockPos): boolean {
  const biomeKey = level.getBiome(pos).getKey();
  if (biomeKey !== "minecraft:frozen_ocean" && biomeKey !== "minecraft:deep_frozen_ocean") {
    return checkAnimalSpawnRules(level, pos);
  }

  return isBlock(level, pos.below(), ICE_LOCATION) && getRawBrightness(level, pos) > 8;
}

export function getRawBrightness(level: WorldGenLevel, pos: BlockPos): number {
  return Math.max(level.getBrightness(LightLayer.SKY, pos), level.getBrightness(LightLayer.BLOCK, pos));
}

export function isBlock(level: WorldGenLevel, pos: BlockPos, location: ResourceLocation): boolean {
  return level.getBlockState(pos).getBlock().getLocation()?.equals(location) ?? false;
}
