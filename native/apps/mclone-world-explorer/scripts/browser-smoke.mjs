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
  const baseUrl = externalBaseUrl ?? `http://127.0.0.1:${port}/`;
  const targetUrl = worldExplorerUrl(baseUrl);
  await assertOrdinaryPageHasNoObserver(context, targetUrl);
  page = await context.newPage();
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      pageErrors.push(message.text());
    }
  });
  targetUrl.searchParams.set("smokeObserver", "1");
  await page.goto(
    targetUrl.href,
    { waitUntil: "networkidle" },
  );
  const shell = page.locator("#world-explorer-shell");
  await shell.waitFor({ state: "visible" });
  await page.waitForFunction(
    () => globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer,
  );
  await waitReady(page);
  await assertTerrainOnlySurface(page);
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
    (yaw) => (
      globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer.snapshot().yawRadians !== yaw
    ),
    initial.yawRadians,
  );
  const gesture = await report(page);
  assertFixedReady(gesture, "raw pointer gesture");
  console.log(`World Explorer ${label} browser smoke: pointer forwarding ready`);

  await page.keyboard.down("Shift");
  await page.mouse.move(bounds.x + bounds.width * 0.5, bounds.y + bounds.height * 0.43);
  await page.mouse.down();
  await page.mouse.move(
    bounds.x + bounds.width * 0.5,
    bounds.y + bounds.height * 0.45,
    { steps: 4 },
  );
  await page.mouse.up();
  await page.keyboard.up("Shift");
  await page.waitForFunction(
    ([focusX, focusZ]) => {
      const report =
        globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer.snapshot();
      return Math.abs(report?.focusX - focusX) > 0.001
        || Math.abs(report?.focusZ - focusZ) > 0.001;
    },
    [gesture.focusX, gesture.focusZ],
  );
  await waitReady(page);
  const shiftPan = await report(page);
  assertFixedReady(shiftPan, "shift-primary pan");
  assertShiftPan(gesture, shiftPan);
  console.log(`World Explorer ${label} browser smoke: shift-primary pan ready`);
  const shiftPanCapture = `/tmp/mclone-world-explorer-web-${label}-shift-pan.png`;
  await canvas.screenshot({ path: shiftPanCapture });

  await twoContactStrafe(context, page, bounds);
  await page.waitForFunction(
    ([focusX, focusZ]) => {
      const report =
        globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer.snapshot();
      return Math.abs(report?.focusX - focusX) > 0.001
        || Math.abs(report?.focusZ - focusZ) > 0.001;
    },
    [shiftPan.focusX, shiftPan.focusZ],
  );
  const touch = await report(page);
  assertFixedReady(touch, "two-contact strafe");
  assertPresentationOnly(shiftPan, touch, "two-contact strafe");
  if (Number.isInteger(touch.focusX) && Number.isInteger(touch.focusZ)) {
    throw new Error(
      `two-contact strafe lost fractional focus:\n${JSON.stringify(touch, null, 2)}`,
    );
  }
  console.log(`World Explorer ${label} browser smoke: two-contact strafe ready`);

  await page.waitForTimeout(100);
  await canvas.focus();
  const heldSamples = [];
  await page.keyboard.down("ArrowRight");
  await page.waitForFunction(
    () => (
      globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer.snapshot().heldMotion === true
    ),
  );
  let heldFrame = await runtimeSnapshot(page);
  heldSamples.push(heldFrame.report);
  for (let sample = 0; sample < 4; sample += 1) {
    await page.waitForFunction(
      (frame) => (
        globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer.frame() > frame
      ),
      heldFrame.frame,
    );
    heldFrame = await runtimeSnapshot(page);
    heldSamples.push(heldFrame.report);
  }
  await page.keyboard.up("ArrowRight");
  await page.waitForFunction(
    () => (
      globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer.snapshot().heldMotion === false
    ),
  );
  assertHeldSamples(heldSamples, touch);
  await waitReady(page);
  const moved = await report(page);
  assertFixedReady(moved, "held keyboard movement");
  console.log(`World Explorer ${label} browser smoke: held movement ready`);
  const movementCapture = `/tmp/mclone-world-explorer-web-${label}-movement.png`;
  await canvas.screenshot({ path: movementCapture });

  await page.evaluate(() => {
    globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__.commands.recenter(-8193, -4097);
  });
  await waitForCenter(page, -8193, -4097);
  await waitReady(page);
  const negative = await report(page);
  assertFixedReady(negative, "negative-coordinate move");
  console.log(`World Explorer ${label} browser smoke: negative coordinates ready`);

  const rebasesBefore = negative.totalRebases;
  await page.evaluate(() => {
    globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__.commands.recenter(1000000, -1000000);
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
    captures: {
      initial: initialCapture,
      movement: movementCapture,
      shiftPan: shiftPanCapture,
      teleport: finalCapture,
    },
    gesture,
    heldSamples,
    initial,
    launch: {
      headed: launch.headed,
      useWayland: launch.useWayland,
      waylandDisplay: launch.waylandDisplay,
    },
    moved,
    negative,
    shiftPan,
    target: label,
    teleported,
    touch,
  };
  const receiptPath = `/tmp/mclone-world-explorer-web-${label}-receipt.json`;
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ receipt: receiptPath, ...receipt }, null, 2));
} catch (error) {
  const runtime = await page?.evaluate(() => ({
    report:
      globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer.snapshot() ?? null,
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
    const report =
      globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer.snapshot();
    return report?.targetReady === true
      && report?.pendingRefills === 0
      && report?.readySlots === report?.allocationSlots;
  }, null, { timeout: 30_000 });
}

async function waitForCenter(page, x, z) {
  await page.waitForFunction(
    ([expectedX, expectedZ]) => {
      const report =
        globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer.snapshot();
      return report?.centerX === expectedX && report?.centerZ === expectedZ;
    },
    [x, z],
    { timeout: 30_000 },
  );
}

async function report(page) {
  return await page.evaluate(
    () => globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__.observer.snapshot(),
  );
}

async function runtimeSnapshot(page) {
  return await page.evaluate(() => ({
    frame: globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__.observer.frame(),
    report: globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__.observer.snapshot(),
  }));
}

function worldExplorerUrl(baseUrl) {
  return new URL(
    "?seed=12345&centerX=-304&centerZ=336"
      + "&blocksAcross=4096&view=3d&projection=perspective",
    baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`,
  );
}

async function assertOrdinaryPageHasNoObserver(context, targetUrl) {
  const ordinaryPage = await context.newPage();
  try {
    await ordinaryPage.goto(targetUrl.href, { waitUntil: "networkidle" });
    await ordinaryPage.locator("#world-explorer-status").waitFor({
      state: "hidden",
      timeout: 30_000,
    });
    const observerInstalled = await ordinaryPage.evaluate(
      () => "__MCLONE_WORLD_EXPLORER_SMOKE__" in globalThis,
    );
    if (observerInstalled) {
      throw new Error("ordinary World Explorer page installed the smoke observer");
    }
    await assertTerrainOnlySurface(ordinaryPage);
  } finally {
    await ordinaryPage.close();
  }
}

async function assertTerrainOnlySurface(targetPage) {
  const visibleChildren = await targetPage.locator("#world-explorer-shell").evaluate(
    (element) => [...element.children]
      .filter((child) => {
        const style = getComputedStyle(child);
        return !child.hidden
          && style.display !== "none"
          && style.visibility !== "hidden";
      })
      .map((child) => child.id),
  );
  if (visibleChildren.length !== 1
      || visibleChildren[0] !== "world-explorer-canvas") {
    throw new Error(
      `World Explorer content area is not terrain-only: ${visibleChildren.join(", ")}`,
    );
  }
}

function assertFixedReady(value, stage) {
  if (value.allocationSlots !== 160
      || value.readySlots !== value.allocationSlots
      || value.pendingRefills !== 0
      || value.drawnLevels !== 10
      || value.fixedResidentBytes !== 86_553_600
      || value.pendingVegetationTiles !== 0
      || value.residentBytes <= 0) {
    throw new Error(`${stage} is not fixed and ready:\n${JSON.stringify(value, null, 2)}`);
  }
}

function assertPresentationOnly(before, after, stage) {
  if (after.revision !== before.revision
      || after.totalRefills !== before.totalRefills
      || after.totalRebases !== before.totalRebases
      || after.allocationSlots !== before.allocationSlots) {
    throw new Error(
      `${stage} changed residency:\n`
        + `${JSON.stringify({ before, after }, null, 2)}`,
    );
  }
}

function assertHeldSamples(samples, before) {
  const expectedX = -Math.sin(before.yawRadians);
  const expectedZ = -Math.cos(before.yawRadians);
  const moving = samples.filter((sample) => (
    Math.hypot(sample.focusX - before.focusX, sample.focusZ - before.focusZ) > 0.001
  ));
  const distinct = new Set(moving.map((sample) => (
    `${sample.focusX.toFixed(6)}:${sample.focusZ.toFixed(6)}`
  )));
  const deltas = moving.map((sample, index) => {
    const previous = index === 0 ? before : moving[index - 1];
    const deltaX = sample.focusX - previous.focusX;
    const deltaZ = sample.focusZ - previous.focusZ;
    return {
      alongHeading: deltaX * expectedX + deltaZ * expectedZ,
      acrossHeading: deltaX * -expectedZ + deltaZ * expectedX,
    };
  });
  if (distinct.size < 3
      || deltas.some(({ alongHeading, acrossHeading }) => (
        alongHeading < 0.0
        || alongHeading > 200.0
        || Math.abs(acrossHeading) > 0.001
      ))) {
    throw new Error(
      `held movement was not smoothly frame-timed and camera-relative:\n`
        + `${JSON.stringify({ before, deltas, samples }, null, 2)}`,
    );
  }
}

function assertShiftPan(before, after) {
  const deltaX = after.focusX - before.focusX;
  const deltaZ = after.focusZ - before.focusZ;
  const expectedX = -Math.cos(before.yawRadians);
  const expectedZ = Math.sin(before.yawRadians);
  const alongHeading = deltaX * expectedX + deltaZ * expectedZ;
  const acrossHeading = deltaX * -expectedZ + deltaZ * expectedX;
  if (Math.abs(after.yawRadians - before.yawRadians) > 1.0e-10
      || alongHeading <= 1.0
      || Math.abs(acrossHeading) > 0.001) {
    throw new Error(
      `shift-primary drag did not pan down with the grabbed terrain:\n`
        + `${JSON.stringify({
          before,
          after,
          alongHeading,
          acrossHeading,
        }, null, 2)}`,
    );
  }
}

async function twoContactStrafe(context, page, bounds) {
  const client = await context.newCDPSession(page);
  const first = {
    x: bounds.x + bounds.width * 0.42,
    y: bounds.y + bounds.height * 0.48,
    radiusX: 8,
    radiusY: 8,
    force: 1,
    id: 1,
  };
  const second = {
    ...first,
    x: bounds.x + bounds.width * 0.58,
    id: 2,
  };
  await client.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: [first, second],
  });
  await client.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [
      { ...first, x: first.x - 1 },
      { ...second, x: second.x - 1 },
    ],
  });
  await client.send("Input.dispatchTouchEvent", {
    type: "touchEnd",
    touchPoints: [],
  });
  await client.detach();
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
