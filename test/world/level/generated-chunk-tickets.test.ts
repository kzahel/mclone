import { describe, expect, test } from "vitest";

import { FullChunkStatus } from "../../../src/world/level/entity/full-chunk-status";
import {
  GENERATED_CHUNK_ENTITY_TICKING_LEVEL,
  GeneratedChunkTicketSet,
} from "../../../src/world/level/generated-chunk-tickets";

describe("GeneratedChunkTicketSet", () => {
  test("derives full status from propagated ticket levels", () => {
    const tickets = new GeneratedChunkTicketSet();
    tickets.replaceSource("player_view", [{
      source: "player_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 2,
      level: GENERATED_CHUNK_ENTITY_TICKING_LEVEL,
    }]);

    expect(tickets.getFullStatus(0, 0)).toBe(FullChunkStatus.ENTITY_TICKING);
    expect(tickets.getFullStatus(1, 0)).toBe(FullChunkStatus.TICKING);
    expect(tickets.getFullStatus(2, 0)).toBe(FullChunkStatus.BORDER);
    expect(tickets.getFullStatus(3, 0)).toBe(FullChunkStatus.INACCESSIBLE);
  });
});
