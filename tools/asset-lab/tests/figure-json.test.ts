import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  assertBoxOnlyFigure,
  figure,
  legacyFigure,
  type FigureAsset,
} from "../src/dsl";
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

test("canonical figure rejects deprecated curved primitives", () => {
  assert.throws(
    () =>
      figure("curved", ({ part }) => {
        part("orb", { primitive: { kind: "sphere", radius: 1 } });
      }),
    /canonical figures may use only box primitives/,
  );

  const legacy = legacyFigure("curved_legacy", ({ part, sphere }) => {
    part("orb", sphere({ radius: 1 }));
  });
  assert.equal(legacy.parts[0]?.primitive.kind, "sphere");
});

test("swim macro exports ordinary body tail fin keys and locomotion", () => {
  const asset = figure("swimmer", ({ mat, part, box, swim }) => {
    mat("skin", "#447799");
    part("body", box({ size: [1, 1, 1], material: "skin" }));
    part("tail", box({ parent: "body", size: [0.2, 0.6, 0.6], material: "skin" }));
    part("tail_tip", box({ parent: "tail", size: [0.1, 0.8, 0.5], material: "skin" }));
    part("fin_l", box({ parent: "body", size: [0.4, 0.1, 0.3], material: "skin" }));
    part("fin_r", box({ parent: "body", size: [0.4, 0.1, 0.3], material: "skin" }));
    swim("swim", {
      duration: 1,
      samples: 5,
      body: "body",
      bodyBob: 0.02,
      bodySwayDegrees: 3,
      cycleDistance: 1.4,
      leftFin: "fin_l",
      rightFin: "fin_r",
      tail: "tail",
      tailTip: "tail_tip",
    });
  });

  const clip = asset.clips.swim;
  assert.ok(clip);
  assert.equal(clip.locomotion?.kind, "swim");
  assert.equal(clip.locomotion?.cycleDistance, 1.4);
  assert.deepEqual(clip.locomotion?.direction, [0, 0, -1]);
  assert.equal(clip.locomotion?.contacts, undefined);
  assert.deepEqual(
    new Set(clip.keys.map(([part]) => part)),
    new Set(["body", "tail", "tail_tip", "fin_l", "fin_r"]),
  );

  const finKeys = clip.keys.filter(([, time]) => time === 0.25);
  const leftFin = finKeys.find(([part]) => part === "fin_l")?.[2].rot;
  const rightFin = finKeys.find(([part]) => part === "fin_r")?.[2].rot;
  assert.ok(leftFin);
  assert.ok(rightFin);
  assert.equal(leftFin[2], -rightFin[2]);
});

test("slither macro exports a phased segment wave and locomotion", () => {
  const asset = figure("slitherer", ({ mat, part, box, slither }) => {
    mat("skin", "#447744");
    part("body", box({ size: [0.4, 0.3, 0.7], material: "skin" }));
    part("middle", box({ parent: "body", size: [0.3, 0.25, 0.6], material: "skin" }));
    part("tail", box({ parent: "middle", size: [0.2, 0.2, 0.5], material: "skin" }));
    slither("slither", {
      duration: 1,
      samples: 5,
      body: "body",
      bodyBob: 0.01,
      cycleDistance: 0.8,
      degrees: 10,
      phaseStep: 0.125,
      segments: ["body", "middle", "tail"],
    });
  });

  const clip = asset.clips.slither;
  assert.ok(clip);
  assert.equal(clip.locomotion?.kind, "slither");
  assert.equal(clip.locomotion?.cycleDistance, 0.8);
  assert.deepEqual(clip.locomotion?.direction, [0, 0, -1]);
  assert.equal(clip.locomotion?.contacts, undefined);
  assert.deepEqual(
    new Set(clip.keys.map(([part]) => part)),
    new Set(["body", "middle", "tail"]),
  );

  const quarterKeys = clip.keys.filter(([, time]) => time === 0.25);
  const body = quarterKeys.find(([part]) => part === "body")?.[2];
  const middle = quarterKeys.find(([part]) => part === "middle")?.[2];
  assert.ok(body?.rot);
  assert.ok(body.at);
  assert.ok(middle?.rot);
  assert.notEqual(body.rot[1], middle.rot[1]);
});

test("canonical and legacy examples cross the canonical JSON boundary", async () => {
  const canonicalSources = await discoverFigureSources("examples");
  const legacySources = await discoverFigureSources("legacy-examples");
  assert.ok(canonicalSources.length > 0);
  assert.ok(legacySources.length > 0);

  const names = new Set<string>();
  for (const sourcePath of [...canonicalSources, ...legacySources]) {
    const document = await loadFigureJsonDocument(sourcePath);
    assert.equal(serializeFigureAsset(document.asset), document.json);
    assert.equal(parseFigureAssetJson(document.json, sourcePath).name, document.asset.name);
    assert.equal(names.has(document.asset.name), false, `duplicate figure name '${document.asset.name}'`);
    names.add(document.asset.name);
  }

  for (const sourcePath of canonicalSources) {
    const document = await loadFigureJsonDocument(sourcePath);
    assert.doesNotThrow(() => assertBoxOnlyFigure(document.asset), sourcePath);
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

async function discoverFigureSources(directory: string): Promise<string[]> {
  const root = path.join(assetLabRoot, directory);
  const entries = await fs.readdir(root, { withFileTypes: true });
  return entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(root, entry.name, "figure.ts"))
    .sort();
}
