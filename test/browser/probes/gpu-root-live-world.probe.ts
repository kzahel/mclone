import { expect, test } from "@playwright/test";
import type { GpuTitleBootResult } from "../../../src/renderer/main";
import { FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const GPU_ROOT_LIVE_LOADING_SCREENSHOT_PATH = "/tmp/mclone-gpu-root-live-loading.png";
const GPU_ROOT_LIVE_WORLD_SCREENSHOT_PATH = "/tmp/mclone-gpu-root-live-world.png";

interface GpuGuiState {
  readonly ready: boolean;
  readonly mode: "title" | "loading" | "world" | "paused" | "error";
  readonly screenTitle: string;
  readonly lastAction?: string;
  readonly worldReady?: boolean;
  readonly worldResult?: {
    readonly ok: boolean;
    readonly mode?: string;
    readonly viewDistance?: number;
    readonly loadedChunkCount?: number;
    readonly expectedLoadedChunkCount?: number;
  };
  readonly frameCount: number;
  readonly width: number;
  readonly height: number;
  readonly error?: string;
}

test.setTimeout(120_000);

test("root title starts a live world without routing through the smoke scenario", async ({ page }) => {
  await page.goto("/", { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });
  const titleResult = (await page.evaluate(() => window.__mcloneReady)) as GpuTitleBootResult | { readonly ok: false; readonly reason: string };
  expect(titleResult.ok, JSON.stringify(titleResult)).toBe(true);
  expect(titleResult.ok && "mode" in titleResult ? titleResult.mode : undefined).toBe("gpu_title");
  await page.waitForFunction(() => window.__mcloneGui?.state.ready === true, undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });
  await expect(page.locator("button, input, select, textarea, details, form")).toHaveCount(0);

  const box = await page.locator("#renderer").boundingBox();
  expect(box).not.toBeNull();
  const titleState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  const startWorldCenterY = (Math.floor(titleState.height / 4) + 48 + 10) / titleState.height;
  await page.mouse.click(box!.x + (box!.width / 2), box!.y + (box!.height * startWorldCenterY));
  await page.waitForFunction(() => window.__mcloneGui?.state.mode === "loading", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });
  await page.locator("#renderer").screenshot({ path: GPU_ROOT_LIVE_LOADING_SCREENSHOT_PATH });

  await page.waitForFunction(
    () => window.__mcloneGui?.state.worldReady === true || window.__mcloneGui?.state.mode === "error",
    undefined,
    { timeout: 110_000 },
  );
  const worldState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(worldState.error).toBeUndefined();
  expect(worldState.mode).toBe("world");
  expect(worldState.lastAction).toBe("start_world");
  expect(worldState.worldResult?.ok).toBe(true);
  expect(worldState.worldResult?.mode).toBe("live_world");
  expect(worldState.worldResult?.loadedChunkCount).toBeGreaterThanOrEqual(worldState.worldResult?.expectedLoadedChunkCount ?? 1);
  await page.waitForFunction(() => (window.__mcloneGui?.state.frameCount ?? 0) >= 3, undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame,
  });
  await expect(page.locator("button, input, select, textarea, details, form")).toHaveCount(0);
  await page.locator("#renderer").screenshot({ path: GPU_ROOT_LIVE_WORLD_SCREENSHOT_PATH });
});
