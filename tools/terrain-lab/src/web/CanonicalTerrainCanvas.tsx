import { useEffect, useRef, useState } from "react";
import type { CanonicalTerrainLab } from "../../generated/pkg/mclone_terrain_lab";
import {
  canonicalTerrainChunkOrder,
  mclone_terrain_lab_create_canonical,
} from "../../generated/pkg/mclone_terrain_lab";

import {
  canonicalTerrainCenterChunk,
  footprintBlocks,
  type TerrainLabCamera,
  type TerrainLabState,
  type TerrainLabTexturePresentation,
  type TerrainLabVisualProfile,
} from "../state";
import type {
  CanonicalCoordinate,
  CanonicalWorkerPackedAdmission,
  CanonicalWorkerResponse,
} from "./canonical-worker-protocol";
import { initializeTerrainLab } from "./terrain-lab-wasm";
import {
  loadTerrainVisualAssets,
  optionalReferenceBytes,
} from "./visual-assets";
import { useWorldViewNavigation } from "./use-world-view-navigation";

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
  workerPresentationMs: number;
  workerMeshMs: number;
  workerPackMs: number;
  workerTransferMs: number;
  meshUploadMs: number;
  mainDecodeMs: number;
  maxAdmissionMs: number;
  meshTargetChunks: number;
  warmHits: number;
  warmChunks: number;
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
  visualProfile: TerrainLabVisualProfile;
  texturePresentation: TerrainLabTexturePresentation;
  comparison?: boolean;
  camera: TerrainLabCamera;
  cacheEnabled: boolean;
  cacheEpoch: number;
  onStateChange: (state: TerrainLabState) => void;
  onCameraChange: (camera: TerrainLabCamera) => void;
  onReport: (report: CanonicalTerrainReport) => void;
  onError: (error: string | undefined) => void;
}

interface CanonicalPackedAcceptReport {
  activeChunks: number;
  warmChunks: number;
  targetChunks: number;
  sectionCount: number;
  vertexCount: number;
  indexCount: number;
  meshUploadMs: number;
  decodeMs: number;
  residentMeshUsedBytes: number;
}

interface CanonicalPackedPrepareReport {
  activeChunks: number;
  warmChunks: number;
  warmAvailable: CanonicalCoordinate[];
  evictedChunks: number;
  removedSections: number;
  vertexCount: number;
  indexCount: number;
  residentMeshUsedBytes: number;
}

interface ResponsiveCanonicalTerrainLab extends CanonicalTerrainLab {
  preparePackedChunks(coordinatesJson: string): string;
  activatePackedChunk(chunkX: number, chunkZ: number): string;
  acceptPackedMesh(
    chunkX: number,
    chunkZ: number,
    fingerprint: string,
    packedSections: Uint8Array,
  ): string;
  resetProfile(seed: string, profile: string): void;
}

type PendingCanonicalChunk =
  | { kind: "warm"; coordinate: CanonicalCoordinate }
  | { kind: "mesh"; admission: CanonicalWorkerPackedAdmission };

const CANONICAL_PENDING_HIGH_WATER = 2;
const CANONICAL_WORKER_MAX_BATCH = 16;

