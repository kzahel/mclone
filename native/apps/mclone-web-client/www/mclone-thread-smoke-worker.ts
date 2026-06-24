interface ThreadSmokeMessage {
  kind?: string;
  memory?: unknown;
  addend?: unknown;
}

const workerSelf = self as unknown as DedicatedWorkerGlobalScope;

workerSelf.onmessage = (event: MessageEvent) => {
  const { kind, memory, addend } = (event.data ?? {}) as ThreadSmokeMessage;
  if (kind !== "mclone-thread-smoke") {
    workerSelf.postMessage({
      ok: false,
      reason: `unexpected worker message kind ${String(kind)}`,
    });
    return;
  }
  if (!memory || typeof memory !== "object") {
    workerSelf.postMessage({
      ok: false,
      reason: "message did not include WebAssembly.Memory",
    });
    return;
  }
  const memoryLike = memory as { buffer?: unknown };
  if (!(memoryLike.buffer instanceof SharedArrayBuffer)) {
    workerSelf.postMessage({
      ok: false,
      reason: "WebAssembly.Memory buffer is not shared",
    });
    return;
  }

  const view = new Int32Array(memoryLike.buffer);
  const initialValue = Atomics.load(view, 0);
  const finalValue = Atomics.add(view, 0, Number(addend) || 0) + (Number(addend) || 0);
  Atomics.store(view, 1, 1);
  Atomics.notify(view, 1, 1);

  workerSelf.postMessage({
    ok: true,
    initialValue,
    finalValue,
    sharedArrayBuffer: true,
    atomics: true,
  });
};
