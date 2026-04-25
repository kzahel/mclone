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

const GENERATED_CHUNK_STATUS_PARENT: Readonly<Record<GeneratedChunkStatus, GeneratedChunkStatus>> = {
  [GeneratedChunkStatus.EMPTY]: GeneratedChunkStatus.EMPTY,
  [GeneratedChunkStatus.STRUCTURE_STARTS]: GeneratedChunkStatus.EMPTY,
  [GeneratedChunkStatus.STRUCTURE_REFERENCES]: GeneratedChunkStatus.STRUCTURE_STARTS,
  [GeneratedChunkStatus.BIOMES]: GeneratedChunkStatus.STRUCTURE_REFERENCES,
  [GeneratedChunkStatus.NOISE]: GeneratedChunkStatus.BIOMES,
  [GeneratedChunkStatus.SURFACE]: GeneratedChunkStatus.NOISE,
  [GeneratedChunkStatus.CARVERS]: GeneratedChunkStatus.SURFACE,
  [GeneratedChunkStatus.LIQUID_CARVERS]: GeneratedChunkStatus.CARVERS,
  [GeneratedChunkStatus.FEATURES]: GeneratedChunkStatus.LIQUID_CARVERS,
  [GeneratedChunkStatus.LIGHT]: GeneratedChunkStatus.FEATURES,
  [GeneratedChunkStatus.SPAWN]: GeneratedChunkStatus.LIGHT,
  [GeneratedChunkStatus.HEIGHTMAPS]: GeneratedChunkStatus.SPAWN,
  [GeneratedChunkStatus.FULL]: GeneratedChunkStatus.HEIGHTMAPS,
};

const GENERATED_STATUS_BY_RANGE = [
  GeneratedChunkStatus.FULL,
  GeneratedChunkStatus.FEATURES,
  GeneratedChunkStatus.LIQUID_CARVERS,
  GeneratedChunkStatus.STRUCTURE_STARTS,
  GeneratedChunkStatus.STRUCTURE_STARTS,
  GeneratedChunkStatus.STRUCTURE_STARTS,
  GeneratedChunkStatus.STRUCTURE_STARTS,
  GeneratedChunkStatus.STRUCTURE_STARTS,
  GeneratedChunkStatus.STRUCTURE_STARTS,
  GeneratedChunkStatus.STRUCTURE_STARTS,
  GeneratedChunkStatus.STRUCTURE_STARTS,
] as const satisfies readonly GeneratedChunkStatus[];

const GENERATED_STATUS_DISTANCE: Readonly<Record<GeneratedChunkStatus, number>> = (() => {
  const distances = new Map<GeneratedChunkStatus, number>();
  let range = 0;
  for (let statusIndex = GENERATED_CHUNK_STATUSES.length - 1; statusIndex >= 0; statusIndex--) {
    while (
      range + 1 < GENERATED_STATUS_BY_RANGE.length
      && statusIndex <= generatedChunkStatusIndex(GENERATED_STATUS_BY_RANGE[range + 1]!)
    ) {
      range++;
    }

    distances.set(GENERATED_CHUNK_STATUSES[statusIndex]!, range);
  }

  return Object.fromEntries(distances) as Record<GeneratedChunkStatus, number>;
})();

export function generatedChunkStatusIndex(status: GeneratedChunkStatus): number {
  return GENERATED_CHUNK_STATUS_INDEX[status];
}

export function getGeneratedChunkStatusParent(status: GeneratedChunkStatus): GeneratedChunkStatus {
  return GENERATED_CHUNK_STATUS_PARENT[status];
}

export function getGeneratedStatusAroundFullChunk(distance: number): GeneratedChunkStatus {
  if (distance >= GENERATED_STATUS_BY_RANGE.length) {
    return GeneratedChunkStatus.EMPTY;
  }

  return distance < 0 ? GeneratedChunkStatus.FULL : GENERATED_STATUS_BY_RANGE[distance]!;
}

export function getGeneratedChunkStatusDistance(status: GeneratedChunkStatus): number {
  return GENERATED_STATUS_DISTANCE[status];
}

export function getGeneratedChunkDependencyStatus(
  requestedStatus: GeneratedChunkStatus,
  radius: number,
): GeneratedChunkStatus {
  if (radius === 0) {
    return getGeneratedChunkStatusParent(requestedStatus);
  }

  return getGeneratedStatusAroundFullChunk(getGeneratedChunkStatusDistance(requestedStatus) + radius);
}

export function isGeneratedChunkStatusAtLeast(
  actual: GeneratedChunkStatus,
  required: GeneratedChunkStatus,
): boolean {
  return generatedChunkStatusIndex(actual) >= generatedChunkStatusIndex(required);
}
