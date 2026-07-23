import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, devices } from "@playwright/test";

import { resolveBrowserWebGpuLaunch } from "../../scripts/browser-webgpu-env.mjs";

const terrainLabRoot = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(terrainLabRoot, "..", "..");
const port = Number.parseInt(process.env.TERRAIN_LAB_PLAYWRIGHT_PORT ?? "4190", 10);
const launch = resolveBrowserWebGpuLaunch();

if (!Number.isInteger(port) || port <= 0) {
  throw new Error(`Invalid TERRAIN_LAB_PLAYWRIGHT_PORT '${process.env.TERRAIN_LAB_PLAYWRIGHT_PORT}'`);
}

const launchOptions = {
  args: ["--enable-unsafe-webgpu", ...launch.chromeArgs],
  env: { ...process.env, ...launch.browserEnv },
};

export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 90_000,
  expect: { timeout: 30_000 },
  fullyParallel: false,
  workers: 1,
  reporter: process.env.CI ? "dot" : "list",
  outputDir: path.join(repositoryRoot, "generated-assets", "terrain-lab-playwright"),
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    channel: process.env.PLAYWRIGHT_CHROME_CHANNEL ?? "chrome",
    headless: launch.headless,
    launchOptions,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
  webServer: {
    command:
      `pnpm web:build && pnpm exec vite preview --config src/web/vite.config.ts `
      + `--host 127.0.0.1 --port ${port} --strictPort`,
    cwd: terrainLabRoot,
    reuseExistingServer: false,
    timeout: 180_000,
    url: `http://127.0.0.1:${port}/terrain/`,
  },
  projects: [
    {
      name: "desktop-chrome",
      use: { viewport: { width: 1440, height: 1000 } },
    },
    {
      name: "phone-chrome",
      use: {
        ...devices["Pixel 7"],
        channel: process.env.PLAYWRIGHT_CHROME_CHANNEL ?? "chrome",
        headless: launch.headless,
        launchOptions,
      },
    },
  ],
});
