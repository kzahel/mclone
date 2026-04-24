import {
  type ConfigureLightingWorldRequest,
  type LightBlockChangeBatchRequest,
  type LightErrorResult,
  type LightProgressResult,
  type LightingCommandRequest,
  type LightingResult,
  type LightingWorkerRequest,
  type LightingWorkerResponse,
  type RemoveLightChunkRequest,
  type RequestInitialLightRequest,
  type SetLightingViewRequest,
  type UpsertLightChunkRequest,
} from "./lighting-protocol";
import { connectLightingWorkerSession, type LightingWorkerHostEndpoint } from "./lighting-worker-client";

const DEFAULT_MAX_QUEUED_COMMANDS = 1024;
const DEFAULT_MAX_QUEUED_RESULTS = 1024;

export interface LightingWorkerHandlerOptions {
  readonly maxQueuedCommands?: number;
  readonly maxQueuedResults?: number;
}

interface LightingWorkerState {
  configuredWorld?: ConfigureLightingWorldRequest;
  view?: SetLightingViewRequest;
  readonly chunks: Map<string, UpsertLightChunkRequest>;
}

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

function yieldMailboxTurn(): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, 0);
  });
}

function lightError(message: string, request?: RequestInitialLightRequest | RemoveLightChunkRequest): LightErrorResult {
  return request === undefined
    ? { type: "light_error", message }
    : {
        type: "light_error",
        message,
        chunkViewRevision: request.chunkViewRevision,
        chunkX: request.chunkX,
        chunkZ: request.chunkZ,
        chunkRevision: request.chunkRevision,
      };
}

function lightProgress(
  stage: string,
  current: number,
  total: number,
  request?: RequestInitialLightRequest,
): LightProgressResult {
  return request === undefined
    ? { type: "light_progress", stage, current, total }
    : {
        type: "light_progress",
        stage,
        current,
        total,
        chunkViewRevision: request.chunkViewRevision,
        chunkX: request.chunkX,
        chunkZ: request.chunkZ,
        chunkRevision: request.chunkRevision,
      };
}

class LightingWorkerMailbox {
  private readonly maxQueuedCommands: number;
  private readonly maxQueuedResults: number;
  private readonly queuedCommands: LightingCommandRequest[] = [];
  private readonly resultQueue: LightingResult[] = [];
  private readonly state: LightingWorkerState = {
    chunks: new Map(),
  };
  private activeCommandCount = 0;
  private drainingCommands = false;

  public constructor(options: LightingWorkerHandlerOptions = {}) {
    this.maxQueuedCommands = options.maxQueuedCommands ?? DEFAULT_MAX_QUEUED_COMMANDS;
    this.maxQueuedResults = options.maxQueuedResults ?? DEFAULT_MAX_QUEUED_RESULTS;
  }

  public enqueueCommand(message: LightingCommandRequest): void {
    if (this.activeCommandCount >= this.maxQueuedCommands) {
      throw new Error(`lighting mailbox full (${this.activeCommandCount.toString()}/${this.maxQueuedCommands.toString()} commands)`);
    }

    this.activeCommandCount++;
    this.queuedCommands.push(message);
    if (!this.drainingCommands) {
      this.drainingCommands = true;
      queueMicrotask(() => {
        void this.drainCommands();
      });
    }
  }

  public pollResults(maxResults: number | undefined): LightingWorkerResponse {
    const resultLimit = maxResults === undefined ? this.resultQueue.length : Math.max(0, maxResults);
    const results = this.resultQueue.splice(0, resultLimit);
    return {
      type: "lighting_result_batch",
      results,
      pendingResultCount: this.resultQueue.length,
    };
  }

