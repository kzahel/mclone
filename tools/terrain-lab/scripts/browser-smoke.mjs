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
const largeCanonical = process.argv.includes("--large-canonical");
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
    `${baseUrl}/terrain/?seed=-98765&x=-304&z=336&blocks=512&detail=auto`
      + "&panes=canonical%2Ccpu%2Cgpu&canonical=final&radius=1"
      + "&water=1&vegetation=1&view=3d&layer=terrain",
    { waitUntil: "networkidle" },
  );
  const shell = page.locator(".appShell");
  await page.locator("[data-testid='lab-status']").waitFor({ state: "visible" });
  try {
    await waitForComparison(shell);
  } catch (error) {
    const failureCapture = `/tmp/mclone-terrain-lab-${label}-comparison-timeout.png`;
    await page.screenshot({ path: failureCapture, fullPage: true });
    const statusText = await page.locator("[data-testid='lab-status']").textContent();
    throw new Error(
      `${error instanceof Error ? error.message : String(error)}\n`
      + `Status: ${statusText ?? "missing"}\n`
      + `Browser errors: ${pageErrors.join("\n") || "none"}\n`
      + `Capture: ${failureCapture}`,
    );
  }
  await waitForCanonical(shell, 9);
  const sourceControls = page.locator("[data-testid='preview-source-controls']");
  await sourceControls.waitFor({ state: "visible" });
  const sourceGuide = await sourceControls.innerText();
  if (!sourceGuide.includes("Same coordinates, independent readiness")) {
    throw new Error(`Terrain Lab compare guidance is missing:\n${sourceGuide}`);
  }
  const expectedCompareLayout = mobile ? "stacked" : "side-by-side";
  if (await shell.getAttribute("data-compare-layout") !== expectedCompareLayout
      || Number(await shell.getAttribute("data-vertex-count")) <= 49_152
      || await shell.getAttribute("data-target-ready") !== "true") {
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
  assertLargeFieldMetrics(comparisonMetrics, "512 block review");

  const pageCapture = `/tmp/mclone-terrain-lab-${label}.png`;
  const canonicalCapture = `/tmp/mclone-terrain-lab-${label}-canonical.png`;
  const canvasCapture = `/tmp/mclone-terrain-lab-${label}-canvas.png`;
  const orbitCapture = `/tmp/mclone-terrain-lab-${label}-orbit.png`;
  const errorCapture = `/tmp/mclone-terrain-lab-${label}-map-error.png`;
  const continentScaleCapture = `/tmp/mclone-terrain-lab-${label}-continent-scale.png`;
  const stressRaceCapture = `/tmp/mclone-terrain-lab-${label}-stress-race.png`;
  await page.screenshot({ path: pageCapture, fullPage: true });
  await page.locator("canvas[aria-label='Canonical textured terrain preview']").screenshot({
    path: canonicalCapture,
  });
  await page.locator("canvas[aria-label='Live GPU terrain preview']").screenshot({
    path: canvasCapture,
  });

  const stage = page.locator("[data-testid='terrain-stage']");
  const layoutStageBox = await stage.boundingBox();
  if (!layoutStageBox) {
    throw new Error("Terrain Lab stage has no interactive bounds");
  }
  const sourceControlsBox = await sourceControls.boundingBox();
  const viewportControlsBox = await page
    .locator("[data-testid='viewport-controls']")
    .boundingBox();
  if (!sourceControlsBox || !viewportControlsBox
      || sourceControlsBox.y + sourceControlsBox.height > viewportControlsBox.y + 1
      || viewportControlsBox.y + viewportControlsBox.height > layoutStageBox.y + 1) {
    throw new Error("Terrain Lab source and viewport controls are not adjacent to the preview");
  }
  await stage.scrollIntoViewIfNeeded();
  const stageBox = await stage.boundingBox();
  if (!stageBox) {
    throw new Error("Terrain Lab stage disappeared after scrolling");
  }
  const viewportHeight = page.viewportSize()?.height ?? 900;
  const orbitStartY = Math.max(
    100,
    Math.min(stageBox.y + 200, viewportHeight - 100),
  );
  await page.mouse.move(
    stageBox.x + stageBox.width * 0.5,
    orbitStartY,
  );
  await page.mouse.down();
  await page.mouse.move(
    stageBox.x + stageBox.width * 0.7,
    orbitStartY - 80,
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
  let navigationRevision = resetRevision;
  if (!mobile) {
    const beforePanUrl = page.url();
    const beforePanYaw = Number(await shell.getAttribute("data-camera-yaw"));
    const beforePanPitch = Number(await shell.getAttribute("data-camera-pitch"));
    await page.mouse.move(
      stageBox.x + stageBox.width * 0.5,
      orbitStartY,
    );
    await page.mouse.down({ button: "right" });
    await page.mouse.move(
      stageBox.x + stageBox.width * 0.6,
      orbitStartY + 50,
    );
    await page.mouse.up({ button: "right" });
    await page.waitForFunction(
      (previous) => window.location.href !== previous,
      beforePanUrl,
    );
    await waitForRevision(shell, resetRevision);
    navigationRevision = Number(await shell.getAttribute("data-render-revision"));
    const afterPanYaw = Number(await shell.getAttribute("data-camera-yaw"));
    const afterPanPitch = Number(await shell.getAttribute("data-camera-pitch"));
    if (Math.abs(afterPanYaw - beforePanYaw) > 1e-6
        || Math.abs(afterPanPitch - beforePanPitch) > 1e-6) {
      throw new Error(
        `Right-button pan changed orbit: yaw ${beforePanYaw} -> ${afterPanYaw}, `
        + `pitch ${beforePanPitch} -> ${afterPanPitch}`,
      );
    }
  }
  const beforeArrowUrl = page.url();
  await stage.focus();
  await page.keyboard.press("ArrowRight");
  await waitForRevision(shell, navigationRevision);
  if (page.url() === beforeArrowUrl) {
    throw new Error("Focused preview ArrowRight did not pan terrain");
  }
  navigationRevision = Number(await shell.getAttribute("data-render-revision"));

  await page.getByLabel("Diagnostic layer").selectOption("error");
  await waitForRevision(shell, navigationRevision);
  const errorRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByRole("button", { name: "Map", exact: true }).click();
  await page.getByRole("button", { name: "Zoom out" }).click();
  await waitForRevision(shell, errorRevision);
  await page.evaluate(() => window.scrollTo(0, 0));
  await settlePaint(page);
  await page.screenshot({ path: errorCapture, fullPage: true });

  const localMapRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByLabel("LOD content checkpoint").selectOption("cover");
  await waitForRevision(shell, localMapRevision);
  const coverRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByLabel("Diagnostic layer").selectOption("terrain");
  await waitForRevision(shell, coverRevision);
  const continentStartRevision = Number(await shell.getAttribute("data-render-revision"));
  for (let index = 0; index < 6; index += 1) {
    await page.getByRole("button", { name: "Zoom out" }).click();
  }
  await waitForRevision(shell, continentStartRevision);
  let vegetationRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByRole("button", { name: "Cold current view" }).click();
  await waitForRevision(shell, vegetationRevision);
  const continentScaleMetrics = await readComparisonMetrics(shell);
  assertLargeFieldMetrics(continentScaleMetrics, "65.5 km Cover");
  const vegetationScaleCold = await readVegetationScaleBenchmark(shell);
  assertVegetationScaleBenchmark(vegetationScaleCold, "65.5 km cold Cover", {
    expectCpu: true,
    expectCold: true,
  });
  await page.locator("canvas[aria-label='Live GPU terrain preview']").screenshot({
    path: continentScaleCapture,
  });

  vegetationRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByRole("button", { name: "Zoom in" }).click();
  await waitForRevision(shell, vegetationRevision);
  vegetationRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByRole("button", { name: "Zoom out" }).click();
  await waitForRevision(shell, vegetationRevision);
  const vegetationScaleWarm = await readVegetationScaleBenchmark(shell);
  if (vegetationScaleWarm.cacheHits <= 0) {
    throw new Error(
      `65.5 km warm Cover did not reuse cached tiles: ${
        JSON.stringify(vegetationScaleWarm)
      }`,
    );
  }

  await page.getByRole("button", { name: "Cache off", exact: true }).click();
  vegetationRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByRole("button", { name: "Cold current view" }).click();
  await waitForRevision(shell, vegetationRevision);
  const vegetationScaleCacheOff = await readVegetationScaleBenchmark(shell);
  assertVegetationScaleBenchmark(vegetationScaleCacheOff, "65.5 km cache-off Cover", {
    expectCpu: true,
    expectCacheOff: true,
  });

  const finalUrl = page.url();
  if (!finalUrl.includes("layer=terrain")
      || !finalUrl.includes("view=map")
      || !finalUrl.includes("blocks=65536")
      || !finalUrl.includes("detail=auto")
      || !finalUrl.includes("stage=cover")) {
    throw new Error(`Terrain Lab controls did not round-trip through the URL: ${finalUrl}`);
  }

  const gpuOnlyUrl = new URL(finalUrl);
  gpuOnlyUrl.searchParams.set("panes", "gpu");
  await page.goto(gpuOnlyUrl.href, { waitUntil: "networkidle" });
  await page.locator("[data-testid='lab-status']").waitFor({ state: "visible" });
  await waitForTerrainTarget(shell);
  const vegetationScaleGpuOnly = await readVegetationScaleBenchmark(shell);
  assertVegetationScaleBenchmark(vegetationScaleGpuOnly, "65.5 km GPU-only Cover", {
    expectCpu: false,
  });
  const gpuIndependentCapture =
    `/tmp/mclone-terrain-lab-${label}-vegetation-cover-gpu-independent.png`;
  await page.locator("canvas[aria-label='Live GPU terrain preview']").screenshot({
    path: gpuIndependentCapture,
  });
  let gpuRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByLabel("Diagnostic layer").selectOption("forests");
  await waitForTerrainTarget(shell, gpuRevision);
  const forestScaleCaptures = {};
  for (const spacing of [1024, 512, 256]) {
    gpuRevision = Number(await shell.getAttribute("data-render-revision"));
    await page.getByLabel("Terrain resolution").selectOption(String(spacing));
    await waitForTerrainTarget(shell, gpuRevision, false);
    const effectiveSpacing = Number(await shell.getAttribute("data-effective-spacing"));
    if (effectiveSpacing !== spacing) {
      throw new Error(
        `Forest scale matrix requested 1:${spacing}, got 1:${effectiveSpacing}`,
      );
    }
    const capture =
      `/tmp/mclone-terrain-lab-${label}-forest-summary-map-${spacing}.png`;
    await page.locator("canvas[aria-label='Live GPU terrain preview']").screenshot({
      path: capture,
    });
    forestScaleCaptures[`map${spacing}`] = capture;
  }
  gpuRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByRole("button", { name: "3D terrain", exact: true }).click();
  await waitForTerrainTarget(shell, gpuRevision, false);
  const forestThreeDimensionalCapture =
    `/tmp/mclone-terrain-lab-${label}-forest-summary-3d-256.png`;
  await page.locator("canvas[aria-label='Live GPU terrain preview']").screenshot({
    path: forestThreeDimensionalCapture,
  });
  forestScaleCaptures.threeDimensional256 = forestThreeDimensionalCapture;

  await page.evaluate(() => {
    const race = [];
    const shell = document.querySelector(".appShell");
    const record = () => {
      const state = `${shell?.getAttribute("data-cpu-target-ready")}/${
        shell?.getAttribute("data-gpu-target-ready")
      }`;
      if (race.at(-1) !== state) {
        race.push(state);
      }
    };
    record();
    const observer = new MutationObserver(record);
    observer.observe(shell, {
      attributes: true,
      attributeFilter: ["data-cpu-target-ready", "data-gpu-target-ready"],
    });
    window.terrainLabRaceStates = race;
    window.terrainLabRaceObserver = observer;
  });
  const beforeStressRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByRole("button", { name: "Run stress race" }).click();
  await waitForStressRace(shell, beforeStressRevision);
  const stressBenchmark = {
    cacheEnabled: await shell.getAttribute("data-cache-enabled"),
    cacheHits: Number(await shell.getAttribute("data-cache-hits")),
    cpuRequestMs: Number(await shell.getAttribute("data-cpu-request-ms")),
    cpuTargetMs: Number(await shell.getAttribute("data-cpu-target-ms")),
    cpuTiles: Number(await shell.getAttribute("data-request-cpu-tiles")),
    effectiveSpacing: Number(await shell.getAttribute("data-effective-spacing")),
    gpuTargetMs: Number(await shell.getAttribute("data-gpu-target-ms")),
    gpuTiles: Number(await shell.getAttribute("data-request-gpu-tiles")),
    raceStates: [],
    samplesPerAxis: Number(await shell.getAttribute("data-samples-per-axis")),
    stressUrl: page.url(),
    visibleTiles: Number(await shell.getAttribute("data-visible-tiles")),
  };
  const stressComparison = await page
    .locator("[data-testid='terrain-diagnostics']")
    .innerText();
  const stressMetrics = await readComparisonMetrics(shell);
  assertLargeFieldMetrics(stressMetrics, "cold stress race");
  const raceStates = await page.evaluate(() => {
    window.terrainLabRaceObserver?.disconnect();
    return window.terrainLabRaceStates ?? [];
  });
  if (!raceStates.some((state) => state === "true/false" || state === "false/true")) {
    throw new Error(`CPU/GPU panels never published independently: ${raceStates.join(", ")}`);
  }
  stressBenchmark.raceStates = raceStates;
  await settlePaint(page);
  await page.screenshot({ path: stressRaceCapture, fullPage: true });
  if (stressBenchmark.cacheEnabled !== "false"
      || stressBenchmark.cacheHits !== 0
      || stressBenchmark.cpuTargetMs <= 0
      || stressBenchmark.gpuTargetMs <= 0) {
    throw new Error(`Cold stress benchmark is incomplete: ${JSON.stringify(stressBenchmark)}`);
  }
  const adapterName = await page.locator("[data-testid='adapter-name']").textContent();
  const viewport = await page.evaluate(() => ({
    devicePixelRatio: window.devicePixelRatio,
    height: window.innerHeight,
    width: window.innerWidth,
  }));
  const largeCanonicalReport = largeCanonical
    ? await proveLargeCanonicalFootprint(page, shell, label)
    : undefined;
  if (pageErrors.length > 0) {
    throw new Error(`Browser errors:\n${pageErrors.join("\n")}`);
  }

  const report = {
    adapter: adapterName,
    captures: {
      canonicalCapture,
      canvasCapture,
      continentScaleCapture,
      errorCapture,
      forestScaleCaptures,
      gpuIndependentCapture,
      orbitCapture,
      pageCapture,
      stressRaceCapture,
    },
    comparison: stressComparison,
    comparisonMetrics,
    continentScaleMetrics,
    finalUrl,
    largeCanonical: largeCanonicalReport,
    stressBenchmark,
    stressMetrics,
    vegetationScale: {
      cacheOff: vegetationScaleCacheOff,
      cold: vegetationScaleCold,
      gpuOnly: vegetationScaleGpuOnly,
      warm: vegetationScaleWarm,
    },
    target: externalBaseUrl ? "hosted" : "local-preview",
    launch: {
      autoConfiguredWayland: launch.autoConfiguredWayland,
      headed: launch.headed,
      useWayland: launch.useWayland,
      waylandDisplay: launch.waylandDisplay,
    },
    viewport,
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
      return renderRevision > 0 && comparisonRevision === renderRevision;
    },
  );
}

