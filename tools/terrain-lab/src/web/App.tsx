import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  DEFAULT_TERRAIN_LAB_CAMERA,
  REVIEW_TERRAIN_LAB_STATE,
  TERRAIN_LAB_CANONICAL_RADII,
  TERRAIN_LAB_SPACINGS,
  footprintBlocks,
  parseTerrainLabState,
  parseTerrainLabReviewCamera,
  proceduralSourceForPanes,
  switchTerrainLabProfile,
  terrainLabPlayHref,
  terrainLabSearch,
  toggleTerrainLabPane,
  validSeed,
  type CanonicalTerrainStage,
  type TerrainLabDetail,
  type TerrainLabContentStage,
  type TerrainLabLayer,
  type TerrainLabCamera,
  type TerrainLabPane,
  type TerrainLabProfile,
  type TerrainLabProjection,
  type StreamedPlanAtlasTopology,
  type SemanticTerrainCorrection,
  type SemanticTerrainFeatures,
  type SemanticTerrainSubstrate,
  type SemanticTerrainVerticalScale,
  type TerrainLabComparisonVisualProfile,
  type TerrainLabState,
  type TerrainLabTexturePresentation,
  type TerrainLabView,
  type TerrainLabVisualProfile,
} from "../state";
import {
  CanonicalTerrainCanvas,
  type CanonicalTerrainReport,
} from "./CanonicalTerrainCanvas";
import {
  RuntimeCompositionCanvas,
  type RuntimeCompositionReport,
} from "./RuntimeCompositionCanvas";
import {
  LandformPlanCanvas,
  type LandformPlanReport,
} from "./LandformPlanCanvas";
import {
  StreamedPlanAtlasCanvas,
  type StreamedPlanAtlasReport,
} from "./StreamedPlanAtlasCanvas";
import {
  SemanticTerrainCanvas,
  type SemanticTerrainReport,
} from "./SemanticTerrainCanvas";
import type {
  LandformPlanPointReceipt,
} from "./landform-plan-worker-protocol";
import {
  panTerrainLabByFraction,
  zoomTerrainLabByFactor,
} from "./use-world-view-navigation";
import {
  TerrainCanvas,
  type TerrainLabAdapterReport,
  type TerrainLabComparisonReport,
  type TerrainLabPointReceipt,
  type TerrainLabRenderReport,
} from "./TerrainCanvas";
import {
  loadTerrainVisualAssets,
  visualProfileUsesMinecraftReference,
} from "./visual-assets";

type LabStatus = "loading" | "ready" | "rendering" | "error";
type BenchmarkProfile = "interactive" | "stress";

const PANE_OPTIONS: Array<{ value: TerrainLabPane; label: string; note: string }> = [
  {
    value: "runtime",
    label: "Runtime composed",
    note: "Production exact terrain and procedural horizon on one depth target",
  },
  { value: "canonical", label: "Real terrain", note: "Exact final chunks with textures" },
  {
    value: "plan",
    label: "Landform plan",
    note: "Research basins, divides, drainage, sinks, and quiet space",
  },
  {
    value: "atlas",
    label: "Planner atlas",
    note: "Pannable comparison including semantic multiscale refinement",
  },
  {
    value: "semantic",
    label: "Semantic terrain",
    note: "Standalone parent, regional, local, and correction reconstruction",
  },
  { value: "cpu", label: "CPU LOD", note: "Production CPU preview evaluator" },
  {
    value: "macro",
    label: "Fast macro",
    note: "Bounded approximate vanilla density sampler",
  },
  { value: "gpu", label: "GPU LOD", note: "GPU-resident production preview evaluator" },
];

const LAYER_OPTIONS: Array<{ value: TerrainLabLayer; label: string }> = [
  { value: "terrain", label: "Terrain" },
  { value: "height", label: "Height" },
  { value: "error", label: "CPU/GPU height error" },
  { value: "continentalness", label: "Continents" },
  { value: "climate", label: "Climate" },
  { value: "rivers", label: "River channels & banks" },
  { value: "wetlands", label: "Wetlands & pools" },
  { value: "landforms", label: "Landform class" },
  { value: "biomes", label: "Biome recipe" },
  { value: "surface", label: "Surface recipe" },
  { value: "streams", label: "Planned streams" },
  { value: "forests", label: "Forest summary" },
];

const VISUAL_PROFILE_OPTIONS: Array<{
  value: TerrainLabVisualProfile;
  label: string;
  note: string;
}> = [
  {
    value: "mclone-original",
    label: "Mclone Original",
    note: "Curated first-party art, filled by coherent provisional textures",
  },
  {
    value: "minecraft-reference",
    label: "Minecraft Reference",
    note: "Local Minecraft 1.17.1 textures for visual parity work",
  },
  {
    value: "hybrid-authoring",
    label: "Authoring Hybrid",
    note: "Curated first-party art, then Minecraft reference, then provisional",
  },
  {
    value: "first-party-coverage",
    label: "Coverage Debug",
    note: "Curated art or conspicuous numbered diagnostics; always textured",
  },
  {
    value: "provisional-audit",
    label: "Provisional Audit",
    note: "Provisional first-party textures only",
  },
];

