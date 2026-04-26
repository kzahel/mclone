// Debug tooling — see tactical 40. Throwaway when real Player/Input lands.

import { SectionPos } from "../../core/section-pos";
import { DEFAULT_MOVEMENT_PHYSICS, MovementCommandClock } from "../../runtime/movement";
import type { PlayerInputCommand, SetChunkViewRequest, WorldPerformanceSnapshot } from "../../runtime/protocol/world-messages";
import {
  PLAYER_COLLISION_REVISION,
  PLAYER_COMMAND_QUANTUM_US,
  PLAYER_MOVEMENT_PHYSICS_REVISION,
  playerInputToQueuedMoveCommand,
} from "../../runtime/session/player-loop";
import { deleteIndexedDbWorldStorage } from "../../runtime/storage/indexeddb-world-storage";
import { Vec3 } from "../../world/phys/vec3";
import { AlertScreen } from "../../client/gui/screens/alert-screen";
import { DebugSettingsScreen, type GuiDebugSettingsState } from "../../client/gui/screens/debug-settings-screen";
import { OptionsScreen, type GuiOptionsState } from "../../client/gui/screens/options-screen";
import { PauseScreen } from "../../client/gui/screens/pause-screen";
import { ProgressScreen } from "../../client/gui/screens/progress-screen";
import { GuiComponent } from "../../client/gui/gui-component";
import { ScreenManager } from "../../client/gui/screen-manager";
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
import {
  getExpectedLoadedChunkCount,
  readBrowserRenderConfig,
  writeStoredBrowserRenderConfig,
  type BrowserRenderConfig,
} from "../browser-render-config";
import { GuiOverlayHost } from "../gui/gui-overlay-host";
import { GuiRenderer } from "../gui/gui-renderer";
import type { LoadingProgress } from "../loading-progress";
import { progressFraction } from "../loading-progress";
import { BROWSER_RENDERER_HOST, type BrowserRendererHost } from "../renderer-host";
import { advanceTextureAtlasAnimations } from "../texture/texture-atlas";
import type { BrowserCanvasTarget, WebGpuDeviceContext } from "../webgpu-target";
import { DebugInput } from "./debug-input";
import {
  readDebugSessionConfig,
  writeStartLastWorld,
  writeStoredDebugSessionConfig,
  type DebugMovementMode,
  type DebugSessionConfig,
} from "./debug-session-config";
import {
  applyPredictedCameraInput,
  buildFreeCameraInputCommand,
  buildPlayerInputCommand,
  createCameraStateFromMovementBody,
  createChunkViewRequestForCameraState,
  createChunkViewRequestForPlayerState,
  getDebugPlayerButtonMask,
  isSamePlayerInput,
  mergeDebugInputFrame,
  type DebugCameraState,
  type DebugInjectedInput,
} from "./debug-player-controls";

const LIGHT_TICK_INTERVAL_MS = 1000.0;
const WORLD_POLL_INTERVAL_MS = 50.0;

const DEFAULT_INITIAL_POSITION = new Vec3(8.5, 104.0, 40.5);
const DEFAULT_INITIAL_X_ROT = 30.0;
const DEFAULT_INITIAL_Y_ROT = 180.0;

let showGlobalErrorScreen: ((message: string) => void) | undefined;

