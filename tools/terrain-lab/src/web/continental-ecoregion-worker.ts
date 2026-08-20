/// <reference lib="webworker" />

import initTerrainLab, {
  ContinentalEcoregionCompiler,
  continentalEcoregionSuiteSha256,
} from "../../generated/pkg/mclone_terrain_lab";

import type {
  ContinentalEcoregionAtlasMetadata,
  ContinentalEcoregionWorkerRequest,
  ContinentalEcoregionWorkerResponse,
  ContinentalEcoregionWorkerSummary,
} from "./continental-ecoregion-worker-protocol";

let compiler: ContinentalEcoregionCompiler | undefined;
let compilerKey = "";
let activeRevision = 0;
const ready = initTerrainLab();
const verifiedSuite = ready.then(() => continentalEcoregionSuiteSha256());

self.onmessage = (
  event: MessageEvent<ContinentalEcoregionWorkerRequest>,
): void => {
  const request = event.data;
  activeRevision = Math.max(activeRevision, request.revision);
  void verifiedSuite
    .then((suiteSha256) => {
      if (request.revision !== activeRevision) {
        return;
      }
      const nextKey = `${request.seed}|${request.topology}`;
      if (!compiler || compilerKey !== nextKey) {
        compiler?.free();
        compiler = new ContinentalEcoregionCompiler(
          request.seed,
          request.topology,
        );
        compilerKey = nextKey;
      }
      const started = performance.now();
      const metadata = JSON.parse(compiler.compile(
        request.centerX,
        request.centerZ,
        request.blocksAcross,
        request.aspectRatio,
        request.samplesAcross,
      )) as ContinentalEcoregionAtlasMetadata;
      const response: ContinentalEcoregionWorkerSummary = {
        type: "summary",
        revision: request.revision,
        compileMs: performance.now() - started,
        suiteSha256,
        metadata,
        land: compiler.land(),
        inlandDistanceQuarterBlocks: compiler.inlandDistanceQuarterBlocks(),
        continentStory: compiler.continentStory(),
        provinceKind: compiler.provinceKind(),
        ecoregionKind: compiler.ecoregionKind(),
        transition: compiler.transition(),
        openness: compiler.openness(),
        forestCore: compiler.forestCore(),
        clearingCore: compiler.clearingCore(),
        clearingCause: compiler.clearingCause(),
        majorWater: compiler.majorWater(),
        wetland: compiler.wetland(),
        corridor: compiler.corridor(),
        continentId: compiler.continentId(),
        provinceId: compiler.provinceId(),
        ecoregionId: compiler.ecoregionId(),
        clearingId: compiler.clearingId(),
        productionLand: compiler.productionLand(),
        productionSurfaceY: compiler.productionSurfaceY(),
        productionTemperature: compiler.productionTemperature(),
        productionMoisture: compiler.productionMoisture(),
        productionRelief: compiler.productionRelief(),
        productionRuggedness: compiler.productionRuggedness(),
        productionWater: compiler.productionWater(),
        productionBiomeKind: compiler.productionBiomeKind(),
      };
      if (request.revision === activeRevision) {
        post(response);
      }
    })
    .catch((error: unknown) => {
      post({
        type: "error",
        revision: request.revision,
        message: errorMessage(error),
      });
    });
};

function post(response: ContinentalEcoregionWorkerResponse): void {
  if (response.type === "error") {
    self.postMessage(response);
    return;
  }
  self.postMessage(response, {
    transfer: [
      response.land.buffer,
      response.inlandDistanceQuarterBlocks.buffer,
      response.continentStory.buffer,
      response.provinceKind.buffer,
      response.ecoregionKind.buffer,
      response.transition.buffer,
      response.openness.buffer,
      response.forestCore.buffer,
      response.clearingCore.buffer,
      response.clearingCause.buffer,
      response.majorWater.buffer,
      response.wetland.buffer,
      response.corridor.buffer,
      response.continentId.buffer,
      response.provinceId.buffer,
      response.ecoregionId.buffer,
      response.clearingId.buffer,
      response.productionLand.buffer,
      response.productionSurfaceY.buffer,
      response.productionTemperature.buffer,
      response.productionMoisture.buffer,
      response.productionRelief.buffer,
      response.productionRuggedness.buffer,
      response.productionWater.buffer,
      response.productionBiomeKind.buffer,
    ],
  });
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
