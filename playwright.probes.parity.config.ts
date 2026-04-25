import { defineConfig } from "@playwright/test";
import probesConfig from "./playwright.probes.config";
import { PARITY_PROBE_TEST_MATCH } from "./playwright.probes.tiers";

export default defineConfig({
  ...probesConfig,
  testMatch: [...PARITY_PROBE_TEST_MATCH],
});