interface DebugRuntimeState {
  ready: boolean;
  readonly worldTransport: "worker" | "remote";
  mode?: "loading" | "world" | "paused" | "error";
  screenTitle?: string;
  lastAction?: string;
  pauseScreenActive?: boolean;
  seed: string;
  movementMode: DebugMovementMode;
  preset: DebugSessionConfig["preset"];
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
  width?: number;
  height?: number;
  viewDistance?: number;
  renderDistance?: number;
  lightingMode?: BrowserRenderConfig["lightingMode"];
  liquidSimulationMode?: BrowserRenderConfig["liquidSimulationMode"];
  worldStorageMode?: BrowserRenderConfig["worldStorageMode"];
  options?: GuiOptionsState;
  debugSettings?: GuiDebugSettingsState;
  frameCount: number;
  renderWorldCounters?: RenderWorldPerformanceCounters;
  renderQueueStats?: RenderSceneQueueStats;
  worldPerformance?: WorldPerformanceSnapshot;
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

function readClearWorldStorage(): boolean {
  const url = new URL(window.location.href);
  const value = url.searchParams.get("clearWorldStorage");
  return value === "1" || value === "true";
}

function createDebugRuntimeController(
  worldTransport: "worker" | "remote",
  sessionConfig: DebugSessionConfig,
): {
  readonly controller: DebugRuntimeController;
  readonly getInjectedInput: () => DebugInjectedInput | null;
} {
  let injectedInput: DebugInjectedInput | null = null;
  const state: DebugRuntimeState = {
    ready: false,
    mode: "loading",
    screenTitle: "narrator.screen.progress",
    pauseScreenActive: false,
    worldTransport,
    seed: sessionConfig.seed.toString(),
    movementMode: sessionConfig.movementMode,
    preset: sessionConfig.preset,
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

function clearTransientWorldStorageQueryParam(): void {
  const url = new URL(window.location.href);
  if (!url.searchParams.has("clearWorldStorage")) {
    return;
  }

  url.searchParams.delete("clearWorldStorage");
  window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
}

function createGuiOptionsState(config: BrowserRenderConfig): GuiOptionsState {
  return {
    viewDistance: config.viewDistance,
    renderDistance: config.renderDistance,
    lightingMode: config.lightingMode,
    liquidSimulationMode: config.liquidSimulationMode,
  };
}

function createGuiDebugSettingsState(config: BrowserRenderConfig, sessionConfig: DebugSessionConfig): GuiDebugSettingsState {
  return {
    movementMode: sessionConfig.movementMode,
    preset: sessionConfig.preset,
    worldStorageMode: config.worldStorageMode,
  };
}

function copyGuiOptionsState(options: GuiOptionsState): GuiOptionsState {
  return {
    viewDistance: options.viewDistance,
    renderDistance: options.renderDistance,
    lightingMode: options.lightingMode,
    liquidSimulationMode: options.liquidSimulationMode,
  };
}

function copyGuiDebugSettingsState(settings: GuiDebugSettingsState): GuiDebugSettingsState {
  return {
    movementMode: settings.movementMode,
    preset: settings.preset,
    worldStorageMode: settings.worldStorageMode,
  };
}

function createPinnedBrowserRendererHost(
  deviceContext: WebGpuDeviceContext,
  target: BrowserCanvasTarget,
): BrowserRendererHost {
  return {
    requestWebGpuDeviceContext: async (progress) => {
      progress?.onRequestAdapter?.();
      progress?.onRequestDevice?.();
      return { ok: true, context: deviceContext };
    },
    createCanvasTarget: () => target,
    createRenderWorldWorkerEndpoint: () => BROWSER_RENDERER_HOST.createRenderWorldWorkerEndpoint(),
  };
}

function emptyDebugInputFrame(): ReturnType<DebugInput["consumeFrame"]> {
  return {
    heldKeys: new Set(),
    mouseDeltaX: 0,
    mouseDeltaY: 0,
    locked: false,
    joystickX: 0,
    joystickY: 0,
    moveForward: false,
    moveBack: false,
    flyUp: false,
    flyDown: false,
  };
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

function applyFreeCameraInput(
  camera: DebugCameraState,
  inputFrame: ReturnType<DebugInput["consumeFrame"]>,
  dtSeconds: number,
): DebugCameraState {
  const inputCommand = buildFreeCameraInputCommand(camera.yRot, camera.xRot, inputFrame, dtSeconds, 0);
  return applyPredictedCameraInput(camera, inputCommand, dtSeconds);
}

async function boot(): Promise<void> {
  const canvas = document.querySelector<HTMLCanvasElement>("#renderer");
  if (!canvas) {
    // eslint-disable-next-line no-console
    console.error("error: canvas #renderer not found");
    return;
  }
  const rendererCanvas = canvas;

  const runtimeConfig = readWorldTransport();
  const url = new URL(window.location.href);
  const renderConfig = readBrowserRenderConfig(url, window.localStorage);
  const sessionConfig = readDebugSessionConfig(url, window.localStorage);
  const optionsState = createGuiOptionsState(renderConfig);
  const debugSettingsState = createGuiDebugSettingsState(renderConfig, sessionConfig);
  const initialCamera = readInitialCamera();
  const preserveInitialCamera = readPreserveInitialCamera();
  const debugRuntime = createDebugRuntimeController(runtimeConfig.worldTransport, sessionConfig);
  debugRuntime.controller.state.options = copyGuiOptionsState(optionsState);
  debugRuntime.controller.state.debugSettings = copyGuiDebugSettingsState(debugSettingsState);
  window.__mcloneDebug = debugRuntime.controller;

  const deviceContextResult = await BROWSER_RENDERER_HOST.requestWebGpuDeviceContext();
  if (!deviceContextResult.ok) {
    debugRuntime.controller.state.mode = "error";
    debugRuntime.controller.state.error = deviceContextResult.reason;
    return;
  }
  const { device, format } = deviceContextResult.context;
  resizeCanvasToDisplaySize(rendererCanvas, device.limits.maxTextureDimension2D);
  const target = BROWSER_RENDERER_HOST.createCanvasTarget(rendererCanvas, device, format);
  if (target === undefined) {
    debugRuntime.controller.state.mode = "error";
    debugRuntime.controller.state.error = "canvas.getContext('webgpu') returned null";
    return;
  }

  const guiSize = GuiRenderer.calculateGuiSize(rendererCanvas.width, rendererCanvas.height);
  const screenManager = new ScreenManager(guiSize.guiWidth, guiSize.guiHeight);
  const progressScreen = new ProgressScreen(false);
  progressScreen.progressStartNoAbort("Loading world");
  screenManager.setScreen(progressScreen);

  let debugHudText = "";
  const guiOverlayHost = await GuiOverlayHost.create(device, rendererCanvas, screenManager, (drawList) => {
    if (screenManager.currentScreen !== null || debugHudText.length === 0) {
      return;
    }
    // WebGPU: lightweight debug HUD is drawn into the GUI pass instead of a DOM text overlay.
    GuiComponent.drawString(drawList, screenManager.font, debugHudText, 2, 2, 0xffffffff);
  });
  const renderGuiFrameNow = (): void => {
    const resized = resizeCanvasToDisplaySize(rendererCanvas, device.limits.maxTextureDimension2D);
    const size = GuiRenderer.calculateGuiSize(resized.width, resized.height);
    debugRuntime.controller.state.width = size.guiWidth;
    debugRuntime.controller.state.height = size.guiHeight;
    const view = target.ctx.getCurrentTexture().createView();
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view,
          clearValue: { r: 0.06, g: 0.07, b: 0.08, a: 1 },
          loadOp: "clear",
          storeOp: "store",
        },
      ],
    });
    pass.end();
    guiOverlayHost.encode(encoder, view, target.format, resized.width, resized.height);
    device.queue.submit([encoder.finish()]);
  };
  guiOverlayHost.attachInput(() => {
    if (debugRuntime.controller.state.ready !== true) {
      renderGuiFrameNow();
    }
  });

