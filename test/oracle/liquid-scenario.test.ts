import { describe, expect, test } from "vitest";

import {
  boundsMaxX,
  boundsMaxY,
  boundsMaxZ,
  normalizeLiquidScenario,
} from "../../oracle/lib/integration/liquid-scenario.ts";

describe("normalizeLiquidScenario", () => {
  test("fills vanilla defaults and exposes inclusive max bounds", () => {
    const scenario = normalizeLiquidScenario({
      scenario: "water_slope",
      seed: "12345",
      ticks: 5,
      bounds: { minX: -2, minY: 63, minZ: 4, sizeX: 3, sizeY: 2, sizeZ: 5 },
      commands: ["setblock 0 64 0 minecraft:water"],
    });

    expect(scenario.minecraftVersion).toBe("1.17.1");
    expect(scenario.generator).toBe("default");
    expect(scenario.generateStructures).toBe(false);
    expect(scenario.tickMargin).toBe(1);
    expect(boundsMaxX(scenario.bounds)).toBe(0);
    expect(boundsMaxY(scenario.bounds)).toBe(64);
    expect(boundsMaxZ(scenario.bounds)).toBe(8);
  });

  test("rejects commands that are not single mcfunction command lines", () => {
    expect(() =>
      normalizeLiquidScenario({
        scenario: "bad",
        seed: "12345",
        ticks: 0,
        bounds: { minX: 0, minY: 0, minZ: 0, sizeX: 1, sizeY: 1, sizeZ: 1 },
        commands: ["/setblock 0 0 0 minecraft:water"],
      })
    ).toThrow(/omit the leading slash/);

    expect(() =>
      normalizeLiquidScenario({
        scenario: "bad",
        seed: "12345",
        ticks: 0,
        bounds: { minX: 0, minY: 0, minZ: 0, sizeX: 1, sizeY: 1, sizeZ: 1 },
        commands: ["setblock 0 0 0 minecraft:water\nstop"],
      })
    ).toThrow(/single command line/);
  });
});
