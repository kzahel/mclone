import type { FullChunkStatus } from "../../world/level/entity/full-chunk-status";
import type { GeneratedChunkStatus as GeneratedChunkStatusName } from "../../world/level/generated-chunk-status";
import type { GeneratedChunkTicketChunkSourceDebugRecord } from "../../world/level/generated-chunk-tickets";
import type { GeneratedChunkAccessType } from "../../world/level/generated-proto-chunk";

export interface WorldDebugRequestOptions {
  readonly chunkLifecycle?: boolean;
}

export type GeneratedChunkLifecycleJobState = "pending" | "fulfilled" | "rejected";
export type GeneratedChunkLifecyclePreloadResult = "loaded" | "already_loaded" | "missing" | "stale";

export interface GeneratedChunkLifecycleViewState {
  readonly centerChunkX: number;
  readonly centerChunkZ: number;
  readonly radius: number;
}

export type GeneratedChunkPublicationBlockerKind =
  | "outside_publish_view"
  | "already_published"
  | "dirty_published"
  | "missing_materialized_chunk"
  | "waiting_for_full_neighbor"
  | "waiting_for_light"
  | "ready_to_publish";

export interface GeneratedChunkPublicationBlocker {
  readonly kind: GeneratedChunkPublicationBlockerKind;
  readonly chunkX?: number;
  readonly chunkZ?: number;
  readonly requiredStatus?: GeneratedChunkStatusName;
  readonly actualStatus?: GeneratedChunkStatusName;
}

export interface GeneratedChunkLifecycleStatusJobDebugRecord {
  readonly status: GeneratedChunkStatusName;
  readonly state: GeneratedChunkLifecycleJobState;
  readonly chunkViewJobRevision: number;
  readonly result?: boolean;
}

export interface GeneratedChunkLifecyclePreloadDebugRecord {
  readonly state: GeneratedChunkLifecycleJobState;
  readonly chunkViewJobRevision: number;
  readonly result?: GeneratedChunkLifecyclePreloadResult;
}

export interface GeneratedChunkLifecycleAccessDebugRecord {
  readonly type: GeneratedChunkAccessType;
  readonly status: GeneratedChunkStatusName;
  readonly hasBlockSections: boolean;
  readonly isUnsaved: boolean;
  readonly contentVersion: number;
}

export interface GeneratedChunkLifecycleRecord {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly inAuthorityView: boolean;
  readonly inFullView: boolean;
  readonly inPublishView: boolean;
  readonly ticketLevel: number;
  readonly ticketFullStatus: FullChunkStatus;
  readonly ticketSources: readonly GeneratedChunkTicketChunkSourceDebugRecord[];
  readonly generatedStatus: GeneratedChunkStatusName;
  readonly hasBlockSections: boolean;
  readonly chunkLoaded: boolean;
  readonly holderFullStatus?: FullChunkStatus;
  readonly entityStatus?: FullChunkStatus;
  readonly access?: GeneratedChunkLifecycleAccessDebugRecord;
  readonly preload?: GeneratedChunkLifecyclePreloadDebugRecord;
  readonly statusJobs: readonly GeneratedChunkLifecycleStatusJobDebugRecord[];
  readonly published: boolean;
  readonly dirtyForPublication: boolean;
  readonly dirtyDurable: boolean;
  readonly queuedForUnload: boolean;
  readonly pendingUnload: boolean;
  readonly pendingStorageWrite: boolean;
  readonly lightInputSent: boolean;
  readonly lightAccepted: boolean;
  readonly publicationBlocker: GeneratedChunkPublicationBlocker;
}

export interface GeneratedChunkLifecycleCounts {
  readonly total: number;
  readonly inAuthorityView: number;
  readonly inPublishView: number;
  readonly loaded: number;
  readonly materialized: number;
  readonly published: number;
  readonly dirtyForPublication: number;
  readonly queuedForUnload: number;
  readonly pendingUnload: number;
  readonly byGeneratedStatus: Readonly<Record<string, number>>;
  readonly byHolderFullStatus: Readonly<Record<string, number>>;
  readonly byPublicationBlocker: Readonly<Record<string, number>>;
}

export interface GeneratedChunkLifecycleSnapshot {
  readonly currentChunkView?: GeneratedChunkLifecycleViewState;
  readonly chunkViewJobRevision: number;
  readonly activeChunkViewJobRevision?: number;
  readonly records: readonly GeneratedChunkLifecycleRecord[];
  readonly counts: GeneratedChunkLifecycleCounts;
}