  const updateGuiState = (): void => {
    const currentScreen = screenManager.currentScreen;
    const size = GuiRenderer.calculateGuiSize(rendererCanvas.width, rendererCanvas.height);
    debugRuntime.controller.state.width = size.guiWidth;
    debugRuntime.controller.state.height = size.guiHeight;
    debugRuntime.controller.state.pauseScreenActive = currentScreen !== null && debugRuntime.controller.state.ready;
    if (debugRuntime.controller.state.error === undefined) {
      debugRuntime.controller.state.mode = debugRuntime.controller.state.ready
        ? (currentScreen === null ? "world" : "paused")
        : "loading";
      debugRuntime.controller.state.screenTitle = currentScreen?.getTitle() ?? "";
    }
    debugRuntime.controller.state.options = copyGuiOptionsState(optionsState);
    debugRuntime.controller.state.debugSettings = copyGuiDebugSettingsState(debugSettingsState);
  };
  const persistGuiSettings = (): void => {
    writeStoredBrowserRenderConfig(window.localStorage, {
      ...optionsState,
      worldStorageMode: debugSettingsState.worldStorageMode,
    });
    const nextSessionConfig: DebugSessionConfig = {
      seed: sessionConfig.seed,
      movementMode: debugSettingsState.movementMode,
      preset: debugSettingsState.preset,
    };
    writeStoredDebugSessionConfig(window.localStorage, nextSessionConfig);
    writeStartLastWorld(window.localStorage, nextSessionConfig);
    debugRuntime.controller.state.movementMode = nextSessionConfig.movementMode;
    debugRuntime.controller.state.preset = nextSessionConfig.preset;
    debugRuntime.controller.state.viewDistance = optionsState.viewDistance;
    debugRuntime.controller.state.renderDistance = optionsState.renderDistance;
    debugRuntime.controller.state.lightingMode = optionsState.lightingMode;
    debugRuntime.controller.state.liquidSimulationMode = optionsState.liquidSimulationMode;
    debugRuntime.controller.state.worldStorageMode = debugSettingsState.worldStorageMode;
    updateGuiState();
  };
  const setErrorScreen = (message: string): void => {
    debugRuntime.controller.state.mode = "error";
    debugRuntime.controller.state.error = message;
    screenManager.setScreen(new AlertScreen(() => {}, "Debug Error", message, "Back"));
    debugRuntime.controller.state.screenTitle = "Debug Error";
    updateGuiState();
    renderGuiFrameNow();
  };
  showGlobalErrorScreen = setErrorScreen;

