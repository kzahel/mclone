import { expect, test, type Page } from "@playwright/test";
import { FAST_VISUAL_PROBE_PARAMS } from "./fast-visual-probe-config";

const FROZEN_SCREENSHOT_PATH = "/tmp/mclone-debug-frozen-ocean.png";
const BADLANDS_SCREENSHOT_PATH = "/tmp/mclone-debug-badlands.png";
const GIANT_TAIGA_SCREENSHOT_PATH = "/tmp/mclone-debug-giant-taiga.png";
const SHATTERED_SAVANNA_SCREENSHOT_PATH = "/tmp/mclone-debug-shattered-savanna.png";
const MUSHROOM_SCREENSHOT_PATH = "/tmp/mclone-debug-mushroom-fields.png";

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

interface SurfaceFrame {
  readonly name: string;
  readonly screenshotPath: string;
  readonly cameraX: string;
  readonly cameraY: string;
  readonly cameraZ: string;
  readonly cameraYaw: string;
  readonly cameraPitch: string;
  readonly expectedChunkX: number;
  readonly expectedChunkZ: number;
}

const SURFACE_FRAMES: readonly SurfaceFrame[] = [
  {
    name: "frozen ocean",
    screenshotPath: FROZEN_SCREENSHOT_PATH,
    cameraX: "-3930.5",
    cameraY: "124",
    cameraZ: "-3928.5",
    cameraYaw: "225",
    cameraPitch: "55",
    expectedChunkX: -246,
    expectedChunkZ: -246,
  },
  {
    name: "badlands",
    screenshotPath: BADLANDS_SCREENSHOT_PATH,
    cameraX: "-5112.5",
    cameraY: "116",
    cameraZ: "1592.5",
    cameraYaw: "225",
    cameraPitch: "50",
    expectedChunkX: -320,
    expectedChunkZ: 99,
  },
  {
    name: "giant taiga",
    screenshotPath: GIANT_TAIGA_SCREENSHOT_PATH,
    cameraX: "-138.5",
    cameraY: "126",
    cameraZ: "1093.5",
    cameraYaw: "225",
    cameraPitch: "55",
    expectedChunkX: -9,
    expectedChunkZ: 68,
  },
  {
    name: "shattered savanna",
    screenshotPath: SHATTERED_SAVANNA_SCREENSHOT_PATH,
    cameraX: "965.5",
    cameraY: "168",
    cameraZ: "3189.5",
    cameraYaw: "225",
    cameraPitch: "55",
    expectedChunkX: 60,
    expectedChunkZ: 199,
  },
  {
    name: "mushroom fields",
    screenshotPath: MUSHROOM_SCREENSHOT_PATH,
    cameraX: "-7130.5",
    cameraY: "96",
    cameraZ: "6197.5",
    cameraYaw: "225",
    cameraPitch: "72",
    expectedChunkX: -446,
    expectedChunkZ: 387,
  },
] as const;

function createSurfaceUrl(frame: SurfaceFrame): string {
  return `/debug.html?${new URLSearchParams({
    ...FAST_VISUAL_PROBE_PARAMS,
    worldTransport: "worker",
    clearWorldStorage: "1",
    cameraX: frame.cameraX,
    cameraY: frame.cameraY,
    cameraZ: frame.cameraZ,
    cameraYaw: frame.cameraYaw,
    cameraPitch: frame.cameraPitch,
  }).toString()}`;
}

async function readDebugState(page: Page): Promise<DebugRuntimeState> {
  return await page.evaluate(() => window.__mcloneDebug!.state as DebugRuntimeState);
}

test.setTimeout(60_000);

for (const frame of SURFACE_FRAMES) {
  test(`debug free-cam captures the ${frame.name} surface material family in the worker-generated world`, async ({ page }) => {
    await page.goto(createSurfaceUrl(frame), { waitUntil: "load" });
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
    await page.locator("#renderer").screenshot({ path: frame.screenshotPath });

    const state = await readDebugState(page);
    expect(state.error).toBeUndefined();
    expect(state.worldTransport).toBe("worker");
    expect(state.loadedChunkCount).toBeGreaterThan(0);
    expect(state.playerChunkX).toBe(frame.expectedChunkX);
    expect(state.playerChunkZ).toBe(frame.expectedChunkZ);
  });
}
