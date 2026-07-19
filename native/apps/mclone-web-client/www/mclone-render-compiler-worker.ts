import type { WebRenderWorkerActor } from "mclone-web-client-wasm";

// The generated wasm-bindgen namespace is loaded through a versioned browser URL.
// The bare specifier is a type-only path mapping used by the no-emit TS gate.
type WasmModule = typeof import("mclone-web-client-wasm");

// Browser-only bootstrap fields. Everything else in the structured-clone frame
// is opaque to this broker and is interpreted by the worker-resident Rust actor.
interface WorkerBootstrapFrame {
  bindgenJsUrl?: string;
  bindgenWasmUrl?: string;
  requestId?: number;
}

let wasmModulePromise: Promise<WasmModule> | null = null;
let renderActor: WebRenderWorkerActor | null = null;
const workerSelf = self as unknown as DedicatedWorkerGlobalScope;

workerSelf.onmessage = async (event: MessageEvent) => {
  const frame = (event.data ?? {}) as WorkerBootstrapFrame;
  try {
    let actor = renderActor;
    if (actor === null) {
      const module = await loadWasmModule(frame.bindgenJsUrl, frame.bindgenWasmUrl);
      // A compile frame can arrive while the initialization import is pending.
      // Recheck after awaiting the shared promise so only the first frame creates
      // the actor and every later frame reaches the same Rust-owned lifecycle.
      actor = renderActor;
      if (actor === null) {
        actor = new module.WebRenderWorkerActor(event.data ?? {});
        renderActor = actor;
        workerSelf.postMessage(actor.readyReport());
        return;
      }
    }
    const report = actor.handleMessage(event.data ?? {});
    if (report !== undefined) {
      workerSelf.postMessage(report);
    }
  } catch (error) {
    // Module loading is necessarily browser-owned. Once the actor exists, its
    // ordinary domain failures are returned as Rust-authored reports and do not
    // reach this last-resort bootstrap/FFI failure envelope.
    workerSelf.postMessage({
      ok: false,
      kind: renderActor === null ? "render-compiler-ready" : "render-compiler-error",
      requestId: Number(frame.requestId) || 0,
      reason: stringifyError(error),
    });
  }
};

function loadWasmModule(
  bindgenJsUrl: string | undefined,
  bindgenWasmUrl: string | undefined,
): Promise<WasmModule> {
  wasmModulePromise ??= import(bindgenJsUrl as string).then(async (module: WasmModule) => {
    await module.default(bindgenWasmUrl);
    return module;
  });
  return wasmModulePromise;
}

function stringifyError(error: unknown): string {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}
