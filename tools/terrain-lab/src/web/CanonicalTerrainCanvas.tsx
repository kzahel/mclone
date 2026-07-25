import { useEffect, useRef, useState } from "react";
import {
  PolledWorkerTransport,
} from "../../../../native/apps/mclone-web-client/www/mclone-worker-transport";
import type { CanonicalTerrainLab } from "../../generated/pkg/mclone_terrain_lab";
import {
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
import { initializeTerrainLab } from "./terrain-lab-wasm";
import {
  loadTerrainVisualAssets,
  optionalReferenceBytes,
  type TerrainVisualAssetBytes,
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
  transportKind: string;
  crossOriginIsolated: boolean;
  resultArenaCapacityBytes: number;
  resultArenaHighWaterBytes: number;
  resultArenaOverflowCount: number;
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

interface CanonicalCoordinatorReport extends CanonicalTerrainReport {
  needsPump: boolean;
  renderChanged: boolean;
  error: string | null;
}

interface ResponsiveCanonicalTerrainLab extends CanonicalTerrainLab {
  beginExactCoverage(
    workerTransport: PolledWorkerTransport,
    authoredBytes: Uint8Array,
    referenceBytes: Uint8Array,
    provisionalBytes: Uint8Array,
    diagnosticBytes: Uint8Array,
    centerX: number,
    centerZ: number,
    radius: number,
    profile: string,
    visualProfile: string,
    texturePresentation: string,
    seed: string,
    stage: string,
    waterVisible: boolean,
    vegetationVisible: boolean,
    cacheEnabled: boolean,
    cacheEpoch: number,
  ): string;
  pumpExactCoverage(): string;
  shutdownExactWorker(): void;
}

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
  const visualAssetsRef = useRef<TerrainVisualAssetBytes | undefined>(undefined);
  const workerTransportRef = useRef<PolledWorkerTransport | undefined>(undefined);
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
      visualAssetsRef.current = assets;
      workerTransportRef.current = new PolledWorkerTransport(new Worker(
        new URL("./canonical-worker.ts", import.meta.url),
        {
          type: "module",
          name: "mclone-canonical-terrain",
        },
      ));
      setInitialized(true);
    }).catch((error: unknown) => {
      if (!cancelled) {
        onError(errorMessage(error));
      }
    });
    return () => {
      cancelled = true;
      if (renderFrameRef.current !== undefined) {
        cancelAnimationFrame(renderFrameRef.current);
      }
      try {
        labRef.current?.shutdownExactWorker();
      } catch {
        // The browser transport below remains the final lifecycle backstop.
      }
      workerTransportRef.current?.terminate();
      labRef.current?.free();
      workerTransportRef.current = undefined;
      labRef.current = undefined;
      visualAssetsRef.current = undefined;
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
    const assets = visualAssetsRef.current;
    const transport = workerTransportRef.current;
    if (!initialized || !lab || !assets || !transport) {
      return;
    }
    let cancelled = false;
    let pumpFrame = 0;
    try {
      const report = parseJson<CanonicalCoordinatorReport>(
        lab.beginExactCoverage(
          transport,
          assets.authored,
          optionalReferenceBytes(assets),
          assets.provisional,
          assets.diagnostic,
          coverageCenterChunkX * 16,
          coverageCenterChunkZ * 16,
          state.canonicalRadius,
          state.profile,
          visualProfile,
          texturePresentation,
          state.seed,
          state.canonicalStage,
          state.waterVisible,
          state.vegetationVisible,
          cacheEnabled,
          cacheEpoch,
        ),
      );
      publishReport(report);
      scheduleRenderRef.current();
      if (report.needsPump) {
        pumpFrame = requestAnimationFrame(pump);
      }
    } catch (error: unknown) {
      onError(errorMessage(error));
    }

    return () => {
      cancelled = true;
      if (pumpFrame !== 0) {
        cancelAnimationFrame(pumpFrame);
      }
    };

    function pump(): void {
      pumpFrame = 0;
      if (cancelled) {
        return;
      }
      try {
        const report = parseJson<CanonicalCoordinatorReport>(
          lab!.pumpExactCoverage(),
        );
        publishReport(report);
        if (report.renderChanged) {
          scheduleRenderRef.current();
        }
        if (report.error) {
          onError(report.error);
          return;
        }
        if (report.needsPump) {
          pumpFrame = requestAnimationFrame(pump);
        }
      } catch (error: unknown) {
        onError(errorMessage(error));
      }
    }

    function publishReport(report: CanonicalCoordinatorReport): void {
      setLatestReport(report);
      onReport(report);
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