  private async drainCommands(): Promise<void> {
    try {
      while (this.queuedCommands.length > 0) {
        const command = this.queuedCommands.shift()!;
        try {
          this.processCommand(command);
        } catch (error) {
          this.enqueueResult(lightError(error instanceof Error ? error.message : String(error)));
        } finally {
          this.activeCommandCount--;
        }

        await yieldMailboxTurn();
      }
    } finally {
      this.drainingCommands = false;
      if (this.queuedCommands.length > 0) {
        this.drainingCommands = true;
        queueMicrotask(() => {
          void this.drainCommands();
        });
      }
    }
  }

  private processCommand(command: LightingCommandRequest): void {
    switch (command.type) {
      case "configure_light_world":
        this.state.configuredWorld = command;
        this.state.chunks.clear();
        this.enqueueResult(lightProgress("configured", 1, 1));
        break;
      case "set_light_view":
        this.state.view = command;
        this.enqueueResult(lightProgress("view_updated", 1, 1));
        break;
      case "upsert_light_chunk":
        this.processUpsertChunk(command);
        break;
      case "remove_light_chunk":
        this.processRemoveChunk(command);
        break;
      case "request_initial_light":
        this.processInitialLightRequest(command);
        break;
      case "block_light_update_batch":
        this.processBlockLightUpdateBatch(command);
        break;
    }
  }

  private processUpsertChunk(command: UpsertLightChunkRequest): void {
    this.state.chunks.set(chunkKey(command.chunkX, command.chunkZ), command);
  }

  private processRemoveChunk(command: RemoveLightChunkRequest): void {
    const key = chunkKey(command.chunkX, command.chunkZ);
    const existing = this.state.chunks.get(key);
    if (existing !== undefined && existing.chunkRevision === command.chunkRevision) {
      this.state.chunks.delete(key);
    }
  }

  private processInitialLightRequest(command: RequestInitialLightRequest): void {
    if (this.state.configuredWorld === undefined) {
      this.enqueueResult(lightError("request_initial_light received before configure_light_world", command));
      return;
    }

    const required = [
      {
        chunkX: command.chunkX,
        chunkZ: command.chunkZ,
        chunkRevision: command.chunkRevision,
      },
      ...command.neighbors,
    ];
    let ready = 0;
    for (const dependency of required) {
      const chunk = this.state.chunks.get(chunkKey(dependency.chunkX, dependency.chunkZ));
      if (chunk !== undefined && chunk.chunkRevision === dependency.chunkRevision) {
        ready++;
      }
    }

    this.enqueueResult(lightProgress(
      ready === required.length ? "initial_light_ready_for_solver" : "initial_light_waiting_for_inputs",
      ready,
      required.length,
      command,
    ));
  }

  private processBlockLightUpdateBatch(command: LightBlockChangeBatchRequest): void {
    this.enqueueResult(lightProgress("block_light_updates_queued", command.changes.length, command.changes.length));
  }

  private enqueueResult(result: LightingResult): void {
    if (this.maxQueuedResults <= 0) {
      return;
    }

    while (this.resultQueue.length >= this.maxQueuedResults) {
      const progressIndex = this.resultQueue.findIndex((queued) => queued.type === "light_progress");
      this.resultQueue.splice(progressIndex >= 0 ? progressIndex : 0, 1);
    }

    this.resultQueue.push(result);
  }
}

export function createLightingWorkerHandler(options: LightingWorkerHandlerOptions = {}): (message: LightingWorkerRequest) => Promise<LightingWorkerResponse> {
  const mailbox = new LightingWorkerMailbox(options);

  return async (message) => {
    if (message.type === "poll_light_results") {
      return mailbox.pollResults(message.maxResults);
    }

    mailbox.enqueueCommand(message);
    return { type: "lighting_ack" };
  };
}

const maybeWorkerGlobal = globalThis as Partial<LightingWorkerHostEndpoint>;
if (typeof maybeWorkerGlobal.postMessage === "function" && typeof maybeWorkerGlobal.addEventListener === "function") {
  connectLightingWorkerSession(
    maybeWorkerGlobal as LightingWorkerHostEndpoint,
    createLightingWorkerHandler(),
  );
}
