import { expect, test } from "@playwright/test";
import type { GpuTitleBootResult } from "../../../src/renderer/main";

const GPU_TITLE_SCREENSHOT_PATH = "/tmp/mclone-gpu-title-screen.png";

interface GpuGuiState {
  readonly ready: boolean;
  readonly mode: "title";
  readonly lastAction?: string;
  readonly width: number;
  readonly height: number;
}

test.setTimeout(30_000);

test("captures the opt-in GPU title screen and routes button clicks without visible DOM controls", async ({ page }) => {
  await page.goto("/smoke.html?gpuTitle=1", { waitUntil: "load" });
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
  const startWorldCenterY = (Math.floor(stateBeforeClick.height / 4) + 48 + 24 + 10) / stateBeforeClick.height;
  await page.mouse.click(box!.x + (box!.width / 2), box!.y + (box!.height * startWorldCenterY));
  const state = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(state.mode).toBe("title");
  expect(state.lastAction).toBe("start_world");
});
