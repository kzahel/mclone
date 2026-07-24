export const TERRAIN_LAB_SPACINGS = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024] as const;
export const TERRAIN_LAB_CELLS_PER_AXIS = 64;
export const TERRAIN_LAB_MIN_BLOCKS_ACROSS = 64;
export const TERRAIN_LAB_MAX_BLOCKS_ACROSS = 131_072;

export type TerrainLabSpacing = (typeof TERRAIN_LAB_SPACINGS)[number];
export type TerrainLabDetail = "auto" | TerrainLabSpacing;
export type TerrainLabSource = "gpu" | "reference" | "split";
export type TerrainLabView = "map" | "3d";
export type TerrainLabLayer = "terrain" | "height" | "error" | "continentalness" | "climate";

export interface TerrainLabState {
  seed: string;
  centerX: number;
  centerZ: number;
  blocksAcross: number;
  detail: TerrainLabDetail;
  source: TerrainLabSource;
  view: TerrainLabView;
  layer: TerrainLabLayer;
}

export interface TerrainLabCamera {
  yaw: number;
  pitch: number;
}

export const DEFAULT_TERRAIN_LAB_STATE: TerrainLabState = {
  seed: "-98765",
  centerX: -304,
  centerZ: 336,
  blocksAcross: 2_048,
  detail: "auto",
  source: "reference",
  view: "3d",
  layer: "terrain",
};

export const REVIEW_TERRAIN_LAB_STATE: TerrainLabState = {
  ...DEFAULT_TERRAIN_LAB_STATE,
  source: "split",
};

export const DEFAULT_TERRAIN_LAB_CAMERA: TerrainLabCamera = {
  yaw: Math.PI / 4,
  pitch: 0.48,
};

const I64_MIN = -(1n << 63n);
const I64_MAX = (1n << 63n) - 1n;
const I32_MIN = -2_147_483_648;
const I32_MAX = 2_147_483_647;
const SOURCES = new Set<TerrainLabSource>(["gpu", "reference", "split"]);
const VIEWS = new Set<TerrainLabView>(["map", "3d"]);
const LAYERS = new Set<TerrainLabLayer>([
  "terrain",
  "height",
  "error",
  "continentalness",
  "climate",
]);

export function parseTerrainLabState(
  search: string,
  fallback: TerrainLabState = DEFAULT_TERRAIN_LAB_STATE,
): TerrainLabState {
  const params = new URLSearchParams(search);
  const legacySpacing = validSpacing(params.get("spacing"));
  return {
    seed: validSeed(params.get("seed")) ?? fallback.seed,
    centerX: validI32(params.get("x")) ?? fallback.centerX,
    centerZ: validI32(params.get("z")) ?? fallback.centerZ,
    blocksAcross:
      validBlocksAcross(params.get("blocks"))
      ?? (legacySpacing === undefined ? undefined : legacySpacing * TERRAIN_LAB_CELLS_PER_AXIS)
      ?? fallback.blocksAcross,
    detail: validDetail(params.get("detail")) ?? legacySpacing ?? fallback.detail,
    source: validMember(params.get("source"), SOURCES) ?? fallback.source,
    view: validMember(params.get("view"), VIEWS) ?? fallback.view,
    layer: validMember(params.get("layer"), LAYERS) ?? fallback.layer,
  };
}

export function terrainLabSearch(state: TerrainLabState): string {
  const params = new URLSearchParams();
  params.set("seed", state.seed);
  params.set("x", String(state.centerX));
  params.set("z", String(state.centerZ));
  params.set("blocks", String(state.blocksAcross));
  params.set("detail", String(state.detail));
  params.set("source", state.source);
  params.set("view", state.view);
  params.set("layer", state.layer);
  return `?${params.toString()}`;
}

export function footprintBlocks(state: Pick<TerrainLabState, "blocksAcross">): number {
  return state.blocksAcross;
}

export function nextBlocksAcross(
  blocksAcross: number,
  direction: "in" | "out",
): number {
  const factor = direction === "in" ? 0.5 : 2;
  return clampBlocksAcross(blocksAcross * factor);
}

