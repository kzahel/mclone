import type { PackedChunkLight, PackedChunkLightDelta } from "../../world/level/packed-chunk-snapshot";

export interface LightingChunkRevision {
  readonly chunkViewRevision: number;
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly chunkRevision: number;
}

export interface ConfigureLightingWorldRequest {
  readonly type: "configure_light_world";
  readonly seed: bigint;
  readonly minBuildHeight: number;
  readonly height: number;
  readonly blockRegistryVersion: number;
}

export interface SetLightingViewRequest {
  readonly type: "set_light_view";
  readonly chunkViewRevision: number;
  readonly centerChunkX: number;
  readonly centerChunkZ: number;
  readonly loadRadius: number;
  readonly publishRadius: number;
}

export interface PackedLightInputSection {
  readonly y: number;
  readonly blockStateIds: Uint16Array | Uint32Array;
}

export interface UpsertLightChunkRequest extends LightingChunkRevision {
  readonly type: "upsert_light_chunk";
  readonly decorated: boolean;
  readonly sections: readonly PackedLightInputSection[];
}

export interface RemoveLightChunkRequest extends LightingChunkRevision {
  readonly type: "remove_light_chunk";
}

export interface LightingNeighborRevision {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly chunkRevision: number;
}

export interface RequestInitialLightRequest extends LightingChunkRevision {
  readonly type: "request_initial_light";
  readonly neighbors: readonly LightingNeighborRevision[];
}

export interface LightBlockChange {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly oldBlockStateId: number;
  readonly newBlockStateId: number;
}

export interface LightBlockChangeBatchRequest {
  readonly type: "block_light_update_batch";
  readonly batchId: number;
  readonly chunkViewRevision: number;
  readonly changes: readonly LightBlockChange[];
  readonly chunkRevisions: readonly LightingChunkRevision[];
}

export interface PollLightingResultsRequest {
  readonly type: "poll_light_results";
  readonly maxResults?: number;
}

export type LightingCommandRequest =
  | ConfigureLightingWorldRequest
  | SetLightingViewRequest
  | UpsertLightChunkRequest
  | RemoveLightChunkRequest
  | RequestInitialLightRequest
  | LightBlockChangeBatchRequest;

export type LightingWorkerRequest = LightingCommandRequest | PollLightingResultsRequest;

export interface ChunkLightReadyResult extends LightingChunkRevision {
  readonly type: "chunk_light_ready";
  readonly light: PackedChunkLight;
}

export interface ChunkLightDeltaResult extends LightingChunkRevision {
  readonly type: "chunk_light_delta";
  readonly light: PackedChunkLightDelta;
}

export interface LightBlockChangeBatchCompleteResult {
  readonly type: "block_light_update_complete";
  readonly batchId: number;
  readonly chunkViewRevision: number;
  readonly changeCount: number;
  readonly chunkRevisions: readonly LightingChunkRevision[];
}

export interface LightProgressResult {
  readonly type: "light_progress";
  readonly stage: string;
  readonly current: number;
  readonly total: number;
  readonly chunkViewRevision?: number;
  readonly chunkX?: number;
  readonly chunkZ?: number;
  readonly chunkRevision?: number;
}

export interface LightErrorResult {
  readonly type: "light_error";
  readonly message: string;
  readonly chunkViewRevision?: number;
  readonly chunkX?: number;
  readonly chunkZ?: number;
  readonly chunkRevision?: number;
}

export type LightingResult =
  | ChunkLightReadyResult
  | ChunkLightDeltaResult
  | LightBlockChangeBatchCompleteResult
  | LightProgressResult
  | LightErrorResult;

export interface LightingAckResponse {
  readonly type: "lighting_ack";
}

export interface LightingResultBatch {
  readonly type: "lighting_result_batch";
  readonly results: readonly LightingResult[];
  readonly pendingResultCount: number;
}

export interface LightingWorkerErrorResponse {
  readonly type: "lighting_worker_error";
  readonly message: string;
}

export type LightingWorkerResponse = LightingAckResponse | LightingResultBatch | LightingWorkerErrorResponse;

export interface LightingService {
  configureWorld(request: ConfigureLightingWorldRequest): Promise<void>;
  setView(request: SetLightingViewRequest): Promise<void>;
  upsertChunk(request: UpsertLightChunkRequest): Promise<void>;
  removeChunk(request: RemoveLightChunkRequest): Promise<void>;
  requestInitialLight(request: RequestInitialLightRequest): Promise<void>;
  enqueueBlockChanges(request: LightBlockChangeBatchRequest): Promise<void>;
  pollResults(request: PollLightingResultsRequest): Promise<LightingResultBatch>;
}

export function collectLightingRequestTransferables(request: LightingWorkerRequest): Transferable[] {
  const transferables: Transferable[] = [];
  if (request.type !== "upsert_light_chunk") {
    return transferables;
  }

  for (const section of request.sections) {
    transferables.push(section.blockStateIds.buffer);
  }

  return transferables;
}

export function collectLightingResultTransferables(result: LightingResult): Transferable[] {
  const transferables: Transferable[] = [];
  switch (result.type) {
    case "chunk_light_ready":
      for (const section of [...result.light.sky, ...result.light.block]) {
        transferables.push(section.data.buffer);
      }
      break;
    case "chunk_light_delta":
      for (const section of [...(result.light.sky ?? []), ...(result.light.block ?? [])]) {
        if (section.data !== undefined) {
          transferables.push(section.data.buffer);
        }
      }
      break;
    case "block_light_update_complete":
    case "light_progress":
    case "light_error":
      break;
  }

  return transferables;
}

export function collectLightingResponseTransferables(response: LightingWorkerResponse): Transferable[] {
  if (response.type !== "lighting_result_batch") {
    return [];
  }

  return response.results.flatMap(collectLightingResultTransferables);
}
