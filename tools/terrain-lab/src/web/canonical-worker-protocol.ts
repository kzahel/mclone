import type { CanonicalTerrainStage } from "../state";

export interface CanonicalWorkerInit {
  type: "init";
  epoch: number;
  seed: string;
  stage: CanonicalTerrainStage;
}

export interface CanonicalWorkerCompile {
  type: "compile";
  epoch: number;
  chunkX: number;
  chunkZ: number;
}

export type CanonicalWorkerRequest = CanonicalWorkerInit | CanonicalWorkerCompile;

export interface CanonicalWorkerReady {
  type: "ready";
  epoch: number;
}

export interface CanonicalWorkerResult {
  type: "result";
  epoch: number;
  chunkX: number;
  chunkZ: number;
  minY: number;
  height: number;
  fingerprint: string;
  generationMs: number;
  dependencyCacheHits: number;
  generatedDependencyChunks: number;
  retainedDependencyChunks: number;
  blocks: Uint8Array;
  biomes: Int32Array;
}

export interface CanonicalWorkerError {
  type: "error";
  epoch: number;
  message: string;
}

export type CanonicalWorkerResponse =
  | CanonicalWorkerReady
  | CanonicalWorkerResult
  | CanonicalWorkerError;
