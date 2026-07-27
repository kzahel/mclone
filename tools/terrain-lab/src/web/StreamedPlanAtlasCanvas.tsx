import { useEffect, useMemo, useRef, useState } from "react";

import type {
  TerrainLabCamera,
  TerrainLabState,
} from "../state";
import type {
  AtlasCandidateReceipt,
  AtlasRegionIdentity,
  FeatureGraphAtlas,
  FallbackAtlas,
  HierarchyAtlas,
  MultiscaleWitnessAtlas,
  StreamedPlanAtlasSummary,
  StreamedPlanAtlasWorkerResponse,
} from "./streamed-plan-atlas-worker-protocol";
import { initializeTerrainLab } from "./terrain-lab-wasm";
import { useWorldViewNavigation } from "./use-world-view-navigation";

export interface StreamedPlanAtlasReport {
  schema: string;
  topology: string;
  queryMs: number;
  phaseTwoWitnessSha256: string;
  coverageClipped: boolean;
  fallback: AtlasCandidateReceipt;
  hierarchy: AtlasCandidateReceipt;
  featureGraph: AtlasCandidateReceipt;
  multiscaleWitness: MultiscaleWitnessAtlas;
}

interface StreamedPlanAtlasCanvasProps {
  state: TerrainLabState;
  camera: TerrainLabCamera;
  cacheEpoch: number;
  onStateChange: (state: TerrainLabState) => void;
  onCameraChange: (camera: TerrainLabCamera) => void;
  onReport: (report: StreamedPlanAtlasReport | undefined) => void;
  onError: (error: string | undefined) => void;
}

interface CanvasSize {
  width: number;
  height: number;
  cssWidth: number;
  cssHeight: number;
  dpr: number;
}

interface AtlasPanel {
  x: number;
  y: number;
  width: number;
  height: number;
  title: string;
}

interface AtlasView {
  minX: number;
  minZ: number;
  maxX: number;
  maxZ: number;
  width: number;
  height: number;
}

