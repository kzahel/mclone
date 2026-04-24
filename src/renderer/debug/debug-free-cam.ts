// Debug tooling — see tactical 40. Throwaway when real Player/Input lands.

import { SectionPos } from "../../core/section-pos";
import { Vec3 } from "../../world/phys/vec3";
import {
  applyRenderWorldDirtySections,
  createSceneDepthTarget,
  encodeSceneFrame,
  getSceneLoadedChunkCount,
  initializeRendererScene,
  resizeCanvasToDisplaySize,
  type RendererScene,
} from "../scene-setup";
import { getExpectedLoadedChunkCount, readBrowserRenderConfig } from "../browser-render-config";
import { DebugInput } from "./debug-input";
import {
  buildPlayerInputCommand,
  createCameraStateFromPlayerState,
  createChunkViewRequestForPlayerState,
  isSamePlayerInput,
  mergeDebugInputFrame,
  type DebugInjectedInput,
} from "./debug-player-controls";

const SEED = 12_345n;
const LIGHT_TICK_INTERVAL_MS = 1000.0;
const WORLD_POLL_INTERVAL_MS = 50.0;

const DEFAULT_INITIAL_POSITION = new Vec3(8.5, 104.0, 40.5);
const DEFAULT_INITIAL_X_ROT = 30.0;
const DEFAULT_INITIAL_Y_ROT = 180.0;

interface DebugRuntimeState {
  ready: boolean;
  readonly worldTransport: "worker" | "remote";
  saveId?: string;
  sessionId?: string;
  playerId?: string;
  playerTick?: number;
  playerPosition?: readonly [number, number, number];
  playerChunkX?: number;
  playerChunkZ?: number;
  chunkViewCenterX?: number;
  chunkViewCenterZ?: number;
  loadedChunkCount: number;
  expectedLoadedChunkCount?: number;
  viewDistance?: number;
  renderDistance?: number;
  frameCount: number;
  error?: string;
}

interface DebugRuntimeController {
  readonly state: DebugRuntimeState;
  setInjectedInput(input: DebugInjectedInput | null): void;
}

declare global {
  interface Window {
    __mcloneDebug?: DebugRuntimeController;
  }
}

function readWorldTransport(): {
  readonly worldTransport: "worker" | "remote";
  readonly remoteWorldHostUrl?: string;
} {
  const url = new URL(window.location.href);
  if (url.searchParams.get("worldTransport") === "remote") {
    return {
      worldTransport: "remote",
      remoteWorldHostUrl: url.searchParams.get("worldHostUrl") ?? undefined,
    };
  }

  return { worldTransport: "worker" };
}

function readInitialCamera(): {
  readonly position: Vec3;
  readonly xRot: number;
  readonly yRot: number;
} {
  const url = new URL(window.location.href);
  const x = Number.parseFloat(url.searchParams.get("cameraX") ?? "");
  const y = Number.parseFloat(url.searchParams.get("cameraY") ?? "");
  const z = Number.parseFloat(url.searchParams.get("cameraZ") ?? "");
  const xRot = Number.parseFloat(url.searchParams.get("cameraPitch") ?? "");
  const yRot = Number.parseFloat(url.searchParams.get("cameraYaw") ?? "");

  return {
    position: new Vec3(
      Number.isFinite(x) ? x : DEFAULT_INITIAL_POSITION.x,
      Number.isFinite(y) ? y : DEFAULT_INITIAL_POSITION.y,
      Number.isFinite(z) ? z : DEFAULT_INITIAL_POSITION.z,
    ),
    xRot: Number.isFinite(xRot) ? xRot : DEFAULT_INITIAL_X_ROT,
    yRot: Number.isFinite(yRot) ? yRot : DEFAULT_INITIAL_Y_ROT,
  };
}

function readPreserveInitialCamera(): boolean {
  const url = new URL(window.location.href);
  const value = url.searchParams.get("preserveInitialCamera");
  return value === "1" || value === "true";
}

