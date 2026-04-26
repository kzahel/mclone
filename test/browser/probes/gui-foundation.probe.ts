import { expect, test } from "@playwright/test";
import type { BootResult } from "../../../src/renderer/main";
import { createGeneratedWorldSmokeSearchParams } from "../../../src/renderer/generated-world-smoke-scenario";
import { FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const GUI_FOUNDATION_SCREENSHOT_PATH = "/tmp/mclone-gui-foundation.png";

test.setTimeout(45_000);

test("captures the Gui0 WebGPU overlay over a world frame", async ({ page }) => {
  const params = new URLSearchParams({
    ...Object.fromEntries(createGeneratedWorldSmokeSearchParams({
      worldTransport: "worker",
      worldStorageMode: "none",
      guiProbe: "1",
    })),
  });
  await page.goto(`/smoke.html?${params.toString()}`, { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });
  const result = (await page.evaluate(() => window.__mcloneReady)) as BootResult;
  expect(result.ok, JSON.stringify(result)).toBe(true);
  await page.locator("#renderer").screenshot({ path: GUI_FOUNDATION_SCREENSHOT_PATH });
});
