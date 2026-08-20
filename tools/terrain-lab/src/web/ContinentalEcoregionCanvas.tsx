import { useEffect, useMemo, useRef, useState } from "react";

import type {
  ContinentalEcoregionLayer,
  TerrainLabCamera,
  TerrainLabState,
} from "../state";
import type {
  ContinentalEcoregionAtlasMetadata,
  ContinentalEcoregionWorkerQuery,
  ContinentalEcoregionWorkerResponse,
  ContinentalEcoregionWorkerSummary,
} from "./continental-ecoregion-worker-protocol";
import { initializeTerrainLab } from "./terrain-lab-wasm";
import { useWorldViewNavigation } from "./use-world-view-navigation";

interface CanvasSize {
  width: number;
  height: number;
  cssWidth: number;
  cssHeight: number;
  dpr: number;
}

interface AtlasViewport {
  minX: number;
  minZ: number;
  blocksAcross: number;
  blocksTall: number;
}

const INTERACTION_SETTLE_MS = 100;

export interface ContinentalEcoregionReport {
  schema: string;
  witnessSha256: string;
  semanticSha256: string;
  topology: string;
  compileMs: number;
  drawMs: number;
  sampleCount: number;
  sampleStepBlocks: number;
  landFraction: number;
  oceanFraction: number;
  productionTerrainUnchanged: boolean;
  metadata: ContinentalEcoregionAtlasMetadata;
}

interface ContinentalEcoregionCanvasProps {
  state: TerrainLabState;
  camera: TerrainLabCamera;
  onStateChange: (state: TerrainLabState) => void;
  onCameraChange: (camera: TerrainLabCamera) => void;
  onReport: (report: ContinentalEcoregionReport | undefined) => void;
  onError: (error: string | undefined) => void;
}

const NONE = 255;
const PROVINCE_COLORS = [
  [82, 129, 91],
  [73, 119, 126],
  [157, 151, 82],
  [55, 101, 67],
  [137, 128, 108],
  [111, 137, 105],
] as const;
const ECOREGION_COLORS = [
  [34, 75, 51],
  [165, 195, 96],
  [52, 119, 81],
  [57, 130, 126],
  [75, 127, 68],
  [145, 127, 84],
  [113, 137, 104],
] as const;
const CLEARING_COLORS = [
  [198, 199, 93],
  [179, 108, 62],
  [143, 155, 102],
  [117, 176, 140],
  [179, 163, 104],
] as const;
const HABITAT_ROUTE_COLORS = [
  [44, 164, 176],
  [65, 178, 151],
  [175, 154, 87],
  [210, 189, 92],
] as const;
const PRODUCTION_BIOME_COLORS = [
  [23, 75, 105],
  [192, 177, 119],
  [42, 132, 150],
  [203, 218, 218],
  [48, 94, 79],
  [176, 151, 77],
  [65, 116, 66],
  [147, 174, 91],
] as const;
const PRODUCTION_BIOME_LABELS = [
  "ocean",
  "shore",
  "river",
  "snowy alpine",
  "cool wet conifer",
  "warm dry steppe",
  "temperate woodland",
  "temperate meadow",
] as const;

type ProductionEcoregionLayer = Extract<
  ContinentalEcoregionLayer,
  `production-${string}`
>;