  const reportLoadingProgress = (progress: LoadingProgress): void => {
    debugRuntime.controller.state.loadingStage = progress.stage;
    debugRuntime.controller.state.loadingDetail = progress.detail;
    debugRuntime.controller.state.loadingProgress = progressFraction(progress);
    progressScreen.updateProgress(progress);
    updateGuiState();
    renderGuiFrameNow();
  };
  reportLoadingProgress({ stage: "Starting renderer", fraction: 0 });
  if (readClearWorldStorage()) {
    reportLoadingProgress({ stage: "Clearing stored world", fraction: 0.01 });
    if (typeof indexedDB === "undefined") {
      throw new Error("clearWorldStorage requested but IndexedDB is unavailable");
    }
    await deleteIndexedDbWorldStorage(indexedDB);
    clearTransientWorldStorageQueryParam();
  }

  const sceneResult = await initializeRendererScene(rendererCanvas, {
    seed: sessionConfig.seed,
    preset: sessionConfig.preset,
    viewDistance: renderConfig.viewDistance,
    renderDistance: renderConfig.renderDistance,
    engineConfig: {
      lightingMode: renderConfig.lightingMode,
      liquidSimulationMode: renderConfig.liquidSimulationMode,
    },
    worldTransport: runtimeConfig.worldTransport,
    remoteWorldHostUrl: runtimeConfig.remoteWorldHostUrl,
    skyColor: renderConfig.skyColor,
    clearColorScale: renderConfig.clearColorScale,
    worldStorageMode: renderConfig.worldStorageMode,
    rendererHost: createPinnedBrowserRendererHost(deviceContextResult.context, target),
    onProgress: reportLoadingProgress,
  });
  if (!sceneResult.ok) {
    debugRuntime.controller.state.error = sceneResult.reason;
    setErrorScreen(sceneResult.reason);
    return;
  }
  writeStoredDebugSessionConfig(window.localStorage, sessionConfig);
  writeStartLastWorld(window.localStorage, sessionConfig);
  const scene: RendererScene = sceneResult.scene;
  const expectedLoadedChunkCount = getExpectedLoadedChunkCount(scene.viewDistance);
  debugRuntime.controller.state.expectedLoadedChunkCount = expectedLoadedChunkCount;

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

