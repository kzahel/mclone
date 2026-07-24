import { useCallback, useEffect, useMemo, useState } from "react";

import {
  DEFAULT_TERRAIN_LAB_CAMERA,
  REVIEW_TERRAIN_LAB_STATE,
  TERRAIN_LAB_SPACINGS,
  footprintBlocks,
  nextSpacing,
  panTerrainLabState,
  parseTerrainLabState,
  terrainLabSearch,
  validSeed,
  type TerrainLabLayer,
  type TerrainLabCamera,
  type TerrainLabSource,
  type TerrainLabState,
  type TerrainLabView,
} from "../state";
import {
  TerrainCanvas,
  type TerrainLabAdapterReport,
  type TerrainLabComparisonReport,
  type TerrainLabRenderReport,
} from "./TerrainCanvas";

type LabStatus = "loading" | "ready" | "rendering" | "error";

const SOURCE_OPTIONS: Array<{ value: TerrainLabSource; label: string; note: string }> = [
  { value: "reference", label: "CPU final", note: "Complete production CPU sampler" },
  { value: "split", label: "Compare", note: "Same coordinates: CPU base left, GPU base right" },
  { value: "gpu", label: "GPU base", note: "Production fields through base surface" },
];

const LAYER_OPTIONS: Array<{ value: TerrainLabLayer; label: string }> = [
  { value: "terrain", label: "Terrain" },
  { value: "height", label: "Height" },
  { value: "error", label: "Base error" },
  { value: "continentalness", label: "Continents" },
  { value: "climate", label: "Climate" },
];

export function App(): React.JSX.Element {
  const [state, setState] = useState<TerrainLabState>(() =>
    parseTerrainLabState(window.location.search)
  );
  const [status, setStatus] = useState<LabStatus>("loading");
  const [adapter, setAdapter] = useState<TerrainLabAdapterReport>();
  const [renderReport, setRenderReport] = useState<TerrainLabRenderReport>();
  const [comparison, setComparison] = useState<TerrainLabComparisonReport>();
  const [error, setError] = useState<string>();
  const [camera, setCamera] = useState<TerrainLabCamera>(DEFAULT_TERRAIN_LAB_CAMERA);

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
  const renderStatus = status === "rendering" ? "updating" : status;
  const compareLayout = state.source !== "split"
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
      data-continentalness-error={comparison?.meanAbsoluteContinentalnessError ?? ""}
      data-compare-layout={compareLayout}
      data-vertex-count={renderReport?.vertexCount ?? 0}
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
          <div className={`statusPill ${status}`} data-testid="lab-status">
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
            <SegmentedControl<TerrainLabSource>
              label="Preview source"
              value={state.source}
              options={SOURCE_OPTIONS}
              onChange={(source) => patchState({ source })}
            />
            <SourceGuide source={state.source} />
          </div>
          <TerrainCanvas
            state={state}
            camera={camera}
            onStateChange={updateState}
            onCameraChange={setCamera}
            onAdapter={setAdapter}
            onRender={setRenderReport}
            onComparison={setComparison}
            onError={setError}
            onStatus={setStatus}
          />
          <div className="viewerFooter">
            <div>
              <span className="footerLabel">footprint</span>
              <strong>{formatDistance(footprint)} × {formatDistance(footprint)}</strong>
            </div>
            <div>
              <span className="footerLabel">chunk equivalent</span>
              <strong>{formatInteger(chunkWidth)} × {formatInteger(chunkWidth)}</strong>
            </div>
            <div>
              <span className="footerLabel">sample spacing</span>
              <strong>{state.spacing} blocks</strong>
            </div>
            <div className="approximationNote">
              <SourceFootnote source={state.source} />
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

          <ControlSection number="02" title="Scale">
            <label className="fieldLabel">
              <span>Sample spacing</span>
              <select
                value={state.spacing}
                onChange={(event) =>
                  patchState({ spacing: Number(event.target.value) as TerrainLabState["spacing"] })
                }
              >
                {TERRAIN_LAB_SPACINGS.map((spacing) => (
                  <option key={spacing} value={spacing}>
                    {spacing} blocks · {formatDistance(spacing * 64)} view
                  </option>
                ))}
              </select>
            </label>
            <div className="zoomRow">
              <button
                type="button"
                className="zoomButton"
                onClick={() => patchState({ spacing: nextSpacing(state.spacing, "in") })}
                aria-label="Zoom in"
              >
                <span>+</span> closer
              </button>
              <div className="scaleReadout">
                <strong>{formatDistance(footprint)}</strong>
                <span>across</span>
              </div>
              <button
                type="button"
                className="zoomButton"
                onClick={() => patchState({ spacing: nextSpacing(state.spacing, "out") })}
                aria-label="Zoom out"
              >
                <span>−</span> farther
              </button>
            </div>
            <PanPad state={state} onChange={updateState} />
          </ControlSection>

          <ControlSection number="03" title="Presentation">
            <SegmentedControl<TerrainLabView>
              label="View"
              value={state.view}
              options={[
                { value: "3d", label: "3D terrain" },
                { value: "map", label: "Map" },
              ]}
              onChange={(view) => patchState({ view })}
            />
            <label className="fieldLabel">
              <span>Diagnostic layer</span>
              <select
                value={state.layer}
                onChange={(event) => patchState({ layer: event.target.value as TerrainLabLayer })}
              >
                {LAYER_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>{option.label}</option>
                ))}
              </select>
            </label>
            <button
              type="button"
              className="cameraResetButton"
              onClick={() => setCamera(DEFAULT_TERRAIN_LAB_CAMERA)}
            >
              Reset 3D camera
            </button>
          </ControlSection>

          <ControlSection number="04" title="Evidence" subdued>
            <Diagnostics
              adapter={adapter}
              report={renderReport}
              comparison={comparison}
            />
          </ControlSection>
        </aside>
      </main>
    </div>
  );
}