export function CanonicalTerrainCanvas({
  state,
  visualProfile,
  texturePresentation,
  comparison = false,
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
  const workerIdentityRef = useRef<string | undefined>(undefined);
  const workerReadyRef = useRef(false);
  const epochRef = useRef(0);
  const residentRef = useRef(new Set<string>());
  const residentIdentityRef = useRef<string | undefined>(undefined);
  const presentationIdentityRef = useRef<string | undefined>(undefined);
  const renderFrameRef = useRef<number | undefined>(undefined);
  const currentViewRef = useRef({ state, camera });
  currentViewRef.current = { state, camera };
  const [initialized, setInitialized] = useState(false);
  const [canvasSize, setCanvasSize] = useState({ width: 900, height: 700 });
  const [latestReport, setLatestReport] = useState<CanonicalTerrainReport>();
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
      return {
        x: clientX - rect.left,
        y: clientY - rect.top,
        width: rect.width,
        height: rect.height,
      };
    },
  });
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
    setInitialized(false);
    setLatestReport(undefined);
    void Promise.all([
      initializeTerrainLab(),
      loadTerrainVisualAssets(),
    ]).then(async ([, assets]) => {
      if (cancelled) {
        return;
      }
      const lab = await mclone_terrain_lab_create_canonical(
        canvas,
        assets.authored,
        optionalReferenceBytes(assets),
        assets.provisional,
        assets.diagnostic,
        visualProfile,
        texturePresentation,
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
  }, [onError, texturePresentation, visualProfile]);

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
    const epoch = ++epochRef.current;
    const requestStarted = performance.now();
    const residentIdentity = [
      state.profile,
      state.seed,
      state.canonicalStage,
      visualProfile,
      texturePresentation,
      cacheEpoch,
      cacheEnabled ? "cache-on" : "cache-off",
    ].join(":");
    const presentationIdentity = [
      state.waterVisible ? "water" : "dry",
      state.vegetationVisible ? "vegetation" : "bare",
    ].join(":");
    const hardReset = residentIdentityRef.current !== residentIdentity;
    const presentationReset =
      presentationIdentityRef.current !== presentationIdentity;
    if (hardReset || presentationReset) {
      residentIdentityRef.current = residentIdentity;
      presentationIdentityRef.current = presentationIdentity;
      residentRef.current.clear();
      lab.resetProfile(state.seed, state.profile);
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
    const prepared = parseJson<CanonicalPackedPrepareReport>(
      lab.preparePackedChunks(JSON.stringify(coordinates)),
    );
    const warmKeys = new Set(prepared.warmAvailable.map(canonicalCoordinateKey));
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
      workerPresentationMs: 0,
      workerMeshMs: 0,
      workerPackMs: 0,
      workerTransferMs: 0,
      meshUploadMs: 0,
      mainDecodeMs: 0,
      maxAdmissionMs: 0,
      meshTargetChunks: 0,
      warmHits: 0,
      warmChunks: prepared.warmChunks,
      firstChunkMs: residentRef.current.size > 0 ? 0 : null,
      completeMs: null,
      vertexCount: prepared.vertexCount,
      indexCount: prepared.indexCount,
      retainedDependencyChunks: 0,
      cachedChunks: 0,
      residentRawBytes: 0,
      cacheRawBytes: 0,
      residentMeshUsedBytes: prepared.residentMeshUsedBytes,
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
      if (warmKeys.has(canonicalCoordinateKey(coordinate))) {
        pending.push({ kind: "warm", coordinate });
      } else {
        missing.push(coordinate);
      }
    }
    let cancelled = false;
    let admissionFrame: number | undefined;
    let finished = false;
    let nextMissingIndex = 0;
    let batchIndex = 0;
    let sessionBegan = false;
    let beginSent = false;
    let workerInFlight = false;
    let worker = workerRef.current;
    const needsWorkerInit =
      !worker || workerIdentityRef.current !== residentIdentity;
    if (needsWorkerInit) {
      worker?.terminate();
      worker = new Worker(new URL("./canonical-worker.ts", import.meta.url), {
        type: "module",
        name: `mclone-canonical-terrain-${epoch}`,
      });
      workerRef.current = worker;
      workerIdentityRef.current = residentIdentity;
      workerReadyRef.current = false;
    }
    if (worker) {
      worker.onmessage = (event: MessageEvent<CanonicalWorkerResponse>): void => {
        const response = event.data;
        if (response.type === "ready") {
          workerReadyRef.current = true;
          beginWorker();
          return;
        }
        if (cancelled || response.epoch !== epoch || epochRef.current !== epoch) {
          report.staleChunks += 1;
          return;
        }
        if (response.type === "began") {
          sessionBegan = true;
          report.cachedChunks = response.rawCacheChunks;
          report.cacheRawBytes = response.rawCacheBytes;
          updateTrackedBytes(report);
          publishReport(report);
          pumpWorker();
          maybeFinish();
          return;
        }
        if (response.type === "error") {
          finished = true;
          onError(response.message);
          worker.terminate();
          if (workerRef.current === worker) {
            workerRef.current = undefined;
            workerIdentityRef.current = undefined;
            workerReadyRef.current = false;
          }
          return;
        }
        workerInFlight = false;
        report.generationMs += response.generationMs;
        report.workerPresentationMs += response.presentationMs;
        report.workerMeshMs += response.meshMs;
        report.workerPackMs += response.packMs;
        report.workerTransferMs += response.transferMs;
        report.meshTargetChunks += response.deduplicatedTargetChunks;
        report.cachedChunks = response.rawCacheChunks;
        report.cacheRawBytes = response.rawCacheBytes;
        report.cacheHits += response.admissions.filter(
          (admission) => admission.rawCacheHit,
        ).length;
        updateTrackedBytes(report);
        pending.push(
          ...response.admissions.map(
            (admission): PendingCanonicalChunk => ({ kind: "mesh", admission }),
          ),
        );
        publishReport(report);
        scheduleAdmission();
        pumpWorker();
      };
      worker.onerror = (event): void => {
        finished = true;
        onError(event.message || "Canonical terrain worker failed.");
      };
      if (needsWorkerInit) {
        worker.postMessage({
          type: "init",
          epoch,
          profile: state.profile,
          visualProfile,
          texturePresentation,
          seed: state.seed,
          stage: state.canonicalStage,
        });
      } else if (workerReadyRef.current) {
        beginWorker();
      }
    }
    scheduleAdmission();
    maybeFinish();

    return () => {
      cancelled = true;
      if (admissionFrame !== undefined) {
        cancelAnimationFrame(admissionFrame);
      }
    };

    function beginWorker(): void {
      if (cancelled || beginSent || !worker) {
        return;
      }
      beginSent = true;
      worker.postMessage({
        type: "begin",
        epoch,
        coordinates,
        waterVisible: state.waterVisible,
        vegetationVisible: state.vegetationVisible,
        cacheEnabled,
      });
    }

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
          const admissionStarted = performance.now();
          acceptChunk(lab!, next, report);
          report.admissionFrames += 1;
          report.maxFrameAdmissions = Math.max(report.maxFrameAdmissions, 1);
          report.maxAdmissionMs = Math.max(
            report.maxAdmissionMs,
            performance.now() - admissionStarted,
          );
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
        || !sessionBegan
        || workerInFlight
        || pending.length >= CANONICAL_PENDING_HIGH_WATER
      ) {
        return;
      }
      if (nextMissingIndex >= missing.length) {
        maybeFinish();
        return;
      }
      const batchSize = Math.min(
        CANONICAL_WORKER_MAX_BATCH,
        2 ** batchIndex,
        missing.length - nextMissingIndex,
      );
      batchIndex += 1;
      const batch = missing.slice(nextMissingIndex, nextMissingIndex + batchSize);
      nextMissingIndex += batch.length;
      workerInFlight = true;
      worker.postMessage({
        type: "compileBatch",
        epoch,
        coordinates: batch,
      });
    }

    function acceptChunk(
      currentLab: ResponsiveCanonicalTerrainLab,
      next: PendingCanonicalChunk,
      current: CanonicalTerrainReport,
    ): void {
      const coordinate = next.kind === "warm" ? next.coordinate : next.admission;
      const accepted = parseJson<CanonicalPackedAcceptReport>(
        next.kind === "warm"
          ? currentLab.activatePackedChunk(coordinate.chunkX, coordinate.chunkZ)
          : currentLab.acceptPackedMesh(
            coordinate.chunkX,
            coordinate.chunkZ,
            next.admission.fingerprint,
            next.admission.packedSections,
          ),
      );
      residentRef.current.add(canonicalCoordinateKey(coordinate));
      current.publishedChunks = residentRef.current.size;
      current.queuedChunks = Math.max(0, current.requestedChunks - current.publishedChunks);
      if (next.kind === "warm") {
        current.warmHits += 1;
      } else {
        current.retainedDependencyChunks =
          next.admission.retainedDependencyChunks;
      }
      current.meshUploadMs += accepted.meshUploadMs;
      current.mainDecodeMs += accepted.decodeMs;
      current.vertexCount = accepted.vertexCount;
      current.indexCount = accepted.indexCount;
      current.warmChunks = accepted.warmChunks;
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
        || !sessionBegan
      ) {
        return;
      }
      finished = true;
      report.complete = true;
      report.queuedChunks = 0;
      report.completeMs = performance.now() - requestStarted;
      publishReport(report);
      scheduleRenderRef.current();
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
    state.vegetationVisible,
    state.waterVisible,
    texturePresentation,
    visualProfile,
  ]);

  return (
    <div
      ref={stageRef}
      className="terrainStage canonicalTerrainStage"
      data-testid="canonical-terrain-stage"
      data-comparison={comparison ? "true" : "false"}
      data-visual-profile={visualProfile}
      data-texture-presentation={texturePresentation}
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
        aria-label="Canonical textured terrain preview"
      />
      <div className="canvasTopline" aria-hidden="true">
        <span className="canvasBadge primary">
          {comparison ? "Compare" : "Canonical"} · {visualProfileLabel(visualProfile)}
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
        {texturePresentation === "flat-colors" ? "derived flat colors" : "textured"} · {
          state.canonicalStage
        } · two-finger pan + zoom
      </div>
    </div>
  );

}

function updateTrackedBytes(report: CanonicalTerrainReport): void {
  report.trackedBytes = report.residentRawBytes
    + report.cacheRawBytes
    + report.residentMeshUsedBytes;
}

function canonicalCoordinateKey(
  coordinate: CanonicalCoordinate,
): string {
  return `${coordinate.chunkX}:${coordinate.chunkZ}`;
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

function visualProfileLabel(profile: TerrainLabVisualProfile): string {
  switch (profile) {
    case "mclone-original":
      return "Mclone Original";
    case "minecraft-reference":
      return "Minecraft Reference";
    case "hybrid-authoring":
      return "Authoring Hybrid";
    case "first-party-coverage":
      return "Coverage Debug";
    case "provisional-audit":
      return "Provisional Audit";
  }
}
