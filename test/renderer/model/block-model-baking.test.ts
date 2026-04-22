import { readFileSync } from "node:fs";
import path from "node:path";
import { beforeEach, describe, expect, test } from "vitest";
import { Direction } from "../../../src/core/direction";
import { Registry } from "../../../src/core/registry";
import { ResourceLocation } from "../../../src/core/resource-location";
import { RotatedPillarBlock } from "../../../src/world/level/block/rotated-pillar-block";
import { BlockBehaviour } from "../../../src/world/level/block/state/block-behaviour";
import { Material as BlockMaterial } from "../../../src/world/level/material/material";
import { ModelBakery } from "../../../src/renderer/model/model-bakery";
import { BlockModelRepository, type BlockModelSource } from "../../../src/renderer/model/block-model-repository";
import { BlockModelRotation } from "../../../src/renderer/model/block-model-rotation";
import { BlockModelShaper } from "../../../src/renderer/model/block-model-shaper";
import { Material } from "../../../src/renderer/model/material";
import { ModelManager } from "../../../src/renderer/model/model-manager";
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
    if (cached) {
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

function quadVertices(quad: readonly number[]): {
  readonly position: readonly [number, number, number];
  readonly uv: readonly [number, number];
}[] {
  return Array.from({ length: 4 }, (_, index) => {
    const vertexIndex = index * 8;
    return {
      position: [
        intBitsToFloat(quad[vertexIndex]!),
        intBitsToFloat(quad[vertexIndex + 1]!),
        intBitsToFloat(quad[vertexIndex + 2]!),
      ] as const,
      uv: [
        intBitsToFloat(quad[vertexIndex + 4]!),
        intBitsToFloat(quad[vertexIndex + 5]!),
      ] as const,
    };
  });
}

describe("Block model baking", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  test("ModelBakery bakes the stone cube into six culled quads with Minecraft's vertex packing order", () => {
    const repository = new BlockModelRepository(new ExtractedAssetModelSource());
    const bakery = new ModelBakery(repository, createSpriteGetter());
    const bakedModel = bakery.bake(new ResourceLocation("minecraft:block/stone"), BlockModelRotation.X0_Y0);

    expect(bakery.bake(new ResourceLocation("minecraft:block/stone"), BlockModelRotation.X0_Y0)).toBe(bakedModel);
    expect(bakedModel.getQuads(undefined, undefined)).toHaveLength(0);

    for (const direction of Direction.values()) {
      expect(bakedModel.getQuads(undefined, direction)).toHaveLength(1);
    }

    const northQuad = bakedModel.getQuads(undefined, Direction.NORTH)[0]!;
    expect(northQuad.getDirection()).toBe(Direction.NORTH);
    expect(northQuad.getTintIndex()).toBe(-1);
    expect(northQuad.isShade()).toBe(true);
    expect(northQuad.getSprite().getName().toString()).toBe("minecraft:block/stone");
    expect(northQuad.getVertices()).toHaveLength(32);
    expect(quadVertices(northQuad.getVertices())).toEqual([
      { position: [1, 1, 0], uv: [0, 0] },
      { position: [1, 0, 0], uv: [0, 1] },
      { position: [0, 0, 0], uv: [1, 1] },
      { position: [0, 1, 0], uv: [1, 0] },
    ]);
  });

  test("BlockModelShaper maps registered block states to baked models by ModelResourceLocation", () => {
    const repository = new BlockModelRepository(new ExtractedAssetModelSource());
    const bakery = new ModelBakery(repository, createSpriteGetter());
    const modelManager = new ModelManager(bakery.getMissingBakedModel());
    const block = new RotatedPillarBlock(BlockBehaviour.Properties.of(BlockMaterial.WOOD)).setLocation(new ResourceLocation("mclone:test_log"));
    Registry.register(Registry.BLOCK, block.getLocation()!, block);

    const axisYState = block.defaultBlockState();
    const axisXState = axisYState.setValue(RotatedPillarBlock.AXIS, Direction.Axis.X);
    const yModel = bakery.bake(new ResourceLocation("minecraft:block/stone"));
    const xModel = bakery.bake(new ResourceLocation("minecraft:block/oak_log"));

    expect(BlockModelShaper.stateToModelLocation(axisYState).toString()).toBe("mclone:test_log#axis=y");
    expect(BlockModelShaper.stateToModelLocation(axisXState).toString()).toBe("mclone:test_log#axis=x");

    modelManager.setModel(BlockModelShaper.stateToModelLocation(axisYState), yModel);
    modelManager.setModel(BlockModelShaper.stateToModelLocation(axisXState), xModel);

    const shaper = new BlockModelShaper(modelManager);
    shaper.rebuildCache();

    expect(shaper.getBlockModel(axisYState)).toBe(yModel);
    expect(shaper.getBlockModel(axisXState)).toBe(xModel);
    expect(shaper.getParticleIcon(axisXState).getName().toString()).toBe("minecraft:block/oak_log");
  });
});
