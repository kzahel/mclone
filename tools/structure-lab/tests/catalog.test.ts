import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { buildWebCatalog } from "../src/build-web-catalog";
import { parseStructureCatalog } from "../src/catalog-model";

test("builds a public-safe catalog from checked Rust preview receipts", async () => {
  const parent = await fs.mkdtemp(path.join(os.tmpdir(), "mclone-structure-catalog-test-"));
  const outRoot = path.join(parent, "catalog");
  try {
    const catalog = await buildWebCatalog({ outRoot, thumbnails: false });
    assert.equal(catalog.summary.structures, 14);
    assert.equal(catalog.summary.families, 4);
    const standard = catalog.structures.find(
      (entry) => entry.structureId === "farmstead-cottage-a-v2",
    );
    assert.ok(standard);
    assert.equal(standard.runtimeStatus, "promoted");
    assert.equal(catalog.summary.runtimePromoted, 13);
    assert.equal(
      catalog.structures.find(
        (entry) => entry.structureId === "farmstead-rosehip-chicken-coop-v1",
      )?.runtimeStatus,
      "lab-only",
    );
    assert.ok(catalog.structures.every((entry) => entry.assetProvenance.minecraftReference === 0));
    assert.ok(catalog.structures.every((entry) => entry.assetProvenance.unknown === 0));
    const deployed = parseStructureCatalog(
      JSON.parse(await fs.readFile(path.join(outRoot, "catalog", "catalog.v1.json"), "utf8")),
      "test catalog",
    );
    assert.deepEqual(deployed, catalog);
    await fs.access(path.join(outRoot, standard.meshPath));
    await fs.access(path.join(outRoot, standard.atlasPath));
  } finally {
    await fs.rm(parent, { recursive: true, force: true });
  }
});