function SourceGuide({ source }: { source: TerrainLabSource }): React.JSX.Element {
  if (source === "split") {
    return (
      <div className="sourceGuide">
        <strong>Same coordinates, paired views</strong>
        <span>
          CPU production base is first; GPU production base is second. Both panels
          render the exact same world coordinates with one shared seed, center, scale,
          camera, and layer. Wide screens place them left and right; phones stack
          full-width panels to preserve detail. Orbit or pan once to move both
          together, then compare matching terrain directly.
        </span>
      </div>
    );
  }
  if (source === "gpu") {
    return (
      <div className="sourceGuide">
        <strong>GPU production base</strong>
        <span>
          Production large-scale fields through bathymetry. Final rivers, wetlands,
          and planned streams are not ported yet.
        </span>
      </div>
    );
  }
  return (
    <div className="sourceGuide">
      <strong>CPU final reference</strong>
      <span>
        Complete production CPU terrain, including final rivers, wetlands, and planned
        streams.
      </span>
    </div>
  );
}

function SourceFootnote({ source }: { source: TerrainLabSource }): React.JSX.Element {
  if (source === "split") {
    return (
      <>
        Compare uses CPU base versus GPU base. Final rivers, wetlands, and planned
        streams are excluded from both panels.
      </>
    );
  }
  if (source === "gpu") {
    return (
      <>
        GPU A2 ports production base fields. Final rivers, wetlands, and planned
        streams are not ported yet.
      </>
    );
  }
  return (
    <>
      CPU final includes rivers, wetlands, and planned streams. Base-field parity is
      reported separately in Evidence.
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

function PanPad({
  state,
  onChange,
}: {
  state: TerrainLabState;
  onChange: (state: TerrainLabState) => void;
}): React.JSX.Element {
  const distance = Math.max(state.spacing * 8, footprintBlocks(state) / 4);
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
}: {
  adapter: TerrainLabAdapterReport | undefined;
  report: TerrainLabRenderReport | undefined;
  comparison: TerrainLabComparisonReport | undefined;
}): React.JSX.Element {
  const memory = report ? formatBytes(report.residentBytes) : "—";
  const adapterLabel = useMemo(() => {
    if (!adapter) {
      return "requesting adapter";
    }
    return adapter.name || `${adapter.backend} adapter`;
  }, [adapter]);
  return (
    <div className="diagnostics" data-testid="terrain-diagnostics">
      <div className="metricGrid">
        <Metric label="CPU reference" value={formatMs(report?.cpuReferenceMs)} />
        <Metric label="Encode + submit" value={formatMs(report?.encodeSubmitMs)} />
        <Metric label="Base mean Δ" value={formatBlocks(comparison?.meanAbsoluteBaseSurfaceError)} />
        <Metric label="Base P95 Δ" value={formatBlocks(comparison?.p95AbsoluteBaseSurfaceError)} />
        <Metric label="Base maximum Δ" value={formatBlocks(comparison?.maxAbsoluteBaseSurfaceError)} />
        <Metric
          label="Ocean agreement"
          value={comparison ? `${(comparison.oceanWaterPresenceAgreement * 100).toFixed(1)}%` : "pending"}
        />
        <Metric label="Final mean Δ" value={formatBlocks(comparison?.meanAbsoluteSurfaceError)} />
        <Metric label="Final P95 Δ" value={formatBlocks(comparison?.p95AbsoluteSurfaceError)} />
        <Metric label="GPU resident" value={memory} />
        <Metric label="Vertices" value={report ? formatInteger(report.vertexCount) : "—"} />
      </div>
      <dl className="detailList">
        <div><dt>Adapter</dt><dd data-testid="adapter-name">{adapterLabel}</dd></div>
        <div><dt>Backend</dt><dd>{adapter?.backend ?? "—"}</dd></div>
        <div><dt>Samples</dt><dd>{report ? formatInteger(report.sampleCount) : "—"}</dd></div>
        <div><dt>Field</dt><dd>{shortRevision(report?.fieldRevision)}</dd></div>
        <div><dt>GPU evaluator</dt><dd>{shortRevision(report?.gpuEvaluatorRevision)}</dd></div>
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

function formatMs(value: number | undefined): string {
  return value === undefined ? "—" : `${value.toFixed(value >= 10 ? 1 : 2)} ms`;
}

function formatBlocks(value: number | undefined): string {
  return value === undefined ? "pending" : `${value.toFixed(1)} blocks`;
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
