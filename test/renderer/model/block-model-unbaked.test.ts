import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, test } from "vitest";
import { Direction } from "../../../src/core/direction";
import { ResourceLocation } from "../../../src/core/resource-location";
import { BlockModel, GENERATION_MARKER, GuiLight } from "../../../src/renderer/model/block-model";
import { BlockModelRepository, type BlockModelSource } from "../../../src/renderer/model/block-model-repository";

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

function readModel(location: string): string {
  const parsed = new ResourceLocation(location);
  const filePath = path.join(ASSETS_ROOT, parsed.getNamespace(), "models", `${parsed.getPath()}.json`);
  return readFileSync(filePath, "utf8");
}

describe("Block model unbaked graph", () => {
  test("BlockModel parses elements, fills missing UVs, and keeps element rotation data", () => {
    const cube = BlockModel.fromString(readModel("minecraft:block/cube"));
    const rotated = BlockModel.fromString(readModel("minecraft:block/small_dripleaf_top"));
    const cubeElement = cube.getElements()[0]!;
    const northFace = cubeElement.faces.get(Direction.NORTH)!;
    const rotatedElement = rotated.getElements()[6]!;

    expect(cube.getElements()).toHaveLength(1);
    expect(northFace.cullForDirection).toBe(Direction.NORTH);
    expect(northFace.texture).toBe("#north");
    expect(northFace.uv.uvs).toEqual([0, 0, 16, 16]);
    expect(rotatedElement.rotation?.axis).toBe(Direction.Axis.Y);
    expect(rotatedElement.rotation?.angle).toBe(45);
    expect(rotatedElement.rotation?.origin.x()).toBe(0.5);
    expect(rotated.getElements()[2]!.faces.get(Direction.UP)!.uv.rotation).toBe(270);
  });

  test("BlockModelRepository resolves parent chains and texture references from extracted block models", () => {
    const repository = new BlockModelRepository(new ExtractedAssetModelSource());
    const stone = repository.resolveBlockModel(new ResourceLocation("minecraft:block/stone"));
    const oakLog = repository.resolveBlockModel(new ResourceLocation("minecraft:block/oak_log"));

    expect(stone.isResolved()).toBe(true);
    expect(stone.getRootModel().name).toBe("minecraft:block/block");
    expect(stone.getElements()).toHaveLength(1);
    expect(stone.getMaterial("particle").texture().toString()).toBe("minecraft:block/stone");
    expect(stone.getMaterial("north").texture().toString()).toBe("minecraft:block/stone");
    expect(oakLog.getMaterial("particle").texture().toString()).toBe("minecraft:block/oak_log");
    expect(oakLog.getMaterial("down").texture().toString()).toBe("minecraft:block/oak_log_top");
    expect(oakLog.getMaterial("up").texture().toString()).toBe("minecraft:block/oak_log_top");
    expect(oakLog.getMaterial("west").texture().toString()).toBe("minecraft:block/oak_log");
  });

  test("item models inherit display transforms and gui light through builtin/generated", () => {
    const repository = new BlockModelRepository(new ExtractedAssetModelSource());
    const cookedPorkchop = repository.resolveBlockModel(new ResourceLocation("minecraft:item/cooked_porkchop"));
    const transforms = cookedPorkchop.getTransforms();

    expect(cookedPorkchop.getRootModel()).toBe(GENERATION_MARKER);
    expect(cookedPorkchop.getGuiLight()).toBe(GuiLight.FRONT);
    expect(transforms.ground.translation.y()).toBeCloseTo(0.125);
    expect(transforms.firstPersonLeftHand.rotation.y()).toBe(-90);
    expect(transforms.firstPersonLeftHand.translation.x()).toBeCloseTo(1.13 / 16);
    expect(cookedPorkchop.getMaterial("layer0").texture().toString()).toBe("minecraft:item/cooked_porkchop");
  });
});
