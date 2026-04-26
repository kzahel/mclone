import { expect, test } from "@playwright/test";
import type { GpuTitleBootResult } from "../../../src/renderer/main";

const GPU_TITLE_SCREENSHOT_PATH = "/tmp/mclone-gpu-root-title-screen.png";
const GPU_OPTIONS_SCREENSHOT_PATH = "/tmp/mclone-gpu-root-options-screen.png";

interface GpuGuiState {
  readonly ready: boolean;
  readonly mode: "title" | "options";
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
});
