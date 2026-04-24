import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import type { ChunkLightDeltaMessage } from "../../runtime/protocol/world-messages";
import { ClientChunkCache } from "../../world/level/client-chunk-cache";
import { Vec3 } from "../../world/phys/vec3";
import type { LoadingProgressSink } from "../loading-progress";
import { ChunkBufferBuilderPack } from "../chunk-buffer-builder-pack";
import { serializeSectionMeshBuild } from "./chunk-mesh-protocol";
import { createBrowserChunkMeshContext, type BrowserChunkMeshContext } from "./mesh-worker-context";
import { connectRenderWorldWorkerSession, type RenderWorldWorkerHostEndpoint } from "./render-world-worker-client";
import {
  type BuildRenderSectionMeshRequest,
  type InitializeRenderWorldRequest,
  type RenderWorldChunkKey,
  type RenderWorldRequest,
  type RenderWorldResponse,
  type RenderWorldSectionOrigin,
} from "./render-world-protocol";
import { RenderChunkRegion } from "./render-chunk-region";
import { buildSectionMesh } from "./section-mesh-compiler";

const SECTION_MESH_PADDING_BLOCKS = 2;
const SECTION_MESH_MAX_BLOCK_OFFSET = 17;

export interface RenderWorldWorkerContext extends BrowserChunkMeshContext {
  readonly level: ClientChunkCache;
}

export type RenderWorldContextFactory = (
  request: InitializeRenderWorldRequest,
  onProgress?: LoadingProgressSink,
) => Promise<RenderWorldWorkerContext>;

function sectionOrigin(sectionX: number, sectionY: number, sectionZ: number): RenderWorldSectionOrigin {
  return {
    x: SectionPos.sectionToBlockCoord(sectionX),
    y: SectionPos.sectionToBlockCoord(sectionY),
    z: SectionPos.sectionToBlockCoord(sectionZ),
  };
}

export function collectChunkDirtySectionOrigins(
  minBuildHeight: number,
  height: number,
  chunkX: number,
  chunkZ: number,
): readonly RenderWorldSectionOrigin[] {
  const minSectionY = SectionPos.blockToSectionCoord(minBuildHeight);
  const sectionCount = Math.trunc(height / 16);
  const dirty = new Map<string, RenderWorldSectionOrigin>();

  for (let sectionY = minSectionY; sectionY < minSectionY + sectionCount; sectionY++) {
    for (let dirtyZ = chunkZ - 1; dirtyZ <= chunkZ + 1; dirtyZ++) {
      for (let dirtyX = chunkX - 1; dirtyX <= chunkX + 1; dirtyX++) {
        for (let dirtyY = sectionY - 1; dirtyY <= sectionY + 1; dirtyY++) {
          const key = `${dirtyX},${dirtyY},${dirtyZ}`;
          dirty.set(key, sectionOrigin(dirtyX, dirtyY, dirtyZ));
        }
      }
    }
  }

  return [...dirty.values()];
}

export function collectLightDeltaDirtySectionOrigins(delta: ChunkLightDeltaMessage): readonly RenderWorldSectionOrigin[] {
  const dirty = new Map<string, RenderWorldSectionOrigin>();
  const changedSectionY = new Set<number>();
  for (const section of delta.light.sky ?? []) {
    changedSectionY.add(section.y);
  }
  for (const section of delta.light.block ?? []) {
    changedSectionY.add(section.y);
  }

  for (const sectionY of changedSectionY) {
    for (let dirtyZ = delta.chunkZ - 1; dirtyZ <= delta.chunkZ + 1; dirtyZ++) {
      for (let dirtyX = delta.chunkX - 1; dirtyX <= delta.chunkX + 1; dirtyX++) {
        for (let dirtyY = sectionY - 1; dirtyY <= sectionY + 1; dirtyY++) {
          const key = `${dirtyX},${dirtyY},${dirtyZ}`;
          dirty.set(key, sectionOrigin(dirtyX, dirtyY, dirtyZ));
        }
      }
    }
  }

  return [...dirty.values()];
}

export function collectRequiredMeshChunks(origin: RenderWorldSectionOrigin): readonly RenderWorldChunkKey[] {
  const minChunkX = SectionPos.blockToSectionCoord(origin.x - SECTION_MESH_PADDING_BLOCKS);
  const minChunkZ = SectionPos.blockToSectionCoord(origin.z - SECTION_MESH_PADDING_BLOCKS);
  const maxChunkX = SectionPos.blockToSectionCoord(origin.x + SECTION_MESH_MAX_BLOCK_OFFSET);
  const maxChunkZ = SectionPos.blockToSectionCoord(origin.z + SECTION_MESH_MAX_BLOCK_OFFSET);
  const chunks: RenderWorldChunkKey[] = [];

  for (let chunkX = minChunkX; chunkX <= maxChunkX; chunkX++) {
    for (let chunkZ = minChunkZ; chunkZ <= maxChunkZ; chunkZ++) {
      chunks.push({ chunkX, chunkZ });
    }
  }

  return chunks;
}

function missingMeshChunks(level: ClientChunkCache, origin: RenderWorldSectionOrigin): readonly RenderWorldChunkKey[] {
  const missing: RenderWorldChunkKey[] = [];
  for (const chunk of collectRequiredMeshChunks(origin)) {
    if (level.getChunk(chunk.chunkX, chunk.chunkZ, false) === null) {
      missing.push(chunk);
    }
  }

  return missing;
}