  const openOptionsScreen = (lastScreen: PauseScreen): void => {
    debugRuntime.controller.state.lastAction = "options";
    screenManager.setScreen(new OptionsScreen(lastScreen, optionsState, {
      onChanged: persistGuiSettings,
      onDone: () => {
        debugRuntime.controller.state.lastAction = "options_done";
        updateGuiState();
      },
    }));
    updateGuiState();
  };
  const openDebugSettingsScreen = (lastScreen: PauseScreen): void => {
    debugRuntime.controller.state.lastAction = "debug_settings";
    screenManager.setScreen(new DebugSettingsScreen(lastScreen, debugSettingsState, {
      onChanged: persistGuiSettings,
      onDone: () => {
        debugRuntime.controller.state.lastAction = "debug_settings_done";
        updateGuiState();
      },
    }));
    updateGuiState();
  };
  const openPauseMenu = (): void => {
    if (!debugRuntime.controller.state.ready || screenManager.currentScreen !== null) {
      return;
    }
    if (document.pointerLockElement === rendererCanvas) {
      void document.exitPointerLock();
    }
    const pauseScreen = new PauseScreen(true, {
      onReturnToGame: () => {
        debugRuntime.controller.state.lastAction = "back_to_game";
        screenManager.setScreen(null);
        updateGuiState();
      },
      onOptions: () => openOptionsScreen(pauseScreen),
      onDebugSettings: () => openDebugSettingsScreen(pauseScreen),
    });
    screenManager.setScreen(pauseScreen);
    updateGuiState();
  };
  window.addEventListener("keydown", (event) => {
    if (event.defaultPrevented || event.key !== "Escape") {
      return;
    }
    if (screenManager.currentScreen === null) {
      openPauseMenu();
      event.preventDefault();
    }
  });

  const input = new DebugInput(rendererCanvas, {
    isGuiActive: () => screenManager.currentScreen !== null,
  });

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
  const queuedPlayerInputs: PlayerInputCommand[] = [];
  let playerInputSendInFlight = false;
  let lastPlayerButtonMask = 0;
  let lastChunkViewRequest: SetChunkViewRequest | undefined;
  const playerCommandClock = new MovementCommandClock({
    commandQuantumUs: PLAYER_COMMAND_QUANTUM_US,
    maxStepCountPerCommand: 4,
    maxCatchupStepCount: 24,
  });
  const drivesPlayer = sessionConfig.movementMode === "player";

  async function flushPlayerInputQueue(): Promise<void> {
    if (playerInputSendInFlight) {
      return;
    }

    playerInputSendInFlight = true;
    try {
      while (queuedPlayerInputs.length > 0) {
        const inputCommand = queuedPlayerInputs.shift()!;
        await scene.clientRuntime.sendPlayerCommand({
          type: "set_player_input",
          input: inputCommand,
        });
      }
    } catch (error) {
      const message = formatUnknownError(error);
      debugRuntime.controller.state.error = message;
      setErrorScreen(message);
      // eslint-disable-next-line no-console
      console.error(error);
    } finally {
      playerInputSendInFlight = false;
      if (queuedPlayerInputs.length > 0) {
        void flushPlayerInputQueue();
      }
    }
  }

  function shouldQueuePlayerInput(inputCommand: PlayerInputCommand): boolean {
    if (!isSamePlayerInput(lastInputCommand, inputCommand)) {
      return true;
    }

    return Math.hypot(inputCommand.moveX, inputCommand.moveY, inputCommand.moveZ) > 0.0
      || (inputCommand.buttons ?? 0) !== 0
      || (inputCommand.edgeButtons ?? 0) !== 0;
  }

  function queuePlayerInput(inputCommand: PlayerInputCommand): boolean {
    if (!shouldQueuePlayerInput(inputCommand)) {
      return false;
    }

    lastInputCommand = inputCommand;
    nextInputSequence++;
    queuedPlayerInputs.push(inputCommand);
    void flushPlayerInputQueue();
    return true;
  }

