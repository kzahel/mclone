export const TERRAIN_LAB_SPACINGS = [2, 4, 8, 16, 32, 64, 128, 256, 512, 1024] as const;
export const TERRAIN_LAB_CELLS_PER_AXIS = 64;

export type TerrainLabSpacing = (typeof TERRAIN_LAB_SPACINGS)[number];
export type TerrainLabSource = "gpu" | "reference" | "split";
export type TerrainLabView = "map" | "3d";
export type TerrainLabLayer = "terrain" | "height" | "error" | "continentalness" | "climate";

export interface TerrainLabState {
  seed: string;
  centerX: number;
  centerZ: number;
  spacing: TerrainLabSpacing;
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
  spacing: 32,
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
  return {
    seed: validSeed(params.get("seed")) ?? fallback.seed,
    centerX: validI32(params.get("x")) ?? fallback.centerX,
    centerZ: validI32(params.get("z")) ?? fallback.centerZ,
    spacing: validSpacing(params.get("spacing")) ?? fallback.spacing,
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
  params.set("spacing", String(state.spacing));
  params.set("source", state.source);
  params.set("view", state.view);
  params.set("layer", state.layer);
  return `?${params.toString()}`;
}

export function footprintBlocks(state: Pick<TerrainLabState, "spacing">): number {
  return state.spacing * TERRAIN_LAB_CELLS_PER_AXIS;
}

export function nextSpacing(
  spacing: TerrainLabSpacing,
  direction: "in" | "out",
): TerrainLabSpacing {
  const current = TERRAIN_LAB_SPACINGS.indexOf(spacing);
  const delta = direction === "in" ? -1 : 1;
  const index = Math.max(0, Math.min(TERRAIN_LAB_SPACINGS.length - 1, current + delta));
  return TERRAIN_LAB_SPACINGS[index] ?? spacing;
}

export function panTerrainLabState(
  state: TerrainLabState,
  deltaX: number,
  deltaZ: number,
): TerrainLabState {
  return {
    ...state,
    centerX: clampI32(snapToSpacing(state.centerX + deltaX, state.spacing)),
    centerZ: clampI32(snapToSpacing(state.centerZ + deltaZ, state.spacing)),
  };
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

function validMember<T extends string>(value: string | null, values: Set<T>): T | undefined {
  return value !== null && values.has(value as T) ? value as T : undefined;
}

function snapToSpacing(value: number, spacing: number): number {
  return Math.round(value / spacing) * spacing;
}

function clampI32(value: number): number {
  return Math.max(I32_MIN, Math.min(I32_MAX, value));
}
