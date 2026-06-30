import { chromium } from "@playwright/test";
import { spawn, spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { existsSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, normalize, resolve, sep } from "node:path";
import { inflateSync } from "node:zlib";
import { buildWebGlue, stagedWebRoot } from "./build-web-glue.mjs";

/**
 * @typedef {import("@playwright/test").Page} Page
 * @typedef {import("@playwright/test").Locator} Locator
 */

const scriptDir = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(scriptDir, "..");
const nativeRoot = resolve(appRoot, "../..");
const repoRoot = resolve(nativeRoot, "..");
const referenceAssetPackPath = join(repoRoot, "reference", "minecraft-1.17.1", "extracted.zip");
const wasmBindgenVersion = "0.2.125";
const wasmPath = join(
  nativeRoot,
  "target",
  "wasm32-unknown-unknown",
  "debug",
  "mclone_web_client.wasm",
);
const bindgenToolRoot = join(nativeRoot, "target", `wasm-bindgen-cli-${wasmBindgenVersion}`);
const bindgenBin = join(bindgenToolRoot, "bin", process.platform === "win32" ? "wasm-bindgen.exe" : "wasm-bindgen");
const bindgenOutDir = join(
  nativeRoot,
  "target",
  "wasm32-unknown-unknown",
  "debug",
  "mclone-web-client-bindgen",
);
const movementPerf = process.argv.includes("--movement-perf")
  || process.env.MCLONE_NATIVE_WEB_MOVEMENT_PERF === "1";
const blockEditProbe = process.argv.includes("--block-edit-probe")
  || process.env.MCLONE_NATIVE_WEB_BLOCK_EDIT_PROBE === "1";
const remoteWebSocket = process.argv.includes("--remote-websocket")
  || process.env.MCLONE_NATIVE_WEB_REMOTE_WEBSOCKET === "1";
const appLoop = movementPerf
  || blockEditProbe
  || remoteWebSocket
  || process.argv.includes("--app-loop")
  || process.argv.includes("--mobile-app-loop")
  || process.env.MCLONE_NATIVE_WEB_APP_LOOP === "1";
const mobileAppLoop = process.argv.includes("--mobile-app-loop")
  || process.env.MCLONE_NATIVE_WEB_MOBILE_APP_LOOP === "1";
const mobileViewport = mobileAppLoop || movementPerf;
const serveOnly = process.argv.includes("--serve")
  || process.env.MCLONE_NATIVE_WEB_SERVE === "1";
const screenshotPath = process.env.MCLONE_NATIVE_WEB_SMOKE_SCREENSHOT
  ?? (movementPerf
    ? "/tmp/mclone-native-web-movement-perf.png"
    : blockEditProbe
    ? "/tmp/mclone-native-web-block-edit-probe.png"
    : mobileAppLoop
    ? "/tmp/mclone-native-web-mobile-app.png"
    : appLoop ? "/tmp/mclone-native-web-app.png" : "/tmp/mclone-native-web-smoke.png");
const canvasScreenshotPath = process.env.MCLONE_NATIVE_WEB_CANVAS_SCREENSHOT
  ?? (movementPerf
    ? "/tmp/mclone-native-web-movement-perf-canvas.png"
    : blockEditProbe
    ? "/tmp/mclone-native-web-block-edit-probe-canvas.png"
    : mobileAppLoop
    ? "/tmp/mclone-native-web-mobile-app-canvas.png"
    : appLoop ? "/tmp/mclone-native-web-app-canvas.png" : "/tmp/mclone-native-web-canvas.png");
const nativeUiCanvasScreenshotPath = process.env.MCLONE_NATIVE_WEB_UI_CANVAS_SCREENSHOT
  ?? "/tmp/mclone-native-web-ui-canvas.png";
const mobileNativeUiCanvasScreenshotPath = process.env.MCLONE_NATIVE_WEB_MOBILE_UI_CANVAS_SCREENSHOT
  ?? "/tmp/mclone-native-web-mobile-ui-canvas.png";
const mobileNativeOptionsCanvasScreenshotPath = process.env.MCLONE_NATIVE_WEB_MOBILE_OPTIONS_CANVAS_SCREENSHOT
  ?? "/tmp/mclone-native-web-mobile-options-canvas.png";
const movementPerfReportPath = process.env.MCLONE_NATIVE_WEB_MOVEMENT_PERF_REPORT
  ?? "/tmp/mclone-native-web-movement-perf.json";
const blockEditProbeReportPath = process.env.MCLONE_NATIVE_WEB_BLOCK_EDIT_PROBE_REPORT
  ?? "/tmp/mclone-native-web-block-edit-probe.json";
const movementPerfChunkBoundaries = Math.max(
  1,
  Number.parseInt(process.env.MCLONE_NATIVE_WEB_MOVEMENT_PERF_CHUNKS ?? "3", 10) || 3,
);
const blockEditProbeBreaks = Math.max(
  1,
  Number.parseInt(process.env.MCLONE_NATIVE_WEB_BLOCK_EDIT_PROBE_BREAKS ?? "1", 10) || 1,
);
const requireChunk = process.argv.includes("--require-chunk")
  || process.env.MCLONE_NATIVE_WEB_REQUIRE_CHUNK === "1";
const requireCanvas = requireChunk
  || process.argv.includes("--require-canvas")
  || process.env.MCLONE_NATIVE_WEB_REQUIRE_CANVAS === "1";
const requireThreading = process.argv.includes("--require-threading")
  || (!process.argv.includes("--skip-threading")
    && process.env.MCLONE_NATIVE_WEB_REQUIRE_THREADING !== "0");
// --build-only compiles the wasm, runs wasm-bindgen (emitting mclone_web_client.d.ts via
// --typescript), stages the browser-loadable web root, and exits before launching a browser. This
// is how native:web:typecheck cheaply materializes the .d.ts its tsconfig path-maps, without a full
// smoke run.
const buildOnly = process.argv.includes("--build-only")
  || process.env.MCLONE_NATIVE_WEB_BUILD_ONLY === "1";
const DIRT_BLOCK_STATE_ID = 5;

run().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});

async function run() {
  buildWasm();
  buildBindgenBundle();
  const webRoot = await buildWebGlue();
  if (buildOnly) {
    console.log(`bindgen output (with mclone_web_client.d.ts) ready in ${bindgenOutDir}`);
    console.log(`staged native web root ready in ${webRoot}`);
    return;
  }
  const server = await startServer(webRoot);
  const remoteServer = remoteWebSocket ? await startNativeWebSocketServer() : null;
  /** @type {import("@playwright/test").Browser | undefined} */
  let browser;
  try {
    const address = server.address();
    const port = address && typeof address === "object" ? address.port : 0;
    const baseUrl = `http://127.0.0.1:${port}`;
    if (serveOnly) {
      await serveUntilStopped(baseUrl, remoteServer);
      return;
    }
    browser = await chromium.launch({
      channel: process.env.PLAYWRIGHT_CHROME_CHANNEL ?? "chrome",
      headless: process.env.HEADED === "1" ? false : true,
      args: [
        "--enable-unsafe-webgpu",
        ...(process.platform === "darwin" ? ["--use-angle=metal"] : []),
      ],
    });
    const page = await browser.newPage(mobileViewport
      ? {
          viewport: { width: 390, height: 844 },
          deviceScaleFactor: 2,
          isMobile: true,
          hasTouch: true,
        }
      : undefined);
    /** @type {string[]} */
    const pageErrors = [];
    /** @type {string[]} */
    const pageLogs = [];
    page.on("pageerror", (error) => pageErrors.push(String(error)));
    page.on("console", (message) => {
      pageLogs.push(`${message.type()}: ${message.text()}`);
      if (message.type() === "error") pageErrors.push(message.text());
    });

    if (appLoop) {
      const appUrl = remoteServer
        ? `${baseUrl}/app.html?remoteWsUrl=${encodeURIComponent(remoteServer.websocketUrl)}`
        : `${baseUrl}/app.html`;
      await page.goto(appUrl, { waitUntil: "load" });
      await page.waitForFunction(
        () => typeof globalThis.__mcloneWebApp !== "undefined",
        undefined,
        { timeout: 20_000 },
      );
      try {
        await page.waitForFunction(
          () => {
            const app = globalThis.__mcloneWebApp;
            return app?.ready === true || app?.state?.failed === true;
          },
          undefined,
          { timeout: 60_000 },
        );
      } catch (error) {
        const state = await page.evaluate(() => globalThis.__mcloneWebApp?.state ?? null);
        throw new Error(`native web app did not finish booting: ${error instanceof Error ? error.message : String(error)}\nstate=${JSON.stringify(state, null, 2)}\nlogs=${pageLogs.join("\n")}`);
      }
      const bootState = await page.evaluate(() => globalThis.__mcloneWebApp.state);
      if (!bootState?.ready || !bootState?.ok) {
        throw new Error(`native web app failed to boot:\n${JSON.stringify(bootState, null, 2)}`);
      }
      const canvas = page.locator("#mclone-canvas");
      if (blockEditProbe) {
        const blockEditProbeResult = await runBlockEditProbe(page, canvas);
        const result = await page.evaluate(() => globalThis.__mcloneWebApp.state);
        let pageScreenshotCaptured = false;
        try {
          await page.screenshot({ path: screenshotPath, fullPage: false, timeout: 5_000 });
          pageScreenshotCaptured = true;
        } catch (error) {
          console.warn(`page screenshot skipped: ${error instanceof Error ? error.message : String(error)}`);
        }
        const canvasPng = await canvas.screenshot({ path: canvasScreenshotPath, timeout: 60_000 });
        const canvasPixels = analyzePng(canvasPng);
        const report = {
          url: appUrl,
          screenshotPath,
          pageScreenshotCaptured,
          canvasScreenshotPath,
          blockEditProbeReportPath,
          appLoop,
          blockEditProbe,
          blockEditProbeBreaks,
          remoteWebSocket,
          remoteWebSocketUrl: remoteServer?.websocketUrl ?? null,
          canvasPixels,
          blockEditProbeResult,
          result,
        };
        await writeFile(blockEditProbeReportPath, `${JSON.stringify(report, null, 2)}\n`);
        assertBlockEditProbeResult(report, pageErrors, canvasPixels);
        console.log(JSON.stringify(report, null, 2));
        return;
      }
      if (movementPerf) {
        const movementPerfProbe = await runMovementPerfProbe(page, canvas);
        const result = await page.evaluate(() => globalThis.__mcloneWebApp.state);
        let pageScreenshotCaptured = false;
        try {
          await page.screenshot({ path: screenshotPath, fullPage: false, timeout: 5_000 });
          pageScreenshotCaptured = true;
        } catch (error) {
          console.warn(`page screenshot skipped: ${error instanceof Error ? error.message : String(error)}`);
        }
        const canvasPng = await canvas.screenshot({ path: canvasScreenshotPath, timeout: 60_000 });
        const canvasPixels = analyzePng(canvasPng);
        const report = {
          url: appUrl,
          screenshotPath,
          pageScreenshotCaptured,
          canvasScreenshotPath,
          movementPerfReportPath,
          appLoop,
          mobileViewport,
          remoteWebSocket,
          remoteWebSocketUrl: remoteServer?.websocketUrl ?? null,
          movementPerf,
          movementPerfChunkBoundaries,
          canvasPixels,
          movementPerfProbe,
          result,
        };
        report.movementPerfProbe.summary = summarizeCompileTimings(
          movementPerfProbe.movementCompileTimings,
        );
        await writeFile(movementPerfReportPath, `${JSON.stringify(report, null, 2)}\n`);
        assertMovementPerfResult(report, pageErrors, canvasPixels);
        console.log(JSON.stringify(report, null, 2));
        return;
      }
      if (mobileAppLoop) {
        await page.waitForFunction(
          () => {
            const state = globalThis.__mcloneWebApp?.state;
            return state?.ok === true
              && state.movementMode === "WALK"
              && state.lastReport?.movementMode === "WALK"
              && state.pendingCompileJobCount === 0;
          },
          undefined,
          { timeout: 60_000 },
        );
        await waitForWebAppStreamingSettled(page);
        const mobileTouchProbe = await exerciseMobileTouchControls(page, canvas);
        await waitForWebAppStreamingSettled(page);
        const result = await page.evaluate(() => globalThis.__mcloneWebApp.state);
        let pageScreenshotCaptured = false;
        try {
          await page.screenshot({ path: screenshotPath, fullPage: false, timeout: 5_000 });
          pageScreenshotCaptured = true;
        } catch (error) {
          console.warn(`page screenshot skipped: ${error instanceof Error ? error.message : String(error)}`);
        }
        const canvasPng = await canvas.screenshot({ path: canvasScreenshotPath, timeout: 60_000 });
        const canvasPixels = analyzePng(canvasPng);

        assertMobileAppLoopResult(result, pageErrors, canvasPixels, mobileTouchProbe);
        console.log(JSON.stringify({
          url: appUrl,
          screenshotPath,
          pageScreenshotCaptured,
          canvasScreenshotPath,
          appLoop,
          mobileAppLoop,
          remoteWebSocket,
          remoteWebSocketUrl: remoteServer?.websocketUrl ?? null,
          canvasPixels,
          mobileTouchProbe,
          result,
        }, null, 2));
        return;
      }
      await canvas.click({ position: { x: 640, y: 360 } });
      await page.waitForFunction(
        () => {
          const state = globalThis.__mcloneWebApp?.state;
          return state?.ok === true
            && state.movementMode === "WALK"
            && state.lastReport?.movementMode === "WALK"
            && state.onGround === true
            && state.pendingCompileJobCount === 0;
        },
        undefined,
        { timeout: 60_000 },
      );
      const walkingStart = await page.evaluate(() => {
        const state = globalThis.__mcloneWebApp.state;
        return {
          cameraX: state.cameraX,
          cameraZ: state.cameraZ,
          commandCount: state.lastReport?.commandCount ?? 0,
        };
      });
      await dispatchKeyboardEvent(page, "keydown", {
        code: "KeyW",
        key: ",",
      });
      try {
        await page.waitForFunction(
          (start) => {
            const state = globalThis.__mcloneWebApp?.state;
            const dx = Number(state?.cameraX) - start.cameraX;
            const dz = Number(state?.cameraZ) - start.cameraZ;
            const moved = Math.hypot(dx, dz);
            // 067 Stage 3: the streaming app spawns the camera deterministically at the
            // server spawn, which here faces terrain — WALK advances until collision caps
            // it at the block boundary (~0.2). Moving and then hitting horizontal
            // collision still proves WALK-mode movement + collision both work.
            const walkedFreely = moved > 0.2;
            const walkedIntoTerrain = moved > 0.1 && state?.horizontalCollision === true;
            return state?.ok === true
              && state.movementMode === "WALK"
              && state.lastReport?.movementMode === "WALK"
              && state.lastReport?.onGround === true
              && (walkedFreely || walkedIntoTerrain)
              && (state.lastReport?.commandCount ?? 0) > start.commandCount;
          },
          walkingStart,
          { timeout: 60_000 },
        );
      } catch (error) {
        const state = await page.evaluate(() => globalThis.__mcloneWebApp?.state ?? null);
        throw new Error(`native web app did not advance walking movement after physical KeyW with Dvorak key value: ${error instanceof Error ? error.message : String(error)}\nstate=${JSON.stringify(state, null, 2)}`);
      } finally {
        await dispatchKeyboardEvent(page, "keyup", {
          code: "KeyW",
          key: ",",
        });
      }
      const walkingProbe = await page.evaluate((start) => {
        const state = globalThis.__mcloneWebApp.state;
        const dx = Number(state.cameraX) - start.cameraX;
        const dz = Number(state.cameraZ) - start.cameraZ;
        const moved = Math.hypot(dx, dz);
        const walkedFreely = moved > 0.2;
        const walkedIntoTerrain = moved > 0.1 && state.horizontalCollision === true;
        return {
          ok: state.movementMode === "WALK"
            && state.lastReport?.movementMode === "WALK"
            && state.lastReport?.onGround === true
            && (walkedFreely || walkedIntoTerrain),
          start,
          end: {
            cameraX: state.cameraX,
            cameraZ: state.cameraZ,
            movementMode: state.movementMode,
            onGround: state.onGround,
            horizontalCollision: state.horizontalCollision,
            commandCount: state.lastReport?.commandCount ?? 0,
          },
          distance: moved,
          walkedIntoTerrain,
        };
      }, walkingStart);
      const targetPreviewProbe = await captureTargetPreviewProbe(page);
      const blockInteractionProbe = await exerciseBlockInteraction(page, canvas);
      await page.keyboard.press("n");
      await page.waitForFunction(
        () => {
          const state = globalThis.__mcloneWebApp?.state;
          return state?.ok === true && state.movementMode === "NOCLIP";
        },
        undefined,
        { timeout: 10_000 },
      );
      await page.mouse.down();
      await page.mouse.move(700, 330);
      await page.mouse.up();
      await page.keyboard.down("w");
      await page.waitForFunction(
        () => {
          const state = globalThis.__mcloneWebApp?.state;
          return state?.ok === true
            && (state.centerX !== 0 || state.centerZ !== 0)
            && state.lastReport?.movementMode === "NOCLIP"
            && state.renderCount >= 3
            && state.frameCount > 0;
        },
        undefined,
        { timeout: 60_000 },
      );
      await page.keyboard.up("w");
      await page.waitForFunction(
        () => {
          const state = globalThis.__mcloneWebApp?.state;
          return state?.ok === true
            && state.loadedCenterX === state.centerX
            && state.loadedCenterZ === state.centerZ
            && state.pendingCompileJobCount === 0;
        },
        undefined,
        { timeout: 60_000 },
      );
      const result = await page.evaluate(() => globalThis.__mcloneWebApp.state);
      let pageScreenshotCaptured = false;
      try {
        await page.screenshot({ path: screenshotPath, fullPage: false, timeout: 5_000 });
        pageScreenshotCaptured = true;
      } catch (error) {
        console.warn(`page screenshot skipped: ${error instanceof Error ? error.message : String(error)}`);
      }
      const canvasPng = await canvas.screenshot({ path: canvasScreenshotPath, timeout: 60_000 });
      const canvasPixels = analyzePng(canvasPng);
      const nativeUiProbe = await captureNativeUiProbe(page, canvas);

      assertAppLoopResult(
        result,
        pageErrors,
        canvasPixels,
        walkingProbe,
        targetPreviewProbe,
        blockInteractionProbe,
        remoteServer?.websocketUrl ?? null,
      );
      assertNativeUiProbe(nativeUiProbe);
      console.log(JSON.stringify({
        url: appUrl,
        screenshotPath,
        pageScreenshotCaptured,
        canvasScreenshotPath,
        nativeUiCanvasScreenshotPath,
        appLoop,
        remoteWebSocket,
        remoteWebSocketUrl: remoteServer?.websocketUrl ?? null,
        canvasPixels,
        nativeUiProbe,
        walkingProbe,
        targetPreviewProbe,
        blockInteractionProbe,
        result,
      }, null, 2));
      return;
    }

    const smokeUrl = remoteServer
      ? `${baseUrl}/?remoteWsUrl=${encodeURIComponent(remoteServer.websocketUrl)}`
      : baseUrl;
    await page.goto(smokeUrl, { waitUntil: "load" });
    await page.waitForFunction(
      () => typeof globalThis.__mcloneNativeReady !== "undefined",
      undefined,
      { timeout: 20_000 },
    );
    const result = await page.evaluate(() => globalThis.__mcloneNativeReady);
    let pageScreenshotCaptured = false;
    try {
      await page.screenshot({ path: screenshotPath, fullPage: false, timeout: 5_000 });
      pageScreenshotCaptured = true;
    } catch (error) {
      console.warn(`page screenshot skipped: ${error instanceof Error ? error.message : String(error)}`);
    }
    const canvas = page.locator("#mclone-canvas");
    const canvasPng = await canvas.screenshot({ path: canvasScreenshotPath, timeout: 60_000 });
    const canvasPixels = analyzePng(canvasPng);

    assertSmokeResult(result, pageErrors, canvasPixels);
    console.log(JSON.stringify({
      url: baseUrl,
      screenshotPath,
      pageScreenshotCaptured,
      canvasScreenshotPath,
      requireCanvas,
      requireChunk,
      requireThreading,
      remoteWebSocket,
      remoteWebSocketUrl: remoteServer?.websocketUrl ?? null,
      canvasPixels,
      result,
    }, null, 2));
  } finally {
    await browser?.close();
    await remoteServer?.stop();
    await new Promise((resolveClose) => server.close(resolveClose));
  }
}

