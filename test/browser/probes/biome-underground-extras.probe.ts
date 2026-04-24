import { expect, test, type Page } from "@playwright/test";
import { FAST_VISUAL_PROBE_PARAMS } from "./fast-visual-probe-config";

const EMERALD_SCREENSHOT_PATH = "/tmp/mclone-debug-mountain-emerald.png";

interface DebugRuntimeState {
  readonly ready: boolean;
  readonly worldTransport: "worker" | "remote";
  readonly playerChunkX?: number;
  readonly playerChunkZ?: number;
  readonly loadedChunkCount: number;
  readonly frameCount: number;
  readonly error?: string;
}

async function readDebugState(page: Page): Promise<DebugRuntimeState> {
  return await page.evaluate(() => window.__mcloneDebug!.state as DebugRuntimeState);
}

test.setTimeout(60_000);

test("captures exposed mountain emerald ore in the worker-generated world", async ({ page }) => {
  await page.goto(
    `/debug.html?${new URLSearchParams({
      ...FAST_VISUAL_PROBE_PARAMS,
      worldTransport: "worker",
      clearWorldStorage: "1",
      preserveInitialCamera: "1",
      cameraX: "-46.5",
      cameraY: "29.5",
      cameraZ: "50.5",
      cameraYaw: "135",
      cameraPitch: "15",
    }).toString()}`,
    { waitUntil: "load" },
  );
  await page.waitForFunction(() => typeof window.__mcloneDebug !== "undefined", undefined, { timeout: 20_000 });
  await page.waitForFunction(
    () => window.__mcloneDebug!.state.ready === true || window.__mcloneDebug!.state.error !== undefined,
    undefined,
    { timeout: 45_000 },
  );
  await page.waitForFunction(() => (window.__mcloneDebug?.state.frameCount ?? 0) >= 1, undefined, { timeout: 20_000 });
  await page.waitForTimeout(2_000);
  await page.locator("#renderer").screenshot({ path: EMERALD_SCREENSHOT_PATH });

  const state = await readDebugState(page);
  expect(state.error).toBeUndefined();
  expect(state.worldTransport).toBe("worker");
  expect(state.loadedChunkCount).toBeGreaterThan(0);
  expect(state.playerChunkX).toBe(-3);
  expect(state.playerChunkZ).toBe(3);
});
