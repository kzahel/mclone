import { Registry } from "../../core/registry";
import { ResourceLocation } from "../../core/resource-location";
import type { Block } from "../../world/level/block/block";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { Property } from "../../world/level/block/state/properties/property";
import { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { type BakedModel } from "./baked-model";
import { ModelManager } from "./model-manager";
import { ModelResourceLocation } from "./model-resource-location";

export class BlockModelShaper {
  private readonly modelByStateCache = new Map<BlockState, BakedModel>();

  public constructor(private readonly modelManager: ModelManager) {}

  public getParticleIcon(state: BlockState): TextureAtlasSprite {
    return this.getBlockModel(state).getParticleIcon();
  }

  public getBlockModel(state: BlockState): BakedModel {
    return this.modelByStateCache.get(state) ?? this.modelManager.getMissingModel();
  }

  public getModelManager(): ModelManager {
    return this.modelManager;
  }

  public rebuildCache(): void {
    this.modelByStateCache.clear();
    for (const block of Registry.BLOCK as Iterable<object>) {
      const typedBlock = block as Block;
      typedBlock.getStateDefinition()
        .getPossibleStates()
        .forEach((state) => this.modelByStateCache.set(state, this.modelManager.getModel(BlockModelShaper.stateToModelLocation(state))));
    }
  }

  public static stateToModelLocation(state: BlockState): ModelResourceLocation {
    const location = Registry.BLOCK.getKey(state.getBlock() as unknown as object);
    if (location === undefined) {
      throw new Error(`Block ${state.getBlock()} is not registered`);
    }

    return BlockModelShaper.stateToModelLocationFromKey(location, state);
  }

  public static stateToModelLocationFromKey(location: ResourceLocation, state: BlockState): ModelResourceLocation {
    return new ModelResourceLocation(location, BlockModelShaper.statePropertiesToString(state.getValues()));
  }

  public static statePropertiesToString(values: ReadonlyMap<Property<unknown>, unknown>): string {
    let result = "";
    for (const [property, value] of values.entries()) {
      if (result.length !== 0) {
        result += ",";
      }

      result += `${property.getName()}=${property.getNameForValue(value)}`;
    }

    return result;
  }
}
