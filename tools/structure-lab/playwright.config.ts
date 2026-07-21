import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@playwright/test";

const structureLabRoot = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(structureLabRoot, "..", "..");
const port = Number.parseInt(process.env.STRUCTURE_LAB_PLAYWRIGHT_PORT ?? "4189", 10);
if (!Number.isInteger(port) || port <= 0) {
  throw new Error(`Invalid STRUCTURE_LAB_PLAYWRIGHT_PORT '${process.env.STRUCTURE_LAB_PLAYWRIGHT_PORT}'`);
}

export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 60_000,
  expect: { timeout: 15_000 },
  fullyParallel: false,
  reporter: process.env.CI ? "dot" : "list",
  outputDir: path.join(repoRoot, "generated-assets", "structure-lab-playwright", "test-results"),
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
    viewport: { width: 1440, height: 1000 },
  },
  webServer: {
    command: `pnpm web:build && pnpm exec vite preview --config src/web/vite.config.ts --host 127.0.0.1 --port ${port} --strictPort`,
    cwd: structureLabRoot,
    reuseExistingServer: false,
    timeout: 180_000,
    url: `http://127.0.0.1:${port}/structures/`,
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
