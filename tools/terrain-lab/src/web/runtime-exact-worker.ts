/// <reference lib="webworker" />

import initTerrainLab, {
  TerrainRuntimeExactWorkerActor,
} from "../../generated/pkg/mclone_terrain_lab";

const workerSelf = self as unknown as DedicatedWorkerGlobalScope;
let wasmPromise: ReturnType<typeof initTerrainLab> | undefined;
let actor: TerrainRuntimeExactWorkerActor | undefined;

workerSelf.onmessage = (event: MessageEvent<unknown>): void => {
  void forwardToRust(event.data).catch((error: unknown) => {
    console.error("Runtime exact terrain Worker bootstrap failed:", error);
    workerSelf.postMessage({
      kind: "exact-error",
      message: error instanceof Error ? error.message : String(error),
    });
  });
};

async function forwardToRust(frame: unknown): Promise<void> {
  wasmPromise ??= initTerrainLab();
  await wasmPromise;
  actor ??= new TerrainRuntimeExactWorkerActor();
  const dispatch = actor.handleMessage(frame);
  try {
    const response = dispatch.message as { bytes?: Uint8Array };
    const transfer = response.bytes instanceof Uint8Array
      ? [response.bytes.buffer]
      : [];
    workerSelf.postMessage(response, transfer);
  } finally {
    dispatch.free();
  }
}
