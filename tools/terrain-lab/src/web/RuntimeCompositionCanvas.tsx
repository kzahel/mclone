import { useEffect, useRef, useState } from "react";

import {
  PolledWorkerTransport,
} from "../../../../native/apps/mclone-web-client/www/mclone-worker-transport";
import type {
  TerrainRuntimeCompositionLab,
} from "../../generated/pkg/mclone_terrain_lab";
import {
  mclone_terrain_lab_create_runtime_composition,
} from "../../generated/pkg/mclone_terrain_lab";

import type {
  TerrainLabCamera,
  TerrainLabState,
  TerrainLabTexturePresentation,
  TerrainLabVisualProfile,
} from "../state";
import { initializeTerrainLab } from "./terrain-lab-wasm";
import {
  loadTerrainVisualAssets,
  optionalReferenceBytes,
} from "./visual-assets";
import { terrainCanvasBackingSize } from "./canvas-size";
import { useWorldViewNavigation } from "./use-world-view-navigation";

export interface RuntimeCompositionReport {
  exactDesiredChunks: number;
  exactPaintedChunks: number;
  exactQueuedChunks: number;
  exactPendingAdmissions: number;
  exactInFlight: boolean;
  exactComplete: boolean;
  exactNaturalTreeRecords: number;
  exactOwnedTreeRecords: number;
  proxyOwnedTreeRecords: number;
  exactVertexCount: number;
  exactIndexCount: number;
  horizonReadySlots: number;
  horizonPendingRefills: number;
  vegetationReadyTiles: number;
  vegetationPendingTiles: number;
  treeInstances: number;
  treeProxySuppressedRecords: number;
  coarseReady: boolean;
  targetReady: boolean;
  needsRedraw: boolean;
}

interface RuntimeCompositionCanvasProps {
  state: TerrainLabState;
  visualProfile: TerrainLabVisualProfile;
  texturePresentation: TerrainLabTexturePresentation;
  camera: TerrainLabCamera;
  onStateChange: (state: TerrainLabState) => void;
  onCameraChange: (camera: TerrainLabCamera) => void;
  onReport: (report: RuntimeCompositionReport) => void;
  onError: (error: string | undefined) => void;
}

