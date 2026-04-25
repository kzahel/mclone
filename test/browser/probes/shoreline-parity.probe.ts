import { expect, test, type Page } from "@playwright/test";
import { FAST_VISUAL_PROBE_PARAMS, FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const BEACH_SCREENSHOT_PATH = "/tmp/mclone-debug-beach.png";
const RIVER_SCREENSHOT_PATH = "/tmp/mclone-debug-river.png";
const SNOWY_BEACH_SCREENSHOT_PATH = "/tmp/mclone-debug-snowy-beach.png";
const FROZEN_OCEAN_SCREENSHOT_PATH = "/tmp/mclone-debug-frozen-ocean-shoreline.png";
const STONE_SHORE_SCREENSHOT_PATH = "/tmp/mclone-debug-stone-shore.png";

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

interface ShorelineFrame {
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

const SHORELINE_FRAMES: readonly ShorelineFrame[] = [
  {
    name: "beach and lukewarm ocean",
    screenshotPath: BEACH_SCREENSHOT_PATH,
    cameraX: "-8063.5",
    cameraY: "108",
    cameraZ: "-8191.5",
    cameraYaw: "225",
    cameraPitch: "74",
    expectedChunkX: -504,
    expectedChunkZ: -512,
  },
  {
    name: "river transition",
    screenshotPath: RIVER_SCREENSHOT_PATH,
    cameraX: "-6687.5",
    cameraY: "104",
    cameraZ: "-8191.5",
    cameraYaw: "225",
    cameraPitch: "76",
    expectedChunkX: -418,
    expectedChunkZ: -512,
  },
  {
    name: "snowy beach",
    screenshotPath: SNOWY_BEACH_SCREENSHOT_PATH,
    cameraX: "-863.5",
    cameraY: "100",
    cameraZ: "-8231.5",
    cameraYaw: "225",
    cameraPitch: "58",
    expectedChunkX: -54,
    expectedChunkZ: -515,
  },
  {
    name: "frozen ocean shoreline",
    screenshotPath: FROZEN_OCEAN_SCREENSHOT_PATH,
    cameraX: "-6463.5",
    cameraY: "112",
    cameraZ: "-8191.5",
    cameraYaw: "225",
    cameraPitch: "76",
    expectedChunkX: -404,
    expectedChunkZ: -512,
  },
  {
    name: "stone shore",
    screenshotPath: STONE_SHORE_SCREENSHOT_PATH,
    cameraX: "960.5",
    cameraY: "132",
    cameraZ: "-8127.5",
    cameraYaw: "225",
    cameraPitch: "60",
    expectedChunkX: 60,
    expectedChunkZ: -508,
  },
] as const;

function createFrameUrl(frame: ShorelineFrame): string {
  return `/debug.html?${new URLSearchParams({
    ...FAST_VISUAL_PROBE_PARAMS,
    worldTransport: "worker",
    clearWorldStorage: "1",
    preserveInitialCamera: "1",
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

test.setTimeout(FAST_VISUAL_PROBE_TIMEOUTS.test);

for (const frame of SHORELINE_FRAMES) {
  test(`debug free-cam captures ${frame.name} parity in the worker-generated world`, async ({ page }) => {
    await page.goto(createFrameUrl(frame), { waitUntil: "load" });
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
    await page.locator("#renderer").screenshot({ path: frame.screenshotPath });

    const state = await readDebugState(page);
    expect(state.error).toBeUndefined();
    expect(state.worldTransport).toBe("worker");
    expect(state.loadedChunkCount).toBeGreaterThan(0);
    expect(state.playerChunkX).toBe(frame.expectedChunkX);
    expect(state.playerChunkZ).toBe(frame.expectedChunkZ);
  });
}
