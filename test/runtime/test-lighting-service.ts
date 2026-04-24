import {
  LightingWorkerClient,
  connectLightingWorkerSession,
  type LightingWorkerClientEndpoint,
  type LightingWorkerHostEndpoint,
  type LightingWorkerMessageListener,
  type LightingWorkerRequestEnvelope,
  type LightingWorkerResponseEnvelope,
} from "../../src/runtime/lighting/lighting-worker-client";
import { createLightingWorkerHandler } from "../../src/runtime/lighting/lighting-worker";

class TestLightingEndpoint<TOutgoing, TIncoming> {
  private peer?: TestLightingEndpoint<TIncoming, TOutgoing>;
  private readonly messageListeners = new Set<LightingWorkerMessageListener<TIncoming>>();
  private readonly errorListeners = new Set<(event: unknown) => void>();

  public connect(peer: TestLightingEndpoint<TIncoming, TOutgoing>): void {
    this.peer = peer;
  }

  public postMessage(message: TOutgoing, _transfer: readonly Transferable[] = []): void {
    queueMicrotask(() => {
      this.peer?.dispatchMessage(message);
    });
  }

  public addEventListener(type: "message" | "error" | "messageerror", listener: ((event: unknown) => void) | LightingWorkerMessageListener<TIncoming>): void {
    if (type === "message") {
      this.messageListeners.add(listener as LightingWorkerMessageListener<TIncoming>);
      return;
    }

    this.errorListeners.add(listener as (event: unknown) => void);
  }

  public removeEventListener(type: "message" | "error" | "messageerror", listener: ((event: unknown) => void) | LightingWorkerMessageListener<TIncoming>): void {
    if (type === "message") {
      this.messageListeners.delete(listener as LightingWorkerMessageListener<TIncoming>);
      return;
    }

    this.errorListeners.delete(listener as (event: unknown) => void);
  }

  private dispatchMessage(message: TIncoming): void {
    for (const listener of this.messageListeners) {
      listener({ data: message });
    }
  }
}

export function createTestLightingService(): LightingWorkerClient {
  const rawClientEndpoint = new TestLightingEndpoint<LightingWorkerRequestEnvelope, LightingWorkerResponseEnvelope>();
  const rawHostEndpoint = new TestLightingEndpoint<LightingWorkerResponseEnvelope, LightingWorkerRequestEnvelope>();
  rawClientEndpoint.connect(rawHostEndpoint);
  rawHostEndpoint.connect(rawClientEndpoint);
  connectLightingWorkerSession(
    rawHostEndpoint as unknown as LightingWorkerHostEndpoint,
    createLightingWorkerHandler(),
  );
  return new LightingWorkerClient(rawClientEndpoint as unknown as LightingWorkerClientEndpoint);
}
