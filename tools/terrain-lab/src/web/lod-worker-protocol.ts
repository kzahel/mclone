export interface LodWorkerInit {
  type: "init";
  mode: "exact" | "macro";
  epoch: number;
  seed: string;
}

export interface LodWorkerCompile {
  type: "compile";
  epoch: number;
  revision: number;
  seed: string;
  tileX: number;
  tileZ: number;
  sampleSpacing: number;
  surfaceQuality: "basic" | "inferred";
}

export type LodWorkerRequest = LodWorkerInit | LodWorkerCompile;

export interface LodWorkerReady {
  type: "ready";
  epoch: number;
}

export interface LodWorkerResult {
  type: "result";
  epoch: number;
  revision: number;
  seed: string;
  tileX: number;
  tileZ: number;
  sampleSpacing: number;
  surfaceQuality: "basic" | "inferred";
  compileMs: number;
  generatedDensityColumns: number;
  reusedDensityColumns: number;
  retainedDensityColumns: number;
  samples: Float32Array;
}

export interface LodWorkerError {
  type: "error";
  epoch: number;
  revision?: number;
  seed?: string;
  tileX?: number;
  tileZ?: number;
  sampleSpacing?: number;
  surfaceQuality?: "basic" | "inferred";
  message: string;
}

export type LodWorkerResponse =
  | LodWorkerReady
  | LodWorkerResult
  | LodWorkerError;
