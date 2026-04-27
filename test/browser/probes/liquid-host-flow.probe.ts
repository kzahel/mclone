import { expect, test, type Page } from "@playwright/test";
import { FAST_LIQUID_VISUAL_PROBE_PARAMS, FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const LIQUID_FLOW_SCREENSHOT_PATH = "/tmp/mclone-debug-liquid-flow.png";

interface DebugRuntimeState {
  readonly ready: boolean;
  readonly worldTransport: "worker" | "remote";
  readonly playerChunkX?: number;
  readonly playerChunkZ?: number;
  readonly loadedChunkCount: number;
  readonly frameCount: number;
  readonly error?: string;
}

function createLiquidFlowUrl(): string {
  return `/?mode=debug&${new URLSearchParams({
    ...FAST_LIQUID_VISUAL_PROBE_PARAMS,
    worldTransport: "worker",
    clearWorldStorage: "1",
    preserveInitialCamera: "1",
    cameraX: "45.5",
    cameraY: "104",
    cameraZ: "58.5",
    cameraYaw: "180",
    cameraPitch: "62",
  }).toString()}`;
}

async function readDebugState(page: Page): Promise<DebugRuntimeState> {
  return await page.evaluate(() => window.__mcloneDebug!.state as DebugRuntimeState);
}

test("debug free-cam captures host-executed water flow in the worker generated world", async ({ page }) => {
  await page.goto(createLiquidFlowUrl(), { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneDebug !== "undefined", undefined, { timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready });
  await page.waitForFunction(
    () => window.__mcloneDebug!.state.ready === true || window.__mcloneDebug!.state.error !== undefined,
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready },
  );
  await page.waitForFunction(
    () => {
      const state = window.__mcloneDebug?.state;
      return (state?.loadedChunkCount ?? 0) > 0 && (state?.frameCount ?? 0) >= 10;
    },
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  await page.waitForTimeout(6_000);
  await page.locator("#renderer").screenshot({ path: LIQUID_FLOW_SCREENSHOT_PATH });

  const state = await readDebugState(page);
  expect(state.error).toBeUndefined();
  expect(state.worldTransport).toBe("worker");
  expect(state.loadedChunkCount).toBeGreaterThan(0);
  expect(state.playerChunkX).toBe(2);
  expect(state.playerChunkZ).toBe(3);
});
