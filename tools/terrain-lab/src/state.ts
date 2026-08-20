export const TERRAIN_LAB_SPACINGS = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024] as const;
export const TERRAIN_LAB_CELLS_PER_AXIS = 64;
export const TERRAIN_LAB_MIN_BLOCKS_ACROSS = 1;
export const TERRAIN_LAB_MAX_BLOCKS_ACROSS = 131_072;
export const TERRAIN_LAB_CHUNK_WIDTH = 16;
export const TERRAIN_LAB_CANONICAL_RADII = [0, 1, 2, 3, 4, 5, 7, 10, 15] as const;

export type TerrainLabSpacing = (typeof TERRAIN_LAB_SPACINGS)[number];
export type TerrainLabDetail = "auto" | TerrainLabSpacing;
export type TerrainLabSurfaceQuality = "basic" | "inferred";
export type TerrainLabProfile = "mclone-overworld-v1" | "overworld";
export type TerrainLabVisualProfile =
  | "mclone-original"
  | "minecraft-reference"
  | "hybrid-authoring"
  | "first-party-coverage"
  | "provisional-audit";
export type TerrainLabTexturePresentation = "textured" | "flat-colors";
export type TerrainLabComparisonVisualProfile = "off" | TerrainLabVisualProfile;
export type TerrainLabSource = "gpu" | "reference" | "macro" | "split";
export type StreamedPlanAtlasTopology = "plane" | "cylinder-x" | "torus";
export type ContinentalEcoregionTopology = "plane" | "cylinder-x-196608";
export type ContinentalEcoregionLayer =
  | "composed"
  | "land-ocean"
  | "province"
  | "ecoregion"
  | "transition"
  | "openness"
  | "clearings"
  | "water"
  | "habitat"
  | "production-control";
export type SemanticTerrainSubstrate = "flat" | "quiet";
export type SemanticTerrainFeatures = "range" | "basin" | "combined";
export type SemanticTerrainCorrection = "regional" | "local";
export type SemanticTerrainVerticalScale = "1x" | "8x" | "24x";
export type TerrainLabPane =
  | "runtime"
  | "canonical"
  | "ecoregion"
  | "plan"
  | "wildlife"
  | "atlas"
  | "semantic"
  | "cpu"
  | "macro"
  | "gpu";
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
  | "streams"
  | "forests";

export interface TerrainLabState {
  profile: TerrainLabProfile;
  visualProfile: TerrainLabVisualProfile;
  texturePresentation: TerrainLabTexturePresentation;
  compareVisualProfile: TerrainLabComparisonVisualProfile;
  seed: string;
  centerX: number;
  centerZ: number;
  blocksAcross: number;
  detail: TerrainLabDetail;
  surfaceQuality: TerrainLabSurfaceQuality;
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
  ecoregionTopology: ContinentalEcoregionTopology;
  ecoregionLayer: ContinentalEcoregionLayer;
  planBasinsVisible: boolean;
  planQuietVisible: boolean;
  planDrainageVisible: boolean;
  planDividesVisible: boolean;
  planConfluencesVisible: boolean;
  planSinksVisible: boolean;
  atlasTopology: StreamedPlanAtlasTopology;
  atlasRegionsVisible: boolean;
  atlasFallbackSamplesVisible: boolean;
  atlasFallbackFeaturesVisible: boolean;
  atlasHierarchyVisible: boolean;
  atlasFacetsVisible: boolean;
  atlasGraphBoundsVisible: boolean;
  atlasGraphEdgesVisible: boolean;
  atlasWitnessParentVisible: boolean;
  atlasWitnessRegionalVisible: boolean;
  atlasWitnessLocalVisible: boolean;
  atlasWitnessBoundsVisible: boolean;
  atlasIdentityVisible: boolean;
  atlasSeamsVisible: boolean;
  semanticSubstrate: SemanticTerrainSubstrate;
  semanticFeatures: SemanticTerrainFeatures;
  semanticTopology: StreamedPlanAtlasTopology;
  semanticCorrection: SemanticTerrainCorrection;
  semanticVerticalScale: SemanticTerrainVerticalScale;
  semanticGuidesVisible: boolean;
}

export interface TerrainLabCamera {
  yaw: number;
  pitch: number;
}

