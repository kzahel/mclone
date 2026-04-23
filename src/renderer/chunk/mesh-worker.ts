import { BlockPos } from "../../core/block-pos";
import { ResourceLocation } from "../../core/resource-location";
import { OverworldBiomeSource } from "../../worldgen/biome/overworld-biome-source";
import { ChunkBlockId } from "../../worldgen/chunk/chunk-block-buffer";
import { ClientChunkCache } from "../../world/level/client-chunk-cache";
import { createBlockStateResolver, type BlockStateResolver, type ChunkSnapshot } from "../../world/level/chunk-snapshot";
import type { BlockState } from "../../world/level/block/state/block-state";
import { FoliageColor } from "../../world/level/foliage-color";
import { registerGeneratedRenderBlocks } from "../../world/level/generated-render-blocks";
import { GrassColor } from "../../world/level/grass-color";
import { BlockColors } from "../block/block-colors";
import { BlockRenderDispatcher } from "../block/block-render-dispatcher";
import { ChunkBufferBuilderPack } from "../chunk-buffer-builder-pack";
import { BlockModelRepository } from "../model/block-model-repository";
import { BlockModelShaper } from "../model/block-model-shaper";
import { preloadBlockModelSource } from "../model/browser-block-model-source";
import { ModelBakery } from "../model/model-bakery";
import { ModelManager } from "../model/model-manager";
import { BrowserTextureAtlasSource } from "../texture/browser-native-image-loader";
import { MissingTextureAtlasSprite } from "../texture/missing-texture-atlas-sprite";
import type { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { TextureAtlas } from "../texture/texture-atlas";
import {
  serializeSectionMeshBuild,
  type BuildSectionMeshRequest,
  type InitializeMeshWorkerRequest,
  type MeshWorkerRequest,
  type MeshWorkerResponse,
} from "./chunk-mesh-protocol";
import { connectMeshWorkerSession, type MeshWorkerHostEndpoint } from "./mesh-worker-client";
import { RenderChunkRegion } from "./render-chunk-region";
import { buildSectionMesh } from "./section-mesh-compiler";
import { Vec3 } from "../../world/phys/vec3";

const GRASS_COLORMAP_LOCATION = new ResourceLocation("minecraft:colormap/grass");
const FOLIAGE_COLORMAP_LOCATION = new ResourceLocation("minecraft:colormap/foliage");

interface MeshWorkerContext {
  readonly seed: bigint;
  readonly minBuildHeight: number;
  readonly height: number;
  readonly airState: BlockState;
  readonly biomeSource: OverworldBiomeSource;
  readonly blockStateResolver: BlockStateResolver;
  readonly blockRenderer: BlockRenderDispatcher;
}

function createSpriteLookup(sprites: readonly TextureAtlasSprite[]): (location: ResourceLocation) => TextureAtlasSprite {
  const spritesByName = new Map(sprites.map((sprite) => [sprite.getName().toString(), sprite] as const));
  const missing = spritesByName.get(MissingTextureAtlasSprite.getLocation().toString());
  if (missing === undefined) {
    throw new Error("Missing texture atlas sprite was not loaded for mesh worker");
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

async function createMeshWorkerContext(request: InitializeMeshWorkerRequest): Promise<MeshWorkerContext> {
  const generatedBlocks = registerGeneratedRenderBlocks();
  const atlasSource = new BrowserTextureAtlasSource();
  await initializeBiomeColorTables(atlasSource);

  const atlas = new TextureAtlas(TextureAtlas.LOCATION_BLOCKS);
  const preparations = await atlas.prepareToStitch(atlasSource, generatedBlocks.spriteLocations, 0);
  const getSprite = createSpriteLookup(preparations.regions);

  const modelSource = await preloadBlockModelSource(generatedBlocks.blockLocations);
  const repository = new BlockModelRepository(modelSource);
  const bakery = new ModelBakery(repository, (material) => getSprite(material.texture()));
  const modelManager = new ModelManager(bakery.getMissingBakedModel());
  bakery.bakeTopLevelBlockModels(modelManager);
  const blockModelShaper = new BlockModelShaper(modelManager);
  blockModelShaper.rebuildCache();

  return {
    seed: request.seed,
    minBuildHeight: request.minBuildHeight,
    height: request.height,
    airState: generatedBlocks.airState,
    biomeSource: new OverworldBiomeSource(request.seed),
    blockStateResolver: createBlockStateResolver(generatedBlocks.airState),
    blockRenderer: new BlockRenderDispatcher(
      blockModelShaper,
      BlockColors.createDefault(),
      (location) => getSprite(location),
      generatedBlocks.blockStateById[ChunkBlockId.WATER]!,
      generatedBlocks.blockStateById[ChunkBlockId.LAVA]!,
    ),
  };
}

function createJobLevel(context: MeshWorkerContext, snapshots: readonly ChunkSnapshot[]): ClientChunkCache {
  const level = new ClientChunkCache({
    airState: context.airState,
    minBuildHeight: context.minBuildHeight,
    height: context.height,
    biomeSource: context.biomeSource,
    biomeZoomSeed: context.seed,
    blockStateResolver: context.blockStateResolver,
  });

  for (const snapshot of snapshots) {
    level.applyChunkSnapshot(snapshot);
  }

  return level;
}

async function buildWorkerResponse(context: MeshWorkerContext, request: BuildSectionMeshRequest): Promise<MeshWorkerResponse> {
  const level = createJobLevel(context, request.snapshots);
  const origin = new BlockPos(request.origin.x, request.origin.y, request.origin.z);
  const region = RenderChunkRegion.createIfNotEmpty(level, origin.offset(-1, -1, -1), origin.offset(16, 16, 16), 1);
  const buffers = new ChunkBufferBuilderPack();

  try {
    const build = buildSectionMesh(
      origin,
      new Vec3(request.camera.x, request.camera.y, request.camera.z),
      region,
      context.blockRenderer,
      buffers,
    );

    return {
      type: "section_mesh_built",
      result: serializeSectionMeshBuild(build, buffers),
    };
  } finally {
    buffers.discardAll();
  }
}

let meshWorkerContextPromise: Promise<MeshWorkerContext> | undefined;

async function handleMeshWorkerRequest(message: MeshWorkerRequest): Promise<MeshWorkerResponse> {
  switch (message.type) {
    case "initialize_mesh_worker":
      meshWorkerContextPromise = createMeshWorkerContext(message);
      await meshWorkerContextPromise;
      return { type: "mesh_worker_ready" };
    case "build_section_mesh": {
      if (meshWorkerContextPromise === undefined) {
        throw new Error("build_section_mesh received before initialize_mesh_worker");
      }

      return buildWorkerResponse(await meshWorkerContextPromise, message);
    }
  }
}

connectMeshWorkerSession(
  globalThis as unknown as MeshWorkerHostEndpoint,
  handleMeshWorkerRequest,
);
