import { useEffect, useRef, useState } from "react";
import type {
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent as ReactPointerEvent,
} from "react";
import type { CanonicalTerrainLab } from "../../generated/pkg/mclone_terrain_lab";
import {
  canonicalTerrainChunkOrder,
  mclone_terrain_lab_create_canonical,
} from "../../generated/pkg/mclone_terrain_lab";

import {
  arrowPanTerrainLabState,
  canonicalTerrainCenterChunk,
  footprintBlocks,
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
import { CanonicalRawCache } from "./canonical-raw-cache";
import { initializeTerrainLab } from "./terrain-lab-wasm";

export interface CanonicalTerrainReport {
  epoch: number;
  requestedChunks: number;
  publishedChunks: number;
  queuedChunks: number;
  residentHits: number;
  cacheHits: number;
  admissionFrames: number;
  maxFrameAdmissions: number;
  staleChunks: number;
  generationMs: number;
  meshUploadMs: number;
  firstChunkMs: number | null;
  completeMs: number | null;
  vertexCount: number;
  indexCount: number;
  retainedDependencyChunks: number;
  cachedChunks: number;
  residentRawBytes: number;
  cacheRawBytes: number;
  residentMeshUsedBytes: number;
  trackedBytes: number;
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
  residentRawBytes: number;
  residentMeshUsedBytes: number;
}

interface CanonicalRetainReport {
  residentChunks: number;
  removedChunks: number;
  removedSections: number;
  vertexCount: number;
  indexCount: number;
  residentRawBytes: number;
  residentMeshUsedBytes: number;
}

interface ResponsiveCanonicalTerrainLab extends CanonicalTerrainLab {
  retainChunks(coordinatesJson: string): string;
  resetProfile(seed: string, profile: string): void;
}

interface PendingCanonicalChunk {
  result: CanonicalWorkerResult;
  cacheHit: boolean;
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
const CANONICAL_PENDING_HIGH_WATER = 2;

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
  const labRef = useRef<ResponsiveCanonicalTerrainLab | undefined>(undefined);
  const workerRef = useRef<Worker | undefined>(undefined);
  const epochRef = useRef(0);
  const cacheRef = useRef(new CanonicalRawCache());
  const residentRef = useRef(new Set<string>());
  const residentIdentityRef = useRef<string | undefined>(undefined);
  const renderFrameRef = useRef<number | undefined>(undefined);
  const pointerStartRef = useRef<PointerStart | undefined>(undefined);
  const activePointersRef = useRef(new Map<number, ActivePointer>());
  const pinchStartRef = useRef<PinchStart | undefined>(undefined);
  const currentViewRef = useRef({ state, camera });
  currentViewRef.current = { state, camera };
  const [initialized, setInitialized] = useState(false);
  const [canvasSize, setCanvasSize] = useState({ width: 900, height: 700 });
  const [latestReport, setLatestReport] = useState<CanonicalTerrainReport>();
  const coverageCenterChunkX = canonicalTerrainCenterChunk(state.centerX);
  const coverageCenterChunkZ = canonicalTerrainCenterChunk(state.centerZ);
  const scheduleRenderRef = useRef<() => void>(() => undefined);
  scheduleRenderRef.current = (): void => {
    if (renderFrameRef.current !== undefined) {
      return;
    }
    renderFrameRef.current = requestAnimationFrame(() => {
      renderFrameRef.current = undefined;
      const currentLab = labRef.current;
      if (!currentLab) {
        return;
      }
      const current = currentViewRef.current;
      try {
        currentLab.render(
          current.state.centerX,
          current.state.centerZ,
          current.state.blocksAcross,
          current.state.view,
          current.camera.yaw,
          current.camera.pitch,
          current.state.projection,
        );
      } catch (error: unknown) {
        onError(errorMessage(error));
      }
    });
  };

  useEffect(() => {
    let cancelled = false;
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    void Promise.all([
      initializeTerrainLab(),
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
      ) as ResponsiveCanonicalTerrainLab;
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
      if (renderFrameRef.current !== undefined) {
        cancelAnimationFrame(renderFrameRef.current);
      }
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
    scheduleRenderRef.current();
  }, [canvasSize, initialized]);

  useEffect(() => {
    if (!initialized) {
      return;
    }
    scheduleRenderRef.current();
  }, [
    camera.pitch,
    camera.yaw,
    initialized,
    state.blocksAcross,
    state.centerX,
    state.centerZ,
    state.projection,
    state.view,
  ]);

  useEffect(() => {
    const lab = labRef.current;
    if (!initialized || !lab) {
      return;
    }
    try {
      lab.setPresentation(state.waterVisible, state.vegetationVisible);
      scheduleRenderRef.current();
    } catch (error: unknown) {
      onError(errorMessage(error));
    }
  }, [
    initialized,
    onError,
    state.vegetationVisible,
    state.waterVisible,
  ]);

  useEffect(() => {
    const lab = labRef.current;
    if (!initialized || !lab) {
      return;
    }
    const epoch = ++epochRef.current;
    const requestStarted = performance.now();
    workerRef.current?.terminate();
    workerRef.current = undefined;
    const residentIdentity = [
      state.profile,
      state.seed,
      state.canonicalStage,
      cacheEpoch,
      cacheEnabled ? "cache-on" : "cache-off",
    ].join(":");
    if (residentIdentityRef.current !== residentIdentity) {
      residentIdentityRef.current = residentIdentity;
      residentRef.current.clear();
      cacheRef.current.clear();
      lab.resetProfile(state.seed, state.profile);
    }
    if (!cacheEnabled) {
      cacheRef.current.clear();
    }
    const coordinates = JSON.parse(
      canonicalTerrainChunkOrder(
        coverageCenterChunkX * 16,
        coverageCenterChunkZ * 16,
        state.canonicalRadius,
      ),
    ) as CanonicalCoordinate[];
    const desiredKeys = new Set(coordinates.map(canonicalCoordinateKey));
    residentRef.current = new Set(
      [...residentRef.current].filter((key) => desiredKeys.has(key)),
    );
    const retained = parseJson<CanonicalRetainReport>(
      lab.retainChunks(JSON.stringify(coordinates)),
    );
    const report: CanonicalTerrainReport = {
      epoch,
      requestedChunks: coordinates.length,
      publishedChunks: residentRef.current.size,
      queuedChunks: coordinates.length - residentRef.current.size,
      residentHits: residentRef.current.size,
      cacheHits: 0,
      admissionFrames: 0,
      maxFrameAdmissions: 0,
      staleChunks: 0,
      generationMs: 0,
      meshUploadMs: 0,
      firstChunkMs: residentRef.current.size > 0 ? 0 : null,
      completeMs: null,
      vertexCount: retained.vertexCount,
      indexCount: retained.indexCount,
      retainedDependencyChunks: 0,
      cachedChunks: cacheRef.current.size,
      residentRawBytes: retained.residentRawBytes,
      cacheRawBytes: cacheRef.current.rawBytes,
      residentMeshUsedBytes: retained.residentMeshUsedBytes,
      trackedBytes: 0,
      complete: false,
    };
    updateTrackedBytes(report);
    publishReport(report);
    const pending: PendingCanonicalChunk[] = [];
    const missing: CanonicalCoordinate[] = [];
    for (const coordinate of coordinates) {
      if (residentRef.current.has(canonicalCoordinateKey(coordinate))) {
        continue;
      }
      const cached = cacheEnabled
        ? cacheRef.current.get(cacheKey(state, coordinate))
        : undefined;
      if (cacheEnabled && cached) {
        pending.push({ result: cached, cacheHit: true });
      } else {
        missing.push(coordinate);
      }
    }
    let cancelled = false;
    let admissionFrame: number | undefined;
    let finished = false;
    let nextMissingIndex = 0;
    let workerReady = false;
    let workerInFlight = false;
    const worker = missing.length > 0
      ? new Worker(new URL("./canonical-worker.ts", import.meta.url), {
        type: "module",
        name: `mclone-canonical-terrain-${epoch}`,
      })
      : undefined;
    if (worker) {
      workerRef.current = worker;
      worker.onmessage = (event: MessageEvent<CanonicalWorkerResponse>): void => {
        const response = event.data;
        if (cancelled || response.epoch !== epoch || epochRef.current !== epoch) {
          report.staleChunks += 1;
          return;
        }
        if (response.type === "ready") {
          workerReady = true;
          pumpWorker();
          return;
        }
        if (response.type === "error") {
          finished = true;
          onError(response.message);
          worker.terminate();
          return;
        }
        workerInFlight = false;
        if (cacheEnabled) {
          cacheRef.current.set(cacheKey(state, response), response);
        }
        pending.push({ result: response, cacheHit: false });
        scheduleAdmission();
        pumpWorker();
      };
      worker.onerror = (event): void => {
        finished = true;
        onError(event.message || "Canonical terrain worker failed.");
      };
      worker.postMessage({
        type: "init",
        epoch,
        profile: state.profile,
        seed: state.seed,
        stage: state.canonicalStage,
      });
    }
    scheduleAdmission();
    maybeFinish();

    return () => {
      cancelled = true;
      if (admissionFrame !== undefined) {
        cancelAnimationFrame(admissionFrame);
      }
      worker?.terminate();
      if (worker && workerRef.current === worker) {
        workerRef.current = undefined;
      }
    };

    function scheduleAdmission(): void {
      if (cancelled || finished || admissionFrame !== undefined) {
        return;
      }
      if (pending.length === 0) {
        pumpWorker();
        maybeFinish();
        return;
      }
      admissionFrame = requestAnimationFrame(() => {
        admissionFrame = undefined;
        if (cancelled || epochRef.current !== epoch) {
          return;
        }
        const next = pending.shift();
        if (next) {
          acceptChunk(lab!, next.result, report, next.cacheHit);
          report.admissionFrames += 1;
          report.maxFrameAdmissions = Math.max(report.maxFrameAdmissions, 1);
          publishReport(report);
          scheduleRenderRef.current();
        }
        pumpWorker();
        if (pending.length > 0) {
          scheduleAdmission();
        }
        maybeFinish();
      });
    }

    function pumpWorker(): void {
      if (
        !worker
        || !workerReady
        || workerInFlight
        || pending.length >= CANONICAL_PENDING_HIGH_WATER
      ) {
        return;
      }
      const coordinate = missing[nextMissingIndex++];
      if (!coordinate) {
        maybeFinish();
        return;
      }
      workerInFlight = true;
      worker.postMessage({
        type: "compile",
        epoch,
        chunkX: coordinate.chunkX,
        chunkZ: coordinate.chunkZ,
      });
    }

    function acceptChunk(
      currentLab: ResponsiveCanonicalTerrainLab,
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
      residentRef.current.add(canonicalCoordinateKey(result));
      current.publishedChunks = residentRef.current.size;
      current.queuedChunks = Math.max(0, current.requestedChunks - current.publishedChunks);
      if (cacheHit) {
        current.cacheHits += 1;
      } else {
        current.generationMs += result.generationMs;
      }
      current.meshUploadMs += accepted.meshUploadMs;
      current.vertexCount = accepted.vertexCount;
      current.indexCount = accepted.indexCount;
      current.retainedDependencyChunks = result.retainedDependencyChunks;
      current.cachedChunks = cacheRef.current.size;
      current.residentRawBytes = accepted.residentRawBytes;
      current.cacheRawBytes = cacheRef.current.rawBytes;
      current.residentMeshUsedBytes = accepted.residentMeshUsedBytes;
      updateTrackedBytes(current);
      current.firstChunkMs ??= performance.now() - requestStarted;
    }

    function maybeFinish(): void {
      if (
        cancelled
        || finished
        || pending.length > 0
        || workerInFlight
        || nextMissingIndex < missing.length
        || (missing.length > 0 && !workerReady)
      ) {
        return;
      }
      finished = true;
      report.complete = true;
      report.queuedChunks = 0;
      report.completeMs = performance.now() - requestStarted;
      publishReport(report);
      scheduleRenderRef.current();
      worker?.terminate();
      if (worker && workerRef.current === worker) {
        workerRef.current = undefined;
      }
    }

    function publishReport(current: CanonicalTerrainReport): void {
      const copy = { ...current };
      setLatestReport(copy);
      onReport(copy);
    }
  }, [
    cacheEnabled,
    cacheEpoch,
    coverageCenterChunkX,
    coverageCenterChunkZ,
    initialized,
    onError,
    onReport,
    state.canonicalRadius,
    state.canonicalStage,
    state.profile,
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
        <span className="canvasBadge primary">
          Canonical · {state.profile === "overworld" ? "Vanilla" : "Mclone"} · {
            state.canonicalStage
          }
        </span>
        <span className="canvasBadge">
          {latestReport
            ? `${latestReport.publishedChunks}/${latestReport.requestedChunks} ${
                latestReport.requestedChunks === 1 ? "chunk" : "chunks"
              }`
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
  return `${
    state.profile
  }:${state.seed}:${state.canonicalStage}:${coordinate.chunkX}:${coordinate.chunkZ}`;
}

function updateTrackedBytes(report: CanonicalTerrainReport): void {
  report.trackedBytes = report.residentRawBytes
    + report.cacheRawBytes
    + report.residentMeshUsedBytes;
}

function canonicalCoordinateKey(
  coordinate: Pick<CanonicalWorkerResult, "chunkX" | "chunkZ">,
): string {
  return `${coordinate.chunkX}:${coordinate.chunkZ}`;
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
    : `${blocks.toLocaleString()} ${blocks === 1 ? "block" : "blocks"}`;
}
