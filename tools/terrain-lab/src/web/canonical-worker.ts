/// <reference lib="webworker" />

import initTerrainLab, {
  CanonicalTerrainCompiler,
} from "../../generated/pkg/mclone_terrain_lab";

import type {
  CanonicalWorkerRequest,
  CanonicalWorkerResponse,
  CanonicalWorkerResult,
} from "./canonical-worker-protocol";

let compiler: CanonicalTerrainCompiler | undefined;
let activeEpoch = 0;

self.onmessage = (event: MessageEvent<CanonicalWorkerRequest>): void => {
  const request = event.data;
  if (request.type === "init") {
    activeEpoch = request.epoch;
    compiler?.free();
    compiler = undefined;
    void initTerrainLab()
      .then(() => {
        if (activeEpoch !== request.epoch) {
          return;
        }
        compiler = CanonicalTerrainCompiler.withProfile(
          request.seed,
          request.profile,
          request.stage,
        );
        post({ type: "ready", epoch: request.epoch });
      })
      .catch((error: unknown) => {
        post({
          type: "error",
          epoch: request.epoch,
          message: errorMessage(error),
        });
      });
    return;
  }
  if (request.epoch !== activeEpoch || !compiler) {
    return;
  }
  try {
    const started = performance.now();
    const payload = compiler.compile(request.chunkX, request.chunkZ);
    const blocks = payload.blocks;
    const biomes = payload.biomes;
    const result: CanonicalWorkerResult = {
      type: "result",
      epoch: request.epoch,
      chunkX: payload.chunkX,
      chunkZ: payload.chunkZ,
      minY: payload.minY,
      height: payload.height,
      fingerprint: payload.fingerprint,
      generationMs: performance.now() - started,
      dependencyCacheHits: payload.dependencyCacheHits,
      generatedDependencyChunks: payload.generatedDependencyChunks,
      retainedDependencyChunks: payload.retainedDependencyChunks,
      blocks,
      biomes,
    };
    payload.free();
    self.postMessage(result, {
      transfer: [blocks.buffer, biomes.buffer],
    });
  } catch (error: unknown) {
    post({
      type: "error",
      epoch: request.epoch,
      message: errorMessage(error),
    });
  }
};

function post(response: CanonicalWorkerResponse): void {
  self.postMessage(response);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
