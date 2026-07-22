import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { buildWebCatalog } from "../src/build-web-catalog";
import { parseAnimalCatalog } from "../src/catalog-model";
import { figureAlphaModes, type FigureAsset } from "../src/dsl";
import { discoverCanonicalFigureSources } from "../src/discover-figures";
import { FIRST_PARTY_FIGURES } from "../src/first-party-figures";
import { assetLabRoot } from "../src/vite-figure-path";

test("builds deterministic canonical JSON catalogue artifacts", async () => {
  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mclone-animal-catalog-"));
  const outRoot = path.join(tempRoot, "output");
  const sourcePaths = [
    path.join(assetLabRoot, "examples/chicken/figure.ts"),
    path.join(assetLabRoot, "examples/king_cobra/figure.ts"),
    path.join(assetLabRoot, "examples/roly_poly/figure.ts"),
  ];
  try {
    const catalog = await buildWebCatalog({ outRoot, sourcePaths, thumbnails: false });
    assert.equal(catalog.schemaVersion, 1);
    assert.equal(catalog.summary.canonicalFigures, 3);
    assert.equal(catalog.summary.runtimePromotedFigures, 1);
    assert.deepEqual(
      catalog.figures.map((figure) => figure.name),
      ["chicken", "king_cobra", "roly_poly"],
    );
    assert.equal(catalog.figures[0]?.defaultClip, "walk");
    assert.deepEqual(catalog.figures[0]?.alphaModes, ["opaque"]);
    assert.deepEqual(catalog.figures[0]?.metadata, {
      bodyPlans: ["biped"],
      disposition: "neutral",
      groups: ["animal"],
      habitats: ["land"],
      scale: "medium",
    });
    assert.deepEqual(catalog.figures[0]?.runtimePromotion, {
      figureId: "mclone:chicken",
      jsonPath: "assets/mclone/figures/chicken.figure.json",
    });
    assert.equal(catalog.figures[1]?.defaultClip, "slither");
    assert.equal(catalog.figures[1]?.runtimePromotion, undefined);
    assert.equal(catalog.figures[2]?.defaultClip, "crawl");
    assert.deepEqual(
      catalog.figures[2]?.clips.map(({ label, name, nextClip, role }) => ({
        label,
        name,
        nextClip,
        role,
      })),
      [
        { label: "Crawl", name: "crawl", nextClip: undefined, role: "locomotion" },
        { label: "Roll up", name: "roll_up", nextClip: undefined, role: "action" },
        { label: "Unroll", name: "unroll", nextClip: "crawl", role: "action" },
      ],
    );

    const written = parseAnimalCatalog(
      JSON.parse(await fs.readFile(path.join(outRoot, "catalog/catalog.v1.json"), "utf8")),
      "test catalogue",
    );
    assert.deepEqual(written, catalog);
    const inconsistentSummary = structuredClone(catalog);
    inconsistentSummary.summary.runtimePromotedFigures = 0;
    assert.throws(
      () => parseAnimalCatalog(inconsistentSummary, "inconsistent catalogue"),
      /expected 0 runtime-promoted figures but contains 1/,
    );
    const unsafeRuntimePath = structuredClone(catalog);
    const chickenPromotion = unsafeRuntimePath.figures[0]?.runtimePromotion;
    assert.ok(chickenPromotion);
    chickenPromotion.jsonPath = "../chicken.figure.json";
    assert.throws(
      () => parseAnimalCatalog(unsafeRuntimePath, "unsafe catalogue"),
      /invalid runtime promotion metadata/,
    );
    const invalidMetadata = structuredClone(catalog);
    (invalidMetadata.figures[0]!.metadata.groups as unknown as string[])[0] = "machine";
    assert.throws(
      () => parseAnimalCatalog(invalidMetadata, "invalid metadata catalogue"),
      /invalid creature metadata.*groups entry 0 'machine' is invalid/s,
    );
    const invalidAlphaMode = structuredClone(catalog);
    (invalidAlphaMode.figures[0]!.alphaModes as unknown as string[])[0] = "sorted";
    assert.throws(
      () => parseAnimalCatalog(invalidAlphaMode, "invalid alpha catalogue"),
      /invalid figure at index 0/,
    );
    for (const figure of catalog.figures) {
      const json = await fs.readFile(path.join(outRoot, figure.jsonPath), "utf8");
      assert.equal(Buffer.byteLength(json), figure.semanticBytes);
      assert.equal(createHash("sha256").update(json).digest("hex"), figure.semanticSha256);
      await assert.rejects(fs.access(path.join(outRoot, figure.thumbnailPath)));
    }
  } finally {
    await fs.rm(tempRoot, { force: true, recursive: true });
  }
});

test("reports alpha modes from rendered faces rather than unused declarations", () => {
  const asset: FigureAsset = {
    schemaVersion: 1,
    name: "alpha-catalog-test",
    materials: {
      smooth: { color: "#ffffff", alphaMode: "blend" },
      unused: { color: "#ffffff", alphaMode: "mask" },
    },
    textures: {
      translucent: {
        palette: { ".": "transparent", "#": "#ffffff80" },
        pixels: [".#"],
      },
      unused: {
        palette: { ".": "transparent", "#": "#ffffff" },
        pixels: ["##"],
      },
    },
    parts: [{
      name: "body",
      material: "smooth",
      texture: "translucent",
      primitive: { kind: "box", size: [1, 1, 1] },
    }],
    clips: {},
  };

  assert.deepEqual(figureAlphaModes(asset), ["blend"]);
  delete asset.parts[0]!.material;
  assert.deepEqual(figureAlphaModes(asset), ["mask"]);
});

test("discovers canonical examples without the legacy rounded archive", async () => {
  const sources = await discoverCanonicalFigureSources();
  assert.ok(sources.length > 80);
  assert.ok(sources.every((source) => source.includes(`${path.sep}examples${path.sep}`)));
  assert.ok(sources.every((source) => !source.includes("legacy-examples")));
  assert.ok(sources.some((source) => source.endsWith(path.join("chicken", "figure.ts"))));
  const sourceSet = new Set(sources.map((source) => path.resolve(source)));
  assert.deepEqual(
    FIRST_PARTY_FIGURES.filter((figure) => sourceSet.has(path.resolve(figure.sourcePath)))
      .map((figure) => figure.name)
      .sort(),
    ["chicken", "player", "upright_bear"],
  );
});

test("rejects duplicate canonical figure names", async () => {
  const sourcePath = path.join(assetLabRoot, "examples/chicken/figure.ts");
  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mclone-animal-catalog-"));
  try {
    await assert.rejects(
      buildWebCatalog({
        outRoot: path.join(tempRoot, "output"),
        sourcePaths: [sourcePath, sourcePath],
        thumbnails: false,
      }),
      /Duplicate canonical figure name 'chicken'/,
    );
  } finally {
    await fs.rm(tempRoot, { force: true, recursive: true });
  }
});
