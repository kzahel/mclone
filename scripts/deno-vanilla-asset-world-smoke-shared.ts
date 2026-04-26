import { Registry } from "../src/core/registry.ts";
import { ResourceLocation } from "../src/core/resource-location.ts";
import type { AssetPack } from "../src/renderer/assets/asset-pack.ts";
import { BlockColors } from "../src/renderer/block/block-colors.ts";
import { BlockRenderDispatcher } from "../src/renderer/block/block-render-dispatcher.ts";
import { BlockModelRepository } from "../src/renderer/model/block-model-repository.ts";
import { BlockModelShaper } from "../src/renderer/model/block-model-shaper.ts";
import { ModelBakery } from "../src/renderer/model/model-bakery.ts";
import { ModelManager } from "../src/renderer/model/model-manager.ts";
import { preloadBlockModelSource } from "../src/renderer/model/browser-block-model-source.ts";
import { MissingTextureAtlasSprite } from "../src/renderer/texture/missing-texture-atlas-sprite.ts";
import { TextureAtlas, type TextureAtlasPreparations } from "../src/renderer/texture/texture-atlas.ts";
import type { TextureAtlasSprite } from "../src/renderer/texture/texture-atlas-sprite.ts";
import type { Block } from "../src/world/level/block/block.ts";
import { registerGeneratedRenderBlocks } from "../src/world/level/generated-render-blocks.ts";
import { ChunkBlockId } from "../src/worldgen/chunk/chunk-block-buffer.ts";
import { DenoFileAssetPack, DenoFileTextureAtlasSource } from "./deno-file-asset-source.ts";

export const DENO_VANILLA_ASSET_WORLD_SEED = 12345n;
export const DENO_VANILLA_ASSET_WORLD_MIN_BUILD_HEIGHT = 0;
export const DENO_VANILLA_ASSET_WORLD_HEIGHT = 16;
export const DENO_VANILLA_ASSET_WORLD_VIEW_DISTANCE = 2;
export const DENO_VANILLA_ASSET_WORLD_TEXTURE_MIP_LEVEL = 0;
export const DENO_VANILLA_ASSET_WORLD_STONE_BLOCK = new ResourceLocation("minecraft:stone");
export const DENO_VANILLA_ASSET_WORLD_STONE_TEXTURE = new ResourceLocation("minecraft:block/stone");
export const DENO_VANILLA_ASSET_WORLD_BLOCKS = [DENO_VANILLA_ASSET_WORLD_STONE_BLOCK] as const;
export const DENO_VANILLA_ASSET_WORLD_SPRITES = [DENO_VANILLA_ASSET_WORLD_STONE_TEXTURE] as const;

export interface DenoVanillaAssetAtlasResources {
  readonly assetPack: DenoFileAssetPack;
  readonly atlasSource: DenoFileTextureAtlasSource;
  readonly preparations: TextureAtlasPreparations;
}

export async function prepareDenoVanillaAssetAtlasResources(
  atlas: TextureAtlas,
): Promise<DenoVanillaAssetAtlasResources> {
  const assetPack = new DenoFileAssetPack();
  assertRequiredAssets(assetPack);
  const atlasSource = new DenoFileTextureAtlasSource(assetPack);
  const preparations = await atlas.prepareToStitch(
    atlasSource,
    DENO_VANILLA_ASSET_WORLD_SPRITES,
    DENO_VANILLA_ASSET_WORLD_TEXTURE_MIP_LEVEL,
  );

  return { assetPack, atlasSource, preparations };
}

export async function createDenoVanillaAssetBlockRenderer(
  assetPack: AssetPack,
  blocks: ReturnType<typeof registerGeneratedRenderBlocks>,
  spriteLookup: (location: ResourceLocation) => TextureAtlasSprite,
): Promise<BlockRenderDispatcher> {
  const modelSource = await preloadBlockModelSource(assetPack, DENO_VANILLA_ASSET_WORLD_BLOCKS);
  const repository = new BlockModelRepository(modelSource);
  const bakery = new ModelBakery(repository, (material) => spriteLookup(material.texture()));
  const modelManager = new ModelManager(bakery.getMissingBakedModel());
  for (const blockLocation of DENO_VANILLA_ASSET_WORLD_BLOCKS) {
    const block = Registry.BLOCK.get(blockLocation) as Block | undefined;
    if (block === undefined) {
      throw new Error(`Deno vanilla asset smoke block is not registered: ${blockLocation.toString()}`);
    }

    for (const state of block.getStateDefinition().getPossibleStates()) {
      const modelLocation = BlockModelShaper.stateToModelLocation(state);
      modelManager.setModel(modelLocation, bakery.bake(modelLocation));
    }
  }

  const shaper = new BlockModelShaper(modelManager);
  shaper.rebuildCache();
  return new BlockRenderDispatcher(
    shaper,
    BlockColors.createDefault(),
    spriteLookup,
    blocks.blockStateById[ChunkBlockId.WATER]!,
    blocks.blockStateById[ChunkBlockId.LAVA]!,
  );
}

export function createSpriteLookup(sprites: readonly TextureAtlasSprite[]): (location: ResourceLocation) => TextureAtlasSprite {
  const spritesByName = new Map(sprites.map((sprite) => [sprite.getName().toString(), sprite] as const));
  const missing = spritesByName.get(MissingTextureAtlasSprite.getLocation().toString());
  if (missing === undefined) {
    throw new Error("Missing texture atlas sprite was not loaded for Deno vanilla asset smoke");
  }

  return (location) => spritesByName.get(location.toString()) ?? missing;
}

function assertRequiredAssets(assetPack: DenoFileAssetPack): void {
  for (const path of [
    "assets/minecraft/blockstates/stone.json",
    "assets/minecraft/models/block/stone.json",
    "assets/minecraft/models/block/stone_mirrored.json",
    "assets/minecraft/models/block/cube.json",
    "assets/minecraft/models/block/cube_all.json",
    "assets/minecraft/models/block/cube_mirrored.json",
    "assets/minecraft/models/block/cube_mirrored_all.json",
    "assets/minecraft/models/block/block.json",
    "assets/minecraft/textures/block/stone.png",
  ]) {
    if (!assetPack.has(path)) {
      throw new Error(`Deno vanilla asset smoke requires extracted Minecraft asset ${path}`);
    }
  }
}