export function ContinentalEcoregionCanvas({
  state,
  camera,
  onStateChange,
  onCameraChange,
  onReport,
  onError,
}: ContinentalEcoregionCanvasProps): React.JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rasterRef = useRef<HTMLCanvasElement | undefined>(undefined);
  const rasterKeyRef = useRef("");
  const stageRef = useRef<HTMLDivElement>(null);
  const workerRef = useRef<Worker | undefined>(undefined);
  const revisionRef = useRef(0);
  const inFlightRevisionRef = useRef<number | undefined>(undefined);
  const pendingQueryRef = useRef<ContinentalEcoregionWorkerQuery | undefined>(
    undefined,
  );
  const pendingTimerRef = useRef(0);
  const pendingDueAtRef = useRef(0);
  const responseRef = useRef<ContinentalEcoregionWorkerSummary | undefined>(undefined);
  const postPendingQueryRef = useRef<() => void>(() => undefined);
  const schedulePendingQueryRef = useRef<(delayMs: number) => void>(() => undefined);
  const [response, setResponse] = useState<ContinentalEcoregionWorkerSummary>();
  const [updating, setUpdating] = useState(false);
  const [navigationReady, setNavigationReady] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState<number>();
  const [canvasSize, setCanvasSize] = useState<CanvasSize>({
    width: 1_200,
    height: 780,
    cssWidth: 1_200,
    cssHeight: 780,
    dpr: 1,
  });
  postPendingQueryRef.current = (): void => {
    const worker = workerRef.current;
    const query = pendingQueryRef.current;
    if (!worker || !query || inFlightRevisionRef.current !== undefined) {
      return;
    }
    pendingQueryRef.current = undefined;
    pendingDueAtRef.current = 0;
    inFlightRevisionRef.current = query.revision;
    worker.postMessage(query);
  };
  schedulePendingQueryRef.current = (delayMs: number): void => {
    if (pendingTimerRef.current !== 0) {
      window.clearTimeout(pendingTimerRef.current);
      pendingTimerRef.current = 0;
    }
    const boundedDelay = Math.max(0, delayMs);
    pendingDueAtRef.current = performance.now() + boundedDelay;
    if (boundedDelay === 0) {
      postPendingQueryRef.current();
      return;
    }
    pendingTimerRef.current = window.setTimeout(() => {
      pendingTimerRef.current = 0;
      postPendingQueryRef.current();
    }, boundedDelay);
  };
  const navigationState = useMemo<TerrainLabState>(
    () => ({ ...state, view: "map" }),
    [state],
  );
  const navigation = useWorldViewNavigation({
    stageRef,
    enabled: navigationReady,
    state: navigationState,
    camera,
    onStateChange: (next) => onStateChange({
      ...state,
      centerX: next.centerX,
      centerZ: next.centerZ,
      blocksAcross: next.blocksAcross,
    }),
    onCameraChange,
    resolveViewport: (clientX, clientY) => {
      const stage = stageRef.current;
      if (!stage) {
        return undefined;
      }
      const rect = stage.getBoundingClientRect();
      return {
        x: clientX - rect.left,
        y: clientY - rect.top,
        width: rect.width,
        height: rect.height,
      };
    },
    onTap: (clientX, clientY) => {
      const stage = stageRef.current;
      if (!stage || !response) {
        return;
      }
      const rect = stage.getBoundingClientRect();
      const column = Math.max(0, Math.min(
        response.metadata.columns - 1,
        Math.floor((clientX - rect.left) / Math.max(rect.width, 1)
          * response.metadata.columns),
      ));
      const row = Math.max(0, Math.min(
        response.metadata.rows - 1,
        Math.floor((clientY - rect.top) / Math.max(rect.height, 1)
          * response.metadata.rows),
      ));
      setSelectedIndex(row * response.metadata.columns + column);
    },
  });

  useEffect(() => {
    let cancelled = false;
    void initializeTerrainLab()
      .then(() => {
        if (!cancelled) {
          setNavigationReady(true);
        }
      })
      .catch((error: unknown) => onError(errorMessage(error)));
    return () => {
      cancelled = true;
    };
  }, [onError]);

  useEffect(() => {
    const worker = new Worker(
      new URL("./continental-ecoregion-worker.ts", import.meta.url),
      { type: "module" },
    );
    workerRef.current = worker;
    worker.onmessage = (
      event: MessageEvent<ContinentalEcoregionWorkerResponse>,
    ): void => {
      const next = event.data;
      if (next.revision !== inFlightRevisionRef.current) {
        return;
      }
      inFlightRevisionRef.current = undefined;
      const hasPending = pendingQueryRef.current !== undefined;
      if (next.type === "error") {
        if (!hasPending) {
          onError(next.message);
        }
      } else if (!hasPending) {
        responseRef.current = next;
        setResponse(next);
      }
      setUpdating(hasPending);
      if (hasPending) {
        schedulePendingQueryRef.current(
          Math.max(0, pendingDueAtRef.current - performance.now()),
        );
      }
    };
    worker.onerror = (event): void => {
      inFlightRevisionRef.current = undefined;
      setUpdating(false);
      onError(event.message || "Continental/ecoregion Worker failed.");
    };
    return () => {
      if (pendingTimerRef.current !== 0) {
        window.clearTimeout(pendingTimerRef.current);
        pendingTimerRef.current = 0;
      }
      worker.terminate();
      workerRef.current = undefined;
      inFlightRevisionRef.current = undefined;
      pendingQueryRef.current = undefined;
      responseRef.current = undefined;
    };
  }, [onError]);

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) {
      return;
    }
    const resize = (): void => {
      const rect = stage.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      setCanvasSize({
        width: Math.max(1, Math.round(rect.width * dpr)),
        height: Math.max(1, Math.round(rect.height * dpr)),
        cssWidth: Math.max(1, rect.width),
        cssHeight: Math.max(1, rect.height),
        dpr,
      });
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(stage);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!workerRef.current) {
      return;
    }
    revisionRef.current += 1;
    const query: ContinentalEcoregionWorkerQuery = {
      type: "query",
      revision: revisionRef.current,
      seed: state.seed,
      topology: state.ecoregionTopology,
      centerX: state.centerX,
      centerZ: state.centerZ,
      blocksAcross: state.blocksAcross,
      aspectRatio: canvasSize.cssWidth / Math.max(canvasSize.cssHeight, 1),
      samplesAcross: 256,
    };
    pendingQueryRef.current = query;
    setUpdating(true);
    setSelectedIndex(undefined);
    onReport(undefined);
    const published = responseRef.current;
    const canRetain = published?.metadata.seed === state.seed
      && published.metadata.topology === state.ecoregionTopology;
    if (!canRetain && published) {
      responseRef.current = undefined;
      rasterKeyRef.current = "";
      setResponse(undefined);
    }
    schedulePendingQueryRef.current(canRetain ? INTERACTION_SETTLE_MS : 0);
  }, [
    canvasSize.cssHeight,
    canvasSize.cssWidth,
    onReport,
    state.blocksAcross,
    state.centerX,
    state.centerZ,
    state.ecoregionTopology,
    state.seed,
  ]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    const context = canvas.getContext("2d");
    if (!context) {
      onError("This browser does not expose a 2D canvas context.");
      return;
    }
    const started = performance.now();
    drawAtlas(
      context,
      canvasSize,
      rasterRef,
      rasterKeyRef,
      state.ecoregionLayer,
      response,
      selectedIndex,
      requestedAtlasViewport(state, canvasSize),
    );
    if (response) {
      onReport({
        schema: response.metadata.receiptSchema,
        witnessSha256: response.suiteSha256,
        semanticSha256: response.metadata.semanticSha256,
        topology: response.metadata.topology,
        compileMs: response.compileMs,
        drawMs: performance.now() - started,
        sampleCount: response.metadata.sampleCount,
        sampleStepBlocks: response.metadata.sampleStepBlocks,
        landFraction: response.metadata.metrics.landFraction,
        oceanFraction: response.metadata.metrics.oceanFraction,
        productionTerrainUnchanged: response.metadata.productionTerrainUnchanged,
        metadata: response.metadata,
      });
    }
  }, [
    canvasSize,
    onError,
    onReport,
    response,
    selectedIndex,
    state.blocksAcross,
    state.centerX,
    state.centerZ,
    state.ecoregionLayer,
  ]);

  return (
    <div className="continentalEcoregionMap">
      <div
        ref={stageRef}
        className="terrainStage continentalEcoregionStage"
        data-testid="continental-ecoregion-stage"
        data-render-ready={response ? "true" : "false"}
        data-render-updating={updating ? "true" : "false"}
        data-semantic-sha256={response?.metadata.semanticSha256 ?? ""}
        data-retained-frame-shifted={response
          && (response.metadata.centerX !== state.centerX
            || response.metadata.centerZ !== state.centerZ
            || response.metadata.blocksAcross !== requestedAtlasViewport(
              state,
              canvasSize,
            ).blocksAcross)
          ? "true"
          : "false"}
        tabIndex={0}
        aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight"
        onPointerDown={navigation.onPointerDown}
        onPointerMove={navigation.onPointerMove}
        onPointerUp={navigation.onPointerUp}
        onPointerCancel={navigation.onPointerCancel}
        onContextMenu={(event) => event.preventDefault()}
        onKeyDown={navigation.onKeyDown}
      >
        <canvas
          ref={canvasRef}
          className="terrainCanvas"
          width={canvasSize.width}
          height={canvasSize.height}
          aria-label="Continental and ecoregional authored plan atlas"
        />
        <div className="canvasTopline" aria-hidden="true">
          <span className="canvasBadge primary">
            {response
              ? updating
                ? "updating plan · frame retained"
                : isProductionLayer(state.ecoregionLayer)
                  ? "production control ready"
                  : "continental plan ready"
              : "planning 65–131 km geography"}
          </span>
          <span className="canvasBadge">
            {isProductionLayer(state.ecoregionLayer)
              ? "field 21 · unbounded plane"
              : "candidate · production unchanged"}
          </span>
          <span className="canvasBadge">{layerLabel(state.ecoregionLayer)}</span>
          <span className="canvasBadge">
            {response
              ? `${formatDistance(response.metadata.blocksAcross)} · 1:${response.metadata.sampleStepBlocks}`
              : state.ecoregionTopology}
          </span>
        </div>
        {response && selectedIndex !== undefined ? (
          <EcoregionInspector
            response={response}
            index={selectedIndex}
            layer={state.ecoregionLayer}
          />
        ) : null}
        <div className="canvasHint">
          Drag to pan · wheel or pinch to zoom · tap a plan sample to inspect
        </div>
      </div>
      <EcoregionLegend layer={state.ecoregionLayer} response={response} />
    </div>
  );
}

