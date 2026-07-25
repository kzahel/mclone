/// <reference lib="webworker" />

import initTerrainLab, {
  CanonicalTerrainWorkerActor,
} from "../../generated/pkg/mclone_terrain_lab";

const workerSelf = self as unknown as DedicatedWorkerGlobalScope;
let wasmPromise: ReturnType<typeof initTerrainLab> | undefined;
let actor: CanonicalTerrainWorkerActor | undefined;

workerSelf.onmessage = (event: MessageEvent<unknown>): void => {
  void forwardToRust(event.data).catch((error: unknown) => {
    // Wasm bootstrap is necessarily browser-owned. Actor-domain failures are
    // returned as Rust-authored opaque frames and never reach this fallback.
    setTimeout(() => {
      throw error;
    }, 0);
  });
};

async function forwardToRust(frame: unknown): Promise<void> {
  wasmPromise ??= initTerrainLab();
  await wasmPromise;
  actor ??= new CanonicalTerrainWorkerActor();
  const dispatch = actor.handleMessage(frame);
  try {
    workerSelf.postMessage(dispatch.message, {
      transfer: Array.from(dispatch.transferables) as Transferable[],
    });
  } finally {
    dispatch.free();
  }
}
