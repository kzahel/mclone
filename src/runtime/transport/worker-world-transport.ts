import type { ClientChunkCache } from "../../world/level/client-chunk-cache";
import {
  clonePackedChunkLightDelta,
  clonePackedChunkSnapshot,
  collectPackedChunkLightDeltaTransferables,
  collectPackedChunkSnapshotTransferables,
} from "../../world/level/packed-chunk-snapshot";
import type { WorldHost } from "../protocol/world-host";
import type {
  OpenWorldRequest,
  PollWorldUpdatesRequest,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  WorldClientMessage,
  WorldHostMessage,
  WorldOpenedMessage,
} from "../protocol/world-messages";
import { TransportWorldClient, type TransportWorldClientOptions, type WorldTransport } from "./local-world-transport";

export interface WorldWorkerRequestEnvelope {
  readonly requestId: number;
  readonly message: WorldClientMessage;
}

export interface WorldWorkerResponseEnvelope {
  readonly requestId: number;
  readonly messages: readonly WorldHostMessage[];
}

export interface WorldWorkerMessageEvent<T> {
  readonly data: T;
}

export type WorldWorkerMessageListener<T> = (event: WorldWorkerMessageEvent<T>) => void;
type WorldWorkerErrorListener = (event: unknown) => void;

export interface WorldWorkerMessageEndpoint<TOutgoing, TIncoming> {
  postMessage(message: TOutgoing, transfer?: readonly Transferable[]): void;
  addEventListener(type: "message", listener: WorldWorkerMessageListener<TIncoming>): void;
  removeEventListener(type: "message", listener: WorldWorkerMessageListener<TIncoming>): void;
  start?(): void;
  close?(): void;
}

export interface WorldWorkerClientEndpoint extends WorldWorkerMessageEndpoint<WorldWorkerRequestEnvelope, WorldWorkerResponseEnvelope> {
  addEventListener(type: "message", listener: WorldWorkerMessageListener<WorldWorkerResponseEnvelope>): void;
  addEventListener(type: "error", listener: WorldWorkerErrorListener): void;
  addEventListener(type: "messageerror", listener: WorldWorkerErrorListener): void;
  removeEventListener(type: "message", listener: WorldWorkerMessageListener<WorldWorkerResponseEnvelope>): void;
  removeEventListener(type: "error", listener: WorldWorkerErrorListener): void;
  removeEventListener(type: "messageerror", listener: WorldWorkerErrorListener): void;
  terminate?(): void;
}

export interface WorldWorkerHostEndpoint extends WorldWorkerMessageEndpoint<WorldWorkerResponseEnvelope, WorldWorkerRequestEnvelope> {}

function formatUnknownError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function cloneHostMessagesForTransfer(messages: readonly WorldHostMessage[]): {
  readonly messages: readonly WorldHostMessage[];
  readonly transfer: readonly Transferable[];
} {
  const transfer: Transferable[] = [];
  const cloned = messages.map((message) => {
    switch (message.type) {
      case "chunk_snapshot": {
        const snapshot = clonePackedChunkSnapshot(message.snapshot);
        transfer.push(...collectPackedChunkSnapshotTransferables(snapshot));
        return {
          type: "chunk_snapshot",
          snapshot,
        } satisfies WorldHostMessage;
      }
      case "chunk_light_delta": {
        const light = clonePackedChunkLightDelta(message.light);
        transfer.push(...collectPackedChunkLightDeltaTransferables(light));
        return {
          type: "chunk_light_delta",
          chunkX: message.chunkX,
          chunkZ: message.chunkZ,
          light,
        } satisfies WorldHostMessage;
      }
      default:
        return message;
    }
  });

  return { messages: cloned, transfer };
}

export function connectWorldWorkerSession(
  endpoint: WorldWorkerHostEndpoint,
  hostFactory: (request: OpenWorldRequest) => WorldHost,
): void {
  let host: WorldHost | undefined;

  const onMessage: WorldWorkerMessageListener<WorldWorkerRequestEnvelope> = (event) => {
    void handleMessage(event.data);
  };

  async function handleMessage(envelope: WorldWorkerRequestEnvelope): Promise<void> {
    let messages: readonly WorldHostMessage[];

    try {
      switch (envelope.message.type) {
        case "open_world":
          host ??= hostFactory(envelope.message);
          messages = await host.openWorld(envelope.message);
          break;
        case "set_chunk_view":
          if (host === undefined) {
            messages = [{ type: "world_error", message: "set_chunk_view received before open_world" }];
            break;
          }

          messages = await host.setChunkView(envelope.message);
          break;
        case "set_player_input":
          if (host === undefined) {
            messages = [{ type: "world_error", message: "set_player_input received before open_world" }];
            break;
          }

          messages = await host.setPlayerInput(envelope.message);
          break;
        case "poll_world_updates":
          if (host === undefined) {
            messages = [{ type: "world_error", message: "poll_world_updates received before open_world" }];
            break;
          }

          messages = await host.pollUpdates(envelope.message);
          break;
      }
    } catch (error) {
      messages = [{ type: "world_error", message: formatUnknownError(error) }];
    }

    const prepared = cloneHostMessagesForTransfer(messages);
    endpoint.postMessage({
      requestId: envelope.requestId,
      messages: prepared.messages,
    }, prepared.transfer);
  }

  endpoint.addEventListener("message", onMessage);
  endpoint.start?.();
}

export class WorkerWorldTransport implements WorldTransport {
  private nextRequestId = 1;
  private readonly pending = new Map<number, {
    resolve: (messages: readonly WorldHostMessage[]) => void;
    reject: (error: unknown) => void;
  }>();

  private readonly onMessage: WorldWorkerMessageListener<WorldWorkerResponseEnvelope> = (event) => {
    const pending = this.pending.get(event.data.requestId);
    if (pending === undefined) {
      return;
    }

    this.pending.delete(event.data.requestId);
    pending.resolve(event.data.messages);
  };

  private readonly onError: WorldWorkerErrorListener = (event) => {
    const error = typeof ErrorEvent !== "undefined" && event instanceof ErrorEvent ? event.error ?? event.message : event;
    this.rejectAllPending(formatUnknownError(error));
  };

  public constructor(private readonly endpoint: WorldWorkerClientEndpoint) {
    endpoint.addEventListener("message", this.onMessage);
    endpoint.addEventListener("error", this.onError);
    endpoint.addEventListener("messageerror", this.onError);
  }

  public supportsChunkViewDeduplication(): boolean {
    return true;
  }

  public openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    return this.send(request);
  }

  public setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    return this.send(request);
  }

  public setPlayerInput(request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    return this.send(request);
  }

  public pollUpdates(request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]> {
    return this.send(request);
  }

  private send(message: WorldClientMessage): Promise<readonly WorldHostMessage[]> {
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

export class WorkerWorldClient extends TransportWorldClient {
  public constructor(
    transport: WorkerWorldTransport,
    levelFactory: (worldOpened: WorldOpenedMessage) => ClientChunkCache,
    options?: TransportWorldClientOptions,
  ) {
    super(transport, levelFactory, options);
  }
}

export function createGeneratedWorldWorker(): Worker {
  // Browser runtime: a module worker replaces vanilla's integrated singleplayer server thread here.
  return new Worker(new URL("../host/generated-world-worker.ts", import.meta.url), {
    type: "module",
    name: "mclone-generated-world-host",
  });
}
