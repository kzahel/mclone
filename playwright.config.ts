import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "test/browser",
  fullyParallel: false,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:5173",
    channel: "chrome",
    launchOptions: {
      args: ["--enable-unsafe-webgpu"],
    },
    trace: "retain-on-failure",
  },
  webServer: [
    {
      command: "pnpm dev:browser",
      port: 5173,
      reuseExistingServer: !process.env.CI,
      stdout: "pipe",
      stderr: "pipe",
    },
  ],
});
