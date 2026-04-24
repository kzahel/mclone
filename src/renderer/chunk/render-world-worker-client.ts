import type { RenderWorldChunkUpdateResult, RenderWorldUpdateMessage, RenderWorldUpdateSink } from "../../runtime/transport/local-world-transport";
import type { LoadingProgress, LoadingProgressSink } from "../loading-progress";
import {
  collectRenderWorldRequestTransferables,
  collectRenderWorldResponseTransferables,
  type BuildRenderSectionMeshRequest,
  type InitializeRenderWorldRequest,
  type IngestRenderWorldUpdatesRequest,
  type RenderWorldDirtySectionsResponse,
  type RenderWorldErrorResponse,
  type RenderWorldMeshBuildResponse,
  type RenderWorldProgressResponse,
  type RenderWorldReadyResponse,
  type RenderWorldRequest,
  type RenderWorldResponse,
  type RenderWorldSectionOrigin,
  type RenderWorldStats,
  type RenderWorldStatsResponse,
} from "./render-world-protocol";

const MAX_RENDER_WORLD_INGEST_TRANSFERABLES = 256;

function countRenderWorldUpdateTransferables(message: RenderWorldUpdateMessage): number {
  if (message.type === "chunk_snapshot") {
    return (message.snapshot.sections.length * 2)
      + (message.snapshot.light?.sky.length ?? 0)
      + (message.snapshot.light?.block.length ?? 0);
  }

  if (message.type === "chunk_light_delta") {
    return (message.light.sky?.filter((section) => section.data !== undefined).length ?? 0)
      + (message.light.block?.filter((section) => section.data !== undefined).length ?? 0);
  }

  return 0;
}

export interface RenderWorldWorkerRequestEnvelope {
  readonly requestId: number;
  readonly message: RenderWorldRequest;
}

export interface RenderWorldWorkerResponseEnvelope {
  readonly requestId: number;
  readonly message: RenderWorldResponse;
}

export interface RenderWorldWorkerMessageEvent<T> {
  readonly data: T;
}

export type RenderWorldWorkerMessageListener<T> = (event: RenderWorldWorkerMessageEvent<T>) => void;
type RenderWorldWorkerErrorListener = (event: unknown) => void;

export interface RenderWorldWorkerMessageEndpoint<TOutgoing, TIncoming> {
  postMessage(message: TOutgoing, transfer?: readonly Transferable[]): void;
  addEventListener(type: "message", listener: RenderWorldWorkerMessageListener<TIncoming>): void;
  removeEventListener(type: "message", listener: RenderWorldWorkerMessageListener<TIncoming>): void;
  start?(): void;
  close?(): void;
}

export interface RenderWorldWorkerClientEndpoint extends RenderWorldWorkerMessageEndpoint<RenderWorldWorkerRequestEnvelope, RenderWorldWorkerResponseEnvelope> {
  addEventListener(type: "message", listener: RenderWorldWorkerMessageListener<RenderWorldWorkerResponseEnvelope>): void;
  addEventListener(type: "error", listener: RenderWorldWorkerErrorListener): void;
  addEventListener(type: "messageerror", listener: RenderWorldWorkerErrorListener): void;
  removeEventListener(type: "message", listener: RenderWorldWorkerMessageListener<RenderWorldWorkerResponseEnvelope>): void;
  removeEventListener(type: "error", listener: RenderWorldWorkerErrorListener): void;
  removeEventListener(type: "messageerror", listener: RenderWorldWorkerErrorListener): void;
  terminate?(): void;
}

export interface RenderWorldWorkerHostEndpoint extends RenderWorldWorkerMessageEndpoint<RenderWorldWorkerResponseEnvelope, RenderWorldWorkerRequestEnvelope> {}

export interface RenderWorldWorkerPerformanceCounters {
  readonly ingestBatchCount: number;
  readonly meshBuildRequestCount: number;
  readonly meshNotReadyResponseCount: number;
  readonly meshCompletionCount: number;
}

function formatUnknownError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function workerError(message: string): RenderWorldErrorResponse {
  return {
    type: "render_world_error",
    message,
  };
}

function isRenderWorldReadyResponse(message: RenderWorldResponse): message is RenderWorldReadyResponse {
  return message.type === "render_world_ready";
}

