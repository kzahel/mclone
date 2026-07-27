import type {
  TerrainVegetationWorkerActor,
} from "mclone-web-client-wasm";

type WasmModule = typeof import("mclone-web-client-wasm");

interface WorkerBootstrapFrame {
  bindgenJsUrl?: string;
  bindgenWasmUrl?: string;
}

let wasmModulePromise: Promise<WasmModule> | null = null;
let actor: TerrainVegetationWorkerActor | null = null;
const workerSelf = self as unknown as DedicatedWorkerGlobalScope;

workerSelf.onmessage = async (event: MessageEvent) => {
  const frame = (event.data ?? {}) as WorkerBootstrapFrame;
  try {
    const module = await loadWasmModule(frame.bindgenJsUrl, frame.bindgenWasmUrl);
    actor ??= new module.TerrainVegetationWorkerActor();
    const dispatch = actor.handleMessage(event.data ?? {});
    try {
      workerSelf.postMessage(dispatch.message);
    } finally {
      dispatch.free();
    }
  } catch (error) {
    workerSelf.postMessage({
      kind: "error",
      message: stringifyError(error),
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