export function StreamedPlanAtlasCanvas({
  state,
  camera,
  cacheEpoch,
  onStateChange,
  onCameraChange,
  onReport,
  onError,
}: StreamedPlanAtlasCanvasProps): React.JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const workerRef = useRef<Worker | undefined>(undefined);
  const revisionRef = useRef(0);
  const cacheEpochRef = useRef(cacheEpoch);
  const [summary, setSummary] = useState<StreamedPlanAtlasSummary>();
  const [queryMs, setQueryMs] = useState(0);
  const [navigationReady, setNavigationReady] = useState(false);
  const [canvasSize, setCanvasSize] = useState<CanvasSize>({
    width: 1_200,
    height: 700,
    cssWidth: 1_200,
    cssHeight: 700,
    dpr: 1,
  });
  const navigationState = useMemo<TerrainLabState>(
    () => ({ ...state, view: "map" }),
    [state],
  );
  const navigation = useWorldViewNavigation({
    stageRef,
    enabled: navigationReady,
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
      new URL("./streamed-plan-atlas-worker.ts", import.meta.url),
      { type: "module" },
    );
    workerRef.current = worker;
    worker.onmessage = (
      event: MessageEvent<StreamedPlanAtlasWorkerResponse>,
    ): void => {
      const response = event.data;
      if (response.revision !== revisionRef.current) {
        return;
      }
      if (response.type === "error") {
        onError(response.message);
        return;
      }
      setSummary(response.summary);
      setQueryMs(response.queryMs);
      onReport(reportFromSummary(response.summary, response.queryMs));
    };
    worker.onerror = (event): void => {
      onError(event.message || "Streamed-plan atlas Worker failed.");
    };
    return () => {
      worker.terminate();
      workerRef.current = undefined;
    };
  }, [onError, onReport]);

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
    const worker = workerRef.current;
    if (!worker) {
      return;
    }
    const panels = atlasPanels(canvasSize);
    const panel = panels[0]!;
    const aspectRatio = panel.width / Math.max(panel.height, 1);
    const clearCache = cacheEpoch !== cacheEpochRef.current;
    cacheEpochRef.current = cacheEpoch;
    revisionRef.current += 1;
    const revision = revisionRef.current;
    onReport(undefined);
    const timer = window.setTimeout(() => {
      worker.postMessage({
        type: "query",
        revision,
        seed: state.seed,
        topology: state.atlasTopology,
        centerX: state.centerX,
        centerZ: state.centerZ,
        blocksAcross: state.blocksAcross,
        aspectRatio,
        clearCache,
      });
    }, 45);
    return () => window.clearTimeout(timer);
  }, [
    cacheEpoch,
    canvasSize.cssHeight,
    canvasSize.cssWidth,
    state.atlasTopology,
    state.blocksAcross,
    state.centerX,
    state.centerZ,
    state.seed,
    onReport,
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
    drawAtlas(context, canvasSize, state, summary, queryMs);
  }, [
    canvasSize,
    onError,
    queryMs,
    state.atlasFacetsVisible,
    state.atlasFallbackFeaturesVisible,
    state.atlasFallbackSamplesVisible,
    state.atlasGraphBoundsVisible,
    state.atlasGraphEdgesVisible,
    state.atlasHierarchyVisible,
    state.atlasIdentityVisible,
    state.atlasRegionsVisible,
    state.atlasSeamsVisible,
    state.atlasTopology,
    state.atlasWitnessBoundsVisible,
    state.atlasWitnessLocalVisible,
    state.atlasWitnessParentVisible,
    state.atlasWitnessRegionalVisible,
    state.blocksAcross,
    state.centerX,
    state.centerZ,
    summary,
  ]);

  return (
    <div
      ref={stageRef}
      className="terrainStage streamedPlanAtlasStage"
      data-testid="streamed-plan-atlas-stage"
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
        aria-label="Streamed planner structural atlas"
      />
      <div className="canvasTopline" aria-hidden="true">
        <span className="canvasBadge primary">
          {summary ? "atlas ready" : "querying atlas"}
        </span>
        <span className="canvasBadge">research · production disconnected</span>
        <span className="canvasBadge">{topologyLabel(state.atlasTopology)}</span>
        {summary?.coverage.clipped ? (
          <span className="canvasBadge warning">coverage cost-capped</span>
        ) : null}
      </div>
      <div className="canvasHint">
        Drag to query new canonical regions · wheel or pinch to zoom
      </div>
    </div>
  );
}

function drawAtlas(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  state: TerrainLabState,
  summary: StreamedPlanAtlasSummary | undefined,
  queryMs: number,
): void {
  context.setTransform(1, 0, 0, 1, 0, 0);
  context.fillStyle = "#061012";
  context.fillRect(0, 0, canvas.width, canvas.height);
  const panels = atlasPanels(canvas);
  if (!summary) {
    for (const panel of panels) {
      drawLoadingPanel(context, canvas, panel);
    }
    return;
  }
  const drawPanel = (
    panel: AtlasPanel,
    draw: () => void,
  ): void => {
    context.save();
    context.beginPath();
    context.rect(panel.x, panel.y, panel.width, panel.height);
    context.clip();
    draw();
    context.restore();
  };
  drawPanel(
    panels[0]!,
    () => drawFallbackPanel(
      context,
      canvas,
      panels[0]!,
      state,
      summary,
      summary.fallback,
    ),
  );
  drawPanel(
    panels[1]!,
    () => drawHierarchyPanel(
      context,
      canvas,
      panels[1]!,
      state,
      summary,
      summary.hierarchy,
    ),
  );
  drawPanel(
    panels[2]!,
    () => drawGraphPanel(
      context,
      canvas,
      panels[2]!,
      state,
      summary,
      summary.featureGraph,
    ),
  );
  drawPanel(
    panels[3]!,
    () => drawMultiscaleWitnessPanel(
      context,
      canvas,
      panels[3]!,
      state,
      summary,
      summary.multiscaleWitness,
    ),
  );
  const receipts = [
    summary.fallback,
    summary.hierarchy,
    summary.featureGraph,
    summary.multiscaleWitness,
  ];
  for (const [index, panel] of panels.entries()) {
    const receipt = receipts[index]!;
    drawPanelChrome(context, canvas, panel, receipt, queryMs, summary.coverage.clipped);
  }
}

