// Debug tooling — see tactical 40. Throwaway when real Player/Input lands.

import { SectionPos } from "../../core/section-pos";
import { DEFAULT_MOVEMENT_PHYSICS, MovementCommandClock } from "../../runtime/movement";
import type { OpenWorldPreset, PlayerInputCommand, SetChunkViewRequest, WorldPerformanceSnapshot } from "../../runtime/protocol/world-messages";
import {
  PLAYER_COLLISION_REVISION,
  PLAYER_COMMAND_QUANTUM_US,
  PLAYER_MOVEMENT_PHYSICS_REVISION,
  playerInputToQueuedMoveCommand,
} from "../../runtime/session/player-loop";
import { deleteIndexedDbWorldStorage } from "../../runtime/storage/indexeddb-world-storage";
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
import {
  BROWSER_RENDER_CONFIG_QUERY_KEYS,
  clearStoredBrowserRenderConfig,
  getDefaultRenderDistance,
  getExpectedLoadedChunkCount,
  readBrowserRenderConfig,
  writeStoredBrowserRenderConfig,
  type BrowserRenderConfig,
  type StoredBrowserRenderConfig,
} from "../browser-render-config";
import type { LoadingProgress } from "../loading-progress";
import { progressFraction } from "../loading-progress";
import { advanceTextureAtlasAnimations } from "../texture/texture-atlas";
import { DebugInput } from "./debug-input";
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

const DEFAULT_SEED = 12_345n;
const DEFAULT_PRESET: OpenWorldPreset = "browser_smoke";
const DEFAULT_MOVEMENT_MODE: DebugMovementMode = "player";
const DEBUG_SESSION_CONFIG_STORAGE_KEY = "mclone.debug.sessionConfig.v1";
const START_LAST_WORLD_STORAGE_KEY = "mclone.start.lastWorld.v1";
const DEBUG_SESSION_CONFIG_QUERY_KEYS = ["seed", "movementMode", "preset"] as const;
const LIGHT_TICK_INTERVAL_MS = 1000.0;
const WORLD_POLL_INTERVAL_MS = 50.0;

const DEFAULT_INITIAL_POSITION = new Vec3(8.5, 104.0, 40.5);
const DEFAULT_INITIAL_X_ROT = 30.0;
const DEFAULT_INITIAL_Y_ROT = 180.0;

type DebugMovementMode = "player" | "freecam";

interface DebugSessionConfig {
  readonly seed: bigint;
  readonly movementMode: DebugMovementMode;
  readonly preset: OpenWorldPreset;
}

