import { useCallback, useEffect, useMemo, useState } from "react";

import {
  DEFAULT_TERRAIN_LAB_CAMERA,
  REVIEW_TERRAIN_LAB_STATE,
  TERRAIN_LAB_CANONICAL_RADII,
  TERRAIN_LAB_SPACINGS,
  footprintBlocks,
  nextBlocksAcross,
  panTerrainLabState,
  parseTerrainLabState,
  proceduralSourceForPanes,
  terrainLabSearch,
  toggleTerrainLabPane,
  validSeed,
  type CanonicalTerrainStage,
  type TerrainLabDetail,
  type TerrainLabContentStage,
  type TerrainLabLayer,
  type TerrainLabCamera,
  type TerrainLabPane,
  type TerrainLabState,
  type TerrainLabView,
} from "../state";
import {
  CanonicalTerrainCanvas,
  type CanonicalTerrainReport,
} from "./CanonicalTerrainCanvas";
import {
  TerrainCanvas,
  type TerrainLabAdapterReport,
  type TerrainLabComparisonReport,
  type TerrainLabPointReceipt,
  type TerrainLabRenderReport,
} from "./TerrainCanvas";

type LabStatus = "loading" | "ready" | "rendering" | "error";
type BenchmarkProfile = "interactive" | "stress";

const PANE_OPTIONS: Array<{ value: TerrainLabPane; label: string; note: string }> = [
  { value: "canonical", label: "Real terrain", note: "Exact final chunks with textures" },
  { value: "cpu", label: "CPU LOD", note: "Production CPU preview evaluator" },
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
];

