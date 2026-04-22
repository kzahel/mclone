import { ResourceLocation } from "../../core/resource-location";
import { Registry } from "../../core/registry";
import type { Block } from "../../world/level/block/block";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { Property } from "../../world/level/block/state/properties/property";
import type { StateDefinition } from "../../world/level/block/state/state-definition";
import { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { BlockModelDefinition } from "./block-model-definition";
import { BLOCK_ENTITY_MARKER, BlockModel, GENERATION_MARKER } from "./block-model";
import { BlockModelRepository } from "./block-model-repository";
import { type BakedModel } from "./baked-model";
import { BlockModelRotation } from "./block-model-rotation";
import { BlockModelShaper } from "./block-model-shaper";
import { type Material } from "./material";
import { type ModelState } from "./model-state";
import { ModelManager } from "./model-manager";
import { ModelResourceLocation } from "./model-resource-location";
import { type UnbakedModel } from "./unbaked-model";

function cacheKey(location: ResourceLocation, modelState: ModelState): string {
  if (modelState instanceof BlockModelRotation) {
    return `${location.toString()}|${modelState.getIndexValue()}|${modelState.isUvLocked() ? "uv" : "nouv"}`;
  }

  const matrix = modelState.getRotation().getMatrix();
  return `${location.toString()}|${[
    matrix.m00,
    matrix.m01,
    matrix.m02,
    matrix.m03,
    matrix.m10,
    matrix.m11,
    matrix.m12,
    matrix.m13,
    matrix.m20,
    matrix.m21,
    matrix.m22,
    matrix.m23,
    matrix.m30,
    matrix.m31,
    matrix.m32,
    matrix.m33,
  ].join(",")}|${modelState.isUvLocked() ? "uv" : "nouv"}`;
}

function stateMatchesVariant(definition: StateDefinition<Block, BlockState>, variant: string): (state: BlockState) => boolean {
  if (variant.length === 0) {
    return () => true;
  }

  const expectedValues = new Map<Property<unknown>, unknown>();
  for (const entry of variant.split(",")) {
    if (entry.length === 0) {
      continue;
    }

    const [propertyName, valueName, ...rest] = entry.split("=");
    if (propertyName === undefined || valueName === undefined || rest.length !== 0) {
      throw new Error(`Invalid blockstate variant '${variant}'`);
    }

    const property = definition.getProperty(propertyName);
    if (property === undefined) {
      throw new Error(`Unknown blockstate property: '${propertyName}'`);
    }

    const value = property.getValue(valueName);
    if (value === undefined) {
      throw new Error(`Unknown value: '${valueName}' for blockstate property: '${propertyName}' ${property.getPossibleValues()}`);
    }

    expectedValues.set(property, value);
  }

  const block = definition.getOwner();
  return (state) => {
    if (state.getBlock() !== block) {
      return false;
    }

    for (const [property, value] of expectedValues.entries()) {
      if (state.getValue(property) !== value) {
        return false;
      }
    }

    return true;
  };
}

export class ModelBakery {
  public static readonly MISSING_MODEL_LOCATION = new ModelResourceLocation(new ResourceLocation("builtin/missing"), "missing");

  private readonly unbakedCache = new Map<string, UnbakedModel>();
  private readonly bakedCache = new Map<string, BakedModel>();
  private readonly loadingStack = new Set<string>();
  private readonly context = new BlockModelDefinition.Context();

  public constructor(
    private readonly repository: BlockModelRepository,
    private readonly spriteGetter: (material: Material) => TextureAtlasSprite,
  ) {
    this.unbakedCache.set(
      ModelBakery.MISSING_MODEL_LOCATION.toString(),
      this.repository.resolveBlockModel(new ResourceLocation("builtin/missing")),
    );
  }

  public bake(location: ResourceLocation, modelState: ModelState = BlockModelRotation.X0_Y0): BakedModel {
    const key = cacheKey(location, modelState);
    const cached = this.bakedCache.get(key);
    if (cached !== undefined) {
      return cached;
    }

    const model = this.getModel(location);
    if (model instanceof BlockModel && model.getRootModel() === GENERATION_MARKER) {
      throw new Error("ItemModelGenerator is not implemented yet");
    }

    const bakedModel = model.bake(this, this.spriteGetter, modelState, location);
    this.bakedCache.set(key, bakedModel);
    return bakedModel;
  }

  public getMissingBakedModel(): BakedModel {
    return this.bake(new ResourceLocation("builtin/missing"));
  }

  public getModel(location: ResourceLocation): UnbakedModel {
    const key = location.toString();
    const cached = this.unbakedCache.get(key);
    if (cached !== undefined) {
      return cached;
    }

    if (this.loadingStack.has(key)) {
      throw new Error(`Circular reference while loading ${location}`);
    }

    this.loadingStack.add(key);
    try {
      const model = this.loadModel(location);
      this.unbakedCache.set(key, model);
      return model;
    } catch (error) {
      if (key === ModelBakery.MISSING_MODEL_LOCATION.toString()) {
        throw error;
      }

      return this.unbakedCache.get(ModelBakery.MISSING_MODEL_LOCATION.toString())!;
    } finally {
      this.loadingStack.delete(key);
    }
  }

  private loadModel(location: ResourceLocation): UnbakedModel {
    if (!(location instanceof ModelResourceLocation)) {
      return this.repository.resolveBlockModel(location);
    }

    if (location.getVariant() === "inventory") {
      return this.repository.resolveBlockModel(new ResourceLocation(location.getNamespace(), `item/${location.getPath()}`));
    }

    const blockLocation = new ResourceLocation(location.getNamespace(), location.getPath());
    const block = Registry.BLOCK.get(blockLocation) as Block | undefined;
    if (block === undefined) {
      throw new Error(`Unknown block ${blockLocation}`);
    }

    const definition = block.getStateDefinition();
    this.context.setDefinition(definition);
    const blockStateJson = this.repository.getBlockStateJson(blockLocation);
    if (blockStateJson === undefined) {
      throw new Error(`Missing blockstate ${blockLocation}`);
    }

    const blockModelDefinition = BlockModelDefinition.fromString(this.context, blockStateJson);
    const stateLocations = new Map<string, BlockState>();
    const modelsByState = new Map<BlockState, UnbakedModel | undefined>();
    for (const state of definition.getPossibleStates()) {
      stateLocations.set(BlockModelShaper.stateToModelLocationFromKey(blockLocation, state).toString(), state);
    }

    const multiPart = blockModelDefinition.isMultiPart() ? blockModelDefinition.getMultiPart() : undefined;
    if (multiPart !== undefined) {
      for (const state of definition.getPossibleStates()) {
        modelsByState.set(state, multiPart);
      }
    }

    for (const [variantKey, variant] of blockModelDefinition.getVariants().entries()) {
      const predicate = stateMatchesVariant(definition, variantKey);
      for (const state of definition.getPossibleStates()) {
        if (!predicate(state)) {
          continue;
        }

        const previous = modelsByState.get(state);
        if (previous !== undefined && previous !== multiPart) {
          throw new Error(`Overlapping definition with: ${variantKey}`);
        }

        modelsByState.set(state, variant);
      }
    }

    const missingModel = this.unbakedCache.get(ModelBakery.MISSING_MODEL_LOCATION.toString())!;
    for (const [stateLocation, state] of stateLocations.entries()) {
      this.unbakedCache.set(stateLocation, modelsByState.get(state) ?? missingModel);
    }

    return this.unbakedCache.get(location.toString()) ?? missingModel;
  }

  public bakeTopLevelBlockModels(modelManager: ModelManager): void {
    for (const block of Registry.BLOCK as Iterable<object>) {
      const typedBlock = block as Block;
      for (const state of typedBlock.getStateDefinition().getPossibleStates()) {
        const location = BlockModelShaper.stateToModelLocation(state);
        modelManager.setModel(location, this.bake(location, BlockModelRotation.X0_Y0));
      }
    }
  }

  public isBlockEntityMarker(model: BlockModel): boolean {
    return model.getRootModel() === BLOCK_ENTITY_MARKER;
  }
}
