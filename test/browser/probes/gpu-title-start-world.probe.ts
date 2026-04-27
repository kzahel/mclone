import { expect, test } from "@playwright/test";
import type { BootResult, GpuTitleBootResult } from "../../../src/renderer/main";
import { createGeneratedWorldSmokeSearchParams, validateGeneratedWorldSmokeResult } from "../../../src/renderer/generated-world-smoke-scenario";
import { FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const GPU_TITLE_LOADING_SCREENSHOT_PATH = "/tmp/mclone-gpu-title-loading.png";
const GPU_TITLE_WORLD_SCREENSHOT_PATH = "/tmp/mclone-gpu-title-world.png";
const GPU_TITLE_PAUSE_SCREENSHOT_PATH = "/tmp/mclone-gpu-title-pause.png";
const GPU_TITLE_PAUSE_OPTIONS_SCREENSHOT_PATH = "/tmp/mclone-gpu-title-pause-options.png";
const GPU_TITLE_PAUSE_DEBUG_SETTINGS_SCREENSHOT_PATH = "/tmp/mclone-gpu-title-pause-debug-settings.png";
const GPU_TITLE_RETURNED_SCREENSHOT_PATH = "/tmp/mclone-gpu-title-returned-title.png";
const GPU_TITLE_RESTARTED_WORLD_SCREENSHOT_PATH = "/tmp/mclone-gpu-title-restarted-world.png";

interface GpuGuiState {
  readonly ready: boolean;
  readonly mode: "title" | "loading" | "world" | "paused" | "error";
  readonly screenTitle: string;
  readonly lastAction?: string;
  readonly loadingStage?: string;
  readonly loadingProgress?: number;
  readonly worldReady?: boolean;
  readonly worldResult?: BootResult;
  readonly pauseScreenActive?: boolean;
  readonly frameCount: number;
  readonly inputEventCount: number;
  readonly cameraPosition?: readonly [number, number, number];
  readonly cameraYaw?: number;
  readonly cameraPitch?: number;
  readonly options?: {
    readonly viewDistance: number;
    readonly renderDistance: number;
    readonly lightingMode: string;
    readonly liquidSimulationMode: string;
  };
  readonly debugSettings?: {
    readonly movementMode: string;
    readonly preset: string;
    readonly worldStorageMode: string;
  };
  readonly error?: string;
  readonly width: number;
  readonly height: number;
}

test.setTimeout(120_000);

test("starts the generated world from the GPU title screen without DOM controls", async ({ page }) => {
  const params = createGeneratedWorldSmokeSearchParams({
    worldTransport: "worker",
    worldStorageMode: "none",
    gpuTitle: "1",
  });

  await page.goto(`/smoke.html?${params.toString()}`, { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });
  const titleResult = (await page.evaluate(() => window.__mcloneReady)) as GpuTitleBootResult | { readonly ok: false; readonly reason: string };
  expect(titleResult.ok, JSON.stringify(titleResult)).toBe(true);
  expect(titleResult.ok && "mode" in titleResult ? titleResult.mode : undefined).toBe("gpu_title");
  await page.waitForFunction(() => window.__mcloneGui?.state.ready === true, undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });

  await expect(page.locator("button, input, select, textarea")).toHaveCount(0);
  const box = await page.locator("#renderer").boundingBox();
  expect(box).not.toBeNull();
  const titleState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  const startWorldCenterY = (Math.floor(titleState.height / 4) + 48 + 10) / titleState.height;
  await page.mouse.click(box!.x + (box!.width / 2), box!.y + (box!.height * startWorldCenterY));

  await page.waitForFunction(
    () => window.__mcloneGui?.state.mode !== "title",
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready },
  );
  await page.locator("#renderer").screenshot({ path: GPU_TITLE_LOADING_SCREENSHOT_PATH });

  await page.waitForFunction(
    () => window.__mcloneGui?.state.worldReady === true || window.__mcloneGui?.state.mode === "error",
    undefined,
    { timeout: 100_000 },
  );
  const state = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(state.mode, state.error).toBe("world");
  expect(state.lastAction).toBe("start_world");
  expect(state.worldResult?.ok, JSON.stringify(state.worldResult)).toBe(true);
  expect(validateGeneratedWorldSmokeResult(state.worldResult as Extract<BootResult, { ok: true }>, {
    expectedWorldTransport: "worker",
    requirePlayerInput: true,
    requireSteps: true,
  })).toEqual([]);

  await page.waitForFunction(
    () => (window.__mcloneGui?.state.frameCount ?? 0) >= 3,
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );

  await expect(page.locator("button, input, select, textarea")).toHaveCount(0);
  await page.locator("#renderer").screenshot({ path: GPU_TITLE_WORLD_SCREENSHOT_PATH });

  const liveState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  const liveBox = await page.locator("#renderer").boundingBox();
  expect(liveBox).not.toBeNull();
  await page.mouse.click(liveBox!.x + (liveBox!.width / 2), liveBox!.y + (liveBox!.height / 2));
  await page.waitForFunction(
    () => document.pointerLockElement === document.querySelector("#renderer"),
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  await page.keyboard.down("w");
  await page.mouse.move(liveBox!.x + (liveBox!.width / 2) + 24, liveBox!.y + (liveBox!.height / 2) + 6, { steps: 3 });
  await page.waitForFunction(
    ({ frameCount, inputEventCount }) => {
      const state = window.__mcloneGui?.state;
      return state !== undefined
        && state.frameCount >= frameCount + 3
        && state.inputEventCount > inputEventCount;
    },
    { frameCount: liveState.frameCount, inputEventCount: liveState.inputEventCount },
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  await page.keyboard.up("w");

  const movedState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(movedState.frameCount).toBeGreaterThan(liveState.frameCount);
  expect(movedState.inputEventCount).toBeGreaterThan(liveState.inputEventCount);
  expect(movedState.cameraPosition).toBeDefined();
  expect(movedState.cameraPosition).not.toEqual(liveState.cameraPosition);

  await page.keyboard.press("Escape");
  await page.waitForFunction(
    () => window.__mcloneGui?.state.mode === "paused",
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  await expect(page.locator("button, input, select, textarea")).toHaveCount(0);
  await page.locator("#renderer").screenshot({ path: GPU_TITLE_PAUSE_SCREENSHOT_PATH });

  const pausedState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(pausedState.pauseScreenActive).toBe(true);
  expect(pausedState.screenTitle).toBe("menu.game");
  expect(pausedState.cameraPosition).toBeDefined();

  const pauseOptionsCenterX = (Math.floor(pausedState.width / 2) - 53) / pausedState.width;
  const pauseOptionsCenterY = (Math.floor(pausedState.height / 4) + 96 - 16 + 10) / pausedState.height;
  await page.mouse.click(liveBox!.x + (liveBox!.width * pauseOptionsCenterX), liveBox!.y + (liveBox!.height * pauseOptionsCenterY));
  await page.waitForFunction(
    () => window.__mcloneGui?.state.screenTitle === "options.title",
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  const optionsState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(optionsState.mode).toBe("paused");
  expect(optionsState.lastAction).toBe("options");
  expect(optionsState.options?.viewDistance).toBeGreaterThan(0);
  await page.locator("#renderer").screenshot({ path: GPU_TITLE_PAUSE_OPTIONS_SCREENSHOT_PATH });

  const optionsDoneCenterY = (Math.min(optionsState.height - 28, (Math.floor(optionsState.height / 6) - 12) + (24 * 4) + 12) + 10) / optionsState.height;
  await page.mouse.click(liveBox!.x + (liveBox!.width / 2), liveBox!.y + (liveBox!.height * optionsDoneCenterY));
  await page.waitForFunction(
    () => window.__mcloneGui?.state.screenTitle === "menu.game",
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  const pauseBaselineState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(pauseBaselineState.lastAction).toBe("options_done");

  const pauseDebugSettingsCenterX = (Math.floor(pauseBaselineState.width / 2) + 53) / pauseBaselineState.width;
  const pauseDebugSettingsCenterY = (Math.floor(pauseBaselineState.height / 4) + 96 - 16 + 10) / pauseBaselineState.height;
  await page.mouse.click(liveBox!.x + (liveBox!.width * pauseDebugSettingsCenterX), liveBox!.y + (liveBox!.height * pauseDebugSettingsCenterY));
  await page.waitForFunction(
    () => window.__mcloneGui?.state.screenTitle === "debug.settings.title",
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  const debugSettingsState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(debugSettingsState.mode).toBe("paused");
  expect(debugSettingsState.lastAction).toBe("debug_settings");
  expect(debugSettingsState.debugSettings?.preset).toBeDefined();
  await page.locator("#renderer").screenshot({ path: GPU_TITLE_PAUSE_DEBUG_SETTINGS_SCREENSHOT_PATH });

  const debugDoneCenterY = (Math.min(debugSettingsState.height - 28, (Math.floor(debugSettingsState.height / 6) - 12) + (24 * 4) + 12) + 10) / debugSettingsState.height;
  await page.mouse.click(liveBox!.x + (liveBox!.width / 2), liveBox!.y + (liveBox!.height * debugDoneCenterY));
  await page.waitForFunction(
    () => window.__mcloneGui?.state.screenTitle === "menu.game",
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  const debugReturnedPauseState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(debugReturnedPauseState.lastAction).toBe("debug_settings_done");

  await page.keyboard.down("w");
  await page.mouse.move(liveBox!.x + (liveBox!.width / 2) + 48, liveBox!.y + (liveBox!.height / 2) + 12, { steps: 3 });
  await page.waitForFunction(
    ({ frameCount }) => {
      const state = window.__mcloneGui?.state;
      return state !== undefined
        && state.mode === "paused"
        && state.frameCount >= frameCount + 3;
    },
    { frameCount: debugReturnedPauseState.frameCount },
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  await page.keyboard.up("w");
  const pausedInputState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(pausedInputState.cameraPosition).toEqual(debugReturnedPauseState.cameraPosition);

  const backToGameCenterY = (Math.floor(pausedInputState.height / 4) + 24 - 16 + 10) / pausedInputState.height;
  await page.mouse.click(liveBox!.x + (liveBox!.width / 2), liveBox!.y + (liveBox!.height * backToGameCenterY));
  await page.waitForFunction(
    () => window.__mcloneGui?.state.mode === "world" && window.__mcloneGui?.state.pauseScreenActive === false,
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );

  const resumedState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  await page.mouse.click(liveBox!.x + (liveBox!.width / 2), liveBox!.y + (liveBox!.height / 2));
  await page.waitForFunction(
    () => document.pointerLockElement === document.querySelector("#renderer"),
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  await page.keyboard.down("w");
  await page.waitForFunction(
    ({ frameCount, cameraPosition }) => {
      const state = window.__mcloneGui?.state;
      return state !== undefined
        && state.mode === "world"
        && state.frameCount >= frameCount + 3
        && JSON.stringify(state.cameraPosition) !== JSON.stringify(cameraPosition);
    },
    { frameCount: resumedState.frameCount, cameraPosition: resumedState.cameraPosition },
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  await page.keyboard.up("w");

  await page.keyboard.press("Escape");
  await page.waitForFunction(
    () => window.__mcloneGui?.state.mode === "paused",
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  const quitPauseState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  const disconnectCenterY = (Math.floor(quitPauseState.height / 4) + 120 - 16 + 10) / quitPauseState.height;
  await page.mouse.click(liveBox!.x + (liveBox!.width / 2), liveBox!.y + (liveBox!.height * disconnectCenterY));
  await page.waitForFunction(
    () => window.__mcloneGui?.state.mode === "title" && window.__mcloneGui?.state.screenTitle === "Title Screen",
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready },
  );
  const returnedTitleState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(returnedTitleState.lastAction).toBe("disconnect");
  await page.locator("#renderer").screenshot({ path: GPU_TITLE_RETURNED_SCREENSHOT_PATH });

  const restartStartWorldCenterY = (Math.floor(returnedTitleState.height / 4) + 48 + 10) / returnedTitleState.height;
  await page.mouse.click(liveBox!.x + (liveBox!.width / 2), liveBox!.y + (liveBox!.height * restartStartWorldCenterY));
  await page.waitForFunction(
    () => window.__mcloneGui?.state.worldReady === true || window.__mcloneGui?.state.mode === "error",
    undefined,
    { timeout: 100_000 },
  );
  await page.waitForFunction(
    () => (window.__mcloneGui?.state.frameCount ?? 0) >= 3,
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  const restartedWorldState = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(restartedWorldState.mode, restartedWorldState.error).toBe("world");
  await page.locator("#renderer").screenshot({ path: GPU_TITLE_RESTARTED_WORLD_SCREENSHOT_PATH });
});
