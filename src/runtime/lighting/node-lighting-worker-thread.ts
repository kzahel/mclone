import { parentPort } from "node:worker_threads";
import { connectLightingWorkerSession, type LightingWorkerHostEndpoint, type LightingWorkerMessageListener } from "./lighting-worker-client";
import { createLightingWorkerHandler } from "./lighting-worker";
import type { LightingWorkerRequestEnvelope, LightingWorkerResponseEnvelope } from "./lighting-worker-client";

class NodeLightingWorkerHostEndpoint implements LightingWorkerHostEndpoint {
  private readonly listeners = new Map<
    LightingWorkerMessageListener<LightingWorkerRequestEnvelope>,
    (message: LightingWorkerRequestEnvelope) => void
  >();

  public postMessage(message: LightingWorkerResponseEnvelope, transfer: readonly Transferable[] = []): void {
    parentPort!.postMessage(message, transfer as Parameters<NonNullable<typeof parentPort>["postMessage"]>[1]);
  }

  public addEventListener(type: "message", listener: LightingWorkerMessageListener<LightingWorkerRequestEnvelope>): void {
    const wrapped = (message: LightingWorkerRequestEnvelope): void => {
      listener({ data: message });
    };
    this.listeners.set(listener, wrapped);
    parentPort!.on(type, wrapped);
  }

  public removeEventListener(type: "message", listener: LightingWorkerMessageListener<LightingWorkerRequestEnvelope>): void {
    const wrapped = this.listeners.get(listener);
    if (wrapped === undefined) {
      return;
    }

    parentPort!.off(type, wrapped);
    this.listeners.delete(listener);
  }

  public start(): void {
    parentPort!.start();
  }

  public close(): void {
    parentPort!.close();
  }
}

if (parentPort === null) {
  throw new Error("node-lighting-worker-thread must run inside a worker thread");
}

connectLightingWorkerSession(
  new NodeLightingWorkerHostEndpoint(),
  createLightingWorkerHandler(),
);