function isRenderWorldStatsResponse(message: RenderWorldResponse): message is RenderWorldStatsResponse {
  return message.type === "render_world_stats";
}

function isRenderWorldDirtySectionsResponse(message: RenderWorldResponse): message is RenderWorldDirtySectionsResponse {
  return message.type === "render_world_dirty_sections";
}

function isRenderWorldMeshBuildResponse(message: RenderWorldResponse): message is RenderWorldMeshBuildResponse {
  return message.type === "render_section_mesh_built" || message.type === "render_world_mesh_not_ready";
}

function isRenderWorldProgressResponse(message: RenderWorldResponse): message is RenderWorldProgressResponse {
  return message.type === "render_world_progress";
}

function throwIfWorkerError(message: RenderWorldResponse): void {
  if (message.type === "render_world_error") {
    throw new Error(message.message);
  }
}

export function connectRenderWorldWorkerSession(
  endpoint: RenderWorldWorkerHostEndpoint,
  handler: (message: RenderWorldRequest, onProgress?: LoadingProgressSink) => Promise<RenderWorldResponse>,
): void {
  const onMessage: RenderWorldWorkerMessageListener<RenderWorldWorkerRequestEnvelope> = (event) => {
    void handleMessage(event.data);
  };

  async function handleMessage(envelope: RenderWorldWorkerRequestEnvelope): Promise<void> {
    let message: RenderWorldResponse;
    const onProgress = (progress: LoadingProgress): void => {
      endpoint.postMessage({
        requestId: envelope.requestId,
        message: {
          type: "render_world_progress",
          progress,
        },
      });
    };

    try {
      message = await handler(envelope.message, onProgress);
    } catch (error) {
      message = workerError(formatUnknownError(error));
    }

    endpoint.postMessage(
      {
        requestId: envelope.requestId,
        message,
      },
      collectRenderWorldResponseTransferables(message),
    );
  }

  endpoint.addEventListener("message", onMessage);
  endpoint.start?.();
}

export class RenderWorldWorkerClient {
  public readonly concurrency = 1;

  private readonly performanceCounters = {
    ingestBatchCount: 0,
    meshBuildRequestCount: 0,
    meshNotReadyResponseCount: 0,
    meshCompletionCount: 0,
  };
  private nextRequestId = 1;
  private readonly pending = new Map<number, {
    resolve: (message: RenderWorldResponse) => void;
    reject: (error: unknown) => void;
  }>();

  private readonly onMessage: RenderWorldWorkerMessageListener<RenderWorldWorkerResponseEnvelope> = (event) => {
    const pending = this.pending.get(event.data.requestId);
    if (pending === undefined) {
      return;
    }

    if (isRenderWorldProgressResponse(event.data.message)) {
      this.onProgress?.(event.data.message.progress);
      return;
    }

    this.pending.delete(event.data.requestId);
    pending.resolve(event.data.message);
  };

  private readonly onError: RenderWorldWorkerErrorListener = (event) => {
    const error = typeof ErrorEvent !== "undefined" && event instanceof ErrorEvent ? event.error ?? event.message : event;
    this.rejectAllPending(formatUnknownError(error));
  };

  public constructor(
    private readonly endpoint: RenderWorldWorkerClientEndpoint,
    private readonly onProgress?: LoadingProgressSink,
  ) {
    endpoint.addEventListener("message", this.onMessage);
    endpoint.addEventListener("error", this.onError);
    endpoint.addEventListener("messageerror", this.onError);
  }

  public async initialize(request: InitializeRenderWorldRequest): Promise<void> {
    const message = await this.send(request);
    throwIfWorkerError(message);

    if (!isRenderWorldReadyResponse(message)) {
      throw new Error(`Expected render_world_ready, received ${message.type}`);
    }
  }

  public async ingestUpdates(request: IngestRenderWorldUpdatesRequest): Promise<RenderWorldDirtySectionsResponse> {
    this.performanceCounters.ingestBatchCount++;
    const message = await this.send(request);
    throwIfWorkerError(message);

    if (!isRenderWorldDirtySectionsResponse(message)) {
      throw new Error(`Expected render_world_dirty_sections, received ${message.type}`);
    }

    return message;
  }