export function App(): React.JSX.Element {
  const reviewCameraRef = useRef(
    parseTerrainLabReviewCamera(window.location.search),
  );
  const [state, setState] = useState<TerrainLabState>(() =>
    parseTerrainLabState(window.location.search)
  );
  const [status, setStatus] = useState<LabStatus>("loading");
  const [adapter, setAdapter] = useState<TerrainLabAdapterReport>();
  const [renderReport, setRenderReport] = useState<TerrainLabRenderReport>();
  const [comparison, setComparison] = useState<TerrainLabComparisonReport>();
  const [pointReceipt, setPointReceipt] = useState<TerrainLabPointReceipt>();
  const [error, setError] = useState<string>();
  const [camera, setCamera] = useState<TerrainLabCamera>(
    () => reviewCameraRef.current ?? DEFAULT_TERRAIN_LAB_CAMERA,
  );
  const [cacheEnabled, setCacheEnabled] = useState(true);
  const [cacheEpoch, setCacheEpoch] = useState(0);
  const [canonicalCacheEnabled, setCanonicalCacheEnabled] = useState(true);
  const [canonicalCacheEpoch, setCanonicalCacheEpoch] = useState(0);
  const [canonicalReport, setCanonicalReport] = useState<CanonicalTerrainReport>();
  const [runtimeReport, setRuntimeReport] = useState<RuntimeCompositionReport>();
  const [planReport, setPlanReport] = useState<LandformPlanReport>();
  const [planPointReceipt, setPlanPointReceipt] =
    useState<LandformPlanPointReceipt>();
  const [atlasReport, setAtlasReport] = useState<StreamedPlanAtlasReport>();
  const [semanticReport, setSemanticReport] = useState<SemanticTerrainReport>();
  const [atlasCacheEpoch, setAtlasCacheEpoch] = useState(0);
  const [comparisonCanonicalReport, setComparisonCanonicalReport] =
    useState<CanonicalTerrainReport>();
  const [visualAssetsReady, setVisualAssetsReady] = useState(false);
  const [minecraftReferenceAvailable, setMinecraftReferenceAvailable] =
    useState<boolean>();
  const [benchmarkProfile, setBenchmarkProfile] =
    useState<BenchmarkProfile>("interactive");
  const splitLayout = useResponsiveSplitLayout();

  useEffect(() => {
    let cancelled = false;
    void loadTerrainVisualAssets()
      .then((assets) => {
        if (!cancelled) {
          setMinecraftReferenceAvailable(assets.reference !== undefined);
          setVisualAssetsReady(true);
        }
      })
      .catch((loadError: unknown) => {
        if (!cancelled) {
          setError(errorMessage(loadError));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const search = terrainLabSearch(state, reviewCameraRef.current);
    if (window.location.search !== search) {
      window.history.replaceState(null, "", `${window.location.pathname}${search}`);
    }
  }, [state]);

  useEffect(() => {
    setPointReceipt(undefined);
  }, [state.profile]);

  useEffect(() => {
    setCanonicalReport(undefined);
    setRuntimeReport(undefined);
  }, [
    state.canonicalRadius,
    state.seed,
    state.texturePresentation,
    state.visualProfile,
  ]);

  useEffect(() => {
    setComparisonCanonicalReport(undefined);
  }, [
    state.compareVisualProfile,
    state.texturePresentation,
  ]);

  useEffect(() => {
    const restore = (): void => {
      setComparison(undefined);
      setState(parseTerrainLabState(window.location.search));
    };
    window.addEventListener("popstate", restore);
    return () => window.removeEventListener("popstate", restore);
  }, []);

  const updateState = useCallback((next: TerrainLabState) => {
    setComparison(undefined);
    setState(next);
  }, []);

  const patchState = useCallback((patch: Partial<TerrainLabState>) => {
    setComparison(undefined);
    setState((current) => ({ ...current, ...patch }));
  }, []);

  const footprint = footprintBlocks(state);
  const chunkWidth = footprint / 16;
  const runtimeVisible = state.panes.includes("runtime");
  const canonicalVisible = state.panes.includes("canonical");
  const planVisible = state.panes.includes("plan");
  const atlasVisible = state.panes.includes("atlas");
  const semanticVisible = state.panes.includes("semantic");
  const cpuVisible = state.panes.includes("cpu");
  const macroVisible = state.panes.includes("macro");
  const gpuVisible = state.panes.includes("gpu");
  const proceduralVisible = cpuVisible || macroVisible || gpuVisible;
  const terrainPaneVisible =
    runtimeVisible || canonicalVisible || proceduralVisible;
  const viewPaneVisible = terrainPaneVisible || semanticVisible;
  const diagnosticOnly = (planVisible || atlasVisible) && !viewPaneVisible;
  const proceduralSource = proceduralSourceForPanes(state.panes);
  const playHref = terrainLabPlayHref(state);
  const primaryVisualUnavailable = visualAssetsReady
    && visualProfileUsesMinecraftReference(state.visualProfile)
    && minecraftReferenceAvailable === false;
  const compareVisualProfile = state.compareVisualProfile === "off"
    ? undefined
    : state.compareVisualProfile;
  const visualComparisonVisible = canonicalVisible
    && compareVisualProfile !== undefined;
  const comparisonVisualUnavailable = visualAssetsReady
    && compareVisualProfile !== undefined
    && visualProfileUsesMinecraftReference(compareVisualProfile)
    && minecraftReferenceAvailable === false;
  const primaryTexturePresentation = texturePresentationForProfile(
    state.visualProfile,
    state.texturePresentation,
  );
  const comparisonTexturePresentation = compareVisualProfile
    ? texturePresentationForProfile(compareVisualProfile, state.texturePresentation)
    : state.texturePresentation;
  const proceduralState = useMemo(
    () => ({
      ...state,
      source: proceduralSource,
      texturePresentation: primaryTexturePresentation,
    }),
    [primaryTexturePresentation, proceduralSource, state],
  );
  const workspaceStatus: LabStatus = error
    ? "error"
    : !visualAssetsReady
      ? "loading"
      : (!proceduralVisible || primaryVisualUnavailable || status === "ready")
      && (!canonicalVisible || primaryVisualUnavailable || canonicalReport?.complete)
      && (!runtimeVisible || primaryVisualUnavailable || runtimeReport?.targetReady)
      && (!planVisible || planReport)
      && (!atlasVisible || atlasReport)
      && (!semanticVisible || semanticReport)
      && (!visualComparisonVisible
        || comparisonVisualUnavailable
        || comparisonCanonicalReport?.complete)
      ? "ready"
      : status === "loading" && proceduralVisible
        ? "loading"
        : "rendering";
  const renderStatus = workspaceStatus === "rendering" ? "updating" : workspaceStatus;
  const compareLayout = proceduralSource !== "split"
    ? "single"
    : splitLayout === "rows"
      ? "stacked"
      : "side-by-side";

  return (
    <div
      className="appShell"
      data-comparison-ready={comparison ? "true" : "false"}
      data-comparison-revision={comparison?.revision ?? 0}
      data-render-revision={renderReport?.revision ?? 0}
      data-camera-yaw={camera.yaw}
      data-camera-pitch={camera.pitch}
      data-projection={state.projection}
      data-base-mean-error={comparison?.meanAbsoluteBaseSurfaceError ?? ""}
      data-base-p95-error={comparison?.p95AbsoluteBaseSurfaceError ?? ""}
      data-solid-mean-error={comparison?.meanAbsoluteSurfaceError ?? ""}
      data-solid-p95-error={comparison?.p95AbsoluteSurfaceError ?? ""}
      data-solid-max-error={comparison?.maxAbsoluteSurfaceError ?? ""}
      data-display-mean-error={comparison?.meanAbsoluteDisplayError ?? ""}
      data-display-p95-error={comparison?.p95AbsoluteDisplayError ?? ""}
      data-display-max-error={comparison?.maxAbsoluteDisplayError ?? ""}
      data-ocean-agreement={comparison?.oceanWaterPresenceAgreement ?? ""}
      data-material-agreement={comparison?.macroSurfaceMaterialAgreement ?? ""}
      data-channel-agreement={comparison?.channelPresenceAgreement ?? ""}
      data-landform-agreement={comparison?.landformKindAgreement ?? ""}
      data-stage={state.contentStage}
      data-profile={state.profile}
      data-surface-quality={renderReport?.surfaceQuality ?? ""}
      data-visual-profile={state.visualProfile}
      data-texture-presentation={primaryTexturePresentation}
      data-runtime-target-ready={runtimeReport?.targetReady ? "true" : "false"}
      data-runtime-exact-complete={runtimeReport?.exactComplete ? "true" : "false"}
      data-plan-ready={planReport ? "true" : "false"}
      data-plan-build-ms={planReport?.buildMs ?? ""}
      data-plan-transfer-bytes={planReport?.transferBytes ?? 0}
      data-plan-checksum={planReport?.checksum ?? ""}
      data-atlas-ready={atlasReport ? "true" : "false"}
      data-atlas-topology={atlasReport?.topology ?? ""}
      data-atlas-query-ms={atlasReport?.queryMs ?? ""}
      data-atlas-clipped={atlasReport?.coverageClipped ? "true" : "false"}
      data-atlas-witness={atlasReport?.phaseTwoWitnessSha256 ?? ""}
      data-atlas-fallback-checksum={atlasReport?.fallback.semanticSha256 ?? ""}
      data-atlas-hierarchy-checksum={atlasReport?.hierarchy.semanticSha256 ?? ""}
      data-atlas-graph-checksum={atlasReport?.featureGraph.semanticSha256 ?? ""}
      data-atlas-multiscale-checksum={
        atlasReport?.multiscaleWitness.semanticSha256 ?? ""
      }
      data-atlas-multiscale-witness={
        atlasReport?.multiscaleWitness.witnessSha256 ?? ""
      }
      data-atlas-multiscale-parent-projection={
        atlasReport?.multiscaleWitness.parentProjectionSha256 ?? ""
      }
      data-atlas-multiscale-regional-projection={
        atlasReport?.multiscaleWitness.regionalProjectionSha256 ?? ""
      }
      data-atlas-multiscale-failures={
        atlasReport
          ? atlasReport.multiscaleWitness.unresolvedParentCount
            + atlasReport.multiscaleWitness.containmentFailureCount
            + atlasReport.multiscaleWitness.continuityFailureCount
            + atlasReport.multiscaleWitness.terminalFailureCount
          : ""
      }
      data-atlas-fallback-hits={atlasReport?.fallback.cacheHits ?? 0}
      data-atlas-hierarchy-hits={atlasReport?.hierarchy.cacheHits ?? 0}
      data-atlas-graph-hits={atlasReport?.featureGraph.cacheHits ?? 0}
      data-atlas-multiscale-hits={
        atlasReport?.multiscaleWitness.cacheHits ?? 0
      }
      data-semantic-ready={semanticReport ? "true" : "false"}
      data-semantic-compile-ms={semanticReport?.compileMs ?? ""}
      data-semantic-draw-ms={semanticReport?.drawMs ?? ""}
      data-semantic-suite={semanticReport?.suiteSha256 ?? ""}
      data-semantic-checksum={semanticReport?.terrainSha256 ?? ""}
      data-semantic-feature-checksum={semanticReport?.semanticSha256 ?? ""}
      data-semantic-production-unchanged={
        semanticReport?.productionTerrainUnchanged ? "true" : "false"
      }
      data-compare-visual-profile={state.compareVisualProfile}
      data-reference-textures-available={
        minecraftReferenceAvailable === undefined
          ? "loading"
          : minecraftReferenceAvailable ? "true" : "false"
      }
      data-inspected-x={state.profile === "overworld" ? "" : pointReceipt?.worldX ?? ""}
      data-inspected-z={state.profile === "overworld" ? "" : pointReceipt?.worldZ ?? ""}
      data-continentalness-error={comparison?.meanAbsoluteContinentalnessError ?? ""}
      data-compare-layout={compareLayout}
      data-vertex-count={renderReport?.vertexCount ?? 0}
      data-vegetation-summary-tiles={renderReport?.vegetationSummaryTileCount ?? 0}
      data-cpu-vegetation-summary-tiles={renderReport?.cpuVegetationSummaryTileCount ?? 0}
      data-gpu-vegetation-summary-tiles={renderReport?.gpuVegetationSummaryTileCount ?? 0}
      data-vegetation-record-tiles={renderReport?.vegetationRecordTileCount ?? 0}
      data-vegetation-aggregated-tiles={renderReport?.vegetationAggregatedTileCount ?? 0}
      data-tree-instances={renderReport?.treeInstanceCount ?? 0}
      data-tree-instance-bytes={renderReport?.treeInstanceBytes ?? 0}
      data-tree-proxy-vertices={renderReport?.treeProxyVertexCount ?? 0}
      data-vegetation-cell-requests={renderReport?.vegetationCellRequests ?? 0}
      data-vegetation-cell-hits={renderReport?.vegetationCellHits ?? 0}
      data-vegetation-cell-misses={renderReport?.vegetationCellMisses ?? 0}
      data-retained-vegetation-cells={renderReport?.retainedVegetationCells ?? 0}
      data-requested-spacing={renderReport?.requestedSpacing ?? 0}
      data-effective-spacing={renderReport?.effectiveSpacing ?? 0}
      data-published-spacing={renderReport?.publishedSpacing ?? 0}
      data-footprint-blocks={renderReport?.footprintBlocks ?? 0}
      data-resident-tiles={renderReport?.residentTileCount ?? 0}
      data-queued-tiles={renderReport?.queuedTileCount ?? 0}
      data-target-ready={renderReport?.targetReady ? "true" : "false"}
      data-cpu-target-ready={renderReport?.cpuTargetReady ? "true" : "false"}
      data-gpu-target-ready={renderReport?.gpuTargetReady ? "true" : "false"}
      data-cpu-target-ms={renderReport?.cpuTargetReadyMs ?? ""}
      data-gpu-target-ms={renderReport?.gpuTargetReadyMs ?? ""}
      data-macro-request-ms={renderReport?.requestMacroCompileMs ?? ""}
      data-cpu-request-ms={renderReport?.requestCpuReferenceMs ?? ""}
      data-cpu-vegetation-request-ms={renderReport?.requestCpuVegetationMs ?? ""}
      data-cpu-pack-upload-request-ms={renderReport?.requestCpuPackUploadMs ?? ""}
      data-cpu-lattice-points={renderReport?.requestCpuSampleLatticePoints ?? 0}
      data-cpu-terrain-evaluations={renderReport?.requestCpuTerrainSampleEvaluations ?? 0}
      data-cpu-forest-evaluations={renderReport?.requestCpuForestIntentEvaluations ?? 0}
      data-cpu-footprint-summaries={renderReport?.requestCpuForestFootprintSummaries ?? 0}
      data-gpu-lattice-points={renderReport?.requestGpuSampleLatticePoints ?? 0}
      data-gpu-terrain-evaluations={renderReport?.requestGpuTerrainSampleEvaluations ?? 0}
      data-gpu-forest-evaluations={renderReport?.requestGpuForestIntentEvaluations ?? 0}
      data-gpu-footprint-summaries={renderReport?.requestGpuForestFootprintSummaries ?? 0}
      data-cache-enabled={cacheEnabled ? "true" : "false"}
      data-cache-hits={renderReport?.requestCacheHitTiles ?? 0}
      data-visible-tiles={renderReport?.visibleTileCount ?? 0}
      data-request-cpu-tiles={renderReport?.requestCpuCompiledTiles ?? 0}
      data-request-gpu-tiles={renderReport?.requestGpuDispatchedTiles ?? 0}
      data-request-macro-tiles={renderReport?.requestMacroCompiledTiles ?? 0}
      data-samples-per-axis={renderReport?.samplesPerAxis ?? 0}
      data-reference-bytes={renderReport?.referenceBytes ?? 0}
      data-gpu-sample-bytes={renderReport?.gpuSampleBytes ?? 0}
      data-readback-bytes={renderReport?.readbackBytes ?? 0}
      data-request-readback-bytes={renderReport?.requestReadbackBytes ?? 0}
      data-resident-bytes={renderReport?.residentBytes ?? 0}
      data-panes={state.panes.join(",")}
      data-canonical-published={canonicalReport?.publishedChunks ?? 0}
      data-canonical-requested={canonicalReport?.requestedChunks ?? 0}
      data-canonical-complete={canonicalReport?.complete ? "true" : "false"}
      data-canonical-cache-enabled={canonicalCacheEnabled ? "true" : "false"}
      data-canonical-cache-hits={canonicalReport?.cacheHits ?? 0}
      data-canonical-epoch={canonicalReport?.epoch ?? 0}
      data-canonical-resident-hits={canonicalReport?.residentHits ?? 0}
      data-canonical-admission-frames={canonicalReport?.admissionFrames ?? 0}
      data-canonical-max-frame-admissions={canonicalReport?.maxFrameAdmissions ?? 0}
      data-canonical-cached-chunks={canonicalReport?.cachedChunks ?? 0}
      data-canonical-resident-raw-bytes={canonicalReport?.residentRawBytes ?? 0}
      data-canonical-cache-raw-bytes={canonicalReport?.cacheRawBytes ?? 0}
      data-canonical-mesh-used-bytes={canonicalReport?.residentMeshUsedBytes ?? 0}
      data-canonical-tracked-bytes={canonicalReport?.trackedBytes ?? 0}
      data-canonical-worker-generation-ms={canonicalReport?.generationMs ?? 0}
      data-canonical-worker-presentation-ms={
        canonicalReport?.workerPresentationMs ?? 0
      }
      data-canonical-worker-mesh-ms={canonicalReport?.workerMeshMs ?? 0}
      data-canonical-worker-pack-ms={canonicalReport?.workerPackMs ?? 0}
      data-canonical-worker-transfer-ms={canonicalReport?.workerTransferMs ?? 0}
      data-canonical-main-decode-ms={canonicalReport?.mainDecodeMs ?? 0}
      data-canonical-mesh-upload-ms={canonicalReport?.meshUploadMs ?? 0}
      data-canonical-max-admission-ms={canonicalReport?.maxAdmissionMs ?? 0}
      data-canonical-mesh-target-chunks={canonicalReport?.meshTargetChunks ?? 0}
      data-canonical-warm-hits={canonicalReport?.warmHits ?? 0}
      data-canonical-warm-chunks={canonicalReport?.warmChunks ?? 0}
      data-canonical-transport-kind={canonicalReport?.transportKind ?? "unavailable"}
      data-canonical-cross-origin-isolated={
        canonicalReport?.crossOriginIsolated ? "true" : "false"
      }
      data-canonical-result-arena-capacity={
        canonicalReport?.resultArenaCapacityBytes ?? 0
      }
      data-canonical-result-arena-high-water={
        canonicalReport?.resultArenaHighWaterBytes ?? 0
      }
      data-canonical-result-arena-overflows={
        canonicalReport?.resultArenaOverflowCount ?? 0
      }
    >
      <header className="topBar">
        <div className="brandLockup">
          <div className="brandMark" aria-hidden="true">
            <span />
            <span />
            <span />
          </div>
          <div>
            <div className="eyebrow">Mclone Terrain Lab</div>
            <h1>See the whole world take shape.</h1>
          </div>
        </div>
        <div className="headerMeta">
          <div className={`statusPill ${workspaceStatus}`} data-testid="lab-status">
            <span className="statusDot" />
            {renderStatus}
          </div>
          <a className="playLink" href={playHref}>
            <span>Play this seed</span>
            <small>
              Fly at {formatInteger(state.centerX)}, {formatInteger(state.centerZ)}
            </small>
          </a>
        </div>
      </header>

      {error ? (
        <div className="errorBanner" role="alert">
          <strong>Terrain Lab could not render.</strong>
          <span>{error}</span>
        </div>
      ) : null}

      <main className="labWorkbench">
        <section className="viewerColumn" aria-label="Terrain preview">
          <div className="previewToolbar" data-testid="preview-source-controls">
            <PaneToggles
              profile={state.profile}
              panes={state.panes}
              onToggle={(pane) => updateState(toggleTerrainLabPane(state, pane))}
            />
            <WorkspaceGuide
              profile={state.profile}
              visualProfile={state.visualProfile}
              panes={state.panes}
              surfaceQuality={state.surfaceQuality}
            />
          </div>
          <div
            className={`mapToolbar${diagnosticOnly ? " planOnlyToolbar" : ""}`}
            data-testid="viewport-controls"
          >
            {diagnosticOnly ? (
              <div className="planToolbarNote">
                <span>Research projection</span>
                <strong>
                  {atlasVisible
                    ? "2D streamed structural atlas · four synchronized views"
                    : "2D structural map · fixed 32-block cells"}
                </strong>
                <small>
                  Terrain view, projection, and surface controls resume when a
                  terrain pane is visible.
                </small>
              </div>
            ) : (
              <>
            <SegmentedControl<TerrainLabView>
              label="View"
              value={state.view}
              options={[
                { value: "3d", label: "3D terrain" },
                { value: "map", label: "Map" },
              ]}
              onChange={(view) => patchState({ view })}
            />
            {terrainPaneVisible ? (
              <>
            <SegmentedControl<TerrainLabProjection>
              label="3D projection"
              value={state.projection}
              options={[
                {
                  value: "orthographic",
                  label: "Orthographic",
                  note: "Preserve scale across the whole terrain view",
                },
                {
                  value: "perspective",
                  label: "Perspective",
                  note: "Use distance foreshortening",
                },
              ]}
              onChange={(projection) => patchState({ projection })}
            />
            <label className="fieldLabel compactField">
              <span>Resolution</span>
              <select
                aria-label="Terrain resolution"
                value={String(state.detail)}
                onChange={(event) =>
                  patchState({ detail: parseDetail(event.target.value) })
                }
              >
                <option value="auto">Auto · viewport</option>
                {TERRAIN_LAB_SPACINGS.map((spacing) => (
                  <option key={spacing} value={spacing}>
                    1:{spacing} · {spacing} block{spacing === 1 ? "" : "s"} per sample
                  </option>
                ))}
              </select>
            </label>
            <label className="fieldLabel compactField">
              <span>Surface detail</span>
              <select
                aria-label="Terrain surface detail"
                value={state.surfaceQuality}
                onChange={(event) =>
                  patchState({
                    surfaceQuality: event.target.value === "inferred"
                      ? "inferred"
                      : "basic",
                  })
                }
              >
                <option value="basic">Basic · biome tops</option>
                <option value="inferred">Inferred · surface builders</option>
              </select>
            </label>
              </>
            ) : null}
              </>
            )}
            <div className="mapZoom" aria-label="Viewport zoom controls">
              <button
                type="button"
                onClick={() => {
                  void zoomTerrainLabByFactor(state, camera, 0.5)
                    .then(updateState)
                    .catch((zoomError: unknown) => setError(errorMessage(zoomError)));
                }}
                aria-label="Zoom in"
              >
                +
              </button>
              <div>
                <strong>{formatDistance(footprint)}</strong>
                <span>across</span>
              </div>
              <button
                type="button"
                onClick={() => {
                  void zoomTerrainLabByFactor(state, camera, 2)
                    .then(updateState)
                    .catch((zoomError: unknown) => setError(errorMessage(zoomError)));
                }}
                aria-label="Zoom out"
              >
                −
              </button>
            </div>
          </div>
          <div
            className={`paneWorkspace logicalPanes${state.panes.length}${
              !planVisible
              && !atlasVisible
              && !semanticVisible
              && canonicalVisible
              && cpuVisible
              && (macroVisible || gpuVisible)
                ? " threePaneWorkspace"
                : ""
            }`}
            data-testid="pane-workspace"
          >
            {runtimeVisible ? (
              <div className="paneFrame runtimePaneFrame">
                {!visualAssetsReady ? (
                  <VisualProfileUnavailable
                    profile={state.visualProfile}
                    reason="Loading material sources…"
                  />
                ) : primaryVisualUnavailable ? (
                  <VisualProfileUnavailable
                    profile={state.visualProfile}
                    reason="This profile requires a local Minecraft 1.17.1 reference pack."
                  />
                ) : (
                  <RuntimeCompositionCanvas
                    state={state}
                    visualProfile={state.visualProfile}
                    texturePresentation={primaryTexturePresentation}
                    camera={camera}
                    onStateChange={updateState}
                    onCameraChange={setCamera}
                    onReport={setRuntimeReport}
                    onError={setError}
                  />
                )}
              </div>
            ) : null}
            {canonicalVisible ? (
              <div className="paneFrame canonicalPaneFrame">
                <div
                  className={`canonicalPaneSet${
                    visualComparisonVisible ? " comparingMaterials" : ""
                  }`}
                >
                  {!visualAssetsReady ? (
                    <VisualProfileUnavailable
                      profile={state.visualProfile}
                      reason="Loading material sources…"
                    />
                  ) : primaryVisualUnavailable ? (
                    <VisualProfileUnavailable
                      profile={state.visualProfile}
                      reason="This profile requires a local Minecraft 1.17.1 reference pack."
                    />
                  ) : (
                    <CanonicalTerrainCanvas
                      state={state}
                      visualProfile={state.visualProfile}
                      texturePresentation={primaryTexturePresentation}
                      camera={camera}
                      cacheEnabled={canonicalCacheEnabled}
                      cacheEpoch={canonicalCacheEpoch}
                      onStateChange={updateState}
                      onCameraChange={setCamera}
                      onReport={setCanonicalReport}
                      onError={setError}
                    />
                  )}
                  {visualComparisonVisible && compareVisualProfile ? (
                    !visualAssetsReady ? (
                      <VisualProfileUnavailable
                        comparison
                        profile={compareVisualProfile}
                        reason="Loading material sources…"
                      />
                    ) : comparisonVisualUnavailable ? (
                      <VisualProfileUnavailable
                        comparison
                        profile={compareVisualProfile}
                        reason="This comparison requires a local Minecraft 1.17.1 reference pack."
                      />
                    ) : (
                      <CanonicalTerrainCanvas
                        state={state}
                        visualProfile={compareVisualProfile}
                        texturePresentation={comparisonTexturePresentation}
                        comparison
                        camera={camera}
                        cacheEnabled={canonicalCacheEnabled}
                        cacheEpoch={canonicalCacheEpoch}
                        onStateChange={updateState}
                        onCameraChange={setCamera}
                        onReport={setComparisonCanonicalReport}
                        onError={setError}
                      />
                    )
                  ) : null}
                </div>
              </div>
            ) : null}
            {canonicalVisible && proceduralVisible ? (
              <div
                className="pageScrollGutter panePageScrollGutter"
                data-testid="pane-scroll-gutter"
                role="separator"
                aria-label="Swipe here to scroll the page"
                aria-orientation="horizontal"
              >
                <span aria-hidden="true">↕ scroll page</span>
              </div>
            ) : null}
            {planVisible ? (
              <div className="paneFrame landformPlanPaneFrame">
                <LandformPlanCanvas
                  state={state}
                  camera={camera}
                  onStateChange={updateState}
                  onCameraChange={setCamera}
                  onReport={setPlanReport}
                  onInspect={setPlanPointReceipt}
                  onError={setError}
                />
              </div>
            ) : null}
            {atlasVisible ? (
              <div className="paneFrame streamedPlanAtlasPaneFrame">
                <StreamedPlanAtlasCanvas
                  state={state}
                  camera={camera}
                  cacheEpoch={atlasCacheEpoch}
                  onStateChange={updateState}
                  onCameraChange={setCamera}
                  onReport={setAtlasReport}
                  onError={setError}
                />
              </div>
            ) : null}
            {semanticVisible ? (
              <div className="paneFrame semanticTerrainPaneFrame">
                <SemanticTerrainCanvas
                  state={state}
                  camera={camera}
                  onStateChange={updateState}
                  onCameraChange={setCamera}
                  onReport={setSemanticReport}
                  onError={setError}
                />
              </div>
            ) : null}
            {proceduralVisible ? (
              <div
                className={`paneFrame proceduralPaneFrame${
                  cpuVisible && (macroVisible || gpuVisible) ? " twoLogicalPanes" : ""
                }${splitLayout === "rows" ? " splitRows" : ""
                }`}
              >
                {!visualAssetsReady ? (
                  <VisualProfileUnavailable
                    profile={state.visualProfile}
                    reason="Loading material sources…"
                  />
                ) : primaryVisualUnavailable ? (
                  <VisualProfileUnavailable
                    profile={state.visualProfile}
                    reason="This profile requires a local Minecraft 1.17.1 reference pack."
                  />
                ) : (
                  <TerrainCanvas
                    state={proceduralState}
                    camera={camera}
                    splitLayout={splitLayout}
                    cacheEnabled={cacheEnabled}
                    cacheEpoch={cacheEpoch}
                    maxVisibleTilesPerAxis={benchmarkProfile === "stress" ? 12 : 8}
                    onStateChange={updateState}
                    onCameraChange={setCamera}
                    onAdapter={setAdapter}
                    onRender={setRenderReport}
                    onComparison={setComparison}
                    onInspect={setPointReceipt}
                    onError={setError}
                    onStatus={setStatus}
                  />
                )}
              </div>
            ) : null}
          </div>
          <div className="viewerFooter">
            <div>
              <span className="footerLabel">footprint</span>
              <strong>
                {formatDistance(footprint)} ×{" "}
                {formatDistance(renderReport?.viewHeightBlocks ?? footprint)}
              </strong>
            </div>
            <div>
              <span className="footerLabel">chunk equivalent</span>
              <strong>
                {formatInteger(chunkWidth)} ×{" "}
                {formatInteger((renderReport?.viewHeightBlocks ?? footprint) / 16)}
              </strong>
            </div>
            <div>
              <span className="footerLabel">resolution</span>
              <strong>
                {atlasVisible && !terrainPaneVisible
                  ? "1:1024 plan regions"
                  : semanticVisible && !terrainPaneVisible
                  ? "65 × adaptive lattice"
                  : planVisible && !proceduralVisible
                  ? "1:32 plan cells"
                  : (
                    <>
                      {state.detail === "auto" ? "Auto" : `1:${state.detail}`}
                      {" → "}
                      1:{renderReport?.effectiveSpacing ?? "—"}
                    </>
                  )}
              </strong>
            </div>
            <div className="approximationNote">
              <SourceFootnote
                profile={state.profile}
                visualProfile={state.visualProfile}
                panes={state.panes}
              />
            </div>
          </div>
        </section>

        <aside className="controlRail" aria-label="Terrain Lab controls">
          <ControlSection number="01" title="World">
            <label className="fieldLabel">
              <span>Terrain profile</span>
              <select
                aria-label="Terrain profile"
                value={state.profile}
                onChange={(event) =>
                  updateState(switchTerrainLabProfile(
                    state,
                    event.target.value as TerrainLabProfile,
                  ))
                }
              >
                <option value="mclone-overworld-v1">Mclone overworld</option>
                <option value="overworld">Minecraft Java 1.17.1 overworld</option>
              </select>
            </label>
            <SeedControl value={state.seed} onCommit={(seed) => patchState({ seed })} />
            <div className="coordinateGrid">
              <NumberControl
                label="Center X"
                value={state.centerX}
                onCommit={(centerX) => patchState({ centerX })}
              />
              <NumberControl
                label="Center Z"
                value={state.centerZ}
                onCommit={(centerZ) => patchState({ centerZ })}
              />
            </div>
            <div className="buttonRow">
              <button type="button" onClick={() => updateState(REVIEW_TERRAIN_LAB_STATE)}>
                Load review site
              </button>
              <button type="button" onClick={() => patchState({ seed: randomSeed() })}>
                New seed
              </button>
            </div>
          </ControlSection>

          <ControlSection number="02" title="Navigation">
            <div className="navigationReadout">
              <span>Viewport width</span>
              <strong>{formatDistance(footprint)}</strong>
              <small>
                Wheel or pinch changes coverage. Resolution stays{" "}
                {state.detail === "auto" ? "automatic" : `fixed at 1:${state.detail}`}.
              </small>
            </div>
            <PanPad
              state={state}
              camera={camera}
              onChange={updateState}
              onError={(panError) => setError(errorMessage(panError))}
            />
          </ControlSection>

          <ControlSection number="03" title="Presentation">
            {terrainPaneVisible ? (
              <>
            <label className="fieldLabel">
              <span>Visual material profile</span>
              <select
                aria-label="Visual material profile"
                value={state.visualProfile}
                onChange={(event) =>
                  patchState({
                    visualProfile: event.target.value as TerrainLabVisualProfile,
                  })
                }
              >
                {VISUAL_PROFILE_OPTIONS.map((option) => (
                  <option
                    key={option.value}
                    value={option.value}
                    disabled={
                      minecraftReferenceAvailable === false
                      && visualProfileUsesMinecraftReference(option.value)
                    }
                  >
                    {option.label}
                  </option>
                ))}
              </select>
              <small>{visualProfileNote(state.visualProfile)}</small>
            </label>
            <label className="fieldLabel">
              <span>Texture representation</span>
              <select
                aria-label="Texture representation"
                value={primaryTexturePresentation}
                disabled={state.visualProfile === "first-party-coverage"}
                onChange={(event) =>
                  patchState({
                    texturePresentation:
                      event.target.value as TerrainLabTexturePresentation,
                  })
                }
              >
                <option value="textured">Textured</option>
                <option value="flat-colors">
                  Derived flat colors · alpha-weighted texture average
                </option>
              </select>
            </label>
            <label className="fieldLabel">
              <span>Exact material comparison</span>
              <select
                aria-label="Exact material comparison"
                value={state.compareVisualProfile}
                disabled={!canonicalVisible}
                onChange={(event) =>
                  patchState({
                    compareVisualProfile:
                      event.target.value as TerrainLabComparisonVisualProfile,
                  })
                }
              >
                <option value="off">Off · one exact material view</option>
                {VISUAL_PROFILE_OPTIONS.map((option) => (
                  <option
                    key={option.value}
                    value={option.value}
                    disabled={
                      minecraftReferenceAvailable === false
                      && visualProfileUsesMinecraftReference(option.value)
                    }
                  >
                    Compare with {option.label}
                  </option>
                ))}
              </select>
            </label>
            <p className="controlNote">
              Profiles are complete source chains, not independent pack
              checkboxes. Minecraft Reference and Authoring Hybrid require a
              local extracted 1.17.1 pack. Comparison adds a synchronized exact
              pane; it does not change terrain generation.
            </p>
              </>
            ) : null}
            {atlasVisible ? (
              <>
                <SegmentedControl<StreamedPlanAtlasTopology>
                  label="Atlas topology"
                  value={state.atlasTopology}
                  options={[
                    {
                      value: "plane",
                      label: "Plane",
                      note: "Unbounded canonical plan regions",
                    },
                    {
                      value: "cylinder-x",
                      label: "Cylinder X",
                      note: "6.144 km periodic X, unbounded Z",
                    },
                    {
                      value: "torus",
                      label: "Torus",
                      note: "6.144 km periodic X and Z",
                    },
                  ]}
                  onChange={(atlasTopology) => patchState({ atlasTopology })}
                />
                <div className="visibilityToggles atlasVisibilityToggles">
                  <ToggleButton
                    label="Regions"
                    pressed={state.atlasRegionsVisible}
                    onChange={(atlasRegionsVisible) =>
                      patchState({ atlasRegionsVisible })}
                  />
                  <ToggleButton
                    label="Samples"
                    pressed={state.atlasFallbackSamplesVisible}
                    onChange={(atlasFallbackSamplesVisible) =>
                      patchState({ atlasFallbackSamplesVisible })}
                  />
                  <ToggleButton
                    label="Features"
                    pressed={state.atlasFallbackFeaturesVisible}
                    onChange={(atlasFallbackFeaturesVisible) =>
                      patchState({ atlasFallbackFeaturesVisible })}
                  />
                  <ToggleButton
                    label="Hierarchy"
                    pressed={state.atlasHierarchyVisible}
                    onChange={(atlasHierarchyVisible) =>
                      patchState({ atlasHierarchyVisible })}
                  />
                  <ToggleButton
                    label="Facets"
                    pressed={state.atlasFacetsVisible}
                    onChange={(atlasFacetsVisible) =>
                      patchState({ atlasFacetsVisible })}
                  />
                  <ToggleButton
                    label="Graph bounds"
                    pressed={state.atlasGraphBoundsVisible}
                    onChange={(atlasGraphBoundsVisible) =>
                      patchState({ atlasGraphBoundsVisible })}
                  />
                  <ToggleButton
                    label="Graph edges"
                    pressed={state.atlasGraphEdgesVisible}
                    onChange={(atlasGraphEdgesVisible) =>
                      patchState({ atlasGraphEdgesVisible })}
                  />
                  <ToggleButton
                    label="Parent level"
                    pressed={state.atlasWitnessParentVisible}
                    onChange={(atlasWitnessParentVisible) =>
                      patchState({ atlasWitnessParentVisible })}
                  />
                  <ToggleButton
                    label="Regional level"
                    pressed={state.atlasWitnessRegionalVisible}
                    onChange={(atlasWitnessRegionalVisible) =>
                      patchState({ atlasWitnessRegionalVisible })}
                  />
                  <ToggleButton
                    label="Local level"
                    pressed={state.atlasWitnessLocalVisible}
                    onChange={(atlasWitnessLocalVisible) =>
                      patchState({ atlasWitnessLocalVisible })}
                  />
                  <ToggleButton
                    label="Refinement bounds"
                    pressed={state.atlasWitnessBoundsVisible}
                    onChange={(atlasWitnessBoundsVisible) =>
                      patchState({ atlasWitnessBoundsVisible })}
                  />
                  <ToggleButton
                    label="Identities"
                    pressed={state.atlasIdentityVisible}
                    onChange={(atlasIdentityVisible) =>
                      patchState({ atlasIdentityVisible })}
                  />
                  <ToggleButton
                    label="Wrap seams"
                    pressed={state.atlasSeamsVisible}
                    onChange={(atlasSeamsVisible) =>
                      patchState({ atlasSeamsVisible })}
                  />
                </div>
                <p className="controlNote">
                  Every overlay is presentation-only. Panning asks Rust for
                  canonical regions around the new viewport; it does not
                  recenter or mutate a solve.
                </p>
              </>
            ) : null}
            {semanticVisible ? (
              <>
                <SegmentedControl<SemanticTerrainSubstrate>
                  label="Semantic substrate"
                  value={state.semanticSubstrate}
                  options={[
                    {
                      value: "flat",
                      label: "Flat",
                      note: "Constant Y 64 attribution control",
                    },
                    {
                      value: "quiet",
                      label: "Quiet",
                      note: "Low-amplitude coordinate-pure rolling foundation",
                    },
                  ]}
                  onChange={(semanticSubstrate) =>
                    patchState({ semanticSubstrate })}
                />
                <SegmentedControl<SemanticTerrainFeatures>
                  label="Feature reconstruction"
                  value={state.semanticFeatures}
                  options={[
                    {
                      value: "range",
                      label: "Range",
                      note: "Positive compact-support range axes only",
                    },
                    {
                      value: "basin",
                      label: "Basin",
                      note: "Negative basin routes and visual water only",
                    },
                    {
                      value: "combined",
                      label: "Combined",
                      note: "Independent range and basin influences together",
                    },
                  ]}
                  onChange={(semanticFeatures) =>
                    patchState({ semanticFeatures })}
                />
                <SegmentedControl<StreamedPlanAtlasTopology>
                  label="Semantic topology"
                  value={state.semanticTopology}
                  options={[
                    {
                      value: "plane",
                      label: "Plane",
                      note: "Unbounded coordinate-pure reconstruction",
                    },
                    {
                      value: "cylinder-x",
                      label: "Cylinder X",
                      note: "6.144 km periodic X, unbounded Z",
                    },
                    {
                      value: "torus",
                      label: "Torus",
                      note: "6.144 km periodic X and Z",
                    },
                  ]}
                  onChange={(semanticTopology) =>
                    patchState({ semanticTopology })}
                />
                <SegmentedControl<SemanticTerrainCorrection>
                  label="Correction panel"
                  value={state.semanticCorrection}
                  options={[
                    {
                      value: "regional",
                      label: "Regional − parent",
                      note: "First bounded refinement correction",
                    },
                    {
                      value: "local",
                      label: "Local − regional",
                      note: "Second bounded refinement correction",
                    },
                  ]}
                  onChange={(semanticCorrection) =>
                    patchState({ semanticCorrection })}
                />
                <SegmentedControl<SemanticTerrainVerticalScale>
                  label="Vertical scale"
                  value={state.semanticVerticalScale}
                  options={[
                    {
                      value: "1x",
                      label: "Physical 1×",
                      note: "One vertical block uses the horizontal world scale",
                    },
                    {
                      value: "8x",
                      label: "8×",
                      note: "Explicit world-scale diagnostic exaggeration",
                    },
                    {
                      value: "24x",
                      label: "24×",
                      note: "Strong exaggeration for broad-landform inspection",
                    },
                  ]}
                  onChange={(semanticVerticalScale) =>
                    patchState({ semanticVerticalScale })}
                />
                <div className="visibilityToggles">
                  <ToggleButton
                    label="Feature guides"
                    pressed={state.semanticGuidesVisible}
                    onChange={(semanticGuidesVisible) =>
                      patchState({ semanticGuidesVisible })}
                  />
                </div>
                <p className="controlNote">
                  Each panel reconstructs one representation course; finer
                  segments replace their parent rather than stacking on it.
                  This sandbox remains disconnected from Mclone Overworld.
                </p>
              </>
            ) : null}
            {canonicalVisible ? (
              <>
                <SegmentedControl<CanonicalTerrainStage>
                  label="Real terrain checkpoint"
                  value={state.canonicalStage}
                  options={[
                    {
                      value: "final",
                      label: "Final features",
                      note: "Production final chunks, including decorations",
                    },
                    {
                      value: "surface",
                      label: "Surface",
                      note: "Production terrain and surface checkpoint",
                    },
                  ]}
                  onChange={(canonicalStage) => patchState({ canonicalStage })}
                />
                <label className="fieldLabel">
                  <span>Exact chunk footprint</span>
                  <select
                    aria-label="Exact chunk footprint"
                    value={state.canonicalRadius}
                    onChange={(event) =>
                      patchState({ canonicalRadius: Number(event.target.value) })
                    }
                  >
                    {TERRAIN_LAB_CANONICAL_RADII.map((radius) => {
                      const side = radius * 2 + 1;
                      const chunks = side * side;
                      return (
                        <option key={radius} value={radius}>
                          {side} × {side} · {chunks} {
                            chunks === 1 ? "chunk" : "chunks"
                          } · {side * 16} blocks
                        </option>
                      );
                    })}
                  </select>
                </label>
                <div className="visibilityToggles">
                  <ToggleButton
                    label="Water"
                    pressed={state.waterVisible}
                    onChange={(waterVisible) => patchState({ waterVisible })}
                  />
                  <ToggleButton
                    label="Vegetation"
                    pressed={state.vegetationVisible}
                    onChange={(vegetationVisible) => patchState({ vegetationVisible })}
                  />
                </div>
                <p className="controlNote">
                  Visibility remeshes retained exact blocks. It never reruns or
                  changes feature generation.
                </p>
              </>
            ) : null}
            {proceduralVisible ? (
              <>
                <label className="fieldLabel">
                  <span>LOD content checkpoint</span>
                  <select
                    aria-label="LOD content checkpoint"
                    value={state.contentStage}
                    disabled={state.profile === "overworld"}
                    onChange={(event) =>
                      patchState({
                        contentStage: event.target.value as TerrainLabContentStage,
                      })
                    }
                  >
                    {state.profile === "overworld" ? (
                      <option value="surface">
                        Surface · direct vanilla density columns
                      </option>
                    ) : (
                      <>
                        <option value="base">Base · land and ocean fields</option>
                        <option value="hydrology">Hydrology · rivers and wetlands</option>
                        <option value="structured">
                          Structured · planned streams near 1:1–1:4
                        </option>
                        <option value="surface">Surface · biome materials</option>
                        <option value="cover">Cover · vegetation summary</option>
                      </>
                    )}
                  </select>
                </label>
                <p className="controlNote">
                  {state.profile === "overworld"
                    ? "Vanilla LOD samples density, ocean fill, biome, and an approximate surface material directly in a worker. It does not materialize chunks."
                    : "Checkpoints are ordered preview content, not gameplay switches. Planned streams are reconstructed only through 1:4; coarser views say unavailable instead of inventing them."}
                </p>
                <label className="fieldLabel">
                  <span>LOD diagnostic layer</span>
                  <select
                    aria-label="Diagnostic layer"
                    value={state.layer}
                    onChange={(event) =>
                      patchState({ layer: event.target.value as TerrainLabLayer })
                    }
                  >
                    {LAYER_OPTIONS.filter((option) =>
                      state.profile !== "overworld"
                      || ["terrain", "height", "error", "biomes", "surface"].includes(option.value)
                    ).map((option) => (
                      <option key={option.value} value={option.value}>
                        {state.profile === "overworld" && option.value === "error"
                          ? "Exact / macro height error"
                          : option.label}
                      </option>
                    ))}
                  </select>
                </label>
              </>
            ) : null}
            {planVisible ? (
              <>
                <div className="visibilityToggles planVisibilityToggles">
                  <ToggleButton
                    label="Basins"
                    pressed={state.planBasinsVisible}
                    onChange={(planBasinsVisible) =>
                      patchState({ planBasinsVisible })}
                  />
                  <ToggleButton
                    label="Quiet"
                    pressed={state.planQuietVisible}
                    onChange={(planQuietVisible) =>
                      patchState({ planQuietVisible })}
                  />
                  <ToggleButton
                    label="Drainage"
                    pressed={state.planDrainageVisible}
                    onChange={(planDrainageVisible) =>
                      patchState({ planDrainageVisible })}
                  />
                  <ToggleButton
                    label="Divides"
                    pressed={state.planDividesVisible}
                    onChange={(planDividesVisible) =>
                      patchState({ planDividesVisible })}
                  />
                  <ToggleButton
                    label="Confluences"
                    pressed={state.planConfluencesVisible}
                    onChange={(planConfluencesVisible) =>
                      patchState({ planConfluencesVisible })}
                  />
                  <ToggleButton
                    label="Sinks"
                    pressed={state.planSinksVisible}
                    onChange={(planSinksVisible) =>
                      patchState({ planSinksVisible })}
                  />
                </div>
                <p className="controlNote">
                  Landform-plan facts are independent presentation overlays.
                  Toggling them never rebuilds the plan or changes terrain.
                </p>
              </>
            ) : null}
            {viewPaneVisible ? (
              <button
                type="button"
                className="cameraResetButton"
                onClick={() => setCamera(DEFAULT_TERRAIN_LAB_CAMERA)}
              >
                Reset 3D camera
              </button>
            ) : null}
          </ControlSection>

          {terrainPaneVisible || atlasVisible ? (
          <ControlSection number="04" title="Cache & benchmark">
            {atlasVisible ? (
              <>
                <button
                  type="button"
                  onClick={() => {
                    setAtlasReport(undefined);
                    setAtlasCacheEpoch((current) => current + 1);
                  }}
                >
                  Cold rebuild planner atlas
                </button>
                <p className="controlNote">
                  Atlas caches retain 384 canonical plans per view.
                  Cold rebuild clears all four while preserving the same
                  semantic checksums for the current viewport.
                </p>
              </>
            ) : null}
            {canonicalVisible ? (
              <>
                <SegmentedControl<"on" | "off">
                  label="Real terrain cache"
                  value={canonicalCacheEnabled ? "on" : "off"}
                  options={[
                    {
                      value: "on",
                      label: "Exact cache on",
                      note: "Reuse exact chunks for this browser session",
                    },
                    {
                      value: "off",
                      label: "Exact cache off",
                      note: "Regenerate exact chunks after every footprint change",
                    },
                  ]}
                  onChange={(value) => setCanonicalCacheEnabled(value === "on")}
                />
                <button
                  type="button"
                  onClick={() => {
                    setCanonicalReport(undefined);
                    setComparisonCanonicalReport(undefined);
                    setCanonicalCacheEpoch((current) => current + 1);
                  }}
                >
                  Regenerate real terrain
                </button>
              </>
            ) : null}
            {proceduralVisible ? (
              <>
            <SegmentedControl<"on" | "off">
              label="Session tile cache"
              value={cacheEnabled ? "on" : "off"}
              options={[
                {
                  value: "on",
                  label: "Cache on",
                  note: "Reuse up to 192 generated tiles while navigating",
                },
                {
                  value: "off",
                  label: "Cache off",
                  note: "Regenerate whenever the generation viewport changes",
                },
              ]}
              onChange={(value) => setCacheEnabled(value === "on")}
            />
            <p className="controlNote">
              On keeps a session-local 192-tile LRU keyed by seed, aligned
              origin, and spacing. Off retains only the active request and
              disables speculative preload.
            </p>
              </>
            ) : null}
            {terrainPaneVisible ? (
              <>
            <SegmentedControl<BenchmarkProfile>
              label="Workload budget"
              value={benchmarkProfile}
              options={[
                {
                  value: "interactive",
                  label: "Interactive",
                  note: "At most eight visible tiles per axis",
                },
                {
                  value: "stress",
                  label: "Stress",
                  note: "At most twelve visible tiles per axis",
                },
              ]}
              onChange={setBenchmarkProfile}
            />
            <div className="buttonRow benchmarkButtons">
              <button
                type="button"
                onClick={() => {
                  setComparison(undefined);
                  setCacheEpoch((current) => current + 1);
                  setCanonicalReport(undefined);
                  setComparisonCanonicalReport(undefined);
                  setCanonicalCacheEpoch((current) => current + 1);
                  setAtlasReport(undefined);
                  setAtlasCacheEpoch((current) => current + 1);
                }}
              >
                Cold current view
              </button>
              <button
                type="button"
                disabled={state.profile === "overworld"}
                onClick={() => {
                  setComparison(undefined);
                  setCacheEnabled(false);
                  setBenchmarkProfile("stress");
                  setCacheEpoch((current) => current + 1);
                  setState((current) => ({
                    ...current,
                    seed: REVIEW_TERRAIN_LAB_STATE.seed,
                    centerX: REVIEW_TERRAIN_LAB_STATE.centerX,
                    centerZ: REVIEW_TERRAIN_LAB_STATE.centerZ,
                    blocksAcross: 2_048,
                    detail: 2,
                    source: "split",
                    panes: ["cpu", "gpu"],
                    view: "map",
                    layer: "terrain",
                    contentStage: "hydrology",
                  }));
                }}
              >
                Run stress race
              </button>
            </div>
            <p className="controlNote">
              {state.profile === "overworld"
                ? "Use Sampled exact and Fast macro together for the vanilla comparison race."
                : "Stress uses the fixed review seed/site, a cold 2 km Compare map at requested 1:2, and independent CPU/GPU publication."}
            </p>
              </>
            ) : null}
          </ControlSection>
          ) : null}

          <ControlSection number="05" title="Evidence" subdued>
            {planVisible ? (
              <LandformPlanPointEvidence
                receipt={planPointReceipt}
                report={planReport}
              />
            ) : null}
            {atlasVisible ? (
              <StreamedPlanAtlasEvidence report={atlasReport} />
            ) : null}
            {semanticVisible ? (
              <SemanticTerrainEvidence report={semanticReport} />
            ) : null}
            {proceduralVisible ? (
              <PointReceipt profile={state.profile} receipt={pointReceipt} />
            ) : null}
            {terrainPaneVisible ? (
              <Diagnostics
                adapter={adapter}
                report={renderReport}
                comparison={comparison}
                canonical={canonicalReport}
              />
            ) : null}
          </ControlSection>
        </aside>
      </main>
    </div>
  );
}

function useResponsiveSplitLayout(): "columns" | "rows" {
  const query = "(max-width: 1040px)";
  const [layout, setLayout] = useState<"columns" | "rows">(() =>
    window.matchMedia(query).matches ? "rows" : "columns"
  );
  useEffect(() => {
    const media = window.matchMedia(query);
    const update = (): void => setLayout(media.matches ? "rows" : "columns");
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  return layout;
}

function LandformPlanPointEvidence({
  receipt,
  report,
}: {
  receipt: LandformPlanPointReceipt | undefined;
  report: LandformPlanReport | undefined;
}): React.JSX.Element {
  if (!report) {
    return (
      <div className="pointReceipt empty" data-testid="landform-plan-receipt">
        <strong>Building the landform plan</strong>
        <span>
          The bounded research summary is compiling off the main thread.
        </span>
      </div>
    );
  }
  return (
    <div className="pointReceipt" data-testid="landform-plan-receipt">
      <div className="pointReceiptHeading">
        <strong>
          {receipt
            ? `${receipt.worldX}, ${receipt.worldZ}`
            : "Tap the plan to inspect it"}
        </strong>
        <span>{report.topology} · 1:32</span>
      </div>
      {receipt ? (
        <dl className="pointReceiptGrid">
          <div><dt>Cell</dt><dd>{receipt.gridX}, {receipt.gridZ}</dd></div>
          <div><dt>Base Y</dt><dd>{receipt.baseY}</dd></div>
          <div><dt>Basin</dt><dd>{receipt.basinId}</dd></div>
          <div><dt>Receiver</dt><dd>{receipt.receiverId}</dd></div>
          <div><dt>Accumulation</dt><dd>{receipt.accumulation}</dd></div>
          <div><dt>Stream order</dt><dd>{receipt.streamOrder || "—"}</dd></div>
          <div><dt>Uplift</dt><dd>{receipt.uplift.toFixed(3)}</dd></div>
          <div><dt>Quiet</dt><dd>{receipt.quiet.toFixed(3)}</dd></div>
          <div><dt>Broad low</dt><dd>{receipt.broadLow.toFixed(3)}</dd></div>
          <div>
            <dt>Structural flags</dt>
            <dd>{planPointFlags(receipt).join(" · ") || "ordinary"}</dd>
          </div>
        </dl>
      ) : (
        <span className="pointReceiptPrompt">
          Inspection reports basin ownership, receiver hierarchy, drainage
          accumulation/order, envelope strengths, and structural flags.
        </span>
      )}
      <details className="pointReceiptDetails">
        <summary>Plan build evidence</summary>
        <dl className="pointReceiptGrid">
          <div><dt>Build</dt><dd>{report.buildMs.toFixed(2)} ms</dd></div>
          <div><dt>Summary</dt><dd>{formatBytes(report.transferBytes)}</dd></div>
          <div><dt>Channels</dt><dd>{formatInteger(report.channelCells)}</dd></div>
          <div><dt>Confluences</dt><dd>{formatInteger(report.confluences)}</dd></div>
          <div>
            <dt>Drainage / divides</dt>
            <dd>
              {formatInteger(report.drainageSegments)}
              {" / "}
              {formatInteger(report.divideSegments)}
            </dd>
          </div>
          <div><dt>Protected sinks</dt><dd>{report.protectedSinks}</dd></div>
          <div><dt>Schema</dt><dd>{report.schema}</dd></div>
          <div><dt>Checksum</dt><dd>{report.checksum}</dd></div>
        </dl>
      </details>
    </div>
  );
}

function planPointFlags(receipt: LandformPlanPointReceipt): string[] {
  return [
    receipt.ocean && "ocean",
    receipt.channel && "channel",
    receipt.confluence && "confluence",
    receipt.protectedBasin && "protected basin",
    receipt.quietCore && "quiet core",
    receipt.cropEdge && "study edge",
  ].filter((flag): flag is string => flag !== false);
}

function StreamedPlanAtlasEvidence({
  report,
}: {
  report: StreamedPlanAtlasReport | undefined;
}): React.JSX.Element {
  if (!report) {
    return (
      <div className="pointReceipt empty" data-testid="streamed-plan-atlas-evidence">
        <strong>Querying canonical plan regions</strong>
        <span>
          Rust is resolving three prior candidates and the multiscale
          refinement witness around this viewport.
        </span>
      </div>
    );
  }
  return (
    <div className="pointReceipt" data-testid="streamed-plan-atlas-evidence">
      <div className="pointReceiptHeading">
        <strong>Streamed planner atlas</strong>
        <span>
          {report.topology} · {report.queryMs.toFixed(2)} ms
        </span>
      </div>
      <dl className="pointReceiptGrid">
        <AtlasCandidateEvidence label="Fallback" receipt={report.fallback} />
        <AtlasCandidateEvidence label="Hierarchy" receipt={report.hierarchy} />
        <AtlasCandidateEvidence label="Graph" receipt={report.featureGraph} />
        <AtlasCandidateEvidence
          label="Refinement"
          receipt={report.multiscaleWitness}
        />
        <div>
          <dt>Refinement facts</dt>
          <dd>
            {report.multiscaleWitness.parentFactCount} parent ·{" "}
            {report.multiscaleWitness.regionalFactCount} regional ·{" "}
            {report.multiscaleWitness.localFactCount} local
          </dd>
        </div>
        <div>
          <dt>Refinement invariants</dt>
          <dd>
            {report.multiscaleWitness.unresolvedParentCount
              + report.multiscaleWitness.containmentFailureCount
              + report.multiscaleWitness.continuityFailureCount
              + report.multiscaleWitness.terminalFailureCount === 0
              ? "exact · 0 failures"
              : "failed"}
          </dd>
        </div>
        <div>
          <dt>Coverage</dt>
          <dd>{report.coverageClipped ? "cost-capped" : "complete viewport"}</dd>
        </div>
        <div>
          <dt>Phase 2 witness</dt>
          <dd>{report.phaseTwoWitnessSha256.slice(0, 16)}…</dd>
        </div>
      </dl>
      <span className="pointReceiptPrompt">
        Checksums cover canonical semantic snapshots, not canvas pixels.
        Cache hits, eviction, and periodic lifts cannot change them.
      </span>
    </div>
  );
}

function SemanticTerrainEvidence({
  report,
}: {
  report: SemanticTerrainReport | undefined;
}): React.JSX.Element {
  if (!report) {
    return (
      <div className="pointReceipt empty" data-testid="semantic-terrain-evidence">
        <strong>Reconstructing semantic terrain</strong>
        <span>
          Rust is compiling parent, regional, local, and correction surfaces
          off the main thread.
        </span>
      </div>
    );
  }
  return (
    <div className="pointReceipt" data-testid="semantic-terrain-evidence">
      <div className="pointReceiptHeading">
        <strong>Semantic terrain sandbox</strong>
        <span>
          {report.compileMs.toFixed(2)} ms compile · {report.drawMs.toFixed(2)} ms draw
        </span>
      </div>
      <dl className="pointReceiptGrid">
        <div>
          <dt>Height range</dt>
          <dd>{report.minimumHeight.toFixed(2)}–{report.maximumHeight.toFixed(2)}</dd>
        </div>
        <div><dt>Samples</dt><dd>{formatInteger(report.sampleCount)}</dd></div>
        <div>
          <dt>Feature facts</dt>
          <dd>
            {report.parent.featureCount} parent · {report.regional.featureCount}
            {" "}regional · {report.local.featureCount} local
          </dd>
        </div>
        <div>
          <dt>Distance evaluations</dt>
          <dd>
            {formatInteger(report.parent.distanceEvaluationCount)} /{" "}
            {formatInteger(report.regional.distanceEvaluationCount)} /{" "}
            {formatInteger(report.local.distanceEvaluationCount)}
          </dd>
        </div>
        <div>
          <dt>Correction RMS</dt>
          <dd>{report.correction.rms.toFixed(3)} blocks</dd>
        </div>
        <div>
          <dt>Changed samples</dt>
          <dd>{(report.correction.changedSampleFraction * 100).toFixed(1)}%</dd>
        </div>
        <div>
          <dt>Terrain checksum</dt>
          <dd>{report.terrainSha256.slice(0, 16)}…</dd>
        </div>
        <div>
          <dt>Pinned suite</dt>
          <dd>{report.suiteSha256.slice(0, 16)}…</dd>
        </div>
      </dl>
      <span className="pointReceiptPrompt">
        Research only · production terrain unchanged. Checksums cover
        quantized Rust surfaces, not canvas pixels.
      </span>
    </div>
  );
}

function AtlasCandidateEvidence({
  label,
  receipt,
}: {
  label: string;
  receipt: Pick<
    StreamedPlanAtlasReport["fallback"],
    | "canonicalPlanCount"
    | "cacheHits"
    | "cacheMisses"
    | "semanticSha256"
  >;
}): React.JSX.Element {
  return (
    <div>
      <dt>{label}</dt>
      <dd>
        {receipt.canonicalPlanCount} plans · {receipt.cacheHits}h/
        {receipt.cacheMisses}m · {receipt.semanticSha256.slice(0, 10)}
      </dd>
    </div>
  );
}

function PointReceipt({
  profile,
  receipt,
}: {
  profile: TerrainLabProfile;
  receipt: TerrainLabPointReceipt | undefined;
}): React.JSX.Element {
  if (profile === "overworld") {
    return (
      <div className="pointReceipt empty" data-testid="point-receipt">
        <strong>Vanilla point receipts are not in the first pass</strong>
        <span>
          The worker-backed LOD shows height, ocean fill, biome, and approximate
          surface material. Exact chunk inspection remains available visually.
        </span>
      </div>
    );
  }
  if (!receipt) {
    return (
      <div className="pointReceipt empty" data-testid="point-receipt">
        <strong>Tap terrain to inspect it</strong>
        <span>
          The receipt is rebuilt from the production CPU generator, including
          nearby planned-stream records.
        </span>
      </div>
    );
  }
  return (
    <div className="pointReceipt" data-testid="point-receipt">
      <div className="pointReceiptHeading">
        <strong>{receipt.worldX}, {receipt.worldZ}</strong>
        <span>chunk {receipt.chunkX}, {receipt.chunkZ}</span>
      </div>
      <dl className="pointReceiptGrid">
        <div><dt>Landform</dt><dd>{receipt.landform}</dd></div>
        <div><dt>Hydrology</dt><dd>{receipt.hydrology}</dd></div>
        <div><dt>Biome</dt><dd>{receipt.biomeRecipe}</dd></div>
        <div><dt>Why</dt><dd>{receipt.biomeReason}</dd></div>
        <div><dt>Surface</dt><dd>{receipt.surfaceRecipe}</dd></div>
        <div><dt>Height</dt><dd>{receipt.baseSurfaceY} → {receipt.surfaceY}</dd></div>
        <div>
          <dt>River contour</dt>
          <dd>
            {receipt.riverSignedDistance.toFixed(2)} / ±
            {receipt.riverHalfWidth.toFixed(2)}
          </dd>
        </div>
        <div><dt>Channel</dt><dd>{receipt.channelInfluence.toFixed(3)}</dd></div>
        <div><dt>Bank</dt><dd>{receipt.bankInfluence.toFixed(3)}</dd></div>
        <div><dt>Wetland</dt><dd>{receipt.wetlandInfluence.toFixed(3)}</dd></div>
        <div>
          <dt>Planned stream</dt>
          <dd>
            {receipt.plannedStreamStart
              ? `${receipt.plannedStreamStart.chunkX}, ${
                  receipt.plannedStreamStart.chunkZ
                } · ${receipt.plannedStreamInfluence.toFixed(3)}`
              : "none"}
          </dd>
        </div>
        <div>
          <dt>Climate</dt>
          <dd>{receipt.temperature.toFixed(3)} / {receipt.moisture.toFixed(3)}</dd>
        </div>
      </dl>
      <details className="pointReceiptDetails">
        <summary>Production fields and revisions</summary>
        <dl className="pointReceiptGrid">
          <div><dt>Field</dt><dd>{shortRevision(receipt.fieldRevision)}</dd></div>
          <div><dt>Preview</dt><dd>{shortRevision(receipt.previewSchemaRevision)}</dd></div>
          <div><dt>GPU evaluator</dt><dd>{shortRevision(receipt.gpuEvaluatorRevision)}</dd></div>
          <div><dt>Decoration</dt><dd>{shortRevision(receipt.decorationRevision)}</dd></div>
          <div><dt>Carve delta</dt><dd>{receipt.carveDelta} blocks</dd></div>
          <div><dt>Slope</dt><dd>{receipt.slope.toFixed(3)}</dd></div>
          <div><dt>Mountain</dt><dd>{receipt.mountainStrength.toFixed(3)}</dd></div>
          <div><dt>Exposure</dt><dd>{receipt.exposure.toFixed(3)}</dd></div>
          <div><dt>Continental</dt><dd>{receipt.continentalness.toFixed(4)}</dd></div>
          <div><dt>Relief</dt><dd>{receipt.relief.toFixed(4)}</dd></div>
          <div><dt>Ruggedness</dt><dd>{receipt.ruggedness.toFixed(4)}</dd></div>
          <div><dt>Ridges</dt><dd>{receipt.ridges.toFixed(4)}</dd></div>
          <div><dt>Mountain detail</dt><dd>{receipt.mountainDetail.toFixed(4)}</dd></div>
          <div>
            <dt>Adjusted temp</dt>
            <dd>{receipt.adjustedTemperature.toFixed(4)}</dd>
          </div>
          <div>
            <dt>Wetland pool</dt>
            <dd>{receipt.wetlandPoolInfluence.toFixed(4)}</dd>
          </div>
          <div>
            <dt>Submerged outlet</dt>
            <dd>{receipt.submergedOutletInfluence.toFixed(4)}</dd>
          </div>
          <div>
            <dt>Bed / water Y</dt>
            <dd>{receipt.bedY} / {receipt.waterSurfaceY}</dd>
          </div>
          <div>
            <dt>Flow X / Z</dt>
            <dd>{receipt.flowX.toFixed(3)} / {receipt.flowZ.toFixed(3)}</dd>
          </div>
          <div><dt>Grade</dt><dd>{receipt.grade.toFixed(4)}</dd></div>
        </dl>
      </details>
    </div>
  );
}

function PaneToggles({
  profile,
  panes,
  onToggle,
}: {
  profile: TerrainLabProfile;
  panes: TerrainLabPane[];
  onToggle: (pane: TerrainLabPane) => void;
}): React.JSX.Element {
  return (
    <fieldset className="segmentedField paneToggleField">
      <legend>Visible panes</legend>
      <div className="segmentedControl">
        {PANE_OPTIONS.filter((option) =>
          profile === "overworld"
            ? option.value !== "gpu"
              && option.value !== "runtime"
              && option.value !== "plan"
              && option.value !== "atlas"
              && option.value !== "semantic"
            : option.value !== "macro"
        ).map((option) => {
          const visible = panes.includes(option.value);
          const label = profile === "overworld" && option.value === "cpu"
            ? "Sampled exact"
            : option.label;
          const note = profile === "overworld" && option.value === "cpu"
            ? "Direct vanilla density-column height sampling"
            : option.note;
          return (
            <button
              type="button"
              key={option.value}
              className={visible ? "active" : ""}
              aria-pressed={visible}
              title={note}
              onClick={() => onToggle(option.value)}
            >
              {label}
            </button>
          );
        })}
      </div>
    </fieldset>
  );
}

function WorkspaceGuide({
  profile,
  visualProfile,
  panes,
  surfaceQuality,
}: {
  profile: TerrainLabProfile;
  visualProfile: TerrainLabVisualProfile;
  panes: TerrainLabPane[];
  surfaceQuality: TerrainLabState["surfaceQuality"];
}): React.JSX.Element {
  const exact = panes.includes("canonical");
  const plan = panes.includes("plan");
  const atlas = panes.includes("atlas");
  const semantic = panes.includes("semantic");
  const cpu = panes.includes("cpu");
  const macro = panes.includes("macro");
  const gpu = panes.includes("gpu");
  return (
    <div className="sourceGuide">
      <strong>Same coordinates, independent readiness</strong>
      <span>
        {profile === "overworld"
          ? "This workspace is globally vanilla: real chunks, sampled exact, and fast macro use the Java 1.17.1 overworld family. "
          : ""}
        {exact
          ? `Real terrain uses final production chunks with the ${
              visualProfileLabel(visualProfile)
            } material profile, water, and features. `
          : ""}
        {cpu && (macro || gpu)
          ? `${profile === "overworld" ? "Sampled exact and Fast macro" : "CPU and GPU LOD"} panes publish independently at the exact same coordinates. `
          : cpu
            ? `${profile === "overworld" ? "Sampled exact" : "CPU LOD"} shows the broad production surface. `
            : macro
              ? "Fast macro shows the bounded sparse-density approximation. "
            : gpu
              ? "GPU LOD stays resident for broad visual coverage. "
              : ""}
        {cpu || macro || gpu
          ? surfaceQuality === "inferred"
            ? "Inferred surface detail adds one builder-noise lookup per retained vanilla point. "
            : "Basic surface detail uses the already-selected biome top material only. "
          : ""}
        {plan
          ? "Landform plan is a research-only 2D structural diagnostic over the fixed 6.144 km study domain. "
          : ""}
        {atlas
          ? "Planner atlas queries deterministic canonical regions around the freely pannable viewport and compares the Phase 2 fallback, hierarchy, bounded graphs, and multiscale refinement witness. "
          : ""}
        {semantic
          ? "Semantic terrain independently reconstructs parent, regional, and local range/basin courses plus their bounded correction; Mclone Overworld does not consume it. "
          : ""}
        Every visible pane shares seed, center, scale, and navigation. Terrain
        panes also share camera state.
      </span>
    </div>
  );
}

function SourceFootnote({
  profile,
  visualProfile,
  panes,
}: {
  profile: TerrainLabProfile;
  visualProfile: TerrainLabVisualProfile;
  panes: TerrainLabPane[];
}): React.JSX.Element {
  if (profile === "overworld") {
    return (
      <>
        Real terrain uses exact vanilla 1.17.1 production chunks. Sampled exact
        directly samples vanilla density columns; Fast macro uses a center-biome
        estimate and nine vertical density probes per column. Both run in
        independent workers and approximate surface material without generating
        chunks, features, or vegetation.
      </>
    );
  }
  if (panes.length === 1 && panes[0] === "plan") {
    return (
      <>
        Research-only hybrid structure over the fixed 6.144 km Tactical 267
        plane domain. The 32-block cells and skeleton are diagnostic summaries;
        production terrain does not consume this plan.
      </>
    );
  }
  if (panes.length === 1 && panes[0] === "atlas") {
    return (
      <>
        Research-only streamed-plan comparison. Rust owns canonical identity,
        topology lifts, bounded caches, and semantic checksums; the browser
        draws typed facts. Production terrain consumes none of these candidates.
      </>
    );
  }
  if (panes.length === 1 && panes[0] === "semantic") {
    return (
      <>
        Research-only semantic reconstruction over flat or quiet substrate.
        Rust owns feature identity, topology, sampling, surfaces, and exact
        checksums; the browser only draws the four synchronized views.
        Production terrain consumes none of it.
      </>
    );
  }
  return (
    <>
      Real terrain is exact generated blocks with {visualProfileLabel(visualProfile)}
      {" "}materials and preview lighting. Mclone Original fills uncurated
      materials with coherent provisional art; Coverage Debug is the opt-in
      numbered missing-texture view. LOD panes use the same material profile
      while remaining presentation-only. CPU and GPU LOD share natural rivers
      and wetlands; planned streams are reconstructed from production route
      records at near-detail checkpoints. The Landform plan pane is a
      research-only fixed 2D summary. The Planner atlas is a freely pannable
      structural comparison. Neither is consumed by production terrain.
      {" "}Semantic terrain is likewise an isolated reconstruction sandbox.
    </>
  );
}

function VisualProfileUnavailable({
  profile,
  reason,
  comparison = false,
}: {
  profile: TerrainLabVisualProfile;
  reason: string;
  comparison?: boolean;
}): React.JSX.Element {
  return (
    <div
      className="terrainStage visualProfileUnavailable"
      data-testid="visual-profile-unavailable"
      data-comparison={comparison ? "true" : "false"}
      data-visual-profile={profile}
    >
      <div>
        <span>{comparison ? "Comparison unavailable" : "Material profile unavailable"}</span>
        <strong>{visualProfileLabel(profile)}</strong>
        <p>{reason}</p>
      </div>
    </div>
  );
}

function ControlSection({
  number,
  title,
  subdued = false,
  children,
}: {
  number: string;
  title: string;
  subdued?: boolean;
  children: React.ReactNode;
}): React.JSX.Element {
  return (
    <section className={`controlSection${subdued ? " subdued" : ""}`}>
      <div className="sectionHeading">
        <span>{number}</span>
        <h2>{title}</h2>
      </div>
      <div className="sectionBody">{children}</div>
    </section>
  );
}

function SeedControl({
  value,
  onCommit,
}: {
  value: string;
  onCommit: (value: string) => void;
}): React.JSX.Element {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const commit = (): void => {
    const valid = validSeed(draft);
    if (valid !== undefined) {
      onCommit(valid);
    } else {
      setDraft(value);
    }
  };
  return (
    <label className="fieldLabel">
      <span>Signed seed</span>
      <input
        value={draft}
        inputMode="numeric"
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            commit();
            event.currentTarget.blur();
          }
        }}
      />
    </label>
  );
}

function NumberControl({
  label,
  value,
  onCommit,
}: {
  label: string;
  value: number;
  onCommit: (value: number) => void;
}): React.JSX.Element {
  const [draft, setDraft] = useState(String(value));
  useEffect(() => setDraft(String(value)), [value]);
  const commit = (): void => {
    const parsed = Number.parseInt(draft, 10);
    if (
      Number.isInteger(parsed)
      && parsed >= -2_147_483_648
      && parsed <= 2_147_483_647
    ) {
      onCommit(parsed);
    } else {
      setDraft(String(value));
    }
  };
  return (
    <label className="fieldLabel">
      <span>{label}</span>
      <input
        value={draft}
        inputMode="numeric"
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            commit();
            event.currentTarget.blur();
          }
        }}
      />
    </label>
  );
}

