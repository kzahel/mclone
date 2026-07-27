/// <reference lib="webworker" />

import initTerrainLab, {
  TerrainVegetationWorkerActor,
} from "../../generated/pkg/mclone_terrain_lab";

const workerSelf = self as unknown as DedicatedWorkerGlobalScope;
let wasmPromise: ReturnType<typeof initTerrainLab> | undefined;
let actor: TerrainVegetationWorkerActor | undefined;

workerSelf.onmessage = (event: MessageEvent<unknown>): void => {
  void forwardToRust(event.data).catch((error: unknown) => {
    console.error("Runtime vegetation Worker bootstrap failed:", error);
    workerSelf.postMessage({
      error: error instanceof Error ? error.message : String(error),
    });
  });
};

async function forwardToRust(frame: unknown): Promise<void> {
  wasmPromise ??= initTerrainLab();
  await wasmPromise;
  actor ??= new TerrainVegetationWorkerActor();
  const dispatch = actor.handleMessage(frame);
  try {
    workerSelf.postMessage(dispatch.message);
  } finally {
    dispatch.free();
  }
}
