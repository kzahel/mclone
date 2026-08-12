import assert from "node:assert/strict";
import test from "node:test";

import {
  SEMANTIC_TERRAIN_VERTICAL_DATUM,
  semanticTerrainVerticalExaggeration,
  semanticTerrainVerticalOffset,
} from "../src/web/semantic-terrain-projection";

test("uses the horizontal world scale for physical vertical projection", () => {
  const pitch = 0.65;
  const panelScale = 320;
  const blocksAcross = 6_144;
  assert.equal(
    semanticTerrainVerticalOffset(
      SEMANTIC_TERRAIN_VERTICAL_DATUM,
      pitch,
      panelScale,
      blocksAcross,
      1,
    ),
    0,
  );
  const expected = 48
    / (blocksAcross * 0.5)
    * panelScale
    * Math.cos(pitch);
  assert.equal(
    semanticTerrainVerticalOffset(
      SEMANTIC_TERRAIN_VERTICAL_DATUM + 48,
      pitch,
      panelScale,
      blocksAcross,
      1,
    ),
    expected,
  );
  assert.equal(
    semanticTerrainVerticalOffset(
      SEMANTIC_TERRAIN_VERTICAL_DATUM - 48,
      pitch,
      panelScale,
      blocksAcross,
      1,
    ),
    -expected,
  );
});

test("makes peaks shrink with zoom-out and keeps exaggeration explicit", () => {
  const near = semanticTerrainVerticalOffset(112, 0.7, 300, 4_096, 1);
  const far = semanticTerrainVerticalOffset(112, 0.7, 300, 8_192, 1);
  assert.equal(far, near * 0.5);
  assert.equal(
    semanticTerrainVerticalOffset(112, 0.7, 300, 4_096, 8),
    near * 8,
  );
  assert.equal(semanticTerrainVerticalExaggeration("1x"), 1);
  assert.equal(semanticTerrainVerticalExaggeration("8x"), 8);
  assert.equal(semanticTerrainVerticalExaggeration("24x"), 24);
});
