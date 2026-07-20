#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { accessSync, constants, existsSync, readFileSync, readdirSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { inflateSync } from "node:zlib";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..");
const args = new Set(process.argv.slice(2).filter((arg) => arg !== "--"));

if (args.has("-h") || args.has("--help")) {
  printUsage();
  process.exit(0);
}

const shouldProbeBrowser = args.has("--probe-browser-webgpu") || args.has("--probe-all");
const asJson = args.has("--json");

const checks = {
  host: readHostFacts(),
  commands: {
    node: checkNodeCommand(),
    pnpm: checkPathCommand("pnpm", readPnpmVersion()),
    npx: checkPathCommand("npx"),
  },
  packageScripts: readPackageScripts(),
  chrome: checkChromeExecutable(),
  playwrightPackageInstalled: existsSync(path.join(REPO_ROOT, "node_modules", "@playwright", "test")),
  probes: {},
};

checks.capabilities = classifyCapabilities(checks);

if (shouldProbeBrowser) {
  checks.probes.browserWebGpu = await probeBrowserWebGpu();
}

if (asJson) {
  console.log(JSON.stringify(checks, null, 2));
} else {
  printReport(checks);
}

function printUsage() {
  console.log(`Usage: pnpm host:check [-- --probe-browser-webgpu] [-- --probe-all] [-- --json]

Reports whether the current host looks suitable for mclone's native-web/browser
WebGPU validation lanes or only for headless native/oracle validation.

Default mode is cheap and does not launch browsers. Probe flags run the smallest
relevant smoke checks.`);
}

function readHostFacts() {
  const displayVars = {
    DISPLAY: process.env.DISPLAY ?? "",
    WAYLAND_DISPLAY: process.env.WAYLAND_DISPLAY ?? "",
    MIR_SOCKET: process.env.MIR_SOCKET ?? "",
  };
  const hasDisplay = Object.values(displayVars).some((value) => value.length > 0);
  const xdgRuntimeDir = process.env.XDG_RUNTIME_DIR ?? "";
  const waylandSockets = findWaylandSockets(xdgRuntimeDir);
  const inferredWaylandDisplay = displayVars.WAYLAND_DISPLAY || waylandSockets[0] || "";
  const driPath = "/dev/dri";
  const driDevices = existsSync(driPath)
    ? readdirSync(driPath).filter((entry) => entry !== "." && entry !== "..").sort()
    : [];

  return {
    platform: process.platform,
    release: os.release(),
    arch: process.arch,
    ci: Boolean(process.env.CI),
    displayVars,
    hasDisplay,
    xdgRuntimeDir,
    xdgSessionType: process.env.XDG_SESSION_TYPE ?? "",
    waylandSockets,
    inferredWaylandDisplay,
    suggestedWaylandBrowserEnv: createSuggestedWaylandBrowserEnv(inferredWaylandDisplay),
    driPathExists: existsSync(driPath),
    driDevices,
    likelyHeadless: process.platform === "linux" && !hasDisplay,
  };
}

function findWaylandSockets(xdgRuntimeDir) {
  if (!xdgRuntimeDir || !existsSync(xdgRuntimeDir)) {
    return [];
  }

  try {
    return readdirSync(xdgRuntimeDir)
      .filter((entry) => /^wayland-\d+$/u.test(entry))
      .sort();
  } catch {
    return [];
  }
}

function createSuggestedWaylandBrowserEnv(waylandDisplay) {
  if (!waylandDisplay) {
    return undefined;
  }
  return {
    CI: "1",
    HEADED: "1",
    WAYLAND_DISPLAY: waylandDisplay,
    XDG_SESSION_TYPE: "wayland",
    MCLONE_NATIVE_WEB_EXTRA_CHROME_ARGS: "--ozone-platform=wayland",
  };
}

function readPackageScripts() {
  const packageJsonPath = path.join(REPO_ROOT, "package.json");
  try {
    const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));
    return packageJson.scripts ?? {};
  } catch (error) {
    return { error: error instanceof Error ? error.message : String(error) };
  }
}

