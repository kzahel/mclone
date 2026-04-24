import { describe, expect, test } from "vitest";
import {
  collectChunkDirtySectionOrigins,
  collectRequiredMeshChunks,
  createRenderWorldWorkerHandler,
  type RenderWorldWorkerContext,
} from "../../../src/renderer/chunk/render-world-worker";

function fakeContext(loadedChunkCount: number): RenderWorldWorkerContext {
  return {
    level: {
      getLoadedChunkCount: () => loadedChunkCount,
    },
  } as unknown as RenderWorldWorkerContext;
}

describe("RenderWorld worker", () => {
  test("collects mesh-neighbor chunks with the same padded section bounds as the mesh input builder", () => {
    expect(collectRequiredMeshChunks({ x: 0, y: 64, z: 0 })).toEqual([
      { chunkX: -1, chunkZ: -1 },
      { chunkX: -1, chunkZ: 0 },
      { chunkX: -1, chunkZ: 1 },
      { chunkX: 0, chunkZ: -1 },
      { chunkX: 0, chunkZ: 0 },
      { chunkX: 0, chunkZ: 1 },
      { chunkX: 1, chunkZ: -1 },
      { chunkX: 1, chunkZ: 0 },
      { chunkX: 1, chunkZ: 1 },
    ]);
  });

  test("collects vanilla-shaped dirty section origins for full-chunk ingest", () => {
    const dirty = collectChunkDirtySectionOrigins(0, 32, 2, -1);

    expect(dirty).toHaveLength(36);
    expect(dirty).toContainEqual({ x: 16, y: -16, z: -32 });
    expect(dirty).toContainEqual({ x: 32, y: 0, z: -16 });
    expect(dirty).toContainEqual({ x: 48, y: 32, z: 0 });
  });

  test("initializes once and rejects stats before initialization", async () => {
    const handler = createRenderWorldWorkerHandler(async (request) => {
      expect(request).toEqual({
        type: "initialize_render_world",
        seed: 12345n,
        minBuildHeight: 0,
        height: 256,
      });
      return fakeContext(7);
    });

    await expect(handler({ type: "get_render_world_stats" })).rejects.toThrow("get_render_world_stats received before initialize_render_world");
    await expect(handler({
      type: "initialize_render_world",
      seed: 12345n,
      minBuildHeight: 0,
      height: 256,
    })).resolves.toEqual({ type: "render_world_ready" });
    await expect(handler({ type: "get_render_world_stats" })).resolves.toEqual({
      type: "render_world_stats",
      stats: { loadedChunkCount: 7 },
    });
    await expect(handler({
      type: "initialize_render_world",
      seed: 12345n,
      minBuildHeight: 0,
      height: 256,
    })).rejects.toThrow("initialize_render_world received more than once");
  });
});
