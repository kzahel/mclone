import { spawnSync } from "node:child_process";
import { createReadStream } from "node:fs";
import { stat, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, devices } from "@playwright/test";

import { resolveBrowserWebGpuLaunch } from "../../../../scripts/browser-webgpu-env.mjs";

const scriptRoot = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptRoot, "..");
const nativeRoot = path.resolve(appRoot, "../..");
const repositoryRoot = path.resolve(nativeRoot, "..");
const webRoot = path.join(nativeRoot, "target", "mclone-world-explorer-www");
const mobile = process.argv.includes("--mobile");
const skipBuild = process.argv.includes("--skip-build");
const externalBaseUrl = process.env.WORLD_EXPLORER_SMOKE_BASE_URL;
const label = mobile ? "mobile" : "desktop";
const port = Number.parseInt(
  process.env.WORLD_EXPLORER_SMOKE_PORT ?? (mobile ? "4192" : "4191"),
  10,
);
const launch = resolveBrowserWebGpuLaunch();
const pageErrors = [];
let server;
let browser;
let page;

try {
  if (!skipBuild && !externalBaseUrl) {
    run("node", [
      path.join(appRoot, "scripts", "build-web.mjs"),
    ]);
  }
  if (!externalBaseUrl) {
    server = await startServer();
  }
  browser = await chromium.launch({
    channel: process.env.PLAYWRIGHT_CHROME_CHANNEL ?? "chrome",
    headless: launch.headless,
    args: ["--enable-unsafe-webgpu", ...launch.chromeArgs],
    env: { ...process.env, ...launch.browserEnv },
  });
  const context = await browser.newContext(
    mobile
      ? { ...devices["Pixel 7"], isMobile: true }
      : { viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 },
  );
  page = await context.newPage();
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      pageErrors.push(message.text());
    }
  });
  const baseUrl = externalBaseUrl ?? `http://127.0.0.1:${port}/`;
  const targetUrl = new URL(
    "?seed=12345&centerX=-304&centerZ=336"
      + "&blocksAcross=4096&view=3d&projection=perspective",
    baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`,
  );
  await page.goto(
    targetUrl.href,
    { waitUntil: "networkidle" },
  );
  const shell = page.locator("#world-explorer-shell");
  await shell.waitFor({ state: "visible" });
  await waitReady(page);
  const initial = await report(page);
  assertFixedReady(initial, "initial");
  console.log(`World Explorer ${label} browser smoke: initial ready`);
  const initialCapture = `/tmp/mclone-world-explorer-web-${label}-initial.png`;
  await page.locator("#world-explorer-canvas").screenshot({ path: initialCapture });

  const canvas = page.locator("#world-explorer-canvas");
  await canvas.focus();
  const bounds = await canvas.boundingBox();
  if (!bounds) {
    throw new Error("World Explorer canvas has no browser bounds");
  }
  await page.mouse.move(bounds.x + bounds.width * 0.35, bounds.y + bounds.height * 0.55);
  await page.mouse.down();
  await page.mouse.move(
    bounds.x + bounds.width * 0.47,
    bounds.y + bounds.height * 0.55,
    { steps: 4 },
  );
  await page.mouse.up();
  await page.waitForFunction(
    (yaw) => globalThis.__MCLONE_WORLD_EXPLORER__?.report?.yawRadians !== yaw,
    initial.yawRadians,
  );
  const gesture = await report(page);
  assertFixedReady(gesture, "raw pointer gesture");
  console.log(`World Explorer ${label} browser smoke: pointer forwarding ready`);

  await page.keyboard.press("ArrowRight");
  await page.waitForFunction(
    (centerX) => globalThis.__MCLONE_WORLD_EXPLORER__?.report?.centerX !== centerX,
    initial.centerX,
  );
  await waitReady(page);
  const moved = await report(page);
  assertFixedReady(moved, "keyboard movement");
  console.log(`World Explorer ${label} browser smoke: movement ready`);

  await page.evaluate(() => {
    globalThis.__MCLONE_WORLD_EXPLORER__.session.recenter(-8193, -4097);
  });
  await waitForCenter(page, -8193, -4097);
  await waitReady(page);
  const negative = await report(page);
  assertFixedReady(negative, "negative-coordinate move");
  console.log(`World Explorer ${label} browser smoke: negative coordinates ready`);

  const rebasesBefore = negative.totalRebases;
  await page.evaluate(() => {
    globalThis.__MCLONE_WORLD_EXPLORER__.session.recenter(1000000, -1000000);
  });
  await waitForCenter(page, 1000000, -1000000);
  await waitReady(page);
  const teleported = await report(page);
  assertFixedReady(teleported, "teleport");
  console.log(`World Explorer ${label} browser smoke: teleport ready`);
  if (teleported.totalRebases <= rebasesBefore) {
    throw new Error(
      `teleport did not record clipmap rebases: ${rebasesBefore} -> ${teleported.totalRebases}`,
    );
  }

  const finalCapture = `/tmp/mclone-world-explorer-web-${label}-teleport.png`;
  await page.locator("#world-explorer-canvas").screenshot({ path: finalCapture });
  if (pageErrors.length > 0) {
    throw new Error(`browser errors:\n${pageErrors.join("\n")}`);
  }
  const receipt = {
    artifacts: await artifactSizes(),
    captures: { initial: initialCapture, teleport: finalCapture },
    gesture,
    initial,
    launch: {
      headed: launch.headed,
      useWayland: launch.useWayland,
      waylandDisplay: launch.waylandDisplay,
    },
    moved,
    negative,
    target: label,
    teleported,
  };
  const receiptPath = `/tmp/mclone-world-explorer-web-${label}-receipt.json`;
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ receipt: receiptPath, ...receipt }, null, 2));
} catch (error) {
  const runtime = await page?.evaluate(() => ({
    report: globalThis.__MCLONE_WORLD_EXPLORER__?.report ?? null,
    status: document.getElementById("world-explorer-status")?.textContent ?? "",
  })).catch(() => null);
  const failureCapture = `/tmp/mclone-world-explorer-web-${label}-failure.png`;
  await page?.screenshot({ path: failureCapture, fullPage: true }).catch(() => {});
  console.error(JSON.stringify({
    browserErrors: pageErrors,
    failureCapture,
    runtime,
  }, null, 2));
  throw error;
} finally {
  await browser?.close();
  await new Promise((resolve) => server?.close(resolve) ?? resolve());
}

async function waitReady(page) {
  await page.waitForFunction(() => {
    const report = globalThis.__MCLONE_WORLD_EXPLORER__?.report;
    return report?.targetReady === true
      && report?.pendingRefills === 0
      && report?.readySlots === report?.allocationSlots;
  }, null, { timeout: 30_000 });
}

async function waitForCenter(page, x, z) {
  await page.waitForFunction(
    ([expectedX, expectedZ]) => {
      const report = globalThis.__MCLONE_WORLD_EXPLORER__?.report;
      return report?.centerX === expectedX && report?.centerZ === expectedZ;
    },
    [x, z],
    { timeout: 30_000 },
  );
}

async function report(page) {
  return await page.evaluate(() => ({
    ...globalThis.__MCLONE_WORLD_EXPLORER__.report,
  }));
}

function assertFixedReady(value, stage) {
  if (value.allocationSlots !== 160
      || value.readySlots !== value.allocationSlots
      || value.pendingRefills !== 0
      || value.drawnLevels !== 10
      || value.fixedResidentBytes !== 86_551_040
      || value.pendingVegetationTiles !== 0
      || value.residentBytes <= 0) {
    throw new Error(`${stage} is not fixed and ready:\n${JSON.stringify(value, null, 2)}`);
  }
}

async function artifactSizes() {
  if (externalBaseUrl) {
    return null;
  }
  return {
    javascriptBytes: (await stat(
      path.join(webRoot, "pkg", "mclone_world_explorer.js"),
    )).size,
    wasmBytes: (await stat(
      path.join(webRoot, "pkg", "mclone_world_explorer_bg.wasm"),
    )).size,
  };
}

async function startServer() {
  const server = createServer((request, response) => {
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
    server.once("error", reject);
    server.listen(port, "127.0.0.1", resolve);
  });
  return server;
}

function contentType(file) {
  switch (path.extname(file)) {
    case ".css": return "text/css; charset=utf-8";
    case ".html": return "text/html; charset=utf-8";
    case ".js": return "text/javascript; charset=utf-8";
    case ".json": return "application/json; charset=utf-8";
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
