import type { ChunkSnapshot } from "../../world/level/chunk-snapshot";

export interface OpenWorldRequest {
  readonly type: "open_world";
}

export interface SetChunkViewRequest {
  readonly type: "set_chunk_view";
  readonly centerChunkX: number;
  readonly centerChunkZ: number;
  readonly radius: number;
}

export interface WorldOpenedMessage {
  readonly type: "world_opened";
  readonly minBuildHeight: number;
  readonly height: number;
}

export interface ChunkSnapshotMessage {
  readonly type: "chunk_snapshot";
  readonly snapshot: ChunkSnapshot;
}

export interface ChunkUnloadMessage {
  readonly type: "chunk_unload";
  readonly chunkX: number;
  readonly chunkZ: number;
}

export interface WorldErrorMessage {
  readonly type: "world_error";
  readonly message: string;
}

export type WorldClientMessage = OpenWorldRequest | SetChunkViewRequest;
export type WorldHostMessage = WorldOpenedMessage | ChunkSnapshotMessage | ChunkUnloadMessage | WorldErrorMessage;
