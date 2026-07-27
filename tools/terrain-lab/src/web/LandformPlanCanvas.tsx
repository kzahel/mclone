import { useEffect, useMemo, useRef, useState } from "react";

import type {
  TerrainLabCamera,
  TerrainLabState,
} from "../state";
import type {
  LandformPlanPointReceipt,
  LandformPlanSummary,
  LandformPlanWorkerResponse,
} from "./landform-plan-worker-protocol";
import { initializeTerrainLab } from "./terrain-lab-wasm";
import { useWorldViewNavigation } from "./use-world-view-navigation";

const CELL_OCEAN = 1 << 0;
const CELL_CONFLUENCE = 1 << 2;
const SINK_PROTECTED_CLOSED = 1;
const SEGMENT_DRAINAGE = 0;
const SEGMENT_DIVIDE = 1;

export interface LandformPlanReport {
  seed: string;
  schema: string;
  topology: string;
  buildMs: number;
  transferBytes: number;
  checksum: string;
  channelCells: number;
  confluences: number;
  drainageSegments: number;
  divideSegments: number;
  protectedSinks: number;
}

interface LandformPlanCanvasProps {
  state: TerrainLabState;
  camera: TerrainLabCamera;
  onStateChange: (state: TerrainLabState) => void;
  onCameraChange: (camera: TerrainLabCamera) => void;
  onReport: (report: LandformPlanReport | undefined) => void;
  onInspect: (receipt: LandformPlanPointReceipt | undefined) => void;
  onError: (error: string | undefined) => void;
}

interface CanvasSize {
  width: number;
  height: number;
  cssWidth: number;
  cssHeight: number;
  dpr: number;
}

export function LandformPlanCanvas({
  state,
  camera,
  onStateChange,
  onCameraChange,
  onReport,
  onInspect,
  onError,
}: LandformPlanCanvasProps): React.JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const workerRef = useRef<Worker | undefined>(undefined);
  const epochRef = useRef(0);
  const inspectionRevisionRef = useRef(0);
  const [summary, setSummary] = useState<LandformPlanSummary>();
  const [navigationReady, setNavigationReady] = useState(false);
  const [canvasSize, setCanvasSize] = useState<CanvasSize>({
    width: 900,
    height: 700,
    cssWidth: 900,
    cssHeight: 700,
    dpr: 1,
  });
  const [inspectionMarker, setInspectionMarker] =
    useState<{ x: number; y: number }>();
  const navigationState = useMemo<TerrainLabState>(
    () => ({ ...state, view: "map" }),
    [state],
  );
  const navigation = useWorldViewNavigation({
    stageRef,
    enabled: summary !== undefined && navigationReady,
    state: navigationState,
    camera,
    onStateChange: (next) =>
      onStateChange({
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
      const worker = workerRef.current;
      if (!stage || !worker || !summary) {
        return;
      }
      const rect = stage.getBoundingClientRect();
      const x = clientX - rect.left;
      const y = clientY - rect.top;
      const viewHeight = state.blocksAcross * rect.height / Math.max(rect.width, 1);
      const worldX = state.centerX - state.blocksAcross * 0.5
        + x / Math.max(rect.width, 1) * state.blocksAcross;
      const worldZ = state.centerZ - viewHeight * 0.5
        + y / Math.max(rect.height, 1) * viewHeight;
      inspectionRevisionRef.current += 1;
      worker.postMessage({
        type: "inspect",
        epoch: epochRef.current,
        revision: inspectionRevisionRef.current,
        worldX,
        worldZ,
      });
      setInspectionMarker({ x, y });
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
      new URL("./landform-plan-worker.ts", import.meta.url),
      { type: "module" },
    );
    workerRef.current = worker;
    worker.onmessage = (
      event: MessageEvent<LandformPlanWorkerResponse>,
    ): void => {
      const response = event.data;
      if (response.epoch !== epochRef.current) {
        return;
      }
      if (response.type === "error") {
        onError(response.message);
        return;
      }
      if (response.type === "inspection") {
        if (response.revision === inspectionRevisionRef.current) {
          onInspect(response.receipt ?? undefined);
          if (!response.receipt) {
            setInspectionMarker(undefined);
          }
        }
        return;
      }
      setSummary(response);
      onReport(reportFromSummary(response));
    };
    worker.onerror = (event): void => {
      onError(event.message || "Landform-plan Worker failed.");
    };
    return () => {
      worker.terminate();
      workerRef.current = undefined;
    };
  }, [onError, onInspect, onReport]);

  useEffect(() => {
    const worker = workerRef.current;
    if (!worker) {
      return;
    }
    epochRef.current += 1;
    setSummary(undefined);
    setInspectionMarker(undefined);
    onInspect(undefined);
    onReport(undefined);
    worker.postMessage({
      type: "build",
      epoch: epochRef.current,
      seed: state.seed,
    });
  }, [onInspect, onReport, state.seed]);

  useEffect(() => {
    setInspectionMarker(undefined);
    onInspect(undefined);
  }, [
    onInspect,
    state.blocksAcross,
    state.centerX,
    state.centerZ,
  ]);

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
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    const context = canvas.getContext("2d");
    if (!context) {
      onError("This browser does not expose a 2D canvas context.");
      return;
    }
    drawLandformPlan(context, canvasSize, state, summary);
  }, [
    canvasSize,
    onError,
    state.blocksAcross,
    state.centerX,
    state.centerZ,
    state.planBasinsVisible,
    state.planConfluencesVisible,
    state.planDividesVisible,
    state.planDrainageVisible,
    state.planQuietVisible,
    state.planSinksVisible,
    summary,
  ]);

  return (
    <div
      ref={stageRef}
      className="terrainStage landformPlanStage"
      data-testid="landform-plan-stage"
      data-render-ready={summary ? "true" : "false"}
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
        aria-label="Research landform-plan diagnostic"
      />
      {inspectionMarker ? (
        <span
          className="inspectionMarker"
          aria-hidden="true"
          style={{ left: inspectionMarker.x, top: inspectionMarker.y }}
        />
      ) : null}
      <div className="canvasTopline" aria-hidden="true">
        <span className="canvasBadge primary">
          {summary ? "plan ready" : "building plan"}
        </span>
        <span className="canvasBadge">research · 2D</span>
        <span className="canvasBadge">fixed 6.144 km domain</span>
      </div>
      <PlanLegend state={state} />
      <div className="canvasHint">
        Drag to pan · wheel or pinch to zoom · tap to inspect plan cell
      </div>
    </div>
  );
}

