import { expect, test, type Page } from "@playwright/test";

const REMOTE_WORLD_HOST_URL = "http://127.0.0.1:4173";
const CAVE_MOUTH_SCREENSHOT_PATH = "/tmp/mclone-debug-cave-mouth.png";

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

function createCaveMouthUrl(): string {
  return `/debug.html?${new URLSearchParams({
    worldTransport: "remote",
    worldHostUrl: REMOTE_WORLD_HOST_URL,
    cameraX: "-103.5",
    cameraY: "104",
    cameraZ: "-311.5",
    cameraYaw: "225",
    cameraPitch: "35",
  }).toString()}`;
}

async function readDebugState(page: Page): Promise<DebugRuntimeState> {
  return await page.evaluate(() => window.__mcloneDebug!.state as DebugRuntimeState);
}

test("debug free-cam captures a targeted cave-mouth or ravine frame against the remote host", async ({ page }) => {
  await page.goto(createCaveMouthUrl(), { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneDebug !== "undefined", undefined, { timeout: 20_000 });
  await page.waitForFunction(
    () => window.__mcloneDebug!.state.ready === true || window.__mcloneDebug!.state.error !== undefined,
    undefined,
    { timeout: 20_000 },
  );
  await page.waitForFunction(() => (window.__mcloneDebug?.state.frameCount ?? 0) >= 10, undefined, { timeout: 20_000 });
  await page.locator("#renderer").screenshot({ path: CAVE_MOUTH_SCREENSHOT_PATH });

  const state = await readDebugState(page);
  expect(state.error).toBeUndefined();
  expect(state.worldTransport).toBe("remote");
  expect(state.loadedChunkCount).toBeGreaterThan(0);
  expect(state.playerChunkX).toBe(-7);
  expect(state.playerChunkZ).toBe(-20);
  expect(state.playerPosition?.[0]).toBe(-103.5);
  expect(state.playerPosition?.[2]).toBe(-311.5);
});
