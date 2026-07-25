import assert from "node:assert/strict";
import test from "node:test";

import {
  CANONICAL_RAW_CACHE_MAX_CHUNKS,
  CanonicalRawCache,
} from "../src/web/canonical-raw-cache";
import type { CanonicalWorkerResult } from "../src/web/canonical-worker-protocol";

test("bounds canonical raw chunks and promotes recently read entries", () => {
  const cache = new CanonicalRawCache();
  for (let index = 0; index < CANONICAL_RAW_CACHE_MAX_CHUNKS; index += 1) {
    cache.set(String(index), result(index, 4, 2));
  }
  assert.equal(cache.size, CANONICAL_RAW_CACHE_MAX_CHUNKS);
  assert.equal(cache.rawBytes, CANONICAL_RAW_CACHE_MAX_CHUNKS * 12);

  assert.ok(cache.get("0"));
  cache.set("next", result(CANONICAL_RAW_CACHE_MAX_CHUNKS, 8, 3));

  assert.equal(cache.size, CANONICAL_RAW_CACHE_MAX_CHUNKS);
  assert.equal(cache.get("1"), undefined);
  assert.ok(cache.get("0"));
  assert.equal(cache.rawBytes, (CANONICAL_RAW_CACHE_MAX_CHUNKS - 1) * 12 + 20);
});

test("replacing and clearing canonical cache entries keeps byte evidence exact", () => {
  const cache = new CanonicalRawCache();
  cache.set("same", result(0, 3, 1));
  assert.equal(cache.rawBytes, 7);

  cache.set("same", result(0, 9, 4));
  assert.equal(cache.size, 1);
  assert.equal(cache.rawBytes, 25);

  cache.clear();
  assert.equal(cache.size, 0);
  assert.equal(cache.rawBytes, 0);
});

function result(
  chunkX: number,
  blockBytes: number,
  biomeValues: number,
): CanonicalWorkerResult {
  return {
    type: "result",
    epoch: 1,
    chunkX,
    chunkZ: 0,
    minY: 0,
    height: 1,
    fingerprint: String(chunkX),
    generationMs: 0,
    dependencyCacheHits: 0,
    generatedDependencyChunks: 0,
    retainedDependencyChunks: 0,
    blocks: new Uint8Array(blockBytes),
    biomes: new Int32Array(biomeValues),
  };
}