  // Prime chunks at the start so the first frame has something to draw.
  const initialChunkViewRequest: SetChunkViewRequest = {
    type: "set_chunk_view",
    centerChunkX: SectionPos.posToSectionCoord(initialCamera.position.x),
    centerChunkZ: SectionPos.posToSectionCoord(initialCamera.position.z),
    radius: scene.viewDistance,
  };
  if (await scene.clientRuntime.setChunkInterest(initialChunkViewRequest)) {
    applyRenderWorldDirtySections(scene);
    scene.levelRenderer.allChanged();
  }
  lastChunkViewRequest = initialChunkViewRequest;
  if (drivesPlayer) {
    const initialInputCommand: PlayerInputCommand = {
      sequence: nextInputSequence++,
      moveX: 0,
      moveY: 0,
      moveZ: 0,
      yaw: initialCamera.yRot,
      pitch: initialCamera.xRot,
      buttons: 0,
      edgeButtons: 0,
    };
    if (await scene.clientRuntime.sendPlayerCommand({
      type: "set_player_input",
      input: initialInputCommand,
    })) {
      lastInputCommand = initialInputCommand;
    }
  }
  if (!await waitForLoadedChunkRing(scene, expectedLoadedChunkCount, {
    maxAttempts: Math.max(2400, expectedLoadedChunkCount * 32),
    onProgress: (progress) => reportLoadingProgress({
      ...progress,
      fraction: 0.92 + ((progressFraction(progress) ?? 0) * 0.06),
    }),
  })) {
    debugRuntime.controller.state.error =
      `expected ${expectedLoadedChunkCount.toString()} loaded chunks for viewDistance=${scene.viewDistance.toString()}, got ${getSceneLoadedChunkCount(scene).toString()}`;
    setErrorScreen(debugRuntime.controller.state.error);
    return;
  }

  // Mesh jobs can be requested before every chunk needed by their padded
  // neighborhood is present. Re-run the initial visible set after the ring is
  // complete so screenshots start from a settled frame instead of a partial one.
  reportLoadingProgress({ stage: "Building first frame", fraction: 0.98 });
  scene.levelRenderer.allChanged();
  const initialFrame = await renderSceneUntilSettled(scene, camera);
  const initialEncoder = scene.device.createCommandEncoder();
  const initialView = scene.ctx.getCurrentTexture().createView();
  encodeSceneFrame(
    scene,
    initialFrame,
    {
      view: initialView,
      depthView: depthTarget!.view,
      format: scene.format,
    },
    initialEncoder,
  );
  guiOverlayHost.encode(initialEncoder, initialView, scene.format, scene.canvas.width, scene.canvas.height);
  scene.device.queue.submit([initialEncoder.finish()]);
  await scene.device.queue.onSubmittedWorkDone();
  totalFrameCount++;

  const initialPresentation = scene.clientRuntime.publishPresentationState();
  const initialPlayerState = initialPresentation.localPlayerState;
  const initialSessionState = initialPresentation.sessionState;
  if (initialPlayerState !== undefined) {
    const playerChunkViewRequest = createChunkViewRequestForPlayerState(initialPlayerState, scene.viewDistance);
    debugRuntime.controller.state.sessionId = initialSessionState?.sessionId;
    debugRuntime.controller.state.playerId = initialSessionState?.playerId;
    debugRuntime.controller.state.playerTick = initialPlayerState.tick;
    debugRuntime.controller.state.playerPosition = [
      initialPlayerState.position.x,
      initialPlayerState.position.y,
      initialPlayerState.position.z,
    ];
    debugRuntime.controller.state.cameraPosition = [camera.position.x, camera.position.y, camera.position.z];
    debugRuntime.controller.state.cameraYaw = camera.yRot;
    debugRuntime.controller.state.cameraPitch = camera.xRot;
    debugRuntime.controller.state.playerChunkX = playerChunkViewRequest.centerChunkX;
    debugRuntime.controller.state.playerChunkZ = playerChunkViewRequest.centerChunkZ;
    debugRuntime.controller.state.chunkViewCenterX = initialSessionState?.chunkView?.centerChunkX;
    debugRuntime.controller.state.chunkViewCenterZ = initialSessionState?.chunkView?.centerChunkZ;
  }

  debugRuntime.controller.state.ready = true;
  debugRuntime.controller.state.saveId = scene.saveMetadata.saveId;
  debugRuntime.controller.state.loadedChunkCount = getSceneLoadedChunkCount(scene);
  debugRuntime.controller.state.renderWorldCounters = getSceneRenderWorldPerformanceCounters(scene);
  debugRuntime.controller.state.renderQueueStats = getSceneRenderQueueStats(scene);
  debugRuntime.controller.state.worldPerformance = scene.clientRuntime.publishPresentationState().performance;
  debugRuntime.controller.state.viewDistance = scene.viewDistance;
  debugRuntime.controller.state.renderDistance = scene.gameRenderer.getRenderDistance();
  debugRuntime.controller.state.lightingMode = renderConfig.lightingMode;
  debugRuntime.controller.state.liquidSimulationMode = renderConfig.liquidSimulationMode;
  debugRuntime.controller.state.worldStorageMode = renderConfig.worldStorageMode;
  debugRuntime.controller.state.frameCount = totalFrameCount;
  screenManager.setScreen(null);
  updateGuiState();

