// Debug tooling — see tactical 40. Throwaway when real Player/Input lands.

import { SectionPos } from "../../core/section-pos";
import type { PlayerInputCommand, SetChunkViewRequest } from "../../runtime/protocol/world-messages";
import { Vec3 } from "../../world/phys/vec3";
import {
  applyRenderWorldDirtySections,
  createSceneDepthTarget,
  encodeSceneFrame,
  getSceneLoadedChunkCount,
  getSceneRenderQueueStats,
  getSceneRenderWorldPerformanceCounters,
  initializeRendererScene,
  renderSceneUntilSettled,
  resizeCanvasToDisplaySize,
  waitForLoadedChunkRing,
  type RenderSceneQueueStats,
  type RenderWorldPerformanceCounters,
  type RendererScene,
} from "../scene-setup";
import { getExpectedLoadedChunkCount, readBrowserRenderConfig } from "../browser-render-config";
import type { LoadingProgress } from "../loading-progress";
import { progressFraction } from "../loading-progress";
import { advanceTextureAtlasAnimations } from "../texture/texture-atlas";
import { DebugInput } from "./debug-input";
import {
  applyPredictedCameraInput,
  buildPlayerInputCommand,
  createChunkViewRequestForCameraState,
  createChunkViewRequestForPlayerState,
  isSamePlayerInput,
  mergeDebugInputFrame,
  reconcilePredictedCameraState,
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
  cameraPosition?: readonly [number, number, number];
  cameraYaw?: number;
  cameraPitch?: number;
  playerChunkX?: number;
  playerChunkZ?: number;
  chunkViewCenterX?: number;
  chunkViewCenterZ?: number;
  loadedChunkCount: number;
  expectedLoadedChunkCount?: number;
  viewDistance?: number;
  renderDistance?: number;
  frameCount: number;
  renderWorldCounters?: RenderWorldPerformanceCounters;
  renderQueueStats?: RenderSceneQueueStats;
  loadingStage?: string;
  loadingDetail?: string;
  loadingProgress?: number;
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

function showLoadingProgress(progress: LoadingProgress): void {
  const fraction = progressFraction(progress);
  const detail = progress.detail
    ?? (progress.current !== undefined && progress.total !== undefined ? `${progress.current.toString()} / ${progress.total.toString()}` : undefined);
  showOverlayMessage(detail === undefined ? progress.stage : `${progress.stage}\n${detail}`);

  const progressElement = document.querySelector<HTMLElement>("#debug-progress");
  const progressBar = document.querySelector<HTMLElement>("#debug-progress-bar");
  progressElement?.classList.remove("hidden");
  if (progressBar) {
    progressBar.style.width = `${Math.round((fraction ?? 0) * 100).toString()}%`;
  }
}

function hideLoadingProgress(): void {
  document.querySelector<HTMLElement>("#debug-progress")?.classList.add("hidden");
}

function formatUnknownError(error: unknown): string {
  return error instanceof Error ? `${error.name}: ${error.message}` : String(error);
}

function isSameChunkViewRequest(
  left: SetChunkViewRequest | undefined,
  right: SetChunkViewRequest,
): boolean {
  return left !== undefined
    && left.centerChunkX === right.centerChunkX
    && left.centerChunkZ === right.centerChunkZ
    && left.radius === right.radius;
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
  const reportLoadingProgress = (progress: LoadingProgress): void => {
    debugRuntime.controller.state.loadingStage = progress.stage;
    debugRuntime.controller.state.loadingDetail = progress.detail;
    debugRuntime.controller.state.loadingProgress = progressFraction(progress);
    showLoadingProgress(progress);
  };
  reportLoadingProgress({ stage: "Starting renderer", fraction: 0 });

  const sceneResult = await initializeRendererScene(rendererCanvas, {
    seed: SEED,
    viewDistance: renderConfig.viewDistance,
    renderDistance: renderConfig.renderDistance,
    worldTransport: runtimeConfig.worldTransport,
    remoteWorldHostUrl: runtimeConfig.remoteWorldHostUrl,
    skyColor: renderConfig.skyColor,
    clearColorScale: renderConfig.clearColorScale,
    onProgress: reportLoadingProgress,
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
  let textureAnimationElapsedMs = 0.0;
  let renderInFlight = false;
  let totalFrameCount = 0;
  let fpsFrameCount = 0;
  let lastFpsReportMs = lastFrameMs;
  let nextInputSequence = 1;
  let lastInputCommand: PlayerInputCommand | undefined;
  let queuedPlayerInput: PlayerInputCommand | undefined;
  let playerInputSendInFlight = false;
  let lastChunkViewRequest: SetChunkViewRequest | undefined;

  async function flushPlayerInputQueue(): Promise<void> {
    if (playerInputSendInFlight) {
      return;
    }

    playerInputSendInFlight = true;
    try {
      while (queuedPlayerInput !== undefined) {
        const inputCommand = queuedPlayerInput;
        queuedPlayerInput = undefined;
        await scene.worldClient.setPlayerInput({
          type: "set_player_input",
          input: inputCommand,
        });
      }
    } catch (error) {
      const message = formatUnknownError(error);
      debugRuntime.controller.state.error = message;
      showOverlayMessage(`error: ${message}`);
      // eslint-disable-next-line no-console
      console.error(error);
    } finally {
      playerInputSendInFlight = false;
      if (queuedPlayerInput !== undefined) {
        void flushPlayerInputQueue();
      }
    }
  }

  function queuePlayerInput(inputCommand: PlayerInputCommand): void {
    if (isSamePlayerInput(lastInputCommand, inputCommand)) {
      return;
    }

    lastInputCommand = inputCommand;
    nextInputSequence++;
    queuedPlayerInput = inputCommand;
    void flushPlayerInputQueue();
  }

  // Prime chunks at the start so the first frame has something to draw.
  const initialChunkViewRequest: SetChunkViewRequest = {
    type: "set_chunk_view",
    centerChunkX: SectionPos.posToSectionCoord(initialCamera.position.x),
    centerChunkZ: SectionPos.posToSectionCoord(initialCamera.position.z),
    radius: scene.viewDistance,
  };
  if (await scene.worldClient.setChunkView(initialChunkViewRequest)) {
    applyRenderWorldDirtySections(scene);
    scene.levelRenderer.allChanged();
  }
  lastChunkViewRequest = initialChunkViewRequest;
  const initialInputCommand: PlayerInputCommand = {
    sequence: nextInputSequence++,
    moveX: 0,
    moveY: 0,
    moveZ: 0,
    yaw: initialCamera.yRot,
    pitch: initialCamera.xRot,
  };
  if (await scene.worldClient.setPlayerInput({
    type: "set_player_input",
    input: initialInputCommand,
  })) {
    lastInputCommand = initialInputCommand;
  }
  if (!await waitForLoadedChunkRing(scene, expectedLoadedChunkCount, {
    maxAttempts: 2400,
    onProgress: (progress) => reportLoadingProgress({
      ...progress,
      fraction: 0.92 + ((progressFraction(progress) ?? 0) * 0.06),
    }),
  })) {
    debugRuntime.controller.state.error =
      `expected ${expectedLoadedChunkCount.toString()} loaded chunks for viewDistance=${scene.viewDistance.toString()}, got ${getSceneLoadedChunkCount(scene).toString()}`;
    showOverlayMessage(`error: ${debugRuntime.controller.state.error}`);
    return;
  }

  // Mesh jobs can be requested before every chunk needed by their padded
  // neighborhood is present. Re-run the initial visible set after the ring is
  // complete so screenshots start from a settled frame instead of a partial one.
  reportLoadingProgress({ stage: "Building first frame", fraction: 0.98 });
  scene.levelRenderer.allChanged();
  const initialFrame = await renderSceneUntilSettled(scene, camera);
  const initialEncoder = scene.device.createCommandEncoder();
  encodeSceneFrame(
    scene,
    initialFrame,
    {
      view: scene.ctx.getCurrentTexture().createView(),
      depthView: depthTarget!.view,
      format: scene.format,
    },
    initialEncoder,
  );
  scene.device.queue.submit([initialEncoder.finish()]);
  await scene.device.queue.onSubmittedWorkDone();
  totalFrameCount++;

  debugRuntime.controller.state.ready = true;
  debugRuntime.controller.state.saveId = scene.saveMetadata.saveId;
  debugRuntime.controller.state.loadedChunkCount = getSceneLoadedChunkCount(scene);
  debugRuntime.controller.state.renderWorldCounters = getSceneRenderWorldPerformanceCounters(scene);
  debugRuntime.controller.state.renderQueueStats = getSceneRenderQueueStats(scene);
  debugRuntime.controller.state.viewDistance = scene.viewDistance;
  debugRuntime.controller.state.renderDistance = scene.gameRenderer.getRenderDistance();
  debugRuntime.controller.state.frameCount = totalFrameCount;

  const isTouch = typeof window !== "undefined" && window.matchMedia("(pointer: coarse)").matches;
  hideLoadingProgress();
  showOverlayMessage(
    isTouch
      ? "joystick: look · FWD/BACK: move · ▲/▼: fly"
      : "click to capture mouse — WASD + mouse, Space/Shift for up/down, Esc to release",
  );

  async function tick(): Promise<void> {
    const now = performance.now();
    const frameDeltaMs = Math.max(0.0, now - lastFrameMs);
    const dtSeconds = Math.min(0.1, frameDeltaMs / 1000.0);
    lastFrameMs = now;
    // WebGPU: drive vanilla atlas animation ticks from the browser frame loop.
    textureAnimationElapsedMs = advanceTextureAtlasAnimations(scene.atlas, textureAnimationElapsedMs + frameDeltaMs);
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
      if (!preserveInitialCamera) {
        camera = reconcilePredictedCameraState(camera, playerState, lastInputCommand);
      }
      const baseYaw = preserveInitialCamera ? (lastInputCommand?.yaw ?? playerState.rotation.yaw) : camera.yRot;
      const basePitch = preserveInitialCamera ? (lastInputCommand?.pitch ?? playerState.rotation.pitch) : camera.xRot;
      const playerInput = buildPlayerInputCommand(baseYaw, basePitch, inputFrame, dtSeconds, nextInputSequence);
      if (!preserveInitialCamera) {
        camera = applyPredictedCameraInput(camera, playerInput, dtSeconds);
      }
      queuePlayerInput(playerInput);

      const chunkViewRequest = preserveInitialCamera
        ? createChunkViewRequestForPlayerState(playerState, scene.viewDistance)
        : createChunkViewRequestForCameraState(camera, scene.viewDistance);
      if (!isSameChunkViewRequest(lastChunkViewRequest, chunkViewRequest)) {
        if (await scene.worldClient.setChunkView(chunkViewRequest)) {
          applyRenderWorldDirtySections(scene);
        }
        lastChunkViewRequest = chunkViewRequest;
      }
      const playerChunkViewRequest = createChunkViewRequestForPlayerState(playerState, scene.viewDistance);
      const sessionState = scene.worldClient.getSessionState();
      debugRuntime.controller.state.sessionId = sessionState?.sessionId;
      debugRuntime.controller.state.playerId = sessionState?.playerId;
      debugRuntime.controller.state.playerTick = playerState.tick;
      debugRuntime.controller.state.playerPosition = [playerState.position.x, playerState.position.y, playerState.position.z];
      debugRuntime.controller.state.cameraPosition = [camera.position.x, camera.position.y, camera.position.z];
      debugRuntime.controller.state.cameraYaw = camera.yRot;
      debugRuntime.controller.state.cameraPitch = camera.xRot;
      debugRuntime.controller.state.playerChunkX = playerChunkViewRequest.centerChunkX;
      debugRuntime.controller.state.playerChunkZ = playerChunkViewRequest.centerChunkZ;
      debugRuntime.controller.state.chunkViewCenterX = sessionState?.chunkView?.centerChunkX;
      debugRuntime.controller.state.chunkViewCenterZ = sessionState?.chunkView?.centerChunkZ;
      debugRuntime.controller.state.loadedChunkCount = getSceneLoadedChunkCount(scene);
      debugRuntime.controller.state.renderWorldCounters = getSceneRenderWorldPerformanceCounters(scene);
      debugRuntime.controller.state.renderQueueStats = getSceneRenderQueueStats(scene);
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
        totalFrameCount++;
        fpsFrameCount++;
        debugRuntime.controller.state.frameCount = totalFrameCount;
        debugRuntime.controller.state.renderWorldCounters = getSceneRenderWorldPerformanceCounters(scene);
        debugRuntime.controller.state.renderQueueStats = getSceneRenderQueueStats(scene);
        if (now - lastFpsReportMs >= 1000.0) {
          const fps = (fpsFrameCount * 1000.0) / (now - lastFpsReportMs);
          const playerStateForOverlay = scene.worldClient.getPlayerState();
          if (playerStateForOverlay !== undefined) {
            showOverlayMessage(
              `fps ${fps.toFixed(0)}  pos ${camera.position.x.toFixed(1)}, ${camera.position.y.toFixed(1)}, ${camera.position.z.toFixed(1)}  yaw ${camera.yRot.toFixed(0)}  pitch ${camera.xRot.toFixed(0)}  tick ${playerStateForOverlay.tick.toString()}`,
            );
          }
          fpsFrameCount = 0;
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
