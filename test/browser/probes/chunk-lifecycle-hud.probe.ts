import { expect, test } from "@playwright/test";
import { getDefaultRenderDistance } from "../../../src/renderer/browser-render-config";
import { FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const CHUNK_LIFECYCLE_HUD_SCREENSHOT_PATH = "/tmp/mclone-chunk-lifecycle-hud.png";

interface ChunkLifecycleHudDebugState {
  readonly ready: boolean;
  readonly mode?: "loading" | "world" | "paused" | "error";
  readonly frameCount: number;
  readonly chunkLifecycle?: {
    readonly currentChunkView?: {
      readonly centerChunkX: number;
      readonly centerChunkZ: number;
      readonly radius: number;
    };
    readonly records: readonly unknown[];
    readonly counts: {
      readonly total: number;
      readonly inPublishView: number;
      readonly published: number;
      readonly loaded: number;
      readonly byPublicationBlocker: Readonly<Record<string, number>>;
    };
  };
  readonly error?: string;
}

test.setTimeout(60_000);

test("renders the chunk lifecycle debug HUD", async ({ page }) => {
  const viewDistance = 1;
  const params = new URLSearchParams({
    worldTransport: "worker",
    worldStorageMode: "none",
    clearWorldStorage: "1",
    preserveInitialCamera: "1",
    showDebugInfo: "1",
    viewDistance: viewDistance.toString(),
    renderDistance: getDefaultRenderDistance(viewDistance).toString(),
    lightingMode: "none",
    liquidSimulationMode: "none",
  });

  await page.goto(`/?mode=debug&${params.toString()}`, { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneDebug !== "undefined", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });
  await page.waitForFunction(
    () => window.__mcloneDebug?.state.ready === true || window.__mcloneDebug?.state.error !== undefined,
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready },
  );
  await page.waitForFunction(
    () => {
      const state = window.__mcloneDebug?.state as ChunkLifecycleHudDebugState | undefined;
      return state?.chunkLifecycle !== undefined
        && state.chunkLifecycle.records.length > 0
        && state.chunkLifecycle.counts.total > 0
        && (state.frameCount ?? 0) >= 3;
    },
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );

  const state = await page.evaluate(() => window.__mcloneDebug!.state as ChunkLifecycleHudDebugState);
  expect(state.error).toBeUndefined();
  expect(state.mode).toBe("world");
  expect(state.chunkLifecycle).toBeDefined();
  expect(state.chunkLifecycle!.currentChunkView).toBeDefined();
  expect(state.chunkLifecycle!.counts.inPublishView).toBeGreaterThan(0);
  expect(state.chunkLifecycle!.counts.published).toBeGreaterThan(0);
  expect(state.chunkLifecycle!.counts.loaded).toBeGreaterThan(0);
  expect(Object.keys(state.chunkLifecycle!.counts.byPublicationBlocker).length).toBeGreaterThan(0);

  await page.locator("#renderer").screenshot({ path: CHUNK_LIFECYCLE_HUD_SCREENSHOT_PATH });
});
