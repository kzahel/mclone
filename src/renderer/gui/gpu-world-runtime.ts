import {
  DEFAULT_MOVEMENT_PHYSICS,
  MOVEMENT_COMMAND_BUTTONS,
  MovementCommandClock,
  shouldAutoJump,
  type MovementIntent,
} from "../../runtime/movement";
import type {
  PlayerInputCommand,
  SetChunkViewRequest,
  WorldPerformanceSnapshot,
} from "../../runtime/protocol/world-messages";
import {
  PLAYER_COLLISION_REVISION,
  PLAYER_COMMAND_QUANTUM_US,
  PLAYER_MOVEMENT_PHYSICS_REVISION,
  playerInputToQueuedMoveCommand,
} from "../../runtime/session/player-loop";
import type { ScreenManager } from "../../client/gui/screen-manager";
import { DebugSettingsScreen, type GuiDebugSettingsState } from "../../client/gui/screens/debug-settings-screen";
import { OptionsScreen, type GuiOptionsState } from "../../client/gui/screens/options-screen";
import { PauseScreen } from "../../client/gui/screens/pause-screen";
import {
  applyRenderWorldDirtySections,
  createSceneDepthTarget,
  encodeSceneFrame,
  getSceneLoadedChunkCount,
  getSceneRenderQueueStats,
  getSceneRenderWorldPerformanceCounters,
  resizeCanvasToDisplaySize,
  type RenderSceneQueueStats,
  type RenderWorldPerformanceCounters,
  type RendererScene,
} from "../scene-setup";
import type { LevelRenderFrame } from "../level-renderer";
import { advanceTextureAtlasAnimations } from "../texture/texture-atlas";
import { DebugInput, type DebugInputFrame } from "../debug/debug-input";
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
} from "../debug/debug-player-controls";
import { GuiComponent } from "../../client/gui/gui-component";
import type { GuiDrawList } from "./gui-draw-list";
import type { GuiOverlayHost } from "./gui-overlay-host";
import { renderTouchControlsHud } from "./touch-joystick-hud";

const LIGHT_TICK_INTERVAL_MS = 1000.0;
const WORLD_POLL_INTERVAL_MS = 50.0;

export interface GpuWorldRuntimeState {
  ready: boolean;
  mode?: string;
  screenTitle?: string;
  lastAction?: string;
  pauseScreenActive?: boolean;
  frameCount: number;
  inputEventCount: number;
  cameraPosition?: readonly [number, number, number];
  cameraYaw?: number;
  cameraPitch?: number;
  sessionId?: string;
  playerId?: string;
  playerTick?: number;
  playerPosition?: readonly [number, number, number];
  playerChunkX?: number;
  playerChunkZ?: number;
  chunkViewCenterX?: number;
  chunkViewCenterZ?: number;
  loadedChunkCount?: number;
  renderWorldCounters?: RenderWorldPerformanceCounters;
  renderQueueStats?: RenderSceneQueueStats;
  worldPerformance?: WorldPerformanceSnapshot;
  error?: string;
}

export type GpuWorldMovementMode = "player" | "freecam";

export interface GpuWorldRuntimeOptions {
  readonly scene: RendererScene;
  readonly canvas: HTMLCanvasElement;
  readonly screenManager: ScreenManager;
  readonly guiOverlayHost: GuiOverlayHost;
  readonly initialCamera: DebugCameraState;
  readonly initialFrame?: LevelRenderFrame;
  readonly state: GpuWorldRuntimeState;
  readonly optionsState: GuiOptionsState;
  readonly onOptionsChanged: () => void;
  readonly debugSettingsState: GuiDebugSettingsState;
  readonly onDebugSettingsChanged: () => void;
  readonly movementMode?: GpuWorldMovementMode;
  readonly preserveInitialCamera?: boolean;
  readonly requirePointerLock?: boolean;
  readonly onDisconnect?: () => Promise<void> | void;
  readonly onError?: (message: string) => void;
}

export interface GpuWorldRuntime {
  setInjectedInput(input: DebugInjectedInput | null): void;
  stop(): void;
}

export async function startGpuWorldRuntime(options: GpuWorldRuntimeOptions): Promise<GpuWorldRuntime> {
  // WebGPU: browser requestAnimationFrame loop replaces Minecraft's main game loop ownership.
  const runtime = new BrowserGpuWorldRuntime(options);
  await runtime.start();
  return runtime;
}

