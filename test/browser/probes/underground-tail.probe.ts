import { expect, test, type Page } from "@playwright/test";
import { FAST_VISUAL_PROBE_PARAMS } from "./fast-visual-probe-config";

const GLOW_LICHEN_SCREENSHOT_PATH = "/tmp/mclone-debug-glow-lichen.png";
const DRIPSTONE_SCREENSHOT_PATH = "/tmp/mclone-debug-dripstone-cluster.png";

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

async function captureFrame(
  page: Page,
  params: Record<string, string>,
  screenshotPath: string,
): Promise<DebugRuntimeState> {
  await page.goto(
    `/debug.html?${new URLSearchParams({ ...FAST_VISUAL_PROBE_PARAMS, worldTransport: "worker", clearWorldStorage: "1", preserveInitialCamera: "1", ...params }).toString()}`,
    { waitUntil: "load" },
  );
  await page.waitForFunction(() => typeof window.__mcloneDebug !== "undefined", undefined, { timeout: 20_000 });
  await page.waitForFunction(
    () => window.__mcloneDebug!.state.ready === true || window.__mcloneDebug!.state.error !== undefined,
    undefined,
    { timeout: 45_000 },
  );
  await page.waitForFunction(
    () => {
      const state = window.__mcloneDebug?.state;
      return (state?.loadedChunkCount ?? 0) > 0 && (state?.frameCount ?? 0) >= 1;
    },
    undefined,
    { timeout: 45_000 },
  );
  await page.waitForTimeout(2_000);
  await page.locator("#renderer").screenshot({ path: screenshotPath });
  return await readDebugState(page);
}

test.setTimeout(60_000);

test("captures exposed glow lichen in the worker-generated world", async ({ page }) => {
  const state = await captureFrame(
    page,
    {
      cameraX: "90.5",
      cameraY: "35.5",
      cameraZ: "-82.5",
      cameraYaw: "90",
      cameraPitch: "0",
    },
    GLOW_LICHEN_SCREENSHOT_PATH,
  );

  expect(state.error).toBeUndefined();
  expect(state.worldTransport).toBe("worker");
  expect(state.loadedChunkCount).toBeGreaterThan(0);
  expect(state.playerChunkX).toBe(5);
  expect(state.playerChunkZ).toBe(-6);
});

test("captures an exposed dripstone cluster in the worker-generated world", async ({ page }) => {
  const state = await captureFrame(
    page,
    {
      cameraX: "99.5",
      cameraY: "37.5",
      cameraZ: "-74.5",
      cameraYaw: "135",
      cameraPitch: "17",
    },
    DRIPSTONE_SCREENSHOT_PATH,
  );

  expect(state.error).toBeUndefined();
  expect(state.worldTransport).toBe("worker");
  expect(state.loadedChunkCount).toBeGreaterThan(0);
  expect(state.playerChunkX).toBe(6);
  expect(state.playerChunkZ).toBe(-5);
});
