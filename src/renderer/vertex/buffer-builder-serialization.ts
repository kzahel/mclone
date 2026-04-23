import { Vector3f } from "../math/vector3f";
import { BufferBuilderDrawState, BufferBuilderSortState } from "./buffer-builder";
import { DefaultVertexFormat } from "./default-vertex-format";
import { VertexFormat, VertexFormatIndexType, VertexFormatMode } from "./vertex-format";

export type SerializedVertexFormatName =
  | "blit_screen"
  | "block"
  | "new_entity"
  | "particle"
  | "position"
  | "position_color"
  | "position_color_normal"
  | "position_color_lightmap"
  | "position_tex"
  | "position_color_tex"
  | "position_tex_color"
  | "position_color_tex_lightmap"
  | "position_tex_lightmap_color"
  | "position_tex_color_normal";

export type SerializedVertexMode =
  | "lines"
  | "line_strip"
  | "debug_lines"
  | "debug_line_strip"
  | "triangles"
  | "triangle_strip"
  | "triangle_fan"
  | "quads";

export type SerializedIndexType = "byte" | "short" | "int";
export type SerializedSortPoint = readonly [number, number, number];

export interface SerializedBufferBuilderDrawState {
  readonly format: SerializedVertexFormatName;
  readonly vertexCount: number;
  readonly indexCount: number;
  readonly mode: SerializedVertexMode;
  readonly indexType: SerializedIndexType;
  readonly indexOnly: boolean;
  readonly sequentialIndex: boolean;
}

export interface SerializedBufferBuilderSortState {
  readonly mode: SerializedVertexMode;
  readonly vertices: number;
  readonly sortingPoints: readonly SerializedSortPoint[] | null;
  readonly sortX: number;
  readonly sortY: number;
  readonly sortZ: number;
}

const DEFAULT_VERTEX_FORMAT_BY_NAME = new Map<SerializedVertexFormatName, VertexFormat>([
  ["blit_screen", DefaultVertexFormat.BLIT_SCREEN],
  ["block", DefaultVertexFormat.BLOCK],
  ["new_entity", DefaultVertexFormat.NEW_ENTITY],
  ["particle", DefaultVertexFormat.PARTICLE],
  ["position", DefaultVertexFormat.POSITION],
  ["position_color", DefaultVertexFormat.POSITION_COLOR],
  ["position_color_normal", DefaultVertexFormat.POSITION_COLOR_NORMAL],
  ["position_color_lightmap", DefaultVertexFormat.POSITION_COLOR_LIGHTMAP],
  ["position_tex", DefaultVertexFormat.POSITION_TEX],
  ["position_color_tex", DefaultVertexFormat.POSITION_COLOR_TEX],
  ["position_tex_color", DefaultVertexFormat.POSITION_TEX_COLOR],
  ["position_color_tex_lightmap", DefaultVertexFormat.POSITION_COLOR_TEX_LIGHTMAP],
  ["position_tex_lightmap_color", DefaultVertexFormat.POSITION_TEX_LIGHTMAP_COLOR],
  ["position_tex_color_normal", DefaultVertexFormat.POSITION_TEX_COLOR_NORMAL],
]);

const DEFAULT_VERTEX_FORMAT_NAMES = [...DEFAULT_VERTEX_FORMAT_BY_NAME.entries()];

const VERTEX_MODE_BY_NAME = new Map<SerializedVertexMode, VertexFormatMode>([
  ["lines", VertexFormat.Mode.LINES],
  ["line_strip", VertexFormat.Mode.LINE_STRIP],
  ["debug_lines", VertexFormat.Mode.DEBUG_LINES],
  ["debug_line_strip", VertexFormat.Mode.DEBUG_LINE_STRIP],
  ["triangles", VertexFormat.Mode.TRIANGLES],
  ["triangle_strip", VertexFormat.Mode.TRIANGLE_STRIP],
  ["triangle_fan", VertexFormat.Mode.TRIANGLE_FAN],
  ["quads", VertexFormat.Mode.QUADS],
]);

