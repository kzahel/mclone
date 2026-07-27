import assert from "node:assert/strict";
import test from "node:test";

import {
  SEMANTIC_TERRAIN_VERTICAL_DATUM,
  SEMANTIC_TERRAIN_VERTICAL_DISPLAY_SCALE,
  SEMANTIC_TERRAIN_VERTICAL_SPAN,
  semanticTerrainVerticalOffset,
} from "../src/web/semantic-terrain-projection";

test("uses one fixed vertical datum and span", () => {
  const pitch = 0.65;
  const panelScale = 320;
  assert.equal(
    semanticTerrainVerticalOffset(
      SEMANTIC_TERRAIN_VERTICAL_DATUM,
      pitch,
      panelScale,
    ),
    0,
  );
  const expected = 48
    / SEMANTIC_TERRAIN_VERTICAL_SPAN
    * panelScale
    * SEMANTIC_TERRAIN_VERTICAL_DISPLAY_SCALE
    * Math.cos(pitch);
  assert.equal(
    semanticTerrainVerticalOffset(
      SEMANTIC_TERRAIN_VERTICAL_DATUM + 48,
      pitch,
      panelScale,
    ),
    expected,
  );
  assert.equal(
    semanticTerrainVerticalOffset(
      SEMANTIC_TERRAIN_VERTICAL_DATUM - 48,
      pitch,
      panelScale,
    ),
    -expected,
  );
});

test("keeps viewport extrema and horizontal zoom outside the Y contract", () => {
  assert.equal(semanticTerrainVerticalOffset.length, 3);
});