function checkNodeCommand() {
  return {
    ok: true,
    command: "node",
    path: process.execPath,
    summary: process.version,
  };
}

function checkPathCommand(command, version = "") {
  const executablePath = findExecutableOnPath(command);
  return {
    ok: Boolean(executablePath),
    command,
    path: executablePath ?? "",
    summary: version,
    reason: executablePath ? "" : `${command} was not found on PATH`,
  };
}

function checkFirstAvailablePathCommand(candidates) {
  for (const command of candidates) {
    const result = checkPathCommand(command);
    if (result.ok) {
      return result;
    }
  }
  return {
    ok: false,
    command: candidates.join(" | "),
    reason: "no known Chrome or Chromium executable found on PATH",
  };
}

function checkChromeExecutable() {
  const pathResult = checkFirstAvailablePathCommand(["google-chrome", "chrome", "chromium-browser", "chromium"]);
  if (pathResult.ok || process.platform !== "darwin") {
    return pathResult;
  }

  const candidates = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    path.join(os.homedir(), "Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
    path.join(os.homedir(), "Applications/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"),
    path.join(os.homedir(), "Applications/Chromium.app/Contents/MacOS/Chromium"),
  ];
  for (const candidate of candidates) {
    try {
      accessSync(candidate, constants.X_OK);
      return {
        ok: true,
        command: "chrome",
        path: candidate,
        summary: candidate,
      };
    } catch {
      // Keep looking.
    }
  }

  return pathResult;
}

function findExecutableOnPath(command) {
  const pathEnv = process.env.PATH ?? "";
  const pathExt = process.platform === "win32"
    ? (process.env.PATHEXT ?? ".EXE;.CMD;.BAT;.COM").split(";")
    : [""];

  for (const pathEntry of pathEnv.split(path.delimiter)) {
    if (pathEntry.length === 0) {
      continue;
    }

    for (const extension of pathExt) {
      const candidate = path.join(pathEntry, `${command}${extension}`);
      try {
        accessSync(candidate, constants.X_OK);
        return candidate;
      } catch {
        // Keep looking.
      }
    }
  }

  return undefined;
}

function readPnpmVersion() {
  const userAgent = process.env.npm_config_user_agent ?? "";
  const match = userAgent.match(/\bpnpm\/([^\s]+)/u);
  return match ? `pnpm ${match[1]}` : "";
}

function runCommand(command, commandArgs, timeoutMs) {
  const result = spawnSync(command, commandArgs, {
    cwd: REPO_ROOT,
    encoding: "utf8",
    timeout: timeoutMs,
  });
  return normalizeCommandResult(`${command} ${commandArgs.join(" ")}`, result);
}

function normalizeCommandResult(command, result) {
  if (result.error) {
    return {
      ok: false,
      command,
      reason: result.error.message,
      signal: result.signal ?? null,
      status: result.status ?? null,
    };
  }

  const stdout = result.stdout.trim();
  const stderr = result.stderr.trim();
  return {
    ok: result.status === 0,
    command,
    status: result.status,
    signal: result.signal ?? null,
    stdout,
    stderr,
    summary: firstLine(stdout) || firstLine(stderr),
  };
}

