import { readFileSync } from "node:fs";
import path from "node:path";
import { beforeEach, describe, expect, test } from "vitest";
import { Direction } from "../../../src/core/direction";
import { Registry } from "../../../src/core/registry";
import { ResourceLocation } from "../../../src/core/resource-location";
import { Block } from "../../../src/world/level/block/block";
import { BlockBehaviour } from "../../../src/world/level/block/state/block-behaviour";
import { BlockState } from "../../../src/world/level/block/state/block-state";
import { BooleanProperty } from "../../../src/world/level/block/state/properties/boolean-property";
import { StateDefinition } from "../../../src/world/level/block/state/state-definition";
import { Material as BlockMaterial } from "../../../src/world/level/material/material";
import { RotatedPillarBlock } from "../../../src/world/level/block/rotated-pillar-block";
import { BlockModelRepository, type BlockModelSource } from "../../../src/renderer/model/block-model-repository";
import { BlockModelShaper } from "../../../src/renderer/model/block-model-shaper";
import { ModelBakery } from "../../../src/renderer/model/model-bakery";
import { Material } from "../../../src/renderer/model/material";
import { ModelManager } from "../../../src/renderer/model/model-manager";
import { MultiPartBakedModel } from "../../../src/renderer/model/multi-part-baked-model";
import { WeightedBakedModel } from "../../../src/renderer/model/weighted-baked-model";
import { AnimationMetadataSection } from "../../../src/renderer/texture/animation-metadata-section";
import { NativeImage } from "../../../src/renderer/texture/native-image";
import { TextureAtlasSprite, TextureAtlasSpriteInfo, type TextureAtlasUploadTarget } from "../../../src/renderer/texture/texture-atlas-sprite";

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

class TestFenceBlock extends Block {
  public static readonly NORTH = BooleanProperty.create("north");
  public static readonly EAST = BooleanProperty.create("east");
  public static readonly SOUTH = BooleanProperty.create("south");
  public static readonly WEST = BooleanProperty.create("west");

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(
      this.defaultBlockState()
        .setValue(TestFenceBlock.NORTH, false)
        .setValue(TestFenceBlock.EAST, false)
        .setValue(TestFenceBlock.SOUTH, false)
        .setValue(TestFenceBlock.WEST, false),
    );
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(TestFenceBlock.NORTH, TestFenceBlock.EAST, TestFenceBlock.SOUTH, TestFenceBlock.WEST);
  }
}

class FixedRandom {
  public constructor(private readonly value: number) {}

  public nextLong(): number {
    return this.value;
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

function intBitsToFloat(value: number): number {
  const bytes = new ArrayBuffer(4);
  const view = new DataView(bytes);
  view.setInt32(0, value, true);
  return view.getFloat32(0, true);
}

function quadUvs(model: { getQuads(state?: BlockState, direction?: Direction, random?: unknown): readonly { getVertices(): readonly number[] }[] }, state: BlockState, direction: Direction, random?: unknown): readonly [number, number][] {
  const quad = model.getQuads(state, direction, random)[0]!;
  return Array.from({ length: 4 }, (_, index) => {
    const vertexIndex = index * 8;
    return [
      intBitsToFloat(quad.getVertices()[vertexIndex + 4]!),
      intBitsToFloat(quad.getVertices()[vertexIndex + 5]!),
    ] as const;
  });
}

function countDirectionalQuads(model: { getQuads(state?: BlockState, direction?: Direction, random?: unknown): readonly unknown[] }, state: BlockState): number {
  return Direction.values().reduce((sum, direction) => sum + model.getQuads(state, direction).length, 0);
}

describe("Blockstate-driven model baking", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  test("ModelBakery resolves weighted empty variants for blocks without properties", () => {
    const repository = new BlockModelRepository(new ExtractedAssetModelSource());
    const bakery = new ModelBakery(repository, createSpriteGetter());
    const stone = new Block(BlockBehaviour.Properties.of(BlockMaterial.STONE)).setLocation(new ResourceLocation("minecraft:stone"));
    Registry.register(Registry.BLOCK, stone.getLocation()!, stone);

    const bakedModel = bakery.bake(BlockModelShaper.stateToModelLocation(stone.defaultBlockState()));
    expect(bakedModel).toBeInstanceOf(WeightedBakedModel);

    const firstUvs = quadUvs(bakedModel, stone.defaultBlockState(), Direction.NORTH, new FixedRandom(0));
    const secondUvs = quadUvs(bakedModel, stone.defaultBlockState(), Direction.NORTH, new FixedRandom(1));
    expect(firstUvs).not.toEqual(secondUvs);
  });

  test("bakeTopLevelBlockModels populates ModelManager with blockstate variants and multipart models", () => {
    const repository = new BlockModelRepository(new ExtractedAssetModelSource());
    const bakery = new ModelBakery(repository, createSpriteGetter());
    const modelManager = new ModelManager(bakery.getMissingBakedModel());
    const stone = new Block(BlockBehaviour.Properties.of(BlockMaterial.STONE)).setLocation(new ResourceLocation("minecraft:stone"));
    const oakLog = new RotatedPillarBlock(BlockBehaviour.Properties.of(BlockMaterial.WOOD)).setLocation(new ResourceLocation("minecraft:oak_log"));
    const oakFence = new TestFenceBlock(BlockBehaviour.Properties.of(BlockMaterial.WOOD)).setLocation(new ResourceLocation("minecraft:oak_fence"));
    Registry.register(Registry.BLOCK, stone.getLocation()!, stone);
    Registry.register(Registry.BLOCK, oakLog.getLocation()!, oakLog);
    Registry.register(Registry.BLOCK, oakFence.getLocation()!, oakFence);

    bakery.bakeTopLevelBlockModels(modelManager);

    const shaper = new BlockModelShaper(modelManager);
    shaper.rebuildCache();

    const axisYState = oakLog.defaultBlockState();
    const axisXState = axisYState.setValue(RotatedPillarBlock.AXIS, Direction.Axis.X);
    const axisYModel = shaper.getBlockModel(axisYState);
    const axisXModel = shaper.getBlockModel(axisXState);
    expect(axisYModel.getParticleIcon().getName().toString()).toBe("minecraft:block/oak_log");
    expect(axisXModel.getParticleIcon().getName().toString()).toBe("minecraft:block/oak_log");
    expect(axisYModel.getQuads(axisYState, Direction.NORTH)[0]!.getVertices()).not.toEqual(
      axisXModel.getQuads(axisXState, Direction.NORTH)[0]!.getVertices(),
    );

    const isolatedFenceState = oakFence.defaultBlockState();
    const connectedFenceState = isolatedFenceState
      .setValue(TestFenceBlock.NORTH, true)
      .setValue(TestFenceBlock.EAST, true);
    const connectedFenceModel = shaper.getBlockModel(connectedFenceState);
    expect(connectedFenceModel).toBeInstanceOf(MultiPartBakedModel);
    expect(connectedFenceModel.getParticleIcon().getName().toString()).toBe("minecraft:block/oak_planks");
    expect(countDirectionalQuads(connectedFenceModel, connectedFenceState)).toBeGreaterThan(
      countDirectionalQuads(shaper.getBlockModel(isolatedFenceState), isolatedFenceState),
    );
    expect(connectedFenceModel.getQuads(connectedFenceState, Direction.NORTH)).not.toHaveLength(0);
    expect(connectedFenceModel.getQuads(connectedFenceState, Direction.EAST)).not.toHaveLength(0);
  });
});
