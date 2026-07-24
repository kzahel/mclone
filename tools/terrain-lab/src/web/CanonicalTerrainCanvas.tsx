import { useEffect, useRef, useState } from "react";
import type {
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent as ReactPointerEvent,
} from "react";
import type { CanonicalTerrainLab } from "../../generated/pkg/mclone_terrain_lab";
import initTerrainLab, {
  canonicalTerrainChunkOrder,
  mclone_terrain_lab_create_canonical,
} from "../../generated/pkg/mclone_terrain_lab";

import {
  footprintBlocks,
  arrowPanTerrainLabState,
  grabPanTerrainLabStateInView,
  orbitTerrainLabCamera,
  pinchPanZoomTerrainLabState,
  zoomTerrainLabState,
  type TerrainLabCamera,
  type TerrainLabState,
} from "../state";
import type {
  CanonicalWorkerResponse,
  CanonicalWorkerResult,
} from "./canonical-worker-protocol";

export interface CanonicalTerrainReport {
  epoch: number;
  requestedChunks: number;
  publishedChunks: number;
  queuedChunks: number;
  cacheHits: number;
  staleChunks: number;
  generationMs: number;
  meshUploadMs: number;
  firstChunkMs: number | null;
  completeMs: number | null;
  vertexCount: number;
  indexCount: number;
  retainedDependencyChunks: number;
  complete: boolean;
}

interface CanonicalTerrainCanvasProps {
  state: TerrainLabState;
  camera: TerrainLabCamera;
  cacheEnabled: boolean;
  cacheEpoch: number;
  onStateChange: (state: TerrainLabState) => void;
  onCameraChange: (camera: TerrainLabCamera) => void;
  onReport: (report: CanonicalTerrainReport) => void;
  onError: (error: string | undefined) => void;
}

interface CanonicalCoordinate {
  chunkX: number;
  chunkZ: number;
}

interface CanonicalAcceptReport {
  vertexCount: number;
  indexCount: number;
  meshUploadMs: number;
}

interface PointerStart {
  pointerId: number;
  clientX: number;
  clientY: number;
  camera: TerrainLabCamera;
  mode: "orbit" | "pan";
  state: TerrainLabState;
}

interface ActivePointer {
  clientX: number;
  clientY: number;
}

interface PinchStart {
  distance: number;
  clientX: number;
  clientY: number;
  camera: TerrainLabCamera;
  state: TerrainLabState;
}

const AUTHORED_PACK_URL = "/first-party-packs/mclone-authored.pbp";
const FALLBACK_PACK_URL = "/first-party-packs/mclone-generated-fallback.pbp";

