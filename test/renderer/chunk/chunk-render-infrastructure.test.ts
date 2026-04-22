import { readFileSync } from "node:fs";
import path from "node:path";
import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../src/core/block-pos";
import { Direction } from "../../../src/core/direction";
import { Registry } from "../../../src/core/registry";
import { ResourceLocation } from "../../../src/core/resource-location";
import { BlockColors } from "../../../src/renderer/block/block-colors";
import { BlockRenderDispatcher } from "../../../src/renderer/block/block-render-dispatcher";
import { ChunkRenderDispatcher } from "../../../src/renderer/chunk/chunk-render-dispatcher";
import { RenderChunkRegion } from "../../../src/renderer/chunk/render-chunk-region";
import { VisGraph } from "../../../src/renderer/chunk/vis-graph";
import { ChunkBufferBuilderPack } from "../../../src/renderer/chunk-buffer-builder-pack";
import { LevelRenderer } from "../../../src/renderer/level-renderer";
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
import { AirBlock } from "../../../src/world/level/block/air-block";
import { Block } from "../../../src/world/level/block/block";
import { BlockBehaviour } from "../../../src/world/level/block/state/block-behaviour";
import type { BlockState } from "../../../src/world/level/block/state/block-state";
import { Material as BlockMaterial } from "../../../src/world/level/material/material";
import { StaticRenderLevel } from "../../../src/world/level/static-render-level";
import { Vec3 } from "../../../src/world/phys/vec3";

const ASSETS_ROOT = path.resolve(process.cwd(), "reference/minecraft-1.17.1/extracted/assets");

(globalThis as unknown as { GPUBufferUsage?: Record<string, number> }).GPUBufferUsage ??= {
  VERTEX: 1 << 0,
  INDEX: 1 << 1,
  UNIFORM: 1 << 2,
  COPY_DST: 1 << 3,
  MAP_READ: 1 << 4,
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

function createSpriteGetter(): (material: Material) => TextureAtlasSprite {
  const sprites = new Map<string, TextureAtlasSprite>();
  return (material) => {
    const key = material.texture().toString();
    const cached = sprites.get(key);
    if (cached !== undefined) {
      return cached;
    }

    const sprite = new TestSprite(material.texture());
    sprites.set(key, sprite);
    return sprite;
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

function createDispatcher(): BlockRenderDispatcher {
  const repository = new BlockModelRepository(new ExtractedAssetModelSource());
  const bakery = new ModelBakery(repository, createSpriteGetter());
  const modelManager = new ModelManager(bakery.getMissingBakedModel());
  bakery.bakeTopLevelBlockModels(modelManager);
  const shaper = new BlockModelShaper(modelManager);
  shaper.rebuildCache();
  return new BlockRenderDispatcher(shaper, new BlockColors());
}

function createFakeDevice(): GPUDevice {
  return {
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
  } as unknown as GPUDevice;
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
});