function drawAtlas(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  rasterRef: React.MutableRefObject<HTMLCanvasElement | undefined>,
  rasterKeyRef: React.MutableRefObject<string>,
  layer: ContinentalEcoregionLayer,
  response: ContinentalEcoregionWorkerSummary | undefined,
  selectedIndex: number | undefined,
  viewport: AtlasViewport,
): void {
  context.setTransform(1, 0, 0, 1, 0, 0);
  context.fillStyle = "#071214";
  context.fillRect(0, 0, canvas.width, canvas.height);
  if (!response) {
    context.fillStyle = "rgba(218, 232, 222, 0.72)";
    context.font = `${13 * canvas.dpr}px "DM Mono", monospace`;
    context.fillText("Constructing direct coarse plan…", 28 * canvas.dpr, 80 * canvas.dpr);
    return;
  }
  const { columns, rows } = response.metadata;
  const raster = rasterRef.current ?? document.createElement("canvas");
  rasterRef.current = raster;
  if (raster.width !== columns || raster.height !== rows) {
    raster.width = columns;
    raster.height = rows;
  }
  const rasterContext = raster.getContext("2d");
  if (!rasterContext) {
    return;
  }
  const rasterKey = `${response.metadata.semanticSha256}:${layer}`;
  if (rasterKeyRef.current !== rasterKey) {
    const image = rasterContext.createImageData(columns, rows);
    for (let index = 0; index < response.metadata.sampleCount; index += 1) {
      const color = sampleColor(layer, response, index);
      const offset = index * 4;
      image.data[offset] = color[0];
      image.data[offset + 1] = color[1];
      image.data[offset + 2] = color[2];
      image.data[offset + 3] = 255;
    }
    rasterContext.putImageData(image, 0, 0);
    rasterKeyRef.current = rasterKey;
  }
  const destinationX = (response.metadata.minX - viewport.minX)
    / viewport.blocksAcross * canvas.width;
  const destinationY = (response.metadata.minZ - viewport.minZ)
    / viewport.blocksTall * canvas.height;
  const destinationWidth = response.metadata.blocksAcross
    / viewport.blocksAcross * canvas.width;
  const destinationHeight = response.metadata.blocksTall
    / viewport.blocksTall * canvas.height;
  context.imageSmoothingEnabled = false;
  context.drawImage(
    raster,
    destinationX,
    destinationY,
    destinationWidth,
    destinationHeight,
  );
  drawCoordinateGrid(context, canvas, viewport);
  if (selectedIndex !== undefined) {
    const column = selectedIndex % columns;
    const row = Math.floor(selectedIndex / columns);
    const worldX = response.metadata.minX
      + (column + 0.5) * response.metadata.sampleStepBlocks;
    const worldZ = response.metadata.minZ
      + (row + 0.5) * response.metadata.sampleStepBlocks;
    const x = (worldX - viewport.minX) / viewport.blocksAcross * canvas.width;
    const y = (worldZ - viewport.minZ) / viewport.blocksTall * canvas.height;
    context.strokeStyle = "rgba(255, 246, 191, 0.96)";
    context.lineWidth = Math.max(1.5, canvas.dpr * 1.5);
    context.beginPath();
    context.arc(x, y, 7 * canvas.dpr, 0, Math.PI * 2);
    context.stroke();
  }
}