function SegmentedControl<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: Array<{ value: T; label: string; note?: string }>;
  onChange: (value: T) => void;
}): React.JSX.Element {
  return (
    <fieldset className="segmentedField">
      <legend>{label}</legend>
      <div className="segmentedControl">
        {options.map((option) => (
          <button
            type="button"
            key={option.value}
            className={value === option.value ? "active" : ""}
            aria-pressed={value === option.value}
            title={option.note}
            onClick={() => onChange(option.value)}
          >
            {option.label}
          </button>
        ))}
      </div>
    </fieldset>
  );
}

function ToggleButton({
  label,
  pressed,
  onChange,
}: {
  label: string;
  pressed: boolean;
  onChange: (pressed: boolean) => void;
}): React.JSX.Element {
  return (
    <button
      type="button"
      className={pressed ? "active" : ""}
      aria-pressed={pressed}
      onClick={() => onChange(!pressed)}
    >
      {label} {pressed ? "shown" : "hidden"}
    </button>
  );
}

function PanPad({
  state,
  camera,
  onChange,
  onError,
}: {
  state: TerrainLabState;
  camera: TerrainLabCamera;
  onChange: (state: TerrainLabState) => void;
  onError: (error: unknown) => void;
}): React.JSX.Element {
  const pan = (x: number, z: number): void => {
    void panTerrainLabByFraction(state, camera, x, z, 0.25)
      .then(onChange)
      .catch(onError);
  };
  return (
    <div className="panControl">
      <span>Pan a quarter view</span>
      <div className="panPad" aria-label="Pan controls">
        <button type="button" onClick={() => pan(0, -1)} aria-label="Pan north">↑</button>
        <button type="button" onClick={() => pan(-1, 0)} aria-label="Pan west">←</button>
        <button type="button" onClick={() => pan(0, 1)} aria-label="Pan south">↓</button>
        <button type="button" onClick={() => pan(1, 0)} aria-label="Pan east">→</button>
      </div>
    </div>
  );
}

