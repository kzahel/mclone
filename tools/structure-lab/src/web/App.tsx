import { useEffect } from "react";
import type { JSX } from "react";
import type { StructureCatalogEntry } from "../catalog-model";
import type { CameraPreset } from "../viewport";
import { StructureViewer } from "./StructureViewer";
import {
  assetUrl,
  selectedStructure,
  type StatusFilter,
  type StructureRotation,
  useStructureLabStore,
} from "./store";

const cameraOptions: Array<{ label: string; value: CameraPreset }> = [
  { label: "3/4", value: "three-quarter" },
  { label: "Front", value: "front" },
  { label: "Side", value: "side" },
  { label: "Top", value: "top" },
];

export function App(): JSX.Element {
  const state = useStructureLabStore();

  useEffect(() => { void state.loadCatalog(); }, [state.loadCatalog]);
  useEffect(() => {
    window.addEventListener("popstate", state.restoreUrlSelection);
    return () => window.removeEventListener("popstate", state.restoreUrlSelection);
  }, [state.restoreUrlSelection]);
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const update = (dark: boolean): void => state.syncSystemTheme(dark ? "dark" : "light");
    update(media.matches);
    const change = (event: MediaQueryListEvent): void => update(event.matches);
    media.addEventListener("change", change);
    return () => media.removeEventListener("change", change);
  }, [state.syncSystemTheme]);

  const entry = selectedStructure(state.catalog, state.selectedStructureId);
  const families = [...new Map(
    (state.catalog?.structures ?? []).flatMap((structure) =>
      structure.family ? [[structure.family.id, structure.family.label] as const] : []
    ),
  )];
  const filtered = filterStructures(
    state.catalog?.structures ?? [],
    state.search,
    state.familyFilter,
    state.statusFilter,
  );

  return (
    <div className="appShell" data-theme={state.themeMode}>
      <header className="topBar">
        <a className="brandBlock" href="/structures/" aria-label="Structure Lab home">
          <span className="brandMark" aria-hidden="true"><i /><i /><i /></span>
          <span><span className="eyebrow">Mclone workshop</span><strong>Structure Lab</strong></span>
        </a>
        <div className="summaryStrip" aria-label="Catalogue summary">
          <Summary label="structures" value={state.catalog?.summary.structures ?? 0} />
          <Summary label="families" value={state.catalog?.summary.families ?? 0} />
          <Summary label="placed blocks" value={state.catalog?.summary.blocks ?? 0} />
        </div>
        <div className="topActions">
          <span className="readOnlyBadge">Read-only recipes</span>
          <a className="siteLink" href="/">Play Mclone</a>
          <button className="quietButton" type="button" onClick={state.toggleTheme}>
            {state.themeMode === "dark" ? "Light" : "Dark"}
          </button>
        </div>
      </header>

      {state.error ? <div className="errorBanner" role="alert">{state.error}</div> : null}

      <main className="workbench">
        <aside className="catalogSidebar" aria-label="Structure catalogue">
          <div className="sidebarIntro">
            <span className="eyebrow">The pattern shelf</span>
            <h1>Buildings with a story</h1>
            <p>Original block-built places, ready to inspect from footing to ridge.</p>
          </div>
          <div className="catalogControls">
            <label>
              <span>Find a structure</span>
              <input
                type="search"
                value={state.search}
                onChange={(event) => state.setSearch(event.target.value)}
                placeholder="cottage, porch, farm…"
              />
            </label>
            <label>
              <span>Family</span>
              <select value={state.familyFilter} onChange={(event) => state.setFamilyFilter(event.target.value)}>
                <option value="all">All families</option>
                {families.map(([id, label]) => <option key={id} value={id}>{label}</option>)}
              </select>
            </label>
            <label>
              <span>Status</span>
              <select
                aria-label="Runtime status"
                value={state.statusFilter}
                onChange={(event) => state.setStatusFilter(event.target.value as StatusFilter)}
              >
                <option value="all">All recipes</option>
                <option value="promoted">Runtime</option>
                <option value="parity-canary">Parity canaries</option>
                <option value="lab-only">Lab only</option>
              </select>
            </label>
          </div>
          <div className="resultCount" aria-live="polite">
            {filtered.length} pattern{filtered.length === 1 ? "" : "s"}
          </div>
          <div className="catalogList">
            {filtered.map((structure) => (
              <button
                type="button"
                key={structure.structureId}
                className={structure.structureId === state.selectedStructureId ? "catalogCard selected" : "catalogCard"}
                data-structure-id={structure.structureId}
                aria-current={structure.structureId === state.selectedStructureId ? "true" : undefined}
                onClick={() => state.selectStructure(structure.structureId)}
              >
                <span className="thumbnailFrame">
                  <img src={assetUrl(structure.thumbnailPath)} alt="" loading="lazy" />
                  <span className="sizeBadge">{structure.size.join(" × ")}</span>
                </span>
                <span className="cardCopy">
                  <span className="cardTitle"><strong>{structure.label}</strong><StatusBadge entry={structure} /></span>
                  <span>{structure.family?.label ?? "Standalone"} · {structure.placedBlockCount} blocks</span>
                  <span className="tagRow">{structure.tags.slice(0, 3).map((tag) => <i key={tag}>{friendly(tag)}</i>)}</span>
                </span>
              </button>
            ))}
            {state.loadStatus === "loading" ? <div className="listMessage">Opening the pattern drawer…</div> : null}
            {state.loadStatus === "ready" && filtered.length === 0 ? <div className="listMessage">No patterns match.</div> : null}
          </div>
        </aside>

        <section className="viewerColumn" aria-label="Interactive structure viewer">
          {entry ? (
            <div className="viewerPanel">
              <div className="viewerHeader">
                <div>
                  <span className="eyebrow">{entry.family?.label ?? "Original structure"}</span>
                  <h2>{entry.label}</h2>
                </div>
                <div className="cameraActions" aria-label="Camera views">
                  {cameraOptions.map((option) => (
                    <button
                      type="button"
                      key={option.value}
                      className={state.cameraPreset === option.value ? "active" : ""}
                      aria-pressed={state.cameraPreset === option.value}
                      onClick={() => state.setCameraPreset(option.value)}
                    >{option.label}</button>
                  ))}
                </div>
              </div>
              <StructureViewer
                entry={entry}
                boundsVisible={state.boundsVisible}
                cameraPreset={state.cameraPreset}
                hiddenComponents={state.hiddenComponents}
                layer={state.layer}
                markersVisible={state.markersVisible}
                mirror={state.mirror}
                rotation={state.rotation}
                themeMode={state.themeMode}
              />
              <GuideControls entry={entry} />
            </div>
          ) : <div className="emptyViewer">Preparing the workshop…</div>}
        </section>

        <aside className="inspector" aria-label="Structure recipe">
          {entry ? <Inspector entry={entry} /> : <div className="inspectorNote">Choose a pattern to open its recipe.</div>}
        </aside>
      </main>
    </div>
  );
}

