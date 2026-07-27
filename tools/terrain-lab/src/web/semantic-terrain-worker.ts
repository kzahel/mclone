/// <reference lib="webworker" />

import initTerrainLab, {
  SemanticTerrainSandboxCompiler,
  semanticTerrainSandboxSuiteSha256,
} from "../../generated/pkg/mclone_terrain_lab";

import type {
  SemanticTerrainMetadata,
  SemanticTerrainWorkerRequest,
  SemanticTerrainWorkerResponse,
  SemanticTerrainWorkerSummary,
} from "./semantic-terrain-worker-protocol";

let compiler: SemanticTerrainSandboxCompiler | undefined;
let compilerKey = "";
let activeRevision = 0;
const ready = initTerrainLab();
const verifiedSuite = ready.then(() => semanticTerrainSandboxSuiteSha256());

self.onmessage = (
  event: MessageEvent<SemanticTerrainWorkerRequest>,
): void => {
  const request = event.data;
  activeRevision = Math.max(activeRevision, request.revision);
  void verifiedSuite
    .then((suiteSha256) => {
      if (request.revision !== activeRevision) {
        return;
      }
      const nextKey = [
        request.seed,
        request.topology,
        request.substrate,
        request.features,
      ].join("|");
      if (!compiler || compilerKey !== nextKey) {
        compiler?.free();
        compiler = new SemanticTerrainSandboxCompiler(
          request.seed,
          request.topology,
          request.substrate,
          request.features,
        );
        compilerKey = nextKey;
      }
      const started = performance.now();
      const metadata = JSON.parse(
        compiler.compile(
          request.centerX,
          request.centerZ,
          request.blocksAcross,
          request.aspectRatio,
          request.samplesAcross,
        ),
      ) as SemanticTerrainMetadata;
      const response: SemanticTerrainWorkerSummary = {
        type: "summary",
        revision: request.revision,
        compileMs: performance.now() - started,
        suiteSha256,
        correction: request.correction,
        metadata,
        foundation: compiler.foundation(),
        parentHeights: compiler.parentHeights(),
        regionalHeights: compiler.regionalHeights(),
        localHeights: compiler.localHeights(),
        regionalCorrection: compiler.regionalCorrection(),
        localCorrection: compiler.localCorrection(),
        parentWater: compiler.parentWater(),
        regionalWater: compiler.regionalWater(),
        localWater: compiler.localWater(),
      };
      if (request.revision !== activeRevision) {
        return;
      }
      post(response);
    })
    .catch((error: unknown) => {
      post({
        type: "error",
        revision: request.revision,
        message: errorMessage(error),
      });
    });
};

function post(response: SemanticTerrainWorkerResponse): void {
  if (response.type === "error") {
    self.postMessage(response);
    return;
  }
  const transfers = [
    response.foundation.buffer,
    response.parentHeights.buffer,
    response.regionalHeights.buffer,
    response.localHeights.buffer,
    response.regionalCorrection.buffer,
    response.localCorrection.buffer,
    response.parentWater.buffer,
    response.regionalWater.buffer,
    response.localWater.buffer,
  ];
  self.postMessage(response, { transfer: transfers });
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