function sampleColor(
  layer: ContinentalEcoregionLayer,
  response: ContinentalEcoregionWorkerSummary,
  index: number,
): [number, number, number] {
  if (isProductionLayer(layer)) {
    return productionSampleColor(layer, response, index);
  }
  const land = response.land[index]! / 65_535;
  if (land < 0.5) {
    const shelf = Math.max(0, Math.min(1,
      (response.inlandDistanceQuarterBlocks[index]! * 4 + 28_000) / 28_000,
    ));
    return mix([13, 42, 61], [35, 95, 112], shelf);
  }
  const province = palette(PROVINCE_COLORS, response.provinceKind[index]!);
  const ecoregion = palette(ECOREGION_COLORS, response.ecoregionKind[index]!);
  const openness = response.openness[index]! / 65_535;
  const forest = response.forestCore[index]! / 65_535;
  const clearing = response.clearingCore[index]! / 65_535;
  const transition = response.transition[index]! / 65_535;
  const water = response.majorWater[index]! / 65_535;
  const wetland = response.wetland[index]! / 65_535;
  const corridor = response.corridor[index]! / 65_535;
  const leeward = response.leewardExposure[index]! / 65_535;
  const aridity = response.aridity[index]! / 65_535;
  const drainage = response.drainagePermanence[index]! / 65_535;
  switch (layer) {
    case "land-ocean": {
      const inland = Math.max(0, Math.min(1,
        response.inlandDistanceQuarterBlocks[index]! * 4 / 24_000,
      ));
      return mix([169, 178, 118], [72, 105, 69], inland);
    }
    case "province":
      return province;
    case "ecoregion":
      return ecoregion;
    case "transition":
      return mix(ecoregion, [213, 184, 129], transition * 0.92);
    case "aridity": {
      let color = mix([50, 126, 117], [225, 177, 82], aridity);
      color = mix(color, [150, 103, 65], leeward * aridity * 0.58);
      return mix(color, [56, 132, 157], drainage * (1 - aridity) * 0.42);
    }
    case "openness":
      return mix([33, 71, 47], [213, 202, 103], openness);
    case "clearings": {
      const cause = palette(CLEARING_COLORS, response.clearingCause[index]!);
      return mix([30, 66, 45], cause, clearing);
    }
    case "water": {
      const strength = Math.max(water * 0.55, wetland, corridor * 0.72);
      return mix([75, 82, 64], [44, 147, 156], strength);
    }
    case "habitat":
      return mix([
        Math.round(65 + openness * 155),
        Math.round(65 + forest * 130 + wetland * 45),
        Math.round(55 + wetland * 170 + corridor * 30),
      ], palette(HABITAT_ROUTE_COLORS, response.corridorKind[index]!), corridor * 0.86);
    case "composed": {
      let color = mix(province, ecoregion, 0.68);
      color = mix(color, [36, 72, 44], forest * 0.42);
      color = mix(color, [194, 198, 100], clearing * 0.62);
      color = mix(color, [57, 145, 151], Math.max(wetland * 0.62, corridor * 0.34));
      color = mix(color, [204, 155, 76], aridity * 0.66);
      return mix(color, [223, 205, 157], transition * 0.18);
    }
  }
}