function Diagnostics({
  adapter,
  report,
  comparison,
  canonical,
}: {
  adapter: TerrainLabAdapterReport | undefined;
  report: TerrainLabRenderReport | undefined;
  comparison: TerrainLabComparisonReport | undefined;
  canonical: CanonicalTerrainReport | undefined;
}): React.JSX.Element {
  const memory = report ? formatBytes(report.residentBytes) : "—";
  const vanilla = report?.profile === "overworld";
  const primaryLabel = vanilla ? "Sampled exact" : "CPU";
  const alternateLabel = vanilla ? "Fast macro" : "GPU";
  const samplesPerTile = report ? report.samplesPerAxis ** 2 : 0;
  const cpuRequestSamples = report
    ? report.requestCpuCompiledTiles * samplesPerTile
    : 0;
  const alternateRequestSamples = report
    ? (vanilla ? report.requestMacroCompiledTiles : report.requestGpuDispatchedTiles)
      * samplesPerTile
    : 0;
  const adapterLabel = useMemo(() => {
    if (!adapter) {
      return "requesting adapter";
    }
    return adapter.name || `${adapter.backend} adapter`;
  }, [adapter]);
  return (
    <div className="diagnostics" data-testid="terrain-diagnostics">
      <div className="metricGrid">
        <Metric
          label="Real chunks"
          value={canonical
            ? `${canonical.publishedChunks}/${canonical.requestedChunks}`
            : "hidden"}
        />
        <Metric label="Real first chunk" value={formatMs(canonical?.firstChunkMs)} />
        <Metric label="Real complete" value={formatMs(canonical?.completeMs)} />
        <Metric label="Real generation" value={formatMs(canonical?.generationMs)} />
        <Metric label="Real Worker presentation" value={formatMs(canonical?.workerPresentationMs)} />
        <Metric label="Real Worker mesh" value={formatMs(canonical?.workerMeshMs)} />
        <Metric label="Real Worker pack" value={formatMs(canonical?.workerPackMs)} />
        <Metric label="Real Worker transfer" value={formatMs(canonical?.workerTransferMs)} />
        <Metric
          label="Real Worker transport"
          value={canonical
            ? `${canonical.transportKind} · ${canonical.crossOriginIsolated ? "isolated" : "not isolated"}`
            : "—"}
        />
        <Metric
          label="Real result arena"
          value={canonical
            ? `${formatBytes(canonical.resultArenaHighWaterBytes)} / ${formatBytes(canonical.resultArenaCapacityBytes)} · ${canonical.resultArenaOverflowCount} overflow`
            : "—"}
        />
        <Metric label="Real main decode" value={formatMs(canonical?.mainDecodeMs)} />
        <Metric label="Real GPU upload" value={formatMs(canonical?.meshUploadMs)} />
        <Metric label="Real max admission" value={formatMs(canonical?.maxAdmissionMs)} />
        <Metric
          label="Real mesh targets"
          value={canonical ? `${canonical.meshTargetChunks} deduplicated` : "—"}
        />
        <Metric
          label="Real cache"
          value={canonical
            ? `${canonical.cacheHits} hits · ${canonical.cachedChunks} chunks`
            : "—"}
        />
        <Metric
          label="Real tracked (lower bound)"
          value={canonical ? formatBytes(canonical.trackedBytes) : "—"}
        />
        <Metric
          label="Real main raw"
          value={canonical ? formatBytes(canonical.residentRawBytes) : "—"}
        />
        <Metric
          label="Real Worker raw cache"
          value={canonical ? formatBytes(canonical.cacheRawBytes) : "—"}
        />
        <Metric
          label="Real mesh used"
          value={canonical ? formatBytes(canonical.residentMeshUsedBytes) : "—"}
        />
        <Metric
          label="Real resident reuse"
          value={canonical ? `${canonical.residentHits} chunks` : "—"}
        />
        <Metric
          label="Real warm reuse"
          value={canonical
            ? `${canonical.warmHits} hits · ${canonical.warmChunks} inactive`
            : "—"}
        />
        <Metric
          label="Real admission"
          value={canonical
            ? `${canonical.admissionFrames} frames · max ${canonical.maxFrameAdmissions}/frame`
            : "—"}
        />
        <Metric label="CPU reference / frame" value={formatMs(report?.cpuReferenceMs)} />
        <Metric label="CPU vegetation / frame" value={formatMs(report?.cpuVegetationMs)} />
        <Metric label="CPU pack + upload / frame" value={formatMs(report?.cpuPackUploadMs)} />
        <Metric label="Encode + submit" value={formatMs(report?.encodeSubmitMs)} />
        <Metric label={`${primaryLabel} coarse`} value={formatMs(report?.cpuCoarseReadyMs)} />
        <Metric label={`${alternateLabel} coarse${vanilla ? "" : " + readback"}`} value={formatMs(report?.gpuCoarseReadyMs)} />
        <Metric label={`${primaryLabel} target`} value={formatMs(report?.cpuTargetReadyMs)} />
        <Metric label={`${alternateLabel} target${vanilla ? "" : " + readback"}`} value={formatMs(report?.gpuTargetReadyMs)} />
        <Metric
          label={`${primaryLabel} end-to-end`}
          value={formatSampleRate(
            cpuRequestSamples,
            report?.cpuTargetReadyMs,
            report?.cpuTargetReady,
          )}
        />
        <Metric
          label={`${alternateLabel} end-to-end`}
          value={formatSampleRate(
            alternateRequestSamples,
            vanilla ? report?.requestMacroCompileMs : report?.gpuTargetReadyMs,
            report?.gpuTargetReady,
          )}
        />
        <Metric
          label={`${alternateLabel} execution`}
          value={vanilla ? formatMs(report?.requestMacroCompileMs) : "unavailable"}
        />
        <Metric
          label={`${primaryLabel} level`}
          value={report
            ? `${report.cpuPublishedTileCount}/${report.visibleTileCount} · ${
                report.cpuPublishedSpacing ? `1:${report.cpuPublishedSpacing}` : "waiting"
              }`
            : "—"}
        />
        <Metric
          label={`${alternateLabel} level`}
          value={report
            ? `${report.gpuPublishedTileCount}/${report.visibleTileCount} · ${
                report.gpuPublishedSpacing ? `1:${report.gpuPublishedSpacing}` : "waiting"
              }`
            : "—"}
        />
        <Metric label="Base mean Δ" value={formatBlocks(comparison?.meanAbsoluteBaseSurfaceError)} />
        <Metric label="Base P95 Δ" value={formatBlocks(comparison?.p95AbsoluteBaseSurfaceError)} />
        <Metric label="Base maximum Δ" value={formatBlocks(comparison?.maxAbsoluteBaseSurfaceError)} />
        <Metric
          label="Ocean agreement"
          value={comparison ? `${(comparison.oceanWaterPresenceAgreement * 100).toFixed(1)}%` : "pending"}
        />
        <Metric
          label="Material agreement"
          value={comparison
            ? `${(comparison.macroSurfaceMaterialAgreement * 100).toFixed(1)}%`
            : "pending"}
        />
        <Metric label="Final mean Δ" value={formatBlocks(comparison?.meanAbsoluteSurfaceError)} />
        <Metric label="Final P95 Δ" value={formatBlocks(comparison?.p95AbsoluteSurfaceError)} />
        <Metric label="Display mean Δ" value={formatBlocks(comparison?.meanAbsoluteDisplayError)} />
        <Metric label="Display P95 Δ" value={formatBlocks(comparison?.p95AbsoluteDisplayError)} />
        <Metric label="Display maximum Δ" value={formatBlocks(comparison?.maxAbsoluteDisplayError)} />
        <Metric
          label="River agreement"
          value={comparison
            ? `${(comparison.channelPresenceAgreement * 100).toFixed(1)}%`
            : "pending"}
        />
        <Metric
          label="River contour mean Δ"
          value={formatBlocks(comparison?.meanAbsoluteRiverSignedDistanceError)}
        />
        <Metric
          label="Channel influence Δ"
          value={formatDecimal(comparison?.meanAbsoluteChannelInfluenceError)}
        />
        <Metric
          label="Bank influence Δ"
          value={formatDecimal(comparison?.meanAbsoluteBankInfluenceError)}
        />
        <Metric
          label="Wetland influence Δ"
          value={formatDecimal(comparison?.meanAbsoluteWetlandInfluenceError)}
        />
        <Metric
          label="Visible material"
          value={comparison
            ? `${(comparison.visibleSurfaceMaterialAgreement * 100).toFixed(1)}%`
            : "pending"}
        />
        <Metric
          label="Landform agreement"
          value={comparison
            ? `${(comparison.landformKindAgreement * 100).toFixed(3)}%`
            : "pending"}
        />
        <Metric
          label="Biome agreement"
          value={comparison
            ? `${(comparison.biomeRecipeAgreement * 100).toFixed(1)}%`
            : "pending"}
        />
        <Metric
          label="Surface agreement"
          value={comparison
            ? `${(comparison.surfaceRecipeAgreement * 100).toFixed(1)}%`
            : "pending"}
        />
        <Metric label="LOD resident" value={memory} />
        <Metric
          label="Vegetation products"
          value={report
            ? `${formatInteger(report.vegetationSummaryTileCount)} summary · ${
                formatInteger(report.vegetationRecordTileCount)
              } records · ${formatInteger(report.vegetationAggregatedTileCount)} aggregated`
            : "—"}
        />
        <Metric
          label="Vegetation summary lanes"
          value={report
            ? `${formatInteger(report.cpuVegetationSummaryTileCount)} CPU · ${
                formatInteger(report.gpuVegetationSummaryTileCount)
              } GPU`
            : "—"}
        />
        <Metric
          label="CPU Cover work"
          value={report
            ? `${formatInteger(report.requestCpuTerrainSampleEvaluations)} terrain · ${
                formatInteger(report.requestCpuForestIntentEvaluations)
              } forest · ${
                formatInteger(report.requestCpuForestFootprintSummaries)
              } footprints`
            : "—"}
        />
        <Metric
          label="GPU Cover work"
          value={report
            ? `${formatInteger(report.requestGpuTerrainSampleEvaluations)} terrain · ${
                formatInteger(report.requestGpuForestIntentEvaluations)
              } forest · ${
                formatInteger(report.requestGpuForestFootprintSummaries)
              } footprints`
            : "—"}
        />
        <Metric
          label="Tree proxies"
          value={report
            ? `${formatInteger(report.treeInstanceCount)} trees · ${
                formatInteger(report.treeProxyVertexCount)
              } vertices · ${formatBytes(report.treeInstanceBytes)}`
            : "—"}
        />
        <Metric
          label="Vegetation cache"
          value={report
            ? `${formatInteger(report.vegetationCellHits)}/${
                formatInteger(report.vegetationCellRequests)
              } hits · ${formatInteger(report.vegetationCellMisses)} misses · ${
                formatInteger(report.retainedVegetationCells)
              } retained`
            : "—"}
        />
        <Metric
          label={`${primaryLabel} / ${alternateLabel} queue`}
          value={report
            ? `${formatInteger(report.cpuQueuedTileCount)} / ${
                formatInteger(report.gpuQueuedTileCount)
              }`
            : "—"}
        />
      </div>
      <dl className="detailList">
        <div><dt>Adapter</dt><dd data-testid="adapter-name">{adapterLabel}</dd></div>
        <div><dt>Backend</dt><dd>{adapter?.backend ?? "—"}</dd></div>
        <div>
          <dt>Content</dt>
          <dd>
            {report
              ? `${report.contentStage} · planned streams ${
                  report.structuredHydrologyAvailable ? "available" : "unavailable at this LOD"
                }`
              : "—"}
          </dd>
        </div>
        <div>
          <dt>Detail</dt>
          <dd>
            {report
              ? `${report.requestedDetail} → 1:${report.effectiveSpacing}`
              : "—"}
          </dd>
        </div>
        <div>
          <dt>Published</dt>
          <dd>
            {report
              ? `${primaryLabel} 1:${report.cpuPublishedSpacing || "—"} · ${alternateLabel} 1:${
                  report.gpuPublishedSpacing || "—"
                }`
              : "—"}
          </dd>
        </div>
        <div>
          <dt>Cache</dt>
          <dd>
            {report
              ? `${report.cacheEnabled ? "on" : "off"} · ${
                  report.requestCacheHitTiles
                } hits · ${report.residentTileCount} resident`
              : "—"}
          </dd>
        </div>
        <div><dt>Samples</dt><dd>{report ? formatInteger(report.sampleCount) : "—"}</dd></div>
        <div>
          <dt>CPU compile rate</dt>
          <dd>
            {formatSampleRate(
              cpuRequestSamples,
              report?.requestCpuReferenceMs,
              report?.cpuTargetReady,
            )}
          </dd>
        </div>
        <div><dt>Field</dt><dd>{shortRevision(report?.fieldRevision)}</dd></div>
        <div>
          <dt>{vanilla ? "Macro evaluator" : "GPU evaluator"}</dt>
          <dd>
            {shortRevision(vanilla
              ? report?.macroEvaluatorRevision
              : report?.gpuEvaluatorRevision)}
          </dd>
        </div>
        <div><dt>Vegetation</dt><dd>{shortRevision(report?.vegetationRevision)}</dd></div>
        <div><dt>Texture sampling</dt><dd>5 mip · trilinear minification</dd></div>
      </dl>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }): React.JSX.Element {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function formatDistance(blocks: number): string {
  if (blocks >= 1_000) {
    return `${(blocks / 1_000).toLocaleString(undefined, { maximumFractionDigits: 1 })} km`;
  }
  return `${formatInteger(blocks)} ${blocks === 1 ? "block" : "blocks"}`;
}

function formatInteger(value: number): string {
  return value.toLocaleString("en-US", { maximumFractionDigits: 0 });
}

function formatMs(value: number | null | undefined): string {
  return value === undefined || value === null
    ? "—"
    : `${value.toFixed(value >= 10 ? 1 : 2)} ms`;
}

function formatSampleRate(
  samples: number,
  milliseconds: number | null | undefined,
  targetReady: boolean | undefined,
): string {
  if (!samples && targetReady) {
    return "cache hit";
  }
  if (!samples || milliseconds === undefined || milliseconds === null || milliseconds <= 0) {
    return "pending";
  }
  const samplesPerSecond = samples / (milliseconds / 1_000);
  if (samplesPerSecond >= 1_000_000) {
    return `${(samplesPerSecond / 1_000_000).toFixed(2)} M/s`;
  }
  return `${(samplesPerSecond / 1_000).toFixed(1)} K/s`;
}

function formatBlocks(value: number | undefined): string {
  return value === undefined ? "pending" : `${value.toFixed(1)} blocks`;
}

function formatDecimal(value: number | undefined): string {
  return value === undefined ? "pending" : value.toFixed(5);
}

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(2)} MiB`;
  }
  return `${(bytes / 1024).toFixed(1)} KiB`;
}

function shortRevision(value: string | undefined): string {
  if (!value) {
    return "—";
  }
  return value.replace("mclone-overworld-v1-", "").replace("mclone-", "");
}

function randomSeed(): string {
  const words = new Uint32Array(2);
  crypto.getRandomValues(words);
  const high = BigInt(words[1] ?? 0);
  const low = BigInt(words[0] ?? 0);
  return BigInt.asIntN(64, (high << 32n) | low).toString();
}

function parseDetail(value: string): TerrainLabDetail {
  return value === "auto" ? "auto" : Number(value) as TerrainLabDetail;
}

function texturePresentationForProfile(
  profile: TerrainLabVisualProfile,
  presentation: TerrainLabTexturePresentation,
): TerrainLabTexturePresentation {
  return profile === "first-party-coverage" ? "textured" : presentation;
}

function visualProfileLabel(profile: TerrainLabVisualProfile): string {
  return VISUAL_PROFILE_OPTIONS.find((option) => option.value === profile)?.label
    ?? profile;
}

function visualProfileNote(profile: TerrainLabVisualProfile): string {
  return VISUAL_PROFILE_OPTIONS.find((option) => option.value === profile)?.note
    ?? "";
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