/**
 * @param {string} baseUrl
 * @param {{ websocketUrl: string, stop: () => Promise<void> } | null} remoteServer
 */
async function serveUntilStopped(baseUrl, remoteServer) {
  const appUrl = `${baseUrl}/app.html`;
  const smokeUrl = remoteServer
    ? `${baseUrl}/?remoteWsUrl=${encodeURIComponent(remoteServer.websocketUrl)}`
    : baseUrl;

  console.log("mclone native web serving");
  console.log(`  app:   ${appUrl}`);
  console.log(`  smoke: ${smokeUrl}`);
  if (remoteServer) {
    console.log(`  remote websocket: ${remoteServer.websocketUrl}`);
  }
  console.log("Press Ctrl-C to stop.");

  await waitForStopSignal();
}

function waitForStopSignal() {
  return new Promise((resolveSignal) => {
    /** @type {NodeJS.Signals[]} */
    const signals = ["SIGINT", "SIGTERM"];
    /** @param {NodeJS.Signals} signal */
    const stop = (signal) => {
      for (const registeredSignal of signals) {
        process.off(registeredSignal, stop);
      }
      resolveSignal(signal);
    };
    for (const signal of signals) {
      process.once(signal, stop);
    }
  });
}

/**
 * @param {Page} page
 * @param {Locator} canvas
 * @returns {Promise<any>}
 */
async function runMovementPerfProbe(page, canvas) {
  await canvas.evaluate((element) => element.focus());
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      return state?.ok === true
        && state.ready === true
        && state.streamingSettled === true
        && state.loadedCenterX === state.centerX
        && state.loadedCenterZ === state.centerZ
        && state.lastCompileTiming?.status === "accepted";
    },
    undefined,
    { timeout: 60_000 },
  );
  const start = await page.evaluate(() => {
    const state = globalThis.__mcloneWebApp.state;
    const compileTimings = state.compileTimings ?? [];
    return {
      centerX: state.centerX,
      centerZ: state.centerZ,
      cameraX: state.cameraX,
      cameraY: state.cameraY,
      cameraZ: state.cameraZ,
      frameCount: state.frameCount,
      renderCount: state.renderCount,
      compileTimingCount: state.compileTimingCount,
      lastCompileSequence: state.lastCompileTiming?.sequence ?? 0,
      // 067 Stage 3: the streaming loop tags every compile "stream"; the warm-up to idle
      // produces the initial-load compiles, so the most recent settled timing before
      // movement stands in for the old one-shot "initial" compile.
      initialCompileTiming: state.lastCompileTiming
        ?? (compileTimings.length > 0 ? compileTimings[compileTimings.length - 1] : null),
      preMovementCompileTimings: compileTimings,
    };
  });

  await page.keyboard.press("n");
  await page.waitForFunction(
    () => globalThis.__mcloneWebApp?.state?.movementMode === "NOCLIP",
    undefined,
    { timeout: 10_000 },
  );
  await page.evaluate(() => {
    for (let i = 0; i < 4; i += 1) {
      globalThis.__mcloneWebApp.adjustCameraSpeed?.(4);
    }
    globalThis.__mcloneWebApp.setInputKey?.("forward", true);
  });
  try {
    await page.waitForFunction(
      ({ start, movementPerfChunkBoundaries }) => {
        const state = globalThis.__mcloneWebApp?.state;
        if (!state?.ok) return false;
        const movedChunks = Math.max(
          Math.abs(Number(state.centerX) - start.centerX),
          Math.abs(Number(state.centerZ) - start.centerZ),
        );
        return movedChunks >= movementPerfChunkBoundaries;
      },
      { start, movementPerfChunkBoundaries },
      { timeout: 90_000 },
    );
  } finally {
    await page.evaluate(() => {
      globalThis.__mcloneWebApp.setInputKey?.("forward", false);
    });
  }

  await waitForWebAppStreamingSettled(page, 120_000);

  const end = await page.evaluate((start) => {
    const state = globalThis.__mcloneWebApp.state;
    const compileTimings = state.compileTimings ?? [];
    const movementCompileTimings = compileTimings.filter((/** @type {any} */ timing) => (
      Number(timing.sequence) > start.lastCompileSequence
    ));
    return {
      ok: state.ok === true,
      centerX: state.centerX,
      centerZ: state.centerZ,
      loadedCenterX: state.loadedCenterX,
      loadedCenterZ: state.loadedCenterZ,
      cameraX: state.cameraX,
      cameraY: state.cameraY,
      cameraZ: state.cameraZ,
      movementMode: state.movementMode,
      frameCount: state.frameCount,
      renderCount: state.renderCount,
      compileTimingCount: state.compileTimingCount,
      lastFrameGapMs: state.lastFrameGapMs,
      maxFrameGapMs: state.maxFrameGapMs,
      lastCompileTiming: state.lastCompileTiming,
      compileTimings,
      movementCompileTimings,
    };
  }, start);
  const movedChunks = Math.max(
    Math.abs(Number(end.centerX) - start.centerX),
    Math.abs(Number(end.centerZ) - start.centerZ),
  );
  return {
    ok: end.ok
      && movedChunks >= movementPerfChunkBoundaries
      && end.loadedCenterX === end.centerX
      && end.loadedCenterZ === end.centerZ
      && end.movementCompileTimings.length > 0,
    start,
    end: {
      centerX: end.centerX,
      centerZ: end.centerZ,
      loadedCenterX: end.loadedCenterX,
      loadedCenterZ: end.loadedCenterZ,
      cameraX: end.cameraX,
      cameraY: end.cameraY,
      cameraZ: end.cameraZ,
      movementMode: end.movementMode,
      frameCount: end.frameCount,
      renderCount: end.renderCount,
      compileTimingCount: end.compileTimingCount,
      lastFrameGapMs: end.lastFrameGapMs,
      maxFrameGapMs: end.maxFrameGapMs,
      lastCompileTiming: end.lastCompileTiming,
    },
    movedChunks,
    frameCountDelta: end.frameCount - start.frameCount,
    renderCountDelta: end.renderCount - start.renderCount,
    initialCompileTiming: start.initialCompileTiming,
    preMovementCompileTimings: start.preMovementCompileTimings,
    movementCompileTimings: end.movementCompileTimings,
    compileTimings: end.compileTimings,
  };
}

/**
 * @param {Page} page
 * @param {Locator} canvas
 * @returns {Promise<any>}
 */