function productionSampleColor(
  layer: ProductionEcoregionLayer,
  response: ContinentalEcoregionWorkerSummary,
  index: number,
): [number, number, number] {
  const continental = response.productionLand[index]! / 65_535;
  const water = response.productionWater[index]! / 65_535;
  const biome = palette(PRODUCTION_BIOME_COLORS, response.productionBiomeKind[index]!);
  switch (layer) {
    case "production-land":
      return continental < 0.5
        ? mix([12, 48, 72], [66, 126, 139], continental * 2)
        : mix([190, 177, 116], [63, 111, 70], (continental - 0.5) * 2);
    case "production-climate": {
      const temperature = signedUnit(response.productionTemperature[index]!);
      const moisture = signedUnit(response.productionMoisture[index]!);
      const thermal = mix([93, 146, 184], [201, 151, 69], temperature);
      return mix(thermal, [48, 132, 105], moisture * 0.64);
    }
    case "production-biome":
      return biome;
    case "production-openness": {
      if (continental < 0.5) {
        return [20, 73, 101];
      }
      const forest = response.productionForestCoverage[index]! / 65_535;
      return mix([210, 196, 104], [33, 82, 52], forest);
    }
    case "production-height": {
      const height = Math.max(0, Math.min(1,
        (response.productionSurfaceY[index]! - 28) / 132,
      ));
      const lowToRock = mix([61, 111, 68], [149, 135, 105], Math.min(1, height * 1.5));
      const color = mix(lowToRock, [222, 226, 220], Math.max(0, height * 2 - 1));
      return mix(color, [34, 106, 135], water * 0.72);
    }
    case "production-water":
      return mix(
        continental < 0.5 ? [17, 66, 91] : [92, 101, 72],
        [42, 153, 169],
        water,
      );
    case "production-control": {
    const base = palette(
      PRODUCTION_BIOME_COLORS,
      response.productionBiomeKind[index]!,
    );
    const surface = Math.max(0, Math.min(1,
      (response.productionSurfaceY[index]! - 48) / 88,
    ));
    const ruggedness = signedUnit(response.productionRuggedness[index]!);
    let color = mix([31, 50, 45], base, 0.72 + surface * 0.18);
    color = mix(color, [225, 222, 181], ruggedness * 0.22);
    return mix(
      color,
      [45, 151, 167],
      response.productionWater[index]! / 65_535 * 0.62,
    );
    }
  }
}

