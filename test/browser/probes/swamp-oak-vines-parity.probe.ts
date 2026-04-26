import { expect, test, type Page } from "@playwright/test";
import { FAST_VISUAL_PROBE_PARAMS, FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const SWAMP_OAK_VINES_SCREENSHOT_PATH = "/tmp/mclone-debug-swamp-oak-vines.png";

interface DebugRuntimeState {
  readonly ready: boolean;
  readonly worldTransport: "worker" | "remote";
  readonly cameraPosition?: readonly [number, number, number];
  readonly chunkViewCenterX?: number;
  readonly chunkViewCenterZ?: number;
  readonly loadedChunkCount: number;
  readonly frameCount: number;
  readonly error?: string;
}

async function readDebugState(page: Page): Promise<DebugRuntimeState> {
  return await page.evaluate(() => window.__mcloneDebug!.state as DebugRuntimeState);
}

test.setTimeout(FAST_VISUAL_PROBE_TIMEOUTS.test);

test("debug free-cam captures swamp-oak vine decoration in the worker world", async ({ page }) => {
  await page.goto(
    `/debug.html?${new URLSearchParams({
      ...FAST_VISUAL_PROBE_PARAMS,
      worldTransport: "worker",
      movementMode: "freecam",
      clearWorldStorage: "1",
      preserveInitialCamera: "1",
      cameraX: "1005.5",
      cameraY: "112",
      cameraZ: "3149.5",
      cameraYaw: "225",
      cameraPitch: "45",
    }).toString()}`,
    { waitUntil: "load" },
  );
  await page.waitForFunction(() => typeof window.__mcloneDebug !== "undefined", undefined, { timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready });
  await page.waitForFunction(
    () => window.__mcloneDebug!.state.ready === true || window.__mcloneDebug!.state.error !== undefined,
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready },
  );
  await page.waitForFunction(
    () => {
      const state = window.__mcloneDebug?.state;
      return (state?.loadedChunkCount ?? 0) > 0 && (state?.frameCount ?? 0) >= 1;
    },
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  await page.waitForTimeout(FAST_VISUAL_PROBE_TIMEOUTS.screenshotSettle);
  await page.locator("#renderer").screenshot({ path: SWAMP_OAK_VINES_SCREENSHOT_PATH });

  const state = await readDebugState(page);
  expect(state.error).toBeUndefined();
  expect(state.worldTransport).toBe("worker");
  expect(state.loadedChunkCount).toBeGreaterThan(0);
  expect(state.cameraPosition).toBeDefined();
  expect(state.cameraPosition![0]).toBe(1005.5);
  expect(state.chunkViewCenterX).toBeDefined();
  expect(state.chunkViewCenterZ).toBeDefined();
});
