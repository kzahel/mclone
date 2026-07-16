type WasmModule = typeof import("mclone-web-client-wasm") & {
  mclone_web_remote_handshake_frame(profileId: Uint8Array, displayName: string): Uint8Array;
  mclone_web_validate_remote_handshake(frame: Uint8Array): void;
  mclone_web_canonicalize_remote_command(frame: Uint8Array): Uint8Array;
  mclone_web_decode_remote_update_batch(frame: Uint8Array): Array<Uint8Array>;
  mclone_web_remote_control_response(frame: Uint8Array): Uint8Array;
};

export {};

interface RemoteWorkerMessage {
  kind?: string;
  url?: string;
  bindgenJsUrl?: string;
  bindgenWasmUrl?: string;
  frame?: Uint8Array;
  profileId?: Uint8Array;
  displayName?: string;
  batchSequence?: number;
}

const workerSelf = self as unknown as DedicatedWorkerGlobalScope;
const MAX_SOCKET_BUFFERED_COMMAND_BYTES = 8 * 1024 * 1024;
const MAX_UNCONSUMED_UPDATE_BYTES = 64 * 1024 * 1024;

let wasmModule: WasmModule | null = null;
let socket: WebSocket | null = null;
let handshakeComplete = false;
let nextBatchSequence = 1;
let unconsumedUpdateBytes = 0;
const unconsumedBatches = new Map<number, number>();

workerSelf.onmessage = async (event: MessageEvent) => {
  const message = (event.data ?? {}) as RemoteWorkerMessage;
  try {
    switch (message.kind) {
      case "start":
        await start(message);
        break;
      case "command":
        sendCommand(message);
        break;
      case "updates-drained":
        releaseUpdateBatch(message);
        break;
      case "shutdown":
        shutdown();
        break;
      default:
        throw new Error(`unexpected remote websocket worker message ${String(message.kind)}`);
    }
  } catch (error) {
    fail(stringifyError(error));
  }
};

async function start(message: RemoteWorkerMessage): Promise<void> {
  if (socket) throw new Error("remote websocket worker was already started");
  if (!message.url || !message.bindgenJsUrl || !message.bindgenWasmUrl
      || !message.profileId || !message.displayName) {
    throw new Error("remote websocket worker start is missing a required URL");
  }
  const module = await import(message.bindgenJsUrl) as WasmModule;
  await module.default(message.bindgenWasmUrl);
  wasmModule = module;

  const activeSocket = new WebSocket(message.url);
  socket = activeSocket;
  activeSocket.binaryType = "arraybuffer";
  activeSocket.onopen = () => {
    try {
      activeSocket.send(requireModule().mclone_web_remote_handshake_frame(
        message.profileId!,
        message.displayName!,
      ));
    } catch (error) {
      fail(stringifyError(error));
    }
  };
  activeSocket.onmessage = (event: MessageEvent) => {
    try {
      receiveFrame(new Uint8Array(event.data as ArrayBuffer));
    } catch (error) {
      fail(stringifyError(error));
    }
  };
  activeSocket.onerror = () => fail("remote websocket transport failed");
  activeSocket.onclose = () => {
    socket = null;
    workerSelf.postMessage({ kind: "closed", message: "remote websocket transport closed" });
  };
}

function receiveFrame(frame: Uint8Array): void {
  const module = requireModule();
  if (!handshakeComplete) {
    module.mclone_web_validate_remote_handshake(frame);
    handshakeComplete = true;
    workerSelf.postMessage({ kind: "ready" });
    return;
  }

  const decodeStart = performance.now();
  const canonicalFrames = module.mclone_web_decode_remote_update_batch(frame);
  for (const canonical of canonicalFrames) {
    const response = module.mclone_web_remote_control_response(canonical);
    if (response.byteLength > 0) {
      const activeSocket = socket;
      if (!activeSocket || activeSocket.readyState !== WebSocket.OPEN) {
        throw new Error("remote websocket closed before keepalive response");
      }
      if (activeSocket.bufferedAmount > MAX_SOCKET_BUFFERED_COMMAND_BYTES) {
        throw new Error(
          `remote control socket buffer exceeded ${MAX_SOCKET_BUFFERED_COMMAND_BYTES} bytes`,
        );
      }
      activeSocket.send(response);
    }
  }
  const decodeMs = Math.max(0, performance.now() - decodeStart);
  const batchSequence = nextBatchSequence++;
  const batchBytes = frame.byteLength;
  unconsumedUpdateBytes += batchBytes;
  unconsumedBatches.set(batchSequence, batchBytes);
  if (unconsumedUpdateBytes > MAX_UNCONSUMED_UPDATE_BYTES) {
    throw new Error(
      `remote update queue exceeded ${MAX_UNCONSUMED_UPDATE_BYTES} bytes`,
    );
  }

  const frames = canonicalFrames.map((canonical) => {
    const owned = canonical.byteOffset === 0 && canonical.byteLength === canonical.buffer.byteLength
      ? canonical
      : canonical.slice();
    return owned.buffer;
  });
  workerSelf.postMessage(
    {
      kind: "updates",
      batchSequence,
      receivedBytes: batchBytes,
      decodeMs,
      frames,
    },
    frames,
  );
}

function sendCommand(message: RemoteWorkerMessage): void {
  const activeSocket = socket;
  if (!activeSocket || !handshakeComplete || activeSocket.readyState !== WebSocket.OPEN) {
    throw new Error("remote websocket is not ready for commands");
  }
  if (!message.frame) throw new Error("remote command message has no frame");
  if (activeSocket.bufferedAmount > MAX_SOCKET_BUFFERED_COMMAND_BYTES) {
    throw new Error(
      `remote command socket buffer exceeded ${MAX_SOCKET_BUFFERED_COMMAND_BYTES} bytes`,
    );
  }
  const canonical = requireModule().mclone_web_canonicalize_remote_command(message.frame);
  activeSocket.send(canonical);
  workerSelf.postMessage({ kind: "command-sent", bytes: canonical.byteLength });
}

function releaseUpdateBatch(message: RemoteWorkerMessage): void {
  const sequence = Number(message.batchSequence) || 0;
  const bytes = unconsumedBatches.get(sequence);
  if (bytes === undefined) return;
  unconsumedBatches.delete(sequence);
  unconsumedUpdateBytes = Math.max(0, unconsumedUpdateBytes - bytes);
}

function shutdown(): void {
  const activeSocket = socket;
  socket = null;
  handshakeComplete = false;
  if (activeSocket) {
    activeSocket.onopen = null;
    activeSocket.onmessage = null;
    activeSocket.onerror = null;
    activeSocket.onclose = null;
    activeSocket.close();
  }
  workerSelf.close();
}

function requireModule(): WasmModule {
  if (!wasmModule) throw new Error("remote websocket worker wasm is not initialized");
  return wasmModule;
}

function fail(message: string): void {
  workerSelf.postMessage({ kind: "error", message });
  const activeSocket = socket;
  socket = null;
  if (activeSocket) activeSocket.close();
}

function stringifyError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
