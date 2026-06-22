import { chromium } from "@playwright/test";
import { spawn, spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { existsSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, normalize, resolve, sep } from "node:path";
import { inflateSync } from "node:zlib";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(scriptDir, "..");
const nativeRoot = resolve(appRoot, "../..");
const wwwRoot = join(appRoot, "www");
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
const appLoop = movementPerf
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
    : mobileAppLoop
    ? "/tmp/mclone-native-web-mobile-app.png"
    : appLoop ? "/tmp/mclone-native-web-app.png" : "/tmp/mclone-native-web-smoke.png");
const canvasScreenshotPath = process.env.MCLONE_NATIVE_WEB_CANVAS_SCREENSHOT
  ?? (movementPerf
    ? "/tmp/mclone-native-web-movement-perf-canvas.png"
    : mobileAppLoop
    ? "/tmp/mclone-native-web-mobile-app-canvas.png"
    : appLoop ? "/tmp/mclone-native-web-app-canvas.png" : "/tmp/mclone-native-web-canvas.png");
const movementPerfReportPath = process.env.MCLONE_NATIVE_WEB_MOVEMENT_PERF_REPORT
  ?? "/tmp/mclone-native-web-movement-perf.json";
const movementPerfChunkBoundaries = Math.max(
  1,
  Number.parseInt(process.env.MCLONE_NATIVE_WEB_MOVEMENT_PERF_CHUNKS ?? "3", 10) || 3,
);
const requireChunk = process.argv.includes("--require-chunk")
  || process.env.MCLONE_NATIVE_WEB_REQUIRE_CHUNK === "1";
const requireCanvas = requireChunk
  || process.argv.includes("--require-canvas")
  || process.env.MCLONE_NATIVE_WEB_REQUIRE_CANVAS === "1";
const requireThreading = process.argv.includes("--require-threading")
  || (!process.argv.includes("--skip-threading")
    && process.env.MCLONE_NATIVE_WEB_REQUIRE_THREADING !== "0");
const remoteWebSocket = process.argv.includes("--remote-websocket")
  || process.env.MCLONE_NATIVE_WEB_REMOTE_WEBSOCKET === "1";
const DIRT_BLOCK_STATE_ID = 5;

run().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});

