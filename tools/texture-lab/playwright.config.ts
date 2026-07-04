import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@playwright/test";

const textureLabRoot = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(textureLabRoot, "..", "..");
const port = Number.parseInt(process.env.TEXTURE_LAB_PLAYWRIGHT_PORT ?? "5187", 10);
const outputRoot = path.join(repoRoot, "generated-assets", "texture-lab-playwright");
const chromeChannel = process.env.PLAYWRIGHT_CHROME_CHANNEL ?? "chrome";

if (!Number.isInteger(port) || port <= 0) {
  throw new Error(`Invalid TEXTURE_LAB_PLAYWRIGHT_PORT '${process.env.TEXTURE_LAB_PLAYWRIGHT_PORT}'`);
}

export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 45_000,
  expect: {
    timeout: 10_000,
  },
  fullyParallel: false,
  reporter: process.env.CI ? "dot" : "list",
  outputDir: path.join(outputRoot, "test-results"),
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
    viewport: { width: 1440, height: 1000 },
  },
  webServer: {
    command: `pnpm exec tsx tests/fixtures/prepare-texture-lab-fixture.ts && pnpm exec tsx src/web-server/server.ts --port=${port}`,
    cwd: textureLabRoot,
    env: {
      MCLONE_TEXTURE_LAB_OUTPUT_ROOT: outputRoot,
    },
    reuseExistingServer: false,
    timeout: 120_000,
    url: `http://127.0.0.1:${port}/api/index`,
  },
  projects: [
    {
      name: "chrome",
      use: {
        channel: chromeChannel,
      },
    },
  ],
});
