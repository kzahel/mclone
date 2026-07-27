import { useEffect, useMemo, useRef, useState } from "react";

import type {
  TerrainLabCamera,
  TerrainLabState,
} from "../state";
import type {
  SemanticTerrainCorrectionMetrics,
  SemanticTerrainDetailMetrics,
  SemanticTerrainGuide,
  SemanticTerrainWorkerQuery,
  SemanticTerrainWorkerResponse,
  SemanticTerrainWorkerSummary,
} from "./semantic-terrain-worker-protocol";
import {
  SEMANTIC_TERRAIN_VERTICAL_DATUM,
  semanticTerrainVerticalExaggeration,
  semanticTerrainVerticalOffset,
} from "./semantic-terrain-projection";
import { initializeTerrainLab } from "./terrain-lab-wasm";
import { useWorldViewNavigation } from "./use-world-view-navigation";

export interface SemanticTerrainReport {
  schema: string;
  suiteSha256: string;
  semanticSha256: string;
  terrainSha256: string;
  compileMs: number;
  drawMs: number;
  sampleCount: number;
  minimumHeight: number;
  maximumHeight: number;
  parent: SemanticTerrainDetailMetrics;
  regional: SemanticTerrainDetailMetrics;
  local: SemanticTerrainDetailMetrics;
  correction: SemanticTerrainCorrectionMetrics;
  researchOnly: boolean;
  productionTerrainUnchanged: boolean;
}

interface SemanticTerrainCanvasProps {
  state: TerrainLabState;
  camera: TerrainLabCamera;
  onStateChange: (state: TerrainLabState) => void;
  onCameraChange: (camera: TerrainLabCamera) => void;
  onReport: (report: SemanticTerrainReport | undefined) => void;
  onError: (error: string | undefined) => void;
}

interface CanvasSize {
  width: number;
  height: number;
  cssWidth: number;
  cssHeight: number;
  dpr: number;
}

interface Panel {
  x: number;
  y: number;
  width: number;
  height: number;
  title: string;
  subtitle: string;
}

interface SurfacePanelData {
  heights: Float32Array;
  water: Uint8Array;
  detail: "parent" | "regional" | "local";
}