export const DEFAULT_TERRAIN_LAB_STATE: TerrainLabState = {
  profile: "mclone-overworld-v1",
  visualProfile: "mclone-original",
  texturePresentation: "textured",
  compareVisualProfile: "off",
  seed: "-98765",
  centerX: -304,
  centerZ: 336,
  blocksAcross: 512,
  detail: "auto",
  surfaceQuality: "inferred",
  source: "reference",
  panes: ["runtime"],
  canonicalStage: "final",
  canonicalRadius: 2,
  waterVisible: true,
  vegetationVisible: true,
  contentStage: "hydrology",
  view: "3d",
  projection: "orthographic",
  layer: "terrain",
  ecoregionTopology: "plane",
  ecoregionLayer: "composed",
  planBasinsVisible: true,
  planQuietVisible: true,
  planDrainageVisible: true,
  planDividesVisible: true,
  planConfluencesVisible: true,
  planSinksVisible: true,
  atlasTopology: "plane",
  atlasRegionsVisible: true,
  atlasFallbackSamplesVisible: true,
  atlasFallbackFeaturesVisible: true,
  atlasHierarchyVisible: true,
  atlasFacetsVisible: true,
  atlasGraphBoundsVisible: true,
  atlasGraphEdgesVisible: true,
  atlasWitnessParentVisible: true,
  atlasWitnessRegionalVisible: true,
  atlasWitnessLocalVisible: true,
  atlasWitnessBoundsVisible: false,
  atlasIdentityVisible: false,
  atlasSeamsVisible: true,
  semanticSubstrate: "quiet",
  semanticFeatures: "combined",
  semanticTopology: "plane",
  semanticCorrection: "local",
  semanticVerticalScale: "1x",
  semanticGuidesVisible: true,
};

export const REVIEW_TERRAIN_LAB_STATE: TerrainLabState = {
  ...DEFAULT_TERRAIN_LAB_STATE,
  blocksAcross: 2_048,
};

export const DEFAULT_TERRAIN_LAB_CAMERA: TerrainLabCamera = {
  yaw: Math.PI / 4,
  pitch: 0.48,
};

export function parseTerrainLabReviewCamera(
  search: string,
): TerrainLabCamera | undefined {
  const params = new URLSearchParams(search);
  const yaw = validFiniteNumber(params.get("reviewYaw"));
  const pitch = validFiniteNumber(params.get("reviewPitch"));
  if (yaw === undefined || pitch === undefined || pitch < 0.12 || pitch > 1.25) {
    return undefined;
  }
  return { yaw, pitch };
}

const I64_MIN = -(1n << 63n);
const I64_MAX = (1n << 63n) - 1n;
const I32_MIN = -2_147_483_648;
const I32_MAX = 2_147_483_647;
const PROFILES = new Set<TerrainLabProfile>(["mclone-overworld-v1", "overworld"]);
const VISUAL_PROFILES = new Set<TerrainLabVisualProfile>([
  "mclone-original",
  "minecraft-reference",
  "hybrid-authoring",
  "first-party-coverage",
  "provisional-audit",
]);
const TEXTURE_PRESENTATIONS = new Set<TerrainLabTexturePresentation>([
  "textured",
  "flat-colors",
]);
const COMPARISON_VISUAL_PROFILES =
  new Set<TerrainLabComparisonVisualProfile>(["off", ...VISUAL_PROFILES]);