async function run() {
  buildWasm();
  buildBindgenBundle();
  const server = await startServer();
  const remoteServer = remoteWebSocket ? await startNativeWebSocketServer() : null;
  let browser;
  try {
    const port = server.address().port;
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
    const pageErrors = [];
    const pageLogs = [];
    page.on("pageerror", (error) => pageErrors.push(String(error)));
    page.on("console", (message) => {
      pageLogs.push(`${message.type()}: ${message.text()}`);
      if (message.type() === "error") pageErrors.push(message.text());
    });

    if (appLoop) {
      await page.goto(`${baseUrl}/app.html`, { waitUntil: "load" });
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
        const statusText = await page.locator("#status").textContent().catch(() => null);
        throw new Error(`native web app did not finish booting: ${error instanceof Error ? error.message : String(error)}\nstate=${JSON.stringify(state, null, 2)}\nstatus=${statusText}\nlogs=${pageLogs.join("\n")}`);
      }
      const bootState = await page.evaluate(() => globalThis.__mcloneWebApp.state);
      if (!bootState?.ready || !bootState?.ok) {
        throw new Error(`native web app failed to boot:\n${JSON.stringify(bootState, null, 2)}`);
      }
      const canvas = page.locator("#mclone-canvas");
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
          url: `${baseUrl}/app.html`,
          screenshotPath,
          pageScreenshotCaptured,
          canvasScreenshotPath,
          movementPerfReportPath,
          appLoop,
          mobileViewport,
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
          url: `${baseUrl}/app.html`,
          screenshotPath,
          pageScreenshotCaptured,
          canvasScreenshotPath,
          appLoop,
          mobileAppLoop,
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
            return state?.ok === true
              && state.movementMode === "WALK"
              && state.lastReport?.movementMode === "WALK"
              && state.lastReport?.onGround === true
              && Math.hypot(dx, dz) > 0.2
              && (state.lastReport?.commandCount ?? 0) > start.commandCount;
          },
          walkingStart,
          { timeout: 60_000 },
        );
      } catch (error) {
        const state = await page.evaluate(() => globalThis.__mcloneWebApp?.state ?? null);
        throw new Error(`native web app did not advance walking movement after physical KeyW with Dvorak key value: ${error instanceof Error ? error.message : String(error)}\nstate=${JSON.stringify(state, null, 2)}\nlogs=${pageLogs.join("\n")}`);
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
        return {
          ok: state.movementMode === "WALK"
            && state.lastReport?.movementMode === "WALK"
            && state.lastReport?.onGround === true
            && Math.hypot(dx, dz) > 0.2,
          start,
          end: {
            cameraX: state.cameraX,
            cameraZ: state.cameraZ,
            movementMode: state.movementMode,
            onGround: state.onGround,
            commandCount: state.lastReport?.commandCount ?? 0,
          },
          distance: Math.hypot(dx, dz),
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

      assertAppLoopResult(
        result,
        pageErrors,
        canvasPixels,
        walkingProbe,
        targetPreviewProbe,
        blockInteractionProbe,
      );
      console.log(JSON.stringify({
        url: `${baseUrl}/app.html`,
        screenshotPath,
        pageScreenshotCaptured,
        canvasScreenshotPath,
        appLoop,
        canvasPixels,
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
    const signals = ["SIGINT", "SIGTERM"];
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

async function runMovementPerfProbe(page, canvas) {
  await canvas.evaluate((element) => element.focus());
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      return state?.ok === true
        && state.ready === true
        && state.pendingCompileJobCount === 0
        && state.compileInFlight === false
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
      initialCompileTiming: compileTimings.find((timing) => timing.trigger === "initial") ?? null,
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
    globalThis.__mcloneWebApp.setInputKey("forward", true);
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
      globalThis.__mcloneWebApp.setInputKey("forward", false);
    });
  }

  await waitForWebAppStreamingSettled(page, 120_000);

  const end = await page.evaluate((start) => {
    const state = globalThis.__mcloneWebApp.state;
    const compileTimings = state.compileTimings ?? [];
    const movementCompileTimings = compileTimings.filter((timing) => (
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

async function waitForWebAppStreamingSettled(page, timeout = 60_000) {
  await page.waitForFunction(
    () => {
      const state = globalThis.__mcloneWebApp?.state;
      return state?.ok === true
        && state.pendingCompileJobCount === 0
        && state.compileInFlight === false
        && state.loadedCenterX === state.centerX
        && state.loadedCenterZ === state.centerZ;
    },
    undefined,
    { timeout },
  );
}

async function exerciseMobileTouchControls(page, canvas) {
  const initial = await page.evaluate(() => {
    const state = globalThis.__mcloneWebApp.state;
    const toggle = document.getElementById("hud-toggle");
    return {
      hudOpen: state.hudOpen,
      touchControlsVisible: state.touchControlsVisible,
      ariaExpanded: toggle?.getAttribute("aria-expanded") ?? null,
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
      const impulse = globalThis.__mcloneWebApp.touchControlState()?.movementImpulse;
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
        && globalThis.__mcloneWebApp.touchControlState()?.keys?.forward === false,
      distance: Math.hypot(dx, dz),
      active: activeMovementProbe,
      start,
      end: {
        cameraX: state.cameraX,
        cameraZ: state.cameraZ,
        commandCount: state.lastReport?.commandCount ?? 0,
      },
      touch: globalThis.__mcloneWebApp.touchControlState(),
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
  await page.locator("#hud-toggle").click();
  await page.waitForFunction(
    () => document.getElementById("hud-toggle")?.getAttribute("aria-expanded") === "true",
    undefined,
    { timeout: 10_000 },
  );
  const openedHud = await readHudState(page);
  await page.locator("#hud-toggle").click();
  await page.waitForFunction(
    () => document.getElementById("hud-toggle")?.getAttribute("aria-expanded") === "false",
    undefined,
    { timeout: 10_000 },
  );
  const closedHud = await readHudState(page);

  return {
    ok: initial.hudOpen === false
      && initial.touchControlsVisible === true
      && activeMovementProbe?.ok === true
      && movementProbe.ok
      && lookProbe.ok
      && buttonProbe.ok
      && openedHud.hudOpen === true
      && closedHud.hudOpen === false,
    initial,
    movement: movementProbe,
    look: lookProbe,
    button: buttonProbe,
    hud: {
      opened: openedHud,
      closed: closedHud,
    },
  };
}

async function exerciseTouchButton(page, key, pointerId) {
  const selector = `[data-touch-key="${key}"]`;
  await dispatchPointerEventOnSelector(page, selector, "pointerdown", { pointerId, buttons: 1 });
  await page.waitForFunction(
    (key) => globalThis.__mcloneWebApp?.touchControlState?.()?.keys?.[key] === true
      && globalThis.__mcloneWebApp?.state?.touchButtonActiveCount > 0,
    key,
    { timeout: 10_000 },
  );
  const down = await page.evaluate((key) => ({
    keyDown: globalThis.__mcloneWebApp.touchControlState().keys[key],
    activeCount: globalThis.__mcloneWebApp.state.touchButtonActiveCount,
    activeAttribute: document.querySelector(`[data-touch-key="${key}"]`)?.dataset.active ?? null,
  }), key);
  await dispatchPointerEventOnSelector(page, selector, "pointerup", { pointerId, buttons: 0 });
  await page.waitForFunction(
    (key) => globalThis.__mcloneWebApp?.touchControlState?.()?.keys?.[key] === false
      && globalThis.__mcloneWebApp?.state?.touchButtonActiveCount === 0,
    key,
    { timeout: 10_000 },
  );
  const up = await page.evaluate((key) => ({
    keyDown: globalThis.__mcloneWebApp.touchControlState().keys[key],
    activeCount: globalThis.__mcloneWebApp.state.touchButtonActiveCount,
    activeAttribute: document.querySelector(`[data-touch-key="${key}"]`)?.dataset.active ?? null,
  }), key);
  return {
    ok: down.keyDown === true
      && down.activeCount > 0
      && down.activeAttribute === "true"
      && up.keyDown === false
      && up.activeCount === 0
      && up.activeAttribute === "false",
    down,
    up,
  };
}

async function readHudState(page) {
  return page.evaluate(() => {
    const state = globalThis.__mcloneWebApp.state;
    const hud = document.getElementById("runtime-hud");
    const toggle = document.getElementById("hud-toggle");
    return {
      hudOpen: state.hudOpen,
      hidden: hud?.hidden ?? null,
      ariaExpanded: toggle?.getAttribute("aria-expanded") ?? null,
    };
  });
}

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

async function dispatchCanvasPointerEvent(page, type, options) {
  await page.evaluate(
    ({ type, options }) => {
      const canvas = document.getElementById("mclone-canvas");
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

async function dispatchPointerEventOnSelector(page, selector, type, options) {
  await page.evaluate(
    ({ selector, type, options }) => {
      const target = document.querySelector(selector);
      const rect = target.getBoundingClientRect();
      target.dispatchEvent(new PointerEvent(type, {
        bubbles: true,
        cancelable: true,
        pointerId: options.pointerId,
        pointerType: "touch",
        isPrimary: true,
        clientX: rect.left + rect.width * 0.5,
        clientY: rect.top + rect.height * 0.5,
        button: 0,
        buttons: options.buttons,
      }));
    },
    { selector, type, options },
  );
}

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
    const first = globalThis.__mcloneWebApp.previewBlockTarget();
    const second = globalThis.__mcloneWebApp.previewBlockTarget();
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

async function startServer() {
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url ?? "/", "http://127.0.0.1");
      const path = resolveRequestPath(url.pathname);
      const bytes = await readFile(path);
      response.writeHead(200, {
        "Content-Type": contentType(path),
        "Cache-Control": "no-store",
        ...crossOriginIsolationHeaders(),
      });
      response.end(bytes);
    } catch (error) {
      response.writeHead(error?.code === "ENOENT" ? 404 : 500, {
        "Content-Type": "text/plain; charset=utf-8",
        ...crossOriginIsolationHeaders(),
      });
      response.end(error instanceof Error ? error.message : String(error));
    }
  });

  const requestedPort = Number.parseInt(process.env.MCLONE_NATIVE_WEB_SMOKE_PORT ?? "0", 10);
  await new Promise((resolveListen) => {
    server.listen(Number.isFinite(requestedPort) ? requestedPort : 0, "127.0.0.1", resolveListen);
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

function resolveRequestPath(pathname) {
  if (pathname === "/" || pathname === "/index.html") {
    return join(wwwRoot, "index.html");
  }
  if (pathname === "/mclone-web-smoke.js") {
    return join(wwwRoot, "mclone-web-smoke.js");
  }
  if (pathname === "/mclone-render-compiler-worker.js") {
    return join(wwwRoot, "mclone-render-compiler-worker.js");
  }
  if (pathname === "/mclone-integrated-server-worker.js") {
    return join(wwwRoot, "mclone-integrated-server-worker.js");
  }
  if (pathname === "/mclone-thread-smoke-worker.js") {
    return join(wwwRoot, "mclone-thread-smoke-worker.js");
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

  const resolved = resolve(wwwRoot, `.${normalize(pathname)}`);
  if (resolved !== wwwRoot && !resolved.startsWith(`${wwwRoot}${sep}`)) {
    throw new Error(`refusing to serve path outside smoke root: ${pathname}`);
  }
  return resolved;
}

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
      result.canvas.firstCompileRequest,
      result.canvas.renderCompiler,
      result.canvas.secondCompileRequest,
      result.canvas.secondRenderCompiler,
      result.canvas.renderCompilerPendingJobCount,
      result.canvas.sessionPendingCompileJobCount,
      result.canvas.shutdownReport,
    );
  } else if (canvasPixels.distinctColorCount < 1 || canvasPixels.clearColorPixelCount < 16) {
    throw new Error(`canvas screenshot did not contain the rendered clear color:\n${JSON.stringify(canvasPixels, null, 2)}`);
  }
}

function assertAppLoopResult(
  result,
  pageErrors,
  canvasPixels,
  walkingProbe,
  targetPreviewProbe,
  blockInteractionProbe,
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
  if (!walkingProbe?.ok || walkingProbe.distance <= 0.2) {
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
    || !result.compileTimings?.some((timing) => timing.trigger === "movement")
  ) {
    throw new Error(`native web app did not report initial and movement compile timings:\n${JSON.stringify(result, null, 2)}`);
  }
  assertCompileTimingDiagnostics(result.lastCompileTiming, "app last compile timing");
  if (result.runnerKind !== "web-worker" || result.lastReport?.runnerKind !== "web-worker") {
    throw new Error(`native web app did not use the integrated server Web Worker runner:\n${JSON.stringify(result, null, 2)}`);
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
    throw new Error(`native web mobile controls did not satisfy movement/look/HUD probes:\n${JSON.stringify({ mobileTouchProbe, result }, null, 2)}`);
  }
  if (result.compileTimingCount < 1 || !result.lastCompileTiming) {
    throw new Error(`native web mobile app did not expose compile timing diagnostics:\n${JSON.stringify(result, null, 2)}`);
  }
  assertCompileTimingDiagnostics(result.lastCompileTiming, "mobile last compile timing");
  if (result.touchControlsVisible !== true || result.hudOpen !== false) {
    throw new Error(`native web mobile HUD/touch state ended in an unexpected state:\n${JSON.stringify(result, null, 2)}`);
  }
  if (!Number.isFinite(result.cameraX) || !Number.isFinite(result.cameraY) || !Number.isFinite(result.cameraZ)) {
    throw new Error(`native web mobile app did not report a finite camera pose:\n${JSON.stringify(result, null, 2)}`);
  }
  if (canvasPixels.nonClearInteriorPixelCount < 128 || canvasPixels.distinctInteriorColorCount < 2) {
    throw new Error(`mobile app canvas screenshot did not contain generated chunk pixels:\n${JSON.stringify(canvasPixels, null, 2)}`);
  }
}

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
  if (!probe.movementCompileTimings.some((timing) => (
    Number(timing.frameCountAfter) > Number(timing.frameCountBefore)
    && Number(timing.renderCountAfter) > Number(timing.renderCountBefore)
  ))) {
    throw new Error(`native web movement perf did not observe frame/render progress during a background compile:\n${JSON.stringify(probe.movementCompileTimings, null, 2)}`);
  }
  if (canvasPixels.nonClearInteriorPixelCount < 128 || canvasPixels.distinctInteriorColorCount < 2) {
    throw new Error(`movement perf canvas screenshot did not contain generated chunk pixels:\n${JSON.stringify(canvasPixels, null, 2)}`);
  }
}

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
    || timing.renderCompilerTransportKind !== "message-transfer"
    || timing.renderCompilerMetrics?.transportKind !== "message-transfer"
    || Number(timing.renderCompilerWorkerInitCount) <= 0
    || Number(timing.renderCompilerWorkerWasmInitCount) <= 0
    || Number(timing.renderCompilerCompileCount) <= 0
    || Number(timing.renderCompilerWorkerCompileCount) <= 0
    || Number(timing.renderCompilerAssetPackSendCount) <= 0
    || Number(timing.renderCompilerRequestAssetPackByteLength) <= 0
    || Number(timing.renderCompilerRequestByteLength) <= 0
    || Number(timing.renderCompilerTransferredRequestByteLength) <= 0
    || Number(timing.renderCompilerTransferredResponseByteLength) !== Number(timing.packedByteLength)
    || Number(timing.submittedCompileSectionCount) <= 0
    || Number(timing.acceptedCompileSectionCount) < 0
    || Number(timing.staleCompileSectionCount) < 0
    || Number(timing.uploadedSectionCount) < 0
    || Number(timing.removedSectionCount) < 0
  ) {
    throw new Error(`${label} was missing required diagnostics:\n${JSON.stringify(timing, null, 2)}`);
  }
}

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
    renderCompilerAssetPackSendCountMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerAssetPackSendCount) || 0
    ))),
    renderCompilerWorkerInitCountMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerWorkerInitCount) || 0
    ))),
    renderCompilerWorkerWasmInitCountMax: Math.max(0, ...values.map((timing) => (
      Number(timing.renderCompilerWorkerWasmInitCount) || 0
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

function assertChunkRenderResult(
  report,
  canvasPixels,
  firstReport,
  firstCompileRequest,
  renderCompiler,
  secondCompileRequest,
  secondRenderCompiler,
  renderCompilerPendingJobCount,
  sessionPendingCompileJobCount,
  shutdownReport,
) {
  assertRenderCompileRequest(firstCompileRequest, 0, 0);
  assertRenderCompileRequest(secondCompileRequest, 1, 0);
  assertRenderCompilerWorkerResult(renderCompiler, firstCompileRequest, 0, 0);
  assertRenderCompilerWorkerResult(secondRenderCompiler, secondCompileRequest, 1, 0);
  if (
    renderCompiler.requestId !== firstCompileRequest.requestId
    || firstReport.compileRequestId !== firstCompileRequest.requestId
    || secondRenderCompiler.requestId !== secondCompileRequest.requestId
    || report.compileRequestId !== secondCompileRequest.requestId
  ) {
    throw new Error(`worker compile results did not match the Rust-owned request ids:\n${JSON.stringify({ firstCompileRequest, renderCompiler, firstReport, secondCompileRequest, secondRenderCompiler, report }, null, 2)}`);
  }
  if (renderCompilerPendingJobCount !== 0) {
    throw new Error(`render compiler worker still had pending jobs after chunk smoke:\n${JSON.stringify({ renderCompilerPendingJobCount }, null, 2)}`);
  }
  if (sessionPendingCompileJobCount !== 0 || report.pendingCompileJobCount !== 0) {
    throw new Error(`web render session still had pending compile jobs after chunk smoke:\n${JSON.stringify({ sessionPendingCompileJobCount, report }, null, 2)}`);
  }
  if (!report.chunkLoaded || !report.meshBuilt) {
    throw new Error(`generated chunk did not load/build:\n${JSON.stringify(report, null, 2)}`);
  }
  if (!shutdownReport?.ok || shutdownReport.runnerKind !== "web-worker") {
    throw new Error(`generated chunk render did not shut down the integrated server worker cleanly:\n${JSON.stringify({ shutdownReport, report }, null, 2)}`);
  }
  if (report.runnerKind !== "web-worker") {
    throw new Error(`generated chunk render did not use the integrated server Web Worker runner:\n${JSON.stringify(report, null, 2)}`);
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
  if (!firstReport?.ok) {
    throw new Error(`cached chunk session did not produce the first render report:\n${JSON.stringify(firstReport, null, 2)}`);
  }
  if (
    firstReport.centerX !== 0
    || firstReport.centerZ !== 0
    || firstReport.radiusChunks !== 1
    || report.centerX !== 1
    || report.centerZ !== 0
    || report.radiusChunks !== 1
  ) {
    throw new Error(`cached chunk session did not render the expected centers:\n${JSON.stringify({ firstReport, report }, null, 2)}`);
  }
  if (
    firstReport.assetPackParseCount !== 1
    || firstReport.terrainAssetLoadCount !== 1
    || firstReport.atlasUploadCount !== 1
    || firstReport.meshBuildCount !== 1
    || firstReport.meshUploadCount !== 1
    || firstReport.renderCount !== 1
  ) {
    throw new Error(`first cached render did not initialize exactly one asset/atlas/mesh path:\n${JSON.stringify(firstReport, null, 2)}`);
  }
  if (
    firstReport.loadedChunkCount !== 9
    || firstReport.residentSectionCount <= 1
    || firstReport.uploadedSectionCount !== firstReport.residentSectionCount
    || firstReport.removedSectionCount !== 0
    || firstReport.drawnSectionCount <= 0
  ) {
    throw new Error(`first section render did not upload the initial chunk view:\n${JSON.stringify(firstReport, null, 2)}`);
  }
  const workerSummary = renderCompiler.summary;
  if (
    !firstReport.workerCompileUsed
    || firstReport.workerPackedByteLength !== workerSummary.byteLength
    || firstReport.workerSectionCount !== workerSummary.sectionCount
    || firstReport.workerNonEmptySectionCount !== workerSummary.nonEmptySectionCount
    || firstReport.workerVertexCount !== workerSummary.vertexCount
    || firstReport.workerIndexCount !== workerSummary.indexCount
    || firstReport.workerFaceCount !== workerSummary.faceCount
    || firstReport.submittedCompileSectionCount !== firstCompileRequest.submittedCompileSectionCount
    || firstReport.acceptedCompileSectionCount !== firstCompileRequest.submittedCompileSectionCount
    || firstReport.staleCompileSectionCount !== 0
  ) {
    throw new Error(`first section render did not consume the worker-compiled packed payload:\n${JSON.stringify({ firstReport, renderCompiler }, null, 2)}`);
  }
  if (
    report.assetPackParseCount !== 1
    || report.terrainAssetLoadCount !== 1
    || report.atlasUploadCount !== 1
    || report.meshBuildCount !== 2
    || report.meshUploadCount !== 2
    || report.renderCount !== 2
  ) {
    throw new Error(`second cached render rebuilt non-mesh resources:\n${JSON.stringify(report, null, 2)}`);
  }
  const secondWorkerSummary = secondRenderCompiler.summary;
  if (
    !report.workerCompileUsed
    || report.workerPackedByteLength !== secondWorkerSummary.byteLength
    || report.workerSectionCount !== secondWorkerSummary.sectionCount
    || report.workerNonEmptySectionCount !== secondWorkerSummary.nonEmptySectionCount
    || report.workerVertexCount !== secondWorkerSummary.vertexCount
    || report.workerIndexCount !== secondWorkerSummary.indexCount
    || report.workerFaceCount !== secondWorkerSummary.faceCount
    || report.submittedCompileSectionCount !== secondCompileRequest.submittedCompileSectionCount
    || report.acceptedCompileSectionCount !== secondCompileRequest.submittedCompileSectionCount
    || report.staleCompileSectionCount !== 0
  ) {
    throw new Error(`second section render did not consume the worker-compiled packed payload:\n${JSON.stringify({ report, secondRenderCompiler }, null, 2)}`);
  }
  if (
    report.residentSectionCount <= 1
    || report.drawnSectionCount <= 0
    || report.uploadedSectionCount <= 0
    || report.removedSectionCount <= 0
    || report.uploadedSectionCount >= report.residentSectionCount
  ) {
    throw new Error(`second section render did not stream incremental section updates:\n${JSON.stringify(report, null, 2)}`);
  }
  if (
    report.commandCount !== 2
    || report.updateCount <= firstReport.updateCount
    || report.loadedChunkCount !== 9
  ) {
    throw new Error(`unexpected generated chunk runtime counts:\n${JSON.stringify(report, null, 2)}`);
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
  if (canvasPixels.nonClearInteriorPixelCount < 128 || canvasPixels.distinctInteriorColorCount < 2) {
    throw new Error(`canvas screenshot did not contain generated chunk pixels:\n${JSON.stringify(canvasPixels, null, 2)}`);
  }
}

function assertRenderCompileRequest(request, expectedCenterX, expectedCenterZ) {
  if (
    !request?.ok
    || request.requestId <= 0
    || request.centerX !== expectedCenterX
    || request.centerZ !== expectedCenterZ
    || request.radiusChunks !== 1
    || request.submittedCompileSectionCount <= 0
    || request.pendingCompileJobCount <= 0
  ) {
    throw new Error(`Rust render compile request was invalid:\n${JSON.stringify(request, null, 2)}`);
  }
}

function assertRenderCompilerWorkerResult(renderCompiler, request, expectedCenterX, expectedCenterZ) {
  if (!renderCompiler?.ok) {
    throw new Error(`render compiler worker failed:\n${JSON.stringify(renderCompiler, null, 2)}`);
  }
  const summary = renderCompiler.summary;
  const expectedSectionCount = Number(request?.submittedCompileSectionCount) || 0;
  if (
    renderCompiler.centerX !== expectedCenterX
    || renderCompiler.centerZ !== expectedCenterZ
    || renderCompiler.radiusChunks !== 1
    || !renderCompiler.targetedCompileUsed
    || renderCompiler.targetSectionCount !== expectedSectionCount
    || renderCompiler.transportKind !== "message-transfer"
    || renderCompiler.renderCompilerMetrics?.transportKind !== "message-transfer"
    || Number(renderCompiler.workerInitCount) <= 0
    || Number(renderCompiler.workerWasmInitCount) <= 0
    || Number(renderCompiler.compileCount) <= 0
    || Number(renderCompiler.workerCompileCount) <= 0
    || Number(renderCompiler.assetPackSendCount) <= 0
    || Number(renderCompiler.requestAssetPackByteLength) <= 0
    || Number(renderCompiler.requestByteLength) <= 0
    || Number(renderCompiler.transferredRequestByteLength) <= 0
    || !summary?.ok
    || summary.byteLength <= 0
    || summary.sectionCount !== expectedSectionCount
    || summary.nonEmptySectionCount <= 1
    || summary.vertexCount <= 0
    || summary.indexCount <= 0
    || summary.faceCount <= 0
    || summary.visibilityGraphBuildCount !== expectedSectionCount
  ) {
    throw new Error(`render compiler worker did not return the expected targeted section payload:\n${JSON.stringify({ request, renderCompiler }, null, 2)}`);
  }
  if (renderCompiler.packedByteLength !== summary.byteLength) {
    throw new Error(`render compiler worker did not transfer the packed payload:\n${JSON.stringify(renderCompiler, null, 2)}`);
  }
  if (renderCompiler.transferredResponseByteLength !== renderCompiler.packedByteLength) {
    throw new Error(`render compiler worker did not report transferred response bytes:\n${JSON.stringify(renderCompiler, null, 2)}`);
  }
}

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

function paethPredictor(left, up, upLeft) {
  const p = left + up - upLeft;
  const pa = Math.abs(p - left);
  const pb = Math.abs(p - up);
  const pc = Math.abs(p - upLeft);
  if (pa <= pb && pa <= pc) return left;
  if (pb <= pc) return up;
  return upLeft;
}
