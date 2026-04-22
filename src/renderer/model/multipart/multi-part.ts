import { ResourceLocation } from "../../../core/resource-location";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import type { StateDefinition } from "../../../world/level/block/state/state-definition";
import { TextureAtlasSprite } from "../../texture/texture-atlas-sprite";
import { type BakedModel } from "../baked-model";
import { type Material } from "../material";
import { type ModelBakery } from "../model-bakery";
import { type ModelState } from "../model-state";
import { MultiPartBakedModel } from "../multi-part-baked-model";
import { MultiVariant } from "../multi-variant";
import { type UnbakedModel } from "../unbaked-model";
import { Selector } from "./selector";

export class MultiPart implements UnbakedModel {
  public constructor(
    private readonly definition: StateDefinition<Block, BlockState>,
    private readonly selectors: readonly Selector[],
  ) {}

  public static fromJson(definition: StateDefinition<Block, BlockState>, value: unknown): MultiPart {
    if (!Array.isArray(value)) {
      throw new Error("Expected multipart to be an array");
    }

    return new MultiPart(definition, value.map((entry) => Selector.fromJson(entry)));
  }

  public getSelectors(): readonly Selector[] {
    return this.selectors;
  }

  public getMultiVariants(): Set<MultiVariant> {
    const variants = new Set<MultiVariant>();
    for (const selector of this.selectors) {
      variants.add(selector.getVariant());
    }

    return variants;
  }

  public getDependencies(): readonly ResourceLocation[] {
    const dependencies = new Map<string, ResourceLocation>();
    for (const selector of this.selectors) {
      for (const dependency of selector.getVariant().getDependencies()) {
        dependencies.set(dependency.toString(), dependency);
      }
    }

    return [...dependencies.values()];
  }

  public bake(
    bakery: ModelBakery,
    spriteGetter: (material: Material) => TextureAtlasSprite,
    modelState: ModelState,
    location: ResourceLocation,
  ): BakedModel {
    const builder = new MultiPartBakedModel.Builder();
    for (const selector of this.selectors) {
      const bakedModel = selector.getVariant().bake(bakery, spriteGetter, modelState, location);
      builder.add(selector.getPredicate(this.definition), bakedModel);
    }

    return builder.build();
  }
}
