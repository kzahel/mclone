import { spawn, spawnSync } from "node:child_process";
import { writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, devices } from "@playwright/test";

import { resolveBrowserWebGpuLaunch } from "../../../scripts/browser-webgpu-env.mjs";

const terrainLabRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const mobile = process.argv.includes("--mobile");
const label = mobile ? "mobile" : "desktop";
const port = Number.parseInt(
  process.env.TERRAIN_LAB_SMOKE_PORT ?? (mobile ? "4182" : "4181"),
  10,
);
const externalBaseUrl = process.env.TERRAIN_LAB_SMOKE_BASE_URL
  ?.replace(/\/+$/u, "");
const baseUrl = externalBaseUrl ?? `http://127.0.0.1:${port}`;
const launch = resolveBrowserWebGpuLaunch();
const pageErrors = [];
let server;
let browser;

if (!Number.isInteger(port) || port <= 0) {
  throw new Error(`Invalid TERRAIN_LAB_SMOKE_PORT '${process.env.TERRAIN_LAB_SMOKE_PORT}'`);
}

try {
  if (!externalBaseUrl) {
    runBuild();
    server = startPreview();
    await waitForPreview();
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
      : { viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1 },
  );
  const page = await context.newPage();
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      pageErrors.push(message.text());
    }
  });

  await page.goto(
    `${baseUrl}/terrain/?seed=-98765&x=-304&z=336&spacing=32`
      + "&source=split&view=3d&layer=terrain",
    { waitUntil: "networkidle" },
  );
  const shell = page.locator(".appShell");
  await page.locator("[data-testid='lab-status']").waitFor({ state: "visible" });
  await waitForComparison(shell);
  const sourceControls = page.locator("[data-testid='preview-source-controls']");
  await sourceControls.waitFor({ state: "visible" });
  const sourceGuide = await sourceControls.innerText();
  if (!sourceGuide.includes("exact same world coordinates")) {
    throw new Error(`Terrain Lab compare guidance is missing:\n${sourceGuide}`);
  }
  const expectedCompareLayout = mobile ? "stacked" : "side-by-side";
  if (await shell.getAttribute("data-compare-layout") !== expectedCompareLayout
      || Number(await shell.getAttribute("data-vertex-count")) !== 49_152) {
    throw new Error(
      `Terrain Lab Compare did not submit two synchronized ${expectedCompareLayout} terrain views`,
    );
  }
  await page.locator("[data-testid='terrain-diagnostics']").scrollIntoViewIfNeeded();
  await settlePaint(page);
  await page.evaluate(() => window.scrollTo(0, 0));
  await settlePaint(page);
  const initialRevision = Number(await shell.getAttribute("data-render-revision"));
  const initialPitch = Number(await shell.getAttribute("data-camera-pitch"));
  const initialUrl = page.url();
  const comparisonMetrics = await readComparisonMetrics(shell);
  assertLargeFieldMetrics(comparisonMetrics, "2 km review");

  const pageCapture = `/tmp/mclone-terrain-lab-${label}.png`;
  const canvasCapture = `/tmp/mclone-terrain-lab-${label}-canvas.png`;
  const orbitCapture = `/tmp/mclone-terrain-lab-${label}-orbit.png`;
  const errorCapture = `/tmp/mclone-terrain-lab-${label}-map-error.png`;
  const continentScaleCapture = `/tmp/mclone-terrain-lab-${label}-continent-scale.png`;
  await page.screenshot({ path: pageCapture, fullPage: true });
  await page.locator("canvas[aria-label='Live GPU terrain preview']").screenshot({
    path: canvasCapture,
  });

  const stage = page.locator("[data-testid='terrain-stage']");
  const stageBox = await stage.boundingBox();
  if (!stageBox) {
    throw new Error("Terrain Lab stage has no interactive bounds");
  }
  const sourceControlsBox = await sourceControls.boundingBox();
  if (!sourceControlsBox
      || sourceControlsBox.y + sourceControlsBox.height > stageBox.y + 1) {
    throw new Error("Terrain Lab source controls are not immediately before the preview");
  }
  await page.mouse.move(
    stageBox.x + stageBox.width * 0.5,
    stageBox.y + stageBox.height * 0.5,
  );
  await page.mouse.down();
  await page.mouse.move(
    stageBox.x + stageBox.width * 0.7,
    stageBox.y + stageBox.height * 0.38,
  );
  await page.mouse.up();
  await waitForRevision(shell, initialRevision);
  const orbitRevision = Number(await shell.getAttribute("data-render-revision"));
  if (page.url() !== initialUrl) {
    throw new Error(`3D orbit changed terrain URL state: ${page.url()}`);
  }
  const orbitPitch = Number(await shell.getAttribute("data-camera-pitch"));
  if (!(orbitPitch < initialPitch)) {
    throw new Error(
      `Upward orbit drag did not lower pitch: ${initialPitch} -> ${orbitPitch}`,
    );
  }
  await page.locator("canvas[aria-label='Live GPU terrain preview']").screenshot({
    path: orbitCapture,
  });
  await page.getByRole("button", { name: "Reset 3D camera" }).click();
  await waitForRevision(shell, orbitRevision);
  const resetRevision = Number(await shell.getAttribute("data-render-revision"));

  await page.getByLabel("Diagnostic layer").selectOption("error");
  await waitForRevision(shell, resetRevision);
  const errorRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByRole("button", { name: "Map", exact: true }).click();
  await page.getByRole("button", { name: "Zoom out" }).click();
  await waitForRevision(shell, errorRevision);
  await page.evaluate(() => window.scrollTo(0, 0));
  await settlePaint(page);
  await page.screenshot({ path: errorCapture, fullPage: true });

  const localMapRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByLabel("Diagnostic layer").selectOption("continentalness");
  await page.getByLabel("Sample spacing").selectOption("1024");
  await waitForRevision(shell, localMapRevision);
  const continentScaleMetrics = await readComparisonMetrics(shell);
  assertLargeFieldMetrics(continentScaleMetrics, "65.5 km continent");
  await page.locator("canvas[aria-label='Live GPU terrain preview']").screenshot({
    path: continentScaleCapture,
  });

  const finalUrl = page.url();
  if (!finalUrl.includes("layer=continentalness")
      || !finalUrl.includes("view=map")
      || !finalUrl.includes("spacing=1024")) {
    throw new Error(`Terrain Lab controls did not round-trip through the URL: ${finalUrl}`);
  }
  if (pageErrors.length > 0) {
    throw new Error(`Browser errors:\n${pageErrors.join("\n")}`);
  }

  const report = {
    adapter: await page.locator("[data-testid='adapter-name']").textContent(),
    captures: {
      canvasCapture,
      continentScaleCapture,
      errorCapture,
      orbitCapture,
      pageCapture,
    },
    comparison: await page.locator("[data-testid='terrain-diagnostics']").innerText(),
    comparisonMetrics,
    continentScaleMetrics,
    finalUrl,
    target: externalBaseUrl ? "hosted" : "local-preview",
    launch: {
      autoConfiguredWayland: launch.autoConfiguredWayland,
      headed: launch.headed,
      useWayland: launch.useWayland,
      waylandDisplay: launch.waylandDisplay,
    },
    viewport: await page.evaluate(() => ({
      devicePixelRatio: window.devicePixelRatio,
      height: window.innerHeight,
      width: window.innerWidth,
    })),
  };
  const reportPath = `/tmp/mclone-terrain-lab-${label}-report.json`;
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({ ...report, reportPath }, null, 2)}\n`);
} finally {
  if (browser) {
    await browser.close();
  }
  if (server && server.exitCode === null) {
    server.kill("SIGTERM");
  }
}

function runBuild() {
  const result = spawnSync("pnpm", ["web:build"], {
    cwd: terrainLabRoot,
    encoding: "utf8",
    env: process.env,
    stdio: "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`Terrain Lab build failed with status ${String(result.status)}`);
  }
}

function startPreview() {
  const child = spawn(
    "pnpm",
    [
      "exec",
      "vite",
      "preview",
      "--config",
      "src/web/vite.config.ts",
      "--host",
      "127.0.0.1",
      "--port",
      String(port),
      "--strictPort",
    ],
    {
      cwd: terrainLabRoot,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  child.stdout.on("data", (chunk) => process.stdout.write(chunk));
  child.stderr.on("data", (chunk) => process.stderr.write(chunk));
  return child;
}

async function waitForPreview() {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (server.exitCode !== null) {
      throw new Error(`Terrain Lab preview exited with status ${String(server.exitCode)}`);
    }
    try {
      const response = await fetch(`${baseUrl}/terrain/`);
      if (response.ok) {
        return;
      }
    } catch {
      // Vite has not bound its socket yet.
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error("Timed out waiting for the Terrain Lab preview server");
}

async function waitForComparison(shell) {
  await shell.waitFor({ state: "visible" });
  await shell.page().waitForFunction(
    () => {
      const element = document.querySelector(".appShell");
      const renderRevision = Number(element?.getAttribute("data-render-revision") ?? "0");
      const comparisonRevision = Number(
        element?.getAttribute("data-comparison-revision") ?? "0",
      );
      const diagnostics = document.querySelector("[data-testid='terrain-diagnostics']");
      return renderRevision > 0
        && comparisonRevision === renderRevision
        && !diagnostics?.textContent?.includes("pending");
    },
  );
}

async function waitForRevision(shell, previousRevision) {
  await shell.page().waitForFunction(
    (previous) => {
      const value = document
        .querySelector(".appShell")
        ?.getAttribute("data-render-revision");
      const comparison = document
        .querySelector(".appShell")
        ?.getAttribute("data-comparison-revision");
      const diagnostics = document.querySelector("[data-testid='terrain-diagnostics']");
      return Number(value ?? "0") > previous
        && Number(comparison ?? "0") === Number(value ?? "0")
        && !diagnostics?.textContent?.includes("pending");
    },
    previousRevision,
  );
}

async function settlePaint(page) {
  await page.evaluate(() => new Promise((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(resolve));
  }));
}

async function numericAttribute(locator, name) {
  const value = Number(await locator.getAttribute(name));
  if (!Number.isFinite(value)) {
    throw new Error(`Terrain Lab attribute ${name} is not numeric`);
  }
  return value;
}

async function readComparisonMetrics(shell) {
  return {
    baseMeanError: await numericAttribute(shell, "data-base-mean-error"),
    baseP95Error: await numericAttribute(shell, "data-base-p95-error"),
    continentalnessError: await numericAttribute(shell, "data-continentalness-error"),
    oceanAgreement: await numericAttribute(shell, "data-ocean-agreement"),
  };
}

function assertLargeFieldMetrics(metrics, label) {
  if (metrics.baseMeanError > 0.01
      || metrics.baseP95Error > 0.01
      || metrics.continentalnessError > 0.001
      || metrics.oceanAgreement < 0.999) {
    throw new Error(
      `Production large-field comparison regressed at ${label}: ${JSON.stringify(metrics)}`,
    );
  }
}