function PlanLegend({ state }: { state: TerrainLabState }): React.JSX.Element {
  const entries = [
    state.planBasinsVisible && ["basin ownership", "basin"],
    state.planQuietVisible && ["quiet envelope", "quiet"],
    state.planDividesVisible && ["drainage divide", "divide"],
    state.planDrainageVisible && ["drainage route", "drainage"],
    state.planConfluencesVisible && ["confluence", "confluence"],
    state.planSinksVisible && ["protected sink", "sink"],
  ].filter((entry): entry is string[] => entry !== false);
  return (
    <div className="planLegend" aria-label="Visible landform-plan facts">
      {entries.map(([label, kind]) => (
        <span key={kind}>
          <i className={`planLegendSwatch ${kind}`} aria-hidden="true" />
          {label}
        </span>
      ))}
    </div>
  );
}

function drawLandformPlan(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  state: TerrainLabState,
  summary: LandformPlanSummary | undefined,
): void {
  context.setTransform(1, 0, 0, 1, 0, 0);
  context.fillStyle = "#071012";
  context.fillRect(0, 0, canvas.width, canvas.height);
  if (!summary) {
    drawLoadingGrid(context, canvas);
    return;
  }
  const view = planView(canvas, state);
  const worldToScreen = (worldX: number, worldZ: number): [number, number] => [
    (worldX - view.minX) / view.width * canvas.width,
    (worldZ - view.minZ) / view.height * canvas.height,
  ];
  const domainMaxX = summary.minX + summary.widthCells * summary.cellBlocks;
  const domainMaxZ = summary.minZ + summary.depthCells * summary.cellBlocks;
  const firstX = clamp(
    Math.floor((view.minX - summary.minX) / summary.cellBlocks),
    0,
    summary.widthCells - 1,
  );
  const lastX = clamp(
    Math.floor((view.maxX - summary.minX) / summary.cellBlocks),
    0,
    summary.widthCells - 1,
  );
  const firstZ = clamp(
    Math.floor((view.minZ - summary.minZ) / summary.cellBlocks),
    0,
    summary.depthCells - 1,
  );
  const lastZ = clamp(
    Math.floor((view.maxZ - summary.minZ) / summary.cellBlocks),
    0,
    summary.depthCells - 1,
  );
  if (
    view.maxX > summary.minX
    && view.minX < domainMaxX
    && view.maxZ > summary.minZ
    && view.minZ < domainMaxZ
  ) {
    for (let gridZ = firstZ; gridZ <= lastZ; gridZ += 1) {
      for (let gridX = firstX; gridX <= lastX; gridX += 1) {
        const index = gridZ * summary.widthCells + gridX;
        const worldX = summary.minX + gridX * summary.cellBlocks;
        const worldZ = summary.minZ + gridZ * summary.cellBlocks;
        const [screenX, screenY] = worldToScreen(worldX, worldZ);
        const [nextX, nextY] = worldToScreen(
          worldX + summary.cellBlocks,
          worldZ + summary.cellBlocks,
        );
        context.fillStyle = cellColor(summary, index, state);
        context.fillRect(
          Math.floor(screenX),
          Math.floor(screenY),
          Math.ceil(nextX - screenX) + 1,
          Math.ceil(nextY - screenY) + 1,
        );
      }
    }
  }

  if (state.planDividesVisible || state.planDrainageVisible) {
    context.lineCap = "round";
    context.lineJoin = "round";
    for (let index = 0; index < summary.segmentKinds.length; index += 1) {
      const kind = summary.segmentKinds[index]!;
      if (
        (kind === SEGMENT_DIVIDE && !state.planDividesVisible)
        || (kind === SEGMENT_DRAINAGE && !state.planDrainageVisible)
      ) {
        continue;
      }
      const offset = index * 4;
      const aX = summary.segmentCoordinates[offset]!;
      const aZ = summary.segmentCoordinates[offset + 1]!;
      const bX = summary.segmentCoordinates[offset + 2]!;
      const bZ = summary.segmentCoordinates[offset + 3]!;
      if (
        Math.max(aX, bX) < view.minX
        || Math.min(aX, bX) > view.maxX
        || Math.max(aZ, bZ) < view.minZ
        || Math.min(aZ, bZ) > view.maxZ
      ) {
        continue;
      }
      const [startX, startY] = worldToScreen(aX, aZ);
      const [endX, endY] = worldToScreen(bX, bZ);
      context.strokeStyle = kind === SEGMENT_DRAINAGE
        ? "rgba(59, 190, 250, 0.94)"
        : "rgba(244, 197, 78, 0.86)";
      context.lineWidth = kind === SEGMENT_DRAINAGE
        ? Math.max(canvas.dpr, summary.segmentOrders[index]! * 0.9 * canvas.dpr)
        : Math.max(0.75 * canvas.dpr, 1);
      context.beginPath();
      context.moveTo(startX, startY);
      context.lineTo(endX, endY);
      context.stroke();
    }
  }

  if (state.planConfluencesVisible) {
    context.fillStyle = "#fff88a";
    const radius = Math.max(1.5, 2.2 * canvas.dpr);
    for (let index = 0; index < summary.flags.length; index += 1) {
      if (summary.flags[index]! & CELL_CONFLUENCE) {
        const gridX = index % summary.widthCells;
        const gridZ = Math.floor(index / summary.widthCells);
        const [x, y] = worldToScreen(
          summary.minX + (gridX + 0.5) * summary.cellBlocks,
          summary.minZ + (gridZ + 0.5) * summary.cellBlocks,
        );
        if (x >= 0 && x <= canvas.width && y >= 0 && y <= canvas.height) {
          context.beginPath();
          context.arc(x, y, radius, 0, Math.PI * 2);
          context.fill();
        }
      }
    }
  }

  if (state.planSinksVisible) {
    for (let index = 0; index < summary.sinkKinds.length; index += 1) {
      const [x, y] = worldToScreen(
        summary.sinkCoordinates[index * 2]!,
        summary.sinkCoordinates[index * 2 + 1]!,
      );
      if (x < 0 || x > canvas.width || y < 0 || y > canvas.height) {
        continue;
      }
      const protectedSink = summary.sinkKinds[index] === SINK_PROTECTED_CLOSED;
      context.fillStyle = protectedSink ? "#ff4bca" : "#5ad9ff";
      context.strokeStyle = "rgba(5, 14, 15, 0.88)";
      context.lineWidth = 1.5 * canvas.dpr;
      context.beginPath();
      context.arc(
        x,
        y,
        (protectedSink ? 5 : 3) * canvas.dpr,
        0,
        Math.PI * 2,
      );
      context.fill();
      context.stroke();
    }
  }

  const [domainLeft, domainTop] = worldToScreen(summary.minX, summary.minZ);
  const [domainRight, domainBottom] = worldToScreen(domainMaxX, domainMaxZ);
  context.save();
  context.strokeStyle = "rgba(226, 235, 231, 0.58)";
  context.lineWidth = Math.max(1, canvas.dpr);
  context.setLineDash([6 * canvas.dpr, 5 * canvas.dpr]);
  context.strokeRect(
    domainLeft,
    domainTop,
    domainRight - domainLeft,
    domainBottom - domainTop,
  );
  context.restore();
}

