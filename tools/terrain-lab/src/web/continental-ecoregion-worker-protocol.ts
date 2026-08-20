import type { ContinentalEcoregionTopology } from "../state";

export interface ContinentalEcoregionWorkerQuery {
  type: "query";
  revision: number;
  seed: string;
  topology: ContinentalEcoregionTopology;
  centerX: number;
  centerZ: number;
  blocksAcross: number;
  aspectRatio: number;
  samplesAcross: number;
}

export interface ComponentDistribution {
  componentCount: number;
  coveredSamples: number;
  minimumSamples: number;
  medianSamples: number;
  p90Samples: number;
  maximumSamples: number;
  maximumAreaSquareKm: number;
}

export interface EcoregionAdjacencyPair {
  leftKind: number;
  rightKind: number;
  boundaryEdges: number;
}

export interface EcoregionJourneyReceipt {
  label: string;
  distanceBlocks: number;
  runCount: number;
  repeatedSceneAlarms: number;
  meanDwellBlocks: number;
  longestDwellBlocks: number;
}

export interface ContinentalEcoregionAtlasMetrics {
  landFraction: number;
  oceanFraction: number;
  quietSpaceFraction: number;
  transitionFraction: number;
  continentComponents: ComponentDistribution;
  openComponents: ComponentDistribution;
  forestComponents: ComponentDistribution;
  wetlandComponents: ComponentDistribution;
  clearingComponents: ComponentDistribution;
  habitatNetworkComponents: ComponentDistribution;
  ecoregionComponents: ComponentDistribution;
  provinceKindCounts: number[];
  ecoregionKindCounts: number[];
  clearingCauseCounts: number[];
  ecoregionAdjacencies: EcoregionAdjacencyPair[];
  journeys: EcoregionJourneyReceipt[];
}

export interface PlanConstructionCounts {
  requestedSamples: number;
  continentalOwnerEvaluations: number;
  provinceOwnerEvaluations: number;
  ecoregionOwnerEvaluations: number;
  mosaicOwnerEvaluations: number;
  localFieldEvaluations: number;
  exactChunks: number;
}

export interface ProductionControlWork {
  requestedSamples: number;
  fieldSamples: number;
  exactChunks: number;
}

export interface ProductionControlMetrics {
  landFraction: number;
  oceanFraction: number;
  landComponents: ComponentDistribution;
  biomeComponents: ComponentDistribution;
  biomeKindCounts: number[];
  journeys: EcoregionJourneyReceipt[];
}

export interface ContinentalEcoregionAtlasMetadata {
  receiptSchema: string;
  planSchema: string;
  witnessSha256: string;
  productionTerrainUnchanged: boolean;
  seed: string;
  topology: string;
  centerX: number;
  centerZ: number;
  minX: number;
  minZ: number;
  blocksAcross: number;
  blocksTall: number;
  sampleStepBlocks: number;
  columns: number;
  rows: number;
  sampleCount: number;
  semanticSha256: string;
  work: PlanConstructionCounts;
  productionControlRevision: string;
  productionControlTopology: string;
  productionControlSha256: string;
  productionControlWork: ProductionControlWork;
  productionControlMetrics: ProductionControlMetrics;
  continentStories: string[];
  provinceKinds: string[];
  ecoregionKinds: string[];
  clearingCauses: string[];
  metrics: ContinentalEcoregionAtlasMetrics;
}

export interface ContinentalEcoregionWorkerSummary {
  type: "summary";
  revision: number;
  compileMs: number;
  suiteSha256: string;
  metadata: ContinentalEcoregionAtlasMetadata;
  land: Uint16Array;
  inlandDistanceQuarterBlocks: Int16Array;
  continentStory: Uint8Array;
  provinceKind: Uint8Array;
  ecoregionKind: Uint8Array;
  transition: Uint16Array;
  openness: Uint16Array;
  forestCore: Uint16Array;
  clearingCore: Uint16Array;
  clearingCause: Uint8Array;
  majorWater: Uint16Array;
  wetland: Uint16Array;
  corridor: Uint16Array;
  continentId: Uint32Array;
  provinceId: Uint32Array;
  ecoregionId: Uint32Array;
  clearingId: Uint32Array;
  productionLand: Uint16Array;
  productionSurfaceY: Int16Array;
  productionTemperature: Int16Array;
  productionMoisture: Int16Array;
  productionRelief: Int16Array;
  productionRuggedness: Int16Array;
  productionWater: Uint16Array;
  productionBiomeKind: Uint8Array;
}

export interface ContinentalEcoregionWorkerError {
  type: "error";
  revision: number;
  message: string;
}

export type ContinentalEcoregionWorkerRequest = ContinentalEcoregionWorkerQuery;
export type ContinentalEcoregionWorkerResponse =
  | ContinentalEcoregionWorkerSummary
  | ContinentalEcoregionWorkerError;
