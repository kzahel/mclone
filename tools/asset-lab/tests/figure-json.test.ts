import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import type { FigureAsset } from "../src/dsl";
import {
  parseFigureAssetJson,
  roundTripFigureAsset,
  serializeFigureAsset,
} from "../src/figure-json";
import { loadFigureJsonDocument } from "../src/load";
import { assetLabRoot } from "../src/vite-figure-path";

test("serializes and reparses the complete semantic figure", () => {
  const source = tinyFigure();
  const document = roundTripFigureAsset(source, "tiny source");

  assert.notEqual(document.asset, source);
  assert.deepEqual(document.asset, source);
  assert.equal(document.json, serializeFigureAsset(source));
  assert.match(document.json, /\n$/);
});

test("rejects values that cannot cross the JSON contract", () => {
  const source = tinyFigure();
  source.parts[0]!.at = [Number.NaN, 0, 0];

  assert.throws(
    () => roundTripFigureAsset(source, "non-finite source"),
    /part 'body' at\[0\] must be finite/,
  );
});

test("loads a JSON file as a canonical figure document", async () => {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "mclone-asset-lab-json-"));
  try {
    const jsonPath = path.join(tempDir, "tiny.figure.json");
    const expected = serializeFigureAsset(tinyFigure());
    await fs.writeFile(jsonPath, expected, "utf8");

    const document = await loadFigureJsonDocument(jsonPath);

    assert.equal(document.asset.name, "tiny");
    assert.equal(document.json, expected);
  } finally {
    await fs.rm(tempDir, { force: true, recursive: true });
  }
});

test("TypeScript source and checked player JSON resolve identically", async () => {
  const source = await loadFigureJsonDocument(path.join(assetLabRoot, "examples/player/figure.ts"));
  const checked = await loadFigureJsonDocument(
    path.resolve(assetLabRoot, "../../assets/mclone/figures/player.figure.json"),
  );

  assert.deepEqual(source.asset, checked.asset);
  assert.equal(source.json, checked.json);
});

test("every Asset Lab example crosses the canonical JSON boundary", async () => {
  const examplesDir = path.join(assetLabRoot, "examples");
  const entries = await fs.readdir(examplesDir, { withFileTypes: true });
  const sources = entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(examplesDir, entry.name, "figure.ts"))
    .sort();
  assert.ok(sources.length > 0);

  const names = new Set<string>();
  for (const sourcePath of sources) {
    const document = await loadFigureJsonDocument(sourcePath);
    assert.equal(serializeFigureAsset(document.asset), document.json);
    assert.equal(parseFigureAssetJson(document.json, sourcePath).name, document.asset.name);
    assert.equal(names.has(document.asset.name), false, `duplicate figure name '${document.asset.name}'`);
    names.add(document.asset.name);
  }
});

test("reports invalid direct JSON before it reaches Three.js", () => {
  assert.throws(
    () => parseFigureAssetJson("{\"schemaVersion\":1}", "broken.figure.json"),
    /Expected 'broken\.figure\.json' to contain a schema-v1 FigureAsset/,
  );
});

function tinyFigure(): FigureAsset {
  return {
    schemaVersion: 1,
    name: "tiny",
    materials: {
      white: { color: "#ffffff" },
    },
    textures: {},
    parts: [
      {
        name: "body",
        at: [0, 0, 0],
        material: "white",
        primitive: { kind: "box", size: [1, 1, 1] },
      },
    ],
    clips: {},
  };
}
