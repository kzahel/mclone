import { chromium } from "@playwright/test";
import { spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
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
const appLoop = process.argv.includes("--app-loop")
  || process.env.MCLONE_NATIVE_WEB_APP_LOOP === "1";
const screenshotPath = process.env.MCLONE_NATIVE_WEB_SMOKE_SCREENSHOT
  ?? (appLoop ? "/tmp/mclone-native-web-app.png" : "/tmp/mclone-native-web-smoke.png");
const canvasScreenshotPath = process.env.MCLONE_NATIVE_WEB_CANVAS_SCREENSHOT
  ?? (appLoop ? "/tmp/mclone-native-web-app-canvas.png" : "/tmp/mclone-native-web-canvas.png");
const requireChunk = process.argv.includes("--require-chunk")
  || process.env.MCLONE_NATIVE_WEB_REQUIRE_CHUNK === "1";
const requireCanvas = requireChunk
  || process.argv.includes("--require-canvas")
  || process.env.MCLONE_NATIVE_WEB_REQUIRE_CANVAS === "1";
const requireThreading = process.argv.includes("--require-threading")
  || (!process.argv.includes("--skip-threading")
    && process.env.MCLONE_NATIVE_WEB_REQUIRE_THREADING !== "0");

run().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});

async function run() {
  buildWasm();
  buildBindgenBundle();
  const server = await startServer();
  let browser;
  try {
    const port = server.address().port;
    const baseUrl = `http://127.0.0.1:${port}`;
    browser = await chromium.launch({
      channel: process.env.PLAYWRIGHT_CHROME_CHANNEL ?? "chrome",
      headless: process.env.HEADED === "1" ? false : true,
      args: [
        "--enable-unsafe-webgpu",
        ...(process.platform === "darwin" ? ["--use-angle=metal"] : []),
      ],
    });
    const page = await browser.newPage();
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
      await page.keyboard.down("w");
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
      await page.keyboard.up("w");
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

      assertAppLoopResult(result, pageErrors, canvasPixels, walkingProbe, blockInteractionProbe);
      console.log(JSON.stringify({
        url: `${baseUrl}/app.html`,
        screenshotPath,
        pageScreenshotCaptured,
        canvasScreenshotPath,
        appLoop,
        canvasPixels,
        walkingProbe,
        blockInteractionProbe,
        result,
      }, null, 2));
      return;
    }

    await page.goto(baseUrl, { waitUntil: "load" });
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
      canvasPixels,
      result,
    }, null, 2));
  } finally {
    await browser?.close();
    await new Promise((resolveClose) => server.close(resolveClose));
  }
}

async function exerciseBlockInteraction(page, canvas) {
  const breakProbe = await clickBlockInteraction(page, canvas, "left", "break");
  const placeProbe = await clickBlockInteraction(page, canvas, "right", "place");
  return {
    ok: breakProbe.ok && placeProbe.ok,
    break: breakProbe,
    place: placeProbe,
  };
}

async function clickBlockInteraction(page, canvas, button, action) {
  const start = await page.evaluate(() => {
    const state = globalThis.__mcloneWebApp.state;
    return {
      commandCount: state.lastReport?.commandCount ?? 0,
      interactionCount: state.interactionCount ?? 0,
      meshBuildCount: state.lastCompileReport?.meshBuildCount ?? state.lastReport?.meshBuildCount ?? 0,
    };
  });
  await canvas.click({ position: { x: 640, y: 360 }, button });
  await page.waitForFunction(
    ({ start, action }) => {
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
        && compileReport?.commandCount >= interaction.commandCount
        && compileReport?.acceptedCompileSectionCount > 0
        && compileReport?.meshBuildCount > start.meshBuildCount;
    },
    { start, action },
    { timeout: 60_000 },
  );
  return page.evaluate(
    ({ start, action }) => {
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
    { start, action },
  );
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
    );
  } else if (canvasPixels.distinctColorCount < 1 || canvasPixels.clearColorPixelCount < 16) {
    throw new Error(`canvas screenshot did not contain the rendered clear color:\n${JSON.stringify(canvasPixels, null, 2)}`);
  }
}

function assertAppLoopResult(result, pageErrors, canvasPixels, walkingProbe, blockInteractionProbe) {
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
) {
  assertRenderCompileRequest(firstCompileRequest, 0, 0);
  assertRenderCompileRequest(secondCompileRequest, 1, 0);
  assertRenderCompilerWorkerResult(renderCompiler, 0, 0);
  assertRenderCompilerWorkerResult(secondRenderCompiler, 1, 0);
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

function assertRenderCompilerWorkerResult(renderCompiler, expectedCenterX, expectedCenterZ) {
  if (!renderCompiler?.ok) {
    throw new Error(`render compiler worker failed:\n${JSON.stringify(renderCompiler, null, 2)}`);
  }
  const summary = renderCompiler.summary;
  if (
    renderCompiler.centerX !== expectedCenterX
    || renderCompiler.centerZ !== expectedCenterZ
    || renderCompiler.radiusChunks !== 1
    || !summary?.ok
    || summary.byteLength <= 0
    || summary.sectionCount !== 144
    || summary.nonEmptySectionCount <= 1
    || summary.vertexCount <= 0
    || summary.indexCount <= 0
    || summary.faceCount <= 0
    || summary.visibilityGraphBuildCount !== 144
  ) {
    throw new Error(`render compiler worker did not return the expected section payload:\n${JSON.stringify(renderCompiler, null, 2)}`);
  }
  if (renderCompiler.packedByteLength !== summary.byteLength) {
    throw new Error(`render compiler worker did not transfer the packed payload:\n${JSON.stringify(renderCompiler, null, 2)}`);
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