export function RuntimeCompositionCanvas({
  state,
  visualProfile,
  texturePresentation,
  camera,
  onStateChange,
  onCameraChange,
  onReport,
  onError,
}: RuntimeCompositionCanvasProps): React.JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const labRef = useRef<TerrainRuntimeCompositionLab | undefined>(undefined);
  const renderFrameRef = useRef<number | undefined>(undefined);
  const currentViewRef = useRef({ state, camera });
  currentViewRef.current = { state, camera };
  const [initialized, setInitialized] = useState(false);
  const [activeProfile, setActiveProfile] = useState<string>();
  const [canvasSize, setCanvasSize] = useState({ width: 900, height: 700 });
  const [latestReport, setLatestReport] = useState<RuntimeCompositionReport>();
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
  const scheduleRenderRef = useRef<() => void>(() => undefined);
  scheduleRenderRef.current = (): void => {
    if (renderFrameRef.current !== undefined) {
      return;
    }
    renderFrameRef.current = requestAnimationFrame((timestamp) => {
      renderFrameRef.current = undefined;
      const lab = labRef.current;
      if (!lab) {
        return;
      }
      const current = currentViewRef.current;
      try {
        const report = JSON.parse(lab.renderFrame(
          timestamp,
          current.state.centerX,
          current.state.centerZ,
          current.state.blocksAcross,
          current.state.view,
          current.camera.yaw,
          current.camera.pitch,
          current.state.projection,
        )) as RuntimeCompositionReport;
        setLatestReport(report);
        onReport(report);
        if (report.needsRedraw) {
          scheduleRenderRef.current();
        }
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
    setActiveProfile(undefined);
    setLatestReport(undefined);
    void Promise.all([
      initializeTerrainLab(),
      loadTerrainVisualAssets(),
    ]).then(async ([, assets]) => {
      if (cancelled) {
        return;
      }
      const initial = currentViewRef.current;
      const lab = await mclone_terrain_lab_create_runtime_composition(
        canvas,
        assets.authored,
        optionalReferenceBytes(assets),
        assets.provisional,
        assets.diagnostic,
        visualProfile,
        texturePresentation,
        initial.state.profile,
        initial.state.seed,
        initial.state.canonicalRadius,
        initial.state.centerX,
        initial.state.centerZ,
        initial.state.blocksAcross,
        initial.state.view,
        initial.camera.yaw,
        initial.camera.pitch,
        initial.state.projection,
        () => new PolledWorkerTransport(new Worker(
          new URL("./runtime-vegetation-worker.ts", import.meta.url),
          {
            type: "module",
            name: "mclone-terrain-runtime-vegetation",
          },
        )),
        () => new PolledWorkerTransport(new Worker(
          new URL("./runtime-exact-worker.ts", import.meta.url),
          {
            type: "module",
            name: "mclone-terrain-runtime-exact",
          },
        )),
      );
      if (cancelled) {
        lab.shutdown();
        lab.free();
        return;
      }
      labRef.current = lab;
      setActiveProfile(initial.state.profile);
      setInitialized(true);
      scheduleRenderRef.current();
    }).catch((error: unknown) => {
      if (!cancelled) {
        onError(errorMessage(error));
      }
    });
    return () => {
      cancelled = true;
      if (renderFrameRef.current !== undefined) {
        cancelAnimationFrame(renderFrameRef.current);
        renderFrameRef.current = undefined;
      }
      try {
        labRef.current?.shutdown();
      } finally {
        labRef.current?.free();
        labRef.current = undefined;
      }
    };
  }, [
    onError,
    state.canonicalRadius,
    state.profile,
    state.seed,
    texturePresentation,
    visualProfile,
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
    lab.resize(canvasSize.width, canvasSize.height);
    scheduleRenderRef.current();
  }, [canvasSize, initialized]);

  useEffect(() => {
    if (initialized) {
      scheduleRenderRef.current();
    }
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

  return (
    <div
      ref={stageRef}
      className="terrainStage runtimeCompositionStage"
      tabIndex={0}
      data-testid="runtime-composition-stage"
      data-runtime-profile={state.profile}
      data-runtime-active-profile={activeProfile ?? ""}
      data-runtime-target-ready={latestReport?.targetReady ? "true" : "false"}
      data-runtime-exact-complete={latestReport?.exactComplete ? "true" : "false"}
      data-runtime-exact-painted={latestReport?.exactPaintedChunks ?? 0}
      data-runtime-exact-desired={latestReport?.exactDesiredChunks ?? 0}
      data-runtime-proxy-trees={latestReport?.proxyOwnedTreeRecords ?? 0}
      onPointerDown={navigation.onPointerDown}
      onPointerMove={navigation.onPointerMove}
      onPointerUp={navigation.onPointerUp}
      onPointerCancel={navigation.onPointerCancel}
      onKeyDown={navigation.onKeyDown}
    >
      <canvas
        ref={canvasRef}
        className="terrainCanvas"
        width={canvasSize.width}
        height={canvasSize.height}
        aria-label="Runtime composed exact and procedural terrain"
      />
      <div className="canvasTopline">
        <span className="canvasBadge primary">Runtime composed</span>
        <span className="canvasBadge">
          exact {latestReport?.exactPaintedChunks ?? 0}/
          {latestReport?.exactDesiredChunks ?? 0}
        </span>
        <span className="canvasBadge">
          horizon {latestReport?.horizonReadySlots ?? 0} ready
        </span>
        <span className="canvasBadge">
          trees {latestReport?.exactOwnedTreeRecords ?? 0} exact ·{" "}
          {latestReport?.proxyOwnedTreeRecords ?? 0} proxy
        </span>
      </div>
      <div className="canvasHint">
        Shared camera · shared reversed-Z depth · focus-centered exact coverage
      </div>
    </div>
  );
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
