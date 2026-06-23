import { describe, expect, test } from "vitest";

import type { DecodedChunk } from "../../oracle/lib/anvil/chunk.ts";
import { NBT_TAG_COMPOUND, NBT_TAG_END, type NbtCompound } from "../../oracle/lib/anvil/nbt.ts";
import {
  blockStateKey,
  buildLiquidRegionFixture,
  decodeChunkLiquidTicks,
  normalizeLiquidTicks,
  type ScheduledLiquidTickFixture,
} from "../../oracle/lib/integration/liquid-fixture.ts";
import type { LiquidScenarioSpec } from "../../oracle/lib/integration/liquid-scenario.ts";

const BLOCKS_PER_SECTION = 16 * 16 * 16;

function scenario(overrides: Partial<LiquidScenarioSpec> = {}): LiquidScenarioSpec {
  return {
    scenario: "unit_water",
    seed: "12345",
    ticks: 5,
    bounds: { minX: 15, minY: 64, minZ: 0, sizeX: 2, sizeY: 1, sizeZ: 1 },
    commands: [],
    tickMargin: 1,
    minecraftVersion: "1.17.1",
    generator: "default",
    generateStructures: false,
    ...overrides,
  };
}

function fakeChunk(chunkX: number, blocks: readonly number[]): DecodedChunk {
  return {
    dataVersion: 2730,
    chunkX,
    chunkZ: 0,
    status: "full",
    isLightOn: true,
    sections: [
      {
        y: 4,
        palette: [
          { name: "minecraft:air" },
          { name: "minecraft:stone" },
          { name: "minecraft:water", properties: { level: "0" } },
          { name: "minecraft:water", properties: { level: "7" } },
        ],
        blocks,
      },
    ],
    light: { block: [], sky: [] },
    heightmaps: {},
    biomes: [],
  };
}

function sectionBlocks(entries: readonly { localX: number; localY: number; localZ: number; palette: number }[]): number[] {
  const blocks = new Array<number>(BLOCKS_PER_SECTION).fill(1);
  for (const entry of entries) {
    blocks[(entry.localY << 8) | (entry.localZ << 4) | entry.localX] = entry.palette;
  }
  return blocks;
}

function tick(overrides: Partial<ScheduledLiquidTickFixture> = {}): ScheduledLiquidTickFixture {
  return {
    x: 15,
    y: 64,
    z: 0,
    target: "minecraft:water",
    delay: 5,
    priority: "normal",
    ...overrides,
  };
}

describe("buildLiquidRegionFixture", () => {
  test("samples a bounded region across chunks while preserving block-state properties", () => {
    const fixture = buildLiquidRegionFixture(
      scenario(),
      [
        fakeChunk(0, sectionBlocks([{ localX: 15, localY: 0, localZ: 0, palette: 2 }])),
        fakeChunk(1, sectionBlocks([{ localX: 0, localY: 0, localZ: 0, palette: 3 }])),
      ],
      [],
    );

    expect(fixture.module).toBe("liquid-sim");
    expect(fixture.wireFormat.blockOrder).toBe("y-major,z-major,x-minor");
    expect(fixture.wireFormat.paletteEntries).toBe("block-state");
    expect(fixture.palette).toEqual([
      { name: "minecraft:water", properties: { level: "0" } },
      { name: "minecraft:water", properties: { level: "7" } },
    ]);
    expect(fixture.blocks).toEqual([0, 1]);
  });

  test("filters and normalizes persisted liquid ticks around the fixture bounds", () => {
    const fixture = buildLiquidRegionFixture(
      scenario(),
      [
        fakeChunk(0, sectionBlocks([{ localX: 15, localY: 0, localZ: 0, palette: 2 }])),
        fakeChunk(1, sectionBlocks([{ localX: 0, localY: 0, localZ: 0, palette: 3 }])),
      ],
      [
        {
          chunkX: 0,
          chunkZ: 0,
          ticks: [
            tick({ x: 14, delay: 3, priority: "high" }),
            tick({ x: 18, delay: 1 }),
          ],
        },
      ],
    );

    expect(fixture.liquidTicks).toEqual([tick({ x: 14, delay: 3, priority: "high" })]);
  });

  test("uses air for absent sections inside loaded chunks", () => {
    const fixture = buildLiquidRegionFixture(
      scenario({ bounds: { minX: 15, minY: 96, minZ: 0, sizeX: 1, sizeY: 1, sizeZ: 1 } }),
      [fakeChunk(0, sectionBlocks([]))],
      [],
    );

    expect(fixture.palette).toEqual([{ name: "minecraft:air" }]);
    expect(fixture.blocks).toEqual([0]);
  });
});

describe("decodeChunkLiquidTicks", () => {
  test("accepts vanilla empty LiquidTicks lists tagged as TAG_End", () => {
    expect(decodeChunkLiquidTicks({ Level: { LiquidTicks: { type: NBT_TAG_END, values: [] } } })).toEqual([]);
  });

  test("decodes Anvil Level.LiquidTicks compounds and maps vanilla priorities", () => {
    const root: NbtCompound = {
      Level: {
        LiquidTicks: {
          type: NBT_TAG_COMPOUND,
          values: [
            { i: "minecraft:water", x: 1, y: 64, z: 2, t: 0, p: -1 },
            { i: "minecraft:lava", x: 3, y: 10, z: 4, t: 30 },
          ],
        },
      },
    };

    expect(decodeChunkLiquidTicks(root)).toEqual([
      { target: "minecraft:water", x: 1, y: 64, z: 2, delay: 0, priority: "high" },
      { target: "minecraft:lava", x: 3, y: 10, z: 4, delay: 30, priority: "normal" },
    ]);
  });
});

describe("liquid fixture ordering helpers", () => {
  test("sorts block-state properties and scheduled ticks deterministically", () => {
    expect(blockStateKey({ name: "minecraft:water", properties: { falling: "true", level: "7" } })).toBe(
      "minecraft:water[falling=true,level=7]",
    );

    expect(normalizeLiquidTicks([
      tick({ x: 2, delay: 1 }),
      tick({ x: 1, delay: 1, priority: "high" }),
    ])).toEqual([
      tick({ x: 1, delay: 1, priority: "high" }),
      tick({ x: 2, delay: 1 }),
    ]);
  });
});