function drawCoordinateGrid(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  viewport: AtlasViewport,
): void {
  const spacing = viewport.blocksAcross >= 100_000 ? 25_000 : 10_000;
  context.strokeStyle = "rgba(229, 240, 232, 0.13)";
  context.fillStyle = "rgba(238, 244, 240, 0.62)";
  context.lineWidth = Math.max(1, canvas.dpr * 0.75);
  context.font = `${9 * canvas.dpr}px "DM Mono", monospace`;
  const firstX = Math.ceil(viewport.minX / spacing) * spacing;
  for (let worldX = firstX; worldX < viewport.minX + viewport.blocksAcross; worldX += spacing) {
    const x = (worldX - viewport.minX) / viewport.blocksAcross * canvas.width;
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, canvas.height);
    context.stroke();
    context.fillText(`${worldX / 1_000} km`, x + 4 * canvas.dpr, 58 * canvas.dpr);
  }
  const firstZ = Math.ceil(viewport.minZ / spacing) * spacing;
  for (let worldZ = firstZ; worldZ < viewport.minZ + viewport.blocksTall; worldZ += spacing) {
    const y = (worldZ - viewport.minZ) / viewport.blocksTall * canvas.height;
    context.beginPath();
    context.moveTo(0, y);
    context.lineTo(canvas.width, y);
    context.stroke();
  }
}

function requestedAtlasViewport(
  state: TerrainLabState,
  canvas: CanvasSize,
): AtlasViewport {
  const columns = 256;
  const aspectRatio = canvas.cssWidth / Math.max(canvas.cssHeight, 1);
  const rows = Math.max(1, Math.ceil(columns / aspectRatio));
  const sampleStepBlocks = Math.max(1, Math.ceil(state.blocksAcross / columns));
  const blocksAcross = columns * sampleStepBlocks;
  const blocksTall = rows * sampleStepBlocks;
  return {
    minX: state.centerX - Math.floor(blocksAcross / 2),
    minZ: state.centerZ - Math.floor(blocksTall / 2),
    blocksAcross,
    blocksTall,
  };
}

