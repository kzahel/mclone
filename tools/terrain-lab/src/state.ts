export const TERRAIN_LAB_SPACINGS = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024] as const;
export const TERRAIN_LAB_CELLS_PER_AXIS = 64;
export const TERRAIN_LAB_MIN_BLOCKS_ACROSS = 1;
export const TERRAIN_LAB_MAX_BLOCKS_ACROSS = 131_072;
export const TERRAIN_LAB_CHUNK_WIDTH = 16;
export const TERRAIN_LAB_CANONICAL_RADII = [0, 1, 2, 3, 4] as const;

export type TerrainLabSpacing = (typeof TERRAIN_LAB_SPACINGS)[number];
export type TerrainLabDetail = "auto" | TerrainLabSpacing;
export type TerrainLabSource = "gpu" | "reference" | "split";
export type TerrainLabPane = "canonical" | "cpu" | "gpu";
export type CanonicalTerrainStage = "surface" | "final";
export type TerrainLabView = "map" | "3d";
export type TerrainLabProjection = "orthographic" | "perspective";
export type TerrainLabContentStage =
  | "base"
  | "hydrology"
  | "structured"
  | "surface"
  | "cover";
export type TerrainLabLayer =
  | "terrain"
  | "height"
  | "error"
  | "continentalness"
  | "climate"
  | "rivers"
  | "wetlands"
  | "landforms"
  | "biomes"
  | "surface"
  | "streams";

export interface TerrainLabState {
  seed: string;
  centerX: number;
  centerZ: number;
  blocksAcross: number;
  detail: TerrainLabDetail;
  source: TerrainLabSource;
  panes: TerrainLabPane[];
  canonicalStage: CanonicalTerrainStage;
  canonicalRadius: number;
  waterVisible: boolean;
  vegetationVisible: boolean;
  contentStage: TerrainLabContentStage;
  view: TerrainLabView;
  projection: TerrainLabProjection;
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
  blocksAcross: 512,
  detail: "auto",
  source: "split",
  panes: ["canonical", "cpu", "gpu"],
  canonicalStage: "final",
  canonicalRadius: 2,
  waterVisible: true,
  vegetationVisible: true,
  contentStage: "hydrology",
  view: "3d",
  projection: "orthographic",
  layer: "terrain",
};

