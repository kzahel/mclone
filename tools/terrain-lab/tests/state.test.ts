import assert from "node:assert/strict";
import test from "node:test";

import {
  DEFAULT_TERRAIN_LAB_STATE,
  footprintBlocks,
  nextSpacing,
  panTerrainLabState,
  parseTerrainLabState,
  terrainLabSearch,
  validSeed,
} from "../src/state";

test("round-trips complete URL state", () => {
  const state = {
    seed: "-9223372036854775808",
    centerX: -1024,
    centerZ: 2048,
    spacing: 256 as const,
    source: "gpu" as const,
    view: "map" as const,
    layer: "continentalness" as const,
  };
  assert.deepEqual(parseTerrainLabState(terrainLabSearch(state)), state);
});

test("falls back independently for invalid URL fields", () => {
  assert.deepEqual(
    parseTerrainLabState(
      "?seed=nope&x=999999999999&z=-16&spacing=3&source=magic&view=3d&layer=height",
    ),
    {
      ...DEFAULT_TERRAIN_LAB_STATE,
      centerZ: -16,
      view: "3d",
      layer: "height",
    },
  );
});

test("validates the complete signed 64-bit seed range", () => {
  assert.equal(validSeed("9223372036854775807"), "9223372036854775807");
  assert.equal(validSeed("-9223372036854775808"), "-9223372036854775808");
  assert.equal(validSeed("9223372036854775808"), undefined);
  assert.equal(validSeed("1.5"), undefined);
});

test("zooms through power-of-two spacing and reports footprint", () => {
  assert.equal(nextSpacing(2, "in"), 2);
  assert.equal(nextSpacing(32, "in"), 16);
  assert.equal(nextSpacing(32, "out"), 64);
  assert.equal(nextSpacing(1024, "out"), 1024);
  assert.equal(footprintBlocks({ spacing: 256 }), 16_384);
});

test("pans on the active sample lattice", () => {
  const moved = panTerrainLabState(DEFAULT_TERRAIN_LAB_STATE, 79, -47);
  assert.equal(Math.abs(moved.centerX % moved.spacing), 0);
  assert.equal(Math.abs(moved.centerZ % moved.spacing), 0);
  assert.equal(moved.centerX, -224);
  assert.equal(moved.centerZ, 288);
});
