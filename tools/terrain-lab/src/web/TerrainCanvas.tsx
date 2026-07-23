import { useEffect, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent, WheelEvent as ReactWheelEvent } from "react";
import type { TerrainLab } from "../../generated/pkg/mclone_terrain_lab";
import initTerrainLab, {
  mclone_terrain_lab_create,
} from "../../generated/pkg/mclone_terrain_lab";

import {
  footprintBlocks,
  nextSpacing,
  panTerrainLabState,
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
  sampleSpacing: number;
  cellsPerAxis: number;
  samplesPerAxis: number;
  sampleCount: number;
  vertexCount: number;
  footprintBlocks: number;
  footprintChunks: number;
  source: string;
  view: string;
  layer: string;
  topology: string;
  width: number;
  height: number;
  cpuReferenceMs: number;
  encodeSubmitMs: number;
  requestMs: number;
  referenceBytes: number;
  gpuSampleBytes: number;
  readbackBytes: number;
  residentBytes: number;
  comparisonPending: boolean;
  staleResultCount: number;
}

export interface TerrainLabComparisonReport {
  revision: number;
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
  staleResultCount: number;
}

interface TerrainCanvasProps {
  state: TerrainLabState;
  onStateChange: (state: TerrainLabState) => void;
  onAdapter: (report: TerrainLabAdapterReport) => void;
  onRender: (report: TerrainLabRenderReport) => void;
  onComparison: (report: TerrainLabComparisonReport | undefined) => void;
  onError: (error: string | undefined) => void;
  onStatus: (status: "loading" | "ready" | "rendering" | "error") => void;
}

interface PointerStart {
  pointerId: number;
  clientX: number;
  clientY: number;
  state: TerrainLabState;
}

export function TerrainCanvas({
  state,
  onStateChange,
  onAdapter,
  onRender,
  onComparison,
  onError,
  onStatus,
}: TerrainCanvasProps): React.JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const labRef = useRef<TerrainLab | undefined>(undefined);
  const revisionRef = useRef(0);
  const pointerStartRef = useRef<PointerStart | undefined>(undefined);
  const [canvasSize, setCanvasSize] = useState({ width: 1280, height: 720 });
  const [initialized, setInitialized] = useState(false);

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
      await initTerrainLab();
      if (cancelled) {
        return;
      }
      const lab = await mclone_terrain_lab_create(canvas) as TerrainLab;
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
      const height = Math.max(1, Math.min(1536, Math.round(rect.height * pixelRatio)));
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
    let pollFrame = 0;
    const revision = ++revisionRef.current;
    try {
      onStatus("rendering");
      onError(undefined);
      onComparison(undefined);
      lab.resize(canvasSize.width, canvasSize.height);
      const report = parseJson<TerrainLabRenderReport>(
        lab.render(
          revision,
          state.seed,
          state.centerX,
          state.centerZ,
          state.spacing,
          state.source,
          state.view,
          state.layer,
        ),
      );
      onRender(report);
      onStatus("ready");
    } catch (error: unknown) {
      onStatus("error");
      onError(errorMessage(error));
      return;
    }

    const poll = (): void => {
      if (cancelled) {
        return;
      }
      try {
        const result = lab.pollComparison();
        if (result !== undefined) {
          const comparison = parseJson<TerrainLabComparisonReport>(result);
          if (comparison.revision === revision) {
            onComparison(comparison);
            return;
          }
        }
      } catch (error: unknown) {
        onError(errorMessage(error));
        return;
      }
      pollFrame = window.requestAnimationFrame(poll);
    };
    pollFrame = window.requestAnimationFrame(poll);
    return () => {
      cancelled = true;
      window.cancelAnimationFrame(pollFrame);
    };
  }, [
    canvasSize,
    initialized,
    onComparison,
    onError,
    onRender,
    onStatus,
    state,
  ]);

  const beginPan = (event: ReactPointerEvent<HTMLDivElement>): void => {
    if (event.button !== 0) {
      return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    pointerStartRef.current = {
      pointerId: event.pointerId,
      clientX: event.clientX,
      clientY: event.clientY,
      state,
    };
  };

  const finishPan = (event: ReactPointerEvent<HTMLDivElement>): void => {
    const start = pointerStartRef.current;
    if (!start || start.pointerId !== event.pointerId) {
      return;
    }
    pointerStartRef.current = undefined;
    const stage = stageRef.current;
    if (!stage) {
      return;
    }
    const rect = stage.getBoundingClientRect();
    const footprint = footprintBlocks(start.state);
    const deltaX = -((event.clientX - start.clientX) / Math.max(rect.width, 1)) * footprint;
    const deltaZ = ((event.clientY - start.clientY) / Math.max(rect.height, 1)) * footprint;
    if (Math.abs(deltaX) < start.state.spacing && Math.abs(deltaZ) < start.state.spacing) {
      return;
    }
    onStateChange(panTerrainLabState(start.state, Math.round(deltaX), Math.round(deltaZ)));
  };

  const zoomWithWheel = (event: ReactWheelEvent<HTMLDivElement>): void => {
    event.preventDefault();
    onStateChange({
      ...state,
      spacing: nextSpacing(state.spacing, event.deltaY > 0 ? "out" : "in"),
    });
  };

  return (
    <div
      ref={stageRef}
      className="terrainStage"
      data-testid="terrain-stage"
      data-render-ready={initialized ? "true" : "false"}
      onPointerDown={beginPan}
      onPointerUp={finishPan}
      onPointerCancel={() => {
        pointerStartRef.current = undefined;
      }}
      onWheel={zoomWithWheel}
    >
      <canvas
        ref={canvasRef}
        className="terrainCanvas"
        width={canvasSize.width}
        height={canvasSize.height}
        aria-label="Live GPU terrain preview"
      />
      <div className="canvasTopline" aria-hidden="true">
        <span className="canvasBadge primary">live compute</span>
        <span className="canvasBadge">64 × 64 cells</span>
        <span className="canvasBadge">{formatFootprint(footprintBlocks(state))} square</span>
      </div>
      {state.source === "split" ? (
        <div className="splitLabels" aria-hidden="true">
          <span>CPU reference</span>
          <span>GPU approximate</span>
        </div>
      ) : null}
      <div className="canvasHint" aria-hidden="true">
        drag to pan · wheel or pinch controls to zoom
      </div>
    </div>
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
  return `${blocks.toLocaleString()} blocks`;
}
