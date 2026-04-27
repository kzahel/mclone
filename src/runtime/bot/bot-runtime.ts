import { SectionPos } from "../../core/section-pos";
import { MovementCommandClock } from "../movement/movement-command-clock";
import type { ClientPresentationState, ClientRuntime } from "../client/client-runtime";
import type { ClientWorldRevisionFacts } from "../client/client-world";
import {
  PLAYER_COLLISION_REVISION,
  PLAYER_COMMAND_QUANTUM_US,
  PLAYER_MAX_COMMAND_STEP_COUNT,
  PLAYER_MOVEMENT_PHYSICS_REVISION,
} from "../session/player-loop";
import type {
  ClientPlayerState,
  ClientSessionState,
  OpenWorldRequest,
  PlayerInputCommand,
  SetChunkViewRequest,
  WorldOpenedMessage,
} from "../protocol/world-messages";

export interface BotMovementIntent {
  readonly moveX: number;
  readonly moveZ: number;
  readonly yaw: number;
  readonly pitch: number;
  readonly buttons?: number;
  readonly edgeButtons?: number;
}

export interface BotObservation {
  readonly sessionState?: ClientSessionState;
  readonly playerState?: ClientPlayerState;
  readonly presentation: ClientPresentationState;
  readonly revisionFacts: ClientWorldRevisionFacts;
  readonly loadedChunkCount: number;
  readonly entityCount: number;
}

export interface BotControllerTickContext {
  readonly observation: BotObservation;
  readonly elapsedMs: number;
  readonly nowMs: number;
}

export interface BotControllerTickResult {
  readonly movement?: BotMovementIntent;
  readonly status?: string;
}

export interface BotController {
  onStart?(context: BotControllerTickContext): Promise<void> | void;
  tick(context: BotControllerTickContext): Promise<BotControllerTickResult> | BotControllerTickResult;
  onStop?(): Promise<void> | void;
}

export interface BotLogger {
  info(message: string): void;
  error?(message: string): void;
}

export interface BotRuntimeOptions {
  readonly clientRuntime: ClientRuntime;
  readonly openWorldRequest: OpenWorldRequest;
  readonly viewRadius?: number;
  readonly controller?: BotController;
  readonly logger?: BotLogger;
  readonly nowMs?: () => number;
  readonly commandQuantumUs?: number;
}

export interface BotRuntimeTickResult {
  readonly commandCount: number;
  readonly chunkInterestChanged: boolean;
  readonly transportChanged: boolean;
  readonly status?: string;
}

export interface BotRuntimeStatus {
  readonly opened: boolean;
  readonly closed: boolean;
  readonly tickCount: number;
  readonly nextInputSequence: number;
  readonly lastChunkView?: SetChunkViewRequest;
  readonly lastInput?: PlayerInputCommand;
}

export interface RunBotUntilStoppedOptions {
  readonly tickIntervalMs?: number;
  readonly maxTicks?: number;
  readonly signal?: AbortSignal;
}

const DEFAULT_VIEW_RADIUS = 2;
const DEFAULT_TICK_INTERVAL_MS = 50;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function clampAxis(value: number): number {
  return Number.isFinite(value) ? Math.max(-1.0, Math.min(1.0, value)) : 0.0;
}

function clampPitch(value: number): number {
  return Number.isFinite(value) ? Math.max(-90.0, Math.min(90.0, value)) : 0.0;
}

function normalizeDegrees(value: number): number {
  if (!Number.isFinite(value)) {
    return 0.0;
  }
  const normalized = value % 360.0;
  return normalized < 0.0 ? normalized + 360.0 : normalized;
}

function isSameChunkView(left: SetChunkViewRequest | undefined, right: SetChunkViewRequest): boolean {
  return left !== undefined
    && left.centerChunkX === right.centerChunkX
    && left.centerChunkZ === right.centerChunkZ
    && left.radius === right.radius;
}

function createChunkViewRequestForPlayerState(playerState: ClientPlayerState, radius: number): SetChunkViewRequest {
  return {
    type: "set_chunk_view",
    centerChunkX: SectionPos.posToSectionCoord(playerState.position.x),
    centerChunkZ: SectionPos.posToSectionCoord(playerState.position.z),
    radius,
  };
}