async function runBlockEditProbe(page, canvas) {
  await canvas.evaluate((element) => element.focus());
  await canvas.click({ position: { x: 640, y: 360 } });
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      const report = state?.lastReport;
      return state?.ok === true
        && state.ready === true
        && state.streamingSettled === true
        && state.currentTarget?.ok === true
        && state.currentTarget.hit === true
        && state.pendingCompileJobCount === 0
        && Number.isFinite(Number(report?.snapshotUpdateCount))
        && Number.isFinite(Number(report?.sectionBlockUpdateCount))
        && Number.isFinite(Number(report?.unloadUpdateCount));
    },
    undefined,
    { timeout: 60_000 },
  );

  const start = await captureBlockEditTelemetry(page);
  /** @type {any[]} */
  const interactions = [];
  /** @type {any[]} */
  const placements = [];
  for (let i = 0; i < blockEditProbeBreaks; i += 1) {
    await page.keyboard.press("2");
    await page.waitForFunction(
      () => globalThis.__mcloneWebApp?.state?.selectedHotbarSlot === 1,
      undefined,
      { timeout: 10_000 },
    );
    const placement = await clickBlockInteraction(page, canvas, "right", "place", {
      expectedSelectedHotbarSlot: 1,
      expectedResultBlockStateId: DIRT_BLOCK_STATE_ID,
      expectedCarriedItemSynced: true,
    });
    placements.push(placement);
    const interaction = await clickBlockInteraction(page, canvas, "left", "break");
    interactions.push(interaction);
    await page.waitForFunction(
      () => {
        const state = globalThis.__mcloneWebApp?.state;
        return state?.ok === true
          && state.pendingCompileJobCount === 0
          && state.lastCompileReport?.pendingCompileJobCount === 0;
      },
      undefined,
      { timeout: 60_000 },
    );
  }

  await waitForWebAppStreamingSettled(page);
  const end = await captureBlockEditTelemetry(page);
  const updateDeltas = {
    updateCount: end.updateCount - start.updateCount,
    snapshotUpdateCount: end.snapshotUpdateCount - start.snapshotUpdateCount,
    sectionBlockUpdateCount: end.sectionBlockUpdateCount - start.sectionBlockUpdateCount,
    unloadUpdateCount: end.unloadUpdateCount - start.unloadUpdateCount,
  };
  const interactionDeltas = interactions.map((interaction) => ({
    action: interaction?.interaction?.action ?? null,
    ok: interaction?.ok === true,
    updateCountDelta: Number(interaction?.interaction?.updateCountDelta) || 0,
    snapshotUpdateCountDelta: Number(interaction?.interaction?.snapshotUpdateCountDelta) || 0,
    sectionBlockUpdateCountDelta: Number(interaction?.interaction?.sectionBlockUpdateCountDelta) || 0,
    unloadUpdateCountDelta: Number(interaction?.interaction?.unloadUpdateCountDelta) || 0,
  }));
  return {
    ok: interactions.length === blockEditProbeBreaks
      && interactions.every((interaction) => interaction?.ok === true)
      && placements.every((placement) => placement?.ok === true)
      && updateDeltas.sectionBlockUpdateCount >= blockEditProbeBreaks
      && end.frameCount > start.frameCount
      && end.renderCount > start.renderCount,
    breaksRequested: blockEditProbeBreaks,
    breaksCompleted: interactions.length,
    observedSnapshotDuringBreaks: interactionDeltas.some(
      (delta) => delta.snapshotUpdateCountDelta > 0,
    ),
    observedSnapshotDuringProbeWindow: updateDeltas.snapshotUpdateCount > 0,
    start,
    end,
    updateDeltas,
    interactionDeltas,
    interactions,
    placements,
    frameCountDelta: end.frameCount - start.frameCount,
    renderCountDelta: end.renderCount - start.renderCount,
  };
}

/** @param {Page} page */
async function captureBlockEditTelemetry(page) {
  return page.evaluate(() => {
    const state = globalThis.__mcloneWebApp.state;
    const report = state.lastReport ?? {};
    return {
      frameCount: Number(state.frameCount) || 0,
      renderCount: Number(state.renderCount) || 0,
      compileTimingCount: Number(state.compileTimingCount) || 0,
      lastFrameGapMs: Number(state.lastFrameGapMs) || 0,
      maxFrameGapMs: Number(state.maxFrameGapMs) || 0,
      commandCount: Number(report.commandCount) || 0,
      updateCount: Number(report.updateCount) || 0,
      snapshotUpdateCount: Number(report.snapshotUpdateCount) || 0,
      sectionBlockUpdateCount: Number(report.sectionBlockUpdateCount) || 0,
      unloadUpdateCount: Number(report.unloadUpdateCount) || 0,
      pendingCompileJobCount: Number(state.pendingCompileJobCount) || 0,
      renderDirtyChunkCount: Number(state.renderDirtyChunkCount) || 0,
      renderDirtySectionCount: Number(state.renderDirtySectionCount) || 0,
      renderInflightSectionCount: Number(state.renderInflightSectionCount) || 0,
      lastCompileReport: state.lastCompileReport ?? null,
      currentTarget: state.currentTarget ?? null,
    };
  });
}

/**
 * @param {Page} page
 * @param {number} [timeout]
 */
async function waitForWebAppStreamingSettled(page, timeout = 60_000) {
  try {
    await page.waitForFunction(
      () => {
        const state = globalThis.__mcloneWebApp?.state;
        return state?.ok === true
          && state.streamingSettled === true
          && state.loadedCenterX === state.centerX
          && state.loadedCenterZ === state.centerZ;
      },
      undefined,
      { timeout },
    );
  } catch (error) {
    const state = await page.evaluate(() => {
      const state = globalThis.__mcloneWebApp?.state;
      if (!state) {
        return null;
      }
      return {
        ok: state.ok,
        status: state.status,
        uiActive: state.uiActive,
        nativeUiScreen: state.nativeUiScreen,
        centerX: state.centerX,
        centerZ: state.centerZ,
        frameCount: state.frameCount,
        renderCount: state.renderCount,
        loadedCenterX: state.loadedCenterX,
        loadedCenterZ: state.loadedCenterZ,
        streamingSettled: state.streamingSettled,
        pendingCompileJobCount: state.pendingCompileJobCount,
        compileInFlight: state.compileInFlight,
        renderPendingWork: state.renderPendingWork,
        renderDirtyChunkCount: state.renderDirtyChunkCount,
        renderDirtySectionCount: state.renderDirtySectionCount,
        renderInflightSectionCount: state.renderInflightSectionCount,
        compileRequestId: state.lastReport?.compileRequestId,
        loadedDirtyChunkCount: state.lastReport?.loadedDirtyChunkCount,
        loadedDirtySectionCount: state.lastReport?.loadedDirtySectionCount,
        readyCompileSectionCount: state.lastReport?.readyCompileSectionCount,
        deferredCompileSectionCount: state.lastReport?.deferredCompileSectionCount,
        budgetedLoadedChunkCount: state.lastReport?.budgetedLoadedChunkCount,
        budgetedDirtySectionChunkCount: state.lastReport?.budgetedDirtySectionChunkCount,
        submittedCompileSectionCount: state.lastReport?.submittedCompileSectionCount,
        acceptedCompileSectionCount: state.lastReport?.acceptedCompileSectionCount,
        staleCompileSectionCount: state.lastReport?.staleCompileSectionCount,
        streamingIdleReport: state.lastReport?.streamingIdle,
        doorbellPresent: Boolean(state.lastReport?.doorbell),
        tickFrameBusy: state.tickFrameBusy,
        tickPhase: state.tickPhase,
        sessionBusy: globalThis.__mcloneWebApp?.state?.sessionBusy,
        movementMode: state.movementMode,
        touchControlsMode: state.touchControlsMode,
        touchControlsVisible: state.touchControlsVisible,
        touchJoystickActive: state.touchJoystickActive,
        touchMovementLeftImpulse: state.touchMovementLeftImpulse,
        touchMovementForwardImpulse: state.touchMovementForwardImpulse,
        touchLookActive: state.touchLookActive,
        touchButtonActiveCount: state.touchButtonActiveCount,
        lastUiAction: state.lastUiAction,
      };
    });
    throw new Error(`web app streaming did not settle: ${error instanceof Error ? error.message : String(error)}\n${JSON.stringify(state, null, 2)}`);
  }
}

/**
 * @param {Page} page
 * @param {Locator} canvas
 */
async function captureNativeUiProbe(page, canvas) {
  const requestedStatus = await page.evaluate(() => (
    globalThis.__mcloneWebApp?.openNativeTitleUi?.() ?? null
  ));
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      const report = state?.lastReport;
      return state?.ok === true
        && report?.uiActive === true
        && report?.uiCoversWorld === true
        && Number(report?.guiCommandCount) > 500;
    },
    undefined,
    { timeout: 10_000 },
  );
  const state = await page.evaluate(() => {
    const runtimeState = globalThis.__mcloneWebApp.state;
    return {
      ok: runtimeState.ok,
      uiActive: runtimeState.uiActive,
      uiCoversWorld: runtimeState.uiCoversWorld,
      guiCommandCount: runtimeState.guiCommandCount,
      lastReport: {
        ok: runtimeState.lastReport?.ok,
        uiActive: runtimeState.lastReport?.uiActive,
        uiCoversWorld: runtimeState.lastReport?.uiCoversWorld,
        guiCommandCount: runtimeState.lastReport?.guiCommandCount,
        skyRendered: runtimeState.lastReport?.skyRendered,
      },
    };
  });
  const canvasPng = await canvas.screenshot({
    path: nativeUiCanvasScreenshotPath,
    timeout: 60_000,
  });
  const canvasPixels = analyzePng(canvasPng);

  await clickNativeMenuButton(canvas, "title", 2);
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      return state?.uiActive === true
        && state.nativeUiScreen === "options"
        && state.lastUiAction?.action === "openOptions";
    },
    undefined,
    { timeout: 10_000 },
  );
  const openedOptions = await readNativeUiState(page);

  await page.keyboard.press("Escape");
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      return state?.uiActive === true
        && state.nativeUiScreen === "title"
        && state.lastUiAction?.action === "backToTitle";
    },
    undefined,
    { timeout: 10_000 },
  );
  const backedToTitle = await readNativeUiState(page);

  await clickNativeMenuButton(canvas, "title", 0);
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      return state?.uiActive === true
        && state.nativeUiScreen === "newWorld"
        && state.lastUiAction?.action === "openNewWorld";
    },
    undefined,
    { timeout: 10_000 },
  );
  const openedNewWorld = await readNativeUiState(page);

  await clickNativeMenuButton(canvas, "newWorld", 1);
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      return state?.uiActive === false
        && state.nativeUiScreen === "none"
        && state.lastUiAction?.action === "createWorld"
        && state.sessionState === "active"
        && state.sessionKind === "localWorld"
        && state.clientHost === "worker-integrated"
        && state.streamingSettled === true;
    },
    undefined,
    { timeout: 30_000 },
  );
  const createdWorld = await readNativeUiState(page);
  return {
    ok: state.uiActive === true
      && state.uiCoversWorld === true
      && Number(state.guiCommandCount) > 500
      && canvasPixels.nonClearInteriorPixelCount > 128
      && canvasPixels.distinctInteriorColorCount > 2
      && openedOptions.nativeUiScreen === "options"
      && backedToTitle.nativeUiScreen === "title"
      && openedNewWorld.nativeUiScreen === "newWorld"
      && createdWorld.uiActive === false
      && createdWorld.sessionState === "active"
      && createdWorld.sessionKind === "localWorld",
    requestedStatus,
    state,
    openedOptions,
    backedToTitle,
    openedNewWorld,
    createdWorld,
    canvasScreenshotPath: nativeUiCanvasScreenshotPath,
    canvasPixels,
  };
}

/**
 * @param {Page} page
 * @param {Locator} canvas
 */
async function exerciseMobileTouchControls(page, canvas) {
  const initial = await page.evaluate(() => {
    const state = globalThis.__mcloneWebApp.state;
    return {
      debugOverlayVisible: state.debugOverlayVisible,
      touchControlsVisible: state.touchControlsVisible,
    };
  });

  const movementStart = await page.evaluate(() => {
    const state = globalThis.__mcloneWebApp.state;
    return {
      cameraX: state.cameraX,
      cameraZ: state.cameraZ,
      commandCount: state.lastReport?.commandCount ?? 0,
    };
  });
  await dispatchCanvasPointerEvent(page, "pointerdown", {
    pointerId: 31,
    xFraction: 0.24,
    yFraction: 0.72,
    buttons: 1,
  });
  await dispatchCanvasPointerEvent(page, "pointermove", {
    pointerId: 31,
    xFraction: 0.31,
    yFraction: 0.54,
    buttons: 1,
  });
  let activeMovementProbe = null;
  try {
    await page.waitForFunction(
      (start) => {
        const state = globalThis.__mcloneWebApp?.state;
        const impulse = globalThis.__mcloneWebApp?.touchControlState?.()?.movementImpulse;
        const dx = Number(state?.cameraX) - start.cameraX;
        const dz = Number(state?.cameraZ) - start.cameraZ;
        return state?.ok === true
          && state.touchJoystickActive === true
          && impulse?.active === true
          && impulse.left < -0.05
          && impulse.forward > 0.05
          && state.movementMode === "WALK"
          && Math.hypot(dx, dz) > 0.15
          && (state.lastReport?.commandCount ?? 0) > start.commandCount;
      },
      movementStart,
      { timeout: 60_000 },
    );
    activeMovementProbe = await page.evaluate((start) => {
      const state = globalThis.__mcloneWebApp.state;
      const impulse = globalThis.__mcloneWebApp.touchControlState?.()?.movementImpulse;
      const dx = Number(state.cameraX) - start.cameraX;
      const dz = Number(state.cameraZ) - start.cameraZ;
      return {
        ok: impulse?.active === true
          && impulse.left < -0.05
          && impulse.forward > 0.05
          && Math.abs(impulse.left) < 1
          && impulse.forward < 1
          && Math.hypot(dx, dz) > 0.15,
        impulse,
        distance: Math.hypot(dx, dz),
      };
    }, movementStart);
  } finally {
    await dispatchCanvasPointerEvent(page, "pointerup", {
      pointerId: 31,
      xFraction: 0.31,
      yFraction: 0.54,
      buttons: 0,
    });
  }
  await page.waitForFunction(
    () => globalThis.__mcloneWebApp?.state?.touchJoystickActive === false,
    undefined,
    { timeout: 10_000 },
  );
  const movementProbe = await page.evaluate(({ start, activeMovementProbe }) => {
    const state = globalThis.__mcloneWebApp.state;
    const dx = Number(state.cameraX) - start.cameraX;
    const dz = Number(state.cameraZ) - start.cameraZ;
    return {
      ok: Math.hypot(dx, dz) > 0.15
        && state.touchJoystickActive === false
        && state.touchMovementLeftImpulse === 0
        && state.touchMovementForwardImpulse === 0
        && globalThis.__mcloneWebApp.touchControlState?.()?.keys?.forward === false,
      distance: Math.hypot(dx, dz),
      active: activeMovementProbe,
      start,
      end: {
        cameraX: state.cameraX,
        cameraZ: state.cameraZ,
        commandCount: state.lastReport?.commandCount ?? 0,
      },
      touch: globalThis.__mcloneWebApp.touchControlState?.(),
    };
  }, { start: movementStart, activeMovementProbe });

  const lookStart = await page.evaluate(() => {
    const state = globalThis.__mcloneWebApp.state;
    return {
      yaw: state.cameraYawRadians,
      pitch: state.cameraPitchRadians,
    };
  });
  await dispatchCanvasPointerEvent(page, "pointerdown", {
    pointerId: 32,
    xFraction: 0.72,
    yFraction: 0.44,
    buttons: 1,
  });
  await dispatchCanvasPointerEvent(page, "pointermove", {
    pointerId: 32,
    xFraction: 0.92,
    yFraction: 0.50,
    buttons: 1,
  });
  try {
    await page.waitForFunction(
      (start) => {
        const state = globalThis.__mcloneWebApp?.state;
        return state?.ok === true
          && state.touchLookActive === true
          && (
            Math.abs(Number(state.cameraYawRadians) - start.yaw) > 0.05
            || Math.abs(Number(state.cameraPitchRadians) - start.pitch) > 0.03
          );
      },
      lookStart,
      { timeout: 10_000 },
    );
  } finally {
    await dispatchCanvasPointerEvent(page, "pointerup", {
      pointerId: 32,
      xFraction: 0.92,
      yFraction: 0.50,
      buttons: 0,
    });
  }
  await page.waitForFunction(
    () => globalThis.__mcloneWebApp?.state?.touchLookActive === false,
    undefined,
    { timeout: 10_000 },
  );
  const lookProbe = await page.evaluate((start) => {
    const state = globalThis.__mcloneWebApp.state;
    const yawDelta = Math.abs(Number(state.cameraYawRadians) - start.yaw);
    const pitchDelta = Math.abs(Number(state.cameraPitchRadians) - start.pitch);
    return {
      ok: (yawDelta > 0.05 || pitchDelta > 0.03) && state.touchLookActive === false,
      yawDelta,
      pitchDelta,
      start,
      end: {
        yaw: state.cameraYawRadians,
        pitch: state.cameraPitchRadians,
      },
    };
  }, lookStart);

  const buttonProbe = await exerciseTouchButton(page, "jump", 41);
  await canvas.evaluate((element) => element.focus());

  // The native-rendered hamburger is only a platform shortcut for the native pause UI.
  await dispatchTouchMenuPointerEvent(page, "pointerdown", { pointerId: 51, buttons: 1 });
  await dispatchTouchMenuPointerEvent(page, "pointerup", { pointerId: 51, buttons: 0 });
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      const menu = document.getElementById("main-menu");
      return (menu === null || menu.hidden === true)
        && state?.uiActive === true
        && state.nativeUiScreen === "pause";
    },
    undefined,
    { timeout: 10_000 },
  );
  const openedNativeMenu = await readNativeUiState(page);
  const nativeMenuCanvasPng = await canvas.screenshot({
    path: mobileNativeUiCanvasScreenshotPath,
    timeout: 60_000,
  });
  const nativeMenuCanvasPixels = analyzePng(nativeMenuCanvasPng);

  const nativeOptionsProbe = await exerciseMobileNativeOptionsSensitivity(page, canvas);

  await page.keyboard.press("Escape");
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      return state?.uiActive === true
        && state.nativeUiScreen === "pause"
        && state.lastUiAction?.action === "backToPause";
    },
    undefined,
    { timeout: 10_000 },
  );

  await page.keyboard.press("Escape");
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      return state?.uiActive === false
        && state.nativeUiScreen === "none";
    },
    undefined,
    { timeout: 10_000 },
  );
  const closedNativeMenu = await readNativeUiState(page);

  return {
    ok: initial.debugOverlayVisible === false
      && initial.touchControlsVisible === true
      && activeMovementProbe?.ok === true
      && movementProbe.ok
      && lookProbe.ok
      && buttonProbe.ok
      && openedNativeMenu.uiActive === true
      && openedNativeMenu.nativeUiScreen === "pause"
      && nativeOptionsProbe.ok
      && (openedNativeMenu.menuHidden === true || openedNativeMenu.menuHidden === null)
      && nativeMenuCanvasPixels.nonClearInteriorPixelCount > 128
      && nativeMenuCanvasPixels.distinctInteriorColorCount > 2
      && closedNativeMenu.uiActive === false,
    initial,
    movement: movementProbe,
    look: lookProbe,
    button: buttonProbe,
    menu: {
      opened: openedNativeMenu,
      closed: closedNativeMenu,
      nativeCanvasScreenshotPath: mobileNativeUiCanvasScreenshotPath,
      nativeCanvasPixels: nativeMenuCanvasPixels,
    },
    options: nativeOptionsProbe,
  };
}