function drawFallbackPanel(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  panel: AtlasPanel,
  state: TerrainLabState,
  summary: StreamedPlanAtlasSummary,
  atlas: FallbackAtlas,
): void {
  const view = panelView(panel, canvas, state);
  drawPanelBackground(context, panel);
  if (state.atlasRegionsVisible) {
    for (const cell of atlas.cells) {
      fillRegion(
        context,
        panel,
        view,
        cell,
        FALLBACK_PALETTE[cell.regionClass % FALLBACK_PALETTE.length]!,
        summary.baseRegionBlocks,
      );
    }
  }
  if (state.atlasFallbackFeaturesVisible) {
    context.save();
    context.strokeStyle = "rgba(255, 196, 94, 0.82)";
    context.lineWidth = Math.max(1, canvas.dpr);
    context.setLineDash([4 * canvas.dpr, 3 * canvas.dpr]);
    for (const feature of atlas.features) {
      const [x, y] = worldToPanel(panel, view, feature.worldX, feature.worldZ);
      const radius = feature.radiusBlocks / view.width * panel.width;
      context.beginPath();
      context.arc(x, y, Math.max(2, radius), 0, Math.PI * 2);
      context.stroke();
    }
    context.restore();
  }
  if (state.atlasFallbackSamplesVisible) {
    for (const sample of atlas.samples) {
      const [x, y] = worldToPanel(panel, view, sample.worldX, sample.worldZ);
      if (!pointInside(panel, x, y)) {
        continue;
      }
      const height = Math.max(0, Math.min(1, (sample.surfaceY - 40) / 100));
      context.fillStyle = sample.channel
        ? "#4ec9ff"
        : sample.pond
          ? "#59e5dc"
          : sample.island
            ? "#f4d66d"
            : `hsl(${92 - height * 48} 34% ${42 + height * 28}%)`;
      context.beginPath();
      context.arc(x, y, Math.max(2, 2.5 * canvas.dpr), 0, Math.PI * 2);
      context.fill();
      if (sample.waterSurfaceY !== undefined) {
        context.strokeStyle = "rgba(97, 205, 255, 0.88)";
        context.lineWidth = Math.max(1, canvas.dpr);
        context.stroke();
      }
    }
  }
  drawSharedOverlays(context, canvas, panel, view, state, summary, atlas.cells);
}

function drawHierarchyPanel(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  panel: AtlasPanel,
  state: TerrainLabState,
  summary: StreamedPlanAtlasSummary,
  atlas: HierarchyAtlas,
): void {
  const view = panelView(panel, canvas, state);
  drawPanelBackground(context, panel);
  if (state.atlasRegionsVisible) {
    for (const cell of atlas.cells) {
      const mixed = (cell.rootTendency * 0.55 + cell.midTendency * 0.45) / 2_048;
      const light = 31 + Math.round((mixed + 1) * 11);
      fillRegion(
        context,
        panel,
        view,
        cell,
        `hsl(${165 + cell.character * 24} 28% ${light}%)`,
        summary.baseRegionBlocks,
      );
    }
  }
  if (state.atlasHierarchyVisible) {
    const ordered = [...atlas.providers].sort((left, right) => right.level - left.level);
    for (const provider of ordered) {
      const [left, top] = worldToPanel(
        panel,
        view,
        provider.worldMinX,
        provider.worldMinZ,
      );
      const [right, bottom] = worldToPanel(
        panel,
        view,
        provider.worldMinX + provider.extentBlocks,
        provider.worldMinZ + provider.extentBlocks,
      );
      context.save();
      context.strokeStyle = provider.level === 2
        ? "rgba(224, 126, 255, 0.86)"
        : "rgba(82, 231, 202, 0.72)";
      context.lineWidth = (provider.level === 2 ? 2 : 1) * canvas.dpr;
      context.setLineDash(
        provider.level === 2
          ? [8 * canvas.dpr, 5 * canvas.dpr]
          : [3 * canvas.dpr, 3 * canvas.dpr],
      );
      context.strokeRect(left, top, right - left, bottom - top);
      context.restore();
    }
  }
  if (state.atlasFacetsVisible) {
    for (const facet of atlas.facets) {
      const [aX, aY] = worldToPanel(panel, view, facet.worldAX, facet.worldAZ);
      const [bX, bY] = worldToPanel(panel, view, facet.worldBX, facet.worldBZ);
      context.strokeStyle = facet.open
        ? "rgba(204, 255, 109, 0.92)"
        : "rgba(255, 111, 102, 0.42)";
      context.lineWidth = Math.max(
        canvas.dpr,
        (facet.open ? 0.75 + facet.strength * 0.35 : 0.75) * canvas.dpr,
      );
      context.beginPath();
      context.moveTo(aX, aY);
      context.lineTo(bX, bY);
      context.stroke();
    }
  }
  drawSharedOverlays(context, canvas, panel, view, state, summary, atlas.cells);
}