export function shouldQueuePlayerInput(
  previousInputCommand: PlayerInputCommand | undefined,
  inputCommand: PlayerInputCommand,
): boolean {
  if ((inputCommand.stepCount ?? 0) > 0) {
    return true;
  }

  if (!isSamePlayerInput(previousInputCommand, inputCommand)) {
    return true;
  }

  return Math.hypot(inputCommand.moveX, inputCommand.moveY, inputCommand.moveZ) > 0.0
    || (inputCommand.buttons ?? 0) !== 0
    || (inputCommand.edgeButtons ?? 0) !== 0;
}

export interface PlayerPhysicsCommandFrameOptions {
  readonly commandClock: MovementCommandClock;
  readonly inputFrame: DebugInputFrame;
  readonly requirePointerLock?: boolean;
  readonly baseYaw: number;
  readonly basePitch: number;
  readonly dtSeconds: number;
  readonly nowMs: number;
  readonly nextInputSequence: number;
  readonly lastPlayerButtonMask: number;
}

export interface PlayerPhysicsCommandFrameResult {
  readonly inputCommands: readonly PlayerInputCommand[];
  readonly lastPlayerButtonMask: number;
  readonly acceptsGameplayInput: boolean;
}

export function consumePlayerPhysicsCommandsForFrame(
  options: PlayerPhysicsCommandFrameOptions,
): PlayerPhysicsCommandFrameResult {
  // Pointer lock gates captured look/move intent, not fixed-step physics time.
  const acceptsGameplayInput = options.requirePointerLock === false || options.inputFrame.locked;
  const inputFrame = acceptsGameplayInput ? options.inputFrame : emptyInputFrame();
  const commandStepCounts = options.commandClock.consumeElapsedUs(Math.max(0, Math.round(options.dtSeconds * 1_000_000.0)));
  const currentButtonMask = getDebugPlayerButtonMask(inputFrame);
  let edgeButtonMask = currentButtonMask & ~options.lastPlayerButtonMask;
  let nextInputSequence = options.nextInputSequence;
  const inputCommands: PlayerInputCommand[] = [];
  for (const stepCount of commandStepCounts) {
    inputCommands.push(buildPlayerInputCommand(options.baseYaw, options.basePitch, inputFrame, options.dtSeconds, nextInputSequence++, {
      clientTimeUs: Math.max(0, Math.round(options.nowMs * 1_000.0)),
      commandQuantumUs: PLAYER_COMMAND_QUANTUM_US,
      stepCount,
      buttons: currentButtonMask,
      edgeButtons: edgeButtonMask,
      physicsRevision: PLAYER_MOVEMENT_PHYSICS_REVISION,
      collisionRevision: PLAYER_COLLISION_REVISION,
    }));
    edgeButtonMask = 0;
  }

  return {
    inputCommands,
    lastPlayerButtonMask: currentButtonMask,
    acceptsGameplayInput,
  };
}

function movementIntentFromPlayerInput(input: PlayerInputCommand): MovementIntent {
  const buttons = input.buttons ?? 0;
  const edgeButtons = input.edgeButtons ?? 0;
  return {
    wishX: input.moveX,
    wishZ: input.moveZ,
    jump: (buttons & MOVEMENT_COMMAND_BUTTONS.JUMP) !== 0 || (edgeButtons & MOVEMENT_COMMAND_BUTTONS.JUMP) !== 0,
    crouch: (buttons & MOVEMENT_COMMAND_BUTTONS.CROUCH) !== 0,
    sprint: (buttons & MOVEMENT_COMMAND_BUTTONS.SPRINT) !== 0,
    yaw: input.yaw,
    pitch: input.pitch,
  };
}