interface DebugRuntimeState {
  ready: boolean;
  readonly worldTransport: "worker" | "remote";
  seed: string;
  movementMode: DebugMovementMode;
  preset: OpenWorldPreset;
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
  lightingMode?: BrowserRenderConfig["lightingMode"];
  liquidSimulationMode?: BrowserRenderConfig["liquidSimulationMode"];
  worldStorageMode?: BrowserRenderConfig["worldStorageMode"];
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function parseSeedValue(value: string | null | undefined, fallback: bigint): bigint {
  if (value === null || value === undefined || value.trim() === "") {
    return fallback;
  }

  try {
    return BigInt(value.trim());
  } catch {
    return fallback;
  }
}

function parseMovementMode(value: unknown, fallback: DebugMovementMode): DebugMovementMode {
  return value === "player" || value === "freecam" ? value : fallback;
}

function parsePreset(value: unknown, fallback: OpenWorldPreset): OpenWorldPreset {
  return value === "default" || value === "browser_smoke" ? value : fallback;
}

function readStoredDebugSessionConfig(
  storage: Pick<Storage, "getItem"> | undefined,
): Partial<DebugSessionConfig> {
  if (storage === undefined) {
    return {};
  }

  try {
    const raw = storage.getItem(DEBUG_SESSION_CONFIG_STORAGE_KEY);
    if (raw === null) {
      return {};
    }

    const parsed = JSON.parse(raw) as unknown;
    if (!isRecord(parsed)) {
      return {};
    }

    const movementMode = parsed.movementMode === "player" || parsed.movementMode === "freecam"
      ? parsed.movementMode
      : undefined;
    const preset = parsed.preset === "default" || parsed.preset === "browser_smoke"
      ? parsed.preset
      : undefined;
    return {
      seed: typeof parsed.seed === "string" ? parseSeedValue(parsed.seed, DEFAULT_SEED) : undefined,
      movementMode,
      preset,
    };
  } catch {
    return {};
  }
}

function readDebugSessionConfig(url: URL, storage?: Pick<Storage, "getItem">): DebugSessionConfig {
  const stored = readStoredDebugSessionConfig(storage);
  const storedSeed = stored.seed ?? DEFAULT_SEED;
  const storedMovementMode = stored.movementMode ?? DEFAULT_MOVEMENT_MODE;
  const storedPreset = stored.preset ?? DEFAULT_PRESET;
  return {
    seed: parseSeedValue(url.searchParams.get("seed"), storedSeed),
    movementMode: parseMovementMode(url.searchParams.get("movementMode"), storedMovementMode),
    preset: parsePreset(url.searchParams.get("preset"), storedPreset),
  };
}

function writeStoredDebugSessionConfig(
  storage: Pick<Storage, "setItem">,
  config: DebugSessionConfig,
): void {
  storage.setItem(DEBUG_SESSION_CONFIG_STORAGE_KEY, JSON.stringify({
    seed: config.seed.toString(),
    movementMode: config.movementMode,
    preset: config.preset,
  }));
}

function writeStartLastWorld(
  storage: Pick<Storage, "setItem">,
  config: DebugSessionConfig,
): void {
  storage.setItem(START_LAST_WORLD_STORAGE_KEY, JSON.stringify({
    seed: config.seed.toString(),
    movementMode: config.movementMode,
    preset: config.preset,
    lastOpenedAtMs: Date.now(),
  }));
}

function clearStoredDebugSessionConfig(storage: Pick<Storage, "removeItem">): void {
  storage.removeItem(DEBUG_SESSION_CONFIG_STORAGE_KEY);
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

function showOverlayMessage(message: string): void {
  const overlay = document.querySelector<HTMLElement>("#debug-overlay");
  if (overlay) {
    overlay.textContent = message;
    overlay.style.display = "";
  }
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

function clampInteger(value: number, fallback: number, min: number, max: number): number {
  if (!Number.isFinite(value)) {
    return fallback;
  }

  return Math.min(max, Math.max(min, Math.trunc(value)));
}

function reloadWithoutBrowserConfigQueryParams(): void {
  const url = new URL(window.location.href);
  for (const key of BROWSER_RENDER_CONFIG_QUERY_KEYS) {
    url.searchParams.delete(key);
  }
  for (const key of DEBUG_SESSION_CONFIG_QUERY_KEYS) {
    url.searchParams.delete(key);
  }

  window.location.href = `${url.pathname}${url.search}${url.hash}`;
}

function reloadWithClearWorldStorage(): void {
  const url = new URL(window.location.href);
  url.searchParams.set("clearWorldStorage", "1");
  window.location.href = `${url.pathname}${url.search}${url.hash}`;
}

function clearTransientWorldStorageQueryParam(): void {
  const url = new URL(window.location.href);
  if (!url.searchParams.has("clearWorldStorage")) {
    return;
  }

  url.searchParams.delete("clearWorldStorage");
  window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
}

function configureDebugSettingsMenu(config: BrowserRenderConfig, sessionConfig: DebugSessionConfig): void {
  const form = document.querySelector<HTMLFormElement>("#debug-config-form");
  const viewDistanceInput = document.querySelector<HTMLInputElement>("#debug-view-distance");
  const renderDistanceInput = document.querySelector<HTMLInputElement>("#debug-render-distance");
  const seedInput = document.querySelector<HTMLInputElement>("#debug-seed");
  const playerModeInput = document.querySelector<HTMLInputElement>("#debug-mode-player");
  const freeCamModeInput = document.querySelector<HTMLInputElement>("#debug-mode-freecam");
  const disableLightingInput = document.querySelector<HTMLInputElement>("#debug-disable-lighting");
  const disableWaterSimInput = document.querySelector<HTMLInputElement>("#debug-disable-water-sim");
  const disableIndexedDbInput = document.querySelector<HTMLInputElement>("#debug-disable-indexed-db");
  const chunkCountOutput = document.querySelector<HTMLOutputElement>("#debug-config-chunk-count");
  const resetButton = document.querySelector<HTMLButtonElement>("#debug-config-reset");
  const deleteIndexedDbButton = document.querySelector<HTMLButtonElement>("#debug-delete-indexed-db");
  if (
    form === null
    || viewDistanceInput === null
    || renderDistanceInput === null
    || seedInput === null
    || playerModeInput === null
    || freeCamModeInput === null
    || disableLightingInput === null
    || disableWaterSimInput === null
    || disableIndexedDbInput === null
  ) {
    return;
  }

  const renderChunkCount = (): void => {
    if (chunkCountOutput === null) {
      return;
    }

    const viewDistance = clampInteger(Number.parseInt(viewDistanceInput.value, 10), config.viewDistance, 1, 16);
    chunkCountOutput.value = `${getExpectedLoadedChunkCount(viewDistance).toString()} loaded host chunks`;
  };

  viewDistanceInput.value = config.viewDistance.toString();
  renderDistanceInput.value = config.renderDistance.toString();
  seedInput.value = sessionConfig.seed.toString();
  playerModeInput.checked = sessionConfig.movementMode === "player";
  freeCamModeInput.checked = sessionConfig.movementMode === "freecam";
  disableLightingInput.checked = config.lightingMode === "none";
  disableWaterSimInput.checked = config.liquidSimulationMode === "none";
  disableIndexedDbInput.checked = config.worldStorageMode === "none";
  renderChunkCount();

  viewDistanceInput.addEventListener("input", () => {
    const viewDistance = clampInteger(Number.parseInt(viewDistanceInput.value, 10), config.viewDistance, 1, 16);
    renderDistanceInput.value = getDefaultRenderDistance(viewDistance).toString();
    renderChunkCount();
  });
  renderDistanceInput.addEventListener("input", renderChunkCount);
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const viewDistance = clampInteger(Number.parseInt(viewDistanceInput.value, 10), config.viewDistance, 1, 16);
    const renderDistance = clampInteger(Number.parseInt(renderDistanceInput.value, 10), getDefaultRenderDistance(viewDistance), 16, 512);
    const nextConfig: StoredBrowserRenderConfig = {
      viewDistance,
      renderDistance,
      lightingMode: disableLightingInput.checked ? "none" : "vanilla17",
      liquidSimulationMode: disableWaterSimInput.checked ? "none" : "vanilla17",
      worldStorageMode: disableIndexedDbInput.checked ? "none" : "default",
    };
    writeStoredBrowserRenderConfig(window.localStorage, nextConfig);
    const nextSessionConfig: DebugSessionConfig = {
      seed: parseSeedValue(seedInput.value, sessionConfig.seed),
      movementMode: freeCamModeInput.checked ? "freecam" : "player",
      preset: sessionConfig.preset,
    };
    writeStoredDebugSessionConfig(window.localStorage, nextSessionConfig);
    writeStartLastWorld(window.localStorage, nextSessionConfig);
    reloadWithoutBrowserConfigQueryParams();
  });
  resetButton?.addEventListener("click", () => {
    clearStoredBrowserRenderConfig(window.localStorage);
    clearStoredDebugSessionConfig(window.localStorage);
    reloadWithoutBrowserConfigQueryParams();
  });
  deleteIndexedDbButton?.addEventListener("click", () => {
    if (!window.confirm("Delete mclone IndexedDB world storage and reload?")) {
      return;
    }

    reloadWithClearWorldStorage();
  });
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
    showOverlayMessage("error: canvas #renderer not found");
    return;
  }
  const rendererCanvas = canvas;

