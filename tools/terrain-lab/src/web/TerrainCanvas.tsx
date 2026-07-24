import { useEffect, useRef, useState } from "react";
import type {
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent as ReactPointerEvent,
} from "react";
import type { TerrainLab } from "../../generated/pkg/mclone_terrain_lab";
import initTerrainLab, {
  mclone_terrain_lab_create,
  terrainLabInspectPoint,
} from "../../generated/pkg/mclone_terrain_lab";

import {
  footprintBlocks,
  arrowPanTerrainLabState,
  grabPanTerrainLabStateInView,
  orbitTerrainLabCamera,
  zoomTerrainLabState,
  type TerrainLabCamera,
  type TerrainLabSource,
  type TerrainLabState,
} from "../state";

export interface TerrainLabAdapterReport {
  name: string;
  backend: string;
  deviceType: string;
  driver: string;
  driverInfo: string;
  fieldRevision: string;
  referenceSchemaRevision: string;
  gpuEvaluatorRevision: string;
}

export interface TerrainLabRenderReport {
  revision: number;
  fieldRevision: string;
  referenceSchemaRevision: string;
  gpuEvaluatorRevision: string;
  approximation: boolean;
  seed: string;
  centerX: number;
  centerZ: number;
  requestedDetail: string;
  requestedSpacing: number;
  effectiveSpacing: number;
  publishedSpacing: number;
  cpuPublishedSpacing: number;
  gpuPublishedSpacing: number;
  sampleSpacing: number;
  cellsPerAxis: number;
  samplesPerAxis: number;
  sampleCount: number;
  vertexCount: number;
  footprintBlocks: number;
  footprintChunks: number;
  viewWidthBlocks: number;
  viewHeightBlocks: number;
  levelCount: number;
  visibleTileCount: number;
  publishedTileCount: number;
  cpuPublishedTileCount: number;
  gpuPublishedTileCount: number;
  residentTileCount: number;
  queuedTileCount: number;
  cpuQueuedTileCount: number;
  gpuQueuedTileCount: number;
  pendingReadbackCount: number;
  cpuCompiledTiles: number;
  cpuCompiledTilesTotal: number;
  gpuDispatchedTiles: number;
  gpuDispatchedTilesTotal: number;
  requestCpuCompiledTiles: number;
  requestGpuDispatchedTiles: number;
  requestCacheHitTiles: number;
  evictedTilesTotal: number;
  source: string;
  view: string;
  layer: string;
  contentStage: string;
  structuredHydrologyAvailable: boolean;
  topology: string;
  width: number;
  height: number;
  cameraYaw: number;
  cameraPitch: number;
  cpuReferenceMs: number;
  encodeSubmitMs: number;
  requestMs: number;
  coarseReadyMs: number | null;
  targetReadyMs: number | null;
  cpuCoarseReadyMs: number | null;
  cpuTargetReadyMs: number | null;
  gpuCoarseReadyMs: number | null;
  gpuTargetReadyMs: number | null;
  requestCpuReferenceMs: number;
  referenceBytes: number;
  gpuSampleBytes: number;
  readbackBytes: number;
  residentBytes: number;
  comparisonPending: boolean;
  staleResultCount: number;
  coarseReady: boolean;
  targetReady: boolean;
  cpuCoarseReady: boolean;
  cpuTargetReady: boolean;
  gpuCoarseReady: boolean;
  gpuTargetReady: boolean;
  cacheEnabled: boolean;
  budgetLimited: boolean;
  needsRedraw: boolean;
  gpuExecutionTimingAvailable: boolean;
}

