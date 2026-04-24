import { readFileSync } from "node:fs";
import path from "node:path";
import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../src/core/block-pos";
import { Direction } from "../../../src/core/direction";
import { Registry } from "../../../src/core/registry";
import { ResourceLocation } from "../../../src/core/resource-location";
import { BlockColors } from "../../../src/renderer/block/block-colors";
import { BlockRenderDispatcher } from "../../../src/renderer/block/block-render-dispatcher";
import { ChunkRenderDispatcher, type RenderWorldMeshBuildClient } from "../../../src/renderer/chunk/chunk-render-dispatcher";
import { buildSectionMeshInput, decodeSectionMeshResult, serializeSectionMeshBuild } from "../../../src/renderer/chunk/chunk-mesh-protocol";
import { RenderChunkRegion } from "../../../src/renderer/chunk/render-chunk-region";
import type { BuildRenderSectionMeshRequest, RenderWorldMeshBuildResponse } from "../../../src/renderer/chunk/render-world-protocol";
import { buildSectionMesh } from "../../../src/renderer/chunk/section-mesh-compiler";
import { VisGraph } from "../../../src/renderer/chunk/vis-graph";
import { ChunkBufferBuilderPack } from "../../../src/renderer/chunk-buffer-builder-pack";
import { Frustum } from "../../../src/renderer/culling/frustum";
import { GameRenderer } from "../../../src/renderer/game-renderer";
import { LevelRenderer } from "../../../src/renderer/level-renderer";
import { LightTexture } from "../../../src/renderer/light-texture";
import { Matrix4f } from "../../../src/renderer/math/matrix4f";
import { BlockModelRepository, type BlockModelSource } from "../../../src/renderer/model/block-model-repository";
import { BlockModelShaper } from "../../../src/renderer/model/block-model-shaper";
import { Material } from "../../../src/renderer/model/material";
import { ModelBakery } from "../../../src/renderer/model/model-bakery";
import { ModelManager } from "../../../src/renderer/model/model-manager";
import { RenderType } from "../../../src/renderer/render-type";
import { AnimationMetadataSection } from "../../../src/renderer/texture/animation-metadata-section";
import { NativeImage } from "../../../src/renderer/texture/native-image";
import { TextureAtlasSprite, TextureAtlasSpriteInfo, type TextureAtlasUploadTarget } from "../../../src/renderer/texture/texture-atlas-sprite";
import { DefaultVertexFormat } from "../../../src/renderer/vertex/default-vertex-format";
import { ViewArea } from "../../../src/renderer/view-area";
import { OverworldBiomeSource } from "../../../src/worldgen/biome/overworld-biome-source";
import { ChunkBiomeContainer } from "../../../src/worldgen/biome/chunk-biome-container";
import { ClientChunkCache } from "../../../src/world/level/client-chunk-cache";
import { AirBlock } from "../../../src/world/level/block/air-block";
import { Block } from "../../../src/world/level/block/block";
import { LiquidBlock } from "../../../src/world/level/block/liquid-block";
import { BlockBehaviour } from "../../../src/world/level/block/state/block-behaviour";
import type { BlockState } from "../../../src/world/level/block/state/block-state";
import { buildChunkSnapshot, createBlockStateResolver } from "../../../src/world/level/chunk-snapshot";
import { Fluids } from "../../../src/world/level/material/fluids";
import { Material as BlockMaterial } from "../../../src/world/level/material/material";
import { registerGeneratedRenderBlocks } from "../../../src/world/level/generated-render-blocks";
import { StaticRenderLevel } from "../../../src/world/level/static-render-level";
import { AABB } from "../../../src/world/phys/aabb";
import { Vec3 } from "../../../src/world/phys/vec3";
import { ChunkBlockId } from "../../../src/worldgen/chunk/chunk-block-buffer";

const ASSETS_ROOT = path.resolve(process.cwd(), "reference/minecraft-1.17.1/extracted/assets");

(globalThis as unknown as { GPUBufferUsage?: Record<string, number> }).GPUBufferUsage ??= {
  VERTEX: 1 << 0,
  INDEX: 1 << 1,
  UNIFORM: 1 << 2,
  COPY_DST: 1 << 3,
  MAP_READ: 1 << 4,
};
(globalThis as unknown as { GPUTextureUsage?: Record<string, number> }).GPUTextureUsage ??= {
  TEXTURE_BINDING: 1 << 0,
  COPY_DST: 1 << 1,
};