async function waitForCanonical(shell, requestedChunks, timeout = 30_000) {
  await shell.page().waitForFunction(
    (requested) => {
      const element = document.querySelector(".appShell");
      return element?.getAttribute("data-canonical-complete") === "true"
        && Number(element.getAttribute("data-canonical-published")) === requested
        && Number(element.getAttribute("data-canonical-requested")) === requested;
    },
    requestedChunks,
    { timeout },
  );
}

async function proveLargeCanonicalFootprint(page, shell, label) {
  await page.goto(
    `${baseUrl}/terrain/?seed=-98765&profile=mclone-overworld-v1`
      + "&x=-304&z=336&blocks=512&detail=auto"
      + "&panes=canonical&canonical=final&radius=0"
      + "&water=1&vegetation=1&stage=hydrology&view=map&layer=terrain",
    { waitUntil: "networkidle" },
  );
  await waitForCanonical(shell, 1);
  const footprint = page.getByLabel("Exact chunk footprint");
  if (await footprint.locator("option").count() !== 9) {
    throw new Error("The exact footprint selector does not expose nine stepped sizes");
  }

  const initialStarted = performance.now();
  await footprint.selectOption("15");
  await waitForCanonical(shell, 961, 180_000);
  const initialMs = performance.now() - initialStarted;
  const residentRawBytes = await numericAttribute(
    shell,
    "data-canonical-resident-raw-bytes",
  );
  const cacheRawBytes = await numericAttribute(shell, "data-canonical-cache-raw-bytes");
  const meshUsedBytes = await numericAttribute(shell, "data-canonical-mesh-used-bytes");
  const trackedBytes = await numericAttribute(shell, "data-canonical-tracked-bytes");
  const initialWorkerGenerationMs = await numericAttribute(
    shell,
    "data-canonical-worker-generation-ms",
  );
  const initialWorkerMeshMs = await numericAttribute(
    shell,
    "data-canonical-worker-mesh-ms",
  );
  const initialWorkerPresentationMs = await numericAttribute(
    shell,
    "data-canonical-worker-presentation-ms",
  );
  const initialWorkerPackMs = await numericAttribute(
    shell,
    "data-canonical-worker-pack-ms",
  );
  const initialWorkerTransferMs = await numericAttribute(
    shell,
    "data-canonical-worker-transfer-ms",
  );
  const initialMainDecodeMs = await numericAttribute(
    shell,
    "data-canonical-main-decode-ms",
  );
  const initialMeshUploadMs = await numericAttribute(
    shell,
    "data-canonical-mesh-upload-ms",
  );
  const initialMaxAdmissionMs = await numericAttribute(
    shell,
    "data-canonical-max-admission-ms",
  );
  const initialMeshTargetChunks = await numericAttribute(
    shell,
    "data-canonical-mesh-target-chunks",
  );
  if (trackedBytes !== residentRawBytes + cacheRawBytes + meshUsedBytes) {
    throw new Error("The large exact tracked-memory lower bound is inconsistent");
  }
  if (residentRawBytes !== 0
      || initialWorkerMeshMs <= 0
      || initialWorkerGenerationMs <= 0
      || initialWorkerPresentationMs <= 0
      || initialWorkerPackMs <= 0
      || initialWorkerTransferMs <= 0
      || initialMainDecodeMs <= 0
      || initialMeshUploadMs <= 0
      || initialMaxAdmissionMs <= 0
      || initialMeshTargetChunks <= 961) {
    throw new Error("The large exact Worker/main timing evidence is incomplete");
  }

  const centerX = page.getByLabel("Center X");
  const initialEpoch = await numericAttribute(shell, "data-canonical-epoch");
  const shiftStarted = performance.now();
  await centerX.fill("-288");
  await centerX.press("Enter");
  await waitForCanonicalEpochAfter(shell, initialEpoch);
  await waitForCanonicalAttribute(shell, "data-canonical-resident-hits", "930");
  await waitForCanonical(shell, 961, 120_000);
  const shiftMs = performance.now() - shiftStarted;
  const shiftWorkerMeshMs = await numericAttribute(
    shell,
    "data-canonical-worker-mesh-ms",
  );
  const shiftMainDecodeMs = await numericAttribute(
    shell,
    "data-canonical-main-decode-ms",
  );
  const shiftMeshTargetChunks = await numericAttribute(
    shell,
    "data-canonical-mesh-target-chunks",
  );
  if (await shell.getAttribute("data-canonical-admission-frames") !== "31"
      || await shell.getAttribute("data-canonical-max-frame-admissions") !== "1"
      || shiftWorkerMeshMs <= 0
      || shiftMainDecodeMs <= 0
      || shiftMeshTargetChunks <= 31
      || shiftMeshTargetChunks >= 155) {
    throw new Error("The large exact entering edge was not admitted one chunk per frame");
  }

  const shiftedEpoch = await numericAttribute(shell, "data-canonical-epoch");
  const returnStarted = performance.now();
  await centerX.fill("-304");
  await centerX.press("Enter");
  await waitForCanonicalEpochAfter(shell, shiftedEpoch);
  await waitForCanonicalAttribute(shell, "data-canonical-resident-hits", "930");
  await waitForCanonical(shell, 961, 120_000);
  const returnMs = performance.now() - returnStarted;
  const cachedChunks = await numericAttribute(shell, "data-canonical-cached-chunks");
  const warmHits = await numericAttribute(shell, "data-canonical-warm-hits");
  const warmChunks = await numericAttribute(shell, "data-canonical-warm-chunks");
  const returnWorkerMeshMs = await numericAttribute(
    shell,
    "data-canonical-worker-mesh-ms",
  );
  const returnMainDecodeMs = await numericAttribute(
    shell,
    "data-canonical-main-decode-ms",
  );
  if (await shell.getAttribute("data-canonical-cache-hits") !== "0"
      || warmHits !== 31
      || warmChunks !== 31
      || returnWorkerMeshMs !== 0
      || returnMainDecodeMs !== 0
      || await shell.getAttribute("data-canonical-admission-frames") !== "31"
      || cachedChunks !== 992) {
    throw new Error("The large exact warm return violated its bounded paced contract");
  }

  const capture = `/tmp/mclone-terrain-lab-hosted-${label}-canonical-961.png`;
  await page.getByTestId("canonical-terrain-stage").screenshot({ path: capture });
  return {
    admissionFrames: 31,
    cacheHits: 0,
    cachedChunks,
    capture,
    initialMs,
    initialWorkerGenerationMs,
    initialWorkerMeshMs,
    initialWorkerPresentationMs,
    initialWorkerPackMs,
    initialWorkerTransferMs,
    initialMainDecodeMs,
    initialMeshUploadMs,
    initialMaxAdmissionMs,
    initialMeshTargetChunks,
    meshUsedBytes,
    residentHits: 930,
    residentRawBytes,
    returnMs,
    shiftMs,
    shiftWorkerMeshMs,
    shiftMainDecodeMs,
    shiftMeshTargetChunks,
    trackedBytes,
    cacheRawBytes,
    warmHits,
    warmChunks,
    returnWorkerMeshMs,
    returnMainDecodeMs,
  };
}

