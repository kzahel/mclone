import { BlockPos } from "../src/core/block-pos.ts";
import { SectionPos } from "../src/core/section-pos.ts";
import type { ChunkLightDeltaMessage } from "../src/runtime/protocol/world-messages.ts";
import { connectRenderWorldWorkerSession, type RenderWorldWorkerHostEndpoint } from "../src/renderer/chunk/render-world-worker-client.ts";
import { ChunkBufferBuilderPack } from "../src/renderer/chunk-buffer-builder-pack.ts";
import { serializeSectionMeshBuild } from "../src/renderer/chunk/chunk-mesh-protocol.ts";
import { RenderChunkRegion } from "../src/renderer/chunk/render-chunk-region.ts";
import { buildSectionMesh } from "../src/renderer/chunk/section-mesh-compiler.ts";
import {
  type BuildRenderSectionMeshRequest,
  type InitializeRenderWorldRequest,
  type RenderWorldChunkKey,
  type RenderWorldRequest,
  type RenderWorldResponse,
  type RenderWorldSectionOrigin,
  type RenderWorldUpdateMessage,
} from "../src/renderer/chunk/render-world-protocol.ts";
import { createBlockStateResolver } from "../src/world/level/chunk-snapshot.ts";
import { ClientChunkCache } from "../src/world/level/client-chunk-cache.ts";
import { registerGeneratedRenderBlocks } from "../src/world/level/generated-render-blocks.ts";
import { Vec3 } from "../src/world/phys/vec3.ts";
import { OverworldBiomeSource } from "../src/worldgen/biome/overworld-biome-source.ts";
import {
  createDenoVanillaAssetBlockRenderer,
  createSpriteLookup,
  prepareDenoVanillaAssetAtlasResources,
} from "./deno-vanilla-asset-world-smoke-shared.ts";
import { TextureAtlas } from "../src/renderer/texture/texture-atlas.ts";

const SECTION_MESH_PADDING_BLOCKS = 2;
const SECTION_MESH_MAX_BLOCK_OFFSET = 17;

interface DenoVanillaAssetRenderWorldContext {
  readonly level: ClientChunkCache;
  readonly blockRenderer: Awaited<ReturnType<typeof createDenoVanillaAssetBlockRenderer>>;
}

let context: DenoVanillaAssetRenderWorldContext | undefined;

connectRenderWorldWorkerSession(globalThis as unknown as RenderWorldWorkerHostEndpoint, handleRenderWorldRequest);

async function handleRenderWorldRequest(message: RenderWorldRequest): Promise<RenderWorldResponse> {
  switch (message.type) {
    case "initialize_render_world":
      if (context !== undefined) {
        throw new Error("initialize_render_world received more than once");
      }

      context = await createContext(message);
      return { type: "render_world_ready" };
    case "ingest_render_world_updates":
      return ingestUpdates(getContext(message.type), message.messages);
    case "build_render_section_mesh":
      return buildSectionMeshResponse(getContext(message.type), message);
    case "get_render_world_stats":
      return {
        type: "render_world_stats",
        stats: { loadedChunkCount: getContext(message.type).level.getLoadedChunkCount() },
      };
  }
}

function getContext(messageType: string): DenoVanillaAssetRenderWorldContext {
  if (context === undefined) {
    throw new Error(`${messageType} received before initialize_render_world`);
  }

  return context;
}

async function createContext(request: InitializeRenderWorldRequest): Promise<DenoVanillaAssetRenderWorldContext> {
  const blocks = registerGeneratedRenderBlocks();
  const biomeSource = new OverworldBiomeSource(request.seed);
  const level = new ClientChunkCache({
    airState: blocks.airState,
    minBuildHeight: request.minBuildHeight,
    height: request.height,
    biomeSource,
    biomeZoomSeed: request.seed,
    blockStateResolver: createBlockStateResolver(blocks.airState),
    blockStateIds: blocks.blockStateIds,
    skyLight: 15,
    blockLight: 15,
  });
  const atlas = new TextureAtlas(TextureAtlas.LOCATION_BLOCKS);
  const assetResources = await prepareDenoVanillaAssetAtlasResources(atlas);
  const spriteLookup = createSpriteLookup(assetResources.preparations.regions);
  assetResources.atlasSource.close();
  return {
    level,
    blockRenderer: await createDenoVanillaAssetBlockRenderer(assetResources.assetPack, blocks, spriteLookup),
  };
}

function ingestUpdates(
  context: DenoVanillaAssetRenderWorldContext,
  messages: readonly RenderWorldUpdateMessage[],
): RenderWorldResponse {
  const dirtySections = new Map<string, RenderWorldSectionOrigin>();
  for (const update of messages) {
    if (update.type === "chunk_snapshot") {
      context.level.applyPackedChunkSnapshot(update.snapshot);
      for (const dirtySection of collectChunkDirtySectionOrigins(
        context.level.getMinBuildHeight(),
        context.level.getHeight(),
        update.snapshot.chunkX,
        update.snapshot.chunkZ,
      )) {
        dirtySections.set(sectionKey(dirtySection), dirtySection);
      }
      continue;
    }

    if (update.type === "chunk_light_delta") {
      if (context.level.applyChunkLightDelta(update)) {
        for (const dirtySection of collectLightDeltaDirtySectionOrigins(update)) {
          dirtySections.set(sectionKey(dirtySection), dirtySection);
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
      dirtySections.set(sectionKey(dirtySection), dirtySection);
    }
  }

  return {
    type: "render_world_dirty_sections",
    dirtySections: [...dirtySections.values()],
    stats: { loadedChunkCount: context.level.getLoadedChunkCount() },
  };
}

async function buildSectionMeshResponse(
  context: DenoVanillaAssetRenderWorldContext,
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

function collectChunkDirtySectionOrigins(
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
          const origin = sectionOrigin(dirtyX, dirtyY, dirtyZ);
          dirty.set(sectionKey(origin), origin);
        }
      }
    }
  }

  return [...dirty.values()];
}

function collectLightDeltaDirtySectionOrigins(delta: ChunkLightDeltaMessage): readonly RenderWorldSectionOrigin[] {
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
          const origin = sectionOrigin(dirtyX, dirtyY, dirtyZ);
          dirty.set(sectionKey(origin), origin);
        }
      }
    }
  }

  return [...dirty.values()];
}

function collectRequiredMeshChunks(origin: RenderWorldSectionOrigin): readonly RenderWorldChunkKey[] {
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

function sectionOrigin(sectionX: number, sectionY: number, sectionZ: number): RenderWorldSectionOrigin {
  return {
    x: SectionPos.sectionToBlockCoord(sectionX),
    y: SectionPos.sectionToBlockCoord(sectionY),
    z: SectionPos.sectionToBlockCoord(sectionZ),
  };
}

function sectionKey(origin: RenderWorldSectionOrigin): string {
  return `${origin.x},${origin.y},${origin.z}`;
}
