import { readFileSync } from "node:fs";
import path from "node:path";
import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../src/core/block-pos";
import { Registry } from "../../../src/core/registry";
import { ResourceLocation } from "../../../src/core/resource-location";
import { BlockColors } from "../../../src/renderer/block/block-colors";
import { BlockRenderDispatcher } from "../../../src/renderer/block/block-render-dispatcher";
import { BlockModelRepository, type BlockModelSource } from "../../../src/renderer/model/block-model-repository";
import { ModelBakery } from "../../../src/renderer/model/model-bakery";
import { Material } from "../../../src/renderer/model/material";
import { ModelManager } from "../../../src/renderer/model/model-manager";
import { TextureAtlasSprite, TextureAtlasSpriteInfo, type TextureAtlasUploadTarget } from "../../../src/renderer/texture/texture-atlas-sprite";
import { AnimationMetadataSection } from "../../../src/renderer/texture/animation-metadata-section";
import { NativeImage } from "../../../src/renderer/texture/native-image";
import { BufferBuilder, type PoppedBuffer } from "../../../src/renderer/vertex/buffer-builder";
import { DefaultVertexFormat } from "../../../src/renderer/vertex/default-vertex-format";
import { PoseStack } from "../../../src/renderer/vertex/pose-stack";
import { VertexFormat } from "../../../src/renderer/vertex/vertex-format";
import { StaticBlockAndTintGetter } from "../../../src/world/level/static-block-and-tint-getter";
import { Block } from "../../../src/world/level/block/block";
import { BlockBehaviour } from "../../../src/world/level/block/state/block-behaviour";
import type { BlockState } from "../../../src/world/level/block/state/block-state";
import { BlockModelShaper } from "../../../src/renderer/model/block-model-shaper";
import { Material as BlockMaterial } from "../../../src/world/level/material/material";

const ASSETS_ROOT = path.resolve(process.cwd(), "reference/minecraft-1.17.1/extracted/assets");

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
  return new Block(properties).defaultBlockState();
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

function renderBlock(
  dispatcher: BlockRenderDispatcher,
  state: BlockState,
  pos: BlockPos,
  level: StaticBlockAndTintGetter,
): PoppedBuffer {
  const builder = new BufferBuilder(256);
  builder.begin(VertexFormat.Mode.QUADS, DefaultVertexFormat.BLOCK);
  const poseStack = new PoseStack();
  poseStack.pushPose();
  poseStack.translate(pos.getX(), pos.getY(), pos.getZ());
  expect(dispatcher.renderBatched(state, pos, level, poseStack, builder, true)).toBe(true);
  poseStack.popPose();
  builder.end();
  return builder.popNextBuffer();
}

function upFaceRedBytes(buffer: Uint8Array): readonly number[] {
  const upFaceStart = 4 * DefaultVertexFormat.BLOCK.getVertexSize();
  return [
    buffer[upFaceStart + 12]!,
    buffer[upFaceStart + 44]!,
    buffer[upFaceStart + 76]!,
    buffer[upFaceStart + 108]!,
  ];
}

describe("Block tesselation", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  test("isolated stone block tesselates into six quads in BLOCK format", () => {
    const stone = createBlock("minecraft:stone", BlockMaterial.STONE);
    const dispatcher = createDispatcher();
    const pos = new BlockPos(0, 0, 0);
    const level = new StaticBlockAndTintGetter(createAirState());
    level.setBlock(pos, stone.defaultBlockState());

    const popped = renderBlock(dispatcher, stone.defaultBlockState(), pos, level);

    expect(popped.drawState.format()).toBe(DefaultVertexFormat.BLOCK);
    expect(popped.drawState.mode()).toBe(VertexFormat.Mode.QUADS);
    expect(popped.drawState.vertexCount()).toBe(24);
    expect(popped.drawState.indexCount()).toBe(36);
    expect(popped.drawState.sequentialIndex()).toBe(true);
    expect(popped.buffer).toHaveLength(24 * DefaultVertexFormat.BLOCK.getVertexSize());
  });

  test("adjacent solid block culls the shared face", () => {
    const stone = createBlock("minecraft:stone", BlockMaterial.STONE);
    const dispatcher = createDispatcher();
    const pos = new BlockPos(0, 0, 0);
    const neighborPos = new BlockPos(1, 0, 0);
    const level = new StaticBlockAndTintGetter(createAirState());
    level.setBlock(pos, stone.defaultBlockState());
    level.setBlock(neighborPos, stone.defaultBlockState());

    const popped = renderBlock(dispatcher, stone.defaultBlockState(), pos, level);

    expect(popped.drawState.vertexCount()).toBe(20);
    expect(popped.drawState.indexCount()).toBe(30);
    expect(popped.buffer).toHaveLength(20 * DefaultVertexFormat.BLOCK.getVertexSize());
  });

  test("emissive stone keeps a flat top-face shade where AO stone varies per vertex under corner occlusion", () => {
    const pos = new BlockPos(0, 0, 0);

    const aoStone = createBlock("minecraft:stone", BlockMaterial.STONE);
    let dispatcher = createDispatcher();
    let level = new StaticBlockAndTintGetter(createAirState());
    level.setBlock(pos, aoStone.defaultBlockState());
    level.setBlock(pos.above().east(), aoStone.defaultBlockState());
    level.setBlock(pos.above().north(), aoStone.defaultBlockState());
    level.setBlock(pos.above().north().east(), aoStone.defaultBlockState());
    const aoBuffer = renderBlock(dispatcher, aoStone.defaultBlockState(), pos, level).buffer;

    Registry.BLOCK.clear();
    const emissiveStone = createBlock("minecraft:stone", BlockMaterial.STONE, (properties) => {
      properties.lightLevel(() => 1);
    });
    dispatcher = createDispatcher();
    level = new StaticBlockAndTintGetter(createAirState());
    level.setBlock(pos, emissiveStone.defaultBlockState());
    level.setBlock(pos.above().east(), emissiveStone.defaultBlockState());
    level.setBlock(pos.above().north(), emissiveStone.defaultBlockState());
    level.setBlock(pos.above().north().east(), emissiveStone.defaultBlockState());
    const flatBuffer = renderBlock(dispatcher, emissiveStone.defaultBlockState(), pos, level).buffer;
    const aoTopFace = upFaceRedBytes(aoBuffer);
    const flatTopFace = upFaceRedBytes(flatBuffer);

    expect(new Set(flatTopFace).size).toBe(1);
    expect(new Set(aoTopFace).size).toBeGreaterThan(1);
    expect(aoBuffer).not.toEqual(flatBuffer);
  });
});
