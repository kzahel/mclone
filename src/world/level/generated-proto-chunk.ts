import {
  GeneratedChunkStatus,
  generatedChunkStatusIndex,
  type GeneratedChunkStatus as GeneratedChunkStatusName,
} from "./generated-chunk-status";

export const GENERATED_PROTO_CHUNK_CONTENT_VERSION = 1;

export type GeneratedChunkAccessType = "proto" | "level";

export interface GeneratedProtoChunk {
  readonly type: "proto";
  readonly chunkX: number;
  readonly chunkZ: number;
  status: GeneratedChunkStatusName;
  hasBlockSections: boolean;
  isUnsaved: boolean;
  readonly contentVersion: number;
}

export interface GeneratedLevelChunkAccess {
  readonly type: "level";
  readonly chunkX: number;
  readonly chunkZ: number;
  status: typeof GeneratedChunkStatus.FULL;
  hasBlockSections: true;
  isUnsaved: boolean;
  readonly contentVersion: number;
}

export type GeneratedChunkAccess = GeneratedProtoChunk | GeneratedLevelChunkAccess;

export interface GeneratedChunkAccessDebugRecord {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly type: GeneratedChunkAccessType;
  readonly status: GeneratedChunkStatusName;
  readonly hasBlockSections: boolean;
  readonly isUnsaved: boolean;
  readonly contentVersion: number;
}

export function createGeneratedProtoChunk(chunkX: number, chunkZ: number): GeneratedProtoChunk {
  return {
    type: "proto",
    chunkX,
    chunkZ,
    status: GeneratedChunkStatus.EMPTY,
    hasBlockSections: false,
    isUnsaved: false,
    contentVersion: GENERATED_PROTO_CHUNK_CONTENT_VERSION,
  };
}

export function advanceGeneratedChunkAccessStatus(
  access: GeneratedChunkAccess,
  status: GeneratedChunkStatusName,
  hasBlockSections: boolean,
): GeneratedChunkAccess {
  if (access.type === "level") {
    return access;
  }

  const statusAdvanced = generatedChunkStatusIndex(status) > generatedChunkStatusIndex(access.status);
  const sectionsAdvanced = hasBlockSections && !access.hasBlockSections;
  if (!statusAdvanced && !sectionsAdvanced) {
    return access;
  }

  if (status === GeneratedChunkStatus.FULL) {
    return {
      type: "level",
      chunkX: access.chunkX,
      chunkZ: access.chunkZ,
      status: GeneratedChunkStatus.FULL,
      hasBlockSections: true,
      isUnsaved: true,
      contentVersion: access.contentVersion,
    };
  }

  if (statusAdvanced) {
    access.status = status;
  }
  access.hasBlockSections ||= hasBlockSections;
  access.isUnsaved = true;
  return access;
}
