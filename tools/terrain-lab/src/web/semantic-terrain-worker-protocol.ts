import type {
  SemanticTerrainCorrection,
  SemanticTerrainFeatures,
  SemanticTerrainSubstrate,
  StreamedPlanAtlasTopology,
} from "../state";

export interface SemanticTerrainWorkerQuery {
  type: "query";
  revision: number;
  seed: string;
  topology: StreamedPlanAtlasTopology;
  substrate: SemanticTerrainSubstrate;
  features: SemanticTerrainFeatures;
  correction: SemanticTerrainCorrection;
  centerX: number;
  centerZ: number;
  blocksAcross: number;
  aspectRatio: number;
  samplesAcross: number;
}

export interface SemanticTerrainDetailMetrics {
  detail: "parent" | "regional" | "local";
  ownerCount: number;
  featureCount: number;
  segmentCount: number;
  distanceEvaluationCount: number;
  semanticSha256: string;
}

export interface SemanticTerrainCorrectionMetrics {
  minimum: number;
  maximum: number;
  rms: number;
  changedSampleFraction: number;
}

export interface SemanticTerrainGuide {
  detail: "parent" | "regional" | "local";
  family: "range-axis" | "basin-route";
  startX: number;
  startZ: number;
  endX: number;
  endZ: number;
}

export interface SemanticTerrainMetadata {
  receiptSchema: string;
  revision: string;
  researchOnly: boolean;
  productionTerrainUnchanged: boolean;
  seed: string;
  topology: string;
  substrate: SemanticTerrainSubstrate;
  features: SemanticTerrainFeatures;
  centerX: number;
  centerZ: number;
  blocksAcross: number;
  blocksTall: number;
  columns: number;
  rows: number;
  sampleCount: number;
  minimumHeight: number;
  maximumHeight: number;
  parent: SemanticTerrainDetailMetrics;
  regional: SemanticTerrainDetailMetrics;
  local: SemanticTerrainDetailMetrics;
  regionalCorrection: SemanticTerrainCorrectionMetrics;
  localCorrection: SemanticTerrainCorrectionMetrics;
  semanticSha256: string;
  terrainSha256: string;
  guides: SemanticTerrainGuide[];
}

export interface SemanticTerrainWorkerSummary {
  type: "summary";
  revision: number;
  compileMs: number;
  suiteSha256: string;
  correction: SemanticTerrainCorrection;
  metadata: SemanticTerrainMetadata;
  foundation: Float32Array;
  parentHeights: Float32Array;
  regionalHeights: Float32Array;
  localHeights: Float32Array;
  regionalCorrection: Float32Array;
  localCorrection: Float32Array;
  parentWater: Uint8Array;
  regionalWater: Uint8Array;
  localWater: Uint8Array;
}

export interface SemanticTerrainWorkerError {
  type: "error";
  revision: number;
  message: string;
}

export type SemanticTerrainWorkerRequest = SemanticTerrainWorkerQuery;
export type SemanticTerrainWorkerResponse =
  | SemanticTerrainWorkerSummary
  | SemanticTerrainWorkerError;