function cellColor(
  summary: LandformPlanSummary,
  index: number,
  state: TerrainLabState,
): string {
  if (summary.flags[index]! & CELL_OCEAN) {
    return "#16375e";
  }
  const base: [number, number, number] = state.planBasinsVisible
    ? basinColor(summary.basinIds[index]!)
    : [65, 75, 68];
  if (!state.planQuietVisible) {
    return `rgb(${base[0]} ${base[1]} ${base[2]})`;
  }
  const quiet = summary.quiet[index]! / 255;
  return `rgb(${
    Math.round(base[0] * (1 - quiet * 0.25))
  } ${
    Math.round(Math.min(255, base[1] * (1 + quiet * 0.18)))
  } ${
    Math.round(Math.min(255, base[2] * (1 + quiet * 0.10)))
  })`;
}

function basinColor(id: number): [number, number, number] {
  if (id === 0) {
    return [91, 108, 83];
  }
  const palette: Array<[number, number, number]> = [
    [112, 91, 107],
    [86, 117, 112],
    [125, 106, 76],
    [83, 96, 126],
    [112, 121, 82],
    [123, 84, 78],
  ];
  return palette[(id - 1) % palette.length]!;
}

function drawLoadingGrid(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
): void {
  context.strokeStyle = "rgba(183, 220, 108, 0.06)";
  context.lineWidth = Math.max(1, canvas.dpr);
  const spacing = 48 * canvas.dpr;
  for (let x = 0; x < canvas.width; x += spacing) {
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, canvas.height);
    context.stroke();
  }
  for (let y = 0; y < canvas.height; y += spacing) {
    context.beginPath();
    context.moveTo(0, y);
    context.lineTo(canvas.width, y);
    context.stroke();
  }
}

function planView(canvas: CanvasSize, state: TerrainLabState): {
  minX: number;
  minZ: number;
  maxX: number;
  maxZ: number;
  width: number;
  height: number;
} {
  const width = state.blocksAcross;
  const height = width * canvas.cssHeight / Math.max(canvas.cssWidth, 1);
  return {
    minX: state.centerX - width * 0.5,
    minZ: state.centerZ - height * 0.5,
    maxX: state.centerX + width * 0.5,
    maxZ: state.centerZ + height * 0.5,
    width,
    height,
  };
}

function reportFromSummary(summary: LandformPlanSummary): LandformPlanReport {
  return {
    seed: summary.seed,
    schema: summary.schema,
    topology: summary.topology,
    buildMs: summary.buildMs,
    transferBytes: summary.transferBytes,
    checksum: summary.checksum,
    channelCells: summary.channelCells,
    confluences: summary.confluences,
    drainageSegments: summary.drainageSegments,
    divideSegments: summary.divideSegments,
    protectedSinks: summary.protectedSinks,
  };
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value));
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