export function buildDebugOverlayLines(
  state: Pick<
    GpuWorldRuntimeState,
    "cameraPosition"
    | "cameraYaw"
    | "cameraPitch"
    | "playerTick"
    | "playerPosition"
    | "playerChunkX"
    | "playerChunkZ"
    | "chunkViewCenterX"
    | "chunkViewCenterZ"
    | "loadedChunkCount"
  >,
  movementMode: GpuWorldMovementMode,
): string[] {
  const position = movementMode === "player"
    ? (state.playerPosition ?? state.cameraPosition)
    : state.cameraPosition;
  const lines: string[] = [];

  if (position !== undefined) {
    lines.push(`XYZ: ${position.map((value) => value.toFixed(3)).join(" / ")}`);
  }
  if (state.cameraPitch !== undefined || state.cameraYaw !== undefined) {
    lines.push(`Pitch/Yaw: ${formatDebugValue(state.cameraPitch, 1)} / ${formatDebugValue(state.cameraYaw, 1)}`);
  }
  if (state.playerTick !== undefined) {
    lines.push(`# Ticks: ${state.playerTick.toString()}`);
  }
  if (movementMode === "player" && state.playerChunkX !== undefined && state.playerChunkZ !== undefined) {
    lines.push(`Chunk: ${state.playerChunkX.toString()}, ${state.playerChunkZ.toString()}`);
  }
  if (state.chunkViewCenterX !== undefined && state.chunkViewCenterZ !== undefined) {
    lines.push(`View Chunk: ${state.chunkViewCenterX.toString()}, ${state.chunkViewCenterZ.toString()}`);
  }
  if (state.loadedChunkCount !== undefined) {
    lines.push(`Loaded Chunks: ${state.loadedChunkCount.toString()}`);
  }
  lines.push(`Mode: ${movementMode === "player" ? "Player" : "Free Cam"}`);
  return lines;
}

class BrowserGpuWorldRuntime implements GpuWorldRuntime {
  private readonly input: DebugInput;
  private readonly listeners: Array<readonly [EventTarget, string, EventListener]> = [];
  private depthTarget: ReturnType<typeof createSceneDepthTarget> | undefined;
  private camera: DebugCameraState;
  private injectedInput: DebugInjectedInput | null = null;
  private lastFrameMs = performance.now();
  private lastLightTickMs = this.lastFrameMs;
  private lastWorldPollMs = this.lastFrameMs;
  private textureAnimationElapsedMs = 0.0;
  private frameRequest: number | undefined;
  private renderInFlight = false;
  private stopped = false;
  private lastChunkViewRequest: SetChunkViewRequest | undefined;
  private nextInputSequence = 1;
  private lastInputCommand: PlayerInputCommand | undefined;
  private readonly queuedPlayerInputs: PlayerInputCommand[] = [];
  private playerInputSendInFlight = false;
  private lastPlayerButtonMask = 0;
  private disconnectInFlight = false;
  private readonly playerCommandClock = new MovementCommandClock({
    commandQuantumUs: PLAYER_COMMAND_QUANTUM_US,
    maxStepCountPerCommand: 4,
    maxCatchupStepCount: 24,
  });

  public constructor(private readonly options: GpuWorldRuntimeOptions) {
    this.camera = options.initialCamera;
    this.input = new DebugInput(options.canvas, {
      isGuiActive: () => options.screenManager.currentScreen !== null,
      requirePointerLock: options.requirePointerLock,
      onPointerLockChange: (locked) => {
        if (!locked && options.requirePointerLock !== false && options.screenManager.currentScreen === null) {
          this.openPauseMenu();
        }
      },
    });
    options.guiOverlayHost.setOverlayRenderer((drawList) => {
      this.renderDebugOverlay(drawList);
      this.renderTouchHud(drawList);
    });
    this.add(window, "keydown", (event) => {
      const keyboardEvent = event as KeyboardEvent;
      if (keyboardEvent.defaultPrevented || keyboardEvent.key !== "Escape") {
        return;
      }
      if (options.screenManager.currentScreen === null) {
        this.openPauseMenu();
        keyboardEvent.preventDefault();
      }
    });
  }

  public async start(): Promise<void> {
    this.resizeViewport();
    this.lastChunkViewRequest = createChunkViewRequestForCameraState(this.camera, this.options.scene.viewDistance);
    if (this.drivesPlayer()) {
      await this.sendInitialPlayerInput();
    }
    this.updateState();

    this.options.state.ready = true;
    this.queueNextFrame();
  }

  public setInjectedInput(input: DebugInjectedInput | null): void {
    this.injectedInput = input;
  }