async function waitForCanonicalAttribute(shell, name, expected) {
  await shell.page().waitForFunction(
    ([attribute, value]) =>
      document.querySelector(".appShell")?.getAttribute(attribute) === value,
    [name, expected],
  );
}

async function waitForCanonicalEpochAfter(shell, previous) {
  await shell.page().waitForFunction(
    (epoch) =>
      Number(
        document.querySelector(".appShell")?.getAttribute("data-canonical-epoch") ?? "0",
      ) > epoch,
    previous,
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
      return Number(value ?? "0") > previous
        && Number(comparison ?? "0") === Number(value ?? "0");
    },
    previousRevision,
    { timeout: mobile ? 60_000 : 30_000 },
  );
}

async function waitForStressRace(shell, previousRevision) {
  await shell.page().waitForFunction(
    (previous) => {
      const element = document.querySelector(".appShell");
      const render = Number(element?.getAttribute("data-render-revision") ?? "0");
      const comparison = Number(element?.getAttribute("data-comparison-revision") ?? "0");
      return render > previous
        && comparison === render
        && element?.getAttribute("data-cpu-target-ready") === "true"
        && element?.getAttribute("data-gpu-target-ready") === "true"
        && Number(element?.getAttribute("data-cpu-target-ms") ?? "0") > 0
        && Number(element?.getAttribute("data-gpu-target-ms") ?? "0") > 0
        && Number(element?.getAttribute("data-request-cpu-tiles") ?? "0") > 0
        && Number(element?.getAttribute("data-request-gpu-tiles") ?? "0") > 0
        && document.querySelector("[data-testid='lab-status']")
          ?.textContent?.toLowerCase().includes("ready");
    },
    previousRevision,
    { timeout: mobile ? 60_000 : 30_000 },
  );
}