export function SemanticTerrainCanvas({
  state,
  camera,
  onStateChange,
  onCameraChange,
  onReport,
  onError,
}: SemanticTerrainCanvasProps): React.JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const drawBufferRef = useRef<HTMLCanvasElement | undefined>(undefined);
  const stageRef = useRef<HTMLDivElement>(null);
  const workerRef = useRef<Worker | undefined>(undefined);
  const revisionRef = useRef(0);
  const inFlightRevisionRef = useRef<number | undefined>(undefined);
  const pendingQueryRef = useRef<SemanticTerrainWorkerQuery | undefined>(
    undefined,
  );
  const pendingFrameRef = useRef(0);
  const postPendingQueryRef = useRef<() => void>(() => undefined);
  const [response, setResponse] = useState<SemanticTerrainWorkerSummary>();
  const [updating, setUpdating] = useState(false);
  const [navigationReady, setNavigationReady] = useState(false);
  const [canvasSize, setCanvasSize] = useState<CanvasSize>({
    width: 1_200,
    height: 820,
    cssWidth: 1_200,
    cssHeight: 820,
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
  const navigation = useWorldViewNavigation({
    stageRef,
    enabled: navigationReady,
    state,
    camera,
    onStateChange,
    onCameraChange,
    resolveViewport: (clientX, clientY) => {
      const stage = stageRef.current;
      if (!stage) {
        return undefined;
      }
      const rect = stage.getBoundingClientRect();
      const panels = semanticPanels(rect.width, rect.height);
      const localX = clientX - rect.left;
      const localY = clientY - rect.top;
      const panel = panels.find((candidate) =>
        localX >= candidate.x
        && localX <= candidate.x + candidate.width
        && localY >= candidate.y
        && localY <= candidate.y + candidate.height
      ) ?? panels[0]!;
      return {
        x: localX - panel.x,
        y: localY - panel.y,
        width: panel.width,
        height: panel.height,
      };
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
      new URL("./semantic-terrain-worker.ts", import.meta.url),
      { type: "module" },
    );
    workerRef.current = worker;
    worker.onmessage = (
      event: MessageEvent<SemanticTerrainWorkerResponse>,
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
      const hasPendingQuery = pendingQueryRef.current !== undefined;
      setUpdating(hasPendingQuery);
      if (hasPendingQuery && pendingFrameRef.current === 0) {
        pendingFrameRef.current = window.requestAnimationFrame(() => {
          pendingFrameRef.current = 0;
          postPendingQueryRef.current();
        });
      }
    };
    worker.onerror = (event): void => {
      inFlightRevisionRef.current = undefined;
      setUpdating(false);
      onError(event.message || "Semantic terrain Worker failed.");
    };
    return () => {
      if (pendingFrameRef.current !== 0) {
        window.cancelAnimationFrame(pendingFrameRef.current);
        pendingFrameRef.current = 0;
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

  const panelAspect = useMemo(() => {
    const panel = semanticPanels(canvasSize.cssWidth, canvasSize.cssHeight)[0]!;
    return panel.width / Math.max(panel.height, 1);
  }, [canvasSize.cssHeight, canvasSize.cssWidth]);

  useEffect(() => {
    const worker = workerRef.current;
    if (!worker) {
      return;
    }
    revisionRef.current += 1;
    const revision = revisionRef.current;
    pendingQueryRef.current = {
      type: "query",
      revision,
      seed: state.seed,
      topology: state.semanticTopology,
      substrate: state.semanticSubstrate,
      features: state.semanticFeatures,
      correction: state.semanticCorrection,
      centerX: state.centerX,
      centerZ: state.centerZ,
      blocksAcross: state.blocksAcross,
      aspectRatio: panelAspect,
      samplesAcross: 65,
    };
    setUpdating(true);
    postPendingQueryRef.current();
  }, [
    panelAspect,
    state.blocksAcross,
    state.centerX,
    state.centerZ,
    state.seed,
    state.semanticCorrection,
    state.semanticFeatures,
    state.semanticSubstrate,
    state.semanticTopology,
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
    const drawBuffer = drawBufferRef.current ?? document.createElement("canvas");
    drawBufferRef.current = drawBuffer;
    if (
      drawBuffer.width !== canvasSize.width
      || drawBuffer.height !== canvasSize.height
    ) {
      drawBuffer.width = canvasSize.width;
      drawBuffer.height = canvasSize.height;
    }
    const drawContext = drawBuffer.getContext("2d");
    if (!drawContext) {
      onError("This browser cannot allocate a terrain drawing buffer.");
      return;
    }
    const started = performance.now();
    drawSemanticTerrain(drawContext, canvasSize, state, camera, response);
    context.setTransform(1, 0, 0, 1, 0, 0);
    context.drawImage(drawBuffer, 0, 0);
    if (response) {
      const correction = state.semanticCorrection === "regional"
        ? response.metadata.regionalCorrection
        : response.metadata.localCorrection;
      onReport({
        schema: response.metadata.receiptSchema,
        suiteSha256: response.suiteSha256,
        semanticSha256: response.metadata.semanticSha256,
        terrainSha256: response.metadata.terrainSha256,
        compileMs: response.compileMs,
        drawMs: performance.now() - started,
        sampleCount: response.metadata.sampleCount,
        minimumHeight: response.metadata.minimumHeight,
        maximumHeight: response.metadata.maximumHeight,
        parent: response.metadata.parent,
        regional: response.metadata.regional,
        local: response.metadata.local,
        correction,
        researchOnly: response.metadata.researchOnly,
        productionTerrainUnchanged:
          response.metadata.productionTerrainUnchanged,
      });
    }
  }, [
    camera,
    canvasSize,
    onError,
    onReport,
    response,
    state.semanticCorrection,
    state.semanticGuidesVisible,
    state.semanticVerticalScale,
    state.view,
  ]);

  const verticalExaggeration = semanticTerrainVerticalExaggeration(
    state.semanticVerticalScale,
  );
  return (
    <div
      ref={stageRef}
      className="terrainStage semanticTerrainStage"
      data-testid="semantic-terrain-stage"
      data-render-ready={response ? "true" : "false"}
      data-render-updating={updating ? "true" : "false"}
      data-vertical-datum={SEMANTIC_TERRAIN_VERTICAL_DATUM}
      data-vertical-exaggeration={verticalExaggeration}
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
        aria-label="Semantic terrain parent, regional, local, and correction comparison"
      />
      <div className="canvasTopline" aria-hidden="true">
        <span className="canvasBadge primary">
          {response
            ? updating ? "updating terrain · frame retained" : "semantic terrain ready"
            : "reconstructing terrain"}
        </span>
        <span className="canvasBadge">research · production disconnected</span>
        <span className="canvasBadge">{state.semanticTopology}</span>
        {state.view === "3d" ? (
          <span className="canvasBadge">
            {verticalExaggeration === 1
              ? "physical world scale · 1× Y"
              : `world scale · ${verticalExaggeration}× Y exaggeration`}
          </span>
        ) : null}
      </div>
      <div className="canvasHint">
        Drag to {state.view === "3d" ? "orbit" : "pan"} · shift-drag to pan ·
        wheel or pinch to zoom all four views
      </div>
    </div>
  );
}

function drawSemanticTerrain(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  state: TerrainLabState,
  camera: TerrainLabCamera,
  response: SemanticTerrainWorkerSummary | undefined,
): void {
  context.setTransform(1, 0, 0, 1, 0, 0);
  context.fillStyle = "#061012";
  context.fillRect(0, 0, canvas.width, canvas.height);
  const panels = semanticPanels(canvas.width, canvas.height, canvas.cssWidth);
  if (!response) {
    for (const panel of panels) {
      drawLoadingPanel(context, panel, canvas.dpr);
    }
    return;
  }
  const verticalExaggeration = semanticTerrainVerticalExaggeration(
    state.semanticVerticalScale,
  );
  const surfaces: SurfacePanelData[] = [
    {
      heights: response.parentHeights,
      water: response.parentWater,
      detail: "parent",
    },
    {
      heights: response.regionalHeights,
      water: response.regionalWater,
      detail: "regional",
    },
    {
      heights: response.localHeights,
      water: response.localWater,
      detail: "local",
    },
  ];
  for (let index = 0; index < surfaces.length; index += 1) {
    const panel = panels[index]!;
    context.save();
    context.beginPath();
    context.rect(panel.x, panel.y, panel.width, panel.height);
    context.clip();
    if (state.view === "map") {
      drawMapSurface(context, panel, response, surfaces[index]!);
    } else {
      drawThreeDimensionalSurface(
        context,
        panel,
        response,
        surfaces[index]!,
        camera,
        verticalExaggeration,
      );
    }
    if (state.semanticGuidesVisible) {
      drawGuides(
        context,
        panel,
        response,
        surfaces[index]!,
        camera,
        state.view,
        verticalExaggeration,
      );
    }
    context.restore();
    drawPanelFrame(context, panel, canvas.dpr);
  }
  const correction = state.semanticCorrection === "regional"
    ? response.regionalCorrection
    : response.localCorrection;
  const correctionMetrics = state.semanticCorrection === "regional"
    ? response.metadata.regionalCorrection
    : response.metadata.localCorrection;
  drawCorrection(
    context,
    panels[3]!,
    response,
    correction,
    correctionMetrics,
  );
  drawPanelFrame(context, panels[3]!, canvas.dpr);
}

function semanticPanels(
  width: number,
  height: number,
  responsiveWidth = width,
): Panel[] {
  const gap = Math.max(2, Math.round(Math.min(width, height) * 0.004));
  const labels = [
    ["A · Parent terrain", "one broad segment per feature"],
    ["B · Regional terrain", "two replacing child segments"],
    ["C · Local terrain", "four replacing child segments"],
    ["D · Refinement correction", "finer minus its immediate parent"],
  ] as const;
  if (responsiveWidth < 720) {
    const panelHeight = (height - gap * 3) / 4;
    return labels.map(([title, subtitle], index) => ({
      x: 0,
      y: index * (panelHeight + gap),
      width,
      height: panelHeight,
      title,
      subtitle,
    }));
  }
  const panelWidth = (width - gap) / 2;
  const panelHeight = (height - gap) / 2;
  return labels.map(([title, subtitle], index) => ({
    x: (index % 2) * (panelWidth + gap),
    y: Math.floor(index / 2) * (panelHeight + gap),
    width: panelWidth,
    height: panelHeight,
    title,
    subtitle,
  }));
}

function drawMapSurface(
  context: CanvasRenderingContext2D,
  panel: Panel,
  response: SemanticTerrainWorkerSummary,
  data: SurfacePanelData,
): void {
  const { columns, rows, minimumHeight, maximumHeight } = response.metadata;
  const cellWidth = panel.width / Math.max(columns - 1, 1);
  const cellHeight = panel.height / Math.max(rows - 1, 1);
  context.fillStyle = "#081416";
  context.fillRect(panel.x, panel.y, panel.width, panel.height);
  for (let row = 0; row < rows - 1; row += 1) {
    for (let column = 0; column < columns - 1; column += 1) {
      const index = row * columns + column;
      const height = data.heights[index]!;
      context.fillStyle = terrainColor(
        height,
        minimumHeight,
        maximumHeight,
        data.water[index] !== 0,
        hillshade(data.heights, columns, rows, column, row),
      );
      context.fillRect(
        panel.x + column * cellWidth,
        panel.y + row * cellHeight,
        Math.ceil(cellWidth) + 1,
        Math.ceil(cellHeight) + 1,
      );
    }
  }
}

function drawThreeDimensionalSurface(
  context: CanvasRenderingContext2D,
  panel: Panel,
  response: SemanticTerrainWorkerSummary,
  data: SurfacePanelData,
  camera: TerrainLabCamera,
  verticalExaggeration: number,
): void {
  const { columns, rows, minimumHeight, maximumHeight } = response.metadata;
  context.fillStyle = "#071113";
  context.fillRect(panel.x, panel.y, panel.width, panel.height);
  const cells: Array<{ column: number; row: number; depth: number }> = [];
  const cosYaw = Math.cos(camera.yaw);
  const sinYaw = Math.sin(camera.yaw);
  for (let row = 0; row < rows - 1; row += 1) {
    for (let column = 0; column < columns - 1; column += 1) {
      const normalizedX = (column + 0.5) / (columns - 1) * 2 - 1;
      const normalizedZ = (row + 0.5) / (rows - 1) * 2 - 1;
      cells.push({
        column,
        row,
        depth: normalizedX * sinYaw + normalizedZ * cosYaw,
      });
    }
  }
  cells.sort((left, right) => left.depth - right.depth);
  for (const cell of cells) {
    const indices = [
      cell.row * columns + cell.column,
      cell.row * columns + cell.column + 1,
      (cell.row + 1) * columns + cell.column + 1,
      (cell.row + 1) * columns + cell.column,
    ];
    const points = indices.map((index) =>
      projectGridPoint(
        panel,
        response,
        camera,
        index % columns,
        Math.floor(index / columns),
        data.heights[index]!,
        verticalExaggeration,
      )
    );
    const height = indices.reduce((sum, index) => sum + data.heights[index]!, 0) / 4;
    const water = indices.some((index) => data.water[index] !== 0);
    context.fillStyle = terrainColor(
      height,
      minimumHeight,
      maximumHeight,
      water,
      hillshade(data.heights, columns, rows, cell.column, cell.row),
    );
    context.beginPath();
    context.moveTo(points[0]!.x, points[0]!.y);
    for (let index = 1; index < points.length; index += 1) {
      context.lineTo(points[index]!.x, points[index]!.y);
    }
    context.closePath();
    context.fill();
  }
}

function projectGridPoint(
  panel: Panel,
  response: SemanticTerrainWorkerSummary,
  camera: TerrainLabCamera,
  column: number,
  row: number,
  height: number,
  verticalExaggeration: number,
): { x: number; y: number } {
  const { columns, rows } = response.metadata;
  const normalizedX = column / Math.max(columns - 1, 1) * 2 - 1;
  const normalizedZ = row / Math.max(rows - 1, 1) * 2 - 1;
  const cosYaw = Math.cos(camera.yaw);
  const sinYaw = Math.sin(camera.yaw);
  const rotatedX = normalizedX * cosYaw - normalizedZ * sinYaw;
  const rotatedZ = normalizedX * sinYaw + normalizedZ * cosYaw;
  const scale = Math.min(panel.width * 0.4, panel.height * 0.42);
  return {
    x: panel.x + panel.width * 0.5 + rotatedX * scale,
    y: panel.y + panel.height * 0.56
      + rotatedZ * scale * Math.sin(camera.pitch)
      - semanticTerrainVerticalOffset(
        height,
        camera.pitch,
        scale,
        response.metadata.blocksAcross,
        verticalExaggeration,
      ),
  };
}

function drawCorrection(
  context: CanvasRenderingContext2D,
  panel: Panel,
  response: SemanticTerrainWorkerSummary,
  values: Float32Array,
  metrics: SemanticTerrainCorrectionMetrics,
): void {
  const { columns, rows } = response.metadata;
  const cellWidth = panel.width / Math.max(columns - 1, 1);
  const cellHeight = panel.height / Math.max(rows - 1, 1);
  const scale = Math.max(Math.abs(metrics.minimum), Math.abs(metrics.maximum), 0.001);
  context.fillStyle = "#0a1416";
  context.fillRect(panel.x, panel.y, panel.width, panel.height);
  for (let row = 0; row < rows - 1; row += 1) {
    for (let column = 0; column < columns - 1; column += 1) {
      const value = values[row * columns + column]! / scale;
      const strength = Math.min(Math.abs(value), 1);
      context.fillStyle = value < 0
        ? `rgb(${Math.round(18 + strength * 24)} ${Math.round(47 + strength * 90)} ${
          Math.round(62 + strength * 146)
        })`
        : `rgb(${Math.round(24 + strength * 216)} ${Math.round(39 + strength * 116)} ${
          Math.round(42 + strength * 38)
        })`;
      context.fillRect(
        panel.x + column * cellWidth,
        panel.y + row * cellHeight,
        Math.ceil(cellWidth) + 1,
        Math.ceil(cellHeight) + 1,
      );
    }
  }
  context.fillStyle = "rgba(5, 13, 14, 0.78)";
  context.fillRect(
    panel.x + 12,
    panel.y + panel.height - 50,
    Math.min(panel.width - 24, 310),
    28,
  );
  context.fillStyle = "#cfded6";
  context.font = `${Math.max(10, panel.width * 0.018)}px "DM Mono", monospace`;
  context.fillText(
    `−${scale.toFixed(2)}  ·  0  ·  +${scale.toFixed(2)} blocks`,
    panel.x + 22,
    panel.y + panel.height - 31,
  );
}

function drawGuides(
  context: CanvasRenderingContext2D,
  panel: Panel,
  response: SemanticTerrainWorkerSummary,
  data: SurfacePanelData,
  camera: TerrainLabCamera,
  view: TerrainLabState["view"],
  verticalExaggeration: number,
): void {
  const guides = response.metadata.guides.filter((guide) =>
    guide.detail === data.detail
  );
  for (const guide of guides) {
    const start = guidePoint(
      panel,
      response,
      data,
      camera,
      guide,
      true,
      view,
      verticalExaggeration,
    );
    const end = guidePoint(
      panel,
      response,
      data,
      camera,
      guide,
      false,
      view,
      verticalExaggeration,
    );
    context.strokeStyle = guide.family === "range-axis"
      ? "rgba(255, 205, 95, 0.86)"
      : "rgba(97, 211, 255, 0.9)";
    context.lineWidth = Math.max(1.5, panel.width * 0.003);
    context.setLineDash(data.detail === "parent"
      ? []
      : data.detail === "regional" ? [8, 5] : [3, 4]);
    context.beginPath();
    context.moveTo(start.x, start.y);
    context.lineTo(end.x, end.y);
    context.stroke();
  }
  context.setLineDash([]);
}

function guidePoint(
  panel: Panel,
  response: SemanticTerrainWorkerSummary,
  data: SurfacePanelData,
  camera: TerrainLabCamera,
  guide: SemanticTerrainGuide,
  start: boolean,
  view: TerrainLabState["view"],
  verticalExaggeration: number,
): { x: number; y: number } {
  const worldX = start ? guide.startX : guide.endX;
  const worldZ = start ? guide.startZ : guide.endZ;
  const column = Math.max(
    0,
    Math.min(
      response.metadata.columns - 1,
      Math.round(
        (worldX - (response.metadata.centerX - response.metadata.blocksAcross * 0.5))
        / response.metadata.blocksAcross
        * (response.metadata.columns - 1),
      ),
    ),
  );
  const row = Math.max(
    0,
    Math.min(
      response.metadata.rows - 1,
      Math.round(
        (worldZ - (response.metadata.centerZ - response.metadata.blocksTall * 0.5))
        / response.metadata.blocksTall
        * (response.metadata.rows - 1),
      ),
    ),
  );
  if (view === "map") {
    return {
      x: panel.x + column / (response.metadata.columns - 1) * panel.width,
      y: panel.y + row / (response.metadata.rows - 1) * panel.height,
    };
  }
  return projectGridPoint(
    panel,
    response,
    camera,
    column,
    row,
    data.heights[row * response.metadata.columns + column]!,
    verticalExaggeration,
  );
}

function hillshade(
  heights: Float32Array,
  columns: number,
  rows: number,
  column: number,
  row: number,
): number {
  const left = heights[row * columns + Math.max(column - 1, 0)]!;
  const right = heights[row * columns + Math.min(column + 1, columns - 1)]!;
  const top = heights[Math.max(row - 1, 0) * columns + column]!;
  const bottom = heights[Math.min(row + 1, rows - 1) * columns + column]!;
  return Math.max(-1, Math.min(1, (left - right + top - bottom) * 0.035));
}

function terrainColor(
  height: number,
  minimum: number,
  maximum: number,
  water: boolean,
  shade: number,
): string {
  if (water) {
    const lightness = Math.round(35 + shade * 10);
    return `hsl(198 58% ${lightness}%)`;
  }
  const normalized = (height - minimum) / Math.max(maximum - minimum, 1);
  const hue = 95 - normalized * 62;
  const saturation = 25 + normalized * 24;
  const lightness = 28 + normalized * 35 + shade * 12;
  return `hsl(${hue} ${saturation}% ${lightness}%)`;
}

function drawPanelFrame(
  context: CanvasRenderingContext2D,
  panel: Panel,
  dpr: number,
): void {
  const labelHeight = 46 * dpr;
  context.fillStyle = "rgba(4, 12, 13, 0.84)";
  context.fillRect(panel.x, panel.y, panel.width, labelHeight);
  context.fillStyle = "#ecf3ef";
  context.font = `600 ${12 * dpr}px "DM Mono", monospace`;
  context.fillText(panel.title, panel.x + 14 * dpr, panel.y + 19 * dpr);
  context.fillStyle = "rgba(207, 222, 214, 0.65)";
  context.font = `${9 * dpr}px "DM Mono", monospace`;
  context.fillText(panel.subtitle, panel.x + 14 * dpr, panel.y + 35 * dpr);
  context.strokeStyle = "rgba(190, 218, 205, 0.18)";
  context.lineWidth = dpr;
  context.strokeRect(panel.x + 0.5, panel.y + 0.5, panel.width - 1, panel.height - 1);
}

function drawLoadingPanel(
  context: CanvasRenderingContext2D,
  panel: Panel,
  dpr: number,
): void {
  context.fillStyle = "#071113";
  context.fillRect(panel.x, panel.y, panel.width, panel.height);
  context.fillStyle = "rgba(207, 222, 214, 0.46)";
  context.font = `${11 * dpr}px "DM Mono", monospace`;
  context.fillText("reconstructing…", panel.x + 18 * dpr, panel.y + 72 * dpr);
  drawPanelFrame(context, panel, dpr);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
