import { HugeMushroomBlock } from "../../../world/level/block/huge-mushroom-block";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import { BlockPos } from "../../../core/block-pos";
import { AbstractHugeMushroomFeature } from "./abstract-huge-mushroom-feature";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import type { HugeMushroomFeatureConfiguration } from "./configurations/huge-mushroom-feature-configuration";

export class HugeRedMushroomFeature extends AbstractHugeMushroomFeature {
  protected override makeCap(
    level: WorldGenLevel,
    random: SimpleRandomSource,
    pos: BlockPos,
    height: number,
    mutablePos: BlockPos.MutableBlockPos,
    config: HugeMushroomFeatureConfiguration,
  ): void {
    for (let layerY = height - 3; layerY <= height; layerY++) {
      const radius = layerY < height ? config.foliageRadius : config.foliageRadius - 1;
      const innerRadius = config.foliageRadius - 2;

      for (let dx = -radius; dx <= radius; dx++) {
        for (let dz = -radius; dz <= radius; dz++) {
          const isWestEdge = dx === -radius;
          const isEastEdge = dx === radius;
          const isNorthEdge = dz === -radius;
          const isSouthEdge = dz === radius;
          const edgeX = isWestEdge || isEastEdge;
          const edgeZ = isNorthEdge || isSouthEdge;
          if (layerY < height && edgeX === edgeZ) {
            continue;
          }

          mutablePos.setWithOffset(pos, dx, layerY, dz);
          if (!level.getBlockState(mutablePos).isSolidRender(level, mutablePos)) {
            let state = config.capProvider.getState(random, pos);
            if (
              AbstractHugeMushroomFeature.hasDirectionalProperties(state) &&
              state.hasProperty(HugeMushroomBlock.UP)
            ) {
              state = state
                .setValue(HugeMushroomBlock.UP, layerY >= height - 1)
                .setValue(HugeMushroomBlock.WEST, dx < -innerRadius)
                .setValue(HugeMushroomBlock.EAST, dx > innerRadius)
                .setValue(HugeMushroomBlock.NORTH, dz < -innerRadius)
                .setValue(HugeMushroomBlock.SOUTH, dz > innerRadius);
            }

            this.setBlock(level, mutablePos, state);
          }
        }
      }
    }
  }

  protected override getTreeRadiusForHeight(_totalHeight: number, capTopY: number, foliageRadius: number, layerY: number): number {
    let radius = 0;
    if (layerY < capTopY && layerY >= capTopY - 3) {
      radius = foliageRadius;
    } else if (layerY === capTopY) {
      radius = foliageRadius;
    }

    return radius;
  }
}
