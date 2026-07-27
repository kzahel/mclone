/// <reference lib="webworker" />

import initTerrainLab, {
  LandformPlanCompiler,
} from "../../generated/pkg/mclone_terrain_lab";

import type {
  LandformPlanPointReceipt,
  LandformPlanSummary,
  LandformPlanWorkerRequest,
  LandformPlanWorkerResponse,
} from "./landform-plan-worker-protocol";

let compiler: LandformPlanCompiler | undefined;
let activeEpoch = 0;

self.onmessage = (event: MessageEvent<LandformPlanWorkerRequest>): void => {
  const request = event.data;
  if (request.type === "build") {
    activeEpoch = request.epoch;
    compiler?.free();
    compiler = undefined;
    void initTerrainLab()
      .then(() => {
        if (request.epoch !== activeEpoch) {
          return;
        }
        const started = performance.now();
        const nextCompiler = new LandformPlanCompiler(request.seed);
        if (request.epoch !== activeEpoch) {
          nextCompiler.free();
          return;
        }
        compiler = nextCompiler;
        const summary = takeSummary(
          request.epoch,
          performance.now() - started,
          nextCompiler,
        );
        const transfer = transferableBuffers(summary);
        self.postMessage(summary, { transfer });
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
    const receipt = JSON.parse(
      compiler.inspect(request.worldX, request.worldZ),
    ) as LandformPlanPointReceipt | null;
    post({
      type: "inspection",
      epoch: request.epoch,
      revision: request.revision,
      receipt,
    });
  } catch (error: unknown) {
    post({
      type: "error",
      epoch: request.epoch,
      revision: request.revision,
      message: errorMessage(error),
    });
  }
};

function takeSummary(
  epoch: number,
  buildMs: number,
  source: LandformPlanCompiler,
): LandformPlanSummary {
  const summary: LandformPlanSummary = {
    type: "summary",
    epoch,
    seed: source.seed,
    schema: source.schema,
    topology: source.topology,
    minX: source.minX,
    minZ: source.minZ,
    cellBlocks: source.cellBlocks,
    widthCells: source.widthCells,
    depthCells: source.depthCells,
    buildMs,
    transferBytes: 0,
    checksum: source.checksum,
    channelCells: source.channelCells,
    confluences: source.confluences,
    drainageSegments: source.drainageSegments,
    divideSegments: source.divideSegments,
    protectedSinks: source.protectedSinks,
    basinIds: source.basinIds,
    receiverIds: source.receiverIds,
    accumulation: source.accumulation,
    streamOrder: source.streamOrder,
    flags: source.flags,
    uplift: source.uplift,
    quiet: source.quiet,
    broadLow: source.broadLow,
    baseY: source.baseY,
    segmentCoordinates: source.segmentCoordinates,
    segmentKinds: source.segmentKinds,
    segmentOrders: source.segmentOrders,
    segmentAccumulation: source.segmentAccumulation,
    sinkCoordinates: source.sinkCoordinates,
    sinkKinds: source.sinkKinds,
    sinkIds: source.sinkIds,
    sinkLevels: source.sinkLevels,
  };
  summary.transferBytes = transferableBuffers(summary)
    .reduce((bytes, buffer) => bytes + buffer.byteLength, 0);
  return summary;
}

function transferableBuffers(summary: LandformPlanSummary): ArrayBuffer[] {
  return [
    summary.basinIds.buffer,
    summary.receiverIds.buffer,
    summary.accumulation.buffer,
    summary.streamOrder.buffer,
    summary.flags.buffer,
    summary.uplift.buffer,
    summary.quiet.buffer,
    summary.broadLow.buffer,
    summary.baseY.buffer,
    summary.segmentCoordinates.buffer,
    summary.segmentKinds.buffer,
    summary.segmentOrders.buffer,
    summary.segmentAccumulation.buffer,
    summary.sinkCoordinates.buffer,
    summary.sinkKinds.buffer,
    summary.sinkIds.buffer,
    summary.sinkLevels.buffer,
  ] as ArrayBuffer[];
}

function post(response: LandformPlanWorkerResponse): void {
  self.postMessage(response);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