const SOURCES = new Set<TerrainLabSource>(["gpu", "reference", "macro", "split"]);
const ATLAS_TOPOLOGIES = new Set<StreamedPlanAtlasTopology>([
  "plane",
  "cylinder-x",
  "torus",
]);
const ECOREGION_TOPOLOGIES = new Set<ContinentalEcoregionTopology>([
  "plane",
  "cylinder-x-196608",
]);
const ECOREGION_LAYERS = new Set<ContinentalEcoregionLayer>([
  "composed",
  "land-ocean",
  "province",
  "ecoregion",
  "transition",
  "openness",
  "clearings",
  "water",
  "habitat",
  "production-control",
]);
const SEMANTIC_SUBSTRATES = new Set<SemanticTerrainSubstrate>(["flat", "quiet"]);
const SEMANTIC_FEATURES = new Set<SemanticTerrainFeatures>([
  "range",
  "basin",
  "combined",
]);
const SEMANTIC_CORRECTIONS = new Set<SemanticTerrainCorrection>([
  "regional",
  "local",
]);
const SEMANTIC_VERTICAL_SCALES = new Set<SemanticTerrainVerticalScale>([
  "1x",
  "8x",
  "24x",
]);
const SURFACE_QUALITIES = new Set<TerrainLabSurfaceQuality>(["basic", "inferred"]);
const PANES = new Set<TerrainLabPane>([
  "runtime",
  "canonical",
  "ecoregion",
  "plan",
  "wildlife",
  "atlas",
  "semantic",
  "cpu",
  "macro",
  "gpu",
]);
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
  "forests",
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
  return normalizeTerrainLabProfileState({
    profile: validMember(params.get("profile"), PROFILES) ?? fallback.profile,
    visualProfile:
      validMember(params.get("visual"), VISUAL_PROFILES) ?? fallback.visualProfile,
    texturePresentation:
      validMember(params.get("texture"), TEXTURE_PRESENTATIONS)
      ?? fallback.texturePresentation,
    compareVisualProfile:
      validMember(params.get("compareVisual"), COMPARISON_VISUAL_PROFILES)
      ?? fallback.compareVisualProfile,
    seed: validSeed(params.get("seed")) ?? fallback.seed,
    centerX: validI32(params.get("x")) ?? fallback.centerX,
    centerZ: validI32(params.get("z")) ?? fallback.centerZ,
    blocksAcross:
      validBlocksAcross(params.get("blocks"))
      ?? (legacySpacing === undefined ? undefined : legacySpacing * TERRAIN_LAB_CELLS_PER_AXIS)
      ?? fallback.blocksAcross,
    detail: validDetail(params.get("detail")) ?? legacySpacing ?? fallback.detail,
    surfaceQuality:
      validMember(params.get("surface"), SURFACE_QUALITIES) ?? fallback.surfaceQuality,
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
    ecoregionTopology:
      validMember(params.get("ecoregionTopology"), ECOREGION_TOPOLOGIES)
      ?? fallback.ecoregionTopology,
    ecoregionLayer:
      validMember(params.get("ecoregionLayer"), ECOREGION_LAYERS)
      ?? fallback.ecoregionLayer,
    planBasinsVisible:
      validBoolean(params.get("planBasins")) ?? fallback.planBasinsVisible,
    planQuietVisible:
      validBoolean(params.get("planQuiet")) ?? fallback.planQuietVisible,
    planDrainageVisible:
      validBoolean(params.get("planDrainage")) ?? fallback.planDrainageVisible,
    planDividesVisible:
      validBoolean(params.get("planDivides")) ?? fallback.planDividesVisible,
    planConfluencesVisible:
      validBoolean(params.get("planConfluences"))
      ?? fallback.planConfluencesVisible,
    planSinksVisible:
      validBoolean(params.get("planSinks")) ?? fallback.planSinksVisible,
    atlasTopology:
      validMember(params.get("atlasTopology"), ATLAS_TOPOLOGIES)
      ?? fallback.atlasTopology,
    atlasRegionsVisible:
      validBoolean(params.get("atlasRegions")) ?? fallback.atlasRegionsVisible,
    atlasFallbackSamplesVisible:
      validBoolean(params.get("atlasSamples"))
      ?? fallback.atlasFallbackSamplesVisible,
    atlasFallbackFeaturesVisible:
      validBoolean(params.get("atlasFeatures"))
      ?? fallback.atlasFallbackFeaturesVisible,
    atlasHierarchyVisible:
      validBoolean(params.get("atlasHierarchy"))
      ?? fallback.atlasHierarchyVisible,
    atlasFacetsVisible:
      validBoolean(params.get("atlasFacets")) ?? fallback.atlasFacetsVisible,
    atlasGraphBoundsVisible:
      validBoolean(params.get("atlasGraphBounds"))
      ?? fallback.atlasGraphBoundsVisible,
    atlasGraphEdgesVisible:
      validBoolean(params.get("atlasGraphEdges"))
      ?? fallback.atlasGraphEdgesVisible,
    atlasWitnessParentVisible:
      validBoolean(params.get("atlasWitnessParent"))
      ?? fallback.atlasWitnessParentVisible,
    atlasWitnessRegionalVisible:
      validBoolean(params.get("atlasWitnessRegional"))
      ?? fallback.atlasWitnessRegionalVisible,
    atlasWitnessLocalVisible:
      validBoolean(params.get("atlasWitnessLocal"))
      ?? fallback.atlasWitnessLocalVisible,
    atlasWitnessBoundsVisible:
      validBoolean(params.get("atlasWitnessBounds"))
      ?? fallback.atlasWitnessBoundsVisible,
    atlasIdentityVisible:
      validBoolean(params.get("atlasIdentity"))
      ?? fallback.atlasIdentityVisible,
    atlasSeamsVisible:
      validBoolean(params.get("atlasSeams")) ?? fallback.atlasSeamsVisible,
    semanticSubstrate:
      validMember(params.get("semanticSubstrate"), SEMANTIC_SUBSTRATES)
      ?? fallback.semanticSubstrate,
    semanticFeatures:
      validMember(params.get("semanticFeatures"), SEMANTIC_FEATURES)
      ?? fallback.semanticFeatures,
    semanticTopology:
      validMember(params.get("semanticTopology"), ATLAS_TOPOLOGIES)
      ?? fallback.semanticTopology,
    semanticCorrection:
      validMember(params.get("semanticCorrection"), SEMANTIC_CORRECTIONS)
      ?? fallback.semanticCorrection,
    semanticVerticalScale:
      validMember(params.get("semanticVertical"), SEMANTIC_VERTICAL_SCALES)
      ?? fallback.semanticVerticalScale,
    semanticGuidesVisible:
      validBoolean(params.get("semanticGuides"))
      ?? fallback.semanticGuidesVisible,
  });
}

