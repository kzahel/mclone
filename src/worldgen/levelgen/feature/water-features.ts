import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import { belowTop, biasedToBottomHeight, bottom, top, uniformHeight, veryBiasedToBottomHeight } from "../../carver/carver-config";
import type { Block } from "../../../world/level/block/block";
import { Fluids } from "../../../world/level/material/fluids";
import { Features } from "./features";
import { BlockStateConfiguration } from "./configurations/block-state-configuration";
import { ChanceDecoratorConfiguration } from "./configurations/chance-decorator-configuration";
import { RangeDecoratorConfiguration } from "./configurations/range-decorator-configuration";
import { SpringConfiguration } from "./configurations/spring-configuration";
import { FeatureDecorators } from "../placement/feature-decorators";

const WATER_LOCATION = new ResourceLocation("minecraft:water");
const LAVA_LOCATION = new ResourceLocation("minecraft:lava");
const STONE_LOCATION = new ResourceLocation("minecraft:stone");
const GRANITE_LOCATION = new ResourceLocation("minecraft:granite");
const DIORITE_LOCATION = new ResourceLocation("minecraft:diorite");
const ANDESITE_LOCATION = new ResourceLocation("minecraft:andesite");
const DEEPSLATE_LOCATION = new ResourceLocation("minecraft:deepslate");
const TUFF_LOCATION = new ResourceLocation("minecraft:tuff");

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

  public static get LAKE_LAVA() {
    return Features.LAKE.configured(new BlockStateConfiguration(getRequiredBlock(LAVA_LOCATION).defaultBlockState()))
      .decorated(FeatureDecorators.LAVA_LAKE.configured(new ChanceDecoratorConfiguration(80)))
      .decorated(FeatureDecorators.RANGE.configured(new RangeDecoratorConfiguration(biasedToBottomHeight(bottom(), top(), 8))))
      .squared()
      .rarity(8);
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

  public static get SPRING_LAVA() {
    return Features.SPRING.configured(
      new SpringConfiguration(
        Fluids.LAVA.defaultFluidState(),
        true,
        4,
        1,
        new Set([
          getRequiredBlock(STONE_LOCATION),
          getRequiredBlock(GRANITE_LOCATION),
          getRequiredBlock(DIORITE_LOCATION),
          getRequiredBlock(ANDESITE_LOCATION),
          getRequiredBlock(DEEPSLATE_LOCATION),
          getRequiredBlock(TUFF_LOCATION),
        ]),
      ),
    )
      .decorated(
        FeatureDecorators.RANGE.configured(
          new RangeDecoratorConfiguration(veryBiasedToBottomHeight(bottom(), belowTop(8), 8)),
        ),
      )
      .squared()
      .count(20);
  }
}