async function waitForTerrainTarget(shell, previousRevision = 0, requireWork = true) {
  await shell.page().waitForFunction(
    ([previous, workRequired]) => {
      const element = document.querySelector(".appShell");
      return Number(element?.getAttribute("data-render-revision") ?? "0") > previous
        && element?.getAttribute("data-target-ready") === "true"
        && element?.getAttribute("data-gpu-target-ready") === "true"
        && (!workRequired
          || (Number(element?.getAttribute("data-request-gpu-tiles") ?? "0") > 0
            && Number(element?.getAttribute("data-gpu-target-ms") ?? "0") > 0))
        && document.querySelector("[data-testid='lab-status']")
          ?.textContent?.toLowerCase().includes("ready");
    },
    [previousRevision, requireWork],
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
    materialAgreement: await numericAttribute(shell, "data-material-agreement"),
  };
}

async function readVegetationScaleBenchmark(shell) {
  const attributes = {
    cacheEnabled: "data-cache-enabled",
    cacheHits: "data-cache-hits",
    cpuForestEvaluations: "data-cpu-forest-evaluations",
    cpuFootprintSummaries: "data-cpu-footprint-summaries",
    cpuLatticePoints: "data-cpu-lattice-points",
    cpuPackUploadMs: "data-cpu-pack-upload-request-ms",
    cpuReferenceMs: "data-cpu-request-ms",
    cpuSummaryTiles: "data-cpu-vegetation-summary-tiles",
    cpuTerrainEvaluations: "data-cpu-terrain-evaluations",
    cpuTiles: "data-request-cpu-tiles",
    cpuVegetationMs: "data-cpu-vegetation-request-ms",
    effectiveSpacing: "data-effective-spacing",
    footprintBlocks: "data-footprint-blocks",
    gpuForestEvaluations: "data-gpu-forest-evaluations",
    gpuFootprintSummaries: "data-gpu-footprint-summaries",
    gpuLatticePoints: "data-gpu-lattice-points",
    gpuSampleBytes: "data-gpu-sample-bytes",
    gpuSummaryTiles: "data-gpu-vegetation-summary-tiles",
    gpuTargetMs: "data-gpu-target-ms",
    gpuTerrainEvaluations: "data-gpu-terrain-evaluations",
    gpuTiles: "data-request-gpu-tiles",
    readbackBytes: "data-request-readback-bytes",
    recordTiles: "data-vegetation-record-tiles",
    referenceBytes: "data-reference-bytes",
    residentBytes: "data-resident-bytes",
    retainedVegetationCells: "data-retained-vegetation-cells",
    treeInstances: "data-tree-instances",
    vegetationCellRequests: "data-vegetation-cell-requests",
    visibleTiles: "data-visible-tiles",
  };
  const snapshot = await shell.evaluate(
    (element, names) => Object.fromEntries(
      Object.entries(names).map(([key, attribute]) => [
        key,
        element.getAttribute(attribute),
      ]),
    ),
    attributes,
  );
  const benchmark = {};
  for (const key of Object.keys(attributes)) {
    if (key === "cacheEnabled") {
      benchmark[key] = snapshot[key];
    } else {
      const value = Number(snapshot[key]);
      if (!Number.isFinite(value)) {
        throw new Error(`Terrain Lab benchmark attribute ${attributes[key]} is not numeric`);
      }
      benchmark[key] = value;
    }
  }
  return benchmark;
}

