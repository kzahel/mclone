import { describe, expect, test } from "vitest";
import {
  GeneratedChunkStatus,
  getGeneratedChunkDependencyStatus,
  getGeneratedChunkStatusDistance,
  getGeneratedChunkStatusParent,
  getGeneratedStatusAroundFullChunk,
} from "../../../src/world/level/generated-chunk-status";

describe("generated chunk status dependencies", () => {
  test("matches vanilla status parent chain", () => {
    expect(getGeneratedChunkStatusParent(GeneratedChunkStatus.EMPTY)).toBe(GeneratedChunkStatus.EMPTY);
    expect(getGeneratedChunkStatusParent(GeneratedChunkStatus.STRUCTURE_STARTS)).toBe(GeneratedChunkStatus.EMPTY);
    expect(getGeneratedChunkStatusParent(GeneratedChunkStatus.FEATURES)).toBe(GeneratedChunkStatus.LIQUID_CARVERS);
    expect(getGeneratedChunkStatusParent(GeneratedChunkStatus.FULL)).toBe(GeneratedChunkStatus.HEIGHTMAPS);
  });

  test("maps full-chunk distance through vanilla STATUS_BY_RANGE", () => {
    expect(getGeneratedStatusAroundFullChunk(-1)).toBe(GeneratedChunkStatus.FULL);
    expect(getGeneratedStatusAroundFullChunk(0)).toBe(GeneratedChunkStatus.FULL);
    expect(getGeneratedStatusAroundFullChunk(1)).toBe(GeneratedChunkStatus.FEATURES);
    expect(getGeneratedStatusAroundFullChunk(2)).toBe(GeneratedChunkStatus.LIQUID_CARVERS);
    expect(getGeneratedStatusAroundFullChunk(3)).toBe(GeneratedChunkStatus.STRUCTURE_STARTS);
    expect(getGeneratedStatusAroundFullChunk(10)).toBe(GeneratedChunkStatus.STRUCTURE_STARTS);
    expect(getGeneratedStatusAroundFullChunk(11)).toBe(GeneratedChunkStatus.EMPTY);
  });

  test("keeps FEATURES terrain dependencies to the center and one-chunk halo", () => {
    expect(getGeneratedChunkStatusDistance(GeneratedChunkStatus.FEATURES)).toBe(1);
    expect(getGeneratedChunkDependencyStatus(GeneratedChunkStatus.FEATURES, 0)).toBe(GeneratedChunkStatus.LIQUID_CARVERS);
    expect(getGeneratedChunkDependencyStatus(GeneratedChunkStatus.FEATURES, 1)).toBe(GeneratedChunkStatus.LIQUID_CARVERS);

    for (let radius = 2; radius <= 8; radius++) {
      expect(getGeneratedChunkDependencyStatus(GeneratedChunkStatus.FEATURES, radius)).toBe(GeneratedChunkStatus.STRUCTURE_STARTS);
    }
  });
});
