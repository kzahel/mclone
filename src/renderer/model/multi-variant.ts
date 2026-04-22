import { ResourceLocation } from "../../core/resource-location";
import { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { type BakedModel } from "./baked-model";
import { type Material } from "./material";
import { type ModelBakery } from "./model-bakery";
import { type ModelState } from "./model-state";
import { type UnbakedModel } from "./unbaked-model";
import { Variant } from "./variant";
import { WeightedBakedModel } from "./weighted-baked-model";

export class MultiVariant implements UnbakedModel {
  public constructor(private readonly variants: readonly Variant[]) {}

  public static fromJson(value: unknown): MultiVariant {
    if (Array.isArray(value)) {
      if (value.length === 0) {
        throw new Error("Empty variant array");
      }

      return new MultiVariant(value.map((entry) => Variant.fromJson(entry)));
    }

    return new MultiVariant([Variant.fromJson(value)]);
  }

  public getVariants(): readonly Variant[] {
    return this.variants;
  }

  public getDependencies(): readonly ResourceLocation[] {
    const seen = new Set<string>();
    const dependencies: ResourceLocation[] = [];
    for (const variant of this.variants) {
      const location = variant.getModelLocation();
      const key = location.toString();
      if (!seen.has(key)) {
        seen.add(key);
        dependencies.push(location);
      }
    }

    return dependencies;
  }

  public bake(
    bakery: ModelBakery,
    _spriteGetter: (material: Material) => TextureAtlasSprite,
    _modelState: ModelState,
    _location: ResourceLocation,
  ): BakedModel {
    if (this.variants.length === 0) {
      throw new Error("Cannot bake MultiVariant with no variants");
    }

    const builder = new WeightedBakedModel.Builder();
    for (const variant of this.variants) {
      builder.add(bakery.bake(variant.getModelLocation(), variant), variant.getWeight());
    }

    const bakedModel = builder.build();
    if (bakedModel === undefined) {
      throw new Error("Unable to bake MultiVariant");
    }

    return bakedModel;
  }
}