function classifyCapabilities(checks) {
  const headless = checks.host.likelyHeadless;
  const hasWaylandSocket = checks.host.waylandSockets.length > 0;
  const hasPnpm = checks.commands.pnpm.ok;
  const hasDri = checks.host.driDevices.length > 0;
  const hasChrome = checks.chrome.ok;
  const hasPlaywright = checks.playwrightPackageInstalled;

  return {
    nodeUnitAndRuntimeTests: hasPnpm
      ? "available: pnpm test, pnpm typecheck, and native cargo tests are host-display independent"
      : "blocked: pnpm is not available on PATH",
    playwrightChromeWebGpu: headless && hasWaylandSocket && hasChrome && hasPlaywright
      ? "candidate via headed Wayland: WAYLAND_DISPLAY is not exported, but a Wayland socket was detected; use the recommended env command below"
      : headless
      ? "expected unavailable on this host: no DISPLAY/WAYLAND display was detected, so Chrome/WebGPU Playwright failures are expected"
      : hasChrome && hasPlaywright
        ? "candidate: a display is present and Chrome plus @playwright/test were found; confirm with pnpm host:check -- --probe-browser-webgpu"
        : "blocked or incomplete: display is present, but Chrome or @playwright/test was not found",
    browserScreenshotsAndProbes: headless && hasWaylandSocket && hasChrome && hasPlaywright
      ? "candidate via headed Wayland: run Playwright with --headed and the recommended WAYLAND_DISPLAY env"
      : headless
      ? "skip here: run native web browser smokes only on a host with a working Chrome GPU/browser path"
      : "candidate: run pnpm native:web:app-smoke and inspect screenshots under /tmp",
    hardwareGpuDevices: hasDri
      ? `visible: ${checks.host.driDevices.join(", ")}`
      : "not visible through /dev/dri; browser/native WebGPU may still use a software backend depending on the host",
  };
}

async function probeBrowserWebGpu() {
  const attempts = [];
  for (const candidate of createBrowserProbeLaunchCandidates()) {
    const attempt = await runBrowserWebGpuProbe(candidate);
    attempts.push(attempt);
    if (attempt.ok) {
      return {
        ...attempt,
        attempts,
      };
    }
  }

  return {
    ok: false,
    reason: attempts.at(-1)?.reason ?? "all browser WebGPU probe attempts failed",
    attempts,
    summary: attempts.map((attempt) => `${attempt.launchLabel}: ${attempt.ok ? "ok" : attempt.reason}`).join("; "),
  };
}

function createBrowserProbeLaunchCandidates() {
  const args = createBrowserWebGpuLaunchArgs();
  const headless = process.env.HEADED === "1" ? false : true;
  const candidates = [
    {
      launchLabel: "chrome-channel",
      launchOptions: {
        channel: "chrome",
        headless,
        args,
        timeout: 15_000,
      },
    },
  ];

  if (process.platform === "darwin") {
    candidates.push({
      launchLabel: "bundled-chromium-angle-metal",
      launchOptions: {
        headless,
        args,
        timeout: 15_000,
      },
    });
  }

  candidates.push({
    launchLabel: "bundled-chromium",
    launchOptions: {
      headless,
      args,
      timeout: 15_000,
    },
  });

  return candidates;
}

function createBrowserWebGpuLaunchArgs() {
  return [
    "--enable-unsafe-webgpu",
    ...(process.platform === "darwin" ? ["--use-angle=metal"] : []),
    ...(process.env.MCLONE_NATIVE_WEB_EXTRA_CHROME_ARGS
      ?.split(/\s+/u)
      .filter(Boolean) ?? []),
  ];
}

async function runBrowserWebGpuProbe(candidate) {
  let browser;
  let server;
  try {
    const { chromium } = await import("@playwright/test");
    server = await startBrowserProbeServer();
    browser = await chromium.launch(candidate.launchOptions);
    const page = await browser.newPage({ viewport: { width: 240, height: 160 } });
    const result = await page.goto(server.url, { waitUntil: "load", timeout: 15_000 })
      .then(() => runBrowserWebGpuPageProbe(page));
    await page.close();
    return {
      ...result,
      launchLabel: candidate.launchLabel,
      launchArgs: candidate.launchOptions.args,
      summary: result.ok
        ? `${candidate.launchLabel} ${result.format}; WebGPU canvas pixel ${formatPixel(result.webGpuCanvasPixel)}; screenshot ${result.screenshotPath}`
        : `${candidate.launchLabel}: ${result.reason}`,
    };
  } catch (error) {
    return {
      ok: false,
      launchLabel: candidate.launchLabel,
      launchArgs: candidate.launchOptions.args,
      reason: error instanceof Error ? error.message : String(error),
    };
  } finally {
    if (browser !== undefined) {
      await browser.close().catch(() => {});
    }
    if (server !== undefined) {
      await server.close();
    }
  }
}