export interface TerrainLabComparisonReport {
  revision: number;
  sampleSpacing: number;
  tileCount: number;
  sampleCount: number;
  maxAbsoluteSurfaceError: number;
  meanAbsoluteSurfaceError: number;
  p95AbsoluteSurfaceError: number;
  waterPresenceAgreement: number;
  maxAbsoluteBaseSurfaceError: number;
  meanAbsoluteBaseSurfaceError: number;
  p95AbsoluteBaseSurfaceError: number;
  oceanWaterPresenceAgreement: number;
  meanAbsoluteContinentalnessError: number;
  meanAbsoluteReliefError: number;
  meanAbsoluteTemperatureError: number;
  meanAbsoluteMoistureError: number;
  meanAbsoluteRuggednessError: number;
  macroSurfaceMaterialAgreement: number;
  channelPresenceAgreement: number;
  meanAbsoluteRiverSignedDistanceError: number;
  meanAbsoluteChannelInfluenceError: number;
  meanAbsoluteBankInfluenceError: number;
  meanAbsoluteWetlandInfluenceError: number;
  visibleSurfaceMaterialAgreement: number;
  biomeRecipeAgreement: number;
  surfaceRecipeAgreement: number;
  staleResultCount: number;
}

export interface TerrainLabPointReceipt {
  fieldRevision: string;
  worldX: number;
  worldZ: number;
  chunkX: number;
  chunkZ: number;
  quartX: number;
  quartZ: number;
  landform: string;
  hydrology: string;
  biomeRecipe: string;
  biomeReason: string;
  surfaceRecipe: string;
  plannedStreamStart: { chunkX: number; chunkZ: number } | null;
  baseSurfaceY: number;
  surfaceY: number;
  slope: number;
  continentalness: number;
  relief: number;
  ruggedness: number;
  ridges: number;
  mountainDetail: number;
  temperature: number;
  moisture: number;
  riverSignedDistance: number;
  riverDistance: number;
  riverHalfWidth: number;
  channelInfluence: number;
  majorChannelInfluence: number;
  bankInfluence: number;
  wetlandInfluence: number;
  wetlandPoolInfluence: number;
  submergedOutletInfluence: number;
  plannedStreamInfluence: number;
  waterSurfaceY: number;
  bedY: number;
  flowX: number;
  flowZ: number;
  grade: number;
}

interface TerrainCanvasProps {
  state: TerrainLabState;
  camera: TerrainLabCamera;
  cacheEnabled: boolean;
  cacheEpoch: number;
  maxVisibleTilesPerAxis: number;
  onStateChange: (state: TerrainLabState) => void;
  onCameraChange: (camera: TerrainLabCamera) => void;
  onAdapter: (report: TerrainLabAdapterReport) => void;
  onRender: (report: TerrainLabRenderReport) => void;
  onComparison: (report: TerrainLabComparisonReport | undefined) => void;
  onInspect: (receipt: TerrainLabPointReceipt) => void;
  onError: (error: string | undefined) => void;
  onStatus: (status: "loading" | "ready" | "rendering" | "error") => void;
}

interface PointerStart {
  pointerId: number;
  clientX: number;
  clientY: number;
  camera: TerrainLabCamera;
  mode: "orbit" | "pan";
  state: TerrainLabState;
  button: number;
}

interface ActivePointer {
  clientX: number;
  clientY: number;
}

interface PinchStart {
  distance: number;
  state: TerrainLabState;
}

const AUTHORED_PACK_URL = "/first-party-packs/mclone-authored.pbp";
const FALLBACK_PACK_URL = "/first-party-packs/mclone-generated-fallback.pbp";