export const REVIEW_TERRAIN_LAB_STATE: TerrainLabState = {
  ...DEFAULT_TERRAIN_LAB_STATE,
  blocksAcross: 2_048,
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
const PANES = new Set<TerrainLabPane>(["canonical", "cpu", "gpu"]);
const CANONICAL_STAGES = new Set<CanonicalTerrainStage>(["surface", "final"]);
const VIEWS = new Set<TerrainLabView>(["map", "3d"]);
const PROJECTIONS = new Set<TerrainLabProjection>(["orthographic", "perspective"]);
const CONTENT_STAGES = new Set<TerrainLabContentStage>([
  "base",
  "hydrology",
  "structured",
  "surface",
  "cover",
]);
const LAYERS = new Set<TerrainLabLayer>([
  "terrain",
  "height",
  "error",
  "continentalness",
  "climate",
  "rivers",
  "wetlands",
  "landforms",
  "biomes",
  "surface",
  "streams",
]);

export function parseTerrainLabState(
  search: string,
  fallback: TerrainLabState = DEFAULT_TERRAIN_LAB_STATE,
): TerrainLabState {
  const params = new URLSearchParams(search);
  const legacySpacing = validSpacing(params.get("spacing"));
  const legacySource = validMember(params.get("source"), SOURCES);
  const panes = validPanes(params.get("panes"))
    ?? (legacySource
      ? panesForProceduralSource(legacySource)
      : fallback.panes);
  return {
    seed: validSeed(params.get("seed")) ?? fallback.seed,
    centerX: validI32(params.get("x")) ?? fallback.centerX,
    centerZ: validI32(params.get("z")) ?? fallback.centerZ,
    blocksAcross:
      validBlocksAcross(params.get("blocks"))
      ?? (legacySpacing === undefined ? undefined : legacySpacing * TERRAIN_LAB_CELLS_PER_AXIS)
      ?? fallback.blocksAcross,
    detail: validDetail(params.get("detail")) ?? legacySpacing ?? fallback.detail,
    source: proceduralSourceForPanes(panes),
    panes,
    canonicalStage:
      validMember(params.get("canonical"), CANONICAL_STAGES) ?? fallback.canonicalStage,
    canonicalRadius:
      validCanonicalRadius(params.get("radius")) ?? fallback.canonicalRadius,
    waterVisible: validBoolean(params.get("water")) ?? fallback.waterVisible,
    vegetationVisible:
      validBoolean(params.get("vegetation")) ?? fallback.vegetationVisible,
    contentStage:
      validMember(params.get("stage"), CONTENT_STAGES) ?? fallback.contentStage,
    view: validMember(params.get("view"), VIEWS) ?? fallback.view,
    projection:
      validMember(params.get("projection"), PROJECTIONS) ?? fallback.projection,
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
  params.set("source", proceduralSourceForPanes(state.panes));
  params.set("panes", state.panes.join(","));
  params.set("canonical", state.canonicalStage);
  params.set("radius", String(state.canonicalRadius));
  params.set("water", state.waterVisible ? "1" : "0");
  params.set("vegetation", state.vegetationVisible ? "1" : "0");
  params.set("stage", state.contentStage);
  params.set("view", state.view);
  params.set("projection", state.projection);
  params.set("layer", state.layer);
  return `?${params.toString()}`;
}

export function proceduralSourceForPanes(panes: readonly TerrainLabPane[]): TerrainLabSource {
  const cpu = panes.includes("cpu");
  const gpu = panes.includes("gpu");
  if (cpu && gpu) {
    return "split";
  }
  return gpu ? "gpu" : "reference";
}

export function panesForProceduralSource(source: TerrainLabSource): TerrainLabPane[] {
  switch (source) {
    case "gpu":
      return ["gpu"];
    case "split":
      return ["cpu", "gpu"];
    case "reference":
      return ["cpu"];
  }
}

export function toggleTerrainLabPane(
  state: TerrainLabState,
  pane: TerrainLabPane,
): TerrainLabState {
  const visible = state.panes.includes(pane);
  if (visible && state.panes.length === 1) {
    return state;
  }
  const panes = (visible
    ? state.panes.filter((current) => current !== pane)
    : [...state.panes, pane]
  ).sort((left, right) => paneOrder(left) - paneOrder(right));
  return {
    ...state,
    panes,
    source: proceduralSourceForPanes(panes),
  };
}

export function footprintBlocks(state: Pick<TerrainLabState, "blocksAcross">): number {
  return state.blocksAcross;
}

export function canonicalTerrainCenterChunk(center: number): number {
  return Math.floor(center / TERRAIN_LAB_CHUNK_WIDTH);
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

export function grabPanTerrainLabStateInView(
  state: TerrainLabState,
  camera: TerrainLabCamera,
  pointerDeltaX: number,
  pointerDeltaY: number,
  panelWidth: number,
  panelHeight: number,
  panelAspect: number,
): TerrainLabState {
  if (state.view === "map") {
    return grabPanTerrainLabState(
      state,
      pointerDeltaX,
      pointerDeltaY,
      panelWidth,
      panelHeight,
      panelAspect,
    );
  }
  const horizontal =
    (pointerDeltaX / Math.max(panelWidth, 1)) * state.blocksAcross;
  const depth =
    (pointerDeltaY / Math.max(panelHeight, 1))
    * (state.blocksAcross / Math.max(panelAspect, 0.01));
  return panTerrainLabState(
    state,
    Math.sin(camera.yaw) * horizontal + Math.cos(camera.yaw) * depth,
    Math.cos(camera.yaw) * horizontal - Math.sin(camera.yaw) * depth,
  );
}

export function pinchPanZoomTerrainLabState(
  state: TerrainLabState,
  camera: TerrainLabCamera,
  zoomFactor: number,
  anchorX: number,
  anchorZ: number,
  centroidDeltaX: number,
  centroidDeltaY: number,
  panelWidth: number,
  panelHeight: number,
  panelAspect: number,
): TerrainLabState {
  const zoomed = zoomTerrainLabState(
    state,
    zoomFactor,
    state.view === "map" ? anchorX : 0,
    state.view === "map" ? anchorZ : 0,
    panelAspect,
  );
  return grabPanTerrainLabStateInView(
    zoomed,
    camera,
    centroidDeltaX,
    centroidDeltaY,
    panelWidth,
    panelHeight,
    panelAspect,
  );
}

export function arrowPanTerrainLabState(
  state: TerrainLabState,
  key: string,
): TerrainLabState {
  const distance = footprintBlocks(state) / 8;
  switch (key) {
    case "ArrowUp":
      return panTerrainLabState(state, 0, -distance);
    case "ArrowDown":
      return panTerrainLabState(state, 0, distance);
    case "ArrowLeft":
      return panTerrainLabState(state, -distance, 0);
    case "ArrowRight":
      return panTerrainLabState(state, distance, 0);
    default:
      return state;
  }
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

function validPanes(value: string | null): TerrainLabPane[] | undefined {
  if (value === null) {
    return undefined;
  }
  const panes = value
    .split(",")
    .filter((pane): pane is TerrainLabPane => PANES.has(pane as TerrainLabPane))
    .filter((pane, index, all) => all.indexOf(pane) === index)
    .sort((left, right) => paneOrder(left) - paneOrder(right));
  return panes.length > 0 ? panes : undefined;
}

function validCanonicalRadius(value: string | null): number | undefined {
  const parsed = validI32(value);
  return parsed !== undefined
    && TERRAIN_LAB_CANONICAL_RADII.includes(
      parsed as (typeof TERRAIN_LAB_CANONICAL_RADII)[number],
    )
    ? parsed
    : undefined;
}

function validBoolean(value: string | null): boolean | undefined {
  if (value === "1" || value === "true") {
    return true;
  }
  if (value === "0" || value === "false") {
    return false;
  }
  return undefined;
}

function paneOrder(pane: TerrainLabPane): number {
  return ["canonical", "cpu", "gpu"].indexOf(pane);
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
