import { useEffect, useRef, useState } from "react";
import type { TerrainLab } from "../../generated/pkg/mclone_terrain_lab";
import {
  mclone_terrain_lab_create,
  terrainLabInspectPoint,
} from "../../generated/pkg/mclone_terrain_lab";

import {
  footprintBlocks,
  type TerrainLabCamera,
  type TerrainLabSource,
  type TerrainLabState,
} from "../state";
import type {
  LodWorkerResponse,
  LodWorkerResult,
} from "./lod-worker-protocol";
import { initializeTerrainLab } from "./terrain-lab-wasm";
import {
  loadTerrainVisualAssets,
  optionalReferenceBytes,
} from "./visual-assets";
import { terrainCanvasBackingSize } from "./canvas-size";
import { useWorldViewNavigation } from "./use-world-view-navigation";

export interface TerrainLabAdapterReport {
  name: string;
  backend: string;
  deviceType: string;
  driver: string;
  driverInfo: string;
  fieldRevision: string;
  referenceSchemaRevision: string;
  gpuEvaluatorRevision: string;
  macroEvaluatorRevision: string;
}

export interface TerrainLabRenderReport {
  revision: number;
  profile: string;
  fieldRevision: string;
  referenceSchemaRevision: string;
  gpuEvaluatorRevision: string;
  macroEvaluatorRevision: string;
  vegetationRevision: string;
  approximation: boolean;
  seed: string;
  centerX: number;
  centerZ: number;
  requestedDetail: string;
  surfaceQuality: string;
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
  vegetationSummaryTileCount: number;
  cpuVegetationSummaryTileCount: number;
  gpuVegetationSummaryTileCount: number;
  vegetationRecordTileCount: number;
  vegetationAggregatedTileCount: number;
  treeInstanceCount: number;
  treeInstanceBytes: number;
  treeProxyVertexCount: number;
  vegetationCellRequests: number;
  vegetationCellHits: number;
  vegetationCellMisses: number;
  retainedVegetationCells: number;
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
  macroCompiledTilesTotal: number;
  requestCpuCompiledTiles: number;
  requestGpuDispatchedTiles: number;
  requestMacroCompiledTiles: number;
  requestCacheHitTiles: number;
  requestCpuSampleLatticePoints: number;
  requestCpuTerrainSampleEvaluations: number;
  requestCpuForestIntentEvaluations: number;
  requestCpuForestFootprintSummaries: number;
  requestGpuSampleLatticePoints: number;
  requestGpuTerrainSampleEvaluations: number;
  requestGpuForestIntentEvaluations: number;
  requestGpuForestFootprintSummaries: number;
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
  cpuVegetationMs: number;
  cpuPackUploadMs: number;
  encodeSubmitMs: number;
  requestMs: number;
  coarseReadyMs: number | null;
  targetReadyMs: number | null;
  cpuCoarseReadyMs: number | null;
  cpuTargetReadyMs: number | null;
  gpuCoarseReadyMs: number | null;
  gpuTargetReadyMs: number | null;
  requestCpuReferenceMs: number;
  requestCpuVegetationMs: number;
  requestCpuPackUploadMs: number;
  requestMacroCompileMs: number;
  referenceBytes: number;
  gpuSampleBytes: number;
  readbackBytes: number;
  requestReadbackBytes: number;
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
  maxAbsoluteDisplayError: number;
  meanAbsoluteDisplayError: number;
  p95AbsoluteDisplayError: number;
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
  landformKindAgreement: number;
  biomeRecipeAgreement: number;
  surfaceRecipeAgreement: number;
  staleResultCount: number;
}