export function TerrainCanvas({
  state,
  camera,
  cacheEnabled,
  cacheEpoch,
  maxVisibleTilesPerAxis,
  onStateChange,
  onCameraChange,
  onAdapter,
  onRender,
  onComparison,
  onInspect,
  onError,
  onStatus,
}: TerrainCanvasProps): React.JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const labRef = useRef<TerrainLab | undefined>(undefined);
  const revisionRef = useRef(0);
  const pointerStartRef = useRef<PointerStart | undefined>(undefined);
  const activePointersRef = useRef(new Map<number, ActivePointer>());
  const pinchStartRef = useRef<PinchStart | undefined>(undefined);
  const interactionFrameRef = useRef(0);
  const orbitFrameRef = useRef(0);
  const pendingStateRef = useRef<TerrainLabState | undefined>(undefined);
  const pendingCameraRef = useRef<TerrainLabCamera | undefined>(undefined);
  const appliedCacheEpochRef = useRef(0);
  const [canvasSize, setCanvasSize] = useState({ width: 1280, height: 720 });
  const [initialized, setInitialized] = useState(false);
  const [latestReport, setLatestReport] = useState<TerrainLabRenderReport>();
  const [inspectionMarker, setInspectionMarker] = useState<{ x: number; y: number }>();

  useEffect(() => {
    setInspectionMarker(undefined);
  }, [
    camera.pitch,
    camera.yaw,
    state.blocksAcross,
    state.centerX,
    state.centerZ,
    state.seed,
    state.source,
    state.view,
  ]);

  useEffect(() => {
    let cancelled = false;
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    onStatus("loading");
    void (async () => {
      if (!("gpu" in navigator)) {
        throw new Error("This browser does not expose WebGPU.");
      }
      const [, authored, fallback] = await Promise.all([
        initTerrainLab(),
        fetchPack(AUTHORED_PACK_URL),
        fetchPack(FALLBACK_PACK_URL),
      ]);
      if (cancelled) {
        return;
      }
      const lab = await mclone_terrain_lab_create(
        canvas,
        authored,
        fallback,
      ) as TerrainLab;
      if (cancelled) {
        lab.free();
        return;
      }
      labRef.current = lab;
      onAdapter(parseJson<TerrainLabAdapterReport>(lab.adapterReport()));
      setInitialized(true);
      onStatus("ready");
    })().catch((error: unknown) => {
      if (!cancelled) {
        onStatus("error");
        onError(errorMessage(error));
      }
    });
    return () => {
      cancelled = true;
      window.cancelAnimationFrame(interactionFrameRef.current);
      window.cancelAnimationFrame(orbitFrameRef.current);
      labRef.current?.free();
      labRef.current = undefined;
    };
  }, [onAdapter, onError, onStatus]);

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) {
      return;
    }
    const resize = (): void => {
      const rect = stage.getBoundingClientRect();
      const pixelRatio = Math.min(window.devicePixelRatio || 1, 2);
      const width = Math.max(1, Math.min(2048, Math.round(rect.width * pixelRatio)));
      const height = Math.max(1, Math.min(2048, Math.round(rect.height * pixelRatio)));
      setCanvasSize((current) =>
        current.width === width && current.height === height ? current : { width, height }
      );
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(stage);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const lab = labRef.current;
    if (!initialized || !lab) {
      return;
    }
    let cancelled = false;
    let pumpFrame = 0;
    let needsRender = true;
    let comparisonComplete = state.source !== "split";
    const revision = ++revisionRef.current;
    onStatus("rendering");
    onError(undefined);
    onComparison(undefined);
    lab.resize(canvasSize.width, canvasSize.height);
    lab.setCacheEnabled(cacheEnabled);
    if (appliedCacheEpochRef.current !== cacheEpoch) {
      lab.clearCache();
      appliedCacheEpochRef.current = cacheEpoch;
    }

    const pump = (): void => {
      if (cancelled) {
        return;
      }
      try {
        if (needsRender) {
          const stage = stageRef.current;
          if (!stage) {
            return;
          }
          const rect = stage.getBoundingClientRect();
          const panel = terrainPanelSize(rect.width, rect.height, state.source);
          const report = parseJson<TerrainLabRenderReport>(
            lab.render(
              revision,
              state.seed,
              state.centerX,
              state.centerZ,
              state.blocksAcross,
              String(state.detail),
              Math.max(1, Math.round(panel.width)),
              Math.max(1, Math.round(panel.height)),
              maxVisibleTilesPerAxis,
              state.source,
              state.view,
              state.layer,
              state.contentStage,
              camera.yaw,
              camera.pitch,
            ),
          );
          setLatestReport(report);
          onRender(report);
          needsRender = report.needsRedraw;
        }
        const result = lab.pollComparison();
        if (result !== undefined) {
          const comparison = parseJson<TerrainLabComparisonReport>(result);
          if (comparison.revision === revision) {
            onComparison(comparison);
            comparisonComplete = true;
          }
        }
      } catch (error: unknown) {
        onStatus("error");
        onError(errorMessage(error));
        return;
      }
      if (needsRender || !comparisonComplete) {
        pumpFrame = window.requestAnimationFrame(pump);
      } else {
        onStatus("ready");
      }
    };
    pump();
    return () => {
      cancelled = true;
      window.cancelAnimationFrame(pumpFrame);
    };
  }, [
    canvasSize,
    cacheEnabled,
    cacheEpoch,
    camera,
    initialized,
    onComparison,
    onError,
    onRender,
    onStatus,
    maxVisibleTilesPerAxis,
    state,
  ]);

  const queueState = (next: TerrainLabState): void => {
    pendingStateRef.current = next;
    if (interactionFrameRef.current !== 0) {
      return;
    }
    interactionFrameRef.current = window.requestAnimationFrame(() => {
      interactionFrameRef.current = 0;
      const pending = pendingStateRef.current;
      if (pending) {
        pendingStateRef.current = undefined;
        onStateChange(pending);
      }
    });
  };

  const beginInteraction = (event: ReactPointerEvent<HTMLDivElement>): void => {
    if (event.button !== 0 && event.button !== 1 && event.button !== 2) {
      return;
    }
    event.preventDefault();
    event.currentTarget.focus({ preventScroll: true });
    event.currentTarget.setPointerCapture(event.pointerId);
    if (event.pointerType === "touch") {
      activePointersRef.current.set(event.pointerId, {
        clientX: event.clientX,
        clientY: event.clientY,
      });
      const pair = firstPointerPair(activePointersRef.current);
      if (pair) {
        pinchStartRef.current = {
          distance: pointerDistance(pair[0], pair[1]),
          state,
        };
        pointerStartRef.current = undefined;
        return;
      }
    }
    const orbit = state.view === "3d" && event.button === 0 && !event.shiftKey;
    pointerStartRef.current = {
      pointerId: event.pointerId,
      clientX: event.clientX,
      clientY: event.clientY,
      camera,
      mode: orbit ? "orbit" : "pan",
      state,
      button: event.button,
    };
  };

  const moveInteraction = (event: ReactPointerEvent<HTMLDivElement>): void => {
    const stage = stageRef.current;
    if (!stage) {
      return;
    }
    if (event.pointerType === "touch" && activePointersRef.current.has(event.pointerId)) {
      activePointersRef.current.set(event.pointerId, {
        clientX: event.clientX,
        clientY: event.clientY,
      });
      const pair = firstPointerPair(activePointersRef.current);
      const pinch = pinchStartRef.current;
      if (pair && pinch) {
        const distance = pointerDistance(pair[0], pair[1]);
        if (distance > 1) {
          const rect = stage.getBoundingClientRect();
          const midpointX = (pair[0].clientX + pair[1].clientX) * 0.5 - rect.left;
          const midpointY = (pair[0].clientY + pair[1].clientY) * 0.5 - rect.top;
          const panel = terrainPanelAtPointer(
            rect.width,
            rect.height,
            midpointX,
            midpointY,
            state.source,
          );
          queueState(
            zoomTerrainLabState(
              pinch.state,
              pinch.distance / distance,
              state.view === "map" ? panel.normalizedX : 0,
              state.view === "map" ? panel.normalizedZ : 0,
              panel.width / Math.max(panel.height, 1),
            ),
          );
        }
        return;
      }
    }

    const start = pointerStartRef.current;
    if (!start || start.pointerId !== event.pointerId) {
      return;
    }
    const rect = stage.getBoundingClientRect();
    if (start.mode === "pan") {
      const panel = terrainPanelSize(rect.width, rect.height, start.state.source);
      const panelAspect = panel.width / Math.max(panel.height, 1);
      queueState(
        grabPanTerrainLabStateInView(
          start.state,
          start.camera,
          event.clientX - start.clientX,
          event.clientY - start.clientY,
          panel.width,
          panel.height,
          panelAspect,
        ),
      );
      return;
    }
    pendingCameraRef.current = orbitTerrainLabCamera(
      start.camera,
      event.clientX - start.clientX,
      event.clientY - start.clientY,
      rect.width,
      rect.height,
    );
    if (orbitFrameRef.current === 0) {
      orbitFrameRef.current = window.requestAnimationFrame(() => {
        orbitFrameRef.current = 0;
        const pending = pendingCameraRef.current;
        if (pending) {
          pendingCameraRef.current = undefined;
          onCameraChange(pending);
        }
      });
    }
  };

  const finishInteraction = (event: ReactPointerEvent<HTMLDivElement>): void => {
    const start = pointerStartRef.current;
    const wasTap = start?.pointerId === event.pointerId
      && start.button === 0
      && Math.hypot(event.clientX - start.clientX, event.clientY - start.clientY) < 6
      && !pinchStartRef.current;
    if (wasTap) {
      const stage = stageRef.current;
      if (stage) {
        const rect = stage.getBoundingClientRect();
        try {
          const x = event.clientX - rect.left;
          const y = event.clientY - rect.top;
          onInspect(parseJson<TerrainLabPointReceipt>(terrainLabInspectPoint(
              state.seed,
              state.centerX,
              state.centerZ,
              state.blocksAcross,
              Math.max(1, Math.round(rect.width)),
              Math.max(1, Math.round(rect.height)),
              state.source,
              state.view,
              camera.yaw,
              camera.pitch,
              x,
              y,
            )));
          setInspectionMarker({ x, y });
        } catch (error: unknown) {
          onError(errorMessage(error));
        }
      }
    }
    activePointersRef.current.delete(event.pointerId);
    if (activePointersRef.current.size < 2) {
      pinchStartRef.current = undefined;
    }
    if (pointerStartRef.current?.pointerId === event.pointerId) {
      pointerStartRef.current = undefined;
    }
    const pendingState = pendingStateRef.current;
    if (pendingState) {
      pendingStateRef.current = undefined;
      onStateChange(pendingState);
    }
    const pendingCamera = pendingCameraRef.current;
    if (pendingCamera) {
      pendingCameraRef.current = undefined;
      onCameraChange(pendingCamera);
    }
  };

  const handleKeyDown = (
    event: ReactKeyboardEvent<HTMLDivElement>,
  ): void => {
    const next = arrowPanTerrainLabState(state, event.key);
    if (next === state) {
      return;
    }
    event.preventDefault();
    onStateChange(next);
  };

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) {
      return;
    }
    const zoomWithWheel = (event: WheelEvent): void => {
      event.preventDefault();
      const rect = stage.getBoundingClientRect();
      const panel = terrainPanelAtPointer(
        rect.width,
        rect.height,
        event.clientX - rect.left,
        event.clientY - rect.top,
        state.source,
      );
      onStateChange(
        zoomTerrainLabState(
          state,
          Math.exp(event.deltaY * 0.0015),
          state.view === "map" ? panel.normalizedX : 0,
          state.view === "map" ? panel.normalizedZ : 0,
          panel.width / Math.max(panel.height, 1),
        ),
      );
    };
    stage.addEventListener("wheel", zoomWithWheel, { passive: false });
    return () => stage.removeEventListener("wheel", zoomWithWheel);
  }, [onStateChange, state]);

  return (
    <div
      ref={stageRef}
      className={`terrainStage${state.source === "split" ? " compareMode" : ""}`}
      data-testid="terrain-stage"
      data-render-ready={initialized ? "true" : "false"}
      tabIndex={0}
      aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight"
      onPointerDown={beginInteraction}
      onPointerMove={moveInteraction}
      onPointerUp={finishInteraction}
      onPointerCancel={finishInteraction}
      onContextMenu={(event) => event.preventDefault()}
      onKeyDown={handleKeyDown}
    >
      <canvas
        ref={canvasRef}
        className="terrainCanvas"
        width={canvasSize.width}
        height={canvasSize.height}
        aria-label="Live GPU terrain preview"
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
          {latestReport?.targetReady ? "target detail" : "refining"}
        </span>
        <span className="canvasBadge">
          {latestReport && latestReport.publishedSpacing > 0
            ? `${latestReport.publishedTileCount} tiles · 1:${latestReport.publishedSpacing}`
            : "planning tiles"}
        </span>
        <span className="canvasBadge">{formatFootprint(footprintBlocks(state))} across</span>
      </div>
      {state.source === "split" ? (
        <div className="splitLabels" aria-hidden="true">
          <span>
            CPU production {state.contentStage} · {panelReadiness(
              latestReport?.cpuPublishedSpacing,
              latestReport?.cpuTargetReady,
            )}
          </span>
          <span>
            GPU production {state.contentStage} · {panelReadiness(
              latestReport?.gpuPublishedSpacing,
              latestReport?.gpuTargetReady,
            )}
          </span>
        </div>
      ) : null}
      <div className="canvasHint" aria-hidden="true">
        <span className="desktopHint">
          {state.view === "3d"
            ? "tap inspect · left drag orbit · right/shift/middle drag pan · arrows pan · wheel zoom"
            : "tap inspect · drag pan · arrows pan · wheel zoom at pointer"}
        </span>
        <span className="mobileHint">
          {state.view === "3d"
            ? "tap inspect · drag orbit · pinch zoom"
            : "tap inspect · drag pan · pinch zoom"}
        </span>
      </div>
    </div>
  );
}

