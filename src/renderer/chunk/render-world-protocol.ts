import type { ChunkLightDeltaMessage, ChunkSnapshotMessage, ChunkUnloadMessage } from "../../runtime/protocol/world-messages";
import {
  collectPackedChunkLightDeltaTransferables,
  collectPackedChunkSnapshotTransferables,
} from "../../world/level/packed-chunk-snapshot";
import { collectSectionMeshTransferables, type SectionMeshResult } from "./chunk-mesh-protocol";

export interface RenderWorldSectionOrigin {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

export interface RenderWorldCamera {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

export interface RenderWorldChunkKey {
  readonly chunkX: number;
  readonly chunkZ: number;
}

export interface RenderWorldStats {
  readonly loadedChunkCount: number;
  readonly pendingMeshBuildCount?: number;
}

export type RenderWorldUpdateMessage = ChunkSnapshotMessage | ChunkLightDeltaMessage | ChunkUnloadMessage;

export interface InitializeRenderWorldRequest {
  readonly type: "initialize_render_world";
  readonly seed: bigint;
  readonly minBuildHeight: number;
  readonly height: number;
}

export interface IngestRenderWorldUpdatesRequest {
  readonly type: "ingest_render_world_updates";
  readonly messages: readonly RenderWorldUpdateMessage[];
}

export interface BuildRenderSectionMeshRequest {
  readonly type: "build_render_section_mesh";
  readonly origin: RenderWorldSectionOrigin;
  readonly camera: RenderWorldCamera;
}

export interface GetRenderWorldStatsRequest {
  readonly type: "get_render_world_stats";
}

export interface RenderWorldReadyResponse {
  readonly type: "render_world_ready";
}

export interface RenderWorldStatsResponse {
  readonly type: "render_world_stats";
  readonly stats: RenderWorldStats;
}

export interface RenderWorldDirtySectionsResponse {
  readonly type: "render_world_dirty_sections";
  readonly dirtySections: readonly RenderWorldSectionOrigin[];
  readonly stats: RenderWorldStats;
}

export interface RenderSectionMeshBuiltResponse {
  readonly type: "render_section_mesh_built";
  readonly origin: RenderWorldSectionOrigin;
  readonly result: SectionMeshResult;
}

export type RenderWorldMeshNotReadyReason = "missing_neighbors" | "unloaded_section";

export interface RenderWorldMeshNotReadyResponse {
  readonly type: "render_world_mesh_not_ready";
  readonly origin: RenderWorldSectionOrigin;
  readonly reason: RenderWorldMeshNotReadyReason;
  readonly missingChunks?: readonly RenderWorldChunkKey[];
}

export interface RenderWorldErrorResponse {
  readonly type: "render_world_error";
  readonly message: string;
}

export type RenderWorldRequest =
  | InitializeRenderWorldRequest
  | IngestRenderWorldUpdatesRequest
  | BuildRenderSectionMeshRequest
  | GetRenderWorldStatsRequest;

export type RenderWorldMeshBuildResponse = RenderSectionMeshBuiltResponse | RenderWorldMeshNotReadyResponse;

export type RenderWorldResponse =
  | RenderWorldReadyResponse
  | RenderWorldStatsResponse
  | RenderWorldDirtySectionsResponse
  | RenderWorldMeshBuildResponse
  | RenderWorldErrorResponse;

export function collectRenderWorldRequestTransferables(message: RenderWorldRequest): readonly Transferable[] {
  if (message.type !== "ingest_render_world_updates") {
    return [];
  }

  const transferables: Transferable[] = [];
  for (const update of message.messages) {
    if (update.type === "chunk_snapshot") {
      transferables.push(...collectPackedChunkSnapshotTransferables(update.snapshot));
    } else if (update.type === "chunk_light_delta") {
      transferables.push(...collectPackedChunkLightDeltaTransferables(update.light));
    }
  }
  return transferables;
}

export function collectRenderWorldResponseTransferables(message: RenderWorldResponse): readonly Transferable[] {
  return message.type === "render_section_mesh_built" ? collectSectionMeshTransferables(message.result) : [];
}