function drawGraphPanel(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  panel: AtlasPanel,
  state: TerrainLabState,
  summary: StreamedPlanAtlasSummary,
  atlas: FeatureGraphAtlas,
): void {
  const view = panelView(panel, canvas, state);
  drawPanelBackground(context, panel);
  if (state.atlasRegionsVisible) {
    for (const cell of atlas.cells) {
      const light = 27 + Math.min(18, cell.acceptedGraphs * 4);
      fillRegion(
        context,
        panel,
        view,
        cell,
        `hsl(${205 + cell.acceptedGraphs * 12} 32% ${light}%)`,
        summary.baseRegionBlocks,
      );
    }
  }
  for (const graph of atlas.graphs) {
    if (state.atlasGraphBoundsVisible) {
      const [left, top] = worldToPanel(
        panel,
        view,
        graph.boundsMinX,
        graph.boundsMinZ,
      );
      const [right, bottom] = worldToPanel(
        panel,
        view,
        graph.boundsMaxX,
        graph.boundsMaxZ,
      );
      context.save();
      context.strokeStyle = "rgba(233, 151, 255, 0.4)";
      context.lineWidth = Math.max(1, canvas.dpr);
      context.setLineDash([3 * canvas.dpr, 3 * canvas.dpr]);
      context.strokeRect(left, top, right - left, bottom - top);
      context.restore();
    }
    if (state.atlasGraphEdgesVisible) {
      for (const edge of graph.edges) {
        const from = graph.nodes.find((node) => node.index === edge.from);
        const to = graph.nodes.find((node) => node.index === edge.to);
        if (!from || !to) {
          continue;
        }
        const [aX, aY] = worldToPanel(panel, view, from.worldX, from.worldZ);
        const [bX, bY] = worldToPanel(panel, view, to.worldX, to.worldZ);
        context.strokeStyle = "rgba(96, 214, 255, 0.88)";
        context.lineWidth = Math.max(canvas.dpr, edge.width * 0.45 * canvas.dpr);
        context.lineCap = "round";
        context.beginPath();
        context.moveTo(aX, aY);
        context.lineTo(bX, bY);
        context.stroke();
      }
      for (const node of graph.nodes) {
        const [x, y] = worldToPanel(panel, view, node.worldX, node.worldZ);
        const sink = node.index === graph.sinkNode;
        context.fillStyle = sink ? "#ff65d5" : "#d7f7ff";
        context.beginPath();
        context.arc(x, y, (sink ? 3.5 : 2.2) * canvas.dpr, 0, Math.PI * 2);
        context.fill();
      }
    }
  }
  drawSharedOverlays(context, canvas, panel, view, state, summary, atlas.cells);
}