  public stop(): void {
    this.stopped = true;
    if (this.frameRequest !== undefined) {
      cancelAnimationFrame(this.frameRequest);
      this.frameRequest = undefined;
    }
    this.input.dispose();
    this.options.guiOverlayHost.setOverlayRenderer();
    for (const [target, type, listener] of this.listeners) {
      target.removeEventListener(type, listener);
    }
    this.listeners.length = 0;
    this.depthTarget?.texture.destroy();
    this.depthTarget = undefined;
  }

  private add(target: EventTarget, type: string, listener: EventListener): void {
    target.addEventListener(type, listener);
    this.listeners.push([target, type, listener]);
  }

  private renderTouchHud(drawList: GuiDrawList): void {
    if (this.options.screenManager.currentScreen !== null) {
      return;
    }

    const rect = this.options.canvas.getBoundingClientRect();
    renderTouchControlsHud(drawList, this.input.getTouchControlsState(), {
      guiWidth: this.options.screenManager.getWidth(),
      guiHeight: this.options.screenManager.getHeight(),
      cssLeft: rect.left,
      cssTop: rect.top,
      cssWidth: rect.width,
      cssHeight: rect.height,
    });
  }

  private renderDebugOverlay(drawList: GuiDrawList): void {
    if (!this.options.debugSettingsState.showDebugInfo || this.options.screenManager.currentScreen !== null) {
      return;
    }

    const lines = buildDebugOverlayLines(this.options.state, this.options.movementMode ?? "freecam");
    if (lines.length === 0) {
      return;
    }

    const font = this.options.screenManager.font;
    const textX = 2;
    const textY = 2;
    const maxWidth = lines.reduce((width, line) => Math.max(width, font.width(line)), 0);
    const boxWidth = maxWidth + 8;
    const boxHeight = (lines.length * font.lineHeight) + 7;
    GuiComponent.fill(drawList, 0, 0, boxWidth, boxHeight, 0x70000000);
    for (const [index, line] of lines.entries()) {
      GuiComponent.drawString(drawList, font, line, textX, textY + (index * font.lineHeight), 0xffffffff);
    }
  }

  private resizeViewport(): void {
    const { scene, canvas } = this.options;
    const resized = resizeCanvasToDisplaySize(canvas, scene.device.limits.maxTextureDimension2D);
    if (!resized.changed && this.depthTarget !== undefined) {
      return;
    }

    this.depthTarget?.texture.destroy();
    this.depthTarget = createSceneDepthTarget(scene.device, resized.width, resized.height);
    scene.gameRenderer.resize(resized.width, resized.height);
  }

  private queueNextFrame(): void {
    if (this.stopped || this.frameRequest !== undefined) {
      return;
    }

    this.frameRequest = requestAnimationFrame(() => {
      this.frameRequest = undefined;
      void this.tick();
    });
  }

  private drivesPlayer(): boolean {
    return (this.options.movementMode ?? "freecam") === "player";
  }

  private async sendInitialPlayerInput(): Promise<void> {
    const inputCommand: PlayerInputCommand = {
      sequence: this.nextInputSequence++,
      moveX: 0,
      moveY: 0,
      moveZ: 0,
      yaw: this.camera.yRot,
      pitch: this.camera.xRot,
      buttons: 0,
      edgeButtons: 0,
    };
    if (await this.options.scene.clientRuntime.sendPlayerCommand({
      type: "set_player_input",
      input: inputCommand,
    })) {
      this.lastInputCommand = inputCommand;
    }
  }

  private async flushPlayerInputQueue(): Promise<void> {
    if (this.playerInputSendInFlight) {
      return;
    }

    this.playerInputSendInFlight = true;
    try {
      while (this.queuedPlayerInputs.length > 0) {
        const inputCommand = this.queuedPlayerInputs.shift()!;
        await this.options.scene.clientRuntime.sendPlayerCommand({
          type: "set_player_input",
          input: inputCommand,
        });
      }
    } catch (error) {
      const message = formatUnknownError(error);
      this.options.state.error = message;
      this.options.onError?.(message);
      // eslint-disable-next-line no-console
      console.error(error);
    } finally {
      this.playerInputSendInFlight = false;
      if (this.queuedPlayerInputs.length > 0) {
        void this.flushPlayerInputQueue();
      }
    }
  }

  private shouldQueuePlayerInput(inputCommand: PlayerInputCommand): boolean {
    return shouldQueuePlayerInput(this.lastInputCommand, inputCommand);
  }