class ExtractedAssetModelSource implements BlockModelSource {
  public getModelJson(location: ResourceLocation): string | undefined {
    const modelPath = path.join(ASSETS_ROOT, location.getNamespace(), "models", `${location.getPath()}.json`);
    try {
      return readFileSync(modelPath, "utf8");
    } catch {
      return undefined;
    }
  }

  public getBlockStateJson(location: ResourceLocation): string | undefined {
    const blockStatePath = path.join(ASSETS_ROOT, location.getNamespace(), "blockstates", `${location.getPath()}.json`);
    try {
      return readFileSync(blockStatePath, "utf8");
    } catch {
      return undefined;
    }
  }
}

const DUMMY_ATLAS: TextureAtlasUploadTarget = {
  upload(): void {},
};

class TestSprite extends TextureAtlasSprite {
  public constructor(location: ResourceLocation) {
    super(
      DUMMY_ATLAS,
      new TextureAtlasSpriteInfo(location, 16, 16, AnimationMetadataSection.EMPTY),
      0,
      16,
      16,
      0,
      0,
      new NativeImage(16, 16, false),
    );
  }

  public override uvShrinkRatio(): number {
    return 0;
  }
}

function createSpriteCache(): {
  readonly getByMaterial: (material: Material) => TextureAtlasSprite;
  readonly getByLocation: (location: ResourceLocation) => TextureAtlasSprite;
} {
  const sprites = new Map<string, TextureAtlasSprite>();
  const getByLocation = (location: ResourceLocation): TextureAtlasSprite => {
    const key = location.toString();
    const cached = sprites.get(key);
    if (cached !== undefined) {
      return cached;
    }

    const sprite = new TestSprite(location);
    sprites.set(key, sprite);
    return sprite;
  };
  return {
    getByMaterial: (material) => getByLocation(material.texture()),
    getByLocation,
  };
}

function createAirState(): BlockState {
  const properties = BlockBehaviour.Properties.of(BlockMaterial.AIR).noCollission().noOcclusion();
  properties.isAir = true;
  return new AirBlock(properties).defaultBlockState();
}

function createBlock(location: string, material: BlockMaterial, configure?: (properties: BlockBehaviour.Properties) => void): Block {
  const properties = BlockBehaviour.Properties.of(material);
  configure?.(properties);
  const block = new Block(properties).setLocation(new ResourceLocation(location));
  Registry.register(Registry.BLOCK, block.getLocation()!, block);
  return block;
}

function createFluidBlock(location: string, fluid: typeof Fluids.WATER, material: BlockMaterial): BlockState {
  const existing = Registry.BLOCK.get(new ResourceLocation(location)) as Block | undefined;
  if (existing !== undefined) {
    return existing.defaultBlockState();
  }

  const block = new LiquidBlock(fluid, BlockBehaviour.Properties.of(material).noCollission()).setLocation(new ResourceLocation(location));
  Registry.register(Registry.BLOCK, block.getLocation()!, block);
  return block.defaultBlockState();
}

function createDispatcher(): BlockRenderDispatcher {
  const spriteCache = createSpriteCache();
  const waterState = createFluidBlock("minecraft:water", Fluids.WATER, BlockMaterial.WATER);
  const lavaState = createFluidBlock("minecraft:lava", Fluids.LAVA, BlockMaterial.LAVA);
  const repository = new BlockModelRepository(new ExtractedAssetModelSource());
  const bakery = new ModelBakery(repository, spriteCache.getByMaterial);
  const modelManager = new ModelManager(bakery.getMissingBakedModel());
  bakery.bakeTopLevelBlockModels(modelManager);
  const shaper = new BlockModelShaper(modelManager);
  shaper.rebuildCache();
  return new BlockRenderDispatcher(shaper, BlockColors.createDefault(), spriteCache.getByLocation, waterState, lavaState);
}

function createFakeDevice(): GPUDevice {
  return {
    queue: {
      writeTexture() {},
    },
    createBuffer(descriptor: GPUBufferDescriptor) {
      const mappedRange = new ArrayBuffer(Number(descriptor.size));
      return {
        getMappedRange() {
          return mappedRange;
        },
        unmap() {},
        destroy() {},
      } as unknown as GPUBuffer;
    },
    createTexture() {
      return {
        createView() {
          return {} as GPUTextureView;
        },
        destroy() {},
      } as unknown as GPUTexture;
    },
  } as unknown as GPUDevice;
}

