import { expect, test } from "@playwright/test";
import type { BootResult } from "../../../src/renderer/main";
import { createGeneratedWorldSmokeSearchParams } from "../../../src/renderer/generated-world-smoke-scenario";
import { FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const GPU_TOUCH_JOYSTICK_SCREENSHOT_PATH = "/tmp/mclone-gpu-touch-joystick-hud.png";

interface GpuGuiState {
  readonly mode: "title" | "loading" | "world" | "paused" | "error";
  readonly worldReady?: boolean;
  readonly frameCount: number;
  readonly inputEventCount: number;
  readonly error?: string;
}

test.setTimeout(75_000);

test("renders the GPU touch joystick HUD while the left touch control is active", async ({ page }) => {
  const params = createGeneratedWorldSmokeSearchParams({
    worldTransport: "worker",
    worldStorageMode: "none",
    gpuTitle: "1",
  });
  params.set("autoStartWorld", "1");

  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(`/smoke.html?${params.toString()}`, { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });
  const result = (await page.evaluate(() => window.__mcloneReady)) as BootResult;
  expect(result.ok, JSON.stringify(result)).toBe(true);
  await page.waitForFunction(
    () => window.__mcloneGui?.state.worldReady === true || window.__mcloneGui?.state.mode === "error",
    undefined,
    { timeout: 60_000 },
  );
  const worldState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(worldState.error).toBeUndefined();
  expect(worldState.mode).toBe("world");

  const box = await page.locator("#renderer").boundingBox();
  expect(box).not.toBeNull();
  const startX = box!.x + 82;
  const startY = box!.y + box!.height - 132;
  const thumbX = startX + 34;
  const thumbY = startY - 28;
  const client = await page.context().newCDPSession(page);
  await client.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: [{ x: startX, y: startY, radiusX: 8, radiusY: 8, force: 1, id: 1 }],
    modifiers: 0,
  });
  await client.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [{ x: thumbX, y: thumbY, radiusX: 8, radiusY: 8, force: 1, id: 1 }],
    modifiers: 0,
  });
  await page.waitForFunction(
    ({ frameCount, inputEventCount }) => {
      const state = window.__mcloneGui?.state;
      return state !== undefined
        && state.frameCount >= frameCount + 2
        && state.inputEventCount > inputEventCount;
    },
    { frameCount: worldState.frameCount, inputEventCount: worldState.inputEventCount },
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );

  await page.locator("#renderer").screenshot({ path: GPU_TOUCH_JOYSTICK_SCREENSHOT_PATH });
  await client.send("Input.dispatchTouchEvent", {
    type: "touchEnd",
    touchPoints: [],
    modifiers: 0,
  });
});