function GuideControls({ entry }: { entry: StructureCatalogEntry }): JSX.Element {
  const state = useStructureLabStore();
  return (
    <div className="guideControls">
      <div className="layerControl">
        <label htmlFor="build-layer"><span>Build layer</span><strong>{state.layer + 1} / {entry.size[1]}</strong></label>
        <input
          id="build-layer"
          aria-label="Build layer"
          type="range"
          min="0"
          max={entry.size[1] - 1}
          value={state.layer}
          onChange={(event) => state.setLayer(Number(event.target.value))}
        />
        <div className="layerTicks" aria-hidden="true"><span>Footings</span><span>Roofline</span></div>
      </div>
      <div className="viewToggles">
        <label><span>Turn</span>
          <select value={state.rotation} onChange={(event) => state.setRotation(Number(event.target.value) as StructureRotation)}>
            <option value={0}>South · 0°</option><option value={90}>West · 90°</option>
            <option value={180}>North · 180°</option><option value={270}>East · 270°</option>
          </select>
        </label>
        <Toggle label="Mirror" pressed={state.mirror} onClick={() => state.setMirror(!state.mirror)} />
        <Toggle label="Markers" pressed={state.markersVisible} onClick={() => state.setMarkersVisible(!state.markersVisible)} />
        <Toggle label="Bounds" pressed={state.boundsVisible} onClick={() => state.setBoundsVisible(!state.boundsVisible)} />
      </div>
      <div className="componentStrip" aria-label="Structure components">
        <span>Parts</span>
        {entry.components.map((component) => {
          const enabled = !state.hiddenComponents.includes(component.id);
          return (
            <button type="button" key={component.id} aria-pressed={enabled} className={enabled ? "active" : ""} onClick={() => state.toggleComponent(component.id)}>
              <i aria-hidden="true" />{component.label}
            </button>
          );
        })}
      </div>
    </div>
  );
}

