import {
  GeneratedChunkStatus,
  generatedChunkStatusIndex,
  type GeneratedChunkStatus as GeneratedChunkStatusName,
} from "./generated-chunk-status";
import {
  clonePackedChunkSnapshot,
  type PackedChunkSnapshot,
} from "./packed-chunk-snapshot";

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

export interface GeneratedChunkStorageRecord {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly type: GeneratedChunkAccessType;
  readonly status: GeneratedChunkStatusName;
  readonly hasBlockSections: boolean;
  readonly isUnsaved: boolean;
  readonly contentVersion: number;
  readonly writeVersion: number;
  readonly snapshot?: PackedChunkSnapshot;
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

export function createGeneratedChunkStorageRecord(
  access: GeneratedChunkAccess,
  snapshot?: PackedChunkSnapshot,
  writeVersion = 0,
): GeneratedChunkStorageRecord {
  if (access.hasBlockSections && snapshot === undefined) {
    throw new Error(`Generated chunk (${access.chunkX.toString()}, ${access.chunkZ.toString()}) with block sections needs a snapshot`);
  }
  if (snapshot !== undefined && (snapshot.chunkX !== access.chunkX || snapshot.chunkZ !== access.chunkZ)) {
    throw new Error(
      `Generated chunk snapshot (${snapshot.chunkX.toString()}, ${snapshot.chunkZ.toString()}) does not match access (${access.chunkX.toString()}, ${access.chunkZ.toString()})`,
    );
  }

  const record: GeneratedChunkStorageRecord = {
    chunkX: access.chunkX,
    chunkZ: access.chunkZ,
    type: access.type,
    status: access.status,
    hasBlockSections: access.hasBlockSections,
    isUnsaved: false,
    contentVersion: access.contentVersion,
    writeVersion,
  };
  return snapshot === undefined ? record : { ...record, snapshot: clonePackedChunkSnapshot(snapshot) };
}

export function cloneGeneratedChunkStorageRecord(record: GeneratedChunkStorageRecord): GeneratedChunkStorageRecord {
  const clone: GeneratedChunkStorageRecord = {
    chunkX: record.chunkX,
    chunkZ: record.chunkZ,
    type: record.type,
    status: record.status,
    hasBlockSections: record.hasBlockSections,
    isUnsaved: record.isUnsaved,
    contentVersion: record.contentVersion,
    writeVersion: record.writeVersion,
  };
  return record.snapshot === undefined ? clone : { ...clone, snapshot: clonePackedChunkSnapshot(record.snapshot) };
}

export function createGeneratedChunkAccessFromStorageRecord(record: GeneratedChunkStorageRecord): GeneratedChunkAccess {
  if (record.type === "level") {
    return {
      type: "level",
      chunkX: record.chunkX,
      chunkZ: record.chunkZ,
      status: GeneratedChunkStatus.FULL,
      hasBlockSections: true,
      isUnsaved: record.isUnsaved,
      contentVersion: record.contentVersion,
    };
  }

  return {
    type: "proto",
    chunkX: record.chunkX,
    chunkZ: record.chunkZ,
    status: record.status,
    hasBlockSections: record.hasBlockSections,
    isUnsaved: record.isUnsaved,
    contentVersion: record.contentVersion,
  };
}