/**
 * @param {Page} page
 * @param {Locator} canvas
 */
async function exerciseMobileNativeOptionsSensitivity(page, canvas) {
  await dispatchCanvasPointerEvent(page, "pointerdown", {
    pointerId: 61,
    xFraction: 0.5,
    yFraction: 0.514,
    buttons: 1,
  });
  await dispatchCanvasPointerEvent(page, "pointerup", {
    pointerId: 61,
    xFraction: 0.5,
    yFraction: 0.514,
    buttons: 0,
  });
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      return state?.uiActive === true
        && state.nativeUiScreen === "options"
        && state.nativeUiOptionsParent === "pause"
        && state.lastUiAction?.action === "openOptions"
        && state.touchLookSensitivityAvailable === true;
    },
    undefined,
    { timeout: 10_000 },
  );
  const openedOptions = await readNativeUiState(page);

  await dispatchCanvasPointerEvent(page, "pointerdown", {
    pointerId: 62,
    xFraction: 0.744,
    yFraction: 0.601,
    buttons: 1,
  });
  await dispatchCanvasPointerEvent(page, "pointerup", {
    pointerId: 62,
    xFraction: 0.744,
    yFraction: 0.601,
    buttons: 0,
  });
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      return state?.uiActive === true
        && state.nativeUiScreen === "options"
        && state.lastUiAction?.action === "setTouchLookSensitivity"
        && Number(state.lookSensitivity) > 4.9
        && Number(globalThis.localStorage?.getItem("mclone.web.lookSensitivity")) > 4.9;
    },
    undefined,
    { timeout: 10_000 },
  );
  const adjusted = await page.evaluate(() => {
    const state = globalThis.__mcloneWebApp.state;
    return {
      lookSensitivity: state.lookSensitivity,
      touchLookSensitivityAvailable: state.touchLookSensitivityAvailable,
      lastUiAction: state.lastUiAction,
      storedLookSensitivity: globalThis.localStorage?.getItem("mclone.web.lookSensitivity") ?? null,
    };
  });
  const optionsCanvasPng = await canvas.screenshot({
    path: mobileNativeOptionsCanvasScreenshotPath,
    timeout: 60_000,
  });
  const optionsCanvasPixels = analyzePng(optionsCanvasPng);

  return {
    ok: openedOptions.uiActive === true
      && openedOptions.nativeUiScreen === "options"
      && openedOptions.nativeUiOptionsParent === "pause"
      && adjusted.touchLookSensitivityAvailable === true
      && Number(adjusted.lookSensitivity) > 4.9
      && adjusted.lastUiAction?.action === "setTouchLookSensitivity"
      && Number(adjusted.lastUiAction?.touchLookSensitivity) > 4.9
      && Number(adjusted.storedLookSensitivity) > 4.9
      && optionsCanvasPixels.nonClearInteriorPixelCount > 128
      && optionsCanvasPixels.distinctInteriorColorCount > 2,
    opened: openedOptions,
    adjusted,
    nativeCanvasScreenshotPath: mobileNativeOptionsCanvasScreenshotPath,
    nativeCanvasPixels: optionsCanvasPixels,
  };
}

/**
 * @param {Page} page
 * @param {string} key
 * @param {number} pointerId
 */
async function exerciseTouchButton(page, key, pointerId) {
  await dispatchTouchButtonPointerEvent(page, key, "pointerdown", { pointerId, buttons: 1 });
  await page.waitForFunction(
    (key) => globalThis.__mcloneWebApp?.touchControlState?.()?.keys?.[key] === true
      && globalThis.__mcloneWebApp?.state?.touchButtonActiveCount > 0,
    key,
    { timeout: 10_000 },
  );
  const down = await page.evaluate((key) => ({
    keyDown: globalThis.__mcloneWebApp.touchControlState?.().keys[key],
    activeCount: globalThis.__mcloneWebApp.state.touchButtonActiveCount,
  }), key);
  await dispatchTouchButtonPointerEvent(page, key, "pointerup", { pointerId, buttons: 0 });
  await page.waitForFunction(
    (key) => globalThis.__mcloneWebApp?.touchControlState?.()?.keys?.[key] === false
      && globalThis.__mcloneWebApp?.state?.touchButtonActiveCount === 0,
    key,
    { timeout: 10_000 },
  );
  const up = await page.evaluate((key) => ({
    keyDown: globalThis.__mcloneWebApp.touchControlState?.().keys[key],
    activeCount: globalThis.__mcloneWebApp.state.touchButtonActiveCount,
  }), key);
  return {
    ok: down.keyDown === true
      && down.activeCount > 0
      && up.keyDown === false
      && up.activeCount === 0,
    down,
    up,
  };
}

/** @param {Page} page */
async function readNativeUiState(page) {
  return page.evaluate(() => {
    const state = globalThis.__mcloneWebApp.state;
    const menu = document.getElementById("main-menu");
    return {
      debugOverlayVisible: state.debugOverlayVisible,
      uiActive: state.uiActive,
      uiCoversWorld: state.uiCoversWorld,
      nativeUiScreen: state.nativeUiScreen,
      nativeUiOptionsParent: state.nativeUiOptionsParent,
      lastUiAction: state.lastUiAction,
      sessionState: state.sessionState,
      sessionKind: state.sessionKind,
      sessionSeed: state.sessionSeed,
      sessionSeedText: state.sessionSeedText,
      sessionRemoteEndpoint: state.sessionRemoteEndpoint,
      sessionStatusVisible: state.sessionStatusVisible,
      sessionStatusOk: state.sessionStatusOk,
      sessionStatusMessage: state.sessionStatusMessage,
      menuHidden: menu?.hidden ?? null,
    };
  });
}

/**
 * @param {Locator} canvas
 * @param {"title" | "newWorld"} menu
 * @param {number} buttonIndex
 */
async function clickNativeMenuButton(canvas, menu, buttonIndex) {
  const position = await canvas.evaluate(
    (element, { menu, buttonIndex }) => {
      if (!(element instanceof HTMLCanvasElement)) {
        throw new Error("native menu target is not a canvas");
      }
      const pixelWidth = Math.max(1, Number(element.width) || 1);
      const pixelHeight = Math.max(1, Number(element.height) || 1);
      let scale = 1;
      while (
        scale < 4
        && Math.floor(pixelWidth / (scale + 1)) >= 320
        && Math.floor(pixelHeight / (scale + 1)) >= 240
      ) {
        scale += 1;
      }
      const guiWidth = Math.ceil(pixelWidth / scale);
      const guiHeight = Math.ceil(pixelHeight / scale);
      const menuTop = menu === "title"
        ? guiHeight * 0.5 - 34.0
        : guiHeight * 0.5 - 4.0;
      const guiX = guiWidth * 0.5;
      const guiY = menuTop + buttonIndex * 24.0 + 10.0;
      const rect = element.getBoundingClientRect();
      return {
        x: (guiX * scale * rect.width) / pixelWidth,
        y: (guiY * scale * rect.height) / pixelHeight,
      };
    },
    { menu, buttonIndex },
  );
  await canvas.click({ position });
}

/**
 * @param {Page} page
 * @param {string} type
 * @param {{ code: string, key: string }} keyInfo
 */
async function dispatchKeyboardEvent(page, type, { code, key }) {
  await page.evaluate(
    ({ type, code, key }) => {
      window.dispatchEvent(new KeyboardEvent(type, {
        bubbles: true,
        cancelable: true,
        code,
        key,
      }));
    },
    { type, code, key },
  );
}

/**
 * @param {Locator} canvas
 * @param {number} xFraction
 * @param {number} yFraction
 */
async function clickCanvasFraction(canvas, xFraction, yFraction) {
  const box = await canvas.boundingBox();
  if (!box) {
    throw new Error("canvas bounding box was unavailable");
  }
  await canvas.click({
    position: {
      x: box.width * xFraction,
      y: box.height * yFraction,
    },
  });
}

/**
 * @param {Page} page
 * @param {string} type
 * @param {{ pointerId: number, xFraction: number, yFraction: number, buttons: number }} options
 */
async function dispatchCanvasPointerEvent(page, type, options) {
  await page.evaluate(
    ({ type, options }) => {
      const canvas = /** @type {HTMLElement} */ (document.getElementById("mclone-canvas"));
      const rect = canvas.getBoundingClientRect();
      const clientX = rect.left + rect.width * options.xFraction;
      const clientY = rect.top + rect.height * options.yFraction;
      canvas.dispatchEvent(new PointerEvent(type, {
        bubbles: true,
        cancelable: true,
        pointerId: options.pointerId,
        pointerType: "touch",
        isPrimary: true,
        clientX,
        clientY,
        button: 0,
        buttons: options.buttons,
      }));
    },
    { type, options },
  );
}

/**
 * @param {Page} page
 * @param {string} type
 * @param {{ pointerId: number, buttons: number }} options
 */
async function dispatchTouchMenuPointerEvent(page, type, options) {
  await page.evaluate(
    ({ type, options }) => {
      const canvas = /** @type {HTMLElement} */ (document.getElementById("mclone-canvas"));
      const rect = canvas.getBoundingClientRect();
      canvas.dispatchEvent(new PointerEvent(type, {
        bubbles: true,
        cancelable: true,
        pointerId: options.pointerId,
        pointerType: "touch",
        isPrimary: true,
        clientX: rect.left + 30,
        clientY: rect.top + 30,
        button: 0,
        buttons: options.buttons,
      }));
    },
    { type, options },
  );
}

/**
 * @param {Page} page
 * @param {string} key
 * @param {string} type
 * @param {{ pointerId: number, buttons: number }} options
 */
async function dispatchTouchButtonPointerEvent(page, key, type, options) {
  await page.evaluate(
    ({ key, type, options }) => {
      const canvas = /** @type {HTMLElement} */ (document.getElementById("mclone-canvas"));
      const rect = canvas.getBoundingClientRect();
      const size = 58;
      const gap = 12;
      const right = 18;
      const bottom = 24;
      const x1 = Math.max(0, rect.width - right - size);
      const x0 = Math.max(0, x1 - gap - size);
      const y1 = Math.max(0, rect.height - bottom - size);
      const y0 = Math.max(0, y1 - gap - size);
      let center = null;
      if (key === "jump") {
        center = { x: x1 + size * 0.5, y: y0 + size * 0.5 };
      } else if (key === "sprint") {
        center = { x: x0 + size * 0.5, y: y1 + size * 0.5 };
      } else if (key === "descend") {
        center = { x: x1 + size * 0.5, y: y1 + size * 0.5 };
      }
      if (!center) {
        throw new Error(`unknown native touch button ${key}`);
      }
      canvas.dispatchEvent(new PointerEvent(type, {
        bubbles: true,
        cancelable: true,
        pointerId: options.pointerId,
        pointerType: "touch",
        isPrimary: true,
        clientX: rect.left + center.x,
        clientY: rect.top + center.y,
        button: 0,
        buttons: options.buttons,
      }));
    },
    { key, type, options },
  );
}

