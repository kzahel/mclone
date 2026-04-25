export const GENERATED_CHUNK_STATUSES = [
  "empty",
  "structure_starts",
  "structure_references",
  "biomes",
  "noise",
  "surface",
  "carvers",
  "liquid_carvers",
  "features",
  "light",
  "spawn",
  "heightmaps",
  "full",
] as const;

export type GeneratedChunkStatus = typeof GENERATED_CHUNK_STATUSES[number];

export const GeneratedChunkStatus = {
  EMPTY: "empty",
  STRUCTURE_STARTS: "structure_starts",
  STRUCTURE_REFERENCES: "structure_references",
  BIOMES: "biomes",
  NOISE: "noise",
  SURFACE: "surface",
  CARVERS: "carvers",
  LIQUID_CARVERS: "liquid_carvers",
  FEATURES: "features",
  LIGHT: "light",
  SPAWN: "spawn",
  HEIGHTMAPS: "heightmaps",
  FULL: "full",
} as const satisfies Record<string, GeneratedChunkStatus>;

const GENERATED_CHUNK_STATUS_INDEX: Readonly<Record<GeneratedChunkStatus, number>> =
  Object.fromEntries(GENERATED_CHUNK_STATUSES.map((status, index) => [status, index])) as Record<GeneratedChunkStatus, number>;

export function generatedChunkStatusIndex(status: GeneratedChunkStatus): number {
  return GENERATED_CHUNK_STATUS_INDEX[status];
}

export function isGeneratedChunkStatusAtLeast(
  actual: GeneratedChunkStatus,
  required: GeneratedChunkStatus,
): boolean {
  return generatedChunkStatusIndex(actual) >= generatedChunkStatusIndex(required);
}
