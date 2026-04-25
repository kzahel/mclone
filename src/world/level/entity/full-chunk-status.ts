export const FULL_CHUNK_STATUSES = [
  "inaccessible",
  "border",
  "ticking",
  "entity_ticking",
] as const;

export type FullChunkStatus = typeof FULL_CHUNK_STATUSES[number];

export const FullChunkStatus = {
  INACCESSIBLE: "inaccessible",
  BORDER: "border",
  TICKING: "ticking",
  ENTITY_TICKING: "entity_ticking",
} as const satisfies Record<string, FullChunkStatus>;

const FULL_CHUNK_STATUS_INDEX: Readonly<Record<FullChunkStatus, number>> =
  Object.fromEntries(FULL_CHUNK_STATUSES.map((status, index) => [status, index])) as Record<FullChunkStatus, number>;

export function fullChunkStatusIndex(status: FullChunkStatus): number {
  return FULL_CHUNK_STATUS_INDEX[status];
}

export function isFullChunkStatusOrAfter(actual: FullChunkStatus, required: FullChunkStatus): boolean {
  return fullChunkStatusIndex(actual) >= fullChunkStatusIndex(required);
}