function Inspector({ entry }: { entry: StructureCatalogEntry }): JSX.Element {
  const maxMaterial = Math.max(...entry.materialBill.map((material) => material.count), 1);
  return (
    <div className="inspectorStack">
      <section className="recipeHeading">
        <span className="eyebrow">Pattern card</span>
        <h2>{entry.label}</h2>
        <p>{entry.description}</p>
        <StatusBadge entry={entry} long />
      </section>
      <section className="factGrid">
        <Fact label="Width" value={`${entry.size[0]} b`} />
        <Fact label="Height" value={`${entry.size[1]} b`} />
        <Fact label="Depth" value={`${entry.size[2]} b`} />
        <Fact label="Blocks" value={entry.placedBlockCount.toLocaleString()} />
      </section>
      <section className="inspectorSection materialSection">
        <div className="sectionTitle"><h3>Material basket</h3><span>{entry.materialBill.length} kinds</span></div>
        <ol className="materialList">
          {[...entry.materialBill].sort((a, b) => b.count - a.count).map((material) => (
            <li key={material.paletteKey}>
              <span className={`materialSwatch swatch-${material.paletteKey}`} />
              <span><strong>{friendly(material.paletteKey)}</strong><i style={{ width: `${material.count / maxMaterial * 100}%` }} /></span>
              <b>{material.count}</b>
            </li>
          ))}
        </ol>
      </section>
      <section className="inspectorSection">
        <div className="sectionTitle"><h3>Entrances & joins</h3><span>{entry.markers.length + entry.sockets.length}</span></div>
        <dl className="detailList">
          {entry.markers.map((marker) => <div key={marker.kind}><dt>{friendly(marker.kind)}</dt><dd>{marker.pos.join(", ")}</dd></div>)}
          {entry.sockets.map((socket) => <div key={socket.id}><dt>{friendly(socket.kind)}</dt><dd>{socket.facing}</dd></div>)}
        </dl>
      </section>
      <section className="inspectorSection provenanceCard">
        <span className="eyebrow">Source-first proof</span>
        <h3>TypeScript → JSON → Rust → GLB</h3>
        <p>The editable recipe is compiled, reparsed, matched to the Rust canary, and meshed with first-party engine visuals.</p>
        <dl className="detailList"><div><dt>Mesh</dt><dd>{formatBytes(entry.geometry.vertexCount * 36 + entry.geometry.indexCount * 4)}</dd></div><div><dt>Faces</dt><dd>{entry.geometry.faceCount.toLocaleString()}</dd></div><div><dt>Atlas</dt><dd>{entry.artifacts.atlasSpriteCount} sprites</dd></div></dl>
        <code title={entry.source.semanticSha256}>{entry.source.semanticSha256.slice(0, 16)}…</code>
      </section>
    </div>
  );
}

function Toggle({ label, pressed, onClick }: { label: string; pressed: boolean; onClick: () => void }): JSX.Element {
  return <button type="button" className={pressed ? "toggle active" : "toggle"} aria-pressed={pressed} onClick={onClick}>{label}</button>;
}

function StatusBadge({ entry, long = false }: { entry: StructureCatalogEntry; long?: boolean }): JSX.Element {
  const label = entry.runtimeStatus === "promoted" ? "Runtime" : entry.runtimeStatus === "parity-canary" ? (long ? "Runtime parity canary" : "Canary") : "Lab only";
  return <span className={`statusBadge status-${entry.runtimeStatus}`}>{label}</span>;
}

function Summary({ label, value }: { label: string; value: number }): JSX.Element {
  return <div className="summaryItem"><strong>{value.toLocaleString()}</strong><span>{label}</span></div>;
}

function Fact({ label, value }: { label: string; value: string | number }): JSX.Element {
  return <div className="fact"><span>{label}</span><strong>{value}</strong></div>;
}

function filterStructures(
  structures: readonly StructureCatalogEntry[],
  search: string,
  family: string,
  status: StatusFilter,
): StructureCatalogEntry[] {
  const query = search.trim().toLowerCase();
  return structures.filter((entry) => {
    const haystack = [entry.label, entry.description, entry.category, ...entry.tags, entry.family?.label ?? ""].join(" ").toLowerCase();
    return (query === "" || haystack.includes(query))
      && (family === "all" || entry.family?.id === family)
      && (status === "all" || entry.runtimeStatus === status);
  });
}

function friendly(value: string): string {
  return value.replaceAll(/[:_-]+/gu, " ").replaceAll(/\b\w/gu, (letter) => letter.toUpperCase());
}

function formatBytes(bytes: number): string {
  return bytes < 1_000_000 ? `${Math.round(bytes / 1_000)} KB` : `${(bytes / 1_000_000).toFixed(1)} MB`;
}