function drawMultiscaleWitnessPanel(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  panel: AtlasPanel,
  state: TerrainLabState,
  summary: StreamedPlanAtlasSummary,
  atlas: MultiscaleWitnessAtlas,
): void {
  const view = panelView(panel, canvas, state);
  drawPanelBackground(context, panel);
  const visible = (level: number): boolean => {
    switch (level) {
      case 2:
        return state.atlasWitnessParentVisible;
      case 1:
        return state.atlasWitnessRegionalVisible;
      case 0:
        return state.atlasWitnessLocalVisible;
      default:
        return false;
    }
  };
  const ordered = [...atlas.features].sort((left, right) => right.level - left.level);
  if (state.atlasWitnessBoundsVisible) {
    for (const feature of ordered) {
      if (!visible(feature.level)) {
        continue;
      }
      const [left, top] = worldToPanel(
        panel,
        view,
        feature.boundsMinX,
        feature.boundsMinZ,
      );
      const [right, bottom] = worldToPanel(
        panel,
        view,
        feature.boundsMaxX,
        feature.boundsMaxZ,
      );
      context.save();
      context.strokeStyle = witnessLevelColor(feature.level, 0.28);
      context.lineWidth = Math.max(1, canvas.dpr);
      context.setLineDash(witnessLevelDash(feature.level, canvas.dpr));
      context.strokeRect(left, top, right - left, bottom - top);
      context.restore();
    }
  }
  for (const feature of ordered) {
    if (!visible(feature.level)) {
      continue;
    }
    const [startX, startY] = worldToPanel(
      panel,
      view,
      feature.startX,
      feature.startZ,
    );
    const [endX, endY] = worldToPanel(
      panel,
      view,
      feature.endX,
      feature.endZ,
    );
    context.save();
    context.strokeStyle = witnessLevelColor(
      feature.level,
      feature.family === "basin-route" ? 0.94 : 0.78,
    );
    context.lineWidth = Math.max(
      canvas.dpr,
      Math.min(
        6 * canvas.dpr,
        feature.width / view.width * panel.width,
      ),
    );
    context.lineCap = "round";
    if (feature.family === "range-axis") {
      context.setLineDash(witnessLevelDash(feature.level, canvas.dpr));
    }
    context.beginPath();
    context.moveTo(startX, startY);
    context.lineTo(endX, endY);
    context.stroke();
    context.restore();
    if (feature.terminalKind !== 0) {
      context.fillStyle = "#ff70d7";
      context.beginPath();
      context.arc(endX, endY, (feature.level + 2.4) * canvas.dpr, 0, Math.PI * 2);
      context.fill();
    }
  }
  drawSharedOverlays(
    context,
    canvas,
    panel,
    view,
    state,
    summary,
    summary.fallback.cells,
  );
}

function witnessLevelColor(level: number, alpha: number): string {
  switch (level) {
    case 2:
      return `rgba(229, 137, 255, ${alpha})`;
    case 1:
      return `rgba(79, 220, 255, ${alpha})`;
    default:
      return `rgba(195, 239, 102, ${alpha})`;
  }
}

function witnessLevelDash(level: number, dpr: number): number[] {
  switch (level) {
    case 2:
      return [10 * dpr, 5 * dpr];
    case 1:
      return [5 * dpr, 3 * dpr];
    default:
      return [2 * dpr, 2 * dpr];
  }
}

function drawSharedOverlays(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  panel: AtlasPanel,
  view: AtlasView,
  state: TerrainLabState,
  summary: StreamedPlanAtlasSummary,
  cells: AtlasRegionIdentity[],
): void {
  if (state.atlasRegionsVisible) {
    context.strokeStyle = "rgba(226, 238, 232, 0.16)";
    context.lineWidth = Math.max(1, canvas.dpr);
    for (const cell of cells) {
      const [left, top] = worldToPanel(panel, view, cell.worldMinX, cell.worldMinZ);
      const [right, bottom] = worldToPanel(
        panel,
        view,
        cell.worldMinX + summary.baseRegionBlocks,
        cell.worldMinZ + summary.baseRegionBlocks,
      );
      context.strokeRect(left, top, right - left, bottom - top);
    }
  }
  if (state.atlasIdentityVisible) {
    const cellPixels = summary.baseRegionBlocks / view.width * panel.width;
    if (cellPixels >= 48 * canvas.dpr) {
      context.fillStyle = "rgba(229, 240, 234, 0.68)";
      context.font = `${Math.max(8, 8 * canvas.dpr)}px "DM Mono", monospace`;
      for (const cell of cells) {
        const [x, y] = worldToPanel(
          panel,
          view,
          cell.worldMinX + 34,
          cell.worldMinZ + 54,
        );
        context.fillText(
          `${cell.requestedRegionX},${cell.requestedRegionZ} → `
            + `${cell.canonicalRegionX},${cell.canonicalRegionZ}`,
          x,
          y,
        );
      }
    }
  }
  if (state.atlasSeamsVisible && summary.periodBlocks) {
    drawPeriodicSeams(
      context,
      canvas,
      panel,
      view,
      summary.periodBlocks,
      summary.topology.includes("torus"),
    );
  }
  const [coverageLeft, coverageTop] = worldToPanel(
    panel,
    view,
    summary.coverage.minX,
    summary.coverage.minZ,
  );
  const [coverageRight, coverageBottom] = worldToPanel(
    panel,
    view,
    summary.coverage.maxX,
    summary.coverage.maxZ,
  );
  context.save();
  context.strokeStyle = summary.coverage.clipped
    ? "rgba(255, 179, 77, 0.88)"
    : "rgba(223, 238, 229, 0.34)";
  context.lineWidth = Math.max(1, canvas.dpr);
  context.setLineDash([7 * canvas.dpr, 5 * canvas.dpr]);
  context.strokeRect(
    coverageLeft,
    coverageTop,
    coverageRight - coverageLeft,
    coverageBottom - coverageTop,
  );
  context.restore();
}

