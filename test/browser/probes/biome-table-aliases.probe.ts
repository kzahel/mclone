import { expect, test, type Page } from "@playwright/test";
import { FAST_VISUAL_PROBE_PARAMS, FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const MODIFIED_JUNGLE_SCREENSHOT_PATH = "/tmp/mclone-debug-modified-jungle.png";

interface DebugRuntimeState {
  readonly ready: boolean;
  readonly worldTransport: "worker" | "remote";
  readonly playerPosition?: readonly [number, number, number];
  readonly playerChunkX?: number;
  readonly playerChunkZ?: number;
  readonly loadedChunkCount: number;
  readonly frameCount: number;
  readonly error?: string;
}

async function readDebugState(page: Page): Promise<DebugRuntimeState> {
  return await page.evaluate(() => window.__mcloneDebug!.state as DebugRuntimeState);
}

test.setTimeout(FAST_VISUAL_PROBE_TIMEOUTS.test);

test("debug free-cam captures modified-jungle decoration in the worker-generated world", async ({ page }) => {
  await page.goto(
    `/debug.html?${new URLSearchParams({
      ...FAST_VISUAL_PROBE_PARAMS,
      worldTransport: "worker",
      clearWorldStorage: "1",
      cameraX: "8776.5",
      cameraY: "120",
      cameraZ: "-184.5",
      cameraYaw: "225",
      cameraPitch: "15",
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
  await page.locator("#renderer").screenshot({ path: MODIFIED_JUNGLE_SCREENSHOT_PATH });

  const state = await readDebugState(page);
  expect(state.error).toBeUndefined();
  expect(state.worldTransport).toBe("worker");
  expect(state.loadedChunkCount).toBeGreaterThan(0);
  expect(state.playerChunkX).toBe(548);
  expect(state.playerChunkZ).toBe(-12);
});