function assertVegetationScaleBenchmark(benchmark, label, {
  expectCpu,
  expectCacheOff = false,
  expectCold = false,
}) {
  const expectedGpuLattice = benchmark.gpuTiles * 65 * 65;
  if (benchmark.footprintBlocks !== 65_536
      || benchmark.effectiveSpacing < 8
      || (benchmark.effectiveSpacing & (benchmark.effectiveSpacing - 1)) !== 0
      || benchmark.gpuTiles <= 0
      || benchmark.gpuLatticePoints !== expectedGpuLattice
      || benchmark.gpuTerrainEvaluations !== benchmark.gpuLatticePoints * 5
      || benchmark.gpuForestEvaluations !== benchmark.gpuLatticePoints * 4
      || benchmark.gpuFootprintSummaries !== benchmark.gpuLatticePoints
      || benchmark.gpuSummaryTiles <= 0
      || benchmark.gpuTargetMs <= 0
      || benchmark.gpuSampleBytes <= 0
      || benchmark.readbackBytes <= 0
      || benchmark.recordTiles !== 0
      || benchmark.treeInstances !== 0
      || benchmark.vegetationCellRequests !== 0
      || benchmark.retainedVegetationCells !== 0) {
    throw new Error(`${label} violated GPU summary bounds: ${JSON.stringify(benchmark)}`);
  }
  if (expectCpu) {
    const expectedCpuLattice = benchmark.cpuTiles * 65 * 65;
    if (benchmark.cpuTiles <= 0
        || benchmark.cpuLatticePoints !== expectedCpuLattice
        || benchmark.cpuTerrainEvaluations !== benchmark.cpuLatticePoints * 5
        || benchmark.cpuForestEvaluations !== benchmark.cpuLatticePoints * 4
        || benchmark.cpuFootprintSummaries !== benchmark.cpuLatticePoints
        || benchmark.cpuSummaryTiles <= 0
        || benchmark.cpuReferenceMs <= 0
        || benchmark.cpuPackUploadMs <= 0
        || benchmark.referenceBytes <= 0) {
      throw new Error(`${label} violated CPU summary bounds: ${JSON.stringify(benchmark)}`);
    }
  } else if (benchmark.cpuTiles !== 0
      || benchmark.cpuLatticePoints !== 0
      || benchmark.cpuTerrainEvaluations !== 0
      || benchmark.cpuForestEvaluations !== 0
      || benchmark.cpuFootprintSummaries !== 0
      || benchmark.cpuSummaryTiles !== 0
      || benchmark.referenceBytes !== 0) {
    throw new Error(`${label} waited for CPU vegetation: ${JSON.stringify(benchmark)}`);
  }
  if (expectCacheOff
      && (benchmark.cacheEnabled !== "false" || benchmark.cacheHits !== 0)) {
    throw new Error(`${label} did not stay cold with cache off: ${JSON.stringify(benchmark)}`);
  }
  if (expectCold && benchmark.cacheHits !== 0) {
    throw new Error(`${label} inherited cached tiles: ${JSON.stringify(benchmark)}`);
  }
}

function assertLargeFieldMetrics(metrics, label) {
  if (metrics.baseMeanError > 0.01
      || metrics.baseP95Error > 0.01
      || metrics.continentalnessError > 0.001
      || metrics.oceanAgreement < 0.999
      || metrics.materialAgreement < 0.999) {
    throw new Error(
      `Production large-field comparison regressed at ${label}: ${JSON.stringify(metrics)}`,
    );
  }
}
