/// <reference lib="webworker" />

import initTerrainLab, {
  StreamedPlanAtlasCompiler,
} from "../../generated/pkg/mclone_terrain_lab";

import type {
  StreamedPlanAtlasSummary,
  StreamedPlanAtlasWorkerRequest,
  StreamedPlanAtlasWorkerResponse,
} from "./streamed-plan-atlas-worker-protocol";

let compiler: StreamedPlanAtlasCompiler | undefined;
let compilerKey = "";
let activeRevision = 0;
const ready = initTerrainLab();

self.onmessage = (
  event: MessageEvent<StreamedPlanAtlasWorkerRequest>,
): void => {
  const request = event.data;
  activeRevision = Math.max(activeRevision, request.revision);
  void ready
    .then(() => {
      if (request.revision !== activeRevision) {
        return;
      }
      const nextKey = `${request.seed}|${request.topology}`;
      if (!compiler || compilerKey !== nextKey) {
        compiler?.free();
        compiler = new StreamedPlanAtlasCompiler(
          request.seed,
          request.topology,
        );
        compilerKey = nextKey;
      }
      if (request.clearCache) {
        compiler.clearCache();
      }
      const started = performance.now();
      const summary = JSON.parse(
        compiler.query(
          request.centerX,
          request.centerZ,
          request.blocksAcross,
          request.aspectRatio,
        ),
      ) as StreamedPlanAtlasSummary;
      if (request.revision !== activeRevision) {
        return;
      }
      post({
        type: "summary",
        revision: request.revision,
        queryMs: performance.now() - started,
        summary,
      });
    })
    .catch((error: unknown) => {
      post({
        type: "error",
        revision: request.revision,
        message: errorMessage(error),
      });
    });
};

function post(response: StreamedPlanAtlasWorkerResponse): void {
  self.postMessage(response);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