export function CanonicalTerrainCanvas({
  state,
  camera,
  cacheEnabled,
  cacheEpoch,
  onStateChange,
  onCameraChange,
  onReport,
  onError,
}: CanonicalTerrainCanvasProps): React.JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const labRef = useRef<CanonicalTerrainLab | undefined>(undefined);
  const workerRef = useRef<Worker | undefined>(undefined);
  const epochRef = useRef(0);
  const cacheRef = useRef(new Map<string, CanonicalWorkerResult>());
  const appliedCacheEpochRef = useRef(cacheEpoch);
  const pointerStartRef = useRef<PointerStart | undefined>(undefined);
  const activePointersRef = useRef(new Map<number, ActivePointer>());
  const pinchStartRef = useRef<PinchStart | undefined>(undefined);
  const requestStartedRef = useRef(0);
  const currentViewRef = useRef({ state, camera });
  currentViewRef.current = { state, camera };
  const [initialized, setInitialized] = useState(false);
  const [canvasSize, setCanvasSize] = useState({ width: 900, height: 700 });
  const [latestReport, setLatestReport] = useState<CanonicalTerrainReport>();

  useEffect(() => {
    let cancelled = false;
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    void Promise.all([
      initTerrainLab(),
      fetchPack(AUTHORED_PACK_URL),
      fetchPack(FALLBACK_PACK_URL),
    ]).then(async ([, authored, fallback]) => {
      if (cancelled) {
        return;
      }
      const lab = await mclone_terrain_lab_create_canonical(
        canvas,
        authored,
        fallback,
      ) as CanonicalTerrainLab;
      if (cancelled) {
        lab.free();
        return;
      }
      labRef.current = lab;
      setInitialized(true);
    }).catch((error: unknown) => {
      if (!cancelled) {
        onError(errorMessage(error));
      }
    });
    return () => {
      cancelled = true;
      workerRef.current?.terminate();
      labRef.current?.free();
      workerRef.current = undefined;
      labRef.current = undefined;
    };
  }, [onError]);

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) {
      return;
    }
    const resize = (): void => {
      const rect = stage.getBoundingClientRect();
      const ratio = Math.min(window.devicePixelRatio || 1, 2);
      const width = Math.max(1, Math.min(2048, Math.round(rect.width * ratio)));
      const height = Math.max(1, Math.min(2048, Math.round(rect.height * ratio)));
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
    lab.resize(canvasSize.width, canvasSize.height);
    try {
      lab.render(
        state.centerX,
        state.centerZ,
        state.blocksAcross,
        state.view,
        camera.yaw,
        camera.pitch,
      );
    } catch (error: unknown) {
      onError(errorMessage(error));
    }
  }, [
    camera,
    canvasSize,
    initialized,
    onError,
    state.blocksAcross,
    state.centerX,
    state.centerZ,
    state.view,
  ]);

  useEffect(() => {
    const lab = labRef.current;
    if (!initialized || !lab) {
      return;
    }
    try {
      lab.setPresentation(state.waterVisible, state.vegetationVisible);
      lab.render(
        state.centerX,
        state.centerZ,
        state.blocksAcross,
        state.view,
        camera.yaw,
        camera.pitch,
      );
    } catch (error: unknown) {
      onError(errorMessage(error));
    }
  }, [
    camera.pitch,
    camera.yaw,
    initialized,
    onError,
    state.blocksAcross,
    state.centerX,
    state.centerZ,
    state.vegetationVisible,
    state.view,
    state.waterVisible,
  ]);

  useEffect(() => {
    const lab = labRef.current;
    if (!initialized || !lab) {
      return;
    }
    const epoch = ++epochRef.current;
    requestStartedRef.current = performance.now();
    workerRef.current?.terminate();
    workerRef.current = undefined;
    if (appliedCacheEpochRef.current !== cacheEpoch) {
      cacheRef.current.clear();
      appliedCacheEpochRef.current = cacheEpoch;
    }
    if (!cacheEnabled) {
      cacheRef.current.clear();
    }
    lab.resetChunks(state.seed);
    const coordinates = JSON.parse(
      canonicalTerrainChunkOrder(
        state.centerX,
        state.centerZ,
        state.canonicalRadius,
      ),
    ) as CanonicalCoordinate[];
    const report: CanonicalTerrainReport = {
      epoch,
      requestedChunks: coordinates.length,
      publishedChunks: 0,
      queuedChunks: coordinates.length,
      cacheHits: 0,
      staleChunks: 0,
      generationMs: 0,
      meshUploadMs: 0,
      firstChunkMs: null,
      completeMs: null,
      vertexCount: 0,
      indexCount: 0,
      retainedDependencyChunks: 0,
      complete: false,
    };
    publishReport(report);
    const missing: CanonicalCoordinate[] = [];
    for (const coordinate of coordinates) {
      const cached = cacheRef.current.get(cacheKey(state, coordinate));
      if (cacheEnabled && cached) {
        report.cacheHits += 1;
        acceptChunk(lab, cached, report, true);
      } else {
        missing.push(coordinate);
      }
    }
    if (missing.length === 0) {
      finishReport(report);
      renderCanonical(lab);
      return;
    }

    const worker = new Worker(new URL("./canonical-worker.ts", import.meta.url), {
      type: "module",
      name: `mclone-canonical-terrain-${epoch}`,
    });
    workerRef.current = worker;
    let nextIndex = 0;
    const requestNext = (): void => {
      const coordinate = missing[nextIndex++];
      if (!coordinate) {
        finishReport(report);
        renderCanonical(lab);
        worker.terminate();
        if (workerRef.current === worker) {
          workerRef.current = undefined;
        }
        return;
      }
      worker.postMessage({
        type: "compile",
        epoch,
        chunkX: coordinate.chunkX,
        chunkZ: coordinate.chunkZ,
      });
    };
    worker.onmessage = (event: MessageEvent<CanonicalWorkerResponse>): void => {
      const response = event.data;
      if (response.epoch !== epoch || epochRef.current !== epoch) {
        report.staleChunks += 1;
        return;
      }
      if (response.type === "ready") {
        requestNext();
        return;
      }
      if (response.type === "error") {
        onError(response.message);
        worker.terminate();
        return;
      }
      if (cacheEnabled) {
        cacheRef.current.set(cacheKey(state, response), cloneResult(response));
      }
      acceptChunk(lab, response, report, false);
      renderCanonical(lab);
      requestNext();
    };
    worker.onerror = (event): void => {
      onError(event.message || "Canonical terrain worker failed.");
    };
    worker.postMessage({
      type: "init",
      epoch,
      seed: state.seed,
      stage: state.canonicalStage,
    });
    return () => {
      worker.terminate();
      if (workerRef.current === worker) {
        workerRef.current = undefined;
      }
    };

    function acceptChunk(
      currentLab: CanonicalTerrainLab,
      result: CanonicalWorkerResult,
      current: CanonicalTerrainReport,
      cacheHit: boolean,
    ): void {
      const accepted = parseJson<CanonicalAcceptReport>(
        currentLab.acceptChunk(
          state.seed,
          result.chunkX,
          result.chunkZ,
          result.minY,
          result.height,
          result.blocks,
          result.biomes,
          result.fingerprint,
          currentViewRef.current.state.waterVisible,
          currentViewRef.current.state.vegetationVisible,
        ),
      );
      current.publishedChunks += 1;
      current.queuedChunks = Math.max(0, current.requestedChunks - current.publishedChunks);
      if (!cacheHit) {
        current.generationMs += result.generationMs;
      }
      current.meshUploadMs += accepted.meshUploadMs;
      current.vertexCount = accepted.vertexCount;
      current.indexCount = accepted.indexCount;
      current.retainedDependencyChunks = result.retainedDependencyChunks;
      current.firstChunkMs ??= performance.now() - requestStartedRef.current;
      publishReport(current);
    }

    function finishReport(current: CanonicalTerrainReport): void {
      current.complete = true;
      current.queuedChunks = 0;
      current.completeMs = performance.now() - requestStartedRef.current;
      publishReport(current);
    }

    function publishReport(current: CanonicalTerrainReport): void {
      const copy = { ...current };
      setLatestReport(copy);
      onReport(copy);
    }

    function renderCanonical(currentLab: CanonicalTerrainLab): void {
      const current = currentViewRef.current;
      currentLab.render(
        current.state.centerX,
        current.state.centerZ,
        current.state.blocksAcross,
        current.state.view,
        current.camera.yaw,
        current.camera.pitch,
      );
    }
  }, [
    cacheEnabled,
    cacheEpoch,
    initialized,
    onError,
    onReport,
    state.canonicalRadius,
    state.canonicalStage,
    state.centerX,
    state.centerZ,
    state.seed,
  ]);

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) {
      return;
    }
    const zoom = (event: WheelEvent): void => {
      event.preventDefault();
      onStateChange(zoomTerrainLabState(state, Math.exp(event.deltaY * 0.0015)));
    };
    stage.addEventListener("wheel", zoom, { passive: false });
    return () => stage.removeEventListener("wheel", zoom);
  }, [onStateChange, state]);

  const beginPointer = (event: ReactPointerEvent<HTMLDivElement>): void => {
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
        const midpoint = pointerMidpoint(pair[0], pair[1]);
        pinchStartRef.current = {
          distance: pointerDistance(pair[0], pair[1]),
          clientX: midpoint.clientX,
          clientY: midpoint.clientY,
          camera,
          state,
        };
        pointerStartRef.current = undefined;
        return;
      }
    }
    pointerStartRef.current = {
      pointerId: event.pointerId,
      clientX: event.clientX,
      clientY: event.clientY,
      camera,
      mode: state.view === "3d" && event.button === 0 && !event.shiftKey ? "orbit" : "pan",
      state,
    };
  };
  const movePointer = (event: ReactPointerEvent<HTMLDivElement>): void => {
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
          const midpoint = pointerMidpoint(pair[0], pair[1]);
          onStateChange(pinchPanZoomTerrainLabState(
            pinch.state,
            pinch.camera,
            pinch.distance / distance,
            (pinch.clientX - rect.left) / Math.max(rect.width, 1) - 0.5,
            (pinch.clientY - rect.top) / Math.max(rect.height, 1) - 0.5,
            midpoint.clientX - pinch.clientX,
            midpoint.clientY - pinch.clientY,
            rect.width,
            rect.height,
            rect.width / Math.max(rect.height, 1),
          ));
        }
        return;
      }
    }
    const start = pointerStartRef.current;
    if (!start || start.pointerId !== event.pointerId) {
      return;
    }
    const rect = stage.getBoundingClientRect();
    if (start.mode === "orbit") {
      onCameraChange(orbitTerrainLabCamera(
        start.camera,
        event.clientX - start.clientX,
        event.clientY - start.clientY,
        rect.width,
        rect.height,
      ));
    } else {
      onStateChange(grabPanTerrainLabStateInView(
        start.state,
        start.camera,
        event.clientX - start.clientX,
        event.clientY - start.clientY,
        rect.width,
        rect.height,
        rect.width / Math.max(rect.height, 1),
      ));
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

  return (
    <div
      ref={stageRef}
      className="terrainStage canonicalTerrainStage"
      data-testid="canonical-terrain-stage"
      data-render-ready={initialized ? "true" : "false"}
      tabIndex={0}
      aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight"
      onPointerDown={beginPointer}
      onPointerMove={movePointer}
      onPointerUp={(event) => {
        finishPointer(event.pointerId);
      }}
      onPointerCancel={(event) => {
        finishPointer(event.pointerId);
      }}
      onContextMenu={(event) => event.preventDefault()}
      onKeyDown={handleKeyDown}
    >
      <canvas
        ref={canvasRef}
        className="terrainCanvas"
        width={canvasSize.width}
        height={canvasSize.height}
        aria-label="Canonical textured terrain preview"
      />
      <div className="canvasTopline" aria-hidden="true">
        <span className="canvasBadge primary">Canonical · {state.canonicalStage}</span>
        <span className="canvasBadge">
          {latestReport
            ? `${latestReport.publishedChunks}/${latestReport.requestedChunks} chunks`
            : "loading atlas"}
        </span>
        <span className="canvasBadge">
          radius {state.canonicalRadius} · {formatFootprint(footprintBlocks(state))}
        </span>
      </div>
      <div className="canvasHint" aria-hidden="true">
        exact blocks + biomes · two-finger pan + zoom · preview light
      </div>
    </div>
  );

  function finishPointer(pointerId: number): void {
    activePointersRef.current.delete(pointerId);
    if (activePointersRef.current.size < 2) {
      pinchStartRef.current = undefined;
    }
    if (pointerStartRef.current?.pointerId === pointerId) {
      pointerStartRef.current = undefined;
    }
  }
}

function cacheKey(
  state: TerrainLabState,
  coordinate: Pick<CanonicalWorkerResult, "chunkX" | "chunkZ">,
): string {
  return `${state.seed}:${state.canonicalStage}:${coordinate.chunkX}:${coordinate.chunkZ}`;
}

function cloneResult(result: CanonicalWorkerResult): CanonicalWorkerResult {
  return {
    ...result,
    blocks: result.blocks.slice(),
    biomes: result.biomes.slice(),
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

function pointerMidpoint(
  first: ActivePointer,
  second: ActivePointer,
): ActivePointer {
  return {
    clientX: (first.clientX + second.clientX) * 0.5,
    clientY: (first.clientY + second.clientY) * 0.5,
  };
}

async function fetchPack(url: string): Promise<Uint8Array> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to load ${url}: HTTP ${response.status}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

function parseJson<T>(value: string): T {
  return JSON.parse(value) as T;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function formatFootprint(blocks: number): string {
  return blocks >= 1_000
    ? `${(blocks / 1_000).toLocaleString(undefined, { maximumFractionDigits: 1 })} km`
    : `${blocks.toLocaleString()} blocks`;
}
