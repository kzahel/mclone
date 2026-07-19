import type {
  WebRemoteSocketWorkerAction,
  WebRemoteSocketWorkerActor,
} from "mclone-web-client-wasm";

// The generated wasm-bindgen namespace is loaded through a versioned browser URL.
// The bare specifier is a type-only path mapping used by the no-emit TS gate.
type WasmModule = typeof import("mclone-web-client-wasm");

export {};

// Browser-only bootstrap fields plus a Rust-authored opaque actor frame. The
// worker shell deliberately does not know the transport protocol or lifecycle.
interface RemoteWorkerEnvelope {
  bindgenJsUrl?: string;
  bindgenWasmUrl?: string;
  actorFrame?: Uint8Array;
}

const workerSelf = self as unknown as DedicatedWorkerGlobalScope;
let wasmModulePromise: Promise<WasmModule> | null = null;
let actor: WebRemoteSocketWorkerActor | null = null;
let socket: WebSocket | null = null;
let draining = false;

workerSelf.onmessage = async (event: MessageEvent) => {
  const envelope = (event.data ?? {}) as RemoteWorkerEnvelope;
  try {
    if (!(envelope.actorFrame instanceof Uint8Array)) {
      throw new Error("remote websocket worker message has no actor frame");
    }
    let activeActor = actor;
    if (activeActor === null) {
      const module = await loadWasmModule(envelope.bindgenJsUrl, envelope.bindgenWasmUrl);
      activeActor = new module.WebRemoteSocketWorkerActor(envelope.actorFrame);
      actor = activeActor;
    } else {
      activeActor.handleMainFrame(envelope.actorFrame);
    }
    drainActions();
  } catch (error) {
    failBootstrapOrFfi(stringifyError(error));
  }
};

function drainActions(): void {
  const activeActor = actor;
  if (activeActor === null || draining) return;
  draining = true;
  try {
    for (;;) {
      const action = activeActor.takeAction(socket?.bufferedAmount ?? 0);
      if (action === undefined) return;
      try {
        executeAction(activeActor, action);
      } finally {
        action.free();
      }
      if (actor === null) return;
    }
  } finally {
    draining = false;
  }
}

function executeAction(
  activeActor: WebRemoteSocketWorkerActor,
  action: WebRemoteSocketWorkerAction,
): void {
  switch (action.kind) {
    case "open":
      openSocket(activeActor, action.url);
      return;
    case "send": {
      const activeSocket = socket;
      if (!activeSocket || activeSocket.readyState !== WebSocket.OPEN) {
        activeActor.socketFailed("remote websocket is not open for a Rust send action");
        return;
      }
      activeSocket.send(action.takeBytes());
      return;
    }
    case "post":
      workerSelf.postMessage(action.message(), action.transfers());
      return;
    case "close-socket":
      closeSocket();
      return;
    case "close-worker":
      closeSocket();
      actor = null;
      workerSelf.close();
      return;
    default:
      throw new Error(`unsupported remote actor action ${action.kind}`);
  }
}

function openSocket(activeActor: WebRemoteSocketWorkerActor, url: string | undefined): void {
  if (!url) throw new Error("remote actor open action has no URL");
  if (socket !== null) throw new Error("remote actor requested a second WebSocket");
  const activeSocket = new WebSocket(url);
  socket = activeSocket;
  activeSocket.binaryType = "arraybuffer";
  activeSocket.onopen = () => forwardSocketEvent(() => activeActor.socketOpened());
  activeSocket.onmessage = (event: MessageEvent) => forwardSocketEvent(() => {
    activeActor.socketFrame(new Uint8Array(event.data as ArrayBuffer));
  });
  activeSocket.onerror = () => forwardSocketEvent(() => {
    activeActor.socketFailed("remote websocket transport failed");
  });
  activeSocket.onclose = () => {
    if (socket === activeSocket) socket = null;
    forwardSocketEvent(() => activeActor.socketClosed());
  };
}

function forwardSocketEvent(forward: () => void): void {
  try {
    forward();
    drainActions();
  } catch (error) {
    failBootstrapOrFfi(stringifyError(error));
  }
}

function closeSocket(): void {
  const activeSocket = socket;
  socket = null;
  if (!activeSocket) return;
  activeSocket.onopen = null;
  activeSocket.onmessage = null;
  activeSocket.onerror = null;
  activeSocket.onclose = null;
  activeSocket.close();
}

function loadWasmModule(
  bindgenJsUrl: string | undefined,
  bindgenWasmUrl: string | undefined,
): Promise<WasmModule> {
  if (!bindgenJsUrl || !bindgenWasmUrl) {
    return Promise.reject(new Error("remote websocket worker bootstrap URLs are missing"));
  }
  wasmModulePromise ??= import(bindgenJsUrl).then(async (module: WasmModule) => {
    await module.default(bindgenWasmUrl);
    return module;
  });
  return wasmModulePromise;
}

function failBootstrapOrFfi(message: string): void {
  // Rust owns ordinary failures. This envelope is only the last resort when
  // module loading, browser API execution, or the wasm-bindgen boundary fails.
  workerSelf.postMessage({ kind: "error", message });
  closeSocket();
}

function stringifyError(error: unknown): string {
  return error instanceof Error ? (error.stack ?? error.message) : String(error);
}
