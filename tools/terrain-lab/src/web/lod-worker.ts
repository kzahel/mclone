/// <reference lib="webworker" />

import initTerrainLab, {
  VanillaTerrainLodCompiler,
  VanillaTerrainMacroCompiler,
} from "../../generated/pkg/mclone_terrain_lab";

import type {
  LodWorkerCompile,
  LodWorkerRequest,
  LodWorkerResponse,
  LodWorkerResult,
} from "./lod-worker-protocol";

type VanillaCompiler = VanillaTerrainLodCompiler | VanillaTerrainMacroCompiler;

let compiler: VanillaCompiler | undefined;
let activeEpoch = 0;

self.onmessage = (event: MessageEvent<LodWorkerRequest>): void => {
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
        compiler = request.mode === "macro"
          ? new VanillaTerrainMacroCompiler(request.seed)
          : new VanillaTerrainLodCompiler(request.seed);
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
  compile(request, compiler);
};

function compile(
  request: LodWorkerCompile,
  currentCompiler: VanillaCompiler,
): void {
  try {
    const started = performance.now();
    const payload = currentCompiler.compile(
      request.tileX,
      request.tileZ,
      request.sampleSpacing,
    );
    const samples = payload.samples;
    const result: LodWorkerResult = {
      type: "result",
      epoch: request.epoch,
      revision: request.revision,
      seed: request.seed,
      tileX: request.tileX,
      tileZ: request.tileZ,
      sampleSpacing: request.sampleSpacing,
      compileMs: performance.now() - started,
      generatedDensityColumns: payload.generatedDensityColumns,
      reusedDensityColumns: payload.reusedDensityColumns,
      retainedDensityColumns: payload.retainedDensityColumns,
      samples,
    };
    payload.free();
    self.postMessage(result, { transfer: [samples.buffer] });
  } catch (error: unknown) {
    post({
      type: "error",
      epoch: request.epoch,
      revision: request.revision,
      seed: request.seed,
      tileX: request.tileX,
      tileZ: request.tileZ,
      sampleSpacing: request.sampleSpacing,
      message: errorMessage(error),
    });
  }
}

function post(response: LodWorkerResponse): void {
  self.postMessage(response);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
