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
    assert.equal(catalog.summary.structures, 1);
    assert.equal(catalog.structures[0]?.structureId, "farmstead-cottage-a-v2");
    assert.equal(catalog.structures[0]?.runtimeStatus, "parity-canary");
    assert.equal(catalog.structures[0]?.assetProvenance.minecraftReference, 0);
    assert.equal(catalog.structures[0]?.assetProvenance.unknown, 0);
    const deployed = parseStructureCatalog(
      JSON.parse(await fs.readFile(path.join(outRoot, "catalog", "catalog.v1.json"), "utf8")),
      "test catalog",
    );
    assert.deepEqual(deployed, catalog);
    await fs.access(path.join(outRoot, catalog.structures[0]!.meshPath));
    await fs.access(path.join(outRoot, catalog.structures[0]!.atlasPath));
  } finally {
    await fs.rm(parent, { recursive: true, force: true });
  }
});
