import { HugeMushroomBlock } from "../../../world/level/block/huge-mushroom-block";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import { BlockPos } from "../../../core/block-pos";
import { AbstractHugeMushroomFeature } from "./abstract-huge-mushroom-feature";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import type { HugeMushroomFeatureConfiguration } from "./configurations/huge-mushroom-feature-configuration";

export class HugeBrownMushroomFeature extends AbstractHugeMushroomFeature {
  protected override makeCap(
    level: WorldGenLevel,
    random: SimpleRandomSource,
    pos: BlockPos,
    height: number,
    mutablePos: BlockPos.MutableBlockPos,
    config: HugeMushroomFeatureConfiguration,
  ): void {
    const radius = config.foliageRadius;

    for (let dx = -radius; dx <= radius; dx++) {
      for (let dz = -radius; dz <= radius; dz++) {
        const isWestEdge = dx === -radius;
        const isEastEdge = dx === radius;
        const isNorthEdge = dz === -radius;
        const isSouthEdge = dz === radius;
        const edgeX = isWestEdge || isEastEdge;
        const edgeZ = isNorthEdge || isSouthEdge;
        if (edgeX && edgeZ) {
          continue;
        }

        mutablePos.setWithOffset(pos, dx, height, dz);
        if (!level.getBlockState(mutablePos).isSolidRender(level, mutablePos)) {
          const west = isWestEdge || (edgeZ && dx === 1 - radius);
          const east = isEastEdge || (edgeZ && dx === radius - 1);
          const north = isNorthEdge || (edgeX && dz === 1 - radius);
          const south = isSouthEdge || (edgeX && dz === radius - 1);
          let state = config.capProvider.getState(random, pos);
          if (AbstractHugeMushroomFeature.hasDirectionalProperties(state)) {
            state = state
              .setValue(HugeMushroomBlock.WEST, west)
              .setValue(HugeMushroomBlock.EAST, east)
              .setValue(HugeMushroomBlock.NORTH, north)
              .setValue(HugeMushroomBlock.SOUTH, south);
          }

          this.setBlock(level, mutablePos, state);
        }
      }
    }
  }

  protected override getTreeRadiusForHeight(_totalHeight: number, _capTopY: number, foliageRadius: number, layerY: number): number {
    return layerY <= 3 ? 0 : foliageRadius;
  }
}
