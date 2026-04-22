import { describe, expect, test } from "vitest";
import { obfuscateBiomeZoomSeed } from "../../../src/worldgen/biome/biome-zoom.ts";

describe("obfuscateBiomeZoomSeed", () => {
  test("matches BiomeManager.obfuscateSeed for pinned oracle values", () => {
    expect(obfuscateBiomeZoomSeed(0n)).toBe(8794265229978523055n);
    expect(obfuscateBiomeZoomSeed(1n)).toBe(-6467378160175308932n);
    expect(obfuscateBiomeZoomSeed(12345n)).toBe(293737985876514017n);
    expect(obfuscateBiomeZoomSeed(-1n)).toBe(6759447113877070610n);
  });
});
