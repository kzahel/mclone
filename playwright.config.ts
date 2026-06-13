import { defineConfig } from "@playwright/test";

const DEFAULT_DEV_SERVER_PORT = 5073;

function readDevServerPort(): number {
  const parsed = Number.parseInt(process.env.VITE_PORT ?? "", 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : DEFAULT_DEV_SERVER_PORT;
}

const devServerPort = readDevServerPort();
const webGpuLaunchArgs = [
  "--enable-unsafe-webgpu",
  ...(process.platform === "darwin" ? ["--use-angle=metal"] : []),
];

export default defineConfig({
  testDir: "test/browser",
  testMatch: ["**/smoke.test.ts"],
  fullyParallel: false,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: `http://localhost:${devServerPort.toString()}`,
    channel: "chrome",
    launchOptions: {
      args: webGpuLaunchArgs,
    },
    trace: "retain-on-failure",
  },
  webServer: [
    {
      command: "pnpm dev:browser",
      port: devServerPort,
      reuseExistingServer: !process.env.CI,
      stdout: "pipe",
      stderr: "pipe",
    },
  ],
});