const VERTEX_MODE_NAMES = [...VERTEX_MODE_BY_NAME.entries()];

const INDEX_TYPE_BY_NAME = new Map<SerializedIndexType, VertexFormatIndexType>([
  ["byte", VertexFormat.IndexType.BYTE],
  ["short", VertexFormat.IndexType.SHORT],
  ["int", VertexFormat.IndexType.INT],
]);

const INDEX_TYPE_NAMES = [...INDEX_TYPE_BY_NAME.entries()];

function serializeVertexFormat(format: VertexFormat): SerializedVertexFormatName {
  for (const [name, candidate] of DEFAULT_VERTEX_FORMAT_NAMES) {
    if (candidate === format) {
      return name;
    }
  }

  throw new Error(`Unsupported vertex format for serialization: ${format}`);
}

function deserializeVertexFormat(name: SerializedVertexFormatName): VertexFormat {
  const format = DEFAULT_VERTEX_FORMAT_BY_NAME.get(name);
  if (format === undefined) {
    throw new Error(`Unknown serialized vertex format ${name}`);
  }

  return format;
}

function serializeVertexMode(mode: VertexFormatMode): SerializedVertexMode {
  for (const [name, candidate] of VERTEX_MODE_NAMES) {
    if (candidate === mode) {
      return name;
    }
  }

  throw new Error("Unsupported vertex mode for serialization");
}

function deserializeVertexMode(name: SerializedVertexMode): VertexFormatMode {
  const mode = VERTEX_MODE_BY_NAME.get(name);
  if (mode === undefined) {
    throw new Error(`Unknown serialized vertex mode ${name}`);
  }

  return mode;
}

function serializeIndexType(indexType: VertexFormatIndexType): SerializedIndexType {
  for (const [name, candidate] of INDEX_TYPE_NAMES) {
    if (candidate === indexType) {
      return name;
    }
  }

  throw new Error("Unsupported vertex index type for serialization");
}

function deserializeIndexType(name: SerializedIndexType): VertexFormatIndexType {
  const indexType = INDEX_TYPE_BY_NAME.get(name);
  if (indexType === undefined) {
    throw new Error(`Unknown serialized vertex index type ${name}`);
  }

  return indexType;
}

export function serializeDrawState(drawState: BufferBuilderDrawState): SerializedBufferBuilderDrawState {
  return {
    format: serializeVertexFormat(drawState.format()),
    vertexCount: drawState.vertexCount(),
    indexCount: drawState.indexCount(),
    mode: serializeVertexMode(drawState.mode()),
    indexType: serializeIndexType(drawState.indexType()),
    indexOnly: drawState.indexOnly(),
    sequentialIndex: drawState.sequentialIndex(),
  };
}

export function deserializeDrawState(drawState: SerializedBufferBuilderDrawState): BufferBuilderDrawState {
  return new BufferBuilderDrawState(
    deserializeVertexFormat(drawState.format),
    drawState.vertexCount,
    drawState.indexCount,
    deserializeVertexMode(drawState.mode),
    deserializeIndexType(drawState.indexType),
    drawState.indexOnly,
    drawState.sequentialIndex,
  );
}

export function serializeSortState(state: BufferBuilderSortState | undefined): SerializedBufferBuilderSortState | undefined {
  if (state === undefined) {
    return undefined;
  }

  return {
    mode: serializeVertexMode(state.mode),
    vertices: state.vertices,
    sortingPoints: state.sortingPoints?.map((point) => [point.x(), point.y(), point.z()] as const) ?? null,
    sortX: state.sortX,
    sortY: state.sortY,
    sortZ: state.sortZ,
  };
}

export function deserializeSortState(state: SerializedBufferBuilderSortState | undefined): BufferBuilderSortState | undefined {
  if (state === undefined) {
    return undefined;
  }

  return new BufferBuilderSortState(
    deserializeVertexMode(state.mode),
    state.vertices,
    state.sortingPoints?.map(([x, y, z]) => new Vector3f(x, y, z)) ?? null,
    state.sortX,
    state.sortY,
    state.sortZ,
  );
}