  const runtimeConfig = readWorldTransport();
  const url = new URL(window.location.href);
  const renderConfig = readBrowserRenderConfig(url, window.localStorage);
  const sessionConfig = readDebugSessionConfig(url, window.localStorage);
  configureDebugSettingsMenu(renderConfig, sessionConfig);
  const initialCamera = readInitialCamera();
  const preserveInitialCamera = readPreserveInitialCamera();
  const debugRuntime = createDebugRuntimeController(runtimeConfig.worldTransport, sessionConfig);
  window.__mcloneDebug = debugRuntime.controller;
  let showInitialLoadingUi = true;
  const reportLoadingProgress = (progress: LoadingProgress): void => {
    debugRuntime.controller.state.loadingStage = progress.stage;
    debugRuntime.controller.state.loadingDetail = progress.detail;
    debugRuntime.controller.state.loadingProgress = progressFraction(progress);
    if (showInitialLoadingUi) {
      showLoadingProgress(progress);
    }
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
    onProgress: reportLoadingProgress,
  });
  if (!sceneResult.ok) {
    debugRuntime.controller.state.error = sceneResult.reason;
    showOverlayMessage(`error: ${sceneResult.reason}`);
    return;
  }
  writeStoredDebugSessionConfig(window.localStorage, sessionConfig);
  writeStartLastWorld(window.localStorage, sessionConfig);
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
      showOverlayMessage(`error: ${message}`);
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

  showInitialLoadingUi = false;
  hideLoadingProgress();
  updateRuntimeDebugOverlay(undefined);

  function updateRuntimeDebugOverlay(fps: number | undefined): void {
    const playerStateForOverlay = scene.clientRuntime.publishPresentationState().localPlayerState;
    if (playerStateForOverlay === undefined) {
      return;
    }

    showOverlayMessage(
      `fps ${fps === undefined ? "--" : fps.toFixed(0)}  mode ${sessionConfig.movementMode}  pos ${camera.position.x.toFixed(1)}, ${camera.position.y.toFixed(1)}, ${camera.position.z.toFixed(1)}  yaw ${camera.yRot.toFixed(0)}  pitch ${camera.xRot.toFixed(0)}  tick ${playerStateForOverlay.tick.toString()}`,
    );
  }

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
