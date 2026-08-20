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
const externalBaseUrl = process.env.WORLD_EXPLORER_SMOKE_BASE_URL?.replace(/\/+$/u, "");
const port = Number.parseInt(process.env.WORLD_EXPLORER_SMOKE_PORT ?? "4193", 10);
const journeys = [
  "coast-to-wooded-interior",
  "clearing-between-forest-cores",
  "long-forest-edge",
  "connected-water-country",
  "quiet-rolling-interior",
  "upland-to-arid-basin",
];
const frames = [
  ["locator", "map", "65536"],
  ["overview", "map", "16384"],
  ["oblique", "3d", "8192"],
  ["habitat", "3d", "512"],
];
const pageErrors = [];
const captures = [];
const interactiveUrls = [];
const journeyCenters = new Map();
let catalogSha256;
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
  for (const journey of journeys) {
    let horizonPage;
    for (const [frame, view, blocksAcross] of frames) {
      const page = await browser.newPage({
        viewport: { width: 1280, height: 720 },
        deviceScaleFactor: 1,
      });
      page.on("pageerror", (error) => pageErrors.push(error.message));
      page.on("console", (message) => {
        if (message.type() === "error") {
          pageErrors.push(message.text());
        }
      });
      const target = new URL(externalBaseUrl ?? `http://127.0.0.1:${port}/`);
      target.searchParams.set("journey", journey);
      target.searchParams.set("blocksAcross", blocksAcross);
      target.searchParams.set("view", view);
      target.searchParams.set("smokeObserver", "1");
      await page.goto(target.href, { waitUntil: "networkidle" });
      await page.locator("#world-explorer-shell").waitFor({ state: "visible" });
      await page.waitForFunction(
        () => globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer,
      );
      await waitReady(page);
      const snapshot = await report(page);
      assertJourney(snapshot, journey, frame);
      if (catalogSha256 && snapshot.journeyCatalogSha256 !== catalogSha256) {
        throw new Error(`journey catalog changed within one review run`);
      }
      catalogSha256 = snapshot.journeyCatalogSha256;
      const center = journeyCenters.get(journey);
      if (center && (center[0] !== snapshot.centerX || center[1] !== snapshot.centerZ)) {
        throw new Error(`${journey} resolved to inconsistent centers`);
      }
      journeyCenters.set(journey, [snapshot.centerX, snapshot.centerZ]);
      const capture = `/tmp/mclone-journey-${journey}-${frame}-web.png`;
      await page.locator("#world-explorer-canvas").screenshot({ path: capture });
      captures.push(capture);
      if (frame === "oblique") {
        interactiveUrls.push(target.href.replace("smokeObserver=1", "smokeObserver=0"));
      }
      if (frame === "habitat") {
        horizonPage = { page, snapshot };
      } else {
        await page.close();
      }
    }
    const { page, snapshot } = horizonPage;
    const flyX = snapshot.centerX + snapshot.journeyHeadingX * 1_024;
    const flyZ = snapshot.centerZ + snapshot.journeyHeadingZ * 1_024;
    await page.evaluate(([worldX, worldZ]) => {
      globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__.commands.recenter(worldX, worldZ);
    }, [flyX, flyZ]);
    await page.waitForFunction(([worldX, worldZ]) => {
      const current = globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer.snapshot();
      return current?.centerX === worldX && current?.centerZ === worldZ;
    }, [flyX, flyZ]);
    await waitReady(page);
    const fly = await report(page);
    assertJourney(fly, journey, "fly");
    const flyCapture = `/tmp/mclone-journey-${journey}-fly-web.png`;
    await page.locator("#world-explorer-canvas").screenshot({ path: flyCapture });
    captures.push(flyCapture);
    await page.evaluate(() => {
      globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__.commands.shutdown();
    });
    await page.close();
  }
  if (pageErrors.length > 0) {
    throw new Error(`journey browser console errors:\n${pageErrors.join("\n")}`);
  }
  console.log(JSON.stringify({
    status: "ok",
    catalogSha256,
    journeyCenters: Object.fromEntries(journeyCenters),
    captures,
    interactiveUrls,
  }, null, 2));
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

function assertJourney(snapshot, journey, frame) {
  if (snapshot.terrainSource !== "continental-ecoregion-candidate-v1"
      || snapshot.composition !== "horizon"
      || snapshot.journey !== journey
      || !/^[0-9a-f]{64}$/u.test(snapshot.journeyCatalogSha256 ?? "")
      || snapshot.readySlots !== snapshot.allocationSlots
      || snapshot.committedLevels !== snapshot.requestedLevels
      || snapshot.exactDesiredChunks !== 0
      || snapshot.exactPaintedChunks !== 0
      || snapshot.exactVertexCount !== 0
      || snapshot.vegetationCacheCellRequests !== 0
      || snapshot.vegetationCoordinatorState !== "running") {
    throw new Error(
      `${journey} ${frame} is not a complete journey frame:\n`
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
