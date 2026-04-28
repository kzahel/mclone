import { describe, expect, test } from "vitest";

import { FullChunkStatus } from "../../../src/world/level/entity/full-chunk-status";
import {
  GENERATED_CHUNK_ENTITY_TICKING_LEVEL,
  GENERATED_CHUNK_FORCED_LEVEL,
  GENERATED_CHUNK_LIGHT_LEVEL,
  GeneratedChunkTicketSet,
} from "../../../src/world/level/generated-chunk-tickets";

describe("GeneratedChunkTicketSet", () => {
  test("derives full status from propagated ticket levels", () => {
    const tickets = new GeneratedChunkTicketSet();
    tickets.replaceSource("player_view", [{
      source: "player_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
      level: GENERATED_CHUNK_ENTITY_TICKING_LEVEL,
    }]);

    expect(tickets.getFullStatus(0, 0)).toBe(FullChunkStatus.ENTITY_TICKING);
    expect(tickets.getFullStatus(1, 0)).toBe(FullChunkStatus.TICKING);
    expect(tickets.getFullStatus(2, 0)).toBe(FullChunkStatus.BORDER);
    expect(tickets.getFullStatus(3, 0)).toBe(FullChunkStatus.INACCESSIBLE);
    expect(tickets.getCoveredChunkCount()).toBe(1);
    expect(tickets.getDebugLevelRecords()).toHaveLength(25);
  });

  test("coalesces a source area before propagating ticket levels", () => {
    const tickets = new GeneratedChunkTicketSet();
    tickets.replaceSource("player_view", [{
      source: "player_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 3,
      sourceRadius: 1,
      level: GENERATED_CHUNK_ENTITY_TICKING_LEVEL,
    }]);

    expect(tickets.getFullStatus(0, 0)).toBe(FullChunkStatus.ENTITY_TICKING);
    expect(tickets.getFullStatus(1, 1)).toBe(FullChunkStatus.ENTITY_TICKING);
    expect(tickets.getFullStatus(2, 0)).toBe(FullChunkStatus.TICKING);
    expect(tickets.getFullStatus(3, 0)).toBe(FullChunkStatus.BORDER);
    expect(tickets.getFullStatus(4, 0)).toBe(FullChunkStatus.INACCESSIBLE);
    expect(tickets.getDebugLevelRecords().filter((record) => record.status === FullChunkStatus.ENTITY_TICKING)).toHaveLength(9);
    expect(tickets.getDebugLevelRecords().filter((record) => record.status === FullChunkStatus.TICKING)).toHaveLength(16);
    expect(tickets.getDebugLevelRecords().filter((record) => record.status === FullChunkStatus.BORDER)).toHaveLength(24);
  });

  test("derives light and forced full status through the same level path", () => {
    const tickets = new GeneratedChunkTicketSet();
    tickets.replaceSource("light", [{
      source: "light",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
      level: GENERATED_CHUNK_LIGHT_LEVEL,
    }]);
    tickets.replaceSource("forced", [{
      source: "forced",
      centerChunkX: 4,
      centerChunkZ: 0,
      radius: GENERATED_CHUNK_LIGHT_LEVEL - GENERATED_CHUNK_FORCED_LEVEL,
      level: GENERATED_CHUNK_FORCED_LEVEL,
    }]);

    expect(tickets.getFullStatus(0, 0)).toBe(FullChunkStatus.BORDER);
    expect(tickets.getFullStatus(4, 0)).toBe(FullChunkStatus.ENTITY_TICKING);
    expect(tickets.getFullStatus(5, 0)).toBe(FullChunkStatus.TICKING);
    expect(tickets.getFullStatus(6, 0)).toBe(FullChunkStatus.BORDER);
    expect(tickets.getFullStatus(7, 0)).toBe(FullChunkStatus.INACCESSIBLE);
  });
});