/** @param {Page} page */
async function captureTargetPreviewProbe(page) {
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      return state?.ok === true
        && state.currentTarget?.ok === true
        && state.currentTarget.hit === true
        && state.currentTarget.hitBlockStateId >= 0
        && state.pendingCompileJobCount === 0;
    },
    undefined,
    { timeout: 60_000 },
  );
  return page.evaluate(() => {
    const state = globalThis.__mcloneWebApp.state;
    const first = globalThis.__mcloneWebApp.previewBlockTarget?.();
    const second = globalThis.__mcloneWebApp.previewBlockTarget?.();
    return {
      ok: first?.ok === true
        && second?.ok === true
        && first.hit === true
        && second.hit === true
        && first.commandCount === second.commandCount
        && first.updateCount === second.updateCount,
      stateTarget: state.currentTarget,
      first,
      second,
    };
  });
}

/**
 * @param {Page} page
 * @param {Locator} canvas
 */
async function exerciseBlockInteraction(page, canvas) {
  const breakProbe = await clickBlockInteraction(page, canvas, "left", "break");
  await page.keyboard.press("2");
  await page.waitForFunction(
    () => globalThis.__mcloneWebApp?.state?.selectedHotbarSlot === 1,
    undefined,
    { timeout: 10_000 },
  );
  const selectedSlotProbe = await page.evaluate(() => ({
    selectedHotbarSlot: globalThis.__mcloneWebApp.state.selectedHotbarSlot,
  }));
  const placeProbe = await clickBlockInteraction(page, canvas, "right", "place", {
    expectedSelectedHotbarSlot: 1,
    expectedResultBlockStateId: DIRT_BLOCK_STATE_ID,
    expectedCarriedItemSynced: true,
  });
  return {
    ok: breakProbe.ok && selectedSlotProbe.selectedHotbarSlot === 1 && placeProbe.ok,
    break: breakProbe,
    selectedSlot: selectedSlotProbe,
    place: placeProbe,
  };
}

/**
 * @param {Page} page
 * @param {Locator} canvas
 * @param {"left" | "right" | "middle"} button
 * @param {string} action
 * @param {{ expectedSelectedHotbarSlot?: number, expectedResultBlockStateId?: number, expectedCarriedItemSynced?: boolean }} [options]
 */
async function clickBlockInteraction(page, canvas, button, action, options = {}) {
  const start = await page.evaluate(() => {
    const state = globalThis.__mcloneWebApp.state;
    return {
      commandCount: state.lastReport?.commandCount ?? 0,
      interactionCount: state.interactionCount ?? 0,
      meshBuildCount: state.lastCompileReport?.meshBuildCount ?? state.lastReport?.meshBuildCount ?? 0,
    };
  });
  await canvas.click({ position: { x: 640, y: 360 }, button });
  const waitArgs = {
    start,
    action,
    expectedSelectedHotbarSlot: options.expectedSelectedHotbarSlot ?? null,
    expectedResultBlockStateId: options.expectedResultBlockStateId ?? null,
    expectedCarriedItemSynced: options.expectedCarriedItemSynced ?? null,
  };
  try {
    await page.waitForFunction(
      blockInteractionReadyPredicate,
      waitArgs,
      { timeout: 60_000 },
    );
  } catch (error) {
    const diagnostic = await page.evaluate(blockInteractionReadyDiagnostic, waitArgs);
    throw new Error(`timed out waiting for ${action} interaction: ${error instanceof Error ? error.message : String(error)}\n${JSON.stringify(diagnostic, null, 2)}`);
  }
  return page.evaluate(
    ({ start, action, expectedSelectedHotbarSlot, expectedResultBlockStateId, expectedCarriedItemSynced }) => {
      const state = globalThis.__mcloneWebApp.state;
      const interaction = state.lastInteraction;
      const compileReport = state.lastCompileReport;
      const recompiled = compileReport?.commandCount >= interaction?.commandCount
        && compileReport?.acceptedCompileSectionCount > 0
        && compileReport?.meshBuildCount > start.meshBuildCount;
      return {
        ok: state.ok === true
          && interaction?.action === action
          && interaction?.hit === true
          && interaction?.commandSent === true
          && interaction?.changed === true
          && (expectedSelectedHotbarSlot === null || interaction?.selectedHotbarSlot === expectedSelectedHotbarSlot)
          && (expectedResultBlockStateId === null || interaction?.resultBlockStateId === expectedResultBlockStateId)
          && (expectedCarriedItemSynced === null || interaction?.carriedItemSynced === expectedCarriedItemSynced)
          && recompiled,
        start,
        interaction,
        compileReport: {
          commandCount: compileReport?.commandCount ?? 0,
          acceptedCompileSectionCount: compileReport?.acceptedCompileSectionCount ?? 0,
          meshBuildCount: compileReport?.meshBuildCount ?? 0,
          pendingCompileJobCount: compileReport?.pendingCompileJobCount ?? -1,
        },
      };
    },
    waitArgs,
  );
}

/**
 * @typedef {object} BlockInteractionWaitArgs
 * @property {{ commandCount: number, interactionCount: number, meshBuildCount: number }} start
 * @property {string} action
 * @property {number | null} expectedSelectedHotbarSlot
 * @property {number | null} expectedResultBlockStateId
 * @property {boolean | null} expectedCarriedItemSynced
 */

/** @param {BlockInteractionWaitArgs} args */
function blockInteractionReadyPredicate(args) {
  const {
    start,
    action,
    expectedSelectedHotbarSlot,
    expectedResultBlockStateId,
    expectedCarriedItemSynced,
  } = args;
  const state = globalThis.__mcloneWebApp?.state;
  const interaction = state?.lastInteraction;
  const compileReport = state?.lastCompileReport;
  return state?.ok === true
    && state.pendingCompileJobCount === 0
    && (state.interactionCount ?? 0) > start.interactionCount
    && interaction?.ok === true
    && interaction.action === action
    && interaction.hit === true
    && interaction.commandSent === true
    && interaction.changed === true
    && (interaction.interactionUpdateCount ?? 0) > 0
    && (interaction.commandCount ?? 0) > start.commandCount
    && (expectedSelectedHotbarSlot === null || interaction.selectedHotbarSlot === expectedSelectedHotbarSlot)
    && (expectedResultBlockStateId === null || interaction.resultBlockStateId === expectedResultBlockStateId)
    && (expectedCarriedItemSynced === null || interaction.carriedItemSynced === expectedCarriedItemSynced)
    && compileReport?.commandCount >= interaction.commandCount
    && compileReport?.acceptedCompileSectionCount > 0
    && compileReport?.meshBuildCount > start.meshBuildCount;
}

/** @param {BlockInteractionWaitArgs} args */
function blockInteractionReadyDiagnostic({
  start,
  action,
  expectedSelectedHotbarSlot,
  expectedResultBlockStateId,
  expectedCarriedItemSynced,
}) {
  const state = globalThis.__mcloneWebApp?.state;
  const interaction = state?.lastInteraction;
  const compileReport = state?.lastCompileReport;
  const checks = {
    stateOk: state?.ok === true,
    pendingCompileJobCountZero: state?.pendingCompileJobCount === 0,
    interactionAdvanced: (state?.interactionCount ?? 0) > start.interactionCount,
    interactionOk: interaction?.ok === true,
    actionMatches: interaction?.action === action,
    hit: interaction?.hit === true,
    commandSent: interaction?.commandSent === true,
    changed: interaction?.changed === true,
    interactionUpdateCountPositive: (interaction?.interactionUpdateCount ?? 0) > 0,
    commandCountAdvanced: (interaction?.commandCount ?? 0) > start.commandCount,
    selectedSlotMatches: expectedSelectedHotbarSlot === null
      || interaction?.selectedHotbarSlot === expectedSelectedHotbarSlot,
    resultBlockMatches: expectedResultBlockStateId === null
      || interaction?.resultBlockStateId === expectedResultBlockStateId,
    carriedItemSyncedMatches: expectedCarriedItemSynced === null
      || interaction?.carriedItemSynced === expectedCarriedItemSynced,
    compileCommandCoversInteraction: compileReport?.commandCount >= interaction?.commandCount,
    compileAcceptedSections: (compileReport?.acceptedCompileSectionCount ?? 0) > 0,
    compileMeshBuildAdvanced: (compileReport?.meshBuildCount ?? 0) > start.meshBuildCount,
  };
  return {
    ok: Object.values(checks).every(Boolean),
    checks,
    start,
    state: state ? {
      ok: state.ok,
      status: state.status,
      interactionCount: state.interactionCount,
      interactionStatus: state.interactionStatus,
      pendingCompileJobCount: state.pendingCompileJobCount,
      compileInFlight: state.compileInFlight,
      compileQueued: state.compileQueued,
      commandCount: state.lastReport?.commandCount,
      updateCount: state.lastReport?.updateCount,
      meshBuildCount: state.lastReport?.meshBuildCount,
    } : null,
    interaction,
    compileReport,
  };
}

function buildWasm() {
  const result = spawnSync(
    "cargo",
    ["build", "-p", "mclone-web-client", "--target", "wasm32-unknown-unknown"],
    {
      cwd: nativeRoot,
      stdio: "inherit",
      env: process.env,
    },
  );
  if (result.status !== 0) {
    throw new Error(`cargo wasm build failed with status ${result.status ?? "unknown"}`);
  }
}

function buildBindgenBundle() {
  ensureWasmBindgenCli();
  mkdirSync(bindgenOutDir, { recursive: true });
  const result = spawnSync(
    bindgenBin,
    [
      "--target",
      "web",
      // --typescript emits mclone_web_client.d.ts next to the JS glue (070 Stage 2). It is the
      // single source the web-glue type-check gate (native:web:typecheck) checks the wasm-return
      // boundary against. Browser glue itself is served from the 071 staged web root.
      "--typescript",
      "--out-dir",
      bindgenOutDir,
      "--out-name",
      "mclone_web_client",
      wasmPath,
    ],
    {
      cwd: nativeRoot,
      stdio: "inherit",
      env: process.env,
    },
  );
  if (result.status !== 0) {
    throw new Error(`wasm-bindgen failed with status ${result.status ?? "unknown"}`);
  }
}

function ensureWasmBindgenCli() {
  if (existsSync(bindgenBin)) return;

  mkdirSync(bindgenToolRoot, { recursive: true });
  const result = spawnSync(
    "cargo",
    [
      "install",
      "wasm-bindgen-cli",
      "--version",
      wasmBindgenVersion,
      "--locked",
      "--root",
      bindgenToolRoot,
    ],
    {
      cwd: nativeRoot,
      stdio: "inherit",
      env: process.env,
    },
  );
  if (result.status !== 0) {
    throw new Error(`cargo install wasm-bindgen-cli failed with status ${result.status ?? "unknown"}`);
  }
}

/** @param {string} webRoot */
async function startServer(webRoot) {
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url ?? "/", "http://127.0.0.1");
      const path = resolveRequestPath(url.pathname, webRoot);
      const bytes = await readFile(path);
      response.writeHead(200, {
        "Content-Type": contentType(path),
        "Cache-Control": "no-store",
        ...crossOriginIsolationHeaders(),
      });
      response.end(bytes);
    } catch (error) {
      const code = error && typeof error === "object" && "code" in error ? error.code : undefined;
      response.writeHead(code === "ENOENT" ? 404 : 500, {
        "Content-Type": "text/plain; charset=utf-8",
        ...crossOriginIsolationHeaders(),
      });
      response.end(error instanceof Error ? error.message : String(error));
    }
  });

  const requestedPort = Number.parseInt(process.env.MCLONE_NATIVE_WEB_SMOKE_PORT ?? "0", 10);
  await new Promise((resolveListen) => {
    server.listen(Number.isFinite(requestedPort) ? requestedPort : 0, "127.0.0.1", () => resolveListen(undefined));
  });
  return server;
}

function startNativeWebSocketServer() {
  return new Promise((resolveStart, rejectStart) => {
    const args = [
      "run",
      "--manifest-path",
      "native/Cargo.toml",
      "-p",
      "mclone-dedicated-server",
      "--",
      "--listen",
      "127.0.0.1:0",
      "--listen-ws",
      "127.0.0.1:0",
      "--serve-once",
      "--seed",
      "12345",
    ];
    const child = spawn("cargo", args, {
      cwd: repoRoot,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let settled = false;
    let stdout = "";
    let stderr = "";
    const timeout = setTimeout(() => {
      if (settled) return;
      settled = true;
      child.kill("SIGTERM");
      rejectStart(new Error(`timed out waiting for native websocket server\nstdout=${stdout}\nstderr=${stderr}`));
    }, 60_000);

    const tryResolveFromOutput = () => {
      if (settled) return;
      const match = stdout.match(/mclone dedicated websocket listening on (ws:\/\/\S+)/);
      if (!match) return;
      settled = true;
      clearTimeout(timeout);
      resolveStart({
        websocketUrl: match[1],
        async stop() {
          if (child.exitCode !== null || child.signalCode !== null) return;
          child.kill("SIGTERM");
          await new Promise((resolveExit) => child.once("exit", resolveExit));
        },
      });
    };

    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
      tryResolveFromOutput();
    });
    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", (error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      rejectStart(error);
    });
    child.on("exit", (code, signal) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      rejectStart(new Error(
        `native websocket server exited before announcing listener: code=${code} signal=${signal}\nstdout=${stdout}\nstderr=${stderr}`,
      ));
    });
  });
}

/**
 * @param {string} pathname
 * @param {string} [webRoot]
 */
