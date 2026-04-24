import { defineConfig } from "@playwright/test";
import baseConfig from "./playwright.config";

export default defineConfig({
  ...baseConfig,
  testMatch: [
    "**/smoke.test.ts",
    "**/debug-free-cam.test.ts",
  ],
});
