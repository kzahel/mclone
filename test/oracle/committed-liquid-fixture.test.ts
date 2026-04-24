import { describe, expect, test } from "vitest";
import committed from "../fixtures/liquid/water-slope-10-ticks.json" with { type: "json" };

interface CommittedBlockState {
  readonly name: string;
  readonly properties?: Readonly<Record<string, string>>;
}

interface CommittedLiquidTick {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly target: string;
  readonly delay: number;
  readonly priority: string;
}

interface CommittedLiquidFixture {
  readonly module: string;
  readonly minecraftVersion: string;
  readonly dataVersion: number;
  readonly seed: string;
  readonly generator: string;
  readonly generateStructures: boolean;
  readonly scenario: string;
  readonly ticks: number;
  readonly bounds: {
    readonly minX: number;
    readonly minY: number;
    readonly minZ: number;
    readonly sizeX: number;
    readonly sizeY: number;
    readonly sizeZ: number;
  };
  readonly wireFormat: {
    readonly blockOrder: string;
    readonly paletteEntries: string;
  };
  readonly palette: readonly CommittedBlockState[];
  readonly blocks: readonly number[];
  readonly liquidTicks: readonly CommittedLiquidTick[];
}

const fixture = committed as CommittedLiquidFixture;

describe("committed liquid simulation fixture", () => {
  test("metadata identifies the pinned server scenario", () => {
    expect(fixture.module).toBe("liquid-sim");
    expect(fixture.minecraftVersion).toBe("1.17.1");
    expect(fixture.dataVersion).toBe(2730);
    expect(fixture.seed).toBe("12345");
    expect(fixture.generator).toBe("default");
    expect(fixture.generateStructures).toBe(false);
    expect(fixture.scenario).toBe("water_slope_10_ticks");
    expect(fixture.ticks).toBe(10);
    expect(fixture.wireFormat.blockOrder).toBe("y-major,z-major,x-minor");
    expect(fixture.wireFormat.paletteEntries).toBe("block-state");
  });

  test("preserves liquid block-state levels in the bounded region palette", () => {
    expect(fixture.blocks).toHaveLength(fixture.bounds.sizeX * fixture.bounds.sizeY * fixture.bounds.sizeZ);
    expect(fixture.palette).toContainEqual({ name: "minecraft:water", properties: { level: "0" } });
    expect(fixture.palette).toContainEqual({ name: "minecraft:water", properties: { level: "1" } });
    expect(fixture.palette).toContainEqual({ name: "minecraft:water", properties: { level: "2" } });
    for (const blockIndex of fixture.blocks) {
      expect(blockIndex).toBeGreaterThanOrEqual(0);
      expect(blockIndex).toBeLessThan(fixture.palette.length);
    }
  });

  test("captures persisted future liquid ticks inside the tick margin", () => {
    expect(fixture.liquidTicks).toEqual([
      { x: 2, y: 80, z: 2, target: "minecraft:flowing_water", delay: 5, priority: "normal" },
      { x: 3, y: 80, z: 2, target: "minecraft:flowing_water", delay: 5, priority: "normal" },
    ]);
  });
});
