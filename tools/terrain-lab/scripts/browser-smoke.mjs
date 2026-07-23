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
const baseUrl = `http://127.0.0.1:${port}`;
const launch = resolveBrowserWebGpuLaunch();
const pageErrors = [];
let server;
let browser;

if (!Number.isInteger(port) || port <= 0) {
  throw new Error(`Invalid TERRAIN_LAB_SMOKE_PORT '${process.env.TERRAIN_LAB_SMOKE_PORT}'`);
}

try {
  runBuild();
  server = startPreview();
  await waitForPreview();

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
  await page.locator("[data-testid='terrain-diagnostics']").scrollIntoViewIfNeeded();
  await settlePaint(page);
  await page.evaluate(() => window.scrollTo(0, 0));
  await settlePaint(page);
  const initialRevision = Number(await shell.getAttribute("data-render-revision"));

  const pageCapture = `/tmp/mclone-terrain-lab-${label}.png`;
  const canvasCapture = `/tmp/mclone-terrain-lab-${label}-canvas.png`;
  const errorCapture = `/tmp/mclone-terrain-lab-${label}-map-error.png`;
  await page.screenshot({ path: pageCapture, fullPage: true });
  await page.locator("canvas[aria-label='Live GPU terrain preview']").screenshot({
    path: canvasCapture,
  });

  await page.getByLabel("Diagnostic layer").selectOption("error");
  await waitForRevision(shell, initialRevision);
  const errorRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByRole("button", { name: "Map", exact: true }).click();
  await page.getByRole("button", { name: "Zoom out" }).click();
  await waitForRevision(shell, errorRevision);
  await page.evaluate(() => window.scrollTo(0, 0));
  await settlePaint(page);
  await page.screenshot({ path: errorCapture, fullPage: true });

  const finalUrl = page.url();
  if (!finalUrl.includes("layer=error")
      || !finalUrl.includes("view=map")
      || !finalUrl.includes("spacing=64")) {
    throw new Error(`Terrain Lab controls did not round-trip through the URL: ${finalUrl}`);
  }
  if (pageErrors.length > 0) {
    throw new Error(`Browser errors:\n${pageErrors.join("\n")}`);
  }

  const report = {
    adapter: await page.locator("[data-testid='adapter-name']").textContent(),
    captures: { canvasCapture, errorCapture, pageCapture },
    comparison: await page.locator("[data-testid='terrain-diagnostics']").innerText(),
    finalUrl,
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
