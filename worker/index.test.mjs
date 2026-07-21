import assert from "node:assert/strict";
import test from "node:test";
import worker from "./index.js";

test("serves the animal catalogue directory entry without stale HTML", async () => {
  const fixture = bucketFixture();
  const response = await worker.fetch(new Request("https://mclone.example/animals/"), fixture.env);

  assert.equal(fixture.keys[0], "animals/index.html");
  assert.equal(response.status, 200);
  assert.equal(response.headers.get("cache-control"), "no-cache, max-age=0, must-revalidate");
  assert.equal(response.headers.get("cross-origin-opener-policy"), "same-origin");
});

test("caches nested Vite assets immutably and revalidates catalogue data", async () => {
  const fixture = bucketFixture();
  const asset = await worker.fetch(
    new Request("https://mclone.example/animals/assets/index-AbCd1234.js"),
    fixture.env,
  );
  const catalog = await worker.fetch(
    new Request("https://mclone.example/animals/catalog/catalog.v1.json"),
    fixture.env,
  );

  assert.equal(asset.headers.get("cache-control"), "public, max-age=31536000, immutable");
  assert.equal(catalog.headers.get("cache-control"), "no-cache, max-age=0, must-revalidate");
});

function bucketFixture() {
  const keys = [];
  return {
    keys,
    env: {
      BUCKET: {
        async get(key) {
          keys.push(key);
          return {
            body: `fixture:${key}`,
            httpEtag: `etag-${key}`,
            writeHttpMetadata(headers) {
              headers.set("Content-Type", key.endsWith(".html") ? "text/html" : "application/octet-stream");
            },
          };
        },
      },
    },
  };
}
