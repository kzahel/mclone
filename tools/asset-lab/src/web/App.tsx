import { useEffect } from "react";
import type { JSX } from "react";
import type { AnimalCatalogFigure } from "../catalog-model";
import { FigureViewer } from "./FigureViewer";
import {
  assetUrl,
  type MotionFilter,
  useAnimalCatalogueStore,
} from "./store";

const motionOptions: Array<{ label: string; value: MotionFilter }> = [
  { label: "All motion", value: "all" },
  { label: "Quadrupeds", value: "quadruped-walk" },
  { label: "Bipeds", value: "biped-walk" },
  { label: "Flying", value: "wing-flap" },
  { label: "Swimming", value: "swim" },
  { label: "Slithering", value: "slither" },
  { label: "Other rigs", value: "other" },
];

export function App(): JSX.Element {
  const catalog = useAnimalCatalogueStore((state) => state.catalog);
  const error = useAnimalCatalogueStore((state) => state.error);
  const loadCatalog = useAnimalCatalogueStore((state) => state.loadCatalog);
  const loadStatus = useAnimalCatalogueStore((state) => state.loadStatus);
  const motionFilter = useAnimalCatalogueStore((state) => state.motionFilter);
  const restoreUrlSelection = useAnimalCatalogueStore((state) => state.restoreUrlSelection);
  const search = useAnimalCatalogueStore((state) => state.search);
  const selectClip = useAnimalCatalogueStore((state) => state.selectClip);
  const selectFigure = useAnimalCatalogueStore((state) => state.selectFigure);
  const selectedClipName = useAnimalCatalogueStore((state) => state.selectedClipName);
  const selectedFigureName = useAnimalCatalogueStore((state) => state.selectedFigureName);
  const setMotionFilter = useAnimalCatalogueStore((state) => state.setMotionFilter);
  const setSearch = useAnimalCatalogueStore((state) => state.setSearch);
  const syncSystemTheme = useAnimalCatalogueStore((state) => state.syncSystemTheme);
  const themeMode = useAnimalCatalogueStore((state) => state.themeMode);
  const toggleTheme = useAnimalCatalogueStore((state) => state.toggleTheme);

  useEffect(() => {
    void loadCatalog();
  }, [loadCatalog]);

  useEffect(() => {
    window.addEventListener("popstate", restoreUrlSelection);
    return () => window.removeEventListener("popstate", restoreUrlSelection);
  }, [restoreUrlSelection]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const update = (matches: boolean): void => syncSystemTheme(matches ? "dark" : "light");
    update(media.matches);
    const onChange = (event: MediaQueryListEvent): void => update(event.matches);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [syncSystemTheme]);

  useEffect(() => {
    if (!selectedFigureName || loadStatus !== "ready") {
      return;
    }
    const frame = requestAnimationFrame(() => {
      document
        .querySelector<HTMLElement>(`[data-catalog-name='${selectedFigureName}']`)
        ?.scrollIntoView({ block: "nearest" });
    });
    return () => cancelAnimationFrame(frame);
  }, [loadStatus, selectedFigureName]);

  const figures = filterFigures(catalog?.figures ?? [], search, motionFilter);
  const selectedFigure = catalog?.figures.find((figure) => figure.name === selectedFigureName);
  const activeClipName = selectedFigure?.clips.some((clip) => clip.name === selectedClipName)
    ? selectedClipName as string
    : selectedFigure?.defaultClip;

  return (
    <div className="appShell" data-theme={themeMode}>
      <header className="topBar">
        <div className="brandBlock">
          <div className="eyebrow">Mclone Asset Lab</div>
          <h1>Animal Catalogue</h1>
        </div>
        <div className="summaryStrip" aria-label="Catalogue summary">
          <SummaryItem label="figures" value={catalog?.summary.canonicalFigures ?? 0} />
          <SummaryItem label="clips" value={catalog?.summary.clips ?? 0} />
          <SummaryItem label="parts" value={catalog?.summary.parts ?? 0} />
        </div>
        <div className="topActions">
          <a className="siteLink" href="/">Play Mclone</a>
          <button type="button" className="themeButton" onClick={toggleTheme}>
            {themeMode === "dark" ? "Light" : "Dark"}
          </button>
        </div>
      </header>

      {error ? <div className="errorBanner" role="alert">{error}</div> : null}

      <main className="workbench">
        <aside className="catalogSidebar" aria-label="Figure catalogue">
          <div className="catalogControls">
            <label>
              <span>Search</span>
              <input
                type="search"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder="animal, clip, motion"
              />
            </label>
            <label>
              <span>Motion</span>
              <select
                value={motionFilter}
                onChange={(event) => setMotionFilter(event.target.value as MotionFilter)}
              >
                {motionOptions.map((option) => (
                  <option key={option.value} value={option.value}>{option.label}</option>
                ))}
              </select>
            </label>
            <div className="resultCount" aria-live="polite">
              {figures.length} figure{figures.length === 1 ? "" : "s"}
            </div>
          </div>
          <div className="catalogList" aria-label="Canonical figures">
            {figures.map((figure) => (
              <button
                type="button"
                key={figure.name}
                className={figure.name === selectedFigureName ? "catalogRow selected" : "catalogRow"}
                data-catalog-name={figure.name}
                aria-current={figure.name === selectedFigureName ? "true" : undefined}
                onClick={() => selectFigure(figure.name)}
              >
                <img src={assetUrl(figure.thumbnailPath)} alt="" loading="lazy" />
                <span className="catalogRowText">
                  <strong>{figure.label}</strong>
                  <span>{motionLabel(figure)} · {figure.partCount} parts</span>
                </span>
              </button>
            ))}
            {loadStatus === "loading" ? <div className="listMessage">Loading catalogue…</div> : null}
            {loadStatus === "ready" && figures.length === 0 ? (
              <div className="listMessage">No figures match these filters.</div>
            ) : null}
          </div>
        </aside>

        <div className="viewerColumn">
          {selectedFigure && activeClipName ? (
            <FigureViewer
              key={selectedFigure.name}
              clipName={activeClipName}
              figure={selectedFigure}
              onSelectClip={selectClip}
              themeMode={themeMode}
            />
          ) : (
            <div className="emptyViewer">
              {loadStatus === "error" ? "Catalogue unavailable." : "Preparing the catalogue…"}
            </div>
          )}
        </div>

        <aside className="inspector" aria-label="Figure details">
          {selectedFigure && activeClipName ? (
            <FigureInspector figure={selectedFigure} clipName={activeClipName} />
          ) : (
            <div className="inspectorNote">Select a figure to inspect its semantic facts.</div>
          )}
        </aside>
      </main>
    </div>
  );
}

