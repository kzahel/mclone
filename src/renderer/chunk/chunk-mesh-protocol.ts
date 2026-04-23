import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import type { ChunkSnapshot } from "../../world/level/chunk-snapshot";
import { ClientChunkCache } from "../../world/level/client-chunk-cache";
import { Vec3 } from "../../world/phys/vec3";
import {
  deserializeDrawState,
  deserializeSortState,
  serializeDrawState,
  serializeSortState,
  type SerializedBufferBuilderDrawState,
  type SerializedBufferBuilderSortState,
} from "../vertex/buffer-builder-serialization";
import { CHUNK_RENDER_LAYERS, getChunkRenderLayerByName, getChunkRenderLayerName, type ChunkRenderLayerName } from "./chunk-render-layer";
import { deserializeVisibilitySet, serializeVisibilitySet, type SerializedVisibilitySet } from "./visibility-set-serialization";
import { type SectionMeshBuild } from "./section-mesh-compiler";
import { ChunkBufferBuilderPack } from "../chunk-buffer-builder-pack";

const SECTION_MESH_PADDING_BLOCKS = 2;
const SECTION_MESH_MAX_BLOCK_OFFSET = 17;

export interface SectionMeshInput {
  readonly origin: {
    readonly x: number;
    readonly y: number;
    readonly z: number;
  };
  readonly snapshots: readonly ChunkSnapshot[];
}

export interface InitializeMeshWorkerRequest {
  readonly type: "initialize_mesh_worker";
  readonly seed: bigint;
  readonly minBuildHeight: number;
  readonly height: number;
}

export interface BuildSectionMeshRequest extends SectionMeshInput {
  readonly type: "build_section_mesh";
  readonly camera: {
    readonly x: number;
    readonly y: number;
    readonly z: number;
  };
}

export interface SectionMeshLayerPayload {
  readonly renderType: ChunkRenderLayerName;
  readonly drawState: SerializedBufferBuilderDrawState;
  readonly buffer: Uint8Array;
}

export interface SectionMeshResult {
  readonly isCompletelyEmpty: boolean;
  readonly hasBlocks: readonly ChunkRenderLayerName[];
  readonly hasLayers: readonly ChunkRenderLayerName[];
  readonly visibilitySet: SerializedVisibilitySet;
  readonly transparencyState: SerializedBufferBuilderSortState | undefined;
  readonly layers: readonly SectionMeshLayerPayload[];
}

export interface MeshWorkerReadyResponse {
  readonly type: "mesh_worker_ready";
}

export interface SectionMeshBuiltResponse {
  readonly type: "section_mesh_built";
  readonly result: SectionMeshResult;
}

export interface MeshWorkerErrorResponse {
  readonly type: "mesh_worker_error";
  readonly message: string;
}

export type MeshWorkerRequest = InitializeMeshWorkerRequest | BuildSectionMeshRequest;
export type MeshWorkerResponse = MeshWorkerReadyResponse | SectionMeshBuiltResponse | MeshWorkerErrorResponse;

export interface DecodedSectionMeshResult {
  readonly isCompletelyEmpty: boolean;
  readonly hasBlocks: ReadonlySet<import("../render-type").RenderType>;
  readonly hasLayers: ReadonlySet<import("../render-type").RenderType>;
  readonly visibilitySet: import("./visibility-set").VisibilitySet;
  readonly transparencyState: import("../vertex/buffer-builder").BufferBuilderSortState | undefined;
  readonly layers: readonly {
    readonly renderType: import("../render-type").RenderType;
    readonly drawState: import("../vertex/buffer-builder").BufferBuilderDrawState;
    readonly buffer: Uint8Array;
  }[];
}

export function buildSectionMeshInput(level: ClientChunkCache, origin: BlockPos): SectionMeshInput {
  const minChunkX = SectionPos.blockToSectionCoord(origin.getX() - SECTION_MESH_PADDING_BLOCKS);
  const minChunkZ = SectionPos.blockToSectionCoord(origin.getZ() - SECTION_MESH_PADDING_BLOCKS);
  const maxChunkX = SectionPos.blockToSectionCoord(origin.getX() + SECTION_MESH_MAX_BLOCK_OFFSET);
  const maxChunkZ = SectionPos.blockToSectionCoord(origin.getZ() + SECTION_MESH_MAX_BLOCK_OFFSET);
  const snapshots: ChunkSnapshot[] = [];

  for (let chunkX = minChunkX; chunkX <= maxChunkX; chunkX++) {
    for (let chunkZ = minChunkZ; chunkZ <= maxChunkZ; chunkZ++) {
      const snapshot = level.getChunkSnapshot(chunkX, chunkZ);
      if (snapshot === undefined) {
        throw new Error(`Missing chunk snapshot for mesh job (${chunkX}, ${chunkZ})`);
      }

      snapshots.push(snapshot);
    }
  }

  return {
    origin: {
      x: origin.getX(),
      y: origin.getY(),
      z: origin.getZ(),
    },
    snapshots,
  };
}

export function withSectionMeshCamera(input: SectionMeshInput, camera: Vec3): BuildSectionMeshRequest {
  return {
    type: "build_section_mesh",
    origin: input.origin,
    snapshots: input.snapshots,
    camera: {
      x: camera.x,
      y: camera.y,
      z: camera.z,
    },
  };
}

export function serializeSectionMeshBuild(build: SectionMeshBuild, buffers: ChunkBufferBuilderPack): SectionMeshResult {
  const layers: SectionMeshLayerPayload[] = [];

  for (const renderType of CHUNK_RENDER_LAYERS) {
    if (!build.hasLayers.has(renderType)) {
      continue;
    }

    const { drawState, buffer } = buffers.builder(renderType).popNextBuffer();
    layers.push({
      renderType: getChunkRenderLayerName(renderType),
      drawState: serializeDrawState(drawState),
      buffer,
    });
  }

  return {
    isCompletelyEmpty: build.isCompletelyEmpty,
    hasBlocks: [...build.hasBlocks].map(getChunkRenderLayerName),
    hasLayers: [...build.hasLayers].map(getChunkRenderLayerName),
    visibilitySet: serializeVisibilitySet(build.visibilitySet),
    transparencyState: serializeSortState(build.transparencyState),
    layers,
  };
}

export function decodeSectionMeshResult(result: SectionMeshResult): DecodedSectionMeshResult {
  return {
    isCompletelyEmpty: result.isCompletelyEmpty,
    hasBlocks: new Set(result.hasBlocks.map(getChunkRenderLayerByName)),
    hasLayers: new Set(result.hasLayers.map(getChunkRenderLayerByName)),
    visibilitySet: deserializeVisibilitySet(result.visibilitySet),
    transparencyState: deserializeSortState(result.transparencyState),
    layers: result.layers.map((layer) => ({
      renderType: getChunkRenderLayerByName(layer.renderType),
      drawState: deserializeDrawState(layer.drawState),
      buffer: layer.buffer,
    })),
  };
}

export function collectSectionMeshTransferables(result: SectionMeshResult): Transferable[] {
  return result.layers.map((layer) => layer.buffer.buffer as Transferable);
}