  public async buildSectionMesh(request: BuildRenderSectionMeshRequest): Promise<RenderWorldMeshBuildResponse> {
    this.performanceCounters.meshBuildRequestCount++;
    const message = await this.send(request);
    throwIfWorkerError(message);

    if (!isRenderWorldMeshBuildResponse(message)) {
      throw new Error(`Expected render_section_mesh_built or render_world_mesh_not_ready, received ${message.type}`);
    }

    if (message.type === "render_world_mesh_not_ready") {
      this.performanceCounters.meshNotReadyResponseCount++;
    } else {
      this.performanceCounters.meshCompletionCount++;
    }

    return message;
  }

  public async getStats(): Promise<RenderWorldStats> {
    const message = await this.send({ type: "get_render_world_stats" });
    throwIfWorkerError(message);

    if (!isRenderWorldStatsResponse(message)) {
      throw new Error(`Expected render_world_stats, received ${message.type}`);
    }

    return message.stats;
  }

  public getPerformanceCounters(): RenderWorldWorkerPerformanceCounters {
    return { ...this.performanceCounters };
  }

  public close(): void {
    this.rejectAllPending("render world worker client closed");
    this.endpoint.removeEventListener("message", this.onMessage);
    this.endpoint.removeEventListener("error", this.onError);
    this.endpoint.removeEventListener("messageerror", this.onError);
    this.endpoint.terminate?.();
    this.endpoint.close?.();
  }

  private send(message: RenderWorldRequest): Promise<RenderWorldResponse> {
    const requestId = this.nextRequestId++;
    return new Promise((resolve, reject) => {
      this.pending.set(requestId, { resolve, reject });
      this.endpoint.postMessage(
        { requestId, message },
        collectRenderWorldRequestTransferables(message),
      );
    });
  }

  private rejectAllPending(error: string): void {
    for (const pending of this.pending.values()) {
      pending.reject(new Error(error));
    }

    this.pending.clear();
  }
}

export class RenderWorldWorkerUpdateSink implements RenderWorldUpdateSink {
  private currentStats: RenderWorldStats = { loadedChunkCount: 0 };
  private readonly dirtySections: RenderWorldSectionOrigin[] = [];

  public constructor(private readonly client: RenderWorldWorkerClient) {}

  public async ingestUpdates(messages: readonly RenderWorldUpdateMessage[]): Promise<RenderWorldChunkUpdateResult> {
    if (messages.length <= 0) {
      return { chunkChanged: false };
    }

    let chunkChanged = false;
    let batch: RenderWorldUpdateMessage[] = [];
    let transferCount = 0;

    const flushBatch = async (): Promise<void> => {
      if (batch.length <= 0) {
        return;
      }

      const response = await this.client.ingestUpdates({
        type: "ingest_render_world_updates",
        messages: batch,
      });
      batch = [];
      transferCount = 0;
      this.currentStats = response.stats;
      this.dirtySections.push(...response.dirtySections);
      chunkChanged = response.dirtySections.length > 0 || chunkChanged;
    };

    for (const message of messages) {
      const messageTransferCount = countRenderWorldUpdateTransferables(message);
      if (batch.length > 0 && transferCount + messageTransferCount > MAX_RENDER_WORLD_INGEST_TRANSFERABLES) {
        await flushBatch();
      }

      batch.push(message);
      transferCount += messageTransferCount;
    }

    await flushBatch();
    return {
      chunkChanged,
    };
  }

  public getStats(): RenderWorldStats {
    return this.currentStats;
  }

  public getPerformanceCounters(): RenderWorldWorkerPerformanceCounters {
    return this.client.getPerformanceCounters();
  }

  public drainDirtySections(): readonly RenderWorldSectionOrigin[] {
    const drained = [...this.dirtySections];
    this.dirtySections.length = 0;
    return drained;
  }
}

export function createRenderWorldWorker(): Worker {
  // Browser runtime: render-world cache ownership moves off the main thread while WebGPU upload stays main-thread-owned.
  return new Worker(new URL("./render-world-worker.ts", import.meta.url), {
    type: "module",
    name: "mclone-render-world-worker",
  });
}
