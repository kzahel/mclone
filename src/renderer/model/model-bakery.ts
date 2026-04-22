import { ResourceLocation } from "../../core/resource-location";
import { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { BLOCK_ENTITY_MARKER, BlockModel, GENERATION_MARKER } from "./block-model";
import { BlockModelRepository } from "./block-model-repository";
import { type BakedModel } from "./baked-model";
import { BlockModelRotation } from "./block-model-rotation";
import { type Material } from "./material";
import { type ModelState } from "./model-state";
import { ModelResourceLocation } from "./model-resource-location";

function cacheKey(location: ResourceLocation, modelState: ModelState): string {
  const stateKey = modelState instanceof BlockModelRotation ? modelState.getIndexValue().toString() : "custom";
  return `${location.toString()}|${stateKey}|${modelState.isUvLocked() ? "uv" : "nouv"}`;
}

export class ModelBakery {
  public static readonly MISSING_MODEL_LOCATION = new ModelResourceLocation(new ResourceLocation("builtin/missing"), "missing");

  private readonly bakedCache = new Map<string, BakedModel>();

  public constructor(
    private readonly repository: BlockModelRepository,
    private readonly spriteGetter: (material: Material) => TextureAtlasSprite,
  ) {}

  public bake(location: ResourceLocation, modelState: ModelState = BlockModelRotation.X0_Y0): BakedModel {
    const key = cacheKey(location, modelState);
    const cached = this.bakedCache.get(key);
    if (cached !== undefined) {
      return cached;
    }

    const model = this.repository.resolveBlockModel(location);
    if (model.getRootModel() === GENERATION_MARKER) {
      throw new Error("ItemModelGenerator is not implemented yet");
    }

    const bakedModel = model.bake(this, this.spriteGetter, modelState, location);
    this.bakedCache.set(key, bakedModel);
    return bakedModel;
  }

  public getMissingBakedModel(): BakedModel {
    return this.bake(new ResourceLocation("builtin/missing"));
  }

  public isBlockEntityMarker(model: BlockModel): boolean {
    return model.getRootModel() === BLOCK_ENTITY_MARKER;
  }
}