function createDebugRuntimeController(
  worldTransport: "worker" | "remote",
): {
  readonly controller: DebugRuntimeController;
  readonly getInjectedInput: () => DebugInjectedInput | null;
} {
  let injectedInput: DebugInjectedInput | null = null;
  const state: DebugRuntimeState = {
    ready: false,
    worldTransport,
    loadedChunkCount: 0,
    frameCount: 0,
  };
  return {
    controller: {
      state,
      setInjectedInput(input: DebugInjectedInput | null): void {
        injectedInput = input;
      },
    },
    getInjectedInput(): DebugInjectedInput | null {
      return injectedInput;
    },
  };
}

function showOverlayMessage(message: string): void {
  const overlay = document.querySelector<HTMLElement>("#debug-overlay");
  if (overlay) overlay.textContent = message;
}

async function waitForLoadedChunkRing(scene: RendererScene, expectedLoadedChunkCount: number): Promise<boolean> {
  if (getSceneLoadedChunkCount(scene) >= expectedLoadedChunkCount) {
    return true;
  }

  for (let attempt = 0; attempt < 40; attempt++) {
    await new Promise((resolve) => setTimeout(resolve, WORLD_POLL_INTERVAL_MS));
    if (await scene.worldClient.pollUpdates()) {
      applyRenderWorldDirtySections(scene);
    }
    if (getSceneLoadedChunkCount(scene) >= expectedLoadedChunkCount) {
      return true;
    }
  }

  return getSceneLoadedChunkCount(scene) >= expectedLoadedChunkCount;
}

