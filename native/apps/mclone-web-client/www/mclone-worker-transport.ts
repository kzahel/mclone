export type PolledWorkerTransportEvent =
  | { kind: "message"; data: unknown }
  | { kind: "error"; message: string };

/** Browser Worker mechanics with an explicitly polled opaque inbox. */
export class PolledWorkerTransport {
  private worker: Worker | null;
  private readonly inbox: PolledWorkerTransportEvent[];

  constructor(workerOrUrl: Worker | URL | string, workerName?: string) {
    this.inbox = [];
    this.worker = workerOrUrl instanceof Worker
      ? workerOrUrl
      : new Worker(String(workerOrUrl), {
        type: "module",
        ...(workerName === undefined ? {} : { name: workerName }),
      });
    this.worker.onmessage = (event: MessageEvent<unknown>) => {
      this.inbox.push({ kind: "message", data: event.data });
    };
    this.worker.onerror = (event: ErrorEvent) => {
      this.inbox.push({
        kind: "error",
        message: event.message || "browser Worker failed",
      });
    };
  }

  post(message: unknown, transfer: Transferable[] = []): void {
    if (this.worker === null) {
      throw new Error("browser Worker transport is terminated");
    }
    this.worker.postMessage(message, transfer);
  }

  poll(): PolledWorkerTransportEvent | null {
    return this.inbox.shift() ?? null;
  }

  pendingEventCount(): number {
    return this.inbox.length;
  }

  terminate(): void {
    this.worker?.terminate();
    this.worker = null;
    this.inbox.length = 0;
  }
}
