import {
  collectRenderWorldRequestTransferables,
  collectRenderWorldResponseTransferables,
  type BuildRenderSectionMeshRequest,
  type InitializeRenderWorldRequest,
  type IngestRenderWorldUpdatesRequest,
  type RenderWorldDirtySectionsResponse,
  type RenderWorldErrorResponse,
  type RenderWorldMeshBuildResponse,
  type RenderWorldReadyResponse,
  type RenderWorldRequest,
  type RenderWorldResponse,
  type RenderWorldStats,
  type RenderWorldStatsResponse,
} from "./render-world-protocol";

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

function throwIfWorkerError(message: RenderWorldResponse): void {
  if (message.type === "render_world_error") {
    throw new Error(message.message);
  }
}

export function connectRenderWorldWorkerSession(
  endpoint: RenderWorldWorkerHostEndpoint,
  handler: (message: RenderWorldRequest) => Promise<RenderWorldResponse>,
): void {
  const onMessage: RenderWorldWorkerMessageListener<RenderWorldWorkerRequestEnvelope> = (event) => {
    void handleMessage(event.data);
  };

  async function handleMessage(envelope: RenderWorldWorkerRequestEnvelope): Promise<void> {
    let message: RenderWorldResponse;

    try {
      message = await handler(envelope.message);
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

    this.pending.delete(event.data.requestId);
    pending.resolve(event.data.message);
  };

  private readonly onError: RenderWorldWorkerErrorListener = (event) => {
    const error = typeof ErrorEvent !== "undefined" && event instanceof ErrorEvent ? event.error ?? event.message : event;
    this.rejectAllPending(formatUnknownError(error));
  };

  public constructor(private readonly endpoint: RenderWorldWorkerClientEndpoint) {
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
    const message = await this.send(request);
    throwIfWorkerError(message);

    if (!isRenderWorldDirtySectionsResponse(message)) {
      throw new Error(`Expected render_world_dirty_sections, received ${message.type}`);
    }

    return message;
  }

  public async buildSectionMesh(request: BuildRenderSectionMeshRequest): Promise<RenderWorldMeshBuildResponse> {
    const message = await this.send(request);
    throwIfWorkerError(message);

    if (!isRenderWorldMeshBuildResponse(message)) {
      throw new Error(`Expected render_section_mesh_built or render_world_mesh_not_ready, received ${message.type}`);
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