function FigureInspector({
  figure,
  clipName,
}: {
  clipName: string;
  figure: AnimalCatalogFigure;
}): JSX.Element {
  const clip = figure.clips.find((entry) => entry.name === clipName);
  return (
    <div className="inspectorStack">
      <section>
        <div className="eyebrow">Figure details</div>
        <h2>{figure.label}</h2>
        <code>{figure.name}</code>
      </section>
      <section className="factGrid">
        <Fact label="Parts" value={figure.partCount} />
        <Fact label="Materials" value={figure.materialCount} />
        <Fact label="Textures" value={figure.textureCount} />
        <Fact label="Clips" value={figure.clipCount} />
      </section>
      <section className="inspectorSection">
        <h3>Active animation</h3>
        <dl>
          <div><dt>Clip</dt><dd>{clip?.name ?? "—"}</dd></div>
          <div><dt>Duration</dt><dd>{clip ? `${clip.durationSeconds.toFixed(2)}s` : "—"}</dd></div>
          <div><dt>Authored FPS</dt><dd>{clip?.fps ?? "—"}</dd></div>
          <div><dt>Loop</dt><dd>{clip?.loop ? "yes" : "no"}</dd></div>
          <div><dt>Motion</dt><dd>{clip?.locomotionKind ?? "custom"}</dd></div>
        </dl>
      </section>
      <section className="inspectorSection">
        <h3>Semantic source</h3>
        <p>
          Generated from the canonical box-only TypeScript source, serialized,
          reparsed, validated, and hash-checked before this viewer displays it.
        </p>
        <code className="hashLine">{figure.semanticSha256}</code>
      </section>
      <section className="inspectorSection callout">
        <h3>Asset Lab example</h3>
        <p>
          Catalogue inclusion is not runtime promotion. The game independently
          consumes promoted semantic JSON through the shared Rust figure path.
        </p>
      </section>
    </div>
  );
}

function SummaryItem({ label, value }: { label: string; value: number }): JSX.Element {
  return <div className="summaryItem"><strong>{value}</strong><span>{label}</span></div>;
}

function Fact({ label, value }: { label: string; value: number }): JSX.Element {
  return <div className="fact"><strong>{value}</strong><span>{label}</span></div>;
}

function filterFigures(
  figures: readonly AnimalCatalogFigure[],
  search: string,
  motionFilter: MotionFilter,
): AnimalCatalogFigure[] {
  const query = search.trim().toLocaleLowerCase();
  return figures.filter((figure) => {
    const motionKinds = new Set(figure.clips.flatMap((clip) =>
      clip.locomotionKind === undefined ? [] : [clip.locomotionKind]
    ));
    const matchesMotion = motionFilter === "all"
      || (motionFilter === "other" ? motionKinds.size === 0 : motionKinds.has(motionFilter));
    if (!matchesMotion) {
      return false;
    }
    if (!query) {
      return true;
    }
    const haystack = [
      figure.label,
      figure.name,
      ...figure.clips.map((clip) => clip.name),
      ...motionKinds,
    ].join(" ").toLocaleLowerCase();
    return haystack.includes(query);
  });
}

function motionLabel(figure: AnimalCatalogFigure): string {
  const motion = figure.clips.find((clip) => clip.locomotionKind)?.locomotionKind;
  if (!motion) {
    return "custom motion";
  }
  return motion.replaceAll("-", " ");
}