  private queuePlayerInput(inputCommand: PlayerInputCommand): boolean {
    if (!this.shouldQueuePlayerInput(inputCommand)) {
      return false;
    }

    this.lastInputCommand = inputCommand;
    this.nextInputSequence++;
    this.queuedPlayerInputs.push(inputCommand);
    void this.flushPlayerInputQueue();
    return true;
  }

  private async tick(): Promise<void> {
    if (this.stopped) {
      return;
    }

    try {
      const now = performance.now();
      const frameDeltaMs = Math.max(0.0, now - this.lastFrameMs);
      const dtSeconds = Math.min(0.1, frameDeltaMs / 1000.0);
      this.lastFrameMs = now;
      const scene = this.options.scene;

      this.textureAnimationElapsedMs = advanceTextureAtlasAnimations(scene.atlas, this.textureAnimationElapsedMs + frameDeltaMs);
      this.resizeViewport();
      const rawInputFrame = this.input.consumeFrame();
      const inputFrame = this.options.screenManager.currentScreen === null
        ? mergeDebugInputFrame(rawInputFrame, this.injectedInput)
        : emptyInputFrame();

      if (now - this.lastWorldPollMs >= WORLD_POLL_INTERVAL_MS) {
        if (await scene.clientRuntime.drainTransportUpdates()) {
          applyRenderWorldDirtySections(scene);
        }
        this.lastWorldPollMs = now;
      }

      if (now - this.lastLightTickMs >= LIGHT_TICK_INTERVAL_MS) {
        scene.lightTexture.tick();
        this.lastLightTickMs = now;
      }

      await this.updateCameraAndChunkInterest(inputFrame, dtSeconds, now);

      if (!this.renderInFlight) {
        this.renderInFlight = true;
        try {
          const entityPresentation = scene.clientRuntime.publishPresentationState().entityPresentation;
          await this.renderFrame(await scene.gameRenderer.renderLevel(
            0.0,
            Number.MAX_SAFE_INTEGER,
            scene.levelRenderer,
            scene.lightTexture,
            this.camera,
            {
              waitForChunkTasks: false,
              entityPresentation,
            },
          ));
        } finally {
          this.renderInFlight = false;
        }
      }
    } catch (error) {
      const message = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
      this.options.state.error = message;
      this.options.onError?.(message);
      // eslint-disable-next-line no-console
      console.error(error);
      this.stop();
      return;
    }

    this.queueNextFrame();
  }

