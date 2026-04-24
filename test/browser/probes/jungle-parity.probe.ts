import { expect, test, type Page } from "@playwright/test";

const JUNGLE_SCREENSHOT_PATH = "/tmp/mclone-debug-jungle.png";

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

test.setTimeout(60_000);

test("debug free-cam captures jungle vegetation in the worker-generated world", async ({ page }) => {
  await page.goto(
    `/debug.html?${new URLSearchParams({
      worldTransport: "worker",
      cameraX: "1656.5",
      cameraY: "120",
      cameraZ: "-3432.5",
      cameraYaw: "225",
      cameraPitch: "72",
    }).toString()}`,
    { waitUntil: "load" },
  );
  await page.waitForFunction(() => typeof window.__mcloneDebug !== "undefined", undefined, { timeout: 20_000 });
  await page.waitForFunction(
    () => window.__mcloneDebug!.state.ready === true || window.__mcloneDebug!.state.error !== undefined,
    undefined,
    { timeout: 20_000 },
  );
  await page.waitForFunction(
    () => {
      const state = window.__mcloneDebug?.state;
      return (state?.loadedChunkCount ?? 0) > 0 && (state?.frameCount ?? 0) >= 1;
    },
    undefined,
    { timeout: 45_000 },
  );
  await page.waitForTimeout(5_000);
  await page.locator("#renderer").screenshot({ path: JUNGLE_SCREENSHOT_PATH });

  const state = await readDebugState(page);
  expect(state.error).toBeUndefined();
  expect(state.worldTransport).toBe("worker");
  expect(state.loadedChunkCount).toBeGreaterThan(0);
  expect(state.playerChunkX).toBe(103);
  expect(state.playerChunkZ).toBe(-215);
});