function terrainPanelSize(
  width: number,
  height: number,
  source: TerrainLabSource,
): { width: number; height: number } {
  if (source !== "split") {
    return { width, height };
  }
  return width <= height ? { width, height: height / 2 } : { width: width / 2, height };
}

function terrainPanelAtPointer(
  width: number,
  height: number,
  pointerX: number,
  pointerY: number,
  source: TerrainLabSource,
): { width: number; height: number; normalizedX: number; normalizedZ: number } {
  const panel = terrainPanelSize(width, height, source);
  const localX = source === "split" && width > height
    ? pointerX % Math.max(panel.width, 1)
    : pointerX;
  const localY = source === "split" && width <= height
    ? pointerY % Math.max(panel.height, 1)
    : pointerY;
  return {
    ...panel,
    normalizedX: localX / Math.max(panel.width, 1) - 0.5,
    normalizedZ: localY / Math.max(panel.height, 1) - 0.5,
  };
}

function firstPointerPair(
  pointers: Map<number, ActivePointer>,
): [ActivePointer, ActivePointer] | undefined {
  const pair = [...pointers.values()].slice(0, 2);
  return pair.length === 2 ? [pair[0]!, pair[1]!] : undefined;
}

function pointerDistance(first: ActivePointer, second: ActivePointer): number {
  return Math.hypot(second.clientX - first.clientX, second.clientY - first.clientY);
}

async function fetchPack(url: string): Promise<Uint8Array> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to load ${url}: HTTP ${response.status}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

function panelReadiness(
  publishedSpacing: number | undefined,
  targetReady: boolean | undefined,
): string {
  if (!publishedSpacing) {
    return "waiting";
  }
  return targetReady ? `target 1:${publishedSpacing}` : `1:${publishedSpacing} · refining`;
}

function parseJson<T>(value: string): T {
  return JSON.parse(value) as T;
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

function formatFootprint(blocks: number): string {
  if (blocks >= 1_000) {
    return `${(blocks / 1_000).toLocaleString(undefined, { maximumFractionDigits: 1 })} km`;
  }
  return `${blocks.toLocaleString()} blocks`;
}
