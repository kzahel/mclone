import { expect, test, type Page } from "@playwright/test";
import { FAST_VISUAL_PROBE_PARAMS, FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const MONSTER_ROOM_SCREENSHOT_PATH = "/tmp/mclone-debug-monster-room.png";

interface DebugRuntimeState {
  readonly ready: boolean;
  readonly worldTransport: "worker" | "remote";
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

test("debug free-cam captures the translated monster room in the worker world", async ({ page }) => {
  await page.goto(
    `/?mode=debug&${new URLSearchParams({
      ...FAST_VISUAL_PROBE_PARAMS,
      worldTransport: "worker",
      movementMode: "freecam",
      clearWorldStorage: "1",
      preserveInitialCamera: "1",
      cameraX: "216.5",
      cameraY: "194.5",
      cameraZ: "166.5",
      cameraYaw: "0",
      cameraPitch: "70",
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
  await page.locator("#renderer").screenshot({ path: MONSTER_ROOM_SCREENSHOT_PATH });

  const state = await readDebugState(page);
  expect(state.error).toBeUndefined();
  expect(state.worldTransport).toBe("worker");
  expect(state.loadedChunkCount).toBeGreaterThan(0);
  expect(state.chunkViewCenterX).toBe(13);
  expect(state.chunkViewCenterZ).toBe(10);
});