function EcoregionInspector({
  response,
  index,
  layer,
}: {
  response: ContinentalEcoregionWorkerSummary;
  index: number;
  layer: ContinentalEcoregionLayer;
}): React.JSX.Element {
  const { metadata } = response;
  const column = index % metadata.columns;
  const row = Math.floor(index / metadata.columns);
  const worldX = metadata.minX + column * metadata.sampleStepBlocks;
  const worldZ = metadata.minZ + row * metadata.sampleStepBlocks;
  const province = labelAt(metadata.provinceKinds, response.provinceKind[index]!);
  const ecoregion = labelAt(metadata.ecoregionKinds, response.ecoregionKind[index]!);
  const transitionPeer = labelAt(
    metadata.ecoregionKinds,
    response.transitionPeerKind[index]!,
  );
  const clearing = labelAt(metadata.clearingCauses, response.clearingCause[index]!);
  const corridorKind = labelAt(
    metadata.habitatRouteKinds,
    response.corridorKind[index]!,
  );
  if (isProductionLayer(layer)) {
    const biome = PRODUCTION_BIOME_LABELS[response.productionBiomeKind[index]!]!;
    return (
      <div className="ecoregionInspector" data-testid="continental-ecoregion-inspector">
        <div>
          <span>{formatInteger(worldX)}, {formatInteger(worldZ)}</span>
          <strong>{biome}</strong>
        </div>
        <p>{metadata.productionControlRevision} · {metadata.productionControlTopology}</p>
        <dl>
          <div><dt>Surface</dt><dd>{response.productionSurfaceY[index]} m</dd></div>
          <div><dt>Continental / water</dt><dd>{encodedSignedPercent(
            response.productionLand[index]!,
          )} / {
            percent(response.productionWater[index]!)
          }</dd></div>
          <div><dt>Temperature</dt><dd>{signedPercent(
            response.productionTemperature[index]!,
          )}</dd></div>
          <div><dt>Moisture</dt><dd>{signedPercent(
            response.productionMoisture[index]!,
          )}</dd></div>
          <div><dt>Relief</dt><dd>{signedPercent(
            response.productionRelief[index]!,
          )}</dd></div>
          <div><dt>Ruggedness</dt><dd>{signedPercent(
            response.productionRuggedness[index]!,
          )}</dd></div>
        </dl>
      </div>
    );
  }
  return (
    <div className="ecoregionInspector" data-testid="continental-ecoregion-inspector">
      <div>
        <span>{formatInteger(worldX)}, {formatInteger(worldZ)}</span>
        <strong>{ecoregion ?? "ocean"}</strong>
      </div>
      <p>{province ?? "open water"}{clearing ? ` · ${clearing}` : ""}</p>
      <dl>
        <div><dt>Land / inland</dt><dd>{percent(response.land[index]!)} / {
          formatDistance(response.inlandDistanceQuarterBlocks[index]! * 4)
        }</dd></div>
        <div><dt>Open / forest</dt><dd>{percent(response.openness[index]!)} / {
          percent(response.forestCore[index]!)
        }</dd></div>
        <div><dt>Clearing / transition</dt><dd>{percent(response.clearingCore[index]!)} / {
          percent(response.transition[index]!)
        }</dd></div>
        <div><dt>Ecotone peer / width</dt><dd>{transitionPeer ?? "—"} / {
          formatDistance(response.transitionWidthBlocks[index]!)
        }</dd></div>
        <div><dt>Wetland / corridor</dt><dd>{percent(response.wetland[index]!)} / {
          percent(response.corridor[index]!)
        }</dd></div>
        <div><dt>Leeward / aridity</dt><dd>{percent(response.leewardExposure[index]!)} / {
          percent(response.aridity[index]!)
        }</dd></div>
        <div><dt>Drainage permanence</dt><dd>{percent(
          response.drainagePermanence[index]!,
        )}</dd></div>
        <div><dt>Route / ID</dt><dd>{corridorKind ?? "—"} / {
          hexId(response.corridorId[index]!)
        }</dd></div>
        <div><dt>Continent ID</dt><dd>{hexId(response.continentId[index]!)}</dd></div>
        <div><dt>Ecoregion ID</dt><dd>{hexId(response.ecoregionId[index]!)}</dd></div>
      </dl>
    </div>
  );
}

function EcoregionLegend({
  layer,
  response,
}: {
  layer: ContinentalEcoregionLayer;
  response: ContinentalEcoregionWorkerSummary | undefined;
}): React.JSX.Element {
  const entries = layer === "province"
    ? response?.metadata.provinceKinds.map((label, index) => ({
      label,
      color: PROVINCE_COLORS[index]!,
    }))
    : layer === "clearings"
      ? response?.metadata.clearingCauses.map((label, index) => ({
        label,
        color: CLEARING_COLORS[index]!,
      }))
      : layer === "habitat"
        ? response?.metadata.habitatRouteKinds.map((label, index) => ({
          label,
          color: HABITAT_ROUTE_COLORS[index]!,
        }))
      : isProductionLayer(layer)
        ? productionLegendEntries(layer)
        : response?.metadata.ecoregionKinds.map((label, index) => ({
          label,
          color: ECOREGION_COLORS[index]!,
        }));
  return (
    <div className="ecoregionLegend" data-testid="continental-ecoregion-legend">
      <strong>{layerLabel(layer)}</strong>
      <div>
        {isProductionLayer(layer)
          ? null
          : <span><i style={{ background: "rgb(21 61 82)" }} /> ocean</span>}
        {(entries ?? []).map((entry) => (
          <span key={entry.label}>
            <i style={{ background: rgb(entry.color) }} />
            {entry.label.replaceAll("-", " ")}
          </span>
        ))}
      </div>
    </div>
  );
}

