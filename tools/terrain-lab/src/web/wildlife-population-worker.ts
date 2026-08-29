/// <reference lib="webworker" />

import initTerrainLab, {
  WildlifePopulationCompiler,
} from "../../generated/pkg/mclone_terrain_lab";

import type {
  WildlifePopulationSummary,
  WildlifePopulationWorkerRequest,
  WildlifePopulationWorkerResponse,
} from "./wildlife-population-worker-protocol";

let activeEpoch = 0;

self.onmessage = (event: MessageEvent<WildlifePopulationWorkerRequest>): void => {
  const request = event.data;
  activeEpoch = request.epoch;
  void initTerrainLab()
    .then(() => {
      if (request.epoch !== activeEpoch) {
        return;
      }
      const started = performance.now();
      const compiler = new WildlifePopulationCompiler(request.seed, request.profile);
      try {
        const summary = JSON.parse(compiler.compile(
          request.centerX,
          request.centerZ,
          request.blocksAcross,
          request.aspectRatio,
        )) as WildlifePopulationSummary;
        if (request.epoch !== activeEpoch) {
          return;
        }
        post({
          type: "summary",
          epoch: request.epoch,
          buildMs: performance.now() - started,
          summary,
        });
      } finally {
        compiler.free();
      }
    })
    .catch((error: unknown) => {
      post({
        type: "error",
        epoch: request.epoch,
        message: errorMessage(error),
      });
    });
};

function post(response: WildlifePopulationWorkerResponse): void {
  self.postMessage(response);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
