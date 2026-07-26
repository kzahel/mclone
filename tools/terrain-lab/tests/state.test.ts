import assert from "node:assert/strict";
import test from "node:test";

import {
  DEFAULT_TERRAIN_LAB_STATE,
  REVIEW_TERRAIN_LAB_STATE,
  canonicalTerrainCenterChunk,
  footprintBlocks,
  proceduralSourceForPanes,
  parseTerrainLabState,
  switchTerrainLabProfile,
  terrainLabSearch,
  validSeed,
  toggleTerrainLabPane,
  type TerrainLabState,
} from "../src/state";

test("round-trips complete URL state", () => {
  const state: TerrainLabState = {
    profile: "mclone-overworld-v1",
    visualProfile: "hybrid-authoring",
    texturePresentation: "flat-colors",
    compareVisualProfile: "minecraft-reference",
    seed: "-9223372036854775808",
    centerX: -1024,
    centerZ: 2048,
    blocksAcross: 16_384,
    detail: 16 as const,
    surfaceQuality: "inferred" as const,
    source: "gpu" as const,
    panes: ["canonical", "gpu"],
    canonicalStage: "surface" as const,
    canonicalRadius: 1,
    waterVisible: false,
    vegetationVisible: true,
    contentStage: "cover" as const,
    view: "map" as const,
    projection: "perspective" as const,
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

test("reports the serialized viewport footprint", () => {
  assert.equal(footprintBlocks({ blocksAcross: 16_384 }), 16_384);
});

test("accepts one-block links and rejects zero-width viewports", () => {
  assert.equal(parseTerrainLabState("?blocks=1").blocksAcross, 1);
  assert.equal(
    parseTerrainLabState("?blocks=0").blocksAcross,
    DEFAULT_TERRAIN_LAB_STATE.blocksAcross,
  );
});

test("canonical coverage changes only at Euclidean chunk boundaries", () => {
  assert.equal(canonicalTerrainCenterChunk(0), 0);
  assert.equal(canonicalTerrainCenterChunk(15), 0);
  assert.equal(canonicalTerrainCenterChunk(16), 1);
  assert.equal(canonicalTerrainCenterChunk(-1), -1);
  assert.equal(canonicalTerrainCenterChunk(-16), -1);
  assert.equal(canonicalTerrainCenterChunk(-17), -2);
});

test("accepts old spacing links without changing their visible footprint", () => {
  const legacy = parseTerrainLabState("?spacing=32");
  assert.equal(legacy.blocksAcross, 2_048);
  assert.equal(legacy.detail, 32);
});

test("accepts stepped exact footprints through 31 by 31 chunks", () => {
  for (const radius of [0, 1, 2, 3, 4, 5, 7, 10, 15]) {
    assert.equal(parseTerrainLabState(`?radius=${radius}`).canonicalRadius, radius);
  }
  assert.equal(parseTerrainLabState("?radius=6").canonicalRadius, 2);
  assert.equal(parseTerrainLabState("?radius=16").canonicalRadius, 2);
});

test("defaults to the three-pane workspace and gives the review site a name", () => {
  assert.equal(DEFAULT_TERRAIN_LAB_STATE.profile, "mclone-overworld-v1");
  assert.equal(DEFAULT_TERRAIN_LAB_STATE.visualProfile, "mclone-original");
  assert.equal(DEFAULT_TERRAIN_LAB_STATE.texturePresentation, "textured");
  assert.equal(DEFAULT_TERRAIN_LAB_STATE.compareVisualProfile, "off");
  assert.equal(DEFAULT_TERRAIN_LAB_STATE.surfaceQuality, "inferred");
  assert.equal(DEFAULT_TERRAIN_LAB_STATE.source, "split");
  assert.deepEqual(DEFAULT_TERRAIN_LAB_STATE.panes, ["canonical", "cpu", "gpu"]);
  assert.equal(REVIEW_TERRAIN_LAB_STATE.seed, "-98765");
  assert.equal(REVIEW_TERRAIN_LAB_STATE.source, "split");
});

test("maps legacy source links and keeps at least one pane visible", () => {
  assert.deepEqual(parseTerrainLabState("?source=gpu").panes, ["gpu"]);
  assert.deepEqual(parseTerrainLabState("?source=split").panes, ["cpu", "gpu"]);
  assert.equal(proceduralSourceForPanes(["canonical"]), "reference");
  assert.equal(proceduralSourceForPanes(["macro"]), "macro");
  assert.equal(proceduralSourceForPanes(["cpu", "macro"]), "split");
  const onlyCanonical: TerrainLabState = {
    ...DEFAULT_TERRAIN_LAB_STATE,
    panes: ["canonical"],
  };
  assert.equal(toggleTerrainLabPane(onlyCanonical, "canonical"), onlyCanonical);
  assert.deepEqual(toggleTerrainLabPane(onlyCanonical, "gpu").panes, ["canonical", "gpu"]);
});

test("the vanilla profile replaces GPU with fast macro and excludes Mclone-only layers", () => {
  const vanilla = switchTerrainLabProfile(
    {
      ...DEFAULT_TERRAIN_LAB_STATE,
      layer: "streams",
      contentStage: "cover",
    },
    "overworld",
  );
  assert.equal(vanilla.profile, "overworld");
  assert.deepEqual(vanilla.panes, ["canonical", "cpu", "macro"]);
  assert.equal(vanilla.source, "split");
  assert.equal(vanilla.contentStage, "surface");
  assert.equal(vanilla.layer, "terrain");
  assert.equal(toggleTerrainLabPane(vanilla, "gpu"), vanilla);
  assert.deepEqual(
    toggleTerrainLabPane(vanilla, "cpu").panes,
    ["canonical", "macro"],
  );
});

test("vanilla URL state replaces a GPU-only workspace with fast macro", () => {
  const vanilla = parseTerrainLabState(
    "?profile=overworld&panes=gpu&stage=hydrology&layer=continentalness",
  );
  assert.equal(vanilla.profile, "overworld");
  assert.deepEqual(vanilla.panes, ["macro"]);
  assert.equal(vanilla.source, "macro");
  assert.equal(vanilla.contentStage, "surface");
  assert.equal(vanilla.layer, "terrain");
  assert.match(terrainLabSearch(vanilla), /profile=overworld/u);
});

test("keeps camera state outside URL-addressed terrain state", () => {
  assert.deepEqual(DEFAULT_TERRAIN_LAB_STATE, {
    profile: "mclone-overworld-v1",
    visualProfile: "mclone-original",
    texturePresentation: "textured",
    compareVisualProfile: "off",
    seed: "-98765",
    centerX: -304,
    centerZ: 336,
    blocksAcross: 512,
    detail: "auto",
    surfaceQuality: "inferred",
    source: "split",
    panes: ["canonical", "cpu", "gpu"],
    canonicalStage: "final",
    canonicalRadius: 2,
    waterVisible: true,
    vegetationVisible: true,
    contentStage: "hydrology",
    view: "3d",
    projection: "orthographic",
    layer: "terrain",
  });
});

test("defaults to orthographic and validates projection links", () => {
  assert.equal(DEFAULT_TERRAIN_LAB_STATE.projection, "orthographic");
  assert.equal(
    parseTerrainLabState("?projection=perspective").projection,
    "perspective",
  );
  assert.equal(
    parseTerrainLabState("?projection=isometric").projection,
    "orthographic",
  );
});

test("round-trips explicit surface quality and rejects unknown tiers", () => {
  assert.equal(parseTerrainLabState("?surface=inferred").surfaceQuality, "inferred");
  assert.equal(
    parseTerrainLabState("?surface=expensive").surfaceQuality,
    DEFAULT_TERRAIN_LAB_STATE.surfaceQuality,
  );
});