async function startBrowserProbeServer() {
  const html = `<!doctype html><meta charset="utf-8"><body style="margin:0;background:#111"></body>`;
  const server = createServer((_request, response) => {
    response.writeHead(200, {
      "Content-Type": "text/html; charset=utf-8",
      "Cache-Control": "no-store",
    });
    response.end(html);
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  if (address === null || typeof address === "string") {
    throw new Error("browser probe server did not bind to a TCP port");
  }

  return {
    url: `http://127.0.0.1:${address.port.toString()}/`,
    close: () => new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve())),
  };
}

async function runBrowserWebGpuPageProbe(page) {
  const pageResult = await page.evaluate(async () => {
    document.body.innerHTML = [
      "<canvas id=\"webgpu\" width=\"160\" height=\"120\" style=\"display:block;width:160px;height:120px\"></canvas>",
      "<canvas id=\"canvas2d\" width=\"80\" height=\"40\" style=\"display:block;width:80px;height:40px\"></canvas>",
    ].join("");
    if (!navigator.gpu) {
      return { ok: false, reason: "navigator.gpu missing on localhost secure context", isSecureContext };
    }

    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) {
      return { ok: false, reason: "requestAdapter returned null", isSecureContext };
    }

    const device = await adapter.requestDevice();
    const offscreenTexture = device.createTexture({
      size: { width: 4, height: 4 },
      format: "rgba8unorm",
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
    });
    const readback = device.createBuffer({
      size: 1024,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    let encoder = device.createCommandEncoder();
    let pass = encoder.beginRenderPass({
      colorAttachments: [{
        view: offscreenTexture.createView(),
        clearValue: { r: 0.2, g: 0.7, b: 0.1, a: 1 },
        loadOp: "clear",
        storeOp: "store",
      }],
    });
    pass.end();
    encoder.copyTextureToBuffer(
      { texture: offscreenTexture },
      { buffer: readback, bytesPerRow: 256 },
      { width: 4, height: 4 },
    );
    device.queue.submit([encoder.finish()]);
    await device.queue.onSubmittedWorkDone();
    await readback.mapAsync(GPUMapMode.READ);
    const offscreenPixel = [...new Uint8Array(readback.getMappedRange()).slice(0, 4)];
    readback.unmap();

    const canvas = document.querySelector("#webgpu");
    const context = canvas.getContext("webgpu");
    if (!context) {
      return {
        ok: false,
        reason: "canvas.getContext('webgpu') returned null",
        isSecureContext,
        adapterInfo: adapter.info ?? {},
      };
    }

    const format = navigator.gpu.getPreferredCanvasFormat();
    context.configure({ device, format, alphaMode: "opaque" });
    encoder = device.createCommandEncoder();
    pass = encoder.beginRenderPass({
      colorAttachments: [{
        view: context.getCurrentTexture().createView(),
        clearValue: { r: 0, g: 0.35, b: 1, a: 1 },
        loadOp: "clear",
        storeOp: "store",
      }],
    });
    pass.end();
    device.queue.submit([encoder.finish()]);
    await device.queue.onSubmittedWorkDone();

    const context2d = document.querySelector("#canvas2d").getContext("2d");
    context2d.fillStyle = "rgb(255, 0, 0)";
    context2d.fillRect(0, 0, 80, 40);
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));

    return {
      ok: true,
      isSecureContext,
      format,
      adapterInfo: adapter.info ?? {},
      offscreenPixel,
      userAgent: navigator.userAgent,
    };
  });
  if (!pageResult.ok) {
    return pageResult;
  }

  const screenshotPath = "/tmp/mclone-browser-webgpu-probe.png";
  const screenshotBytes = await page.screenshot({ path: screenshotPath });
  const webGpuCanvasPixel = samplePngPixel(screenshotBytes, 20, 20);
  const canvas2dPixel = samplePngPixel(screenshotBytes, 20, 140);
  const offscreenOk = pixelNear(pageResult.offscreenPixel, [51, 179, 26, 255], 4);
  const webGpuCanvasOk = pixelNear(webGpuCanvasPixel, [0, 89, 255, 255], 16);
  const canvas2dOk = pixelNear(canvas2dPixel, [255, 0, 0, 255], 8);
  return {
    ...pageResult,
    ok: offscreenOk && webGpuCanvasOk && canvas2dOk,
    reason: offscreenOk
      ? webGpuCanvasOk
        ? canvas2dOk
          ? undefined
          : `2D canvas screenshot pixel mismatch: got ${formatPixel(canvas2dPixel)}`
        : `WebGPU canvas screenshot pixel mismatch: got ${formatPixel(webGpuCanvasPixel)}`
      : `WebGPU offscreen readback pixel mismatch: got ${formatPixel(pageResult.offscreenPixel)}`,
    screenshotPath,
    webGpuCanvasPixel,
    canvas2dPixel,
    offscreenOk,
    webGpuCanvasOk,
    canvas2dOk,
  };
}

