import assert from "node:assert/strict";
import test from "node:test";

import {
  DEFAULT_TERRAIN_LAB_STATE,
  DEFAULT_TERRAIN_LAB_CAMERA,
  REVIEW_TERRAIN_LAB_STATE,
  arrowPanTerrainLabState,
  footprintBlocks,
  grabPanTerrainLabState,
  grabPanTerrainLabStateInView,
  nextBlocksAcross,
  orbitTerrainLabCamera,
  panTerrainLabState,
  pinchPanZoomTerrainLabState,
  proceduralSourceForPanes,
  parseTerrainLabState,
  terrainLabSearch,
  validSeed,
  toggleTerrainLabPane,
  zoomTerrainLabState,
  type TerrainLabState,
} from "../src/state";

test("round-trips complete URL state", () => {
  const state: TerrainLabState = {
    seed: "-9223372036854775808",
    centerX: -1024,
    centerZ: 2048,
    blocksAcross: 16_384,
    detail: 16 as const,
    source: "gpu" as const,
    panes: ["canonical", "gpu"],
    canonicalStage: "surface" as const,
    canonicalRadius: 1,
    waterVisible: false,
    vegetationVisible: true,
    contentStage: "cover" as const,
    view: "map" as const,
    layer: "continentalness" as const,
  };
  assert.deepEqual(parseTerrainLabState(terrainLabSearch(state)), {
    ...state,
    panes: [...state.panes],
  });
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
  assert.equal(moved.centerX, -368);
  assert.equal(moved.centerZ, 288);
});

test("3d grab pans in the camera ground plane", () => {
  const state = { ...DEFAULT_TERRAIN_LAB_STATE, view: "3d" as const };
  const moved = grabPanTerrainLabStateInView(
    state,
    { yaw: 0, pitch: 0.5 },
    100,
    75,
    800,
    600,
    4 / 3,
  );
  assert.equal(moved.centerX, -256);
  assert.equal(moved.centerZ, 400);
});

test("arrow keys pan an eighth of the visible footprint", () => {
  assert.equal(
    arrowPanTerrainLabState(DEFAULT_TERRAIN_LAB_STATE, "ArrowRight").centerX,
    -240,
  );
  assert.equal(
    arrowPanTerrainLabState(DEFAULT_TERRAIN_LAB_STATE, "ArrowUp").centerZ,
    272,
  );
  assert.equal(
    arrowPanTerrainLabState(DEFAULT_TERRAIN_LAB_STATE, "Enter"),
    DEFAULT_TERRAIN_LAB_STATE,
  );
});

test("anchors map zoom under the pointer", () => {
  const zoomed = zoomTerrainLabState(DEFAULT_TERRAIN_LAB_STATE, 0.5, 0.5, -0.5, 2);
  assert.equal(zoomed.blocksAcross, 256);
  assert.equal(zoomed.centerX, -176);
  assert.equal(zoomed.centerZ, 272);
});

test("two-finger map gesture pans and zooms from one start state", () => {
  const moved = pinchPanZoomTerrainLabState(
    { ...DEFAULT_TERRAIN_LAB_STATE, view: "map" },
    DEFAULT_TERRAIN_LAB_CAMERA,
    0.5,
    -0.25,
    0.25,
    80,
    60,
    800,
    600,
    4 / 3,
  );
  assert.equal(moved.blocksAcross, 256);
  assert.equal(moved.centerX, -394);
  assert.equal(moved.centerZ, 365);
});

test("two-finger 3d gesture pans in the captured camera plane while zooming", () => {
  const moved = pinchPanZoomTerrainLabState(
    { ...DEFAULT_TERRAIN_LAB_STATE, view: "3d" },
    { yaw: 0, pitch: 0.5 },
    0.5,
    -0.25,
    0.25,
    80,
    60,
    800,
    600,
    4 / 3,
  );
  assert.equal(moved.blocksAcross, 256);
  assert.equal(moved.centerX, -285);
  assert.equal(moved.centerZ, 362);
});

test("accepts old spacing links without changing their visible footprint", () => {
  const legacy = parseTerrainLabState("?spacing=32");
  assert.equal(legacy.blocksAcross, 2_048);
  assert.equal(legacy.detail, 32);
});

test("accepts bounded 49- and 81-chunk exact footprints", () => {
  assert.equal(parseTerrainLabState("?radius=3").canonicalRadius, 3);
  assert.equal(parseTerrainLabState("?radius=4").canonicalRadius, 4);
  assert.equal(parseTerrainLabState("?radius=5").canonicalRadius, 2);
});

test("defaults to the three-pane workspace and gives the review site a name", () => {
  assert.equal(DEFAULT_TERRAIN_LAB_STATE.source, "split");
  assert.deepEqual(DEFAULT_TERRAIN_LAB_STATE.panes, ["canonical", "cpu", "gpu"]);
  assert.equal(REVIEW_TERRAIN_LAB_STATE.seed, "-98765");
  assert.equal(REVIEW_TERRAIN_LAB_STATE.source, "split");
});

test("maps legacy source links and keeps at least one pane visible", () => {
  assert.deepEqual(parseTerrainLabState("?source=gpu").panes, ["gpu"]);
  assert.deepEqual(parseTerrainLabState("?source=split").panes, ["cpu", "gpu"]);
  assert.equal(proceduralSourceForPanes(["canonical"]), "reference");
  const onlyCanonical: TerrainLabState = {
    ...DEFAULT_TERRAIN_LAB_STATE,
    panes: ["canonical"],
  };
  assert.equal(toggleTerrainLabPane(onlyCanonical, "canonical"), onlyCanonical);
  assert.deepEqual(toggleTerrainLabPane(onlyCanonical, "gpu").panes, ["canonical", "gpu"]);
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
    blocksAcross: 512,
    detail: "auto",
    source: "split",
    panes: ["canonical", "cpu", "gpu"],
    canonicalStage: "final",
    canonicalRadius: 2,
    waterVisible: true,
    vegetationVisible: true,
    contentStage: "hydrology",
    view: "3d",
    layer: "terrain",
  });
});
