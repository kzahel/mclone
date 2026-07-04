import type { TextureCandidateEntry, TextureImageRef } from "../core/index-model";

export function primaryCandidateImage(candidate: TextureCandidateEntry): TextureImageRef | null {
  return (
    [
      candidate.images.projected,
      candidate.images.raw,
      candidate.images.rawTile,
      candidate.images.reviewSheet,
      candidate.images.contactSheet,
    ].find((image) => image.exists) ?? null
  );
}
