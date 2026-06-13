import type { GeneratedChunkLifecycleRecord, GeneratedChunkLifecycleSnapshot } from "../../runtime/protocol/chunk-lifecycle";
import type { LevelRenderFrame } from "../level-renderer";
import { BufferBuilderDrawState } from "../vertex/buffer-builder";
import { DefaultVertexFormat } from "../vertex/default-vertex-format";
import { VertexBuffer } from "../vertex/vertex-buffer";
import { VertexFormat } from "../vertex/vertex-format";

const CHUNK_SIZE = 16;
const VERTEX_STRIDE = DefaultVertexFormat.POSITION_COLOR_NORMAL.getVertexSize();
const CAP_HALF_SIZE = 0.75;
const LITTLE_ENDIAN = true;

const PUBLISHED_COLOR = [64, 192, 255, 255] as const;
const PUBLISH_VIEW_COLOR = [255, 214, 64, 255] as const;

export interface ChunkBorderDebugWorldBounds {
  readonly minBuildHeight: number;
  readonly maxBuildHeight: number;
}

export type ChunkBorderDebugRecord = Pick<GeneratedChunkLifecycleRecord, "chunkX" | "chunkZ" | "inPublishView" | "published">;

export interface ChunkBorderDebugCorner {
  readonly x: number;
  readonly z: number;
  readonly published: boolean;
  readonly inPublishView: boolean;
}

export interface ChunkBorderDebugVertexBuffer {
  readonly vertexBuffer: VertexBuffer;
  readonly cornerCount: number;
}

export function collectChunkBorderDebugCorners(snapshot: {
  readonly records: readonly ChunkBorderDebugRecord[];
}): readonly ChunkBorderDebugCorner[] {
  const corners = new Map<string, ChunkBorderDebugCorner>();
  for (const record of snapshot.records) {
    if (!record.published && !record.inPublishView) {
      continue;
    }

    const minX = record.chunkX * CHUNK_SIZE;
    const minZ = record.chunkZ * CHUNK_SIZE;
    addCorner(corners, minX, minZ, record);
    addCorner(corners, minX + CHUNK_SIZE, minZ, record);
    addCorner(corners, minX, minZ + CHUNK_SIZE, record);
    addCorner(corners, minX + CHUNK_SIZE, minZ + CHUNK_SIZE, record);
  }

  return [...corners.values()].sort((left, right) => left.x - right.x || left.z - right.z);
}

export function createChunkBorderDebugVertexBuffer(
  device: GPUDevice,
  frame: LevelRenderFrame,
  snapshot: GeneratedChunkLifecycleSnapshot | undefined,
  worldBounds: ChunkBorderDebugWorldBounds,
): ChunkBorderDebugVertexBuffer | undefined {
  // WebGPU: transient GPU vertex buffer instead of Tesselator immediate-mode debug lines.
  if (snapshot === undefined) {
    return undefined;
  }

  const corners = collectChunkBorderDebugCorners(snapshot);
  if (corners.length === 0) {
    return undefined;
  }

  const lineCount = corners.length * 3;
  const vertexCount = lineCount * 2;
  const buffer = new Uint8Array(vertexCount * VERTEX_STRIDE);
  const dataView = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
  let vertexIndex = 0;
  const minY = worldBounds.minBuildHeight;
  const maxY = worldBounds.maxBuildHeight;

  for (const corner of corners) {
    const color = corner.published ? PUBLISHED_COLOR : PUBLISH_VIEW_COLOR;
    vertexIndex = writeLine(dataView, vertexIndex, frame, corner.x, minY, corner.z, corner.x, maxY, corner.z, color);
    vertexIndex = writeLine(
      dataView,
      vertexIndex,
      frame,
      corner.x - CAP_HALF_SIZE,
      maxY,
      corner.z,
      corner.x + CAP_HALF_SIZE,
      maxY,
      corner.z,
      color,
    );
    vertexIndex = writeLine(
      dataView,
      vertexIndex,
      frame,
      corner.x,
      maxY,
      corner.z - CAP_HALF_SIZE,
      corner.x,
      maxY,
      corner.z + CAP_HALF_SIZE,
      color,
    );
  }

  const vertexBuffer = new VertexBuffer(device);
  vertexBuffer.uploadRaw(
    new BufferBuilderDrawState(
      DefaultVertexFormat.POSITION_COLOR_NORMAL,
      vertexCount,
      vertexCount,
      VertexFormat.Mode.DEBUG_LINES,
      VertexFormat.IndexType.SHORT,
      false,
      true,
    ),
    buffer,
  );
  return {
    vertexBuffer,
    cornerCount: corners.length,
  };
}

function addCorner(
  corners: Map<string, ChunkBorderDebugCorner>,
  x: number,
  z: number,
  record: ChunkBorderDebugRecord,
): void {
  const key = `${x.toString()},${z.toString()}`;
  const existing = corners.get(key);
  corners.set(key, {
    x,
    z,
    published: (existing?.published ?? false) || record.published,
    inPublishView: (existing?.inPublishView ?? false) || record.inPublishView,
  });
}

function writeLine(
  dataView: DataView,
  vertexIndex: number,
  frame: LevelRenderFrame,
  x0: number,
  y0: number,
  z0: number,
  x1: number,
  y1: number,
  z1: number,
  color: readonly [number, number, number, number],
): number {
  const length = Math.hypot(x1 - x0, y1 - y0, z1 - z0);
  const normalX = length === 0 ? 0 : (x1 - x0) / length;
  const normalY = length === 0 ? 1 : (y1 - y0) / length;
  const normalZ = length === 0 ? 0 : (z1 - z0) / length;
  let nextVertexIndex = writeVertex(dataView, vertexIndex, frame, x0, y0, z0, color, normalX, normalY, normalZ);
  nextVertexIndex = writeVertex(dataView, nextVertexIndex, frame, x1, y1, z1, color, normalX, normalY, normalZ);
  return nextVertexIndex;
}

function writeVertex(
  dataView: DataView,
  vertexIndex: number,
  frame: LevelRenderFrame,
  worldX: number,
  worldY: number,
  worldZ: number,
  color: readonly [number, number, number, number],
  normalX: number,
  normalY: number,
  normalZ: number,
): number {
  const offset = vertexIndex * VERTEX_STRIDE;
  dataView.setFloat32(offset + 0, worldX - frame.cameraPosition[0], LITTLE_ENDIAN);
  dataView.setFloat32(offset + 4, worldY - frame.cameraPosition[1], LITTLE_ENDIAN);
  dataView.setFloat32(offset + 8, worldZ - frame.cameraPosition[2], LITTLE_ENDIAN);
  dataView.setUint8(offset + 12, color[0]);
  dataView.setUint8(offset + 13, color[1]);
  dataView.setUint8(offset + 14, color[2]);
  dataView.setUint8(offset + 15, color[3]);
  dataView.setInt8(offset + 16, normalIntValue(normalX));
  dataView.setInt8(offset + 17, normalIntValue(normalY));
  dataView.setInt8(offset + 18, normalIntValue(normalZ));
  dataView.setInt8(offset + 19, 0);
  return vertexIndex + 1;
}

function normalIntValue(value: number): number {
  return Math.trunc(Math.max(-1, Math.min(1, value)) * 127);
}