function samplePngPixel(pngBytes, x, y) {
  const png = decodePng(pngBytes);
  if (x < 0 || x >= png.width || y < 0 || y >= png.height) {
    throw new Error(`PNG sample coordinate (${x.toString()}, ${y.toString()}) is outside ${png.width.toString()}x${png.height.toString()}`);
  }

  const offset = ((y * png.width) + x) * 4;
  return [
    png.pixels[offset + 0],
    png.pixels[offset + 1],
    png.pixels[offset + 2],
    png.pixels[offset + 3],
  ];
}

function decodePng(pngBytes) {
  const bytes = Buffer.from(pngBytes);
  const signature = "89504e470d0a1a0a";
  if (bytes.subarray(0, 8).toString("hex") !== signature) {
    throw new Error("screenshot is not a PNG");
  }

  let width = 0;
  let height = 0;
  let bitDepth = 0;
  let colorType = 0;
  let interlace = 0;
  const idatChunks = [];
  let offset = 8;
  while (offset < bytes.byteLength) {
    const length = bytes.readUInt32BE(offset);
    const type = bytes.subarray(offset + 4, offset + 8).toString("ascii");
    const dataStart = offset + 8;
    const dataEnd = dataStart + length;
    const data = bytes.subarray(dataStart, dataEnd);
    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      bitDepth = data[8];
      colorType = data[9];
      interlace = data[12];
    } else if (type === "IDAT") {
      idatChunks.push(data);
    } else if (type === "IEND") {
      break;
    }
    offset = dataEnd + 4;
  }

  if (width <= 0 || height <= 0) {
    throw new Error("PNG is missing a valid IHDR");
  }
  if (bitDepth !== 8 || interlace !== 0 || (colorType !== 2 && colorType !== 6)) {
    throw new Error(`unsupported PNG format: bitDepth=${String(bitDepth)} colorType=${String(colorType)} interlace=${String(interlace)}`);
  }

  const sourceChannels = colorType === 6 ? 4 : 3;
  const stride = width * sourceChannels;
  const inflated = inflateSync(Buffer.concat(idatChunks));
  const raw = new Uint8Array(width * height * sourceChannels);
  let sourceOffset = 0;
  for (let row = 0; row < height; row++) {
    const filter = inflated[sourceOffset++];
    const rowOffset = row * stride;
    for (let column = 0; column < stride; column++) {
      const value = inflated[sourceOffset++];
      const left = column >= sourceChannels ? raw[rowOffset + column - sourceChannels] : 0;
      const up = row > 0 ? raw[rowOffset + column - stride] : 0;
      const upLeft = row > 0 && column >= sourceChannels ? raw[rowOffset + column - stride - sourceChannels] : 0;
      raw[rowOffset + column] = (value + unfilterValue(filter, left, up, upLeft)) & 0xff;
    }
  }

  const pixels = new Uint8Array(width * height * 4);
  for (let pixel = 0; pixel < width * height; pixel++) {
    const sourceOffsetForPixel = pixel * sourceChannels;
    const targetOffset = pixel * 4;
    pixels[targetOffset + 0] = raw[sourceOffsetForPixel + 0];
    pixels[targetOffset + 1] = raw[sourceOffsetForPixel + 1];
    pixels[targetOffset + 2] = raw[sourceOffsetForPixel + 2];
    pixels[targetOffset + 3] = sourceChannels === 4 ? raw[sourceOffsetForPixel + 3] : 255;
  }

  return {
    width,
    height,
    pixels,
  };
}

