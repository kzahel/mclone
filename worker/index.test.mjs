import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import worker from "./index.js";

test("fallback Worker returns an isolated 404 without an R2 binding", async () => {
  const response = await worker.fetch(new Request("https://mclone.example/missing"));

  assert.equal(response.status, 404);
  assert.equal(await response.text(), "Not Found");
  assert.equal(response.headers.get("cross-origin-opener-policy"), "same-origin");
  assert.equal(response.headers.get("cross-origin-embedder-policy"), "require-corp");
  assert.equal(response.headers.get("cross-origin-resource-policy"), "same-origin");
});

test("Wrangler serves the aggregate bundle as static assets without R2", async () => {
  const config = await readFile(new URL("./wrangler.toml", import.meta.url), "utf8");

  assert.match(config, /\[assets\]\s+directory = "\.\.\/dist-native-web"/);
  assert.doesNotMatch(config, /\[\[r2_buckets\]\]/);
});

test("static assets retain isolation and immutable hashed/runtime caching", async () => {
  const headers = await readFile(new URL("./_headers", import.meta.url), "utf8");

  assert.match(headers, /\/\*\s+Cross-Origin-Opener-Policy: same-origin/);
  assert.match(headers, /Cross-Origin-Embedder-Policy: require-corp/);
  assert.match(headers, /Cross-Origin-Resource-Policy: same-origin/);
  for (const path of [
    "/assets/*",
    "/animals/assets/*",
    "/structures/assets/*",
    "/terrain/assets/*",
    "/textures/assets/*",
    "/textures/media/*",
    "/pkg/*",
    "/reference/minecraft-1.17.1/extracted.zip",
    "/first-party-packs/*",
  ]) {
    assert.ok(headers.includes(`${path}\n  Cache-Control: public, max-age=31536000, immutable`));
  }
});

test("the aggregate bundle exposes a root hub and dedicated play route", async () => {
  const hub = await readFile(
    new URL("../native/apps/mclone-web-client/www/hub.html", import.meta.url),
    "utf8",
  );
  const bundle = await readFile(
    new URL("../scripts/deploy-native-web.sh", import.meta.url),
    "utf8",
  );

  for (const path of ["/play/", "/explore/", "/terrain/", "/textures/", "/animals/"]) {
    assert.ok(hub.includes(`href="${path}"`), `hub is missing ${path}`);
  }
  assert.match(bundle, /cp "\$WEB_ROOT\/hub\.html" "\$DEPLOY_DIR\/index\.html"/);
  assert.match(bundle, /"\$DEPLOY_DIR\/play\/index\.html"/);
  assert.match(bundle, /cp -R "\$TEXTURE_LAB_WEB_ROOT"\/\. "\$DEPLOY_DIR\/textures"\//);
});
