import { describe, expect, test } from "vitest";

import { BlockPos } from "../../../src/core/block-pos";
import { SectionPos } from "../../../src/core/section-pos";
import { LightLayer } from "../../../src/world/level/light-layer";
import { StaticRenderLevel } from "../../../src/world/level/static-render-level";
import { registerGeneratedRenderBlocks } from "../../../src/world/level/generated-render-blocks";
import type { BlockGetter } from "../../../src/world/level/block-getter";
import type { LightChunkGetter } from "../../../src/world/level/chunk/light-chunk-getter";
import { LevelLightEngine } from "../../../src/world/level/lighting/level-light-engine";
import { ChunkBlockId } from "../../../src/worldgen/chunk/chunk-block-buffer";

class TestLightChunkGetter implements LightChunkGetter {
  public readonly lightUpdates: Array<{ layer: LightLayer; section: SectionPos }> = [];

  public constructor(private readonly level: StaticRenderLevel) {}

  public getChunkForLighting(chunkX: number, chunkZ: number): BlockGetter | null {
    return this.level.getChunk(chunkX, chunkZ, false);
  }

  public onLightUpdate(layer: LightLayer, section: SectionPos): void {
    this.lightUpdates.push({ layer, section });
  }

  public getLevel(): StaticRenderLevel {
    return this.level;
  }
}

function createTestLevel() {
  const palette = registerGeneratedRenderBlocks();
  return {
    level: new StaticRenderLevel(palette.airState, 0, 0, 0, 32),
    air: palette.airState,
    stone: palette.blockStateById[ChunkBlockId.STONE]!,
    lava: palette.blockStateById[ChunkBlockId.LAVA]!,
  };
}

function runUntilIdle(engine: LevelLightEngine): void {
  for (let iteration = 0; iteration < 100; iteration++) {
    if (!engine.hasLightWork()) {
      return;
    }
    engine.runUpdates(100_000, true, false);
  }

  throw new Error("light engine did not become idle");
}

function prepareSection(level: StaticRenderLevel, engine: LevelLightEngine, sectionX: number, sectionY: number, sectionZ: number): void {
  level.getChunk(sectionX, sectionZ, true);
  engine.updateSectionStatus(SectionPos.of(sectionX, sectionY, sectionZ), false);
}

function buildStoneRoom(
  level: StaticRenderLevel,
  stone: ReturnType<typeof createTestLevel>["stone"],
  minX: number,
  maxX: number,
  minZ: number,
  maxZ: number,
  minY: number,
  roofY: number,
): void {
  for (let y = minY; y <= roofY; y++) {
    for (let x = minX; x <= maxX; x++) {
      level.setBlock(new BlockPos(x, y, minZ), stone);
      level.setBlock(new BlockPos(x, y, maxZ), stone);
    }
    for (let z = minZ; z <= maxZ; z++) {
      level.setBlock(new BlockPos(minX, y, z), stone);
      level.setBlock(new BlockPos(maxX, y, z), stone);
    }
  }

  for (let x = minX; x <= maxX; x++) {
    for (let z = minZ; z <= maxZ; z++) {
      level.setBlock(new BlockPos(x, roofY, z), stone);
    }
  }
}

describe("LevelLightEngine block light", () => {
  test("propagates and removes an emitting block source", () => {
    const { level, air, lava } = createTestLevel();
    const chunkGetter = new TestLightChunkGetter(level);
    const engine = new LevelLightEngine(chunkGetter, true, false);
    prepareSection(level, engine, 0, 0, 0);

    const source = new BlockPos(1, 1, 1);
    level.setBlock(source, lava);
    engine.onBlockEmissionIncrease(source, lava.getLightEmission());
    runUntilIdle(engine);

    const blockLight = engine.getLayerListener(LightLayer.BLOCK);
    expect(blockLight.getLightValue(source)).toBe(15);
    expect(blockLight.getLightValue(new BlockPos(2, 1, 1))).toBe(14);
    expect(blockLight.getLightValue(new BlockPos(3, 1, 1))).toBe(13);
    expect(chunkGetter.lightUpdates.some((update) => update.layer === LightLayer.BLOCK)).toBe(true);

    level.setBlock(source, air);
    engine.checkBlock(source);
    runUntilIdle(engine);

    expect(blockLight.getLightValue(source)).toBe(0);
    expect(blockLight.getLightValue(new BlockPos(2, 1, 1))).toBe(0);
  });

  test("propagates across active section boundaries", () => {
    const { level, lava } = createTestLevel();
    const chunkGetter = new TestLightChunkGetter(level);
    const engine = new LevelLightEngine(chunkGetter, true, false);
    prepareSection(level, engine, 0, 0, 0);
    prepareSection(level, engine, 1, 0, 0);

    const source = new BlockPos(15, 1, 1);
    level.setBlock(source, lava);
    engine.onBlockEmissionIncrease(source, lava.getLightEmission());
    runUntilIdle(engine);

    const blockLight = engine.getLayerListener(LightLayer.BLOCK);
    expect(blockLight.getLightValue(source)).toBe(15);
    expect(blockLight.getLightValue(new BlockPos(16, 1, 1))).toBe(14);
  });
});

describe("LevelLightEngine sky light", () => {
  test("keeps open vertical air columns at full sky light", () => {
    const { level } = createTestLevel();
    const chunkGetter = new TestLightChunkGetter(level);
    const engine = new LevelLightEngine(chunkGetter, false, true);
    prepareSection(level, engine, 0, 0, 0);
    engine.enableLightSources({ x: 0, z: 0 }, true);
    runUntilIdle(engine);

    const skyLight = engine.getLayerListener(LightLayer.SKY);
    expect(skyLight.getLightValue(new BlockPos(4, 15, 4))).toBe(15);
    expect(skyLight.getLightValue(new BlockPos(4, 0, 4))).toBe(15);
  });

  test("darkens blocks inside an opaque room", () => {
    const { level, stone } = createTestLevel();
    const chunkGetter = new TestLightChunkGetter(level);
    const engine = new LevelLightEngine(chunkGetter, false, true);
    prepareSection(level, engine, 0, 0, 0);
    buildStoneRoom(level, stone, 3, 5, 3, 5, 0, 14);
    engine.enableLightSources({ x: 0, z: 0 }, true);
    runUntilIdle(engine);

    const skyLight = engine.getLayerListener(LightLayer.SKY);
    expect(skyLight.getLightValue(new BlockPos(4, 15, 4))).toBe(15);
    expect(skyLight.getLightValue(new BlockPos(4, 13, 4))).toBe(0);
  });

  test("combines sky and block light for raw brightness", () => {
    const { level, lava, stone } = createTestLevel();
    const chunkGetter = new TestLightChunkGetter(level);
    const engine = new LevelLightEngine(chunkGetter, true, true);
    prepareSection(level, engine, 0, 0, 0);
    buildStoneRoom(level, stone, 4, 8, 4, 8, 0, 14);
    const source = new BlockPos(6, 1, 6);
    level.setBlock(source, lava);
    engine.enableLightSources({ x: 0, z: 0 }, true);
    engine.onBlockEmissionIncrease(source, lava.getLightEmission());
    runUntilIdle(engine);

    expect(engine.getRawBrightness(new BlockPos(6, 13, 6), 0)).toBe(3);
    expect(engine.getRawBrightness(new BlockPos(6, 0, 6), 0)).toBe(14);
  });
});
