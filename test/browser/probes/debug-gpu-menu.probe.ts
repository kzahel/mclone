import { expect, test } from "@playwright/test";
import { getDefaultRenderDistance } from "../../../src/renderer/browser-render-config";
import { FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const DEBUG_GPU_WORLD_SCREENSHOT_PATH = "/tmp/mclone-debug-gpu-world.png";
const DEBUG_GPU_OPTIONS_SCREENSHOT_PATH = "/tmp/mclone-debug-gpu-options.png";
const DEBUG_GPU_SETTINGS_SCREENSHOT_PATH = "/tmp/mclone-debug-gpu-settings.png";

interface DebugRuntimeState {
  readonly ready: boolean;
  readonly mode?: "loading" | "world" | "paused" | "error";
  readonly screenTitle?: string;
  readonly lastAction?: string;
  readonly frameCount: number;
  readonly width?: number;
  readonly height?: number;
  readonly error?: string;
}

test.setTimeout(60_000);

test("/?mode=debug uses GPU menus without visible DOM controls", async ({ page }) => {
  const viewDistance = 1;
  const params = new URLSearchParams({
    worldTransport: "worker",
    worldStorageMode: "none",
    clearWorldStorage: "1",
    preserveInitialCamera: "1",
    viewDistance: viewDistance.toString(),
    renderDistance: getDefaultRenderDistance(viewDistance).toString(),
    lightingMode: "none",
    liquidSimulationMode: "none",
  });

  await page.goto(`/?mode=debug&${params.toString()}`, { waitUntil: "load" });
  await expect(page.locator("button, input, select, textarea, details, form")).toHaveCount(0);
  await page.waitForFunction(() => typeof window.__mcloneDebug !== "undefined", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });
  await page.waitForFunction(
    () => window.__mcloneDebug?.state.ready === true || window.__mcloneDebug?.state.error !== undefined,
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready },
  );
  const worldState = await page.evaluate(() => window.__mcloneDebug!.state as DebugRuntimeState);
  expect(worldState.error).toBeUndefined();
  expect(worldState.mode).toBe("world");
  await page.waitForFunction(() => (window.__mcloneDebug?.state.frameCount ?? 0) >= 3, undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame,
  });
  await page.locator("#renderer").screenshot({ path: DEBUG_GPU_WORLD_SCREENSHOT_PATH });

  const box = await page.locator("#renderer").boundingBox();
  expect(box).not.toBeNull();
  await page.keyboard.press("Escape");
  await page.waitForFunction(() => window.__mcloneDebug?.state.screenTitle === "menu.game", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame,
  });
  const pausedState = await page.evaluate(() => window.__mcloneDebug!.state as DebugRuntimeState);
  expect(pausedState.width).toBeDefined();
  expect(pausedState.height).toBeDefined();

  const optionsCenterX = (Math.floor(pausedState.width! / 2) - 53) / pausedState.width!;
  const optionsCenterY = (Math.floor(pausedState.height! / 4) + 96 - 16 + 10) / pausedState.height!;
  await page.mouse.click(box!.x + (box!.width * optionsCenterX), box!.y + (box!.height * optionsCenterY));
  await page.waitForFunction(() => window.__mcloneDebug?.state.screenTitle === "options.title", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame,
  });
  const optionsState = await page.evaluate(() => window.__mcloneDebug!.state as DebugRuntimeState);
  await page.locator("#renderer").screenshot({ path: DEBUG_GPU_OPTIONS_SCREENSHOT_PATH });

  const doneCenterY = (Math.min(optionsState.height! - 28, (Math.floor(optionsState.height! / 6) - 12) + (24 * 4) + 12) + 10) / optionsState.height!;
  await page.mouse.click(box!.x + (box!.width / 2), box!.y + (box!.height * doneCenterY));
  await page.waitForFunction(() => window.__mcloneDebug?.state.screenTitle === "menu.game", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame,
  });

  const debugCenterX = (Math.floor(pausedState.width! / 2) + 53) / pausedState.width!;
  await page.mouse.click(box!.x + (box!.width * debugCenterX), box!.y + (box!.height * optionsCenterY));
  await page.waitForFunction(() => window.__mcloneDebug?.state.screenTitle === "debug.settings.title", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame,
  });
  const debugState = await page.evaluate(() => window.__mcloneDebug!.state as DebugRuntimeState);
  expect(debugState.lastAction).toBe("debug_settings");
  await page.locator("#renderer").screenshot({ path: DEBUG_GPU_SETTINGS_SCREENSHOT_PATH });
});
