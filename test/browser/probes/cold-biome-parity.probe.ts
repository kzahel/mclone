import { expect, test, type Page } from "@playwright/test";
import { FAST_VISUAL_PROBE_PARAMS, FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const SNOWY_TAIGA_SCREENSHOT_PATH = "/tmp/mclone-debug-snowy-taiga.png";
const ICE_SPIKES_SCREENSHOT_PATH = "/tmp/mclone-debug-ice-spikes.png";
const GIANT_TAIGA_SCREENSHOT_PATH = "/tmp/mclone-debug-giant-taiga-parity.png";
const MUSHROOM_FIELDS_SCREENSHOT_PATH = "/tmp/mclone-debug-mushroom-fields-parity.png";

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

interface ColdBiomeFrame {
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

const COLD_BIOME_FRAMES: readonly ColdBiomeFrame[] = [
  {
    name: "snowy taiga",
    screenshotPath: SNOWY_TAIGA_SCREENSHOT_PATH,
    cameraX: "-1912.5",
    cameraY: "126",
    cameraZ: "-6392.5",
    cameraYaw: "225",
    cameraPitch: "60",
    expectedChunkX: -120,
    expectedChunkZ: -400,
  },
  {
    name: "ice spikes",
    screenshotPath: ICE_SPIKES_SCREENSHOT_PATH,
    cameraX: "-1311.5",
    cameraY: "148",
    cameraZ: "-7711.5",
    cameraYaw: "225",
    cameraPitch: "58",
    expectedChunkX: -82,
    expectedChunkZ: -482,
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
    name: "mushroom fields",
    screenshotPath: MUSHROOM_FIELDS_SCREENSHOT_PATH,
    cameraX: "-7130.5",
    cameraY: "96",
    cameraZ: "6197.5",
    cameraYaw: "225",
    cameraPitch: "72",
    expectedChunkX: -446,
    expectedChunkZ: 387,
  },
] as const;

function createFrameUrl(frame: ColdBiomeFrame): string {
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

for (const frame of COLD_BIOME_FRAMES) {
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