  private async updateCameraAndChunkInterest(
    inputFrame: DebugInputFrame,
    dtSeconds: number,
    nowMs: number,
  ): Promise<void> {
    const scene = this.options.scene;
    const presentation = scene.clientRuntime.publishPresentationState();
    const playerState = presentation.localPlayerState;
    if (!this.drivesPlayer()) {
      this.camera = applyFreeCameraInput(this.camera, inputFrame, dtSeconds);
      await this.setChunkInterestIfChanged(createChunkViewRequestForCameraState(this.camera, scene.viewDistance));
      return;
    }

    if (playerState === undefined) {
      await this.setChunkInterestIfChanged(createChunkViewRequestForCameraState(this.camera, scene.viewDistance));
      return;
    }

    const predictionView = scene.clientRuntime.getClientWorld().getPredictionView();
    const predictionService = scene.clientRuntime.getPredictionService();
    const reconcileResult = predictionService.reconcileClientWorldSnapshot({
      clientWorld: predictionView,
      physicsParams: DEFAULT_MOVEMENT_PHYSICS,
    });
    const reconciledBody = reconcileResult?.body ?? predictionService.getPredictedBody();
    const authoritativeYaw = this.lastInputCommand?.yaw ?? playerState.rotation.yaw;
    const authoritativePitch = this.lastInputCommand?.pitch ?? playerState.rotation.pitch;
    if (this.options.preserveInitialCamera !== true) {
      this.camera = createCameraStateFromMovementBody(reconciledBody, authoritativeYaw, authoritativePitch);
    }
    if (this.options.screenManager.currentScreen !== null) {
      this.lastPlayerButtonMask = 0;
      const chunkViewRequest = this.options.preserveInitialCamera === true
        ? createChunkViewRequestForPlayerState(playerState, scene.viewDistance)
        : createChunkViewRequestForCameraState(this.camera, scene.viewDistance);
      await this.setChunkInterestIfChanged(chunkViewRequest);
      return;
    }
    const baseYaw = this.options.preserveInitialCamera === true
      ? (this.lastInputCommand?.yaw ?? playerState.rotation.yaw)
      : this.camera.yRot;
    const basePitch = this.options.preserveInitialCamera === true
      ? (this.lastInputCommand?.pitch ?? playerState.rotation.pitch)
      : this.camera.xRot;
    const physicsCommands = consumePlayerPhysicsCommandsForFrame({
      commandClock: this.playerCommandClock,
      inputFrame,
      requirePointerLock: this.options.requirePointerLock,
      baseYaw,
      basePitch,
      dtSeconds,
      nowMs,
      nextInputSequence: this.nextInputSequence,
      lastPlayerButtonMask: this.lastPlayerButtonMask,
    });
    this.lastPlayerButtonMask = physicsCommands.lastPlayerButtonMask;
    let autoJumpEdgeAvailable = this.options.optionsState.autoJump;
    for (let playerInput of physicsCommands.inputCommands) {
      if (
        autoJumpEdgeAvailable
        && ((playerInput.buttons ?? 0) & MOVEMENT_COMMAND_BUTTONS.JUMP) === 0
        && ((playerInput.edgeButtons ?? 0) & MOVEMENT_COMMAND_BUTTONS.JUMP) === 0
        && shouldAutoJump(
          reconciledBody,
          movementIntentFromPlayerInput(playerInput),
          predictionView.createCollisionWorld(),
          DEFAULT_MOVEMENT_PHYSICS,
        )
      ) {
        playerInput = {
          ...playerInput,
          edgeButtons: (playerInput.edgeButtons ?? 0) | MOVEMENT_COMMAND_BUTTONS.JUMP,
        };
        autoJumpEdgeAvailable = false;
      }
      if (this.queuePlayerInput(playerInput) && this.options.preserveInitialCamera !== true) {
        const predictedBody = predictionService.advanceCommandReplay({
          command: playerInputToQueuedMoveCommand(playerState.playerId, playerInput).command,
          clientWorld: predictionView,
          physicsParams: DEFAULT_MOVEMENT_PHYSICS,
        });
        this.camera = createCameraStateFromMovementBody(predictedBody, playerInput.yaw, playerInput.pitch);
      }
    }

    const chunkViewRequest = this.options.preserveInitialCamera === true
      ? createChunkViewRequestForPlayerState(playerState, scene.viewDistance)
      : createChunkViewRequestForCameraState(this.camera, scene.viewDistance);
    await this.setChunkInterestIfChanged(chunkViewRequest);
  }

  private async setChunkInterestIfChanged(chunkViewRequest: SetChunkViewRequest): Promise<void> {
    if (isSameChunkViewRequest(this.lastChunkViewRequest, chunkViewRequest)) {
      return;
    }

    if (await this.options.scene.clientRuntime.setChunkInterest(chunkViewRequest)) {
      applyRenderWorldDirtySections(this.options.scene);
    }
    this.lastChunkViewRequest = chunkViewRequest;
  }

  private async renderFrame(frame: LevelRenderFrame): Promise<void> {
    const depthTarget = this.depthTarget;
    if (depthTarget === undefined) {
      return;
    }

    const scene = this.options.scene;
    const encoder = scene.device.createCommandEncoder();
    const view = scene.ctx.getCurrentTexture().createView();
    encodeSceneFrame(
      scene,
      frame,
      {
        view,
        depthView: depthTarget.view,
        format: scene.format,
      },
      encoder,
    );
    this.updateState();
    this.options.guiOverlayHost.encode(encoder, view, scene.format, scene.canvas.width, scene.canvas.height);
    scene.device.queue.submit([encoder.finish()]);
    this.options.state.frameCount++;
  }