function drawPeriodicSeams(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  panel: AtlasPanel,
  view: AtlasView,
  period: number,
  periodicZ: boolean,
): void {
  context.save();
  context.strokeStyle = "rgba(255, 214, 95, 0.84)";
  context.lineWidth = Math.max(1, 1.4 * canvas.dpr);
  context.setLineDash([10 * canvas.dpr, 5 * canvas.dpr]);
  for (
    let x = Math.ceil(view.minX / period) * period;
    x <= view.maxX;
    x += period
  ) {
    const [screenX] = worldToPanel(panel, view, x, view.minZ);
    context.beginPath();
    context.moveTo(screenX, panel.y);
    context.lineTo(screenX, panel.y + panel.height);
    context.stroke();
  }
  if (periodicZ) {
    for (
      let z = Math.ceil(view.minZ / period) * period;
      z <= view.maxZ;
      z += period
    ) {
      const [, screenY] = worldToPanel(panel, view, view.minX, z);
      context.beginPath();
      context.moveTo(panel.x, screenY);
      context.lineTo(panel.x + panel.width, screenY);
      context.stroke();
    }
  }
  context.restore();
}

function fillRegion(
  context: CanvasRenderingContext2D,
  panel: AtlasPanel,
  view: AtlasView,
  region: AtlasRegionIdentity,
  color: string,
  extent: number,
): void {
  const [left, top] = worldToPanel(panel, view, region.worldMinX, region.worldMinZ);
  const [right, bottom] = worldToPanel(
    panel,
    view,
    region.worldMinX + extent,
    region.worldMinZ + extent,
  );
  context.fillStyle = color;
  context.fillRect(
    Math.floor(left),
    Math.floor(top),
    Math.ceil(right - left) + 1,
    Math.ceil(bottom - top) + 1,
  );
}

function drawPanelChrome(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  panel: AtlasPanel,
  receipt: AtlasCandidateReceipt,
  queryMs: number,
  clipped: boolean,
): void {
  context.save();
  context.strokeStyle = "rgba(226, 240, 233, 0.18)";
  context.lineWidth = Math.max(1, canvas.dpr);
  context.strokeRect(panel.x, panel.y, panel.width, panel.height);
  context.fillStyle = "rgba(4, 13, 15, 0.82)";
  context.fillRect(panel.x, panel.y, panel.width, 58 * canvas.dpr);
  context.fillStyle = "#e5eee9";
  context.font = `600 ${12 * canvas.dpr}px Manrope, sans-serif`;
  context.fillText(panel.title, panel.x + 12 * canvas.dpr, panel.y + 22 * canvas.dpr);
  context.fillStyle = clipped ? "#ffc66d" : "rgba(206, 223, 215, 0.72)";
  context.font = `${8 * canvas.dpr}px "DM Mono", monospace`;
  context.fillText(
    `${receipt.canonicalPlanCount} canonical / ${receipt.viewedPlanCount} viewed · `
      + `${receipt.cacheHits} hit ${receipt.cacheMisses} miss · ${queryMs.toFixed(1)} ms`,
    panel.x + 12 * canvas.dpr,
    panel.y + 41 * canvas.dpr,
  );
  context.fillStyle = "rgba(183, 220, 108, 0.68)";
  context.fillText(
    receipt.semanticSha256.slice(0, 12),
    panel.x + 12 * canvas.dpr,
    panel.y + panel.height - 12 * canvas.dpr,
  );
  context.restore();
}

