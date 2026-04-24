import { ResourceLocation } from "../../core/resource-location";
import { OverworldBiomeSource } from "../../worldgen/biome/overworld-biome-source";
import { ChunkBlockId } from "../../worldgen/chunk/chunk-block-buffer";
import { createBlockStateResolver, type BlockStateResolver } from "../../world/level/chunk-snapshot";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { BlockStateIdMap } from "../../world/level/block/state/block-state-id";
import { FoliageColor } from "../../world/level/foliage-color";
import { registerGeneratedRenderBlocks } from "../../world/level/generated-render-blocks";
import { GrassColor } from "../../world/level/grass-color";
import { getBrowserAssetPack } from "../assets/asset-pack";
import type { LoadingProgressSink } from "../loading-progress";
import { BlockColors } from "../block/block-colors";
import { BlockRenderDispatcher } from "../block/block-render-dispatcher";
import { BlockModelRepository } from "../model/block-model-repository";
import { BlockModelShaper } from "../model/block-model-shaper";
import { preloadBlockModelSource } from "../model/browser-block-model-source";
import { ModelBakery } from "../model/model-bakery";
import { ModelManager } from "../model/model-manager";
import { BrowserTextureAtlasSource } from "../texture/browser-native-image-loader";
import { MissingTextureAtlasSprite } from "../texture/missing-texture-atlas-sprite";
import type { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { DEFAULT_BLOCK_ATLAS_MIP_LEVEL, TextureAtlas } from "../texture/texture-atlas";

const GRASS_COLORMAP_LOCATION = new ResourceLocation("minecraft:colormap/grass");
const FOLIAGE_COLORMAP_LOCATION = new ResourceLocation("minecraft:colormap/foliage");

export interface BrowserChunkMeshContextRequest {
  readonly seed: bigint;
  readonly minBuildHeight: number;
  readonly height: number;
}

export interface BrowserChunkMeshContext {
  readonly seed: bigint;
  readonly minBuildHeight: number;
  readonly height: number;
  readonly airState: BlockState;
  readonly biomeSource: OverworldBiomeSource;
  readonly blockStateResolver: BlockStateResolver;
  readonly blockStateIds: BlockStateIdMap;
  readonly blockRenderer: BlockRenderDispatcher;
}

function createSpriteLookup(sprites: readonly TextureAtlasSprite[]): (location: ResourceLocation) => TextureAtlasSprite {
  const spritesByName = new Map(sprites.map((sprite) => [sprite.getName().toString(), sprite] as const));
  const missing = spritesByName.get(MissingTextureAtlasSprite.getLocation().toString());
  if (missing === undefined) {
    throw new Error("Missing texture atlas sprite was not loaded for chunk mesh context");
  }

  return (location) => spritesByName.get(location.toString()) ?? missing;
}

async function initializeBiomeColorTables(atlasSource: BrowserTextureAtlasSource): Promise<void> {
  const [grassPixels, foliagePixels] = await Promise.all([
    atlasSource.loadColorMap(GRASS_COLORMAP_LOCATION),
    atlasSource.loadColorMap(FOLIAGE_COLORMAP_LOCATION),
  ]);
  GrassColor.init(grassPixels);
  FoliageColor.init(foliagePixels);
}

export async function createBrowserChunkMeshContext(
  request: BrowserChunkMeshContextRequest,
  onProgress?: LoadingProgressSink,
): Promise<BrowserChunkMeshContext> {
  const generatedBlocks = registerGeneratedRenderBlocks();
  onProgress?.({ stage: "Loading worker asset pack", fraction: 0 });
  const assetPack = await getBrowserAssetPack((progress) => onProgress?.({
    ...progress,
    stage: `Worker: ${progress.stage}`,
    fraction: progress.fraction === undefined ? undefined : progress.fraction * 0.2,
  }));
  onProgress?.({ stage: "Worker: loading biome colors", fraction: 0.2 });
  const atlasSource = new BrowserTextureAtlasSource(assetPack);
  await initializeBiomeColorTables(atlasSource);

  onProgress?.({ stage: "Worker: preparing texture atlas", fraction: 0.25 });
  const atlas = new TextureAtlas(TextureAtlas.LOCATION_BLOCKS);
  const preparations = await atlas.prepareToStitch(atlasSource, generatedBlocks.spriteLocations, DEFAULT_BLOCK_ATLAS_MIP_LEVEL);
  const getSprite = createSpriteLookup(preparations.regions);

  onProgress?.({ stage: "Worker: loading block models", fraction: 0.45 });
  const modelSource = await preloadBlockModelSource(assetPack, generatedBlocks.blockLocations, (progress) => {
    const fraction = progress.fraction ?? (progress.current !== undefined && progress.total !== undefined && progress.total > 0
      ? progress.current / progress.total
      : 0);
    onProgress?.({
      ...progress,
      stage: `Worker: ${progress.stage}`,
      fraction: 0.45 + (Math.max(0, Math.min(1, fraction)) * 0.25),
    });
  });
  const repository = new BlockModelRepository(modelSource);
  onProgress?.({ stage: "Worker: baking block models", fraction: 0.75 });
  const bakery = new ModelBakery(repository, (material) => getSprite(material.texture()));
  const modelManager = new ModelManager(bakery.getMissingBakedModel());
  bakery.bakeTopLevelBlockModels(modelManager);
  const blockModelShaper = new BlockModelShaper(modelManager);
  blockModelShaper.rebuildCache();
  onProgress?.({ stage: "Worker: renderer model cache ready", fraction: 1 });

  return {
    seed: request.seed,
    minBuildHeight: request.minBuildHeight,
    height: request.height,
    airState: generatedBlocks.airState,
    biomeSource: new OverworldBiomeSource(request.seed),
    blockStateResolver: createBlockStateResolver(generatedBlocks.airState),
    blockStateIds: generatedBlocks.blockStateIds,
    blockRenderer: new BlockRenderDispatcher(
      blockModelShaper,
      BlockColors.createDefault(),
      (location) => getSprite(location),
      generatedBlocks.blockStateById[ChunkBlockId.WATER]!,
      generatedBlocks.blockStateById[ChunkBlockId.LAVA]!,
    ),
  };
}