  private openPauseMenu(): void {
    if (this.options.screenManager.currentScreen !== null) {
      return;
    }

    if (document.pointerLockElement === this.options.canvas) {
      void document.exitPointerLock();
    }
    const pauseScreen = new PauseScreen(true, {
      onReturnToGame: () => {
        this.options.state.lastAction = "back_to_game";
        this.options.screenManager.setScreen(null);
        this.updateState();
      },
      onOptions: () => {
        this.openOptionsScreen(pauseScreen);
      },
      onDebugSettings: () => {
        this.openDebugSettingsScreen(pauseScreen);
      },
      onDisconnect: this.options.onDisconnect === undefined ? undefined : () => {
        if (this.disconnectInFlight) {
          return;
        }

        this.disconnectInFlight = true;
        this.options.state.lastAction = "disconnect";
        this.stop();
        void Promise.resolve(this.options.onDisconnect?.()).catch((error) => {
          const message = formatUnknownError(error);
          this.options.state.error = message;
          this.options.onError?.(message);
          // eslint-disable-next-line no-console
          console.error(error);
        }).finally(() => {
          this.disconnectInFlight = false;
        });
      },
    });
    this.options.screenManager.setScreen(pauseScreen);
    this.updateState();
  }

  private openOptionsScreen(lastScreen: PauseScreen): void {
    this.options.state.lastAction = "options";
    this.options.screenManager.setScreen(new OptionsScreen(lastScreen, this.options.optionsState, {
      onChanged: () => {
        this.options.onOptionsChanged();
        this.updateState();
      },
      onDone: () => {
        this.options.state.lastAction = "options_done";
        this.updateState();
      },
    }));
    this.updateState();
  }

  private openDebugSettingsScreen(lastScreen: PauseScreen): void {
    this.options.state.lastAction = "debug_settings";
    this.options.screenManager.setScreen(new DebugSettingsScreen(lastScreen, this.options.debugSettingsState, {
      onChanged: () => {
        this.options.onDebugSettingsChanged();
        this.updateState();
      },
      onDone: () => {
        this.options.state.lastAction = "debug_settings_done";
        this.updateState();
      },
    }));
    this.updateState();
  }

  private updateState(): void {
    const { scene, state } = this.options;
    const currentScreen = this.options.screenManager.currentScreen;
    state.pauseScreenActive = currentScreen !== null;
    if (state.error === undefined) {
      state.mode = currentScreen === null ? "world" : "paused";
      state.screenTitle = currentScreen?.getTitle() ?? "";
    }
    state.inputEventCount = this.input.getEventCount();
    state.cameraPosition = [this.camera.position.x, this.camera.position.y, this.camera.position.z];
    state.cameraYaw = this.camera.yRot;
    state.cameraPitch = this.camera.xRot;
    state.loadedChunkCount = getSceneLoadedChunkCount(scene);
    state.renderWorldCounters = getSceneRenderWorldPerformanceCounters(scene);
    state.renderQueueStats = getSceneRenderQueueStats(scene);
    const presentation = scene.clientRuntime.publishPresentationState();
    const playerState = presentation.localPlayerState;
    const sessionState = presentation.sessionState;
    state.sessionId = sessionState?.sessionId;
    state.playerId = sessionState?.playerId;
    state.chunkViewCenterX = sessionState?.chunkView?.centerChunkX;
    state.chunkViewCenterZ = sessionState?.chunkView?.centerChunkZ;
    state.worldPerformance = presentation.performance;
    if (playerState !== undefined) {
      const playerChunkViewRequest = createChunkViewRequestForPlayerState(playerState, scene.viewDistance);
      state.playerTick = playerState.tick;
      state.playerPosition = [playerState.position.x, playerState.position.y, playerState.position.z];
      state.playerChunkX = playerChunkViewRequest.centerChunkX;
      state.playerChunkZ = playerChunkViewRequest.centerChunkZ;
    }
  }
}

function emptyInputFrame(): DebugInputFrame {
  return {
    heldKeys: new Set<string>(),
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

function applyFreeCameraInput(
  camera: DebugCameraState,
  inputFrame: DebugInputFrame,
  dtSeconds: number,
): DebugCameraState {
  const inputCommand = buildFreeCameraInputCommand(camera.yRot, camera.xRot, inputFrame, dtSeconds, 0);
  return applyPredictedCameraInput(camera, inputCommand, dtSeconds);
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

function formatUnknownError(error: unknown): string {
  return error instanceof Error ? `${error.name}: ${error.message}` : String(error);
}

function formatDebugValue(value: number | undefined, digits: number): string {
  return value === undefined ? "n/a" : value.toFixed(digits);
}
