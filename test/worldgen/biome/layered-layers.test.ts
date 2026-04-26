import { describe, expect, test } from "vitest";
import { LazyArea, LazyAreaContext } from "../../../src/worldgen/biome/layered/area";
import { OceanMixerLayer } from "../../../src/worldgen/biome/layered/layers";

function createConstantArea(value: number): LazyArea {
  return new LazyArea(new Map(), 16, () => value);
}

describe("OceanMixerLayer", () => {
  test("collapses would-be deep warm ocean cells back to warm ocean", () => {
    const mixedArea = OceanMixerLayer.run(
      new LazyAreaContext(16, 12345n, 100),
      () => createConstantArea(47),
      () => createConstantArea(44),
    )();

    expect(mixedArea.get(0, 0)).toBe(44);
  });

  test("downgrades shore-adjacent warm ocean to lukewarm ocean", () => {
    const landArea = new LazyArea(
      new Map(),
      16,
      (x, z) => (x === 8 && z === 0 ? 1 : 44),
    );
    const mixedArea = OceanMixerLayer.run(
      new LazyAreaContext(16, 12345n, 100),
      () => landArea,
      () => createConstantArea(44),
    )();

    expect(mixedArea.get(0, 0)).toBe(45);
  });
});
