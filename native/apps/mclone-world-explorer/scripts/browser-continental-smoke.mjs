import { spawnSync } from "node:child_process";
import { createReadStream } from "node:fs";
import { createServer } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";

import { resolveBrowserWebGpuLaunch } from "../../../../scripts/browser-webgpu-env.mjs";

const scriptRoot = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptRoot, "..");
const nativeRoot = path.resolve(appRoot, "../..");
const repositoryRoot = path.resolve(nativeRoot, "..");
const webRoot = path.join(nativeRoot, "target", "mclone-world-explorer-www");
const skipBuild = process.argv.includes("--skip-build");
const externalBaseUrl = process.env.WORLD_EXPLORER_SMOKE_BASE_URL;
const port = Number.parseInt(process.env.WORLD_EXPLORER_SMOKE_PORT ?? "4193", 10);
const pageErrors = [];
let server;
let browser;

try {
  if (!skipBuild && !externalBaseUrl) {
    run("node", [path.join(appRoot, "scripts", "build-web.mjs")]);
  }
  if (!externalBaseUrl) {
    server = await startServer();
  }
  const launch = resolveBrowserWebGpuLaunch();
  browser = await chromium.launch({
    channel: process.env.PLAYWRIGHT_CHROME_CHANNEL ?? "chrome",
    headless: launch.headless,
    args: ["--enable-unsafe-webgpu", ...launch.chromeArgs],
    env: { ...process.env, ...launch.browserEnv },
  });
  const context = await browser.newContext({
    viewport: { width: 1280, height: 720 },
    deviceScaleFactor: 1,
  });
  const page = await context.newPage();
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      pageErrors.push(message.text());
    }
  });
  const target = new URL(externalBaseUrl ?? `http://127.0.0.1:${port}/`);
  target.searchParams.set("source", "continental");
  target.searchParams.set("composition", "horizon");
  target.searchParams.set("blocksAcross", "8192");
  target.searchParams.set("view", "3d");
  target.searchParams.set("smokeObserver", "1");
  await page.goto(target.href, { waitUntil: "networkidle" });
  await page.locator("#world-explorer-shell").waitFor({ state: "visible" });
  await page.waitForFunction(
    () => globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer,
  );
  await waitReady(page);
  const initial = await report(page);
  assertCandidate(initial, "initial");
  const initialCapture = "/tmp/mclone-world-explorer-web-continental-initial.png";
  await page.locator("#world-explorer-canvas").screenshot({ path: initialCapture });

  await page.evaluate(() => {
    globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__.commands.recenter(8192, -4096);
  });
  await page.waitForFunction(() => {
    const snapshot = globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer.snapshot();
    return snapshot?.centerX === 8192 && snapshot?.centerZ === -4096;
  });
  await waitReady(page);
  const moved = await report(page);
  assertCandidate(moved, "retained movement");
  if (moved.totalRebases <= initial.totalRebases) {
    throw new Error("continental movement did not publish a clipmap rebase");
  }
  const movementCapture = "/tmp/mclone-world-explorer-web-continental-movement.png";
  await page.locator("#world-explorer-canvas").screenshot({ path: movementCapture });
  if (pageErrors.length > 0) {
    throw new Error(`continental browser console errors:\n${pageErrors.join("\n")}`);
  }
  console.log(JSON.stringify({
    status: "ok",
    url: target.href,
    initialCapture,
    movementCapture,
    readySlots: moved.readySlots,
    allocationSlots: moved.allocationSlots,
    totalRefills: moved.totalRefills,
    totalRebases: moved.totalRebases,
    residentBytes: moved.residentBytes,
    exactPaintedChunks: moved.exactPaintedChunks,
    treeInstanceCount: moved.treeInstanceCount,
  }, null, 2));
  await page.evaluate(() => {
    globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__.commands.shutdown();
  });
} finally {
  await browser?.close();
  await new Promise((resolve) => server?.close(resolve) ?? resolve());
}

async function waitReady(page) {
  await page.waitForFunction(() => {
    const snapshot = globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer.snapshot();
    return snapshot?.targetReady
      && snapshot.readySlots === snapshot.allocationSlots
      && snapshot.pendingRefills === 0;
  }, null, { timeout: 60_000 });
}

async function report(page) {
  return page.evaluate(
    () => globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__.observer.snapshot(),
  );
}

function assertCandidate(snapshot, stage) {
  if (snapshot.terrainSource !== "continental-ecoregion-candidate-v1"
      || snapshot.composition !== "horizon"
      || snapshot.readySlots !== snapshot.allocationSlots
      || snapshot.committedLevels !== snapshot.requestedLevels
      || snapshot.exactDesiredChunks !== 0
      || snapshot.exactPaintedChunks !== 0
      || snapshot.exactVertexCount !== 0
      || snapshot.treeInstanceCount === 0
      || snapshot.vegetationReadyTiles === 0
      || snapshot.vegetationRecordCount !== snapshot.treeInstanceCount
      || snapshot.vegetationCacheCellRequests !== 0
      || snapshot.vegetationCoordinatorState !== "running") {
    throw new Error(
      `${stage} is not a complete source-qualified continental frame:\n`
        + `${JSON.stringify(snapshot, null, 2)}`,
    );
  }
}

async function startServer() {
  const localServer = createServer((request, response) => {
    const requested = new URL(request.url ?? "/", `http://127.0.0.1:${port}`);
    if (requested.pathname === "/favicon.ico") {
      response.writeHead(204).end();
      return;
    }
    const relative = requested.pathname === "/"
      ? "index.html"
      : decodeURIComponent(requested.pathname.replace(/^\/+/u, ""));
    const file = path.resolve(webRoot, relative);
    if (!file.startsWith(`${webRoot}${path.sep}`) && file !== path.join(webRoot, "index.html")) {
      response.writeHead(403).end();
      return;
    }
    response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
    response.setHeader("Cross-Origin-Resource-Policy", "same-origin");
    response.setHeader("Cache-Control", "no-store");
    response.setHeader("Content-Type", contentType(file));
    const stream = createReadStream(file);
    stream.on("error", () => response.writeHead(404).end("not found"));
    stream.pipe(response);
  });
  await new Promise((resolve, reject) => {
    localServer.once("error", reject);
    localServer.listen(port, "127.0.0.1", resolve);
  });
  return localServer;
}

function contentType(file) {
  switch (path.extname(file)) {
    case ".css": return "text/css; charset=utf-8";
    case ".html": return "text/html; charset=utf-8";
    case ".js": return "text/javascript; charset=utf-8";
    case ".wasm": return "application/wasm";
    default: return "application/octet-stream";
  }
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: repositoryRoot,
    env: process.env,
    stdio: "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${command} failed with status ${result.status ?? "unknown"}`);
  }
}