export interface TerrainLabPointReceipt {
  fieldRevision: string;
  previewSchemaRevision: string;
  gpuEvaluatorRevision: string;
  decorationRevision: string;
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
  carveDelta: number;
  slope: number;
  mountainStrength: number;
  exposure: number;
  continentalness: number;
  relief: number;
  ruggedness: number;
  ridges: number;
  mountainDetail: number;
  temperature: number;
  adjustedTemperature: number;
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
  splitLayout: "columns" | "rows";
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

interface TerrainLabExternalCpuTileRequest {
  revision: number;
  profile: "overworld";
  seed: string;
  tileX: number;
  tileZ: number;
  sampleSpacing: number;
  surfaceQuality: "basic" | "inferred";
}

interface WorkerBackedTerrainLab extends TerrainLab {
  nextCpuTileRequest(): string | undefined;
  acceptCpuTile(
    revision: number,
    seed: string,
    tileX: number,
    tileZ: number,
    sampleSpacing: number,
    surfaceQuality: string,
    samples: Float32Array,
    compileMs: number,
  ): boolean;
  rejectCpuTile(
    revision: number,
    seed: string,
    tileX: number,
    tileZ: number,
    sampleSpacing: number,
    surfaceQuality: string,
  ): void;
  nextMacroTileRequest(): string | undefined;
  acceptMacroTile(
    revision: number,
    seed: string,
    tileX: number,
    tileZ: number,
    sampleSpacing: number,
    surfaceQuality: string,
    samples: Float32Array,
    compileMs: number,
  ): boolean;
  rejectMacroTile(
    revision: number,
    seed: string,
    tileX: number,
    tileZ: number,
    sampleSpacing: number,
    surfaceQuality: string,
  ): void;
}

export function TerrainCanvas({
  state,
  camera,
  splitLayout,
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
  const labRef = useRef<WorkerBackedTerrainLab | undefined>(undefined);
  const revisionRef = useRef(0);
  const appliedCacheEpochRef = useRef(0);
  const [canvasSize, setCanvasSize] = useState({ width: 1280, height: 720 });
  const [initialized, setInitialized] = useState(false);
  const [latestReport, setLatestReport] = useState<TerrainLabRenderReport>();
  const [inspectionMarker, setInspectionMarker] = useState<{ x: number; y: number }>();
  const navigation = useWorldViewNavigation({
    stageRef,
    enabled: initialized,
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
      const panel = terrainPanelAtPointer(
        rect.width,
        rect.height,
        clientX - rect.left,
        clientY - rect.top,
        state.source,
        splitLayout,
      );
      return {
        x: panel.localX,
        y: panel.localY,
        width: panel.width,
        height: panel.height,
      };
    },
    onTap: (clientX, clientY) => {
      if (state.profile === "overworld") {
        return;
      }
      const stage = stageRef.current;
      if (!stage) {
        return;
      }
      const rect = stage.getBoundingClientRect();
      try {
        const x = clientX - rect.left;
        const y = clientY - rect.top;
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
          state.projection,
          splitLayout,
          x,
          y,
        )));
        setInspectionMarker({ x, y });
      } catch (error: unknown) {
        onError(errorMessage(error));
      }
    },
  });

  useEffect(() => {
    setInspectionMarker(undefined);
  }, [
    camera.pitch,
    camera.yaw,
    state.blocksAcross,
    state.centerX,
    state.centerZ,
    state.profile,
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
    setInitialized(false);
    setLatestReport(undefined);
    void (async () => {
      if (!("gpu" in navigator)) {
        throw new Error("This browser does not expose WebGPU.");
      }
      const [, assets] = await Promise.all([
        initializeTerrainLab(),
        loadTerrainVisualAssets(),
      ]);
      if (cancelled) {
        return;
      }
      const lab = await mclone_terrain_lab_create(
        canvas,
        assets.authored,
        optionalReferenceBytes(assets),
        assets.provisional,
        assets.diagnostic,
        state.visualProfile,
        state.texturePresentation,
      ) as WorkerBackedTerrainLab;
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
      labRef.current?.free();
      labRef.current = undefined;
    };
  }, [
    onAdapter,
    onError,
    onStatus,
    state.texturePresentation,
    state.visualProfile,
  ]);

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) {
      return;
    }
    const resize = (): void => {
      const rect = stage.getBoundingClientRect();
      const { width, height } = terrainCanvasBackingSize(
        rect.width,
        rect.height,
        window.devicePixelRatio,
      );
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
    let exactWorkerReady = false;
    let exactWorkerInFlight = false;
    let macroWorkerReady = false;
    let macroWorkerInFlight = false;
    const exactWorker = state.profile === "overworld"
      ? new Worker(new URL("./lod-worker.ts", import.meta.url), {
        type: "module",
        name: `vanilla-terrain-exact-${revision}`,
      })
      : undefined;
    const macroWorker = state.profile === "overworld"
      ? new Worker(new URL("./lod-worker.ts", import.meta.url), {
        type: "module",
        name: `vanilla-terrain-macro-${revision}`,
      })
      : undefined;
    onStatus("rendering");
    onError(undefined);
    onComparison(undefined);
    lab.resize(canvasSize.width, canvasSize.height);
    lab.setCacheEnabled(cacheEnabled);
    if (appliedCacheEpochRef.current !== cacheEpoch) {
      lab.clearCache();
      appliedCacheEpochRef.current = cacheEpoch;
    }

    const schedulePump = (): void => {
      if (cancelled || pumpFrame !== 0) {
        return;
      }
      pumpFrame = window.requestAnimationFrame(() => {
        pumpFrame = 0;
        pump();
      });
    };

    if (exactWorker) {
      exactWorker.onmessage = (event: MessageEvent<LodWorkerResponse>): void => {
        const response = event.data;
        if (cancelled || response.epoch !== revision) {
          return;
        }
        if (response.type === "ready") {
          exactWorkerReady = true;
          schedulePump();
          return;
        }
        exactWorkerInFlight = false;
        if (response.type === "error") {
          rejectWorkerTile(lab, "exact", response);
          onStatus("error");
          onError(response.message);
          return;
        }
        acceptWorkerTile(lab, "exact", response);
        needsRender = true;
        schedulePump();
      };
      exactWorker.onerror = (event): void => {
        exactWorkerInFlight = false;
        onStatus("error");
        onError(event.message || "Vanilla sampled-exact worker failed.");
      };
      exactWorker.postMessage({
        type: "init",
        mode: "exact",
        epoch: revision,
        seed: state.seed,
      });
    }
    if (macroWorker) {
      macroWorker.onmessage = (event: MessageEvent<LodWorkerResponse>): void => {
        const response = event.data;
        if (cancelled || response.epoch !== revision) {
          return;
        }
        if (response.type === "ready") {
          macroWorkerReady = true;
          schedulePump();
          return;
        }
        macroWorkerInFlight = false;
        if (response.type === "error") {
          rejectWorkerTile(lab, "macro", response);
          onStatus("error");
          onError(response.message);
          return;
        }
        acceptWorkerTile(lab, "macro", response);
        needsRender = true;
        schedulePump();
      };
      macroWorker.onerror = (event): void => {
        macroWorkerInFlight = false;
        onStatus("error");
        onError(event.message || "Vanilla fast-macro worker failed.");
      };
      macroWorker.postMessage({
        type: "init",
        mode: "macro",
        epoch: revision,
        seed: state.seed,
      });
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
          const panel = terrainPanelSize(
            rect.width,
            rect.height,
            state.source,
            splitLayout,
          );
          const report = parseJson<TerrainLabRenderReport>(
            lab.render(
              revision,
              state.profile,
              state.seed,
              state.centerX,
              state.centerZ,
              state.blocksAcross,
              String(state.detail),
              state.surfaceQuality,
              Math.max(1, Math.round(panel.width)),
              Math.max(1, Math.round(panel.height)),
              maxVisibleTilesPerAxis,
              state.source,
              state.view,
              state.layer,
              state.contentStage,
              camera.yaw,
              camera.pitch,
              state.projection,
              splitLayout,
            ),
          );
          setLatestReport(report);
          onRender(report);
          needsRender = report.needsRedraw;
        }
        pumpWorkers();
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
      if (
        (needsRender && (!exactWorkerInFlight || !macroWorkerInFlight))
        || !comparisonComplete
      ) {
        schedulePump();
      } else {
        if (!needsRender) {
          onStatus("ready");
        }
      }
    };
    pump();
    return () => {
      cancelled = true;
      window.cancelAnimationFrame(pumpFrame);
      exactWorker?.terminate();
      macroWorker?.terminate();
    };

    function pumpWorkers(): void {
      if (exactWorker && exactWorkerReady && !exactWorkerInFlight) {
        const serialized = lab!.nextCpuTileRequest();
        if (serialized !== undefined) {
          const request = parseJson<TerrainLabExternalCpuTileRequest>(serialized);
          exactWorkerInFlight = true;
          exactWorker.postMessage({
            type: "compile",
            epoch: revision,
            ...request,
          });
        }
      }
      if (macroWorker && macroWorkerReady && !macroWorkerInFlight) {
        const serialized = lab!.nextMacroTileRequest();
        if (serialized !== undefined) {
          const request = parseJson<TerrainLabExternalCpuTileRequest>(serialized);
          macroWorkerInFlight = true;
          macroWorker.postMessage({
            type: "compile",
            epoch: revision,
            ...request,
          });
        }
      }
    }
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
    splitLayout,
    state,
  ]);

  return (
    <div
      ref={stageRef}
      className={`terrainStage${state.source === "split" ? " compareMode" : ""}`}
      data-testid="terrain-stage"
      data-render-ready={initialized ? "true" : "false"}
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
        aria-label={
          state.profile === "overworld"
            ? "Worker-backed vanilla terrain preview"
            : "Live GPU terrain preview"
        }
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
            ? `${latestReport.publishedTileCount} ${
                latestReport.publishedTileCount === 1 ? "tile" : "tiles"
              } · 1:${latestReport.publishedSpacing}`
            : "planning tiles"}
        </span>
        <span className="canvasBadge">{formatFootprint(footprintBlocks(state))} across</span>
      </div>
      {state.source === "split" ? (
        <div className="splitLabels" aria-hidden="true">
          <span>
            {state.profile === "overworld" ? "Sampled exact" : `CPU production ${state.contentStage}`} · {panelReadiness(
              latestReport?.cpuPublishedSpacing,
              latestReport?.cpuTargetReady,
            )}
          </span>
          <span>
            {state.profile === "overworld" ? "Fast macro" : `GPU production ${state.contentStage}`} · {panelReadiness(
              latestReport?.gpuPublishedSpacing,
              latestReport?.gpuTargetReady,
            )}
          </span>
        </div>
      ) : null}
      {state.source === "split" && splitLayout === "rows" ? (
        <div
          className="pageScrollGutter splitPageScrollGutter"
          data-testid="lod-scroll-gutter"
          role="separator"
          aria-label="Swipe here to scroll the page"
          aria-orientation="horizontal"
          onPointerDown={(event) => event.stopPropagation()}
        >
          <span aria-hidden="true">↕ scroll page</span>
        </div>
      ) : null}
      <div className="canvasHint" aria-hidden="true">
        <span className="desktopHint">
          {state.profile === "overworld"
            ? state.view === "3d"
              ? "left drag orbit · right/shift/middle drag pan · arrows pan · wheel zoom"
              : "drag pan · arrows pan · wheel zoom at pointer"
            : state.view === "3d"
            ? "tap inspect · left drag orbit · right/shift/middle drag pan · arrows pan · wheel zoom"
            : "tap inspect · drag pan · arrows pan · wheel zoom at pointer"}
        </span>
        <span className="mobileHint">
          {state.profile === "overworld"
            ? state.view === "3d"
              ? "drag orbit · two-finger pan + zoom"
              : "drag pan · two-finger pan + zoom"
            : state.view === "3d"
            ? "tap inspect · drag orbit · two-finger pan + zoom"
            : "tap inspect · drag pan · two-finger pan + zoom"}
        </span>
      </div>
    </div>
  );
}