export function terrainLabSearch(
  state: TerrainLabState,
  reviewCamera?: TerrainLabCamera,
): string {
  const params = new URLSearchParams();
  params.set("profile", state.profile);
  params.set("visual", state.visualProfile);
  params.set("texture", state.texturePresentation);
  params.set("compareVisual", state.compareVisualProfile);
  params.set("seed", state.seed);
  params.set("x", String(state.centerX));
  params.set("z", String(state.centerZ));
  params.set("blocks", String(state.blocksAcross));
  params.set("detail", String(state.detail));
  params.set("surface", state.surfaceQuality);
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
  params.set("ecoregionTopology", state.ecoregionTopology);
  params.set("ecoregionLayer", state.ecoregionLayer);
  params.set("planBasins", state.planBasinsVisible ? "1" : "0");
  params.set("planQuiet", state.planQuietVisible ? "1" : "0");
  params.set("planDrainage", state.planDrainageVisible ? "1" : "0");
  params.set("planDivides", state.planDividesVisible ? "1" : "0");
  params.set("planConfluences", state.planConfluencesVisible ? "1" : "0");
  params.set("planSinks", state.planSinksVisible ? "1" : "0");
  params.set("atlasTopology", state.atlasTopology);
  params.set("atlasRegions", state.atlasRegionsVisible ? "1" : "0");
  params.set("atlasSamples", state.atlasFallbackSamplesVisible ? "1" : "0");
  params.set("atlasFeatures", state.atlasFallbackFeaturesVisible ? "1" : "0");
  params.set("atlasHierarchy", state.atlasHierarchyVisible ? "1" : "0");
  params.set("atlasFacets", state.atlasFacetsVisible ? "1" : "0");
  params.set("atlasGraphBounds", state.atlasGraphBoundsVisible ? "1" : "0");
  params.set("atlasGraphEdges", state.atlasGraphEdgesVisible ? "1" : "0");
  params.set("atlasWitnessParent", state.atlasWitnessParentVisible ? "1" : "0");
  params.set("atlasWitnessRegional", state.atlasWitnessRegionalVisible ? "1" : "0");
  params.set("atlasWitnessLocal", state.atlasWitnessLocalVisible ? "1" : "0");
  params.set("atlasWitnessBounds", state.atlasWitnessBoundsVisible ? "1" : "0");
  params.set("atlasIdentity", state.atlasIdentityVisible ? "1" : "0");
  params.set("atlasSeams", state.atlasSeamsVisible ? "1" : "0");
  params.set("semanticSubstrate", state.semanticSubstrate);
  params.set("semanticFeatures", state.semanticFeatures);
  params.set("semanticTopology", state.semanticTopology);
  params.set("semanticCorrection", state.semanticCorrection);
  params.set("semanticVertical", state.semanticVerticalScale);
  params.set("semanticGuides", state.semanticGuidesVisible ? "1" : "0");
  if (reviewCamera) {
    params.set("reviewYaw", String(reviewCamera.yaw));
    params.set("reviewPitch", String(reviewCamera.pitch));
  }
  return `?${params.toString()}`;
}

