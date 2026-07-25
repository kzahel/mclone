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
    console.error("Canonical terrain Worker bootstrap failed:", error);
    workerSelf.postMessage({
      kind: "error",
      epoch: frameEpoch(event.data),
      message: error instanceof Error ? error.message : String(error),
    });
  });
};

function frameEpoch(frame: unknown): number {
  if (
    typeof frame === "object"
    && frame !== null
    && "epoch" in frame
    && typeof frame.epoch === "number"
  ) {
    return frame.epoch;
  }
  return 0;
}

async function forwardToRust(frame: unknown): Promise<void> {
  wasmPromise ??= initTerrainLab();
  await wasmPromise;
  actor ??= new CanonicalTerrainWorkerActor();
  const dispatch = actor.handleMessage(frame);
  try {
    workerSelf.postMessage(dispatch.message);
  } finally {
    dispatch.free();
  }
}