function palette(
  colors: readonly (readonly [number, number, number])[],
  index: number,
): [number, number, number] {
  const color = index === NONE ? undefined : colors[index];
  return color ? [...color] : [92, 96, 79];
}

function mix(
  from: readonly [number, number, number],
  to: readonly [number, number, number],
  amount: number,
): [number, number, number] {
  const t = Math.max(0, Math.min(1, amount));
  return [0, 1, 2].map((index) =>
    Math.round(from[index]! + (to[index]! - from[index]!) * t)
  ) as [number, number, number];
}

function layerLabel(layer: ContinentalEcoregionLayer): string {
  return {
    composed: "Composed regional plan",
    "land-ocean": "Land, ocean & inland distance",
    province: "Physiographic provinces",
    ecoregion: "Ecoregion identity",
    transition: "Ecoregion transitions",
    aridity: "Rain shadow, aridity & drainage",
    openness: "Vegetation openness",
    clearings: "Planned clearings",
    water: "Water, wetland & riparian relation",
    habitat: "Habitat structure & corridors",
    "production-control": "Current production · field 21",
    "production-land": "Current production · land & ocean",
    "production-climate": "Current production · climate",
    "production-biome": "Current production · biome recipe",
    "production-openness": "Current production · forest openness",
    "production-height": "Current production · surface height",
    "production-water": "Current production · water",
  }[layer];
}

function isProductionLayer(
  layer: ContinentalEcoregionLayer,
): layer is ProductionEcoregionLayer {
  return layer.startsWith("production-");
}

function productionLegendEntries(
  layer: ProductionEcoregionLayer,
): { label: string; color: readonly [number, number, number] }[] {
  switch (layer) {
    case "production-control":
    case "production-biome":
      return PRODUCTION_BIOME_LABELS.map((label, index) => ({
        label,
        color: PRODUCTION_BIOME_COLORS[index]!,
      }));
    case "production-land":
      return [
        { label: "ocean", color: [12, 48, 72] },
        { label: "coast", color: [190, 177, 116] },
        { label: "continental interior", color: [63, 111, 70] },
      ];
    case "production-climate":
      return [
        { label: "cold", color: [93, 146, 184] },
        { label: "warm dry", color: [201, 151, 69] },
        { label: "wet", color: [48, 132, 105] },
      ];
    case "production-openness":
      return [
        { label: "open", color: [210, 196, 104] },
        { label: "forest", color: [33, 82, 52] },
      ];
    case "production-height":
      return [
        { label: "lowland", color: [61, 111, 68] },
        { label: "rock", color: [149, 135, 105] },
        { label: "alpine", color: [222, 226, 220] },
      ];
    case "production-water":
      return [
        { label: "dry land", color: [92, 101, 72] },
        { label: "water", color: [42, 153, 169] },
      ];
  }
}

function labelAt(labels: string[], index: number): string | undefined {
  return index === NONE ? undefined : labels[index];
}

function percent(value: number): string {
  return `${Math.round(value / 65_535 * 100)}%`;
}

function signedUnit(value: number): number {
  return Math.max(0, Math.min(1, value / 32_767 * 0.5 + 0.5));
}

function signedPercent(value: number): string {
  const normalized = value / 32_767;
  return `${normalized >= 0 ? "+" : "−"}${Math.round(Math.abs(normalized) * 100)}%`;
}

function encodedSignedPercent(value: number): string {
  const normalized = value / 65_535 * 2 - 1;
  return `${normalized >= 0 ? "+" : "−"}${Math.round(Math.abs(normalized) * 100)}%`;
}

function formatDistance(blocks: number): string {
  const absolute = Math.abs(blocks);
  return absolute >= 1_000
    ? `${blocks < 0 ? "−" : ""}${(absolute / 1_000).toFixed(absolute >= 10_000 ? 0 : 1)} km`
    : `${blocks} m`;
}

function formatInteger(value: number): string {
  return new Intl.NumberFormat("en-US").format(value);
}

function hexId(value: number): string {
  return value === 0 ? "—" : `…${value.toString(16).padStart(8, "0")}`;
}

function rgb(color: readonly [number, number, number]): string {
  return `rgb(${color[0]} ${color[1]} ${color[2]})`;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