export function App(): React.JSX.Element {
  const [state, setState] = useState<TerrainLabState>(() =>
    parseTerrainLabState(window.location.search)
  );
  const [status, setStatus] = useState<LabStatus>("loading");
  const [adapter, setAdapter] = useState<TerrainLabAdapterReport>();
  const [renderReport, setRenderReport] = useState<TerrainLabRenderReport>();
  const [comparison, setComparison] = useState<TerrainLabComparisonReport>();
  const [pointReceipt, setPointReceipt] = useState<TerrainLabPointReceipt>();
  const [error, setError] = useState<string>();
  const [camera, setCamera] = useState<TerrainLabCamera>(DEFAULT_TERRAIN_LAB_CAMERA);
  const [cacheEnabled, setCacheEnabled] = useState(true);
  const [cacheEpoch, setCacheEpoch] = useState(0);
  const [canonicalCacheEnabled, setCanonicalCacheEnabled] = useState(true);
  const [canonicalCacheEpoch, setCanonicalCacheEpoch] = useState(0);
  const [canonicalReport, setCanonicalReport] = useState<CanonicalTerrainReport>();
  const [benchmarkProfile, setBenchmarkProfile] =
    useState<BenchmarkProfile>("interactive");

  useEffect(() => {
    const search = terrainLabSearch(state);
    if (window.location.search !== search) {
      window.history.replaceState(null, "", `${window.location.pathname}${search}`);
    }
  }, [state]);

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
  const canonicalVisible = state.panes.includes("canonical");
  const cpuVisible = state.panes.includes("cpu");
  const gpuVisible = state.panes.includes("gpu");
  const proceduralVisible = cpuVisible || gpuVisible;
  const proceduralSource = proceduralSourceForPanes(state.panes);
  const proceduralState = useMemo(
    () => ({ ...state, source: proceduralSource }),
    [proceduralSource, state],
  );
  const workspaceStatus: LabStatus = error
    ? "error"
    : (!proceduralVisible || status === "ready")
      && (!canonicalVisible || canonicalReport?.complete)
      ? "ready"
      : status === "loading" && proceduralVisible
        ? "loading"
        : "rendering";
  const renderStatus = workspaceStatus === "rendering" ? "updating" : workspaceStatus;
  const compareLayout = proceduralSource !== "split"
    ? "single"
    : renderReport && renderReport.width <= renderReport.height
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
      data-base-mean-error={comparison?.meanAbsoluteBaseSurfaceError ?? ""}
      data-base-p95-error={comparison?.p95AbsoluteBaseSurfaceError ?? ""}
      data-ocean-agreement={comparison?.oceanWaterPresenceAgreement ?? ""}
      data-material-agreement={comparison?.macroSurfaceMaterialAgreement ?? ""}
      data-channel-agreement={comparison?.channelPresenceAgreement ?? ""}
      data-landform-agreement={comparison?.landformKindAgreement ?? ""}
      data-stage={state.contentStage}
      data-inspected-x={pointReceipt?.worldX ?? ""}
      data-inspected-z={pointReceipt?.worldZ ?? ""}
      data-continentalness-error={comparison?.meanAbsoluteContinentalnessError ?? ""}
      data-compare-layout={compareLayout}
      data-vertex-count={renderReport?.vertexCount ?? 0}
      data-requested-spacing={renderReport?.requestedSpacing ?? 0}
      data-effective-spacing={renderReport?.effectiveSpacing ?? 0}
      data-published-spacing={renderReport?.publishedSpacing ?? 0}
      data-resident-tiles={renderReport?.residentTileCount ?? 0}
      data-queued-tiles={renderReport?.queuedTileCount ?? 0}
      data-target-ready={renderReport?.targetReady ? "true" : "false"}
      data-cpu-target-ready={renderReport?.cpuTargetReady ? "true" : "false"}
      data-gpu-target-ready={renderReport?.gpuTargetReady ? "true" : "false"}
      data-cpu-target-ms={renderReport?.cpuTargetReadyMs ?? ""}
      data-gpu-target-ms={renderReport?.gpuTargetReadyMs ?? ""}
      data-cpu-request-ms={renderReport?.requestCpuReferenceMs ?? ""}
      data-cache-enabled={cacheEnabled ? "true" : "false"}
      data-cache-hits={renderReport?.requestCacheHitTiles ?? 0}
      data-visible-tiles={renderReport?.visibleTileCount ?? 0}
      data-request-cpu-tiles={renderReport?.requestCpuCompiledTiles ?? 0}
      data-request-gpu-tiles={renderReport?.requestGpuDispatchedTiles ?? 0}
      data-samples-per-axis={renderReport?.samplesPerAxis ?? 0}
      data-panes={state.panes.join(",")}
      data-canonical-published={canonicalReport?.publishedChunks ?? 0}
      data-canonical-requested={canonicalReport?.requestedChunks ?? 0}
      data-canonical-complete={canonicalReport?.complete ? "true" : "false"}
      data-canonical-cache-enabled={canonicalCacheEnabled ? "true" : "false"}
      data-canonical-cache-hits={canonicalReport?.cacheHits ?? 0}
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
          <a className="playLink" href="/">Play Mclone</a>
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
              panes={state.panes}
              onToggle={(pane) => updateState(toggleTerrainLabPane(state, pane))}
            />
            <WorkspaceGuide panes={state.panes} />
          </div>
          <div className="mapToolbar" data-testid="viewport-controls">
            <SegmentedControl<TerrainLabView>
              label="View"
              value={state.view}
              options={[
                { value: "3d", label: "3D terrain" },
                { value: "map", label: "Map" },
              ]}
              onChange={(view) => patchState({ view })}
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
            <div className="mapZoom" aria-label="Viewport zoom controls">
              <button
                type="button"
                onClick={() =>
                  patchState({ blocksAcross: nextBlocksAcross(state.blocksAcross, "in") })
                }
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
                onClick={() =>
                  patchState({ blocksAcross: nextBlocksAcross(state.blocksAcross, "out") })
                }
                aria-label="Zoom out"
              >
                −
              </button>
            </div>
          </div>
          <div
            className={`paneWorkspace logicalPanes${state.panes.length}${
              canonicalVisible && cpuVisible && gpuVisible ? " threePaneWorkspace" : ""
            }`}
            data-testid="pane-workspace"
          >
            {canonicalVisible ? (
              <div className="paneFrame canonicalPaneFrame">
                <CanonicalTerrainCanvas
                  state={state}
                  camera={camera}
                  cacheEnabled={canonicalCacheEnabled}
                  cacheEpoch={canonicalCacheEpoch}
                  onStateChange={updateState}
                  onCameraChange={setCamera}
                  onReport={setCanonicalReport}
                  onError={setError}
                />
              </div>
            ) : null}
            {proceduralVisible ? (
              <div
                className={`paneFrame proceduralPaneFrame${
                  cpuVisible && gpuVisible ? " twoLogicalPanes" : ""
                }`}
              >
                <TerrainCanvas
                  state={proceduralState}
                  camera={camera}
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
                {state.detail === "auto" ? "Auto" : `1:${state.detail}`}
                {" → "}
                1:{renderReport?.effectiveSpacing ?? "—"}
              </strong>
            </div>
            <div className="approximationNote">
              <SourceFootnote panes={state.panes} />
            </div>
          </div>
        </section>

        <aside className="controlRail" aria-label="Terrain Lab controls">
          <ControlSection number="01" title="World">
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
            <PanPad state={state} onChange={updateState} />
          </ControlSection>

          <ControlSection number="03" title="Presentation">
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
                          {side} × {side} · {chunks} {chunks === 1 ? "chunk" : "chunks"}
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
                    onChange={(event) =>
                      patchState({
                        contentStage: event.target.value as TerrainLabContentStage,
                      })
                    }
                  >
                    <option value="base">Base · land and ocean fields</option>
                    <option value="hydrology">Hydrology · rivers and wetlands</option>
                    <option value="structured">
                      Structured · planned streams near 1:1–1:4
                    </option>
                    <option value="surface">Surface · biome materials</option>
                    <option value="cover">Cover · vegetation summary</option>
                  </select>
                </label>
                <p className="controlNote">
                  Checkpoints are ordered preview content, not gameplay switches.
                  Planned streams are reconstructed only through 1:4; coarser
                  views say unavailable instead of inventing them.
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
                    {LAYER_OPTIONS.map((option) => (
                      <option key={option.value} value={option.value}>{option.label}</option>
                    ))}
                  </select>
                </label>
              </>
            ) : null}
            <button
              type="button"
              className="cameraResetButton"
              onClick={() => setCamera(DEFAULT_TERRAIN_LAB_CAMERA)}
            >
              Reset 3D camera
            </button>
          </ControlSection>

          <ControlSection number="04" title="Cache & benchmark">
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
                  setCanonicalCacheEpoch((current) => current + 1);
                }}
              >
                Cold current view
              </button>
              <button
                type="button"
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
              Stress uses the fixed review seed/site, a cold 2 km Compare map
              at requested 1:2, and independent CPU/GPU publication.
            </p>
          </ControlSection>

          <ControlSection number="05" title="Evidence" subdued>
            <PointReceipt receipt={pointReceipt} />
            <Diagnostics
              adapter={adapter}
              report={renderReport}
              comparison={comparison}
              canonical={canonicalReport}
            />
          </ControlSection>
        </aside>
      </main>
    </div>
  );
}

