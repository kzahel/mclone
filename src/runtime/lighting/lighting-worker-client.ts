import {
  collectLightingRequestTransferables,
  collectLightingResponseTransferables,
  type ConfigureLightingWorldRequest,
  type LightBlockChangeBatchRequest,
  type LightingService,
  type LightingWorkerRequest,
  type LightingWorkerResponse,
  type PollLightingResultsRequest,
  type RemoveLightChunkRequest,
  type RequestInitialLightRequest,
  type SetLightingViewRequest,
  type UpsertLightChunkRequest,
} from "./lighting-protocol";

export interface LightingWorkerRequestEnvelope {
  readonly requestId: number;
  readonly message: LightingWorkerRequest;
}

export interface LightingWorkerResponseEnvelope {
  readonly requestId: number;
  readonly message: LightingWorkerResponse;
}

export interface LightingWorkerMessageEvent<T> {
  readonly data: T;
}

export type LightingWorkerMessageListener<T> = (event: LightingWorkerMessageEvent<T>) => void;
export type LightingWorkerErrorListener = (event: unknown) => void;

export interface LightingWorkerMessageEndpoint<TOutgoing, TIncoming> {
  postMessage(message: TOutgoing, transfer?: readonly Transferable[]): void;
  addEventListener(type: "message", listener: LightingWorkerMessageListener<TIncoming>): void;
  removeEventListener(type: "message", listener: LightingWorkerMessageListener<TIncoming>): void;
  start?(): void;
  close?(): void;
}

export interface LightingWorkerClientEndpoint extends LightingWorkerMessageEndpoint<LightingWorkerRequestEnvelope, LightingWorkerResponseEnvelope> {
  addEventListener(type: "message", listener: LightingWorkerMessageListener<LightingWorkerResponseEnvelope>): void;
  addEventListener(type: "error", listener: LightingWorkerErrorListener): void;
  addEventListener(type: "messageerror", listener: LightingWorkerErrorListener): void;
  removeEventListener(type: "message", listener: LightingWorkerMessageListener<LightingWorkerResponseEnvelope>): void;
  removeEventListener(type: "error", listener: LightingWorkerErrorListener): void;
  removeEventListener(type: "messageerror", listener: LightingWorkerErrorListener): void;
  terminate?(): void;
}

export interface LightingWorkerHostEndpoint extends LightingWorkerMessageEndpoint<LightingWorkerResponseEnvelope, LightingWorkerRequestEnvelope> {}

export type LightingWorkerHandler = (message: LightingWorkerRequest) => Promise<LightingWorkerResponse>;

function formatUnknownError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function workerError(message: string): LightingWorkerResponse {
  return {
    type: "lighting_worker_error",
    message,
  };
}

function throwIfWorkerError(message: LightingWorkerResponse): void {
  if (message.type === "lighting_worker_error") {
    throw new Error(message.message);
  }
}

async function expectAck(message: Promise<LightingWorkerResponse>, requestType: string): Promise<void> {
  const response = await message;
  throwIfWorkerError(response);
  if (response.type !== "lighting_ack") {
    throw new Error(`Expected lighting_ack for ${requestType}, received ${response.type}`);
  }
}

export function connectLightingWorkerSession(
  endpoint: LightingWorkerHostEndpoint,
  handler: LightingWorkerHandler,
): void {
  const onMessage: LightingWorkerMessageListener<LightingWorkerRequestEnvelope> = (event) => {
    void handleMessage(event.data);
  };

  async function handleMessage(envelope: LightingWorkerRequestEnvelope): Promise<void> {
    let message: LightingWorkerResponse;

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
      collectLightingResponseTransferables(message),
    );
  }

  endpoint.addEventListener("message", onMessage);
  endpoint.start?.();
}

export class LightingWorkerClient implements LightingService {
  private nextRequestId = 1;
  private readonly pending = new Map<number, {
    resolve: (message: LightingWorkerResponse) => void;
    reject: (error: unknown) => void;
  }>();

  private readonly onMessage: LightingWorkerMessageListener<LightingWorkerResponseEnvelope> = (event) => {
    const pending = this.pending.get(event.data.requestId);
    if (pending === undefined) {
      return;
    }

    this.pending.delete(event.data.requestId);
    pending.resolve(event.data.message);
  };

  private readonly onError: LightingWorkerErrorListener = (event) => {
    const error = typeof ErrorEvent !== "undefined" && event instanceof ErrorEvent ? event.error ?? event.message : event;
    this.rejectAllPending(formatUnknownError(error));
  };

  public constructor(private readonly endpoint: LightingWorkerClientEndpoint) {
    endpoint.addEventListener("message", this.onMessage);
    endpoint.addEventListener("error", this.onError);
    endpoint.addEventListener("messageerror", this.onError);
  }

  public configureWorld(request: ConfigureLightingWorldRequest): Promise<void> {
    return expectAck(this.send(request), request.type);
  }

  public setView(request: SetLightingViewRequest): Promise<void> {
    return expectAck(this.send(request), request.type);
  }

  public upsertChunk(request: UpsertLightChunkRequest): Promise<void> {
    return expectAck(this.send(request), request.type);
  }

  public removeChunk(request: RemoveLightChunkRequest): Promise<void> {
    return expectAck(this.send(request), request.type);
  }

  public requestInitialLight(request: RequestInitialLightRequest): Promise<void> {
    return expectAck(this.send(request), request.type);
  }

  public enqueueBlockChanges(request: LightBlockChangeBatchRequest): Promise<void> {
    return expectAck(this.send(request), request.type);
  }

  public async pollResults(request: PollLightingResultsRequest): Promise<Extract<LightingWorkerResponse, { type: "lighting_result_batch" }>> {
    const response = await this.send(request);
    throwIfWorkerError(response);
    if (response.type !== "lighting_result_batch") {
      throw new Error(`Expected lighting_result_batch, received ${response.type}`);
    }

    return response;
  }

  public close(): void {
    this.rejectAllPending("lighting worker client closed");
    this.endpoint.removeEventListener("message", this.onMessage);
    this.endpoint.removeEventListener("error", this.onError);
    this.endpoint.removeEventListener("messageerror", this.onError);
    this.endpoint.terminate?.();
    this.endpoint.close?.();
  }

  private send(message: LightingWorkerRequest): Promise<LightingWorkerResponse> {
    const requestId = this.nextRequestId++;
    return new Promise((resolve, reject) => {
      this.pending.set(requestId, { resolve, reject });
      this.endpoint.postMessage(
        { requestId, message },
        collectLightingRequestTransferables(message),
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

export function createBrowserLightingWorker(): Worker {
  return new Worker(new URL("./lighting-worker.ts", import.meta.url), {
    type: "module",
    name: "mclone-lighting-worker",
  });
}

export function createBrowserLightingService(): LightingWorkerClient {
  return new LightingWorkerClient(createBrowserLightingWorker());
}
