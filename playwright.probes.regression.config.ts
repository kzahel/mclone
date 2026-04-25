import { defineConfig } from "@playwright/test";
import probesConfig from "./playwright.probes.config";
import { REGRESSION_PROBE_TEST_MATCH } from "./playwright.probes.tiers";

export default defineConfig({
  ...probesConfig,
  testMatch: [...REGRESSION_PROBE_TEST_MATCH],
});