function unfilterValue(filter, left, up, upLeft) {
  switch (filter) {
    case 0:
      return 0;
    case 1:
      return left;
    case 2:
      return up;
    case 3:
      return Math.floor((left + up) / 2);
    case 4:
      return paeth(left, up, upLeft);
    default:
      throw new Error(`unsupported PNG filter ${String(filter)}`);
  }
}

function paeth(left, up, upLeft) {
  const estimate = left + up - upLeft;
  const leftDistance = Math.abs(estimate - left);
  const upDistance = Math.abs(estimate - up);
  const upLeftDistance = Math.abs(estimate - upLeft);
  if (leftDistance <= upDistance && leftDistance <= upLeftDistance) {
    return left;
  }
  return upDistance <= upLeftDistance ? up : upLeft;
}

function pixelNear(actual, expected, tolerance) {
  if (!Array.isArray(actual) || actual.length < expected.length) {
    return false;
  }
  return expected.every((value, index) => Math.abs((actual[index] ?? 0) - value) <= tolerance);
}

function formatPixel(pixel) {
  return `[${Array.from(pixel, (value) => String(value)).join(",")}]`;
}

function printReport(checks) {
  const host = checks.host;
  console.log("mclone host capability check");
  console.log("");
  console.log("Host");
  console.log(`  platform: ${host.platform} ${host.arch} (${host.release})`);
  console.log(`  CI: ${host.ci ? "yes" : "no"}`);
  console.log(`  display: ${host.hasDisplay ? "present" : "none"}${formatDisplayVars(host.displayVars)}`);
  console.log(`  XDG_RUNTIME_DIR: ${host.xdgRuntimeDir || "(unset)"}`);
  console.log(`  XDG_SESSION_TYPE: ${host.xdgSessionType || "(unset)"}`);
  console.log(`  Wayland sockets: ${host.waylandSockets.length > 0 ? host.waylandSockets.join(", ") : "(none detected)"}`);
  if (!host.displayVars.WAYLAND_DISPLAY && host.inferredWaylandDisplay) {
    console.log(`  inferred WAYLAND_DISPLAY: ${host.inferredWaylandDisplay}`);
  }
  console.log(`  /dev/dri: ${host.driPathExists ? host.driDevices.join(", ") || "(empty)" : "(missing)"}`);
  console.log(`  likely headless: ${host.likelyHeadless ? "yes" : "no"}`);
  console.log("");

  console.log("Commands");
  printCommandLine("node", checks.commands.node);
  printCommandLine("pnpm", checks.commands.pnpm);
  printCommandLine("npx", checks.commands.npx);
  printCommandLine("chrome/chromium", checks.chrome);
  console.log(`  @playwright/test: ${checks.playwrightPackageInstalled ? "installed" : "missing from node_modules"}`);
  console.log("");

  console.log("Capabilities");
  for (const [name, value] of Object.entries(checks.capabilities)) {
    console.log(`  ${name}: ${value}`);
  }

  if (Object.keys(checks.probes).length > 0) {
    console.log("");
    console.log("Probes");
    if (checks.probes.browserWebGpu) {
      printProbe("Browser WebGPU", checks.probes.browserWebGpu);
    }
  }

  console.log("");
  console.log("Recommended lanes");
  if (host.suggestedWaylandBrowserEnv) {
    console.log(
      `  run browser WebGPU on Wayland: ${
        formatEnvCommand(
          host.suggestedWaylandBrowserEnv,
          "pnpm native:web:app-smoke",
        )
      }`,
    );
    console.log(
      `  run mobile browser WebGPU on Wayland: ${
        formatEnvCommand(
          host.suggestedWaylandBrowserEnv,
          "pnpm native:web:mobile-smoke",
        )
      }`,
    );
    console.log(
      `  run canvas/chunk browser smoke on Wayland: ${
        formatEnvCommand(
          host.suggestedWaylandBrowserEnv,
          "pnpm native:web:chunk-smoke",
        )
      }`,
    );
    console.log("  note: headed Wayland is the browser GPU lane validated on this host; headless Chrome may fail even when browser WebGPU is otherwise available");
  } else if (host.likelyHeadless) {
    console.log("  run: pnpm test, pnpm typecheck, pnpm native:web:build");
    console.log("  skip here: native web browser smokes that require a working Chrome GPU/browser path");
  } else {
    console.log("  run: pnpm native:web:app-smoke for browser pixels");
    console.log("  also run: pnpm native:web:mobile-smoke when touching mobile controls");
  }
  if (host.platform === "darwin") {
    console.log("  macOS browser screenshots: use Chrome channel with --enable-unsafe-webgpu and --use-angle=metal; bundled headless Chromium can present WebGPU canvases as black without ANGLE Metal");
  }
  const probeCommand = "pnpm host:check -- --probe-browser-webgpu";
  console.log(
    `  verify Chrome WebGPU canvas capture now: ${
      host.suggestedWaylandBrowserEnv
        ? formatEnvCommand(host.suggestedWaylandBrowserEnv, probeCommand)
        : probeCommand
    }`,
  );
}

