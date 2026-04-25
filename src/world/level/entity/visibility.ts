import { FullChunkStatus, isFullChunkStatusOrAfter, type FullChunkStatus as FullChunkStatusValue } from "./full-chunk-status";

export const VISIBILITIES = [
  "hidden",
  "tracked",
  "ticking",
] as const;

export type Visibility = typeof VISIBILITIES[number];

export const Visibility = {
  HIDDEN: "hidden",
  TRACKED: "tracked",
  TICKING: "ticking",
} as const satisfies Record<string, Visibility>;

export function isVisibilityAccessible(visibility: Visibility): boolean {
  return visibility !== Visibility.HIDDEN;
}

export function isVisibilityTicking(visibility: Visibility): boolean {
  return visibility === Visibility.TICKING;
}

export function visibilityFromFullChunkStatus(status: FullChunkStatusValue): Visibility {
  if (isFullChunkStatusOrAfter(status, FullChunkStatus.ENTITY_TICKING)) {
    return Visibility.TICKING;
  }

  return isFullChunkStatusOrAfter(status, FullChunkStatus.BORDER) ? Visibility.TRACKED : Visibility.HIDDEN;
}