export function zoomTerrainLabState(
  state: TerrainLabState,
  factor: number,
  anchorX = 0,
  anchorZ = 0,
  panelAspect = 1,
): TerrainLabState {
  if (!Number.isFinite(factor) || factor <= 0) {
    return state;
  }
  const blocksAcross = clampBlocksAcross(state.blocksAcross * factor);
  const oldHeight = state.blocksAcross / Math.max(panelAspect, 0.01);
  const newHeight = blocksAcross / Math.max(panelAspect, 0.01);
  return {
    ...state,
    centerX: clampI32(Math.round(state.centerX + anchorX * (state.blocksAcross - blocksAcross))),
    centerZ: clampI32(Math.round(state.centerZ + anchorZ * (oldHeight - newHeight))),
    blocksAcross,
  };
}

export function panTerrainLabState(
  state: TerrainLabState,
  deltaX: number,
  deltaZ: number,
): TerrainLabState {
  return {
    ...state,
    centerX: clampI32(Math.round(state.centerX + deltaX)),
    centerZ: clampI32(Math.round(state.centerZ + deltaZ)),
  };
}

export function grabPanTerrainLabState(
  state: TerrainLabState,
  pointerDeltaX: number,
  pointerDeltaY: number,
  panelWidth: number,
  panelHeight: number,
  panelAspect: number,
): TerrainLabState {
  return panTerrainLabState(
    state,
    -(pointerDeltaX / Math.max(panelWidth, 1)) * state.blocksAcross,
    -(pointerDeltaY / Math.max(panelHeight, 1))
      * (state.blocksAcross / Math.max(panelAspect, 0.01)),
  );
}

export function orbitTerrainLabCamera(
  camera: TerrainLabCamera,
  deltaX: number,
  deltaY: number,
  width: number,
  height: number,
): TerrainLabCamera {
  return {
    yaw: camera.yaw + (deltaX / Math.max(width, 1)) * Math.PI * 1.5,
    pitch: Math.max(
      0.12,
      Math.min(1.25, camera.pitch + (deltaY / Math.max(height, 1)) * Math.PI),
    ),
  };
}

export function validSeed(value: string | null): string | undefined {
  if (value === null || !/^-?\d+$/u.test(value.trim())) {
    return undefined;
  }
  try {
    const parsed = BigInt(value.trim());
    if (parsed < I64_MIN || parsed > I64_MAX) {
      return undefined;
    }
    return parsed.toString();
  } catch {
    return undefined;
  }
}

function validI32(value: string | null): number | undefined {
  if (value === null || !/^-?\d+$/u.test(value.trim())) {
    return undefined;
  }
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < I32_MIN || parsed > I32_MAX) {
    return undefined;
  }
  return parsed;
}

function validSpacing(value: string | null): TerrainLabSpacing | undefined {
  const parsed = validI32(value);
  return TERRAIN_LAB_SPACINGS.find((spacing) => spacing === parsed);
}

function validDetail(value: string | null): TerrainLabDetail | undefined {
  return value === "auto" ? "auto" : validSpacing(value);
}

function validBlocksAcross(value: string | null): number | undefined {
  const parsed = validI32(value);
  if (
    parsed === undefined
    || parsed < TERRAIN_LAB_MIN_BLOCKS_ACROSS
    || parsed > TERRAIN_LAB_MAX_BLOCKS_ACROSS
  ) {
    return undefined;
  }
  return parsed;
}

function validMember<T extends string>(value: string | null, values: Set<T>): T | undefined {
  return value !== null && values.has(value as T) ? value as T : undefined;
}

function clampBlocksAcross(value: number): number {
  return Math.max(
    TERRAIN_LAB_MIN_BLOCKS_ACROSS,
    Math.min(TERRAIN_LAB_MAX_BLOCKS_ACROSS, Math.round(value)),
  );
}

function clampI32(value: number): number {
  return Math.max(I32_MIN, Math.min(I32_MAX, value));
}