function drawPanelBackground(
  context: CanvasRenderingContext2D,
  panel: AtlasPanel,
): void {
  context.fillStyle = "#081416";
  context.fillRect(panel.x, panel.y, panel.width, panel.height);
}

function drawLoadingPanel(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  panel: AtlasPanel,
): void {
  drawPanelBackground(context, panel);
  context.strokeStyle = "rgba(183, 220, 108, 0.07)";
  context.lineWidth = Math.max(1, canvas.dpr);
  const spacing = 48 * canvas.dpr;
  for (let x = panel.x; x < panel.x + panel.width; x += spacing) {
    context.beginPath();
    context.moveTo(x, panel.y);
    context.lineTo(x, panel.y + panel.height);
    context.stroke();
  }
  for (let y = panel.y; y < panel.y + panel.height; y += spacing) {
    context.beginPath();
    context.moveTo(panel.x, y);
    context.lineTo(panel.x + panel.width, y);
    context.stroke();
  }
}

function atlasPanels(canvas: CanvasSize): AtlasPanel[] {
  const titles = [
    "A · Coordinate-pure fallback",
    "B · Hierarchical shared facts",
    "C · Feature-owned bounded graphs",
    "D · Semantic parent → child refinement",
  ];
  const top = 60 * canvas.dpr;
  const availableHeight = Math.max(canvas.dpr, canvas.height - top);
  if (canvas.cssWidth >= 840) {
    const width = canvas.width / 2;
    const height = availableHeight / 2;
    return titles.map((title, index) => ({
      x: (index % 2) * width,
      y: top + Math.floor(index / 2) * height,
      width,
      height,
      title,
    }));
  }
  const height = availableHeight / 4;
  return titles.map((title, index) => ({
    x: 0,
    y: top + index * height,
    width: canvas.width,
    height,
    title,
  }));
}

function panelView(
  panel: AtlasPanel,
  canvas: CanvasSize,
  state: TerrainLabState,
): AtlasView {
  const width = state.blocksAcross;
  const panelCssWidth = panel.width / canvas.dpr;
  const panelCssHeight = panel.height / canvas.dpr;
  const height = width * panelCssHeight / Math.max(panelCssWidth, 1);
  return {
    minX: state.centerX - width * 0.5,
    minZ: state.centerZ - height * 0.5,
    maxX: state.centerX + width * 0.5,
    maxZ: state.centerZ + height * 0.5,
    width,
    height,
  };
}

function worldToPanel(
  panel: AtlasPanel,
  view: AtlasView,
  worldX: number,
  worldZ: number,
): [number, number] {
  return [
    panel.x + (worldX - view.minX) / view.width * panel.width,
    panel.y + (worldZ - view.minZ) / view.height * panel.height,
  ];
}

function pointInside(
  panel: AtlasPanel,
  x: number,
  y: number,
): boolean {
  return x >= panel.x
    && x <= panel.x + panel.width
    && y >= panel.y
    && y <= panel.y + panel.height;
}

function reportFromSummary(
  summary: StreamedPlanAtlasSummary,
  queryMs: number,
): StreamedPlanAtlasReport {
  return {
    schema: summary.schema,
    topology: summary.topology,
    queryMs,
    phaseTwoWitnessSha256: summary.phaseTwoWitnessSha256,
    coverageClipped: summary.coverage.clipped,
    fallback: summary.fallback,
    hierarchy: summary.hierarchy,
    featureGraph: summary.featureGraph,
    multiscaleWitness: summary.multiscaleWitness,
  };
}

function topologyLabel(topology: TerrainLabState["atlasTopology"]): string {
  switch (topology) {
    case "plane":
      return "plane · unbounded";
    case "cylinder-x":
      return "cylinder X · 6.144 km";
    case "torus":
      return "torus · 6.144 km²";
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

const FALLBACK_PALETTE = [
  "#2c4a43",
  "#39495e",
  "#4d443a",
  "#3a5350",
  "#51445a",
  "#49533b",
];
