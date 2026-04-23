import { BlockPos } from "../core/block-pos";
import type { BlockAndTintGetter } from "../world/level/block-and-tint-getter";
import type { ColorResolver } from "../world/level/color-resolver";
import type { Biome } from "../worldgen/biome/biome";

const GRASS_COLOR_RESOLVER: ColorResolver = {
  getColor(biome: Biome, x: number, z: number): number {
    return biome.getGrassColor(x, z);
  },
};

const FOLIAGE_COLOR_RESOLVER: ColorResolver = {
  getColor(biome: Biome): number {
    return biome.getFoliageColor();
  },
};

const WATER_COLOR_RESOLVER: ColorResolver = {
  getColor(biome: Biome): number {
    return biome.getWaterColor();
  },
};

function getAverageColor(level: BlockAndTintGetter, pos: BlockPos, resolver: ColorResolver): number {
  return level.getBlockTint(pos, resolver);
}

export class BiomeColors {
  public static getAverageGrassColor(level: BlockAndTintGetter, pos: BlockPos): number {
    return getAverageColor(level, pos, GRASS_COLOR_RESOLVER);
  }

  public static getAverageFoliageColor(level: BlockAndTintGetter, pos: BlockPos): number {
    return getAverageColor(level, pos, FOLIAGE_COLOR_RESOLVER);
  }

  public static getAverageWaterColor(level: BlockAndTintGetter, pos: BlockPos): number {
    return getAverageColor(level, pos, WATER_COLOR_RESOLVER);
  }
}