function printCommandLine(label, result) {
  if (result.ok) {
    const detail = result.summary || result.path || "";
    console.log(`  ${label}: ok${detail ? ` (${detail})` : ""}`);
  } else {
    console.log(`  ${label}: missing or failed${result.reason ? ` (${result.reason})` : ""}`);
  }
}

function printProbe(label, result) {
  console.log(`  ${label}: ${result.ok ? "ok" : "failed"}`);
  if (result.reason) {
    console.log(`    reason: ${oneLine(result.reason)}`);
  }
  if (result.summary) {
    console.log(`    summary: ${oneLine(result.summary)}`);
  }
  if (result.stderr) {
    console.log(`    stderr: ${oneLine(result.stderr)}`);
  }
  if (result.stdout) {
    console.log(`    stdout: ${oneLine(result.stdout)}`);
  }
}

function formatDisplayVars(displayVars) {
  const populated = Object.entries(displayVars)
    .filter(([, value]) => value.length > 0)
    .map(([key, value]) => `${key}=${value}`);
  return populated.length > 0 ? ` (${populated.join(", ")})` : " (DISPLAY/WAYLAND_DISPLAY unset)";
}

function formatEnvCommand(env, command) {
  const envPrefix = Object.entries(env)
    .map(([key, value]) => `${key}=${shellWord(value)}`)
    .join(" ");
  return `env ${envPrefix} ${command}`;
}

function shellWord(value) {
  const text = String(value);
  if (/^[A-Za-z0-9_./:=-]+$/u.test(text)) {
    return text;
  }
  return `'${text.replace(/'/gu, "'\\''")}'`;
}

function firstLine(value) {
  return value.split(/\r?\n/u).find((line) => line.trim().length > 0)?.trim() ?? "";
}

function oneLine(value) {
  return String(value).replace(/\s+/gu, " ").trim().slice(0, 500);
}