function resolveRequestPath(pathname, webRoot = stagedWebRoot) {
  if (pathname === "/" || pathname === "/index.html") {
    return join(webRoot, "index.html");
  }
  if (pathname === "/mclone-web-smoke.js") {
    return join(webRoot, "mclone-web-smoke.js");
  }
  if (pathname === "/mclone-render-compiler-worker.js") {
    return join(webRoot, "mclone-render-compiler-worker.js");
  }
  if (pathname === "/mclone-integrated-server-worker.js") {
    return join(webRoot, "mclone-integrated-server-worker.js");
  }
  if (pathname === "/mclone-thread-smoke-worker.js") {
    return join(webRoot, "mclone-thread-smoke-worker.js");
  }
  if (pathname === "/mclone_web_client.wasm") {
    return wasmPath;
  }
  if (pathname === "/pkg/mclone_web_client.js") {
    return join(bindgenOutDir, "mclone_web_client.js");
  }
  if (pathname === "/pkg/mclone_web_client_bg.wasm") {
    return join(bindgenOutDir, "mclone_web_client_bg.wasm");
  }
  if (pathname === "/reference/minecraft-1.17.1/extracted.zip") {
    return referenceAssetPackPath;
  }

  const resolved = resolve(webRoot, `.${normalize(pathname)}`);
  if (resolved !== webRoot && !resolved.startsWith(`${webRoot}${sep}`)) {
    throw new Error(`refusing to serve path outside smoke root: ${pathname}`);
  }
  return resolved;
}

/** @param {string} path */
function contentType(path) {
  if (path.endsWith(".html")) return "text/html; charset=utf-8";
  if (path.endsWith(".js")) return "text/javascript; charset=utf-8";
  if (path.endsWith(".wasm")) return "application/wasm";
  if (path.endsWith(".zip")) return "application/zip";
  return "application/octet-stream";
}

function crossOriginIsolationHeaders() {
  return {
    "Cross-Origin-Opener-Policy": "same-origin",
    "Cross-Origin-Embedder-Policy": "require-corp",
    "Cross-Origin-Resource-Policy": "same-origin",
  };
}

/**
 * @param {any} result
 * @param {string[]} pageErrors
 * @param {any} canvasPixels
 */
function assertSmokeResult(result, pageErrors, canvasPixels) {
  if (pageErrors.length > 0) {
    throw new Error(`browser page errors:\n${pageErrors.join("\n")}`);
  }
  if (!result?.ok) {
    throw new Error(`native web smoke failed:\n${JSON.stringify(result, null, 2)}`);
  }
  if (requireThreading) {
    assertThreadingResult(result.threading);
  }
  if (!result.wasm?.report?.ok) {
    throw new Error(`wasm runtime report failed:\n${JSON.stringify(result.wasm, null, 2)}`);
  }
  if (
    result.wasm.report.commandCount !== 2
    || result.wasm.report.updateCount !== 4
    || !result.wasm.report.protocolCodecRoundtrip
  ) {
    throw new Error(`unexpected runtime message counts:\n${JSON.stringify(result.wasm.report, null, 2)}`);
  }
  if (
    result.wasm.report.loadedChunkCount !== 1
    || !result.wasm.report.centerChunkLoaded
    || !result.wasm.report.movedChunkLoaded
    || !result.wasm.report.previousChunkUnloaded
  ) {
    throw new Error(`client replica did not load center chunk:\n${JSON.stringify(result.wasm.report, null, 2)}`);
  }
  if (!result.webGpu?.ok) {
    throw new Error(`WebGPU probe did not produce a stable result:\n${JSON.stringify(result.webGpu, null, 2)}`);
  }
  if (!result.webGpu.supported) {
    if (requireCanvas) {
      throw new Error(`WebGPU is required for the canvas render smoke:\n${JSON.stringify(result.webGpu, null, 2)}`);
    }
    return;
  }
  if (!result.canvas?.ok || !result.canvas.report?.rendered || !result.canvas.report?.configured) {
    throw new Error(`WebGPU canvas render failed:\n${JSON.stringify(result.canvas, null, 2)}`);
  }
  if (result.canvas.report.width !== 640 || result.canvas.report.height !== 360) {
    throw new Error(`unexpected canvas render size:\n${JSON.stringify(result.canvas.report, null, 2)}`);
  }
  if (requireChunk) {
    assertChunkRenderResult(
      result.canvas.report,
      canvasPixels,
      result.canvas.firstReport,
      result.canvas.firstCenter,
      result.canvas.secondCenter,
      result.canvas.renderCompilerPendingJobCount,
      result.canvas.sessionPendingCompileJobCount,
      result.canvas.shutdownReport,
    );
  } else if (canvasPixels.distinctColorCount < 1 || canvasPixels.clearColorPixelCount < 16) {
    throw new Error(`canvas screenshot did not contain the rendered clear color:\n${JSON.stringify(canvasPixels, null, 2)}`);
  }
}

/**
 * @param {any} result
 * @param {string[]} pageErrors
 * @param {any} canvasPixels
 * @param {any} walkingProbe
 * @param {any} targetPreviewProbe
 * @param {any} blockInteractionProbe
 */
function assertAppLoopResult(
  result,
  pageErrors,
  canvasPixels,
  walkingProbe,
  targetPreviewProbe,
  blockInteractionProbe,
  remoteWebSocketUrl = null,
) {
  if (pageErrors.length > 0) {
    throw new Error(`browser app page errors:\n${pageErrors.join("\n")}`);
  }
  if (!result?.ok || !result.ready) {
    throw new Error(`native web app loop failed:\n${JSON.stringify(result, null, 2)}`);
  }
  if (
    (result.centerX === 0 && result.centerZ === 0)
    || result.loadedCenterX !== result.centerX
    || result.loadedCenterZ !== result.centerZ
    || result.radiusChunks !== 1
    || result.renderCount < 3
    || result.loadedChunkCount !== 9
    || result.residentSectionCount <= 1
    || result.pendingCompileJobCount !== 0
  ) {
    throw new Error(`native web app loop did not move and stream the expected camera chunk view:\n${JSON.stringify(result, null, 2)}`);
  }
  if (result.frameCount <= 0) {
    throw new Error(`native web app requestAnimationFrame loop did not advance:\n${JSON.stringify(result, null, 2)}`);
  }
  if (!walkingProbe?.ok || walkingProbe.distance <= 0.1) {
    throw new Error(`native web app did not move through the walking/collision path before no-clip streaming:\n${JSON.stringify({ walkingProbe, result }, null, 2)}`);
  }
  if (!targetPreviewProbe?.ok) {
    throw new Error(`native web app did not maintain a non-mutating current block target preview:\n${JSON.stringify({ targetPreviewProbe, result }, null, 2)}`);
  }
  if (!blockInteractionProbe?.ok) {
    throw new Error(`native web app did not break/place through the shared interaction path and recompile dirty sections:\n${JSON.stringify({ blockInteractionProbe, result }, null, 2)}`);
  }
  if (result.movementMode !== "NOCLIP" || result.lastReport?.movementMode !== "NOCLIP") {
    throw new Error(`native web app did not keep no-clip as a toggleable streaming fallback:\n${JSON.stringify(result, null, 2)}`);
  }
  if (!result.pointerLockAttempted || (!result.pointerLocked && !result.pointerLockFallback)) {
    throw new Error(`native web app did not exercise pointer-lock or fallback state:\n${JSON.stringify(result, null, 2)}`);
  }
  if (!result.lastCompileReport?.workerCompileUsed) {
    throw new Error(`native web app did not stream through the browser worker compiler:\n${JSON.stringify(result, null, 2)}`);
  }
  if (
    result.compileTimingCount < 2
    || !result.lastCompileTiming
    || !result.compileTimings?.some((/** @type {any} */ timing) => timing.trigger === "stream")
  ) {
    throw new Error(`native web app did not report streaming compile timings:\n${JSON.stringify(result, null, 2)}`);
  }
  assertCompileTimingDiagnostics(result.lastCompileTiming, "app last compile timing");
  const expectedRunnerKind = remoteWebSocketUrl ? "remote-websocket" : "web-worker";
  const expectedClientHost = remoteWebSocketUrl ? "remote-dedicated" : "worker-integrated";
  if (result.runnerKind !== expectedRunnerKind || result.lastReport?.runnerKind !== expectedRunnerKind) {
    throw new Error(`native web app did not use the expected ${expectedRunnerKind} runner:\n${JSON.stringify(result, null, 2)}`);
  }
  if (result.clientHost !== expectedClientHost) {
    throw new Error(`native web app did not report the expected ${expectedClientHost} host mode:\n${JSON.stringify(result, null, 2)}`);
  }
  if (remoteWebSocketUrl && result.remoteWebSocketUrl !== remoteWebSocketUrl) {
    throw new Error(`native web app did not preserve the requested remote websocket URL:\n${JSON.stringify({ remoteWebSocketUrl, result }, null, 2)}`);
  }
  const expectedSessionKind = remoteWebSocketUrl ? "remote" : "localWorld";
  if (
    result.sessionState !== "active"
    || result.sessionKind !== expectedSessionKind
    || (remoteWebSocketUrl && result.sessionRemoteEndpoint !== remoteWebSocketUrl)
    || (!remoteWebSocketUrl && !Number.isFinite(Number(result.sessionSeed)))
    || (!remoteWebSocketUrl && !/^-?\d+$/.test(String(result.sessionSeedText ?? "")))
  ) {
    throw new Error(`native web app did not publish the expected shared session state:\n${JSON.stringify({ remoteWebSocketUrl, result }, null, 2)}`);
  }
  if (
    result.runnerCommandQueueDepth !== 0
    || result.runnerUpdateQueueDepth !== 0
    || result.runnerPendingJobs !== 0
    || result.runnerPendingPublications !== 0
    || result.lastReport?.runnerCommandQueueDepth !== 0
    || result.lastReport?.runnerUpdateQueueDepth !== 0
    || result.lastReport?.runnerPendingJobs !== 0
    || result.lastReport?.runnerPendingPublications !== 0
  ) {
    throw new Error(`native web app integrated server worker did not settle queues/jobs:\n${JSON.stringify(result, null, 2)}`);
  }
  if (!Number.isFinite(result.width) || !Number.isFinite(result.height) || result.width < 960 || result.height < 540) {
    throw new Error(`native web app did not resize the WebGPU canvas from explicit display dimensions:\n${JSON.stringify(result, null, 2)}`);
  }
  if (!result.lastReport?.commandCount || result.lastReport.commandCount <= 2) {
    throw new Error(`native web app did not sync browser camera pose through gameplay commands:\n${JSON.stringify(result, null, 2)}`);
  }
  if (
    !result.lastReport?.skyRendered
    || !Number.isFinite(result.timeOfDay)
    || !Number.isFinite(result.lastReport.sunAngle)
    || !Number.isFinite(result.lastReport.dayTime)
    || result.lastReport.dayTime < 0
  ) {
    throw new Error(`native web app did not render from server time-of-day sky state:\n${JSON.stringify(result, null, 2)}`);
  }
  if (
    result.actorCount <= 0
    || result.drawnActorCount <= 0
    || result.lastReport.drawnActorIndexCount <= 0
    || result.lastReport.actorAtlasWidth <= 1
    || result.lastReport.actorAtlasHeight <= 1
  ) {
    throw new Error(`native web app did not render shared actor presentations:\n${JSON.stringify(result, null, 2)}`);
  }
  if (!Number.isFinite(result.cameraX) || !Number.isFinite(result.cameraY) || !Number.isFinite(result.cameraZ)) {
    throw new Error(`native web app did not report a finite camera pose:\n${JSON.stringify(result, null, 2)}`);
  }
  if (canvasPixels.nonClearInteriorPixelCount < 128 || canvasPixels.distinctInteriorColorCount < 2) {
    throw new Error(`app canvas screenshot did not contain generated chunk pixels:\n${JSON.stringify(canvasPixels, null, 2)}`);
  }
  if (canvasPixels.skyLikePixelCount < 64) {
    throw new Error(`app canvas screenshot did not contain visible sky pixels:\n${JSON.stringify(canvasPixels, null, 2)}`);
  }
}

/** @param {any} nativeUiProbe */
function assertNativeUiProbe(nativeUiProbe) {
  if (!nativeUiProbe?.ok) {
    throw new Error(`native web app did not render the shared Rust UI overlay:\n${JSON.stringify(nativeUiProbe, null, 2)}`);
  }
}

/**
 * @param {any} result
 * @param {string[]} pageErrors
 * @param {any} canvasPixels
 * @param {any} mobileTouchProbe
 */
function assertMobileAppLoopResult(result, pageErrors, canvasPixels, mobileTouchProbe) {
  if (pageErrors.length > 0) {
    throw new Error(`browser mobile app page errors:\n${pageErrors.join("\n")}`);
  }
  if (!result?.ok || !result.ready) {
    throw new Error(`native web mobile app loop failed:\n${JSON.stringify(result, null, 2)}`);
  }
  if (
    result.radiusChunks !== 1
    || result.renderCount < 2
    || result.loadedChunkCount !== 9
    || result.residentSectionCount <= 1
    || result.pendingCompileJobCount !== 0
  ) {
    throw new Error(`native web mobile app did not maintain the expected rendered world:\n${JSON.stringify(result, null, 2)}`);
  }
  if (!mobileTouchProbe?.ok) {
    throw new Error(`native web mobile controls did not satisfy movement/look/native UI probes:\n${JSON.stringify({ mobileTouchProbe, result }, null, 2)}`);
  }
  if (result.compileTimingCount < 1 || !result.lastCompileTiming) {
    throw new Error(`native web mobile app did not expose compile timing diagnostics:\n${JSON.stringify(result, null, 2)}`);
  }
  assertCompileTimingDiagnostics(result.lastCompileTiming, "mobile last compile timing");
  if (result.touchControlsVisible !== true || result.debugOverlayVisible !== false) {
    throw new Error(`native web mobile native UI/touch state ended in an unexpected state:\n${JSON.stringify(result, null, 2)}`);
  }
  if (!Number.isFinite(result.cameraX) || !Number.isFinite(result.cameraY) || !Number.isFinite(result.cameraZ)) {
    throw new Error(`native web mobile app did not report a finite camera pose:\n${JSON.stringify(result, null, 2)}`);
  }
  if (canvasPixels.nonClearInteriorPixelCount < 128 || canvasPixels.distinctInteriorColorCount < 2) {
    throw new Error(`mobile app canvas screenshot did not contain generated chunk pixels:\n${JSON.stringify(canvasPixels, null, 2)}`);
  }
}

/**
 * @param {any} report
 * @param {string[]} pageErrors
 * @param {any} canvasPixels
 */
