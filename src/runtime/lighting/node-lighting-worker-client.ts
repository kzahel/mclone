import { Worker as NodeWorker } from "node:worker_threads";
import {
  LightingWorkerClient,
  type LightingWorkerClientEndpoint,
  type LightingWorkerErrorListener,
  type LightingWorkerMessageListener,
  type LightingWorkerRequestEnvelope,
  type LightingWorkerResponseEnvelope,
} from "./lighting-worker-client";

class NodeLightingWorkerClientEndpoint implements LightingWorkerClientEndpoint {
  private readonly messageListeners = new Map<
    LightingWorkerMessageListener<LightingWorkerResponseEnvelope>,
    (message: LightingWorkerResponseEnvelope) => void
  >();
  private readonly errorListeners = new Map<LightingWorkerErrorListener, (error: Error) => void>();

  public constructor(private readonly worker: NodeWorker) {}

  public postMessage(message: LightingWorkerRequestEnvelope, transfer: readonly Transferable[] = []): void {
    this.worker.postMessage(message, transfer as Parameters<NodeWorker["postMessage"]>[1]);
  }

  public addEventListener(type: "message", listener: LightingWorkerMessageListener<LightingWorkerResponseEnvelope>): void;
  public addEventListener(type: "error" | "messageerror", listener: LightingWorkerErrorListener): void;
  public addEventListener(
    type: "message" | "error" | "messageerror",
    listener: LightingWorkerMessageListener<LightingWorkerResponseEnvelope> | LightingWorkerErrorListener,
  ): void {
    if (type === "message") {
      const wrapped = (message: LightingWorkerResponseEnvelope): void => {
        (listener as LightingWorkerMessageListener<LightingWorkerResponseEnvelope>)({ data: message });
      };
      this.messageListeners.set(listener as LightingWorkerMessageListener<LightingWorkerResponseEnvelope>, wrapped);
      this.worker.on(type, wrapped);
      return;
    }

    if (type === "error") {
      const wrapped = (error: Error): void => {
        (listener as LightingWorkerErrorListener)(error);
      };
      this.errorListeners.set(listener as LightingWorkerErrorListener, wrapped);
      this.worker.on(type, wrapped);
    }
  }

  public removeEventListener(type: "message", listener: LightingWorkerMessageListener<LightingWorkerResponseEnvelope>): void;
  public removeEventListener(type: "error" | "messageerror", listener: LightingWorkerErrorListener): void;
  public removeEventListener(
    type: "message" | "error" | "messageerror",
    listener: LightingWorkerMessageListener<LightingWorkerResponseEnvelope> | LightingWorkerErrorListener,
  ): void {
    if (type === "message") {
      const wrapped = this.messageListeners.get(listener as LightingWorkerMessageListener<LightingWorkerResponseEnvelope>);
      if (wrapped !== undefined) {
        this.worker.off(type, wrapped);
        this.messageListeners.delete(listener as LightingWorkerMessageListener<LightingWorkerResponseEnvelope>);
      }
      return;
    }

    if (type === "error") {
      const wrapped = this.errorListeners.get(listener as LightingWorkerErrorListener);
      if (wrapped !== undefined) {
        this.worker.off(type, wrapped);
        this.errorListeners.delete(listener as LightingWorkerErrorListener);
      }
    }
  }

  public terminate(): void {
    void this.worker.terminate();
  }

  public close(): void {
    void this.worker.terminate();
  }
}

export function createNodeLightingWorker(): NodeWorker {
  return new NodeWorker(new URL("./node-lighting-worker-thread.ts", import.meta.url));
}

export function createNodeLightingService(worker: NodeWorker = createNodeLightingWorker()): LightingWorkerClient {
  return new LightingWorkerClient(new NodeLightingWorkerClientEndpoint(worker));
}
