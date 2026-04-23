import { describe, expect, test } from "vitest";
import { getOverworldBiomeGenerationSettings } from "../../../src/worldgen/biome/overworld-biome-generation-settings.ts";
import { GenerationStep } from "../../../src/worldgen/levelgen/generation-step.ts";
import { CANYON, CAVE, OCEAN_CAVE } from "../../../src/worldgen/carver/overworld-configured-carvers.ts";

function getAirCarvers(key: string) {
  return getOverworldBiomeGenerationSettings(key).carvers(GenerationStep.Carving.AIR).map((supplier) => supplier());
}

describe("Overworld carver wiring", () => {
  test("supported land biomes keep the vanilla default overworld air-carver set", () => {
    expect(getAirCarvers("minecraft:forest")).toEqual([CAVE, CANYON]);
    expect(getAirCarvers("minecraft:swamp")).toEqual([CAVE, CANYON]);
  });

  test("ocean biomes fall back to the vanilla ocean air-carver set even without explicit decoration tables", () => {
    expect(getAirCarvers("minecraft:ocean")).toEqual([OCEAN_CAVE, CANYON]);
    expect(getAirCarvers("minecraft:deep_frozen_ocean")).toEqual([OCEAN_CAVE, CANYON]);
  });

  test("unsupported land biomes still retain default air carvers instead of falling back to EMPTY", () => {
    expect(getAirCarvers("minecraft:dark_forest")).toEqual([CAVE, CANYON]);
    expect(getAirCarvers("minecraft:jungle")).toEqual([CAVE, CANYON]);
  });

  test("liquid carvers are still unwired in the current TS port", () => {
    expect(getOverworldBiomeGenerationSettings("minecraft:ocean").carvers(GenerationStep.Carving.LIQUID)).toEqual([]);
    expect(getOverworldBiomeGenerationSettings("minecraft:forest").carvers(GenerationStep.Carving.LIQUID)).toEqual([]);
  });
});