function assertBlockEditProbeResult(report, pageErrors, canvasPixels) {
  if (pageErrors.length > 0) {
    throw new Error(`browser block edit probe page errors:\n${pageErrors.join("\n")}`);
  }
  const probe = report?.blockEditProbeResult;
  if (!probe?.ok) {
    throw new Error(`native web block edit probe failed:\n${JSON.stringify(report, null, 2)}`);
  }
  if (
    probe.breaksCompleted !== report.blockEditProbeBreaks
    || !Array.isArray(probe.interactionDeltas)
    || probe.interactionDeltas.length !== report.blockEditProbeBreaks
    || !probe.interactionDeltas.every((/** @type {any} */ delta) => delta.ok === true)
    || !Array.isArray(probe.placements)
    || probe.placements.length !== report.blockEditProbeBreaks
    || !probe.placements.every((/** @type {any} */ placement) => placement.ok === true)
  ) {
    throw new Error(`native web block edit probe did not complete every scripted break:\n${JSON.stringify(probe, null, 2)}`);
  }
  if (
    !Number.isFinite(Number(probe.start?.snapshotUpdateCount))
    || !Number.isFinite(Number(probe.start?.sectionBlockUpdateCount))
    || !Number.isFinite(Number(probe.end?.snapshotUpdateCount))
    || !Number.isFinite(Number(probe.end?.sectionBlockUpdateCount))
    || !Number.isFinite(Number(probe.updateDeltas?.snapshotUpdateCount))
    || !Number.isFinite(Number(probe.updateDeltas?.sectionBlockUpdateCount))
  ) {
    throw new Error(`native web block edit probe did not expose update-kind counters:\n${JSON.stringify(probe, null, 2)}`);
  }
  if (Number(probe.updateDeltas.sectionBlockUpdateCount) < report.blockEditProbeBreaks) {
    throw new Error(`native web block edit probe did not observe section block update deltas for each break:\n${JSON.stringify(probe, null, 2)}`);
  }
  if (Number(probe.frameCountDelta) <= 0 || Number(probe.renderCountDelta) <= 0) {
    throw new Error(`native web block edit probe did not keep frames/renders advancing:\n${JSON.stringify(probe, null, 2)}`);
  }
  if (canvasPixels.nonClearInteriorPixelCount < 128 || canvasPixels.distinctInteriorColorCount < 2) {
    throw new Error(`block edit probe canvas screenshot did not contain generated chunk pixels:\n${JSON.stringify(canvasPixels, null, 2)}`);
  }
}

/**
 * @param {any} report
 * @param {string[]} pageErrors
 * @param {any} canvasPixels
 */
function assertMovementPerfResult(report, pageErrors, canvasPixels) {
  if (pageErrors.length > 0) {
    throw new Error(`browser movement perf page errors:\n${pageErrors.join("\n")}`);
  }
  const probe = report.movementPerfProbe;
  const result = report.result;
  if (!probe?.ok || !result?.ok || !result.ready) {
    throw new Error(`native web movement perf did not complete:\n${JSON.stringify(report, null, 2)}`);
  }
  if (probe.movedChunks < report.movementPerfChunkBoundaries) {
    throw new Error(`native web movement perf did not cross enough chunk boundaries:\n${JSON.stringify(probe, null, 2)}`);
  }
  if (probe.frameCountDelta <= 0 || probe.renderCountDelta <= 0) {
    throw new Error(`native web movement perf did not keep frames/renders advancing:\n${JSON.stringify(probe, null, 2)}`);
  }
  if (!probe.initialCompileTiming) {
    throw new Error(`native web movement perf did not capture initial compile timing:\n${JSON.stringify(probe, null, 2)}`);
  }
  assertCompileTimingDiagnostics(probe.initialCompileTiming, "movement perf initial compile timing");
  if (!Array.isArray(probe.movementCompileTimings) || probe.movementCompileTimings.length < 1) {
    throw new Error(`native web movement perf did not capture movement compile timings:\n${JSON.stringify(probe, null, 2)}`);
  }
  for (const timing of probe.movementCompileTimings) {
    assertCompileTimingDiagnostics(timing, "movement perf compile timing");
  }
  if (!probe.movementCompileTimings.some((/** @type {any} */ timing) => (
    Number(timing.frameCountAfter) > Number(timing.frameCountBefore)
    && Number(timing.renderCountAfter) > Number(timing.renderCountBefore)
  ))) {
    throw new Error(`native web movement perf did not observe frame/render progress during a background compile:\n${JSON.stringify(probe.movementCompileTimings, null, 2)}`);
  }
  if (canvasPixels.nonClearInteriorPixelCount < 128 || canvasPixels.distinctInteriorColorCount < 2) {
    throw new Error(`movement perf canvas screenshot did not contain generated chunk pixels:\n${JSON.stringify(canvasPixels, null, 2)}`);
  }
}

/**
 * @param {any} timing
 * @param {string} label
 */
function assertCompileTimingDiagnostics(timing, label) {
  if (
    !timing
    || timing.status !== "accepted"
    || !Number.isFinite(Number(timing.totalMs))
    || Number(timing.totalMs) <= 0
    || !Number.isFinite(Number(timing.beginRequestMs))
    || !Number.isFinite(Number(timing.workerRoundTripMs))
    || Number(timing.workerRoundTripMs) <= 0
    || !Number.isFinite(Number(timing.decodeFinishApplyMs))
    || !Number.isFinite(Number(timing.maxFrameGapMs))
    || Number(timing.packedByteLength) <= 0
    || timing.renderCompilerTransportKind !== "shared-result-buffer"
    || timing.renderCompilerMetrics?.transportKind !== "shared-result-buffer"
    || Number(timing.renderCompilerWorkerInitCount) <= 0
    || Number(timing.renderCompilerWorkerWasmInitCount) <= 0
    || Number(timing.renderCompilerWorkerAssetLoadCount) <= 0
    || Number(timing.renderCompilerWorkerAssetPackInitByteLength) <= 0
    || Number(timing.renderCompilerWorkerAssetPackFileCount) <= 0
    || timing.renderCompilerPersistentAssetCatalog !== true
    || Number(timing.renderCompilerCompileCount) <= 0
    || Number(timing.renderCompilerWorkerCompileCount) <= 0
    || Number(timing.renderCompilerAssetPackSendCount) !== 1
    || Number(timing.renderCompilerRequestAssetPackByteLength) !== 0
    || Number(timing.renderCompilerRequestSnapshotInputByteLength) <= 0
    || Number(timing.renderCompilerRequestByteLength) <= 0
    || Number(timing.renderCompilerTransferredRequestByteLength) !== 0
    || Number(timing.renderCompilerTransferredRequestByteCount)
      < Number(timing.renderCompilerWorkerAssetPackInitByteLength)
    || Number(timing.renderCompilerTransferredResponseByteLength) !== 0
    || timing.renderCompilerSharedInputBufferUsed !== true
    || Number(timing.renderCompilerSharedInputByteLength)
      !== Number(timing.renderCompilerRequestSnapshotInputByteLength)
    || Number(timing.renderCompilerSharedInputBufferCapacityBytes)
      < Number(timing.renderCompilerSharedInputByteLength)
    // 067 Stage 4: the input is a delta, so the "chunk count" is the upserts shipped this
    // compile; a target re-dirtied by a neighbor (not its own revision) legitimately ships 0
    // upserts because the worker mirror already holds it. The delta byte length stays > 0
    // (asserted above via renderCompilerRequestSnapshotInputByteLength) and carries the weight.
    || Number(timing.renderCompilerSnapshotInputChunkCount) < 0
    // 067 follow-up 1: the web submit clones only the changed (delta) columns, never the whole
    // loaded world. The clone count is captured at the clone site, independently of the upserts
    // shipped, so a reintroduced whole-world clone (clone all, ship a delta) makes these diverge.
    || Number(timing.renderCompilerSnapshotInputClonedColumnCount)
      !== Number(timing.renderCompilerSnapshotInputChunkCount)
    || timing.renderCompilerSnapshotInputCompileUsed !== true
    || timing.renderCompilerGeneratedViewFallbackUsed !== false
    || timing.renderCompilerSharedResultBufferUsed !== true
    || Number(timing.renderCompilerSharedResultByteLength) !== Number(timing.packedByteLength)
    || Number(timing.renderCompilerSharedResultBufferCapacityBytes) < Number(timing.packedByteLength)
    || Number(timing.renderCompilerSharedResultResponseCount) <= 0
    || Number(timing.renderCompilerSharedResultByteCount) < Number(timing.packedByteLength)
    || Number(timing.submittedCompileSectionCount) <= 0
    || Number(timing.acceptedCompileSectionCount) < 0
    || Number(timing.staleCompileSectionCount) < 0
    || Number(timing.uploadedSectionCount) < 0
    || Number(timing.removedSectionCount) < 0
  ) {
    throw new Error(`${label} was missing required diagnostics:\n${JSON.stringify(timing, null, 2)}`);
  }
}

/** @param {any[]} timings */
function summarizeCompileTimings(timings) {
  const values = Array.isArray(timings) ? timings : [];
  const totals = values.map((timing) => Number(timing.totalMs) || 0);
  const worker = values.map((timing) => Number(timing.workerRoundTripMs) || 0);
  const apply = values.map((timing) => Number(timing.decodeFinishApplyMs) || 0);
  const frameGaps = values.map((timing) => Number(timing.maxFrameGapMs) || 0);
  return {
    count: values.length,
    totalMs: summarizeNumbers(totals),
    workerRoundTripMs: summarizeNumbers(worker),
    decodeFinishApplyMs: summarizeNumbers(apply),
    maxFrameGapMs: summarizeNumbers(frameGaps),
    packedByteLengthMax: Math.max(0, ...values.map((timing) => Number(timing.packedByteLength) || 0)),
    renderCompilerTransportKinds: [...new Set(values.map((timing) => (
      timing.renderCompilerTransportKind || "unknown"
    )))],
    renderCompilerRequestAssetPackByteLengthMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerRequestAssetPackByteLength) || 0
    ))),
    renderCompilerRequestSnapshotInputByteLengthMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerRequestSnapshotInputByteLength) || 0
    ))),
    renderCompilerWorkerAssetPackInitByteLengthMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerWorkerAssetPackInitByteLength) || 0
    ))),
    renderCompilerTransferredRequestByteLengthMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerTransferredRequestByteLength) || 0
    ))),
    renderCompilerTransferredResponseByteLengthMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerTransferredResponseByteLength) || 0
    ))),
    renderCompilerTransferredRequestByteCountMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerTransferredRequestByteCount) || 0
    ))),
    renderCompilerTransferredResponseByteCountMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerTransferredResponseByteCount) || 0
    ))),
    renderCompilerSharedInputByteLengthMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerSharedInputByteLength) || 0
    ))),
    renderCompilerSharedInputBufferCapacityBytesMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerSharedInputBufferCapacityBytes) || 0
    ))),
    renderCompilerSnapshotInputChunkCountMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerSnapshotInputChunkCount) || 0
    ))),
    renderCompilerSnapshotInputCompileUsed: values.every((timing) => (
      timing.renderCompilerSnapshotInputCompileUsed === true
    )),
    renderCompilerGeneratedViewFallbackUsed: values.some((timing) => (
      timing.renderCompilerGeneratedViewFallbackUsed === true
    )),
    renderCompilerSharedResultByteLengthMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerSharedResultByteLength) || 0
    ))),
    renderCompilerSharedResultBufferCapacityBytesMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerSharedResultBufferCapacityBytes) || 0
    ))),
    renderCompilerSharedResultByteCountMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerSharedResultByteCount) || 0
    ))),
    renderCompilerSharedResultOverflowCountMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerSharedResultOverflowCount) || 0
    ))),
    renderCompilerAssetPackSendCountMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerAssetPackSendCount) || 0
    ))),
    renderCompilerWorkerInitCountMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerWorkerInitCount) || 0
    ))),
    renderCompilerWorkerWasmInitCountMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerWorkerWasmInitCount) || 0
    ))),
    renderCompilerWorkerAssetLoadCountMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerWorkerAssetLoadCount) || 0
    ))),
    renderCompilerWorkerCompileCountMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerWorkerCompileCount) || 0
    ))),
    viewDirtyChunkCountMax: Math.max(0, ...values.map((timing) => Number(timing.viewDirtyChunkCount) || 0)),
    viewRemovalChunkCountMax: Math.max(0, ...values.map((timing) => Number(timing.viewRemovalChunkCount) || 0)),
    loadedDirtyChunkCountMax: Math.max(0, ...values.map((timing) => Number(timing.loadedDirtyChunkCount) || 0)),
    readyCompileSectionCountMax: Math.max(0, ...values.map((timing) => Number(timing.readyCompileSectionCount) || 0)),
    workerVisibilityGraphBuildCountMax: Math.max(0, ...values.map((timing) => Number(timing.workerVisibilityGraphBuildCount) || 0)),
    submittedCompileSectionCountMax: Math.max(0, ...values.map((timing) => Number(timing.submittedCompileSectionCount) || 0)),
    uploadedSectionCountMax: Math.max(0, ...values.map((timing) => Number(timing.uploadedSectionCount) || 0)),
    removedSectionCountMax: Math.max(0, ...values.map((timing) => Number(timing.removedSectionCount) || 0)),
  };
}

/** @param {number[]} values */
function summarizeNumbers(values) {
  if (values.length === 0) {
    return { min: 0, max: 0, avg: 0 };
  }
  const sum = values.reduce((total, value) => total + value, 0);
  return {
    min: Math.min(...values),
    max: Math.max(...values),
    avg: Math.round((sum / values.length) * 10) / 10,
  };
}

/** @param {any} threading */
function assertThreadingResult(threading) {
  if (!threading?.ok) {
    throw new Error(`browser threading smoke failed:\n${JSON.stringify(threading, null, 2)}`);
  }
  if (
    !threading.crossOriginIsolated
    || !threading.sharedArrayBuffer
    || !threading.wasmSharedMemory
    || !threading.atomics
    || !threading.workerRoundtrip
    || threading.initialValue !== 7
    || threading.finalValue !== 42
  ) {
    throw new Error(`browser threading prerequisites are incomplete:\n${JSON.stringify(threading, null, 2)}`);
  }
}

/**
 * @param {any} report
 * @param {any} canvasPixels
 * @param {any} firstReport
 * @param {any} firstCenter
 * @param {any} secondCenter
 * @param {number} renderCompilerPendingJobCount
 * @param {number} sessionPendingCompileJobCount
 * @param {any} shutdownReport
 */
