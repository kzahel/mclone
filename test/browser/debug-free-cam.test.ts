import { expect, test, type Page } from "@playwright/test";

const REMOTE_WORLD_HOST_URL = "http://127.0.0.1:4173";
const DEBUG_SCREENSHOT_PATH = "/tmp/mclone-debug-free-cam.png";

interface DebugRuntimeState {
  readonly ready: boolean;
  readonly worldTransport: "worker" | "remote";
  readonly playerTick?: number;
  readonly playerPosition?: readonly [number, number, number];
  readonly playerChunkX?: number;
  readonly playerChunkZ?: number;
  readonly chunkViewCenterX?: number;
  readonly chunkViewCenterZ?: number;
  readonly loadedChunkCount: number;
  readonly error?: string;
}

function createDebugUrl(): string {
  return `/debug.html?${new URLSearchParams({
    worldTransport: "remote",
    worldHostUrl: REMOTE_WORLD_HOST_URL,
  }).toString()}`;
}

async function readDebugState(page: Page): Promise<DebugRuntimeState> {
  return await page.evaluate(() => window.__mcloneDebug!.state as DebugRuntimeState);
}

test("debug free-cam follows authoritative player_state and moves chunk interest with remote input", async ({ page }) => {
  await page.goto(createDebugUrl(), { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneDebug !== "undefined", undefined, { timeout: 10_000 });
  await page.waitForFunction(
    () => window.__mcloneDebug!.state.ready === true || window.__mcloneDebug!.state.error !== undefined,
    undefined,
    { timeout: 10_000 },
  );

  const initialState = await readDebugState(page);
  expect(initialState.error).toBeUndefined();
  expect(initialState.worldTransport).toBe("remote");
  expect(initialState.loadedChunkCount).toBeGreaterThan(0);
  expect(initialState.playerChunkZ).toBeDefined();
  expect(initialState.chunkViewCenterZ).toBeDefined();

  await page.evaluate(() => {
    window.__mcloneDebug!.setInjectedInput({
      locked: true,
      heldKeys: ["KeyW"],
    });
  });

  await page.waitForFunction(
    ({ initialChunkZ, initialTick }) => {
      const state = window.__mcloneDebug?.state;
      return state?.playerChunkZ !== undefined
        && state.chunkViewCenterZ !== undefined
        && state.playerChunkZ < initialChunkZ
        && state.chunkViewCenterZ < initialChunkZ
        && state.playerTick !== undefined
        && state.playerTick > initialTick;
    },
    {
      initialChunkZ: initialState.playerChunkZ!,
      initialTick: initialState.playerTick ?? 0,
    },
    { timeout: 10_000 },
  );

  await page.evaluate(() => {
    window.__mcloneDebug!.setInjectedInput(null);
  });
  await page.locator("#renderer").screenshot({ path: DEBUG_SCREENSHOT_PATH });

  const movedState = await readDebugState(page);
  expect(movedState.playerPosition?.[2]).toBeLessThan(initialState.playerPosition![2]!);
  expect(movedState.playerChunkX).toBe(movedState.chunkViewCenterX);
  expect(movedState.playerChunkZ).toBe(movedState.chunkViewCenterZ);
  expect(movedState.loadedChunkCount).toBeGreaterThan(0);
});
