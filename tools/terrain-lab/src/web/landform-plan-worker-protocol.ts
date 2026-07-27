export interface LandformPlanWorkerBuild {
  type: "build";
  epoch: number;
  seed: string;
}

export interface LandformPlanWorkerInspect {
  type: "inspect";
  epoch: number;
  revision: number;
  worldX: number;
  worldZ: number;
}

export type LandformPlanWorkerRequest =
  | LandformPlanWorkerBuild
  | LandformPlanWorkerInspect;

export interface LandformPlanSummary {
  type: "summary";
  epoch: number;
  seed: string;
  schema: string;
  topology: string;
  minX: number;
  minZ: number;
  cellBlocks: number;
  widthCells: number;
  depthCells: number;
  buildMs: number;
  transferBytes: number;
  checksum: string;
  channelCells: number;
  confluences: number;
  drainageSegments: number;
  divideSegments: number;
  protectedSinks: number;
  basinIds: Uint16Array;
  receiverIds: Uint32Array;
  accumulation: Uint32Array;
  streamOrder: Uint8Array;
  flags: Uint8Array;
  uplift: Uint8Array;
  quiet: Uint8Array;
  broadLow: Uint8Array;
  baseY: Int16Array;
  segmentCoordinates: Float32Array;
  segmentKinds: Uint8Array;
  segmentOrders: Uint8Array;
  segmentAccumulation: Uint32Array;
  sinkCoordinates: Int32Array;
  sinkKinds: Uint8Array;
  sinkIds: Uint16Array;
  sinkLevels: Float32Array;
}

export interface LandformPlanPointReceipt {
  worldX: number;
  worldZ: number;
  gridX: number;
  gridZ: number;
  basinId: number;
  receiverId: number;
  accumulation: number;
  streamOrder: number;
  ocean: boolean;
  channel: boolean;
  confluence: boolean;
  protectedBasin: boolean;
  quietCore: boolean;
  cropEdge: boolean;
  uplift: number;
  quiet: number;
  broadLow: number;
  baseY: number;
}

export interface LandformPlanWorkerInspection {
  type: "inspection";
  epoch: number;
  revision: number;
  receipt: LandformPlanPointReceipt | null;
}

export interface LandformPlanWorkerError {
  type: "error";
  epoch: number;
  revision?: number;
  message: string;
}

export type LandformPlanWorkerResponse =
  | LandformPlanSummary
  | LandformPlanWorkerInspection
  | LandformPlanWorkerError;