function assertChunkRenderResult(
  report,
  canvasPixels,
  firstReport,
  firstCenter,
  secondCenter,
  renderCompilerPendingJobCount,
  sessionPendingCompileJobCount,
  shutdownReport,
) {
  // 067 Stage 3: the deterministic smoke now pumps `syncOverviewRenderFrame` to idle for
  // two overview centers instead of a two-render begin/finish transaction. Each center is
  // the web analog of desktop `sync_all_render_sections`: many small per-frame compiles fill
  // progressively until the loop idles and the server runner has drained.
  assertOverviewCenter(firstCenter, 0, 0);
  assertOverviewCenter(secondCenter, 1, 0);
  assertOverviewWorkerReport(firstCenter.workerReport, firstCenter, "first overview center");
  assertOverviewWorkerReport(secondCenter.workerReport, secondCenter, "second overview center");
  if (renderCompilerPendingJobCount !== 0) {
    throw new Error(`render compiler worker still had pending jobs after chunk smoke:\n${JSON.stringify({ renderCompilerPendingJobCount }, null, 2)}`);
  }
  if (sessionPendingCompileJobCount !== 0 || report.pendingCompileJobCount !== 0) {
    throw new Error(`web render session still had pending compile jobs after chunk smoke:\n${JSON.stringify({ sessionPendingCompileJobCount, report }, null, 2)}`);
  }
  if (!firstReport?.ok || firstReport.streamingIdle !== true) {
    throw new Error(`first overview center did not settle to a streaming-idle frame:\n${JSON.stringify(firstReport, null, 2)}`);
  }
  if (!report?.ok || report.streamingIdle !== true) {
    throw new Error(`second overview center did not settle to a streaming-idle frame:\n${JSON.stringify(report, null, 2)}`);
  }
  if (
    firstReport.centerX !== 0
    || firstReport.centerZ !== 0
    || firstReport.radiusChunks !== 1
    || report.centerX !== 1
    || report.centerZ !== 0
    || report.radiusChunks !== 1
  ) {
    throw new Error(`overview pump did not stream the expected centers:\n${JSON.stringify({ firstReport, report }, null, 2)}`);
  }
  if (!report.chunkLoaded || !report.meshBuilt) {
    throw new Error(`generated chunk did not load/build:\n${JSON.stringify(report, null, 2)}`);
  }
  if (!shutdownReport?.ok || shutdownReport.runnerKind !== "web-worker") {
    throw new Error(`generated chunk render did not shut down the integrated server worker cleanly:\n${JSON.stringify({ shutdownReport, report }, null, 2)}`);
  }
  if (report.runnerKind !== "web-worker" || firstReport.runnerKind !== "web-worker") {
    throw new Error(`generated chunk render did not use the integrated server Web Worker runner:\n${JSON.stringify({ firstReport, report }, null, 2)}`);
  }
  if (
    report.runnerCommandQueueDepth !== 0
    || report.runnerUpdateQueueDepth !== 0
    || report.runnerPendingJobs !== 0
    || report.runnerPendingPublications !== 0
  ) {
    throw new Error(`generated chunk render left integrated server worker queues/jobs pending:\n${JSON.stringify(report, null, 2)}`);
  }
  if (!report.assetPackLoaded || !report.textured) {
    throw new Error(`generated chunk was not rendered from the packed textured asset path:\n${JSON.stringify(report, null, 2)}`);
  }
  if (!report.skyRendered || !Number.isFinite(report.timeOfDay) || !Number.isFinite(report.sunAngle) || !Number.isFinite(report.dayTime) || report.dayTime < 0) {
    throw new Error(`generated chunk render did not use server time-of-day sky state:\n${JSON.stringify(report, null, 2)}`);
  }
  if (report.assetPackFileCount < 1000 || report.atlasWidth <= 0 || report.atlasHeight <= 0 || report.atlasSpriteCount <= 0) {
    throw new Error(`packed texture atlas did not load expected asset data:\n${JSON.stringify(report, null, 2)}`);
  }
  if (report.actorAtlasWidth <= 1 || report.actorAtlasHeight <= 1) {
    throw new Error(`packed actor texture atlas did not load expected asset data:\n${JSON.stringify(report, null, 2)}`);
  }
  if (
    firstReport.loadedChunkCount !== 9
    || report.loadedChunkCount !== 9
    || Number(firstReport.residentSectionCount) <= 1
    || Number(report.residentSectionCount) <= 1
  ) {
    throw new Error(`overview pump did not stream the expected loaded chunk view:\n${JSON.stringify({ firstReport, report }, null, 2)}`);
  }
  if (
    report.vertexCount <= 0
    || report.indexCount <= 0
    || report.faceCount <= 0
    || report.drawnIndexCount <= 0
    || report.drawnFaceCount <= 0
  ) {
    throw new Error(`generated chunk mesh was empty:\n${JSON.stringify(report, null, 2)}`);
  }
  // The second center reuses the resident worker mesh catalog + ring and the first center's
  // resident cache, so it must incrementally add sections (run >0 per-frame compiles) for the
  // shifted view rather than recompiling the whole world.
  if (Number(secondCenter.compileCount) <= 0) {
    throw new Error(`second overview center did not incrementally add streamed sections:\n${JSON.stringify(secondCenter, null, 2)}`);
  }
  if (canvasPixels.nonClearInteriorPixelCount < 128 || canvasPixels.distinctInteriorColorCount < 2) {
    throw new Error(`canvas screenshot did not contain generated chunk pixels:\n${JSON.stringify(canvasPixels, null, 2)}`);
  }
}

/**
 * @param {any} center
 * @param {number} expectedCenterX
 * @param {number} expectedCenterZ
 */
function assertOverviewCenter(center, expectedCenterX, expectedCenterZ) {
  if (
    !center
    || center.settled !== true
    || center.centerX !== expectedCenterX
    || center.centerZ !== expectedCenterZ
    || Number(center.residentSectionCount) <= 1
    || Number(center.compileCount) <= 0
    || !center.workerReport
  ) {
    throw new Error(`overview center ${expectedCenterX},${expectedCenterZ} did not pump to a settled streamed view:\n${JSON.stringify(center, null, 2)}`);
  }
}

/**
 * @param {any} workerReport
 * @param {any} center
 * @param {string} label
 */
function assertOverviewWorkerReport(workerReport, center, label) {
  if (!workerReport?.ok) {
    throw new Error(`${label} render compiler worker failed:\n${JSON.stringify({ center, workerReport }, null, 2)}`);
  }
  const summary = workerReport.summary;
  if (
    workerReport.transportKind !== "shared-result-buffer"
    || workerReport.renderCompilerMetrics?.transportKind !== "shared-result-buffer"
    || workerReport.snapshotInputCompileUsed !== true
    || workerReport.generatedViewFallbackUsed !== false
    || workerReport.renderCompilerMetrics?.snapshotInputCompileUsed !== true
    || workerReport.renderCompilerMetrics?.generatedViewFallbackUsed !== false
    || workerReport.sharedInputBufferUsed !== true
    || workerReport.sharedResultBufferUsed !== true
    || workerReport.targetedCompileUsed !== true
    || Number(workerReport.targetSectionCount) <= 0
    || Number(workerReport.workerInitCount) <= 0
    || Number(workerReport.workerWasmInitCount) <= 0
    || Number(workerReport.workerAssetLoadCount) <= 0
    || Number(workerReport.workerAssetPackInitByteLength) <= 0
    || Number(workerReport.workerAssetPackFileCount) <= 0
    || workerReport.persistentAssetCatalog !== true
    || Number(workerReport.assetPackSendCount) !== 1
    || Number(workerReport.requestAssetPackByteLength) !== 0
    || Number(workerReport.transferredRequestByteLength) !== 0
    || Number(workerReport.transferredResponseByteLength) !== 0
    || Number(workerReport.sharedInputByteLength) <= 0
    || Number(workerReport.sharedResultByteLength) <= 0
    || Number(workerReport.packedByteLength) <= 0
    || Number(workerReport.sharedResultByteLength) !== Number(workerReport.packedByteLength)
  ) {
    throw new Error(`${label} did not return the expected shared-result streaming payload:\n${JSON.stringify({ center, workerReport }, null, 2)}`);
  }
  if (
    !summary?.ok
    || Number(summary.byteLength) <= 0
    || Number(summary.sectionCount) !== Number(workerReport.targetSectionCount)
    || Number(summary.vertexCount) < 0
    || Number(summary.indexCount) < 0
    || Number(summary.faceCount) < 0
    || Number(summary.visibilityGraphBuildCount) !== Number(workerReport.targetSectionCount)
  ) {
    throw new Error(`${label} render compiler worker summary was malformed:\n${JSON.stringify({ center, workerReport }, null, 2)}`);
  }
}
/** @param {Buffer} bytes */
function analyzePng(bytes) {
  const png = decodePngRgba(bytes);
  const expected = {
    r: Math.round(0.035 * 255),
    g: Math.round(0.04 * 255),
    b: Math.round(0.05 * 255),
  };
  const colors = new Set();
  const interiorColors = new Set();
  let clearColorPixelCount = 0;
  let nonClearInteriorPixelCount = 0;
  let skyLikePixelCount = 0;
  const inset = 4;
  for (let y = 0; y < png.height; y += 1) {
    for (let x = 0; x < png.width; x += 1) {
      const offset = (y * png.width + x) * 4;
      const r = png.rgba[offset];
      const g = png.rgba[offset + 1];
      const b = png.rgba[offset + 2];
      colors.add(`${r},${g},${b}`);
      const isClear =
        Math.abs(r - expected.r) <= 3
        && Math.abs(g - expected.g) <= 3
        && Math.abs(b - expected.b) <= 3;
      if (isClear) {
        clearColorPixelCount += 1;
      }
      if (b >= 96 && b > r + 24 && b >= g + 12) {
        skyLikePixelCount += 1;
      }
      if (x >= inset && x < png.width - inset && y >= inset && y < png.height - inset) {
        interiorColors.add(`${r},${g},${b}`);
        if (!isClear) {
          nonClearInteriorPixelCount += 1;
        }
      }
    }
  }
  return {
    width: png.width,
    height: png.height,
    distinctColorCount: colors.size,
    distinctInteriorColorCount: interiorColors.size,
    clearColorPixelCount,
    nonClearInteriorPixelCount,
    skyLikePixelCount,
    expectedClearColor: expected,
  };
}

/** @param {Buffer} bytes */
function decodePngRgba(bytes) {
  if (
    bytes[0] !== 0x89
    || bytes[1] !== 0x50
    || bytes[2] !== 0x4e
    || bytes[3] !== 0x47
    || bytes[4] !== 0x0d
    || bytes[5] !== 0x0a
    || bytes[6] !== 0x1a
    || bytes[7] !== 0x0a
  ) {
    throw new Error("canvas screenshot is not a PNG");
  }

  let offset = 8;
  let width = 0;
  let height = 0;
  let colorType = 0;
  let bitDepth = 0;
  /** @type {Buffer[]} */
  const idat = [];
  while (offset < bytes.length) {
    const length = bytes.readUInt32BE(offset);
    offset += 4;
    const type = bytes.toString("ascii", offset, offset + 4);
    offset += 4;
    const data = bytes.subarray(offset, offset + length);
    offset += length + 4;

    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      bitDepth = data[8];
      colorType = data[9];
      if (bitDepth !== 8 || (colorType !== 2 && colorType !== 6)) {
        throw new Error(`unsupported PNG format bitDepth=${bitDepth} colorType=${colorType}`);
      }
    } else if (type === "IDAT") {
      idat.push(data);
    } else if (type === "IEND") {
      break;
    }
  }

  if (width <= 0 || height <= 0 || idat.length === 0) {
    throw new Error("invalid PNG screenshot data");
  }

  const compressed = Buffer.concat(idat);
  const raw = inflateSync(compressed);
  const bytesPerPixel = colorType === 6 ? 4 : 3;
  const stride = width * bytesPerPixel;
  const rgba = Buffer.alloc(width * height * 4);
  const unfiltered = Buffer.alloc(height * stride);
  let rawOffset = 0;
  for (let y = 0; y < height; y += 1) {
    const filter = raw[rawOffset];
    rawOffset += 1;
    for (let x = 0; x < stride; x += 1) {
      const current = raw[rawOffset + x];
      const left = x >= bytesPerPixel ? unfiltered[y * stride + x - bytesPerPixel] : 0;
      const up = y > 0 ? unfiltered[(y - 1) * stride + x] : 0;
      const upLeft = x >= bytesPerPixel && y > 0
        ? unfiltered[(y - 1) * stride + x - bytesPerPixel]
        : 0;
      unfiltered[y * stride + x] = unfilterByte(filter, current, left, up, upLeft);
    }
    rawOffset += stride;
  }

  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const sourceOffset = y * stride + x * bytesPerPixel;
      const targetOffset = (y * width + x) * 4;
      rgba[targetOffset] = unfiltered[sourceOffset];
      rgba[targetOffset + 1] = unfiltered[sourceOffset + 1];
      rgba[targetOffset + 2] = unfiltered[sourceOffset + 2];
      rgba[targetOffset + 3] = bytesPerPixel === 4 ? unfiltered[sourceOffset + 3] : 255;
    }
  }

  return { width, height, rgba };
}

/**
 * @param {number} filter
 * @param {number} current
 * @param {number} left
 * @param {number} up
 * @param {number} upLeft
 */
function unfilterByte(filter, current, left, up, upLeft) {
  switch (filter) {
    case 0:
      return current;
    case 1:
      return (current + left) & 0xff;
    case 2:
      return (current + up) & 0xff;
    case 3:
      return (current + Math.floor((left + up) / 2)) & 0xff;
    case 4:
      return (current + paethPredictor(left, up, upLeft)) & 0xff;
    default:
      throw new Error(`unsupported PNG filter ${filter}`);
  }
}

/**
 * @param {number} left
 * @param {number} up
 * @param {number} upLeft
 */
function paethPredictor(left, up, upLeft) {
  const p = left + up - upLeft;
  const pa = Math.abs(p - left);
  const pb = Math.abs(p - up);
  const pc = Math.abs(p - upLeft);
  if (pa <= pb && pa <= pc) return left;
  if (pb <= pc) return up;
  return upLeft;
}
