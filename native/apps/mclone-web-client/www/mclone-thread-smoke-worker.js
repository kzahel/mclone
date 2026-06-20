self.onmessage = (event) => {
  const { kind, memory, addend } = event.data ?? {};
  if (kind !== "mclone-thread-smoke") {
    self.postMessage({
      ok: false,
      reason: `unexpected worker message kind ${String(kind)}`,
    });
    return;
  }
  if (!memory || typeof memory !== "object") {
    self.postMessage({
      ok: false,
      reason: "message did not include WebAssembly.Memory",
    });
    return;
  }
  if (!(memory.buffer instanceof SharedArrayBuffer)) {
    self.postMessage({
      ok: false,
      reason: "WebAssembly.Memory buffer is not shared",
    });
    return;
  }

  const view = new Int32Array(memory.buffer);
  const initialValue = Atomics.load(view, 0);
  const finalValue = Atomics.add(view, 0, Number(addend) || 0) + (Number(addend) || 0);
  Atomics.store(view, 1, 1);
  Atomics.notify(view, 1, 1);

  self.postMessage({
    ok: true,
    initialValue,
    finalValue,
    sharedArrayBuffer: true,
    atomics: true,
  });
};
