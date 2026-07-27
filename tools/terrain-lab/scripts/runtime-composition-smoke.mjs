import { spawn, spawnSync } from "node:child_process";
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
  process.env.TERRAIN_LAB_RUNTIME_SMOKE_PORT ?? (mobile ? "4192" : "4191"),
  10,
);
const externalBaseUrl = process.env.TERRAIN_LAB_SMOKE_BASE_URL
  ?.replace(/\/+$/u, "");
const baseUrl = externalBaseUrl ?? `http://127.0.0.1:${port}`;
const launch = resolveBrowserWebGpuLaunch();
const pageErrors = [];
let server;
let browser;

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
    `${baseUrl}/terrain/?profile=mclone-overworld-v1&visual=mclone-original`
      + "&texture=textured&seed=12345&x=0&z=0&blocks=96"
      + "&detail=auto&surface=inferred&panes=runtime&canonical=final"
      + "&radius=2&water=1&vegetation=1&stage=cover&view=3d"
      + "&projection=perspective&layer=terrain"
      + "&reviewYaw=3.1415927&reviewPitch=0.12",
    { waitUntil: "networkidle" },
  );
  const stage = page.getByTestId("runtime-composition-stage");
  await stage.waitFor({ state: "visible" });
  try {
    await page.waitForFunction(
      () => {
        const shell = document.querySelector(".appShell");
        const stage = document.querySelector("[data-testid='runtime-composition-stage']");
        return shell?.getAttribute("data-runtime-target-ready") === "true"
          && shell?.getAttribute("data-runtime-exact-complete") === "true"
          && stage?.getAttribute("data-runtime-exact-painted")
            === stage?.getAttribute("data-runtime-exact-desired")
          && Number(stage?.getAttribute("data-runtime-exact-desired")) === 25
          && Number(stage?.getAttribute("data-runtime-proxy-trees")) > 0;
      },
      undefined,
      { timeout: 120_000 },
    );
  } catch (error) {
    const failureCapture =
      `/tmp/mclone-terrain-lab-runtime-${label}-timeout.png`;
    await page.screenshot({ path: failureCapture, fullPage: true });
    throw new Error(
      `${error instanceof Error ? error.message : String(error)}\n`
      + `Browser errors: ${pageErrors.join("\n") || "none"}\n`
      + `Capture: ${failureCapture}`,
    );
  }
  if (pageErrors.length > 0) {
    throw new Error(`Runtime composition browser errors:\n${pageErrors.join("\n")}`);
  }
  await page.evaluate(() => new Promise((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(resolve))
  ));

  const canvasCapture = `/tmp/mclone-terrain-lab-runtime-${label}-canvas.png`;
  const pageCapture = `/tmp/mclone-terrain-lab-runtime-${label}.png`;
  await page.locator(
    "canvas[aria-label='Runtime composed exact and procedural terrain']",
  ).screenshot({ path: canvasCapture });
  await page.screenshot({ path: pageCapture, fullPage: true });

  const beforePan = page.url();
  const bounds = await stage.boundingBox();
  if (!bounds) {
    throw new Error("Runtime composition stage has no interactive bounds");
  }
  await page.mouse.move(
    bounds.x + bounds.width * 0.5,
    bounds.y + bounds.height * 0.5,
  );
  await page.mouse.down({ button: "right" });
  await page.mouse.move(
    bounds.x + bounds.width * 0.65,
    bounds.y + bounds.height * 0.58,
  );
  await page.mouse.up({ button: "right" });
  await page.waitForFunction(
    (previous) => window.location.href !== previous,
    beforePan,
  );
  await page.waitForFunction(
    () => document.querySelector(".appShell")
      ?.getAttribute("data-runtime-target-ready") === "true",
    undefined,
    { timeout: 120_000 },
  );
  console.log(
    `Terrain Lab runtime composition ${label} smoke passed\n`
      + `Canvas: ${canvasCapture}\nPage: ${pageCapture}`,
  );
} finally {
  await browser?.close();
  server?.kill("SIGTERM");
}

function runBuild() {
  const result = spawnSync("pnpm", ["web:build"], {
    cwd: terrainLabRoot,
    encoding: "utf8",
    stdio: "inherit",
  });
  if (result.status !== 0) {
    throw new Error(`Terrain Lab runtime build failed with status ${result.status}`);
  }
}

function startPreview() {
  return spawn(
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
    ],
    {
      cwd: terrainLabRoot,
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
}

async function waitForPreview() {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${baseUrl}/terrain/`);
      if (response.ok) {
        return;
      }
    } catch {
      // Preview is still starting.
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`Timed out waiting for Terrain Lab preview at ${baseUrl}`);
}
