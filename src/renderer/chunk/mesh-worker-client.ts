import {
  collectSectionMeshTransferables,
  type BuildSectionMeshRequest,
  type InitializeMeshWorkerRequest,
  type MeshWorkerResponse,
  type MeshWorkerRequest,
  type MeshWorkerErrorResponse,
  type MeshWorkerReadyResponse,
  type SectionMeshBuiltResponse,
  type SectionMeshResult,
} from "./chunk-mesh-protocol";

export interface MeshWorkerRequestEnvelope {
  readonly requestId: number;
  readonly message: MeshWorkerRequest;
}

export interface MeshWorkerResponseEnvelope {
  readonly requestId: number;
  readonly message: MeshWorkerResponse;
}

export interface MeshWorkerMessageEvent<T> {
  readonly data: T;
}

export type MeshWorkerMessageListener<T> = (event: MeshWorkerMessageEvent<T>) => void;
type MeshWorkerErrorListener = (event: unknown) => void;

export interface MeshWorkerMessageEndpoint<TOutgoing, TIncoming> {
  postMessage(message: TOutgoing, transfer?: readonly Transferable[]): void;
  addEventListener(type: "message", listener: MeshWorkerMessageListener<TIncoming>): void;
  removeEventListener(type: "message", listener: MeshWorkerMessageListener<TIncoming>): void;
  start?(): void;
  close?(): void;
}

export interface MeshWorkerClientEndpoint extends MeshWorkerMessageEndpoint<MeshWorkerRequestEnvelope, MeshWorkerResponseEnvelope> {
  addEventListener(type: "message", listener: MeshWorkerMessageListener<MeshWorkerResponseEnvelope>): void;
  addEventListener(type: "error", listener: MeshWorkerErrorListener): void;
  addEventListener(type: "messageerror", listener: MeshWorkerErrorListener): void;
  removeEventListener(type: "message", listener: MeshWorkerMessageListener<MeshWorkerResponseEnvelope>): void;
  removeEventListener(type: "error", listener: MeshWorkerErrorListener): void;
  removeEventListener(type: "messageerror", listener: MeshWorkerErrorListener): void;
  terminate?(): void;
}

export interface MeshWorkerHostEndpoint extends MeshWorkerMessageEndpoint<MeshWorkerResponseEnvelope, MeshWorkerRequestEnvelope> {}

function formatUnknownError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function workerError(message: string): MeshWorkerErrorResponse {
  return {
    type: "mesh_worker_error",
    message,
  };
}

function isSectionMeshBuiltResponse(message: MeshWorkerResponse): message is SectionMeshBuiltResponse {
  return message.type === "section_mesh_built";
}

function isMeshWorkerReadyResponse(message: MeshWorkerResponse): message is MeshWorkerReadyResponse {
  return message.type === "mesh_worker_ready";
}

function transferablesForResponse(message: MeshWorkerResponse): readonly Transferable[] {
  return isSectionMeshBuiltResponse(message) ? collectSectionMeshTransferables(message.result) : [];
}

export function connectMeshWorkerSession(
  endpoint: MeshWorkerHostEndpoint,
  handler: (message: MeshWorkerRequest) => Promise<MeshWorkerResponse>,
): void {
  const onMessage: MeshWorkerMessageListener<MeshWorkerRequestEnvelope> = (event) => {
    void handleMessage(event.data);
  };

  async function handleMessage(envelope: MeshWorkerRequestEnvelope): Promise<void> {
    let message: MeshWorkerResponse;

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
      transferablesForResponse(message),
    );
  }

  endpoint.addEventListener("message", onMessage);
  endpoint.start?.();
}

export class ChunkMeshWorkerClient {
  public readonly concurrency = 1;

  private nextRequestId = 1;
  private readonly pending = new Map<number, {
    resolve: (message: MeshWorkerResponse) => void;
    reject: (error: unknown) => void;
  }>();

  private readonly onMessage: MeshWorkerMessageListener<MeshWorkerResponseEnvelope> = (event) => {
    const pending = this.pending.get(event.data.requestId);
    if (pending === undefined) {
      return;
    }

    this.pending.delete(event.data.requestId);
    pending.resolve(event.data.message);
  };

  private readonly onError: MeshWorkerErrorListener = (event) => {
    const error = typeof ErrorEvent !== "undefined" && event instanceof ErrorEvent ? event.error ?? event.message : event;
    this.rejectAllPending(formatUnknownError(error));
  };

  public constructor(private readonly endpoint: MeshWorkerClientEndpoint) {
    endpoint.addEventListener("message", this.onMessage);
    endpoint.addEventListener("error", this.onError);
    endpoint.addEventListener("messageerror", this.onError);
  }

  public async initialize(request: InitializeMeshWorkerRequest): Promise<void> {
    const message = await this.send(request);
    if (message.type === "mesh_worker_error") {
      throw new Error(message.message);
    }

    if (!isMeshWorkerReadyResponse(message)) {
      throw new Error(`Expected mesh_worker_ready, received ${message.type}`);
    }
  }

  public async buildSectionMesh(request: BuildSectionMeshRequest): Promise<SectionMeshResult> {
    const message = await this.send(request);
    if (message.type === "mesh_worker_error") {
      throw new Error(message.message);
    }

    if (!isSectionMeshBuiltResponse(message)) {
      throw new Error(`Expected section_mesh_built, received ${message.type}`);
    }

    return message.result;
  }

  public close(): void {
    this.rejectAllPending("mesh worker client closed");
    this.endpoint.removeEventListener("message", this.onMessage);
    this.endpoint.removeEventListener("error", this.onError);
    this.endpoint.removeEventListener("messageerror", this.onError);
    this.endpoint.terminate?.();
    this.endpoint.close?.();
  }

  private send(message: MeshWorkerRequest): Promise<MeshWorkerResponse> {
    const requestId = this.nextRequestId++;
    return new Promise((resolve, reject) => {
      this.pending.set(requestId, { resolve, reject });
      this.endpoint.postMessage({ requestId, message });
    });
  }

  private rejectAllPending(error: string): void {
    for (const pending of this.pending.values()) {
      pending.reject(new Error(error));
    }

    this.pending.clear();
  }
}

export function createChunkMeshWorker(): Worker {
  // Browser runtime: CPU chunk meshing moves off the render thread, while GPU uploads stay on the main thread.
  return new Worker(new URL("./mesh-worker.ts", import.meta.url), {
    type: "module",
    name: "mclone-chunk-mesh-worker",
  });
}
