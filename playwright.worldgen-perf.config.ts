import { defineConfig } from "@playwright/test";

const WORLDGEN_PERF_DEV_SERVER_PORT = 5074;

export default defineConfig({
  testDir: "test/browser",
  testMatch: ["**/worldgen-browser-flyby.test.ts"],
  fullyParallel: false,
  workers: 1,
  timeout: 120_000,
  reporter: [["list"]],
  use: {
    baseURL: `http://localhost:${WORLDGEN_PERF_DEV_SERVER_PORT.toString()}`,
    channel: "chrome",
    launchOptions: {
      args: ["--enable-unsafe-webgpu"],
    },
    trace: "retain-on-failure",
  },
  webServer: [
    {
      command: `VITE_PORT=${WORLDGEN_PERF_DEV_SERVER_PORT.toString()} pnpm dev:browser`,
      port: WORLDGEN_PERF_DEV_SERVER_PORT,
      reuseExistingServer: !process.env.CI,
      stdout: "pipe",
      stderr: "pipe",
    },
  ],
});
