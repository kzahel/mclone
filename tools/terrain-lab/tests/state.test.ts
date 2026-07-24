import assert from "node:assert/strict";
import test from "node:test";

import {
  DEFAULT_TERRAIN_LAB_STATE,
  DEFAULT_TERRAIN_LAB_CAMERA,
  REVIEW_TERRAIN_LAB_STATE,
  footprintBlocks,
  grabPanTerrainLabState,
  nextBlocksAcross,
  orbitTerrainLabCamera,
  panTerrainLabState,
  parseTerrainLabState,
  terrainLabSearch,
  validSeed,
  zoomTerrainLabState,
} from "../src/state";

test("round-trips complete URL state", () => {
  const state = {
    seed: "-9223372036854775808",
    centerX: -1024,
    centerZ: 2048,
    blocksAcross: 16_384,
    detail: 16 as const,
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

test("zooms independently from detail and reports footprint", () => {
  assert.equal(nextBlocksAcross(64, "in"), 64);
  assert.equal(nextBlocksAcross(2_048, "in"), 1_024);
  assert.equal(nextBlocksAcross(2_048, "out"), 4_096);
  assert.equal(nextBlocksAcross(131_072, "out"), 131_072);
  assert.equal(footprintBlocks({ blocksAcross: 16_384 }), 16_384);
});

test("pans continuously rather than snapping to the detail lattice", () => {
  const moved = panTerrainLabState(DEFAULT_TERRAIN_LAB_STATE, 79, -47);
  assert.equal(moved.centerX, -225);
  assert.equal(moved.centerZ, 289);
});

test("map grab follows the pointer on both screen axes", () => {
  const moved = grabPanTerrainLabState(
    DEFAULT_TERRAIN_LAB_STATE,
    100,
    75,
    800,
    600,
    4 / 3,
  );
  assert.equal(moved.centerX, -560);
  assert.equal(moved.centerZ, 144);
});

test("anchors map zoom under the pointer", () => {
  const zoomed = zoomTerrainLabState(DEFAULT_TERRAIN_LAB_STATE, 0.5, 0.5, -0.5, 2);
  assert.equal(zoomed.blocksAcross, 1_024);
  assert.equal(zoomed.centerX, 208);
  assert.equal(zoomed.centerZ, 80);
});

test("accepts old spacing links without changing their visible footprint", () => {
  const legacy = parseTerrainLabState("?spacing=32");
  assert.equal(legacy.blocksAcross, 2_048);
  assert.equal(legacy.detail, 32);
});

test("defaults to production truth and gives the fixed comparison site a name", () => {
  assert.equal(DEFAULT_TERRAIN_LAB_STATE.source, "reference");
  assert.equal(REVIEW_TERRAIN_LAB_STATE.seed, "-98765");
  assert.equal(REVIEW_TERRAIN_LAB_STATE.source, "split");
});

test("orbits independently from URL-addressed terrain state", () => {
  const camera = orbitTerrainLabCamera(
    DEFAULT_TERRAIN_LAB_CAMERA,
    200,
    -100,
    800,
    600,
  );
  assert.ok(camera.yaw > DEFAULT_TERRAIN_LAB_CAMERA.yaw);
  assert.ok(camera.pitch < DEFAULT_TERRAIN_LAB_CAMERA.pitch);
  assert.deepEqual(DEFAULT_TERRAIN_LAB_STATE, {
    seed: "-98765",
    centerX: -304,
    centerZ: 336,
    blocksAcross: 2_048,
    detail: "auto",
    source: "reference",
    view: "3d",
    layer: "terrain",
  });
});
