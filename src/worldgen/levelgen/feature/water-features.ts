import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import { belowTop, biasedToBottomHeight, bottom, top, uniformHeight } from "../../carver/carver-config";
import type { Block } from "../../../world/level/block/block";
import { Fluids } from "../../../world/level/material/fluids";
import { Features } from "./features";
import { BlockStateConfiguration } from "./configurations/block-state-configuration";
import { RangeDecoratorConfiguration } from "./configurations/range-decorator-configuration";
import { SpringConfiguration } from "./configurations/spring-configuration";
import { FeatureDecorators } from "../placement/feature-decorators";

const WATER_LOCATION = new ResourceLocation("minecraft:water");
const STONE_LOCATION = new ResourceLocation("minecraft:stone");
const GRANITE_LOCATION = new ResourceLocation("minecraft:granite");
const DIORITE_LOCATION = new ResourceLocation("minecraft:diorite");
const ANDESITE_LOCATION = new ResourceLocation("minecraft:andesite");

function getRequiredBlock(location: ResourceLocation): Block {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block;
}

export class WaterFeatures {
  public static get LAKE_WATER() {
    return Features.LAKE.configured(new BlockStateConfiguration(getRequiredBlock(WATER_LOCATION).defaultBlockState()))
      .decorated(FeatureDecorators.RANGE.configured(new RangeDecoratorConfiguration(uniformHeight(bottom(), top()))))
      .squared()
      .rarity(4);
  }

  public static get SPRING_WATER() {
    return Features.SPRING.configured(
      new SpringConfiguration(
        Fluids.WATER.defaultFluidState(),
        true,
        4,
        1,
        new Set([
          getRequiredBlock(STONE_LOCATION),
          getRequiredBlock(GRANITE_LOCATION),
          getRequiredBlock(DIORITE_LOCATION),
          getRequiredBlock(ANDESITE_LOCATION),
        ]),
      ),
    )
      .decorated(
        FeatureDecorators.RANGE.configured(
          new RangeDecoratorConfiguration(biasedToBottomHeight(bottom(), belowTop(8), 8)),
        ),
      )
      .squared()
      .count(50);
  }
}