export function terrainLabPlayHref(
  state: Pick<TerrainLabState, "profile" | "seed" | "centerX" | "centerZ">,
): string {
  const params = new URLSearchParams();
  params.set("startInWorld", "true");
  params.set("seed", state.seed);
  params.set("generationProfile", state.profile);
  params.set("chunkX", String(canonicalTerrainCenterChunk(state.centerX)));
  params.set("chunkZ", String(canonicalTerrainCenterChunk(state.centerZ)));
  params.set("movementMode", "fly");
  params.set(
    "terrainPresentation",
    state.profile === "mclone-overworld-v1" ? "composed" : "exact-only",
  );
  return `/play/?${params.toString()}`;
}

export function proceduralSourceForPanes(panes: readonly TerrainLabPane[]): TerrainLabSource {
  const cpu = panes.includes("cpu");
  const gpu = panes.includes("gpu");
  const macro = panes.includes("macro");
  if (cpu && (gpu || macro)) {
    return "split";
  }
  return macro ? "macro" : gpu ? "gpu" : "reference";
}

export function panesForProceduralSource(source: TerrainLabSource): TerrainLabPane[] {
  switch (source) {
    case "gpu":
      return ["gpu"];
    case "macro":
      return ["macro"];
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
  if (
    (state.profile === "overworld" && pane === "gpu")
    || (state.profile === "overworld" && pane === "runtime")
    || (state.profile === "overworld" && pane === "ecoregion")
    || (state.profile === "overworld" && pane === "plan")
    || (state.profile === "overworld" && pane === "wildlife")
    || (state.profile === "overworld" && pane === "atlas")
    || (state.profile === "overworld" && pane === "semantic")
    || (state.profile !== "overworld" && pane === "macro")
  ) {
    return state;
  }
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

export function switchTerrainLabProfile(
  state: TerrainLabState,
  profile: TerrainLabProfile,
): TerrainLabState {
  const panes = state.panes.map((pane) => {
    if (
      profile === "overworld"
      && (
        pane === "gpu"
        || pane === "runtime"
        || pane === "ecoregion"
        || pane === "plan"
        || pane === "wildlife"
        || pane === "atlas"
        || pane === "semantic"
      )
    ) {
      return "macro";
    }
    if (profile !== "overworld" && pane === "macro") {
      return "gpu";
    }
    return pane;
  });
  return normalizeTerrainLabProfileState({
    ...state,
    profile,
    panes: [...new Set(panes)].sort((left, right) => paneOrder(left) - paneOrder(right)),
  });
}

export function normalizeTerrainLabProfileState(
  state: TerrainLabState,
): TerrainLabState {
  const panes = state.panes.filter((pane) =>
    state.profile === "overworld"
      ? pane !== "gpu"
        && pane !== "runtime"
        && pane !== "ecoregion"
        && pane !== "plan"
        && pane !== "wildlife"
        && pane !== "atlas"
        && pane !== "semantic"
      : pane !== "macro"
  );
  if (panes.length === 0) {
    panes.push(state.profile === "overworld" ? "macro" : "gpu");
  }
  if (state.profile !== "overworld") {
    return {
      ...state,
      panes,
      source: proceduralSourceForPanes(panes),
    };
  }
  const layer = state.layer === "terrain"
    || state.layer === "height"
    || state.layer === "error"
    || state.layer === "biomes"
    || state.layer === "surface"
    ? state.layer
    : "terrain";
  return {
    ...state,
    panes,
    source: proceduralSourceForPanes(panes),
    contentStage: "surface",
    layer,
  };
}

export function footprintBlocks(state: Pick<TerrainLabState, "blocksAcross">): number {
  return state.blocksAcross;
}

export function canonicalTerrainCenterChunk(center: number): number {
  return Math.floor(center / TERRAIN_LAB_CHUNK_WIDTH);
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

function validFiniteNumber(value: string | null): number | undefined {
  if (value === null || value.trim() === "") {
    return undefined;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function paneOrder(pane: TerrainLabPane): number {
  return [
    "runtime",
    "canonical",
    "ecoregion",
    "plan",
    "wildlife",
    "atlas",
    "semantic",
    "cpu",
    "macro",
    "gpu",
  ].indexOf(pane);
}

function validMember<T extends string>(value: string | null, values: Set<T>): T | undefined {
  return value !== null && values.has(value as T) ? value as T : undefined;
}