function fillBox(level: StaticRenderLevel, from: BlockPos, to: BlockPos, state: BlockState): void {
  for (let z = from.getZ(); z <= to.getZ(); z++) {
    for (let y = from.getY(); y <= to.getY(); y++) {
      for (let x = from.getX(); x <= to.getX(); x++) {
        level.setBlock(new BlockPos(x, y, z), state);
      }
    }
  }
}

function firstBlockVertexColor(buffer: Uint8Array): readonly [number, number, number, number] {
  return [buffer[12]!, buffer[13]!, buffer[14]!, buffer[15]!];
}

class RecordingRenderWorldMeshBuildClient implements RenderWorldMeshBuildClient {
  public readonly concurrency = 1;
  public readonly requests: BuildRenderSectionMeshRequest[] = [];
  public closed = false;

  public constructor(private readonly response: RenderWorldMeshBuildResponse) {}

  public buildSectionMesh(request: BuildRenderSectionMeshRequest): Promise<RenderWorldMeshBuildResponse> {
    this.requests.push(request);
    return Promise.resolve(this.response);
  }

  public close(): void {
    this.closed = true;
  }
}

describe("Chunk render infrastructure", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  test("VisGraph resolves sparse sections as fully visible and fully opaque sections as fully closed", () => {
    const openGraph = new VisGraph();
    const openVisibility = openGraph.resolve();
    expect(openVisibility.visibilityBetween(Direction.NORTH, Direction.SOUTH)).toBe(true);
    expect(openVisibility.visibilityBetween(Direction.UP, Direction.DOWN)).toBe(true);

    const closedGraph = new VisGraph();
    for (let z = 0; z < 16; z++) {
      for (let y = 0; y < 16; y++) {
        for (let x = 0; x < 16; x++) {
          closedGraph.setOpaque(new BlockPos(x, y, z));
        }
      }
    }

    const closedVisibility = closedGraph.resolve();
    expect(closedVisibility.visibilityBetween(Direction.NORTH, Direction.SOUTH)).toBe(false);
    expect(closedVisibility.visibilityBetween(Direction.WEST, Direction.EAST)).toBe(false);
  });

  test("RenderChunkRegion.createIfNotEmpty skips empty chunks and caches padded block states when populated", () => {
    const airState = createAirState();
    const stone = createBlock("minecraft:stone", BlockMaterial.STONE);
    const emptyLevel = new StaticRenderLevel(airState);
    expect(RenderChunkRegion.createIfNotEmpty(emptyLevel, new BlockPos(-1, -1, -1), new BlockPos(16, 16, 16), 1)).toBeNull();

    const populatedLevel = new StaticRenderLevel(airState);
    populatedLevel.setBlock(new BlockPos(0, 0, 0), stone.defaultBlockState());
    const region = RenderChunkRegion.createIfNotEmpty(populatedLevel, new BlockPos(-1, -1, -1), new BlockPos(16, 16, 16), 1);
    expect(region).not.toBeNull();
    expect(region!.getBlockState(new BlockPos(0, 0, 0))).toBe(stone.defaultBlockState());
    expect(region!.getBlockState(new BlockPos(1, 0, 0)).isAir()).toBe(true);
  });

  test("ChunkBufferBuilderPack allocates one builder per chunk render layer", () => {
    const pack = new ChunkBufferBuilderPack();
    for (const renderType of RenderType.chunkBufferLayers()) {
      pack.builder(renderType).begin(renderType.mode(), renderType.format());
      pack.builder(renderType).end();
    }

    pack.clearAll();
    pack.discardAll();

    for (const renderType of RenderType.chunkBufferLayers()) {
      expect(pack.builder(renderType)).toBeDefined();
    }
  });

  test("ChunkRenderDispatcher compiles a populated section into a solid layer buffer and visibility set", async () => {
    const airState = createAirState();
    const stone = createBlock("minecraft:stone", BlockMaterial.STONE);
    const level = new StaticRenderLevel(airState);
    level.setBlock(new BlockPos(0, 0, 0), stone.defaultBlockState());
    level.getChunk(-1, 0);
    level.getChunk(1, 0);
    level.getChunk(0, -1);
    level.getChunk(0, 1);

    const levelRenderer = new LevelRenderer();
    const chunkDispatcher = new ChunkRenderDispatcher(level, levelRenderer, createDispatcher(), createFakeDevice(), (task) => task());
    const viewArea = new ViewArea(chunkDispatcher, level, 1, levelRenderer);
    chunkDispatcher.setCamera(new Vec3(8, 8, 0));
    viewArea.repositionCamera(8, 0);

    const renderChunk = viewArea.getRenderChunkAt(new BlockPos(0, 0, 0));
    expect(renderChunk).not.toBeNull();
    renderChunk!.rebuildChunkAsync(chunkDispatcher);
    await chunkDispatcher.awaitAllTasks();

    const compiled = renderChunk!.getCompiledChunk();
    expect(compiled.hasNoRenderableLayers()).toBe(false);
    expect(compiled.hasBlocks.has(RenderType.solid())).toBe(true);
    expect(renderChunk!.getBuffer(RenderType.solid()).getFormat()).toBe(DefaultVertexFormat.BLOCK);
    expect(compiled.facesCanSeeEachother(Direction.NORTH, Direction.SOUTH)).toBe(true);
  });

  test("ChunkRenderDispatcher routes snow and water into cutout and translucent layers", async () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = new StaticRenderLevel(blocks.airState);
    level.setBlock(new BlockPos(0, 0, 0), blocks.blockStateById[ChunkBlockId.WATER]!);
    level.setBlock(new BlockPos(1, 0, 0), blocks.blockStateById[ChunkBlockId.SNOW]!);
    level.getChunk(-1, 0);
    level.getChunk(1, 0);
    level.getChunk(0, -1);
    level.getChunk(0, 1);

    const levelRenderer = new LevelRenderer();
    const chunkDispatcher = new ChunkRenderDispatcher(level, levelRenderer, createDispatcher(), createFakeDevice(), (task) => task());
    const viewArea = new ViewArea(chunkDispatcher, level, 1, levelRenderer);
    chunkDispatcher.setCamera(new Vec3(8, 8, 0));
    viewArea.repositionCamera(8, 0);

    const renderChunk = viewArea.getRenderChunkAt(new BlockPos(0, 0, 0));
    expect(renderChunk).not.toBeNull();
    renderChunk!.rebuildChunkAsync(chunkDispatcher);
    await chunkDispatcher.awaitAllTasks();

    const compiled = renderChunk!.getCompiledChunk();
    expect(compiled.hasBlocks.has(RenderType.cutout())).toBe(true);
    expect(compiled.hasBlocks.has(RenderType.translucent())).toBe(true);
  });

  test("fluid mesh vertices preserve Java float colors as byte colors", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = new StaticRenderLevel(blocks.airState);
    level.setBlock(new BlockPos(0, 0, 0), blocks.blockStateById[ChunkBlockId.WATER]!);
    level.setBlock(new BlockPos(2, 0, 0), blocks.blockStateById[ChunkBlockId.LAVA]!);

    const origin = new BlockPos(0, 0, 0);
    const region = RenderChunkRegion.createIfNotEmpty(level, origin.offset(-1, -1, -1), origin.offset(16, 16, 16), 1);
    const buffers = new ChunkBufferBuilderPack();
    const build = buildSectionMesh(origin, new Vec3(8, 8, 0), region, createDispatcher(), buffers);

    expect(build.hasBlocks.has(RenderType.translucent())).toBe(true);
    expect(build.hasBlocks.has(RenderType.solid())).toBe(true);

    const water = buffers.builder(RenderType.translucent()).popNextBuffer();
    const lava = buffers.builder(RenderType.solid()).popNextBuffer();
    expect(water.drawState.vertexCount()).toBeGreaterThan(0);
    expect(lava.drawState.vertexCount()).toBeGreaterThan(0);
    expect(firstBlockVertexColor(water.buffer)).toEqual([255, 255, 255, 255]);
    expect(firstBlockVertexColor(lava.buffer)).toEqual([255, 255, 255, 255]);
  });

  test("worker-facing section mesh payloads round-trip chunk snapshots into uploadable layer data", () => {
    const blocks = registerGeneratedRenderBlocks();
    const biomeSource = new OverworldBiomeSource(12345n);
    const level = new StaticRenderLevel(blocks.airState, 15, 15, 0, 16);
    for (let chunkX = -1; chunkX <= 1; chunkX++) {
      for (let chunkZ = -1; chunkZ <= 1; chunkZ++) {
        level.getChunk(chunkX, chunkZ);
      }
    }

    level.setBlock(new BlockPos(0, 0, 0), blocks.blockStateById[ChunkBlockId.STONE]!);

    const cache = new ClientChunkCache({
      airState: blocks.airState,
      minBuildHeight: 0,
      height: 16,
      biomeSource,
      biomeZoomSeed: 12345n,
      blockStateResolver: createBlockStateResolver(blocks.airState),
      blockStateIds: blocks.blockStateIds,
    });
    for (let chunkX = -1; chunkX <= 1; chunkX++) {
      for (let chunkZ = -1; chunkZ <= 1; chunkZ++) {
        cache.applyChunkSnapshot(
          buildChunkSnapshot(
            level.getChunk(chunkX, chunkZ, false)!,
            new ChunkBiomeContainer(0, 16, chunkX, chunkZ, biomeSource).writeBiomes(),
            0,
            16,
          ),
        );
      }
    }

    const origin = new BlockPos(0, 0, 0);
    const meshInput = buildSectionMeshInput(cache, origin);
    expect(meshInput.snapshots).toHaveLength(9);
    expect(cache.getChunkSnapshot(0, 0)).toBeDefined();

    const region = RenderChunkRegion.createIfNotEmpty(cache, origin.offset(-1, -1, -1), origin.offset(16, 16, 16), 1);
    const buffers = new ChunkBufferBuilderPack();
    const build = buildSectionMesh(origin, new Vec3(8, 8, 0), region, createDispatcher(), buffers);
    const decoded = decodeSectionMeshResult(serializeSectionMeshBuild(build, buffers));

    expect(decoded.hasBlocks.has(RenderType.solid())).toBe(true);
    expect(decoded.hasLayers.has(RenderType.solid())).toBe(true);
    expect(decoded.layers).toHaveLength(1);
    expect(decoded.layers[0]!.renderType).toBe(RenderType.solid());
    expect(decoded.layers[0]!.drawState.format()).toBe(DefaultVertexFormat.BLOCK);
    expect(decoded.visibilitySet.visibilityBetween(Direction.NORTH, Direction.SOUTH)).toBe(true);
  });

  test("ChunkRenderDispatcher can build a section from a render-world worker without main-thread chunk snapshots", async () => {
    const blocks = registerGeneratedRenderBlocks();
    const biomeSource = new OverworldBiomeSource(12345n);
    const populatedLevel = new StaticRenderLevel(blocks.airState, 15, 15, 0, 16);
    for (let chunkX = -1; chunkX <= 1; chunkX++) {
      for (let chunkZ = -1; chunkZ <= 1; chunkZ++) {
        populatedLevel.getChunk(chunkX, chunkZ);
      }
    }
    populatedLevel.setBlock(new BlockPos(0, 0, 0), blocks.blockStateById[ChunkBlockId.STONE]!);

    const origin = new BlockPos(0, 0, 0);
    const region = RenderChunkRegion.createIfNotEmpty(populatedLevel, origin.offset(-1, -1, -1), origin.offset(16, 16, 16), 1);
    const buffers = new ChunkBufferBuilderPack();
    const build = buildSectionMesh(origin, new Vec3(8, 8, 0), region, createDispatcher(), buffers);
    const renderWorldClient = new RecordingRenderWorldMeshBuildClient({
      type: "render_section_mesh_built",
      origin: { x: 0, y: 0, z: 0 },
      result: serializeSectionMeshBuild(build, buffers),
    });

    const mainThreadCache = new ClientChunkCache({
      airState: blocks.airState,
      minBuildHeight: 0,
      height: 16,
      biomeSource,
      biomeZoomSeed: 12345n,
      blockStateResolver: createBlockStateResolver(blocks.airState),
      blockStateIds: blocks.blockStateIds,
    });
    const levelRenderer = new LevelRenderer();
    const chunkDispatcher = new ChunkRenderDispatcher(
      mainThreadCache,
      levelRenderer,
      createDispatcher(),
      createFakeDevice(),
      (task) => task(),
      false,
      new ChunkBufferBuilderPack(),
      undefined,
      renderWorldClient,
    );
    const viewArea = new ViewArea(chunkDispatcher, mainThreadCache, 1, levelRenderer);
    chunkDispatcher.setCamera(new Vec3(8, 8, 0));
    viewArea.repositionCamera(8, 0);

    const renderChunk = viewArea.getRenderChunkAt(origin);
    expect(renderChunk).not.toBeNull();
    renderChunk!.rebuildChunkAsync(chunkDispatcher);
    await chunkDispatcher.awaitAllTasks();

    expect(mainThreadCache.getLoadedChunkCount()).toBe(0);
    expect(renderWorldClient.requests).toEqual([{
      type: "build_render_section_mesh",
      origin: { x: 0, y: 0, z: 0 },
      camera: { x: 8, y: 8, z: 0 },
    }]);
    const compiled = renderChunk!.getCompiledChunk();
    expect(compiled.hasBlocks.has(RenderType.solid())).toBe(true);
    expect(compiled.hasLayer.has(RenderType.solid())).toBe(true);
    expect(renderChunk!.getBuffer(RenderType.solid()).getFormat()).toBe(DefaultVertexFormat.BLOCK);
    expect(chunkDispatcher.getMainThreadGpuUploadCount()).toBe(1);

    chunkDispatcher.dispose();
    expect(renderWorldClient.closed).toBe(true);
  });

  test("Frustum accepts boxes in front of the camera and rejects boxes behind it", () => {
    const modelView = new Matrix4f();
    modelView.setIdentity();
    const projection = Matrix4f.perspective(70, 1, 0.05, 64);
    const frustum = new Frustum(modelView, projection);
    frustum.prepare(0, 0, 0);

    expect(frustum.isVisible(new AABB(-1, -1, -5, 1, 1, -3))).toBe(true);
    expect(frustum.isVisible(new AABB(-1, -1, 3, 1, 1, 5))).toBe(false);
  });

  test("LightTexture maps full sky light to bright daylight", () => {
    const lightTexture = new LightTexture(new GameRenderer(800, 600, 64), new StaticRenderLevel(createAirState()), createFakeDevice());
    lightTexture.updateLightTexture(0);

    const daylight = lightTexture.samplePacked(LightTexture.FULL_SKY);
    expect(NativeImage.getR(daylight)).toBeGreaterThan(240);
    expect(NativeImage.getG(daylight)).toBeGreaterThan(240);
    expect(NativeImage.getB(daylight)).toBeGreaterThan(240);
    lightTexture.close();
  });

  test("GameRenderer and LevelRenderer build a camera-driven frame with only front-facing solid chunk draws", async () => {
    const airState = createAirState();
    const stone = createBlock("minecraft:stone", BlockMaterial.STONE);
    const level = new StaticRenderLevel(airState);
    fillBox(level, new BlockPos(0, 0, 0), new BlockPos(15, 15, 0), stone.defaultBlockState());
    fillBox(level, new BlockPos(0, 0, 40), new BlockPos(15, 15, 40), stone.defaultBlockState());
    for (let chunkX = -2; chunkX <= 2; chunkX++) {
      for (let chunkZ = -1; chunkZ <= 4; chunkZ++) {
        level.getChunk(chunkX, chunkZ);
      }
    }

    const levelRenderer = new LevelRenderer();
    const device = createFakeDevice();
    const chunkDispatcher = new ChunkRenderDispatcher(level, levelRenderer, createDispatcher(), device, (task) => task());
    const viewArea = new ViewArea(chunkDispatcher, level, 2, levelRenderer);
    levelRenderer.setLevel(level, chunkDispatcher, viewArea, 2);
    const gameRenderer = new GameRenderer(800, 600, 64);
    const lightTexture = new LightTexture(gameRenderer, level, device);
    lightTexture.tick();

    const frame = await gameRenderer.renderLevel(0, Number.MAX_SAFE_INTEGER, levelRenderer, lightTexture, {
      position: new Vec3(8.5, 8.5, 20),
      xRot: 0,
      yRot: 180,
    }, { waitForChunkTasks: true });

    const solidDraws = frame.layerDraws.get(RenderType.solid()) ?? [];
    expect(solidDraws.length).toBeGreaterThan(0);
    expect(solidDraws.some((draw) => draw.chunkOffset[2] < 0)).toBe(true);
    expect(solidDraws.some((draw) => draw.chunkOffset[2] > 0)).toBe(false);
    expect(frame.fogEnd).toBeGreaterThan(frame.fogStart);
    expect(LightTexture.FULL_BLOCK).toBe(240);
    expect(LightTexture.block(LightTexture.FULL_BRIGHT)).toBe(15);
    expect(LightTexture.sky(LightTexture.FULL_BRIGHT)).toBe(15);
  });
});
