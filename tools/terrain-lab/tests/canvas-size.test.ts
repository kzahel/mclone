import assert from "node:assert/strict";
import test from "node:test";

import { terrainCanvasBackingSize } from "../src/web/canvas-size";

test("uses the requested pixel ratio while both dimensions fit", () => {
  assert.deepEqual(terrainCanvasBackingSize(1_018, 780, 2), {
    width: 2_036,
    height: 1_560,
  });
});

test("scales both dimensions together when a wide canvas reaches the cap", () => {
  const size = terrainCanvasBackingSize(1_306, 780, 2);
  assert.deepEqual(size, {
    width: 2_048,
    height: 1_223,
  });
  assertAspectMatches(size, 1_306, 780);
});

test("keeps responding to width changes after the width reaches the cap", () => {
  const narrower = terrainCanvasBackingSize(1_100, 780, 2);
  const wider = terrainCanvasBackingSize(1_306, 780, 2);
  assert.equal(narrower.width, 2_048);
  assert.equal(wider.width, 2_048);
  assert.notEqual(narrower.height, wider.height);
  assertAspectMatches(narrower, 1_100, 780);
  assertAspectMatches(wider, 1_306, 780);
});

test("scales both dimensions together when a tall canvas reaches the cap", () => {
  const size = terrainCanvasBackingSize(780, 1_306, 2);
  assert.deepEqual(size, {
    width: 1_223,
    height: 2_048,
  });
  assertAspectMatches(size, 780, 1_306);
});

test("bounds excessive device scale and sanitizes unusable dimensions", () => {
  assert.deepEqual(terrainCanvasBackingSize(320, 200, 4), {
    width: 640,
    height: 400,
  });
  assert.deepEqual(terrainCanvasBackingSize(0, Number.NaN, 0), {
    width: 1,
    height: 1,
  });
});

function assertAspectMatches(
  backing: { width: number; height: number },
  cssWidth: number,
  cssHeight: number,
): void {
  const backingAspect = backing.width / backing.height;
  const cssAspect = cssWidth / cssHeight;
  assert.ok(
    Math.abs(backingAspect - cssAspect) <= 1 / backing.height,
    `backing aspect ${backingAspect} did not match CSS aspect ${cssAspect}`,
  );
}