async function createRenderWorldWorkerContext(
  request: InitializeRenderWorldRequest,
  onProgress?: LoadingProgressSink,
): Promise<RenderWorldWorkerContext> {
  const meshContext = await createBrowserChunkMeshContext(request, onProgress);
  const level = new ClientChunkCache({
    airState: meshContext.airState,
    minBuildHeight: meshContext.minBuildHeight,
    height: meshContext.height,
    biomeSource: meshContext.biomeSource,
    biomeZoomSeed: meshContext.seed,
    blockStateResolver: meshContext.blockStateResolver,
    blockStateIds: meshContext.blockStateIds,
  });

  return {
    ...meshContext,
    level,
  };
}

function stats(context: RenderWorldWorkerContext): { readonly loadedChunkCount: number } {
  return {
    loadedChunkCount: context.level.getLoadedChunkCount(),
  };
}

async function buildSectionMeshResponse(
  context: RenderWorldWorkerContext,
  request: BuildRenderSectionMeshRequest,
): Promise<RenderWorldResponse> {
  if (request.origin.y < context.level.getMinBuildHeight() || request.origin.y >= context.level.getMaxBuildHeight()) {
    return {
      type: "render_world_mesh_not_ready",
      origin: request.origin,
      reason: "unloaded_section",
    };
  }

  const missingChunks = missingMeshChunks(context.level, request.origin);
  if (missingChunks.length > 0) {
    return {
      type: "render_world_mesh_not_ready",
      origin: request.origin,
      reason: "missing_neighbors",
      missingChunks,
    };
  }

  const origin = new BlockPos(request.origin.x, request.origin.y, request.origin.z);
  const region = RenderChunkRegion.createIfNotEmpty(context.level, origin.offset(-1, -1, -1), origin.offset(16, 16, 16), 1);
  const buffers = new ChunkBufferBuilderPack();

  try {
    const build = buildSectionMesh(
      origin,
      new Vec3(request.camera.x, request.camera.y, request.camera.z),
      region,
      context.blockRenderer,
      buffers,
    );

    return {
      type: "render_section_mesh_built",
      origin: request.origin,
      result: serializeSectionMeshBuild(build, buffers),
    };
  } finally {
    buffers.discardAll();
  }
}

export function createRenderWorldWorkerHandler(
  contextFactory: RenderWorldContextFactory = createRenderWorldWorkerContext,
): (message: RenderWorldRequest, onProgress?: LoadingProgressSink) => Promise<RenderWorldResponse> {
  let contextPromise: Promise<RenderWorldWorkerContext> | undefined;

  async function getContext(messageType: string): Promise<RenderWorldWorkerContext> {
    if (contextPromise === undefined) {
      throw new Error(`${messageType} received before initialize_render_world`);
    }

    return contextPromise;
  }

  return async (message, onProgress) => {
    switch (message.type) {
      case "initialize_render_world":
        if (contextPromise !== undefined) {
          throw new Error("initialize_render_world received more than once");
        }

        contextPromise = contextFactory(message, onProgress);
        await contextPromise;
        return { type: "render_world_ready" };
      case "ingest_render_world_updates": {
        const context = await getContext(message.type);
        const dirtySections = new Map<string, RenderWorldSectionOrigin>();
        for (const update of message.messages) {
          if (update.type === "chunk_snapshot") {
            context.level.applyPackedChunkSnapshot(update.snapshot);
            for (const dirtySection of collectChunkDirtySectionOrigins(
              context.level.getMinBuildHeight(),
              context.level.getHeight(),
              update.snapshot.chunkX,
              update.snapshot.chunkZ,
            )) {
              dirtySections.set(`${dirtySection.x},${dirtySection.y},${dirtySection.z}`, dirtySection);
            }
            continue;
          }

          if (update.type === "chunk_light_delta") {
            if (context.level.applyChunkLightDelta(update)) {
              for (const dirtySection of collectLightDeltaDirtySectionOrigins(update)) {
                dirtySections.set(`${dirtySection.x},${dirtySection.y},${dirtySection.z}`, dirtySection);
              }
            }
            continue;
          }

          context.level.applyChunkUnload(update.chunkX, update.chunkZ);
          for (const dirtySection of collectChunkDirtySectionOrigins(
            context.level.getMinBuildHeight(),
            context.level.getHeight(),
            update.chunkX,
            update.chunkZ,
          )) {
            dirtySections.set(`${dirtySection.x},${dirtySection.y},${dirtySection.z}`, dirtySection);
          }
        }

        return {
          type: "render_world_dirty_sections",
          dirtySections: [...dirtySections.values()],
          stats: stats(context),
        };
      }
      case "build_render_section_mesh":
        return buildSectionMeshResponse(await getContext(message.type), message);
      case "get_render_world_stats": {
        const context = await getContext(message.type);
        return {
          type: "render_world_stats",
          stats: stats(context),
        };
      }
    }
  };
}

const maybeWorkerGlobal = globalThis as Partial<RenderWorldWorkerHostEndpoint>;
if (typeof maybeWorkerGlobal.postMessage === "function" && typeof maybeWorkerGlobal.addEventListener === "function") {
  connectRenderWorldWorkerSession(
    maybeWorkerGlobal as RenderWorldWorkerHostEndpoint,
    createRenderWorldWorkerHandler(),
  );
}
