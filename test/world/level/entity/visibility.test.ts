import { describe, expect, test } from "vitest";
import { FullChunkStatus } from "../../../../src/world/level/entity/full-chunk-status";
import {
  isVisibilityAccessible,
  isVisibilityTicking,
  visibilityFromFullChunkStatus,
  Visibility,
} from "../../../../src/world/level/entity/visibility";

describe("entity visibility", () => {
  test("maps full chunk status through vanilla entity visibility", () => {
    expect(visibilityFromFullChunkStatus(FullChunkStatus.INACCESSIBLE)).toBe(Visibility.HIDDEN);
    expect(visibilityFromFullChunkStatus(FullChunkStatus.BORDER)).toBe(Visibility.TRACKED);
    expect(visibilityFromFullChunkStatus(FullChunkStatus.TICKING)).toBe(Visibility.TRACKED);
    expect(visibilityFromFullChunkStatus(FullChunkStatus.ENTITY_TICKING)).toBe(Visibility.TICKING);
  });

  test("exposes accessible and ticking flags", () => {
    expect(isVisibilityAccessible(Visibility.HIDDEN)).toBe(false);
    expect(isVisibilityAccessible(Visibility.TRACKED)).toBe(true);
    expect(isVisibilityAccessible(Visibility.TICKING)).toBe(true);
    expect(isVisibilityTicking(Visibility.TRACKED)).toBe(false);
    expect(isVisibilityTicking(Visibility.TICKING)).toBe(true);
  });
});
