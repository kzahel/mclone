import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { buildWebCatalog } from "../src/build-web-catalog";
import { parseAnimalCatalog } from "../src/catalog-model";
import { discoverCanonicalFigureSources } from "../src/discover-figures";
import { assetLabRoot } from "../src/vite-figure-path";

test("builds deterministic canonical JSON catalogue artifacts", async () => {
  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mclone-animal-catalog-"));
  const outRoot = path.join(tempRoot, "output");
  const sourcePaths = [
    path.join(assetLabRoot, "examples/chicken/figure.ts"),
    path.join(assetLabRoot, "examples/king_cobra/figure.ts"),
  ];
  try {
    const catalog = await buildWebCatalog({ outRoot, sourcePaths, thumbnails: false });
    assert.equal(catalog.schemaVersion, 1);
    assert.equal(catalog.summary.canonicalFigures, 2);
    assert.deepEqual(catalog.figures.map((figure) => figure.name), ["chicken", "king_cobra"]);
    assert.equal(catalog.figures[0]?.defaultClip, "walk");
    assert.equal(catalog.figures[1]?.defaultClip, "slither");

    const written = parseAnimalCatalog(
      JSON.parse(await fs.readFile(path.join(outRoot, "catalog/catalog.v1.json"), "utf8")),
      "test catalogue",
    );
    assert.deepEqual(written, catalog);
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

test("discovers canonical examples without the legacy rounded archive", async () => {
  const sources = await discoverCanonicalFigureSources();
  assert.ok(sources.length > 80);
  assert.ok(sources.every((source) => source.includes(`${path.sep}examples${path.sep}`)));
  assert.ok(sources.every((source) => !source.includes("legacy-examples")));
  assert.ok(sources.some((source) => source.endsWith(path.join("chicken", "figure.ts"))));
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