function PointReceipt({
  receipt,
}: {
  receipt: TerrainLabPointReceipt | undefined;
}): React.JSX.Element {
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
  panes,
  onToggle,
}: {
  panes: TerrainLabPane[];
  onToggle: (pane: TerrainLabPane) => void;
}): React.JSX.Element {
  return (
    <fieldset className="segmentedField paneToggleField">
      <legend>Visible panes</legend>
      <div className="segmentedControl">
        {PANE_OPTIONS.map((option) => {
          const visible = panes.includes(option.value);
          return (
            <button
              type="button"
              key={option.value}
              className={visible ? "active" : ""}
              aria-pressed={visible}
              title={option.note}
              onClick={() => onToggle(option.value)}
            >
              {option.label}
            </button>
          );
        })}
      </div>
    </fieldset>
  );
}

function WorkspaceGuide({ panes }: { panes: TerrainLabPane[] }): React.JSX.Element {
  const exact = panes.includes("canonical");
  const cpu = panes.includes("cpu");
  const gpu = panes.includes("gpu");
  return (
    <div className="sourceGuide">
      <strong>Same coordinates, independent readiness</strong>
      <span>
        {exact
          ? "Real terrain uses final production chunks, the first-party atlas, water, and features. Uncurated materials remain visibly marked by generated fallback tiles. "
          : ""}
        {cpu && gpu
          ? "CPU and GPU LOD panes publish independently at the exact same coordinates. "
          : cpu
            ? "CPU LOD shows the broad production surface. "
            : gpu
              ? "GPU LOD stays resident for broad visual coverage. "
              : ""}
        Every visible pane shares seed, center, scale, camera, and navigation.
      </span>
    </div>
  );
}

