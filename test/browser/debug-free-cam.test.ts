import { expect, test, type Page } from "@playwright/test";

const REMOTE_WORLD_HOST_URL = "http://127.0.0.1:4173";
const DEBUG_SCREENSHOT_PATH = "/tmp/mclone-debug-free-cam.png";
const EXPECTED_LOADED_CHUNK_COUNT = 225;

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
  readonly expectedLoadedChunkCount?: number;
  readonly viewDistance?: number;
  readonly renderDistance?: number;
  readonly error?: string;
}

function createDebugUrl(): string {
  return `/debug.html?${new URLSearchParams({
    worldTransport: "remote",
    worldHostUrl: REMOTE_WORLD_HOST_URL,
    preserveInitialCamera: "1",
    cameraX: "965.5",
    cameraY: "168",
    cameraZ: "3189.5",
    cameraYaw: "225",
    cameraPitch: "55",
    viewDistance: "6",
    renderDistance: "192",
    fogColor: "8fb8ff",
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
  await page.waitForFunction(
    (expectedLoadedChunkCount) => {
      const state = window.__mcloneDebug?.state;
      return (state?.loadedChunkCount ?? 0) === expectedLoadedChunkCount && (state?.frameCount ?? 0) >= 10;
    },
    EXPECTED_LOADED_CHUNK_COUNT,
    { timeout: 20_000 },
  );

  const initialState = await readDebugState(page);
  expect(initialState.error).toBeUndefined();
  expect(initialState.worldTransport).toBe("remote");
  expect(initialState.loadedChunkCount).toBe(EXPECTED_LOADED_CHUNK_COUNT);
  expect(initialState.expectedLoadedChunkCount).toBe(EXPECTED_LOADED_CHUNK_COUNT);
  expect(initialState.viewDistance).toBe(6);
  expect(initialState.renderDistance).toBe(192);
  expect(initialState.playerChunkZ).toBeDefined();
  expect(initialState.chunkViewCenterZ).toBeDefined();
  await page.locator("#renderer").screenshot({ path: DEBUG_SCREENSHOT_PATH });

  await page.evaluate(() => {
    window.__mcloneDebug!.setInjectedInput({
      locked: true,
      heldKeys: ["KeyW"],
    });
  });

  await page.waitForFunction(
    ({ initialChunkX, initialChunkZ, initialTick }) => {
      const state = window.__mcloneDebug?.state;
      return state?.playerChunkZ !== undefined
        && state.playerChunkX !== undefined
        && state.chunkViewCenterZ !== undefined
        && state.chunkViewCenterX !== undefined
        && (state.playerChunkX !== initialChunkX || state.playerChunkZ !== initialChunkZ)
        && state.playerChunkX === state.chunkViewCenterX
        && state.playerChunkZ === state.chunkViewCenterZ
        && state.playerTick !== undefined
        && state.playerTick > initialTick;
    },
    {
      initialChunkX: initialState.playerChunkX!,
      initialChunkZ: initialState.playerChunkZ!,
      initialTick: initialState.playerTick ?? 0,
    },
    { timeout: 10_000 },
  );

  await page.evaluate(() => {
    window.__mcloneDebug!.setInjectedInput(null);
  });

  const movedState = await readDebugState(page);
  expect(movedState.playerPosition).not.toEqual(initialState.playerPosition);
  expect(movedState.playerChunkX).toBe(movedState.chunkViewCenterX);
  expect(movedState.playerChunkZ).toBe(movedState.chunkViewCenterZ);
  expect(movedState.loadedChunkCount).toBe(EXPECTED_LOADED_CHUNK_COUNT);
});
