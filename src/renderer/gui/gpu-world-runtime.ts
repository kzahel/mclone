import type { SetChunkViewRequest } from "../../runtime/protocol/world-messages";
import type { ScreenManager } from "../../client/gui/screen-manager";
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
import type { DebugInputFrame } from "../debug/debug-input";
import {
  applyPredictedCameraInput,
  buildFreeCameraInputCommand,
  createChunkViewRequestForCameraState,
  type DebugCameraState,
} from "../debug/debug-player-controls";
import type { GuiOverlayHost } from "./gui-overlay-host";

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
  loadedChunkCount?: number;
  renderWorldCounters?: RenderWorldPerformanceCounters;
  renderQueueStats?: RenderSceneQueueStats;
  error?: string;
}

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
  readonly onError?: (message: string) => void;
}

export interface GpuWorldRuntime {
  stop(): void;
}

export async function startGpuWorldRuntime(options: GpuWorldRuntimeOptions): Promise<GpuWorldRuntime> {
  // WebGPU: browser requestAnimationFrame loop replaces Minecraft's main game loop ownership.
  const runtime = new BrowserGpuWorldRuntime(options);
  await runtime.start();
  return runtime;
}

class BrowserGpuWorldRuntime implements GpuWorldRuntime {
  private readonly input: GpuWorldInput;
  private depthTarget: ReturnType<typeof createSceneDepthTarget> | undefined;
  private camera: DebugCameraState;
  private lastFrameMs = performance.now();
  private lastLightTickMs = this.lastFrameMs;
  private lastWorldPollMs = this.lastFrameMs;
  private textureAnimationElapsedMs = 0.0;
  private frameRequest: number | undefined;
  private renderInFlight = false;
  private stopped = false;
  private lastChunkViewRequest: SetChunkViewRequest | undefined;

  public constructor(private readonly options: GpuWorldRuntimeOptions) {
    this.camera = options.initialCamera;
    this.input = new GpuWorldInput(options.canvas, {
      isGuiActive: () => options.screenManager.currentScreen !== null,
      openPauseMenu: () => this.openPauseMenu(),
    });
  }

  public async start(): Promise<void> {
    this.resizeViewport();
    this.lastChunkViewRequest = createChunkViewRequestForCameraState(this.camera, this.options.scene.viewDistance);
    this.updateState();
    if (this.options.initialFrame !== undefined) {
      await this.renderFrame(this.options.initialFrame);
    }

    this.options.state.ready = true;
    this.queueNextFrame();
  }

  public stop(): void {
    this.stopped = true;
    if (this.frameRequest !== undefined) {
      cancelAnimationFrame(this.frameRequest);
      this.frameRequest = undefined;
    }
    this.input.dispose();
    this.depthTarget?.texture.destroy();
    this.depthTarget = undefined;
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
      if (this.options.screenManager.currentScreen === null) {
        this.camera = applyFreeCameraInput(this.camera, this.input.consumeFrame(), dtSeconds);
      } else {
        this.input.consumeFrame();
      }

      const chunkViewRequest = createChunkViewRequestForCameraState(this.camera, scene.viewDistance);
      if (!isSameChunkViewRequest(this.lastChunkViewRequest, chunkViewRequest)) {
        if (await scene.clientRuntime.setChunkInterest(chunkViewRequest)) {
          applyRenderWorldDirtySections(scene);
        }
        this.lastChunkViewRequest = chunkViewRequest;
      }

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

      if (!this.renderInFlight) {
        this.renderInFlight = true;
        try {
          await this.renderFrame(await scene.gameRenderer.renderLevel(
            0.0,
            Number.MAX_SAFE_INTEGER,
            scene.levelRenderer,
            scene.lightTexture,
            this.camera,
            { waitForChunkTasks: false },
          ));
        } finally {
          this.renderInFlight = false;
        }
      }
    } catch (error) {
      const message = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
      this.options.state.error = message;
      this.options.onError?.(message);
      this.stop();
      return;
    }

    this.queueNextFrame();
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
    this.options.guiOverlayHost.encode(encoder, view, scene.format, scene.canvas.width, scene.canvas.height);
    scene.device.queue.submit([encoder.finish()]);
    await scene.device.queue.onSubmittedWorkDone();
    this.options.state.frameCount++;
    this.updateState();
  }

  private openPauseMenu(): void {
    if (this.options.screenManager.currentScreen !== null) {
      return;
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
  }
}