function SourceFootnote({ panes }: { panes: TerrainLabPane[] }): React.JSX.Element {
  return (
    <>
      Real terrain is exact generated blocks with the production first-party
      atlas and preview lighting. Generated fallback tiles identify materials
      that do not have curated textures yet. LOD panes remain
      presentation-only. CPU and GPU LOD share natural rivers and wetlands;
      planned streams are reconstructed from production route records at
      near-detail checkpoints.
    </>
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
  onChange,
}: {
  state: TerrainLabState;
  onChange: (state: TerrainLabState) => void;
}): React.JSX.Element {
  const distance = footprintBlocks(state) / 4;
  const pan = (x: number, z: number): void =>
    onChange(panTerrainLabState(state, x * distance, z * distance));
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
  const samplesPerTile = report ? report.samplesPerAxis ** 2 : 0;
  const cpuRequestSamples = report
    ? report.requestCpuCompiledTiles * samplesPerTile
    : 0;
  const gpuRequestSamples = report
    ? report.requestGpuDispatchedTiles * samplesPerTile
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
        <Metric label="Real mesh + upload" value={formatMs(canonical?.meshUploadMs)} />
        <Metric
          label="Real cache"
          value={canonical ? `${canonical.cacheHits} hits` : "—"}
        />
        <Metric label="CPU compile / frame" value={formatMs(report?.cpuReferenceMs)} />
        <Metric label="Encode + submit" value={formatMs(report?.encodeSubmitMs)} />
        <Metric label="CPU coarse" value={formatMs(report?.cpuCoarseReadyMs)} />
        <Metric label="GPU coarse + readback" value={formatMs(report?.gpuCoarseReadyMs)} />
        <Metric label="CPU target" value={formatMs(report?.cpuTargetReadyMs)} />
        <Metric label="GPU target + readback" value={formatMs(report?.gpuTargetReadyMs)} />
        <Metric
          label="CPU end-to-end"
          value={formatSampleRate(
            cpuRequestSamples,
            report?.cpuTargetReadyMs,
            report?.cpuTargetReady,
          )}
        />
        <Metric
          label="GPU end-to-end"
          value={formatSampleRate(
            gpuRequestSamples,
            report?.gpuTargetReadyMs,
            report?.gpuTargetReady,
          )}
        />
        <Metric label="GPU execution" value="unavailable" />
        <Metric
          label="CPU level"
          value={report
            ? `${report.cpuPublishedTileCount}/${report.visibleTileCount} · ${
                report.cpuPublishedSpacing ? `1:${report.cpuPublishedSpacing}` : "waiting"
              }`
            : "—"}
        />
        <Metric
          label="GPU level"
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
        <Metric label="GPU resident" value={memory} />
        <Metric
          label="CPU / GPU queue"
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
              ? `CPU 1:${report.cpuPublishedSpacing || "—"} · GPU 1:${
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
        <div><dt>GPU evaluator</dt><dd>{shortRevision(report?.gpuEvaluatorRevision)}</dd></div>
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
  return `${formatInteger(blocks)} blocks`;
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
