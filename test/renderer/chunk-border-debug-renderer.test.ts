import { describe, expect, it } from "vitest";
import { collectChunkBorderDebugCorners, type ChunkBorderDebugRecord } from "../../src/renderer/debug/chunk-border-debug-renderer";

describe("chunk border debug renderer", () => {
  it("collects unique corners from published and publish-view chunks only", () => {
    const records: ChunkBorderDebugRecord[] = [
      { chunkX: 0, chunkZ: 0, inPublishView: true, published: false },
      { chunkX: 1, chunkZ: 0, inPublishView: false, published: true },
      { chunkX: 10, chunkZ: 10, inPublishView: false, published: false },
    ];

    const corners = collectChunkBorderDebugCorners({ records });

    expect(corners).toEqual([
      { x: 0, z: 0, inPublishView: true, published: false },
      { x: 0, z: 16, inPublishView: true, published: false },
      { x: 16, z: 0, inPublishView: true, published: true },
      { x: 16, z: 16, inPublishView: true, published: true },
      { x: 32, z: 0, inPublishView: false, published: true },
      { x: 32, z: 16, inPublishView: false, published: true },
    ]);
  });
});
