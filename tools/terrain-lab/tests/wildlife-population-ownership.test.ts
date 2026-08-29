import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const source = (relativePath: string): Promise<string> =>
  readFile(new URL(relativePath, import.meta.url), "utf8");

test("wildlife pane delegates decisions to the production Rust planner", async () => {
  const [facade, worker, canvas] = await Promise.all([
    source("../../../native/apps/mclone-terrain-lab/src/wildlife_population_web.rs"),
    source("../src/web/wildlife-population-worker.ts"),
    source("../src/web/WildlifePopulationCanvas.tsx"),
  ]);

  assert.match(facade, /WildlifePopulationPlanner/u);
  assert.match(facade, /\.planner\s*\.plan_cell\(cell\)/u);
  assert.match(facade, /McloneOverworldV2/u);
  assert.match(facade, /McloneOverworldV3/u);
  assert.match(facade, /RequiresPublishedBlocks/u);
  assert.match(worker, /WildlifePopulationCompiler/u);
  assert.match(worker, /request\.profile/u);
  assert.doesNotMatch(canvas, /desiredDensity\s*[:=].*[+*/-]/u);
  assert.doesNotMatch(canvas, /rabbitWeight\s*[:=].*[+*/-]/u);
  assert.doesNotMatch(canvas, /deerWeight\s*[:=].*[+*/-]/u);
  assert.doesNotMatch(canvas, /mallardWeight\s*[:=].*[+*/-]/u);
  assert.doesNotMatch(canvas, /beeWeight\s*[:=].*[+*/-]/u);
  assert.doesNotMatch(canvas, /squirrelWeight\s*[:=].*[+*/-]/u);
  assert.doesNotMatch(canvas, /cowWeight\s*[:=].*[+*/-]/u);
  assert.doesNotMatch(canvas, /chickenWeight\s*[:=].*[+*/-]/u);
});
