import type {
  CanonicalTerrainStage,
  TerrainLabProfile,
  TerrainLabTexturePresentation,
  TerrainLabVisualProfile,
} from "../state";

export interface CanonicalCoordinate {
  chunkX: number;
  chunkZ: number;
}

export interface CanonicalWorkerInit {
  type: "init";
  epoch: number;
  profile: TerrainLabProfile;
  visualProfile: TerrainLabVisualProfile;
  texturePresentation: TerrainLabTexturePresentation;
  seed: string;
  stage: CanonicalTerrainStage;
}

export interface CanonicalWorkerBegin {
  type: "begin";
  epoch: number;
  coordinates: CanonicalCoordinate[];
  waterVisible: boolean;
  vegetationVisible: boolean;
  cacheEnabled: boolean;
}

export interface CanonicalWorkerCompileBatch {
  type: "compileBatch";
  epoch: number;
  coordinates: CanonicalCoordinate[];
}

export type CanonicalWorkerRequest =
  | CanonicalWorkerInit
  | CanonicalWorkerBegin
  | CanonicalWorkerCompileBatch;

export interface CanonicalWorkerReady {
  type: "ready";
  epoch: number;
}

export interface CanonicalWorkerBegan {
  type: "began";
  epoch: number;
  rawCacheChunks: number;
  rawCacheBytes: number;
}

export interface CanonicalWorkerPackedAdmission {
  chunkX: number;
  chunkZ: number;
  fingerprint: string;
  rawCacheHit: boolean;
  dependencyCacheHits: number;
  generatedDependencyChunks: number;
  retainedDependencyChunks: number;
  targetChunks: number;
  sectionCount: number;
  vertexCount: number;
  indexCount: number;
  packedSections: Uint8Array;
}

export interface CanonicalWorkerBatchResult {
  type: "batch";
  epoch: number;
  generationMs: number;
  presentationMs: number;
  meshMs: number;
  packMs: number;
  transferMs: number;
  deduplicatedTargetChunks: number;
  rawCacheChunks: number;
  rawCacheBytes: number;
  admissions: CanonicalWorkerPackedAdmission[];
}

export interface CanonicalWorkerError {
  type: "error";
  epoch: number;
  message: string;
}

export type CanonicalWorkerResponse =
  | CanonicalWorkerReady
  | CanonicalWorkerBegan
  | CanonicalWorkerBatchResult
  | CanonicalWorkerError;
