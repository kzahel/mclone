#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { accessSync, constants, existsSync, readFileSync, readdirSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..");
const DEFAULT_BROWSER_TEST_PORT = "5073";
const args = new Set(process.argv.slice(2).filter((arg) => arg !== "--"));

if (args.has("-h") || args.has("--help")) {
  printUsage();
  process.exit(0);
}

const shouldProbeDeno = args.has("--probe-deno-webgpu") || args.has("--probe-all");
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
  chrome: checkFirstAvailablePathCommand(["google-chrome", "chrome", "chromium-browser", "chromium"]),
  playwrightPackageInstalled: existsSync(path.join(REPO_ROOT, "node_modules", "@playwright", "test")),
  probes: {},
};

checks.capabilities = classifyCapabilities(checks);

if (shouldProbeDeno) {
  checks.probes.denoWebGpu = runCommand("pnpm", ["--silent", "smoke:deno:webgpu"], 90_000);
}

if (shouldProbeBrowser) {
  checks.probes.browserWebGpu = await probeBrowserWebGpu();
}

if (asJson) {
  console.log(JSON.stringify(checks, null, 2));
} else {
  printReport(checks);
}

function printUsage() {
  console.log(`Usage: pnpm host:check [-- --probe-deno-webgpu] [-- --probe-browser-webgpu] [-- --probe-all] [-- --json]

Reports whether the current host looks suitable for mclone's browser/WebGPU Playwright
lanes or only for headless Node/Deno validation.

Default mode is cheap and does not launch browsers or download Deno through npx.
Probe flags run the smallest relevant smoke checks.`);
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
    vitePort: process.env.VITE_PORT ?? "",
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
    VITE_PORT: DEFAULT_BROWSER_TEST_PORT,
    CI: "1",
    WAYLAND_DISPLAY: waylandDisplay,
    XDG_SESSION_TYPE: "wayland",
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
      ? "available: pnpm test, pnpm typecheck, and Node headless host tests are host-display independent"
      : "blocked: pnpm is not available on PATH",
    denoHeadlessWebGpu: hasPnpm
      ? "candidate: run pnpm host:check -- --probe-deno-webgpu, or the focused pnpm smoke:deno:* lane for renderer validation"
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
      ? "skip here: run pnpm test:browser, pnpm test:browser:integration, and pnpm probe:browser only on a host with a working Chrome GPU/browser path"
      : "candidate: run the smallest relevant Playwright lane and inspect screenshots under /tmp",
    hardwareGpuDevices: hasDri
      ? `visible: ${checks.host.driDevices.join(", ")}`
      : "not visible through /dev/dri; Deno may still use a software/native backend if its smoke passes",
  };
}

async function probeBrowserWebGpu() {
  try {
    const { chromium } = await import("@playwright/test");
    const browser = await chromium.launch({
      channel: "chrome",
      args: ["--enable-unsafe-webgpu"],
      timeout: 15_000,
    });

    try {
      const page = await browser.newPage();
      const result = await page.evaluate(async () => {
        if (!navigator.gpu) {
          return { ok: false, reason: "navigator.gpu missing" };
        }

        const adapter = await navigator.gpu.requestAdapter();
        if (!adapter) {
          return { ok: false, reason: "requestAdapter returned null" };
        }

        const device = await adapter.requestDevice();
        const canvas = document.createElement("canvas");
        canvas.width = 4;
        canvas.height = 4;
        document.body.appendChild(canvas);

        const context = canvas.getContext("webgpu");
        if (!context) {
          return {
            ok: false,
            reason: "canvas.getContext('webgpu') returned null",
            adapterInfo: adapter.info ?? {},
          };
        }

        const format = navigator.gpu.getPreferredCanvasFormat();
        context.configure({ device, format, alphaMode: "opaque" });
        const encoder = device.createCommandEncoder();
        const pass = encoder.beginRenderPass({
          colorAttachments: [{
            view: context.getCurrentTexture().createView(),
            clearValue: { r: 0.1, g: 0.2, b: 0.3, a: 1 },
            loadOp: "clear",
            storeOp: "store",
          }],
        });
        pass.end();
        device.queue.submit([encoder.finish()]);
        await device.queue.onSubmittedWorkDone();

        return {
          ok: true,
          format,
          adapterInfo: adapter.info ?? {},
        };
      });
      await page.close();
      return result;
    } finally {
      await browser.close();
    }
  } catch (error) {
    return {
      ok: false,
      reason: error instanceof Error ? error.message : String(error),
    };
  }
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
  console.log(`  VITE_PORT: ${host.vitePort || `(unset; default ${DEFAULT_BROWSER_TEST_PORT})`}`);
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
    if (checks.probes.denoWebGpu) {
      printProbe("Deno WebGPU", checks.probes.denoWebGpu);
    }
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
          "pnpm exec playwright test --config playwright.config.ts --headed",
        )
      }`,
    );
    console.log(
      `  run browser integration on Wayland: ${
        formatEnvCommand(
          host.suggestedWaylandBrowserEnv,
          "pnpm exec playwright test --config playwright.integration.config.ts --headed",
        )
      }`,
    );
    console.log(
      `  run a Wayland screenshot probe: ${
        formatEnvCommand(
          host.suggestedWaylandBrowserEnv,
          "pnpm exec playwright test --config playwright.probes.config.ts --headed test/browser/probes/<name>.probe.ts",
        )
      }`,
    );
    console.log("  note: headed Wayland is the browser GPU lane validated on this host; headless Chrome may fail even when Deno WebGPU passes");
  } else if (host.likelyHeadless) {
    console.log("  run: pnpm test, pnpm typecheck, and focused pnpm smoke:deno:* commands");
    console.log("  skip here: pnpm test:browser, pnpm test:browser:integration, and pnpm probe:browser");
  } else {
    console.log("  run: the smallest relevant pnpm test:browser / probe:browser lane for browser pixels");
    console.log("  also run: focused pnpm smoke:deno:* commands when touching headless renderer hosts");
  }
  if (host.vitePort && host.vitePort !== DEFAULT_BROWSER_TEST_PORT) {
    console.log(`  warning: VITE_PORT is currently ${host.vitePort}; set VITE_PORT=${DEFAULT_BROWSER_TEST_PORT} CI=1 for clean Playwright webServer startup`);
  }
  console.log("  verify Deno WebGPU now: pnpm host:check -- --probe-deno-webgpu");
  console.log("  verify Chrome WebGPU now: pnpm host:check -- --probe-browser-webgpu");
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