function terrainPanelSize(
  width: number,
  height: number,
  source: TerrainLabSource,
  splitLayout: "columns" | "rows",
): { width: number; height: number } {
  if (source !== "split") {
    return { width, height };
  }
  return splitLayout === "rows"
    ? { width, height: height / 2 }
    : { width: width / 2, height };
}

function terrainPanelAtPointer(
  width: number,
  height: number,
  pointerX: number,
  pointerY: number,
  source: TerrainLabSource,
  splitLayout: "columns" | "rows",
): {
  width: number;
  height: number;
  localX: number;
  localY: number;
} {
  const panel = terrainPanelSize(width, height, source, splitLayout);
  const localX = source === "split" && splitLayout === "columns"
    ? pointerX % Math.max(panel.width, 1)
    : pointerX;
  const localY = source === "split" && splitLayout === "rows"
    ? pointerY % Math.max(panel.height, 1)
    : pointerY;
  return {
    ...panel,
    localX,
    localY,
  };
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

function acceptWorkerTile(
  lab: WorkerBackedTerrainLab,
  mode: "exact" | "macro",
  result: LodWorkerResult,
): void {
  const accept = mode === "macro"
    ? lab.acceptMacroTile.bind(lab)
    : lab.acceptCpuTile.bind(lab);
  accept(
    result.revision,
    result.seed,
    result.tileX,
    result.tileZ,
    result.sampleSpacing,
    result.surfaceQuality,
    result.samples,
    result.compileMs,
  );
}

function rejectWorkerTile(
  lab: WorkerBackedTerrainLab,
  mode: "exact" | "macro",
  error: Extract<LodWorkerResponse, { type: "error" }>,
): void {
  if (
    error.revision === undefined
    || error.seed === undefined
    || error.tileX === undefined
    || error.tileZ === undefined
    || error.sampleSpacing === undefined
    || error.surfaceQuality === undefined
  ) {
    return;
  }
  const reject = mode === "macro"
    ? lab.rejectMacroTile.bind(lab)
    : lab.rejectCpuTile.bind(lab);
  reject(
    error.revision,
    error.seed,
    error.tileX,
    error.tileZ,
    error.sampleSpacing,
    error.surfaceQuality,
  );
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
  return `${blocks.toLocaleString()} ${blocks === 1 ? "block" : "blocks"}`;
}
