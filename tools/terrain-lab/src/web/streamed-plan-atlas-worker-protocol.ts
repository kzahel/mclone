import type {
  StreamedPlanAtlasTopology,
} from "../state";

export interface StreamedPlanAtlasWorkerQuery {
  type: "query";
  revision: number;
  seed: string;
  topology: StreamedPlanAtlasTopology;
  centerX: number;
  centerZ: number;
  blocksAcross: number;
  aspectRatio: number;
  clearCache: boolean;
}

export type StreamedPlanAtlasWorkerRequest = StreamedPlanAtlasWorkerQuery;

export interface AtlasCoverage {
  minX: number;
  minZ: number;
  maxX: number;
  maxZ: number;
  regionColumns: number;
  regionRows: number;
  clipped: boolean;
}

export interface AtlasCandidateReceipt {
  candidate: string;
  candidateRevision: string;
  semanticSha256: string;
  viewedPlanCount: number;
  canonicalPlanCount: number;
  cacheHits: number;
  cacheMisses: number;
  cacheEvictions: number;
  retainedPlans: number;
}

export interface AtlasRegionIdentity {
  requestedRegionX: number;
  requestedRegionZ: number;
  canonicalRegionX: number;
  canonicalRegionZ: number;
  worldMinX: number;
  worldMinZ: number;
}

export interface FallbackAtlasCell extends AtlasRegionIdentity {
  regionClass: number;
}

export interface FallbackAtlasSample {
  worldX: number;
  worldZ: number;
  surfaceY: number;
  waterSurfaceY?: number;
  ridgeLift: number;
  channel: boolean;
  island: boolean;
  pond: boolean;
  topBlock: number;
}

export interface FallbackAtlasFeature {
  canonicalId: string;
  canonicalOwnerX: number;
  canonicalOwnerZ: number;
  worldX: number;
  worldZ: number;
  radiusBlocks: number;
}

export interface FallbackAtlas extends AtlasCandidateReceipt {
  cells: FallbackAtlasCell[];
  samples: FallbackAtlasSample[];
  features: FallbackAtlasFeature[];
}

export interface HierarchyAtlasCell extends AtlasRegionIdentity {
  rootTendency: number;
  midTendency: number;
  portMask: number;
  character: number;
}

export interface HierarchyAtlasProvider {
  level: number;
  worldMinX: number;
  worldMinZ: number;
  extentBlocks: number;
  tendency: number;
  class: number;
}

export interface HierarchyAtlasFacet {
  vertical: boolean;
  worldAX: number;
  worldAZ: number;
  worldBX: number;
  worldBZ: number;
  open: boolean;
  tendency: number;
  direction: number;
  strength: number;
}

export interface HierarchyAtlas extends AtlasCandidateReceipt {
  cells: HierarchyAtlasCell[];
  providers: HierarchyAtlasProvider[];
  facets: HierarchyAtlasFacet[];
}

export interface FeatureGraphAtlasCell extends AtlasRegionIdentity {
  ownerCellsExamined: number;
  acceptedGraphs: number;
}

export interface FeatureGraphAtlasNode {
  index: number;
  worldX: number;
  worldZ: number;
  potential: number;
}

export interface FeatureGraphAtlasEdge {
  from: number;
  to: number;
  width: number;
}

export interface FeatureGraphAtlasGraph {
  canonicalOwnerX: number;
  canonicalOwnerZ: number;
  anchorX: number;
  anchorZ: number;
  boundsMinX: number;
  boundsMinZ: number;
  boundsMaxX: number;
  boundsMaxZ: number;
  maximumReachBlocks: number;
  nodes: FeatureGraphAtlasNode[];
  edges: FeatureGraphAtlasEdge[];
  sinkNode: number;
  sinkKind: number;
}

export interface FeatureGraphAtlas extends AtlasCandidateReceipt {
  cells: FeatureGraphAtlasCell[];
  graphs: FeatureGraphAtlasGraph[];
}

export interface StreamedPlanAtlasSummary {
  schema: string;
  researchOnly: boolean;
  productionTerrainUnchanged: boolean;
  phaseTwoWitnessSha256: string;
  seed: string;
  topology: string;
  periodBlocks?: number;
  centerX: number;
  centerZ: number;
  blocksAcross: number;
  viewHeightBlocks: number;
  baseRegionBlocks: number;
  coverage: AtlasCoverage;
  fallback: FallbackAtlas;
  hierarchy: HierarchyAtlas;
  featureGraph: FeatureGraphAtlas;
}

export interface StreamedPlanAtlasWorkerSummary {
  type: "summary";
  revision: number;
  queryMs: number;
  summary: StreamedPlanAtlasSummary;
}

export interface StreamedPlanAtlasWorkerError {
  type: "error";
  revision: number;
  message: string;
}

export type StreamedPlanAtlasWorkerResponse =
  | StreamedPlanAtlasWorkerSummary
  | StreamedPlanAtlasWorkerError;