async function boot(): Promise<void> {
  const canvas = document.querySelector<HTMLCanvasElement>("#renderer");
  if (!canvas) {
    showOverlayMessage("error: canvas #renderer not found");
    return;
  }
  const rendererCanvas = canvas;

  const runtimeConfig = readWorldTransport();
  const renderConfig = readBrowserRenderConfig(new URL(window.location.href));
  const initialCamera = readInitialCamera();
  const preserveInitialCamera = readPreserveInitialCamera();
  const debugRuntime = createDebugRuntimeController(runtimeConfig.worldTransport);
  window.__mcloneDebug = debugRuntime.controller;

  const sceneResult = await initializeRendererScene(rendererCanvas, {
    seed: SEED,
    viewDistance: renderConfig.viewDistance,
    renderDistance: renderConfig.renderDistance,
    worldTransport: runtimeConfig.worldTransport,
    remoteWorldHostUrl: runtimeConfig.remoteWorldHostUrl,
    skyColor: renderConfig.skyColor,
    clearColorScale: renderConfig.clearColorScale,
  });
  if (!sceneResult.ok) {
    debugRuntime.controller.state.error = sceneResult.reason;
    showOverlayMessage(`error: ${sceneResult.reason}`);
    return;
  }
  const scene: RendererScene = sceneResult.scene;
  const expectedLoadedChunkCount = getExpectedLoadedChunkCount(scene.viewDistance);
  debugRuntime.controller.state.expectedLoadedChunkCount = expectedLoadedChunkCount;

  const input = new DebugInput(rendererCanvas);
  let depthTarget: ReturnType<typeof createSceneDepthTarget> | undefined;

  function resizeViewport(): void {
    const resized = resizeCanvasToDisplaySize(rendererCanvas, scene.device.limits.maxTextureDimension2D);
    if (!resized.changed && depthTarget !== undefined) {
      return;
    }

    depthTarget?.texture.destroy();
    depthTarget = createSceneDepthTarget(scene.device, resized.width, resized.height);
    scene.gameRenderer.resize(resized.width, resized.height);
  }

  resizeViewport();

  let camera = {
    position: initialCamera.position,
    xRot: initialCamera.xRot,
    yRot: initialCamera.yRot,
  };
  let lastFrameMs = performance.now();
  let lastLightTickMs = lastFrameMs;
  let lastWorldPollMs = lastFrameMs;
  let renderInFlight = false;
  let frameCount = 0;
  let lastFpsReportMs = lastFrameMs;
  let nextInputSequence = 1;
  let lastSentInput: import("../../runtime/protocol/world-messages").PlayerInputCommand | undefined;

  // Prime chunks at the start so the first frame has something to draw.
  if (await scene.worldClient.setChunkView({
    type: "set_chunk_view",
    centerChunkX: SectionPos.posToSectionCoord(initialCamera.position.x),
    centerChunkZ: SectionPos.posToSectionCoord(initialCamera.position.z),
    radius: scene.viewDistance,
  })) {
    applyRenderWorldDirtySections(scene);
    scene.levelRenderer.allChanged();
  }
  if (await scene.worldClient.setPlayerInput({
    type: "set_player_input",
    input: {
      sequence: nextInputSequence++,
      moveX: 0,
      moveY: 0,
      moveZ: 0,
      yaw: initialCamera.yRot,
      pitch: initialCamera.xRot,
    },
  })) {
    lastSentInput = {
      sequence: nextInputSequence - 1,
      moveX: 0,
      moveY: 0,
      moveZ: 0,
      yaw: initialCamera.yRot,
      pitch: initialCamera.xRot,
    };
  }
  if (!await waitForLoadedChunkRing(scene, expectedLoadedChunkCount)) {
    debugRuntime.controller.state.error =
      `expected ${expectedLoadedChunkCount.toString()} loaded chunks for viewDistance=${scene.viewDistance.toString()}, got ${getSceneLoadedChunkCount(scene).toString()}`;
    showOverlayMessage(`error: ${debugRuntime.controller.state.error}`);
    return;
  }
  debugRuntime.controller.state.ready = true;
  debugRuntime.controller.state.saveId = scene.saveMetadata.saveId;
  debugRuntime.controller.state.loadedChunkCount = getSceneLoadedChunkCount(scene);
  debugRuntime.controller.state.viewDistance = scene.viewDistance;
  debugRuntime.controller.state.renderDistance = scene.gameRenderer.getRenderDistance();

  const isTouch = typeof window !== "undefined" && window.matchMedia("(pointer: coarse)").matches;
  showOverlayMessage(
    isTouch
      ? "joystick: look · FWD/BACK: move · ▲/▼: fly"
      : "click to capture mouse — WASD + mouse, Space/Shift for up/down, Esc to release",
  );

  async function tick(): Promise<void> {
    const now = performance.now();
    const dtSeconds = Math.min(0.1, (now - lastFrameMs) / 1000.0);
    lastFrameMs = now;
    resizeViewport();

    const inputFrame = mergeDebugInputFrame(input.consumeFrame(), debugRuntime.getInjectedInput());

    if (now - lastWorldPollMs >= WORLD_POLL_INTERVAL_MS) {
      if (await scene.worldClient.pollUpdates()) {
        applyRenderWorldDirtySections(scene);
      }
      lastWorldPollMs = now;
    }

    if (now - lastLightTickMs >= LIGHT_TICK_INTERVAL_MS) {
      scene.lightTexture.tick();
      lastLightTickMs = now;
    }

    const playerState = scene.worldClient.getPlayerState();
    if (playerState !== undefined) {
      // Client-side rotation prediction: accumulate from our last-sent yaw/pitch,
      // not the server-echoed state (which lags behind). Otherwise input between
      // server echoes gets overwritten and higher framerates lose more rotation.
      const baseYaw = lastSentInput?.yaw ?? playerState.rotation.yaw;
      const basePitch = lastSentInput?.pitch ?? playerState.rotation.pitch;
      const playerInput = buildPlayerInputCommand(baseYaw, basePitch, inputFrame, dtSeconds, nextInputSequence);
      if (!isSamePlayerInput(lastSentInput, playerInput)) {
        if (await scene.worldClient.setPlayerInput({
          type: "set_player_input",
          input: playerInput,
        })) {
          lastSentInput = playerInput;
          nextInputSequence++;
        }
      }

      if (!preserveInitialCamera) {
        camera = createCameraStateFromPlayerState(playerState);
      }
      const chunkViewRequest = createChunkViewRequestForPlayerState(playerState, scene.viewDistance);
      if (await scene.worldClient.setChunkView(chunkViewRequest)) {
        applyRenderWorldDirtySections(scene);
      }
      const sessionState = scene.worldClient.getSessionState();
      debugRuntime.controller.state.sessionId = sessionState?.sessionId;
      debugRuntime.controller.state.playerId = sessionState?.playerId;
      debugRuntime.controller.state.playerTick = playerState.tick;
      debugRuntime.controller.state.playerPosition = [playerState.position.x, playerState.position.y, playerState.position.z];
      debugRuntime.controller.state.playerChunkX = chunkViewRequest.centerChunkX;
      debugRuntime.controller.state.playerChunkZ = chunkViewRequest.centerChunkZ;
      debugRuntime.controller.state.chunkViewCenterX = sessionState?.chunkView?.centerChunkX;
      debugRuntime.controller.state.chunkViewCenterZ = sessionState?.chunkView?.centerChunkZ;
      debugRuntime.controller.state.loadedChunkCount = getSceneLoadedChunkCount(scene);
    }

    if (!renderInFlight) {
      renderInFlight = true;
      try {
        const renderedFrame = await scene.gameRenderer.renderLevel(
          0.0,
          Number.MAX_SAFE_INTEGER,
          scene.levelRenderer,
          scene.lightTexture,
          camera,
          { waitForChunkTasks: false },
        );
        const encoder = scene.device.createCommandEncoder();
        encodeSceneFrame(
          scene,
          renderedFrame,
          {
            view: scene.ctx.getCurrentTexture().createView(),
            depthView: depthTarget!.view,
            format: scene.format,
          },
          encoder,
        );
        scene.device.queue.submit([encoder.finish()]);
        frameCount++;
        debugRuntime.controller.state.frameCount = frameCount;
        if (now - lastFpsReportMs >= 1000.0) {
          const fps = (frameCount * 1000.0) / (now - lastFpsReportMs);
          const playerStateForOverlay = scene.worldClient.getPlayerState();
          if (playerStateForOverlay !== undefined) {
            showOverlayMessage(
              `fps ${fps.toFixed(0)}  pos ${playerStateForOverlay.position.x.toFixed(1)}, ${playerStateForOverlay.position.y.toFixed(1)}, ${playerStateForOverlay.position.z.toFixed(1)}  yaw ${playerStateForOverlay.rotation.yaw.toFixed(0)}  pitch ${playerStateForOverlay.rotation.pitch.toFixed(0)}  tick ${playerStateForOverlay.tick.toString()}`,
            );
          }
          frameCount = 0;
          lastFpsReportMs = now;
        }
      } finally {
        renderInFlight = false;
      }
    }

    requestAnimationFrame(() => {
      void tick();
    });
  }

  requestAnimationFrame(() => {
    void tick();
  });
}

if (typeof window !== "undefined") {
  boot().catch((err: unknown) => {
    const message = err instanceof Error ? `${err.name}: ${err.message}` : String(err);
    if (window.__mcloneDebug) {
      window.__mcloneDebug.state.error = message;
    }
    showOverlayMessage(`error: ${message}`);
    // eslint-disable-next-line no-console
    console.error(err);
  });
  window.addEventListener("error", (ev) => {
    if (window.__mcloneDebug) {
      window.__mcloneDebug.state.error = ev.message;
    }
    showOverlayMessage(`error: ${ev.message}`);
  });
  window.addEventListener("unhandledrejection", (ev) => {
    const reason = ev.reason instanceof Error ? ev.reason.message : String(ev.reason);
    if (window.__mcloneDebug) {
      window.__mcloneDebug.state.error = reason;
    }
    showOverlayMessage(`error: ${reason}`);
  });
}
