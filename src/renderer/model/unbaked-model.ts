import { ResourceLocation } from "../../core/resource-location";
import { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { type BakedModel } from "./baked-model";
import { type Material } from "./material";
import { type ModelBakery } from "./model-bakery";
import { type ModelState } from "./model-state";

export interface UnbakedModel {
  getDependencies(): readonly ResourceLocation[];

  bake(
    bakery: ModelBakery,
    spriteGetter: (material: Material) => TextureAtlasSprite,
    modelState: ModelState,
    location: ResourceLocation,
  ): BakedModel;
}
