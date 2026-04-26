import { expect, test } from "@playwright/test";
import type { BootResult, GpuTitleBootResult } from "../../../src/renderer/main";
import { createGeneratedWorldSmokeSearchParams, validateGeneratedWorldSmokeResult } from "../../../src/renderer/generated-world-smoke-scenario";
import { FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const GPU_TITLE_LOADING_SCREENSHOT_PATH = "/tmp/mclone-gpu-title-loading.png";
const GPU_TITLE_WORLD_SCREENSHOT_PATH = "/tmp/mclone-gpu-title-world.png";

interface GpuGuiState {
  readonly ready: boolean;
  readonly mode: "title" | "loading" | "world" | "error";
  readonly lastAction?: string;
  readonly loadingStage?: string;
  readonly loadingProgress?: number;
  readonly worldReady?: boolean;
  readonly worldResult?: BootResult;
  readonly frameCount: number;
  readonly inputEventCount: number;
  readonly cameraPosition?: readonly [number, number, number];
  readonly cameraYaw?: number;
  readonly cameraPitch?: number;
  readonly error?: string;
  readonly width: number;
  readonly height: number;
}

test.setTimeout(120_000);

test("starts the generated world from the GPU title screen without DOM controls", async ({ page }) => {
  const params = createGeneratedWorldSmokeSearchParams({
    worldTransport: "worker",
    worldStorageMode: "none",
    gpuTitle: "1",
  });

  await page.goto(`/smoke.html?${params.toString()}`, { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });
  const titleResult = (await page.evaluate(() => window.__mcloneReady)) as GpuTitleBootResult | { readonly ok: false; readonly reason: string };
  expect(titleResult.ok, JSON.stringify(titleResult)).toBe(true);
  expect(titleResult.ok && "mode" in titleResult ? titleResult.mode : undefined).toBe("gpu_title");
  await page.waitForFunction(() => window.__mcloneGui?.state.ready === true, undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });

  await expect(page.locator("button, input, select, textarea")).toHaveCount(0);
  const box = await page.locator("#renderer").boundingBox();
  expect(box).not.toBeNull();
  const titleState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  const startWorldCenterY = (Math.floor(titleState.height / 4) + 48 + 24 + 10) / titleState.height;
  await page.mouse.click(box!.x + (box!.width / 2), box!.y + (box!.height * startWorldCenterY));

  await page.waitForFunction(
    () => window.__mcloneGui?.state.mode !== "title",
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready },
  );
  await page.locator("#renderer").screenshot({ path: GPU_TITLE_LOADING_SCREENSHOT_PATH });

  await page.waitForFunction(
    () => window.__mcloneGui?.state.worldReady === true || window.__mcloneGui?.state.mode === "error",
    undefined,
    { timeout: 100_000 },
  );
  const state = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(state.mode, state.error).toBe("world");
  expect(state.lastAction).toBe("start_world");
  expect(state.worldResult?.ok, JSON.stringify(state.worldResult)).toBe(true);
  expect(validateGeneratedWorldSmokeResult(state.worldResult as Extract<BootResult, { ok: true }>, {
    expectedWorldTransport: "worker",
    requirePlayerInput: true,
    requireSteps: true,
  })).toEqual([]);

  await page.waitForFunction(
    () => (window.__mcloneGui?.state.frameCount ?? 0) >= 3,
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );

  await expect(page.locator("button, input, select, textarea")).toHaveCount(0);
  await page.locator("#renderer").screenshot({ path: GPU_TITLE_WORLD_SCREENSHOT_PATH });

  const liveState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  const liveBox = await page.locator("#renderer").boundingBox();
  expect(liveBox).not.toBeNull();
  await page.mouse.move(liveBox!.x + (liveBox!.width / 2), liveBox!.y + (liveBox!.height / 2));
  await page.keyboard.down("w");
  await page.mouse.move(liveBox!.x + (liveBox!.width / 2) + 24, liveBox!.y + (liveBox!.height / 2) + 6, { steps: 3 });
  await page.waitForFunction(
    ({ frameCount, inputEventCount }) => {
      const state = window.__mcloneGui?.state;
      return state !== undefined
        && state.frameCount >= frameCount + 3
        && state.inputEventCount > inputEventCount;
    },
    { frameCount: liveState.frameCount, inputEventCount: liveState.inputEventCount },
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  await page.keyboard.up("w");

  const movedState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(movedState.frameCount).toBeGreaterThan(liveState.frameCount);
  expect(movedState.inputEventCount).toBeGreaterThan(liveState.inputEventCount);
  expect(movedState.cameraPosition).toBeDefined();
  expect(movedState.cameraPosition).not.toEqual(liveState.cameraPosition);
});