function defaultMovementIntent(playerState: ClientPlayerState | undefined): BotMovementIntent {
  return {
    moveX: 0.0,
    moveZ: 0.0,
    yaw: playerState?.rotation.yaw ?? 0.0,
    pitch: playerState?.rotation.pitch ?? 0.0,
    buttons: 0,
    edgeButtons: 0,
  };
}

export class IdleBotController implements BotController {
  public tick(context: BotControllerTickContext): BotControllerTickResult {
    return {
      movement: defaultMovementIntent(context.observation.playerState),
      status: "idle",
    };
  }
}

export interface WanderBotControllerOptions {
  readonly yawDegreesPerSecond?: number;
  readonly moveZ?: number;
}

export class WanderBotController implements BotController {
  private yaw: number | undefined;

  public constructor(private readonly options: WanderBotControllerOptions = {}) {}

  public tick(context: BotControllerTickContext): BotControllerTickResult {
    const playerState = context.observation.playerState;
    const yawDegreesPerSecond = this.options.yawDegreesPerSecond ?? 12.0;
    this.yaw = normalizeDegrees(
      (this.yaw ?? playerState?.rotation.yaw ?? 0.0)
      + yawDegreesPerSecond * (context.elapsedMs / 1000.0),
    );
    return {
      movement: {
        moveX: 0.0,
        moveZ: this.options.moveZ ?? 1.0,
        yaw: this.yaw,
        pitch: playerState?.rotation.pitch ?? 0.0,
        buttons: 0,
        edgeButtons: 0,
      },
      status: "wander",
    };
  }
}

export class BotRuntime {
  private readonly viewRadius: number;
  private readonly controller: BotController;
  private readonly nowMs: () => number;
  private readonly commandClock: MovementCommandClock;
  private opened = false;
  private closed = false;
  private tickCount = 0;
  private nextInputSequence = 1;
  private lastChunkView: SetChunkViewRequest | undefined;
  private lastInput: PlayerInputCommand | undefined;
  private lastNowMs: number | undefined;

  public constructor(private readonly options: BotRuntimeOptions) {
    this.viewRadius = options.viewRadius ?? DEFAULT_VIEW_RADIUS;
    this.controller = options.controller ?? new IdleBotController();
    this.nowMs = options.nowMs ?? (() => (typeof performance === "undefined" ? Date.now() : performance.now()));
    this.commandClock = new MovementCommandClock({
      commandQuantumUs: options.commandQuantumUs ?? PLAYER_COMMAND_QUANTUM_US,
      maxStepCountPerCommand: PLAYER_MAX_COMMAND_STEP_COUNT,
    });
  }

  public async open(): Promise<WorldOpenedMessage> {
    this.assertNotClosed("open");
    if (this.opened) {
      throw new Error("BotRuntime.open() called after the bot was already opened");
    }

    const opened = await this.options.clientRuntime.openWorld(this.options.openWorldRequest);
    this.opened = true;
    this.lastNowMs = this.nowMs();
    const observation = this.observe();
    await this.controller.onStart?.({
      observation,
      elapsedMs: 0,
      nowMs: this.lastNowMs,
    });
    await this.updateChunkInterest(observation.playerState);
    this.options.logger?.info(`bot ${this.options.openWorldRequest.playerProfile?.name ?? "Bot"} joined save ${opened.saveMetadata.saveId}`);
    return opened;
  }

  public async tick(elapsedMs?: number): Promise<BotRuntimeTickResult> {
    this.assertOpen("tick");
    const nowMs = this.nowMs();
    const frameElapsedMs = Math.max(0, elapsedMs ?? (this.lastNowMs === undefined ? DEFAULT_TICK_INTERVAL_MS : nowMs - this.lastNowMs));
    this.lastNowMs = nowMs;

    const transportChanged = await this.options.clientRuntime.drainTransportUpdates();
    const observation = this.observe();
    const chunkInterestChanged = await this.updateChunkInterest(observation.playerState);
    const controllerResult = await this.controller.tick({
      observation,
      elapsedMs: frameElapsedMs,
      nowMs,
    });
    const movement = controllerResult.movement ?? defaultMovementIntent(observation.playerState);
    const commandCount = await this.sendMovementCommands(movement, frameElapsedMs, nowMs);
    this.tickCount++;

    return {
      commandCount,
      chunkInterestChanged,
      transportChanged,
      status: controllerResult.status,
    };
  }

