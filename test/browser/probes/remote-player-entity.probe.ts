import { test, expect, type Page } from "../remote-world-host-fixture";
import type { GpuTitleBootResult } from "../../../src/renderer/main";
import { PLAYER_ENTITY_TYPE_ID } from "../../../src/runtime/protocol/world-messages";
import { FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const REMOTE_PLAYER_SCREENSHOT_PATH = "/tmp/mclone-browser-remote-player-entity.png";

interface GpuGuiState {
  readonly mode: "title" | "loading" | "world" | "paused" | "error";
  readonly worldReady?: boolean;
  readonly frameCount: number;
  readonly error?: string;
}

interface RemoteEntityProbeState {
  readonly frameCount: number;
  readonly entityCount: number;
  readonly playerEntityCount: number;
  readonly playerEntities: readonly {
    readonly entityId: number;
    readonly typeId: string;
    readonly position: readonly [number, number, number];
    readonly yaw: number;
    readonly texture?: string;
    readonly name?: string;
  }[];
}

function createLiveRemotePlayerUrl(remoteWorldHostUrl: string): string {
  const params = new URLSearchParams({
    startWorld: "1",
    worldAuthority: "dedicated",
    dedicatedSocketUrl: remoteWorldHostUrl,
    netTransport: "websocket",
    worldStorageMode: "none",
    viewDistance: "2",
    renderDistance: "128",
    lightingMode: "none",
    liquidSimulationMode: "none",
    movementMode: "player",
    preserveInitialCamera: "1",
    cameraX: "8.5",
    cameraY: "178.0",
    cameraZ: "14.5",
    cameraYaw: "180",
    cameraPitch: "58",
  });
  return `/?${params.toString()}`;
}

async function bootLiveWorld(page: Page, url: string): Promise<void> {
  await page.goto(url, { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });
  const titleResult = (await page.evaluate(() => window.__mcloneReady)) as GpuTitleBootResult | { readonly ok: false; readonly reason: string };
  expect(titleResult.ok, JSON.stringify(titleResult)).toBe(true);
  await page.waitForFunction(
    () => window.__mcloneGui?.state.worldReady === true || window.__mcloneGui?.state.mode === "error",
    undefined,
    { timeout: 110_000 },
  );
  const state = await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
  expect(state.mode, state.error).toBe("world");
  await page.waitForFunction(() => (window.__mcloneGui?.state.frameCount ?? 0) >= 3, undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame,
  });
}

async function readRemoteEntityProbeState(page: Page): Promise<RemoteEntityProbeState> {
  return await page.evaluate((playerTypeId) => {
    const controller = window.__mcloneGui!;
    const scene = (controller.worldRuntime as unknown as { options: { scene: {
      clientRuntime: {
        publishPresentationState(): {
          entityPresentation: readonly {
            entityId: number;
            typeId: string;
            interpolatedPosition: { x: number; y: number; z: number };
            interpolatedRotation: { yaw: number };
            data?: Readonly<Record<string, number | boolean | string>>;
          }[];
        };
      };
    } } }).options.scene;
    const entityPresentation = scene.clientRuntime.publishPresentationState().entityPresentation;
    const playerEntities = entityPresentation
      .filter((entity) => entity.typeId === playerTypeId)
      .map((entity) => ({
        entityId: entity.entityId,
        typeId: entity.typeId,
        position: [
          entity.interpolatedPosition.x,
          entity.interpolatedPosition.y,
          entity.interpolatedPosition.z,
        ] as const,
        yaw: entity.interpolatedRotation.yaw,
        texture: typeof entity.data?.texture === "string" ? entity.data.texture : undefined,
        name: typeof entity.data?.name === "string" ? entity.data.name : undefined,
      }));
    return {
      frameCount: controller.state.frameCount,
      entityCount: entityPresentation.length,
      playerEntityCount: playerEntities.length,
      playerEntities,
    };
  }, PLAYER_ENTITY_TYPE_ID);
}

test.setTimeout(150_000);

test("live remote player snapshots render through LevelRenderer entity batches", async ({ browser, remoteWorldHostUrl }) => {
  const context = await browser.newContext();
  const firstPage = await context.newPage();
  const secondPage = await context.newPage();
  const pageErrors: string[] = [];
  const consoleErrors: string[] = [];
  for (const [label, page] of [["first", firstPage], ["second", secondPage]] as const) {
    page.on("pageerror", (error) => pageErrors.push(`${label}: ${String(error)}`));
    page.on("console", (message) => {
      if (message.type() === "error") {
        consoleErrors.push(`${label}: ${message.text()}`);
      }
    });
  }

  const url = createLiveRemotePlayerUrl(remoteWorldHostUrl);
  await Promise.all([
    bootLiveWorld(firstPage, url),
    bootLiveWorld(secondPage, url),
  ]);

  await expect.poll(
    async () => (await readRemoteEntityProbeState(firstPage)).playerEntityCount,
    { timeout: 20_000 },
  ).toBeGreaterThan(0);
  const entityState = await readRemoteEntityProbeState(firstPage);
  await firstPage.waitForFunction(
    (frameCount) => (window.__mcloneGui?.state.frameCount ?? 0) >= frameCount + 3,
    entityState.frameCount,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.frame },
  );
  await firstPage.locator("#renderer").screenshot({ path: REMOTE_PLAYER_SCREENSHOT_PATH });

  expect(entityState.playerEntities, JSON.stringify(entityState)).toHaveLength(1);
  expect(pageErrors, pageErrors.join("\n")).toEqual([]);
  expect(consoleErrors.filter((message) => /webgpu|validation|gpu/i.test(message)), consoleErrors.join("\n")).toEqual([]);

  await context.close();
});
