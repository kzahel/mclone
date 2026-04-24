import { expect, test, type Page } from "@playwright/test";

const TUFF_SCREENSHOT_PATH = "/tmp/mclone-debug-underground-variety-tuff.png";
const DEEPSLATE_SCREENSHOT_PATH = "/tmp/mclone-debug-underground-variety-deepslate.png";

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

async function captureFrame(
  page: Page,
  params: Record<string, string>,
  screenshotPath: string,
): Promise<DebugRuntimeState> {
  await page.goto(
    `/debug.html?${new URLSearchParams({ worldTransport: "worker", clearWorldStorage: "1", preserveInitialCamera: "1", viewDistance: "1", ...params }).toString()}`,
    {
      waitUntil: "load",
    },
  );
  await page.waitForFunction(() => typeof window.__mcloneDebug !== "undefined", undefined, { timeout: 20_000 });
  await page.waitForFunction(
    () => window.__mcloneDebug!.state.ready === true || window.__mcloneDebug!.state.error !== undefined,
    undefined,
    { timeout: 45_000 },
  );
  await page.waitForFunction(() => (window.__mcloneDebug?.state.frameCount ?? 0) >= 1, undefined, { timeout: 20_000 });
  await page.waitForTimeout(2_000);
  await page.locator("#renderer").screenshot({ path: screenshotPath });
  return await readDebugState(page);
}

test.setTimeout(60_000);

test("captures an exposed tuff pocket in the worker-generated world", async ({ page }) => {
  const state = await captureFrame(
    page,
    {
      cameraX: "55.5",
      cameraY: "13.5",
      cameraZ: "18.5",
      cameraYaw: "57",
      cameraPitch: "15",
    },
    TUFF_SCREENSHOT_PATH,
  );

  expect(state.error).toBeUndefined();
  expect(state.worldTransport).toBe("worker");
  expect(state.loadedChunkCount).toBeGreaterThan(0);
  expect(state.playerChunkX).toBe(3);
  expect(state.playerChunkZ).toBe(1);
});

test("captures an exposed deepslate pocket in the worker-generated world", async ({ page }) => {
  const state = await captureFrame(
    page,
    {
      cameraX: "45.5",
      cameraY: "12.5",
      cameraZ: "23.5",
      cameraYaw: "252",
      cameraPitch: "12",
    },
    DEEPSLATE_SCREENSHOT_PATH,
  );

  expect(state.error).toBeUndefined();
  expect(state.worldTransport).toBe("worker");
  expect(state.loadedChunkCount).toBeGreaterThan(0);
  expect(state.playerChunkX).toBe(2);
  expect(state.playerChunkZ).toBe(1);
});