interface GpuWorldInputOptions {
  readonly isGuiActive: () => boolean;
  readonly openPauseMenu: () => void;
}

class GpuWorldInput {
  private readonly heldKeys = new Set<string>();
  private readonly listeners: Array<readonly [EventTarget, string, EventListener]> = [];
  private mouseDeltaX = 0;
  private mouseDeltaY = 0;
  private lastPointerX: number | undefined;
  private lastPointerY: number | undefined;
  private eventCount = 0;

  public constructor(canvas: HTMLCanvasElement, private readonly options: GpuWorldInputOptions) {
    this.add(window, "keydown", (event) => {
      const keyboardEvent = event as KeyboardEvent;
      if (keyboardEvent.defaultPrevented) {
        return;
      }
      if (keyboardEvent.key === "Escape" || keyboardEvent.code === "Escape") {
        if (!this.options.isGuiActive()) {
          this.clearGameplayInput();
          this.options.openPauseMenu();
          keyboardEvent.preventDefault();
          this.eventCount++;
        }
        return;
      }
      if (this.options.isGuiActive()) {
        this.clearGameplayInput();
        this.eventCount++;
        return;
      }
      if (isMovementKey(keyboardEvent.code)) {
        keyboardEvent.preventDefault();
      }
      this.heldKeys.add(keyboardEvent.code);
      this.eventCount++;
    });
    this.add(window, "keyup", (event) => {
      const keyboardEvent = event as KeyboardEvent;
      if (keyboardEvent.defaultPrevented) {
        return;
      }
      if (this.options.isGuiActive()) {
        this.heldKeys.delete(keyboardEvent.code);
        this.clearGameplayInput();
        this.eventCount++;
        return;
      }
      this.heldKeys.delete(keyboardEvent.code);
      this.eventCount++;
    });
    this.add(canvas, "pointermove", (event) => {
      const pointerEvent = event as PointerEvent;
      if (this.options.isGuiActive()) {
        this.lastPointerX = pointerEvent.clientX;
        this.lastPointerY = pointerEvent.clientY;
        this.mouseDeltaX = 0;
        this.mouseDeltaY = 0;
        this.eventCount++;
        return;
      }
      const movementX = pointerEvent.movementX !== 0 || this.lastPointerX === undefined
        ? pointerEvent.movementX
        : pointerEvent.clientX - this.lastPointerX;
      const movementY = pointerEvent.movementY !== 0 || this.lastPointerY === undefined
        ? pointerEvent.movementY
        : pointerEvent.clientY - this.lastPointerY;
      this.mouseDeltaX += movementX;
      this.mouseDeltaY += movementY;
      this.lastPointerX = pointerEvent.clientX;
      this.lastPointerY = pointerEvent.clientY;
      this.eventCount++;
    });
    this.add(canvas, "pointerleave", () => {
      this.lastPointerX = undefined;
      this.lastPointerY = undefined;
    });
  }

  public consumeFrame(): DebugInputFrame {
    if (this.options.isGuiActive()) {
      this.clearGameplayInput();
      return emptyInputFrame();
    }

    const frame: DebugInputFrame = {
      heldKeys: new Set(this.heldKeys),
      mouseDeltaX: this.mouseDeltaX,
      mouseDeltaY: this.mouseDeltaY,
      locked: this.heldKeys.size > 0 || this.mouseDeltaX !== 0 || this.mouseDeltaY !== 0,
      joystickX: 0,
      joystickY: 0,
      moveForward: false,
      moveBack: false,
      flyUp: false,
      flyDown: false,
    };
    this.mouseDeltaX = 0;
    this.mouseDeltaY = 0;
    return frame;
  }

  public getEventCount(): number {
    return this.eventCount;
  }

  public dispose(): void {
    for (const [target, type, listener] of this.listeners) {
      target.removeEventListener(type, listener);
    }
    this.listeners.length = 0;
    this.heldKeys.clear();
  }

  private add(target: EventTarget, type: string, listener: EventListener): void {
    target.addEventListener(type, listener);
    this.listeners.push([target, type, listener]);
  }

  private clearGameplayInput(): void {
    this.heldKeys.clear();
    this.mouseDeltaX = 0;
    this.mouseDeltaY = 0;
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

function isMovementKey(code: string): boolean {
  return code === "KeyW"
    || code === "KeyA"
    || code === "KeyS"
    || code === "KeyD"
    || code === "Space"
    || code === "ShiftLeft"
    || code === "ShiftRight"
    || code === "ControlLeft"
    || code === "ControlRight";
}
