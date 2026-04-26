import { expect, test } from "@playwright/test";
import type { GpuTitleBootResult } from "../../../src/renderer/main";

const GPU_TITLE_SCREENSHOT_PATH = "/tmp/mclone-gpu-root-title-screen.png";
const GPU_OPTIONS_SCREENSHOT_PATH = "/tmp/mclone-gpu-root-options-screen.png";
const GPU_DEBUG_SETTINGS_SCREENSHOT_PATH = "/tmp/mclone-gpu-root-debug-settings.png";

interface GpuGuiState {
  readonly ready: boolean;
  readonly mode: "title" | "options" | "debug_settings";
  readonly screenTitle: string;
  readonly lastAction?: string;
  readonly width: number;
  readonly height: number;
  readonly options?: {
    readonly viewDistance: number;
    readonly renderDistance: number;
    readonly lightingMode: string;
    readonly liquidSimulationMode: string;
  };
  readonly debugSettings?: {
    readonly movementMode: string;
    readonly preset: string;
    readonly worldStorageMode: string;
  };
}

test.setTimeout(30_000);

test("captures the root GPU title screen and routes button clicks without visible DOM controls", async ({ page }) => {
  await page.goto("/", { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined");
  const result = (await page.evaluate(() => window.__mcloneReady)) as GpuTitleBootResult | { readonly ok: false; readonly reason: string };
  expect(result.ok, JSON.stringify(result)).toBe(true);
  expect(result.ok && "mode" in result ? result.mode : undefined).toBe("gpu_title");

  await page.waitForFunction(() => window.__mcloneGui?.state.ready === true);
  await expect(page.locator("button, input, select, textarea")).toHaveCount(0);
  await page.locator("#renderer").screenshot({ path: GPU_TITLE_SCREENSHOT_PATH });

  const box = await page.locator("#renderer").boundingBox();
  expect(box).not.toBeNull();
  const stateBeforeClick = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  const optionsCenterY = (Math.floor(stateBeforeClick.height / 4) + 48 + (24 * 2) + 10) / stateBeforeClick.height;
  await page.mouse.click(box!.x + (box!.width / 2), box!.y + (box!.height * optionsCenterY));
  await page.waitForFunction(() => window.__mcloneGui?.state.mode === "options");
  const state = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(state.screenTitle).toBe("options.title");
  expect(state.lastAction).toBe("options");
  expect(state.options).toBeDefined();
  await page.locator("#renderer").screenshot({ path: GPU_OPTIONS_SCREENSHOT_PATH });

  const doneCenterY = (Math.min(state.height - 28, (Math.floor(state.height / 6) - 12) + (24 * 4) + 12) + 10) / state.height;
  await page.mouse.click(box!.x + (box!.width / 2), box!.y + (box!.height * doneCenterY));
  await page.waitForFunction(() => window.__mcloneGui?.state.mode === "title");
  const titleState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(titleState.lastAction).toBe("options_done");

  const debugSettingsCenterY = (Math.floor(titleState.height / 4) + 48 + (24 * 3) + 10) / titleState.height;
  await page.mouse.click(box!.x + (box!.width / 2), box!.y + (box!.height * debugSettingsCenterY));
  await page.waitForFunction(() => window.__mcloneGui?.state.mode === "debug_settings");
  const debugState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(debugState.screenTitle).toBe("debug.settings.title");
  expect(debugState.lastAction).toBe("debug_settings");
  expect(debugState.debugSettings).toBeDefined();
  await page.locator("#renderer").screenshot({ path: GPU_DEBUG_SETTINGS_SCREENSHOT_PATH });

  const debugDoneCenterY = (Math.min(debugState.height - 28, (Math.floor(debugState.height / 6) - 12) + (24 * 4) + 12) + 10) / debugState.height;
  await page.mouse.click(box!.x + (box!.width / 2), box!.y + (box!.height * debugDoneCenterY));
  await page.waitForFunction(() => window.__mcloneGui?.state.mode === "title");
  const finalTitleState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(finalTitleState.lastAction).toBe("debug_settings_done");
});
