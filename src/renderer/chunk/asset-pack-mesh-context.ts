import { ResourceLocation } from "../../core/resource-location";
import type { AssetPack } from "../assets/asset-pack";
import { BlockColors } from "../block/block-colors";
import { BlockRenderDispatcher } from "../block/block-render-dispatcher";
import { BlockModelRepository } from "../model/block-model-repository";
import { BlockModelShaper } from "../model/block-model-shaper";
import { preloadBlockModelSource } from "../model/browser-block-model-source";
import { ModelBakery } from "../model/model-bakery";
import { ModelManager } from "../model/model-manager";
import { AssetPackTextureAtlasSource } from "../texture/asset-pack-texture-atlas-source";
import { DEFAULT_BLOCK_ATLAS_MIP_LEVEL, TextureAtlas, type TextureAtlasPreparations } from "../texture/texture-atlas";
import { MissingTextureAtlasSprite } from "../texture/missing-texture-atlas-sprite";
import type { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import type { LoadingProgressSink } from "../loading-progress";
import { ClientChunkCache } from "../../world/level/client-chunk-cache";
import { createBlockStateResolver } from "../../world/level/chunk-snapshot";
import { FoliageColor } from "../../world/level/foliage-color";
import { registerGeneratedRenderBlocks, type GeneratedRenderBlockPalette } from "../../world/level/generated-render-blocks";
import { GrassColor } from "../../world/level/grass-color";
import { OverworldBiomeSource } from "../../worldgen/biome/overworld-biome-source";
import { ChunkBlockId } from "../../worldgen/chunk/chunk-block-buffer";
import type { InitializeRenderWorldRequest } from "./render-world-protocol";
import type { RenderWorldWorkerContext } from "./render-world-worker-handler";

const GRASS_COLORMAP_LOCATION = new ResourceLocation("minecraft:colormap/grass");
const FOLIAGE_COLORMAP_LOCATION = new ResourceLocation("minecraft:colormap/foliage");

export interface AssetPackRendererResources {
  readonly atlasSource: AssetPackTextureAtlasSource;
  readonly preparations: TextureAtlasPreparations;
  readonly blockRenderer: BlockRenderDispatcher;
  readonly getSprite: (location: ResourceLocation) => TextureAtlasSprite;
}

export async function initializeAssetPackBiomeColorTables(
  atlasSource: Pick<AssetPackTextureAtlasSource, "loadColorMap">,
): Promise<void> {
  const [grassPixels, foliagePixels] = await Promise.all([
    atlasSource.loadColorMap(GRASS_COLORMAP_LOCATION),
    atlasSource.loadColorMap(FOLIAGE_COLORMAP_LOCATION),
  ]);
  GrassColor.init(grassPixels);
  FoliageColor.init(foliagePixels);
}

export async function prepareAssetPackRendererResources(
  assetPack: AssetPack,
  blocks: GeneratedRenderBlockPalette,
  atlas: TextureAtlas,
  onProgress?: LoadingProgressSink,
): Promise<AssetPackRendererResources> {
  onProgress?.({ stage: "Loading biome colors", fraction: 0 });
  const atlasSource = new AssetPackTextureAtlasSource(assetPack);
  await initializeAssetPackBiomeColorTables(atlasSource);

  onProgress?.({ stage: "Preparing texture atlas", fraction: 0.15 });
  const preparations = await atlas.prepareToStitch(atlasSource, blocks.spriteLocations, DEFAULT_BLOCK_ATLAS_MIP_LEVEL);
  const getSprite = createSpriteLookup(preparations.regions);

  onProgress?.({ stage: "Loading block models", fraction: 0.45 });
  const blockRenderer = await createAssetPackBlockRenderer(assetPack, blocks, getSprite, onProgress);
  return { atlasSource, preparations, blockRenderer, getSprite };
}

export async function createAssetPackRenderWorldContext(
  assetPack: AssetPack,
  request: InitializeRenderWorldRequest,
  onProgress?: LoadingProgressSink,
): Promise<RenderWorldWorkerContext> {
  const blocks = registerGeneratedRenderBlocks();
  const atlas = new TextureAtlas(TextureAtlas.LOCATION_BLOCKS);
  const resources = await prepareAssetPackRendererResources(assetPack, blocks, atlas, (progress) => {
    onProgress?.({
      ...progress,
      stage: `Worker: ${progress.stage}`,
    });
  });
  resources.atlasSource.close();

  return {
    seed: request.seed,
    minBuildHeight: request.minBuildHeight,
    height: request.height,
    airState: blocks.airState,
    biomeSource: new OverworldBiomeSource(request.seed),
    blockStateResolver: createBlockStateResolver(blocks.airState),
    blockStateIds: blocks.blockStateIds,
    blockRenderer: resources.blockRenderer,
    level: new ClientChunkCache({
      airState: blocks.airState,
      minBuildHeight: request.minBuildHeight,
      height: request.height,
      biomeSource: new OverworldBiomeSource(request.seed),
      biomeZoomSeed: request.seed,
      blockStateResolver: createBlockStateResolver(blocks.airState),
      blockStateIds: blocks.blockStateIds,
    }),
  };
}

async function createAssetPackBlockRenderer(
  assetPack: AssetPack,
  blocks: GeneratedRenderBlockPalette,
  spriteLookup: (location: ResourceLocation) => TextureAtlasSprite,
  onProgress?: LoadingProgressSink,
): Promise<BlockRenderDispatcher> {
  const modelSource = await preloadBlockModelSource(assetPack, blocks.blockLocations, (progress) => {
    const fraction = progress.fraction ?? (progress.current !== undefined && progress.total !== undefined && progress.total > 0
      ? progress.current / progress.total
      : 0);
    onProgress?.({
      ...progress,
      stage: progress.stage,
      fraction: 0.45 + (Math.max(0, Math.min(1, fraction)) * 0.25),
    });
  });
  const repository = new BlockModelRepository(modelSource);
  onProgress?.({ stage: "Baking block models", fraction: 0.75 });
  const bakery = new ModelBakery(repository, (material) => spriteLookup(material.texture()));
  const modelManager = new ModelManager(bakery.getMissingBakedModel());
  bakery.bakeTopLevelBlockModels(modelManager);
  const blockModelShaper = new BlockModelShaper(modelManager);
  blockModelShaper.rebuildCache();
  onProgress?.({ stage: "Renderer model cache ready", fraction: 1 });

  return new BlockRenderDispatcher(
    blockModelShaper,
    BlockColors.createDefault(),
    (location) => spriteLookup(location),
    blocks.blockStateById[ChunkBlockId.WATER]!,
    blocks.blockStateById[ChunkBlockId.LAVA]!,
  );
}

function createSpriteLookup(sprites: readonly TextureAtlasSprite[]): (location: ResourceLocation) => TextureAtlasSprite {
  const spritesByName = new Map(sprites.map((sprite) => [sprite.getName().toString(), sprite] as const));
  const missing = spritesByName.get(MissingTextureAtlasSprite.getLocation().toString());
  if (missing === undefined) {
    throw new Error("Missing texture atlas sprite was not loaded for asset-pack renderer resources");
  }

  return (location) => spritesByName.get(location.toString()) ?? missing;
}