  updateRuntimeDebugOverlay(undefined);

  function updateRuntimeDebugOverlay(fps: number | undefined): void {
    const playerStateForOverlay = scene.clientRuntime.publishPresentationState().localPlayerState;
    if (playerStateForOverlay === undefined) {
      return;
    }

    debugHudText =
      `fps ${fps === undefined ? "--" : fps.toFixed(0)}  mode ${sessionConfig.movementMode}  pos ${camera.position.x.toFixed(1)}, ${camera.position.y.toFixed(1)}, ${camera.position.z.toFixed(1)}  yaw ${camera.yRot.toFixed(0)}  pitch ${camera.xRot.toFixed(0)}  tick ${playerStateForOverlay.tick.toString()}`;
  }

  async function tick(): Promise<void> {
    const now = performance.now();
    const frameDeltaMs = Math.max(0.0, now - lastFrameMs);
    const dtSeconds = Math.min(0.1, frameDeltaMs / 1000.0);
    lastFrameMs = now;
    // WebGPU: drive vanilla atlas animation ticks from the browser frame loop.
    textureAnimationElapsedMs = advanceTextureAtlasAnimations(scene.atlas, textureAnimationElapsedMs + frameDeltaMs);
    resizeViewport();

    const guiActive = screenManager.currentScreen !== null;
    const rawInputFrame = input.consumeFrame();
    const inputFrame = guiActive
      ? emptyDebugInputFrame()
      : mergeDebugInputFrame(rawInputFrame, debugRuntime.getInjectedInput());

    if (now - lastWorldPollMs >= WORLD_POLL_INTERVAL_MS) {
      if (await scene.clientRuntime.drainTransportUpdates()) {
        applyRenderWorldDirtySections(scene);
      }
      lastWorldPollMs = now;
    }

    if (now - lastLightTickMs >= LIGHT_TICK_INTERVAL_MS) {
      scene.lightTexture.tick();
      lastLightTickMs = now;
    }

    const presentation = scene.clientRuntime.publishPresentationState();
    const playerState = presentation.localPlayerState;
    if (!drivesPlayer) {
      camera = applyFreeCameraInput(camera, inputFrame, dtSeconds);
      const chunkViewRequest = createChunkViewRequestForCameraState(camera, scene.viewDistance);
      if (!isSameChunkViewRequest(lastChunkViewRequest, chunkViewRequest)) {
        if (await scene.clientRuntime.setChunkInterest(chunkViewRequest)) {
          applyRenderWorldDirtySections(scene);
        }
        lastChunkViewRequest = chunkViewRequest;
      }
    }

    if (playerState !== undefined) {
      if (drivesPlayer) {
        const predictionView = scene.clientRuntime.getClientWorld().getPredictionView();
        const predictionService = scene.clientRuntime.getPredictionService();
        const reconcileResult = predictionService.reconcileClientWorldSnapshot({
          clientWorld: predictionView,
          physicsParams: DEFAULT_MOVEMENT_PHYSICS,
        });
        const reconciledBody = reconcileResult?.body ?? predictionService.getPredictedBody();
        const authoritativeYaw = lastInputCommand?.yaw ?? playerState.rotation.yaw;
        const authoritativePitch = lastInputCommand?.pitch ?? playerState.rotation.pitch;
        if (!preserveInitialCamera) {
          camera = createCameraStateFromMovementBody(reconciledBody, authoritativeYaw, authoritativePitch);
        }
        const baseYaw = preserveInitialCamera ? (lastInputCommand?.yaw ?? playerState.rotation.yaw) : camera.yRot;
        const basePitch = preserveInitialCamera ? (lastInputCommand?.pitch ?? playerState.rotation.pitch) : camera.xRot;
        const commandStepCounts = playerCommandClock.consumeElapsedUs(Math.max(0, Math.round(dtSeconds * 1_000_000.0)));
        const currentButtonMask = getDebugPlayerButtonMask(inputFrame);
        let edgeButtonMask = currentButtonMask & ~lastPlayerButtonMask;
        lastPlayerButtonMask = currentButtonMask;
        for (const stepCount of commandStepCounts) {
          const playerInput = buildPlayerInputCommand(baseYaw, basePitch, inputFrame, dtSeconds, nextInputSequence, {
            clientTimeUs: Math.max(0, Math.round(now * 1_000.0)),
            commandQuantumUs: PLAYER_COMMAND_QUANTUM_US,
            stepCount,
            buttons: currentButtonMask,
            edgeButtons: edgeButtonMask,
            physicsRevision: PLAYER_MOVEMENT_PHYSICS_REVISION,
            collisionRevision: PLAYER_COLLISION_REVISION,
          });
          edgeButtonMask = 0;
          if (queuePlayerInput(playerInput) && !preserveInitialCamera) {
            const predictedBody = predictionService.advanceCommandReplay({
              command: playerInputToQueuedMoveCommand(playerState.playerId, playerInput).command,
              clientWorld: predictionView,
              physicsParams: DEFAULT_MOVEMENT_PHYSICS,
            });
            camera = createCameraStateFromMovementBody(predictedBody, playerInput.yaw, playerInput.pitch);
          }
        }

        const chunkViewRequest = preserveInitialCamera
          ? createChunkViewRequestForPlayerState(playerState, scene.viewDistance)
          : createChunkViewRequestForCameraState(camera, scene.viewDistance);
        if (!isSameChunkViewRequest(lastChunkViewRequest, chunkViewRequest)) {
          if (await scene.clientRuntime.setChunkInterest(chunkViewRequest)) {
            applyRenderWorldDirtySections(scene);
          }
          lastChunkViewRequest = chunkViewRequest;
        }
      }
      const playerChunkViewRequest = createChunkViewRequestForPlayerState(playerState, scene.viewDistance);
      const sessionState = presentation.sessionState;
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
      debugRuntime.controller.state.worldPerformance = presentation.performance;
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
        const view = scene.ctx.getCurrentTexture().createView();
        encodeSceneFrame(
          scene,
          renderedFrame,
          {
            view,
            depthView: depthTarget!.view,
            format: scene.format,
          },
          encoder,
        );
        guiOverlayHost.encode(encoder, view, scene.format, scene.canvas.width, scene.canvas.height);
        scene.device.queue.submit([encoder.finish()]);
        totalFrameCount++;
        fpsFrameCount++;
        debugRuntime.controller.state.frameCount = totalFrameCount;
        debugRuntime.controller.state.renderWorldCounters = getSceneRenderWorldPerformanceCounters(scene);
        debugRuntime.controller.state.renderQueueStats = getSceneRenderQueueStats(scene);
        debugRuntime.controller.state.worldPerformance = scene.clientRuntime.publishPresentationState().performance;
        if (now - lastFpsReportMs >= 1000.0) {
          updateRuntimeDebugOverlay((fpsFrameCount * 1000.0) / (now - lastFpsReportMs));
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
      window.__mcloneDebug.state.mode = "error";
      window.__mcloneDebug.state.error = message;
    }
    showGlobalErrorScreen?.(message);
    // eslint-disable-next-line no-console
    console.error(err);
  });
  window.addEventListener("error", (ev) => {
    if (window.__mcloneDebug) {
      window.__mcloneDebug.state.mode = "error";
      window.__mcloneDebug.state.error = ev.message;
    }
    showGlobalErrorScreen?.(ev.message);
  });
  window.addEventListener("unhandledrejection", (ev) => {
    const reason = ev.reason instanceof Error ? ev.reason.message : String(ev.reason);
    if (window.__mcloneDebug) {
      window.__mcloneDebug.state.mode = "error";
      window.__mcloneDebug.state.error = reason;
    }
    showGlobalErrorScreen?.(reason);
  });
}
