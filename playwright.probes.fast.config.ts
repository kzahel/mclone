import { defineConfig } from "@playwright/test";
import probesConfig from "./playwright.probes.config";
import { FAST_PROBE_TEST_MATCH } from "./playwright.probes.tiers";

export default defineConfig({
  ...probesConfig,
  testMatch: [...FAST_PROBE_TEST_MATCH],
});
