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
  const stageRef = useRef<HTMLDivElement>(null);
  const workerRef = useRef<Worker | undefined>(undefined);
  const revisionRef = useRef(0);
  const inFlightRevisionRef = useRef<number | undefined>(undefined);
  const pendingQueryRef = useRef<ContinentalEcoregionWorkerQuery | undefined>(
    undefined,
  );
  const pendingFrameRef = useRef(0);
  const postPendingQueryRef = useRef<() => void>(() => undefined);
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
    inFlightRevisionRef.current = query.revision;
    worker.postMessage(query);
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
      if (next.type === "error") {
        onError(next.message);
      } else {
        setResponse(next);
      }
      const hasPending = pendingQueryRef.current !== undefined;
      setUpdating(hasPending);
      if (hasPending && pendingFrameRef.current === 0) {
        pendingFrameRef.current = window.requestAnimationFrame(() => {
          pendingFrameRef.current = 0;
          postPendingQueryRef.current();
        });
      }
    };
    worker.onerror = (event): void => {
      inFlightRevisionRef.current = undefined;
      setUpdating(false);
      onError(event.message || "Continental/ecoregion Worker failed.");
    };
    return () => {
      if (pendingFrameRef.current !== 0) {
        window.cancelAnimationFrame(pendingFrameRef.current);
      }
      worker.terminate();
      workerRef.current = undefined;
      inFlightRevisionRef.current = undefined;
      pendingQueryRef.current = undefined;
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
    postPendingQueryRef.current();
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
      state.ecoregionLayer,
      response,
      selectedIndex,
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
              ? updating ? "updating plan · frame retained" : "continental plan ready"
              : "planning 65–131 km geography"}
          </span>
          <span className="canvasBadge">candidate · production unchanged</span>
          <span className="canvasBadge">{layerLabel(state.ecoregionLayer)}</span>
          <span className="canvasBadge">
            {response
              ? `${formatDistance(response.metadata.blocksAcross)} · 1:${response.metadata.sampleStepBlocks}`
              : state.ecoregionTopology}
          </span>
        </div>
        {response && selectedIndex !== undefined ? (
          <EcoregionInspector response={response} index={selectedIndex} />
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
  layer: ContinentalEcoregionLayer,
  response: ContinentalEcoregionWorkerSummary | undefined,
  selectedIndex: number | undefined,
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
  context.imageSmoothingEnabled = false;
  context.drawImage(raster, 0, 0, canvas.width, canvas.height);
  drawCoordinateGrid(context, canvas, response.metadata);
  if (selectedIndex !== undefined) {
    const column = selectedIndex % columns;
    const row = Math.floor(selectedIndex / columns);
    const x = (column + 0.5) / columns * canvas.width;
    const y = (row + 0.5) / rows * canvas.height;
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
      return [
        Math.round(65 + openness * 155),
        Math.round(65 + forest * 130 + wetland * 45),
        Math.round(55 + wetland * 170 + corridor * 30),
      ];
    case "composed": {
      let color = mix(province, ecoregion, 0.68);
      color = mix(color, [36, 72, 44], forest * 0.42);
      color = mix(color, [194, 198, 100], clearing * 0.62);
      color = mix(color, [57, 145, 151], Math.max(wetland * 0.62, corridor * 0.34));
      return mix(color, [223, 205, 157], transition * 0.18);
    }
  }
}

function drawCoordinateGrid(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  metadata: ContinentalEcoregionAtlasMetadata,
): void {
  const spacing = metadata.blocksAcross >= 100_000 ? 25_000 : 10_000;
  context.strokeStyle = "rgba(229, 240, 232, 0.13)";
  context.fillStyle = "rgba(238, 244, 240, 0.62)";
  context.lineWidth = Math.max(1, canvas.dpr * 0.75);
  context.font = `${9 * canvas.dpr}px "DM Mono", monospace`;
  const firstX = Math.ceil(metadata.minX / spacing) * spacing;
  for (let worldX = firstX; worldX < metadata.minX + metadata.blocksAcross; worldX += spacing) {
    const x = (worldX - metadata.minX) / metadata.blocksAcross * canvas.width;
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, canvas.height);
    context.stroke();
    context.fillText(`${worldX / 1_000} km`, x + 4 * canvas.dpr, 58 * canvas.dpr);
  }
  const firstZ = Math.ceil(metadata.minZ / spacing) * spacing;
  for (let worldZ = firstZ; worldZ < metadata.minZ + metadata.blocksTall; worldZ += spacing) {
    const y = (worldZ - metadata.minZ) / metadata.blocksTall * canvas.height;
    context.beginPath();
    context.moveTo(0, y);
    context.lineTo(canvas.width, y);
    context.stroke();
  }
}

function EcoregionInspector({
  response,
  index,
}: {
  response: ContinentalEcoregionWorkerSummary;
  index: number;
}): React.JSX.Element {
  const { metadata } = response;
  const column = index % metadata.columns;
  const row = Math.floor(index / metadata.columns);
  const worldX = metadata.minX + column * metadata.sampleStepBlocks;
  const worldZ = metadata.minZ + row * metadata.sampleStepBlocks;
  const province = labelAt(metadata.provinceKinds, response.provinceKind[index]!);
  const ecoregion = labelAt(metadata.ecoregionKinds, response.ecoregionKind[index]!);
  const clearing = labelAt(metadata.clearingCauses, response.clearingCause[index]!);
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
        <div><dt>Wetland / corridor</dt><dd>{percent(response.wetland[index]!)} / {
          percent(response.corridor[index]!)
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
      : response?.metadata.ecoregionKinds.map((label, index) => ({
        label,
        color: ECOREGION_COLORS[index]!,
      }));
  return (
    <div className="ecoregionLegend" data-testid="continental-ecoregion-legend">
      <strong>{layerLabel(layer)}</strong>
      <div>
        <span><i style={{ background: "rgb(21 61 82)" }} /> ocean</span>
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
    openness: "Vegetation openness",
    clearings: "Planned clearings",
    water: "Water, wetland & riparian relation",
    habitat: "Habitat structure & corridors",
  }[layer];
}

function labelAt(labels: string[], index: number): string | undefined {
  return index === NONE ? undefined : labels[index];
}

function percent(value: number): string {
  return `${Math.round(value / 65_535 * 100)}%`;
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
