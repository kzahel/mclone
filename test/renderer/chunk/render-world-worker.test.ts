import { describe, expect, test } from "vitest";
import {
  collectChunkDirtySectionOrigins,
  collectLightDeltaDirtySectionOrigins,
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

  test("collects dirty section origins around changed light sections", () => {
    const dirty = collectLightDeltaDirtySectionOrigins({
      type: "chunk_light_delta",
      chunkX: 2,
      chunkZ: -1,
      light: { sky: [{ y: 5 }] },
    });

    expect(dirty).toHaveLength(27);
    expect(dirty).toContainEqual({ x: 16, y: 64, z: -32 });
    expect(dirty).toContainEqual({ x: 32, y: 80, z: -16 });
    expect(dirty).toContainEqual({ x: 48, y: 96, z: 0 });
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

  test("applies light deltas and returns dirty render sections", async () => {
    const applied: unknown[] = [];
    const handler = createRenderWorldWorkerHandler(async () => ({
      ...fakeContext(1),
      level: {
        getLoadedChunkCount: () => 1,
        applyChunkLightDelta(delta: unknown): boolean {
          applied.push(delta);
          return true;
        },
      },
    } as unknown as RenderWorldWorkerContext));
    const delta = {
      type: "chunk_light_delta",
      chunkX: 0,
      chunkZ: 0,
      light: { block: [{ y: 4 }] },
    } as const;

    await handler({
      type: "initialize_render_world",
      seed: 12345n,
      minBuildHeight: 0,
      height: 256,
    });
    await expect(handler({
      type: "ingest_render_world_updates",
      messages: [delta],
    })).resolves.toMatchObject({
      type: "render_world_dirty_sections",
      dirtySections: expect.arrayContaining([
        { x: -16, y: 48, z: -16 },
        { x: 0, y: 64, z: 0 },
        { x: 16, y: 80, z: 16 },
      ]),
      stats: { loadedChunkCount: 1 },
    });
    expect(applied).toEqual([delta]);
  });
});
