import { describe, expect, test } from "vitest";

import {
  compareLiquidRegion,
  type BlockStateFixtureEntry,
  type LiquidRegionFixture,
  type ScheduledLiquidTickFixture,
} from "../../oracle/lib/integration/liquid-fixture.ts";

function fixture(
  overrides: Partial<LiquidRegionFixture> = {},
  palette: readonly BlockStateFixtureEntry[] = [{ name: "minecraft:air" }, { name: "minecraft:water", properties: { level: "0" } }],
): LiquidRegionFixture {
  return {
    module: "liquid-sim",
    minecraftVersion: "1.17.1",
    dataVersion: 2730,
    seed: "12345",
    generator: "default",
    generateStructures: false,
    scenario: "diff",
    ticks: 1,
    bounds: { minX: 10, minY: 64, minZ: 20, sizeX: 2, sizeY: 1, sizeZ: 1 },
    wireFormat: { blockOrder: "y-major,z-major,x-minor", paletteEntries: "block-state" },
    palette,
    blocks: [0, 1],
    liquidTicks: [],
    ...overrides,
  };
}

function tick(overrides: Partial<ScheduledLiquidTickFixture> = {}): ScheduledLiquidTickFixture {
  return {
    target: "minecraft:water",
    x: 11,
    y: 64,
    z: 20,
    delay: 1,
    priority: "normal",
    ...overrides,
  };
}

describe("compareLiquidRegion", () => {
  test("returns no diffs for equivalent block and tick fixtures", () => {
    expect(compareLiquidRegion(fixture(), fixture())).toEqual([]);
  });

  test("reports the first block mismatch with world coordinates", () => {
    expect(compareLiquidRegion(fixture(), fixture({ blocks: [1, 1] }))).toEqual([
      {
        kind: "block_mismatch",
        x: 10,
        y: 64,
        z: 20,
        expected: "minecraft:air",
        actual: "minecraft:water[level=0]",
      },
    ]);
  });

  test("reports persisted liquid tick differences", () => {
    expect(compareLiquidRegion(fixture({ liquidTicks: [tick()] }), fixture())).toEqual([
      { kind: "missing_tick", expected: "1|0|minecraft:water|11,64,20" },
    ]);

    expect(compareLiquidRegion(fixture(), fixture({ liquidTicks: [tick({ delay: 2 })] }))).toEqual([
      { kind: "extra_tick", actual: "2|0|minecraft:water|11,64,20" },
    ]);
  });

  test("short-circuits on bounds mismatch", () => {
    expect(compareLiquidRegion(fixture(), fixture({ bounds: { minX: 0, minY: 64, minZ: 20, sizeX: 2, sizeY: 1, sizeZ: 1 } }))).toEqual([
      {
        kind: "bounds_mismatch",
        expected: { minX: 10, minY: 64, minZ: 20, sizeX: 2, sizeY: 1, sizeZ: 1 },
        actual: { minX: 0, minY: 64, minZ: 20, sizeX: 2, sizeY: 1, sizeZ: 1 },
      },
    ]);
  });
});