  public async runUntilStopped(options: RunBotUntilStoppedOptions = {}): Promise<void> {
    this.assertOpen("runUntilStopped");
    const tickIntervalMs = options.tickIntervalMs ?? DEFAULT_TICK_INTERVAL_MS;
    let remainingTicks = options.maxTicks;
    while (!this.closed && options.signal?.aborted !== true) {
      if (remainingTicks !== undefined) {
        if (remainingTicks <= 0) {
          return;
        }
        remainingTicks--;
      }

      await this.tick(tickIntervalMs);
      await sleep(tickIntervalMs);
    }
  }

  public async close(): Promise<void> {
    if (this.closed) {
      return;
    }

    this.closed = true;
    await this.controller.onStop?.();
    this.options.clientRuntime.close();
  }

  public getStatus(): BotRuntimeStatus {
    return {
      opened: this.opened,
      closed: this.closed,
      tickCount: this.tickCount,
      nextInputSequence: this.nextInputSequence,
      lastChunkView: this.lastChunkView,
      lastInput: this.lastInput,
    };
  }

  public observe(): BotObservation {
    const presentation = this.options.clientRuntime.publishPresentationState();
    const clientWorld = this.options.clientRuntime.getClientWorld();
    return {
      sessionState: presentation.sessionState,
      playerState: presentation.localPlayerState,
      presentation,
      revisionFacts: clientWorld.getRevisionFacts(),
      loadedChunkCount: clientWorld.getRenderView().getRenderLevel().getLoadedChunkCount(),
      entityCount: presentation.entities.length,
    };
  }

  private async updateChunkInterest(playerState: ClientPlayerState | undefined): Promise<boolean> {
    if (playerState === undefined) {
      return false;
    }

    const request = createChunkViewRequestForPlayerState(playerState, this.viewRadius);
    if (isSameChunkView(this.lastChunkView, request)) {
      return false;
    }

    await this.options.clientRuntime.setChunkInterest(request);
    this.lastChunkView = request;
    return true;
  }

  private async sendMovementCommands(
    movement: BotMovementIntent,
    elapsedMs: number,
    nowMs: number,
  ): Promise<number> {
    const stepCounts = this.commandClock.consumeElapsedUs(Math.max(0, Math.round(elapsedMs * 1000.0)));
    let commandCount = 0;
    let edgeButtons = movement.edgeButtons ?? 0;
    for (const stepCount of stepCounts) {
      const input: PlayerInputCommand = {
        sequence: this.nextInputSequence++,
        moveX: clampAxis(movement.moveX),
        moveY: 0.0,
        moveZ: clampAxis(movement.moveZ),
        yaw: normalizeDegrees(movement.yaw),
        pitch: clampPitch(movement.pitch),
        clientTimeUs: Math.max(0, Math.round(nowMs * 1000.0)),
        commandQuantumUs: this.commandClock.commandQuantumUs,
        stepCount,
        buttons: movement.buttons ?? 0,
        edgeButtons,
        physicsRevision: PLAYER_MOVEMENT_PHYSICS_REVISION,
        collisionRevision: PLAYER_COLLISION_REVISION,
      };
      edgeButtons = 0;
      await this.options.clientRuntime.sendPlayerCommand({
        type: "set_player_input",
        input,
      });
      this.lastInput = input;
      commandCount++;
    }
    return commandCount;
  }

  private assertOpen(operation: string): void {
    this.assertNotClosed(operation);
    if (!this.opened) {
      throw new Error(`BotRuntime.${operation}() called before open()`);
    }
  }

  private assertNotClosed(operation: string): void {
    if (this.closed) {
      throw new Error(`BotRuntime.${operation}() called after close()`);
    }
  }
}
