import { useEffect } from "react";
import type { JSX } from "react";
import type { AnimalCatalogFigure } from "../catalog-model";
import { FigureViewer } from "./FigureViewer";
import {
  assetUrl,
  type BodyPlanFilter,
  type DispositionFilter,
  type GroupFilter,
  type HabitatFilter,
  type MotionFilter,
  type PromotionFilter,
  useAnimalCatalogueStore,
} from "./store";

const motionOptions: Array<{ label: string; value: MotionFilter }> = [
  { label: "All motion", value: "all" },
  { label: "Quadrupeds", value: "quadruped-walk" },
  { label: "Bipeds", value: "biped-walk" },
  { label: "Flying", value: "wing-flap" },
  { label: "Swimming", value: "swim" },
  { label: "Slithering", value: "slither" },
  { label: "Special actions", value: "action" },
  { label: "Other rigs", value: "other" },
];

const promotionOptions: Array<{ label: string; value: PromotionFilter }> = [
  { label: "All figures", value: "all" },
  { label: "Runtime only", value: "runtime" },
  { label: "Lab only", value: "asset-lab" },
];

const groupOptions: Array<{ label: string; value: GroupFilter }> = [
  { label: "All groups", value: "all" },
  { label: "Animals", value: "animal" },
  { label: "Fantasy", value: "fantasy" },
  { label: "Monsters", value: "monster" },
  { label: "Humanoids", value: "humanoid" },
  { label: "Constructs", value: "construct" },
];

const bodyPlanOptions: Array<{ label: string; value: BodyPlanFilter }> = [
  { label: "All body plans", value: "all" },
  { label: "Bipeds", value: "biped" },
  { label: "Quadrupeds", value: "quadruped" },
  { label: "Winged", value: "winged" },
  { label: "Swimmers", value: "swimmer" },
  { label: "Serpentine", value: "serpentine" },
  { label: "Crawlers", value: "crawler" },
  { label: "Blobs", value: "blob" },
  { label: "Other", value: "other" },
];

const habitatOptions: Array<{ label: string; value: HabitatFilter }> = [
  { label: "All habitats", value: "all" },
  { label: "Land", value: "land" },
  { label: "Water", value: "water" },
  { label: "Air", value: "air" },
  { label: "Underground", value: "underground" },
];

const dispositionOptions: Array<{ label: string; value: DispositionFilter }> = [
  { label: "All dispositions", value: "all" },
  { label: "Passive", value: "passive" },
  { label: "Neutral", value: "neutral" },
  { label: "Hostile", value: "hostile" },
  { label: "Boss", value: "boss" },
];

export function App(): JSX.Element {
  const bodyPlanFilter = useAnimalCatalogueStore((state) => state.bodyPlanFilter);
  const catalog = useAnimalCatalogueStore((state) => state.catalog);
  const dispositionFilter = useAnimalCatalogueStore((state) => state.dispositionFilter);
  const error = useAnimalCatalogueStore((state) => state.error);
  const groupFilter = useAnimalCatalogueStore((state) => state.groupFilter);
  const habitatFilter = useAnimalCatalogueStore((state) => state.habitatFilter);
  const loadCatalog = useAnimalCatalogueStore((state) => state.loadCatalog);
  const loadStatus = useAnimalCatalogueStore((state) => state.loadStatus);
  const motionFilter = useAnimalCatalogueStore((state) => state.motionFilter);
  const promotionFilter = useAnimalCatalogueStore((state) => state.promotionFilter);
  const restoreUrlSelection = useAnimalCatalogueStore((state) => state.restoreUrlSelection);
  const search = useAnimalCatalogueStore((state) => state.search);
  const selectClip = useAnimalCatalogueStore((state) => state.selectClip);
  const selectFigure = useAnimalCatalogueStore((state) => state.selectFigure);
  const selectedClipName = useAnimalCatalogueStore((state) => state.selectedClipName);
  const selectedFigureName = useAnimalCatalogueStore((state) => state.selectedFigureName);
  const setBodyPlanFilter = useAnimalCatalogueStore((state) => state.setBodyPlanFilter);
  const setDispositionFilter = useAnimalCatalogueStore((state) => state.setDispositionFilter);
  const setGroupFilter = useAnimalCatalogueStore((state) => state.setGroupFilter);
  const setHabitatFilter = useAnimalCatalogueStore((state) => state.setHabitatFilter);
  const setMotionFilter = useAnimalCatalogueStore((state) => state.setMotionFilter);
  const setPromotionFilter = useAnimalCatalogueStore((state) => state.setPromotionFilter);
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

  const figures = filterFigures(
    catalog?.figures ?? [],
    search,
    groupFilter,
    bodyPlanFilter,
    habitatFilter,
    dispositionFilter,
    motionFilter,
    promotionFilter,
  );
  const selectedFigure = catalog?.figures.find((figure) => figure.name === selectedFigureName);
  const activeClipName = selectedFigure?.clips.some((clip) => clip.name === selectedClipName)
    ? selectedClipName as string
    : selectedFigure?.defaultClip;

  return (
    <div className="appShell" data-theme={themeMode}>
      <header className="topBar">
        <div className="brandBlock">
          <div className="eyebrow">Mclone Asset Lab</div>
          <h1>Creature Catalogue</h1>
        </div>
        <div className="summaryStrip" aria-label="Catalogue summary">
          <SummaryItem label="figures" value={catalog?.summary.canonicalFigures ?? 0} />
          <SummaryItem label="clips" value={catalog?.summary.clips ?? 0} />
          <SummaryItem label="parts" value={catalog?.summary.parts ?? 0} />
          <SummaryItem label="runtime" value={catalog?.summary.runtimePromotedFigures ?? 0} />
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
        <aside className="catalogSidebar" aria-label="Creature catalogue">
          <div className="catalogControls">
            <label>
              <span>Search</span>
              <input
                type="search"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder="creature, theme, clip"
              />
            </label>
            <label>
              <span>Group</span>
              <select
                value={groupFilter}
                onChange={(event) => setGroupFilter(event.target.value as GroupFilter)}
              >
                {groupOptions.map((option) => (
                  <option key={option.value} value={option.value}>{option.label}</option>
                ))}
              </select>
            </label>
            <label>
              <span>Body plan</span>
              <select
                value={bodyPlanFilter}
                onChange={(event) => setBodyPlanFilter(event.target.value as BodyPlanFilter)}
              >
                {bodyPlanOptions.map((option) => (
                  <option key={option.value} value={option.value}>{option.label}</option>
                ))}
              </select>
            </label>
            <label>
              <span>Habitat</span>
              <select
                value={habitatFilter}
                onChange={(event) => setHabitatFilter(event.target.value as HabitatFilter)}
              >
                {habitatOptions.map((option) => (
                  <option key={option.value} value={option.value}>{option.label}</option>
                ))}
              </select>
            </label>
            <label>
              <span>Disposition</span>
              <select
                value={dispositionFilter}
                onChange={(event) => setDispositionFilter(event.target.value as DispositionFilter)}
              >
                {dispositionOptions.map((option) => (
                  <option key={option.value} value={option.value}>{option.label}</option>
                ))}
              </select>
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
            <label>
              <span>Runtime status</span>
              <select
                value={promotionFilter}
                onChange={(event) => setPromotionFilter(event.target.value as PromotionFilter)}
              >
                {promotionOptions.map((option) => (
                  <option key={option.value} value={option.value}>{option.label}</option>
                ))}
              </select>
            </label>
            <div className="resultCount" aria-live="polite">
              {figures.length} figure{figures.length === 1 ? "" : "s"}
            </div>
          </div>
          <div className="catalogList" aria-label="Canonical creatures">
            {figures.map((figure) => (
              <button
                type="button"
                key={figure.name}
                className={figure.name === selectedFigureName ? "catalogRow selected" : "catalogRow"}
                data-catalog-name={figure.name}
                data-runtime-promoted={figure.runtimePromotion === undefined ? "false" : "true"}
                data-groups={figure.metadata.groups.join(" ")}
                data-body-plans={figure.metadata.bodyPlans.join(" ")}
                data-habitats={figure.metadata.habitats.join(" ")}
                data-disposition={figure.metadata.disposition}
                aria-current={figure.name === selectedFigureName ? "true" : undefined}
                onClick={() => selectFigure(figure.name)}
              >
                <img src={assetUrl(figure.thumbnailPath)} alt="" loading="lazy" />
                <span className="catalogRowText">
                  <span className="catalogRowHeading">
                    <strong>{figure.label}</strong>
                    {figure.runtimePromotion ? <span className="promotionBadge">Runtime</span> : null}
                  </span>
                  <span>{formatTag(figure.metadata.groups[0] ?? "other")} · {motionLabel(figure)} · {figure.partCount} parts</span>
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
      <section className="inspectorSection classificationSection">
        <h3>Classification</h3>
        <MetadataTags label="Groups" values={figure.metadata.groups} />
        <MetadataTags label="Body" values={figure.metadata.bodyPlans} />
        <MetadataTags label="Habitat" values={figure.metadata.habitats} />
        <MetadataTags label="Disposition" values={[figure.metadata.disposition]} />
        <MetadataTags label="Scale" values={[figure.metadata.scale]} />
        {figure.metadata.themes && figure.metadata.themes.length > 0
          ? <MetadataTags label="Themes" values={figure.metadata.themes} />
          : null}
      </section>
      <section className="inspectorSection renderingSection">
        <h3>Rendering</h3>
        <MetadataTags label="Alpha" values={figure.alphaModes} />
      </section>
      <section className="inspectorSection">
        <h3>Active animation</h3>
        <dl>
          <div><dt>Clip</dt><dd>{clip?.name ?? "—"}</dd></div>
          <div><dt>Label</dt><dd>{clip?.label ?? "—"}</dd></div>
          <div><dt>Role</dt><dd>{clip?.role ?? "—"}</dd></div>
          <div><dt>Duration</dt><dd>{clip ? `${clip.durationSeconds.toFixed(2)}s` : "—"}</dd></div>
          <div><dt>Authored FPS</dt><dd>{clip?.fps ?? "—"}</dd></div>
          <div><dt>Loop</dt><dd>{clip?.loop ? "yes" : "no"}</dd></div>
          <div><dt>Motion</dt><dd>{clip?.locomotionKind ?? "custom"}</dd></div>
          <div><dt>After</dt><dd>{clip?.nextClip ?? (clip?.loop ? "repeat" : "hold final pose")}</dd></div>
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
      {figure.runtimePromotion ? (
        <section className="inspectorSection callout promotionStatus runtimePromoted">
          <h3>Runtime promoted</h3>
          <p>
            This checked semantic figure ships in the game asset pack and is
            available through the shared Rust actor-figure registry.
          </p>
          <dl className="promotionMetadata">
            <div><dt>Figure ID</dt><dd><code>{figure.runtimePromotion.figureId}</code></dd></div>
            <div><dt>Packed JSON</dt><dd><code>{figure.runtimePromotion.jsonPath}</code></dd></div>
          </dl>
        </section>
      ) : (
        <section className="inspectorSection callout promotionStatus assetLabOnly">
          <h3>Asset Lab only</h3>
          <p>
            This canonical example is available for review here but is not yet
            included in the game's runtime actor-figure registry.
          </p>
        </section>
      )}
    </div>
  );
}

function SummaryItem({ label, value }: { label: string; value: number }): JSX.Element {
  return <div className="summaryItem"><strong>{value}</strong><span>{label}</span></div>;
}

function Fact({ label, value }: { label: string; value: number }): JSX.Element {
  return <div className="fact"><strong>{value}</strong><span>{label}</span></div>;
}

function MetadataTags({ label, values }: { label: string; values: readonly string[] }): JSX.Element {
  return (
    <div className="metadataRow">
      <span>{label}</span>
      <div className="metadataTags">
        {values.map((value) => <span className="metadataTag" key={value}>{formatTag(value)}</span>)}
      </div>
    </div>
  );
}

function filterFigures(
  figures: readonly AnimalCatalogFigure[],
  search: string,
  groupFilter: GroupFilter,
  bodyPlanFilter: BodyPlanFilter,
  habitatFilter: HabitatFilter,
  dispositionFilter: DispositionFilter,
  motionFilter: MotionFilter,
  promotionFilter: PromotionFilter,
): AnimalCatalogFigure[] {
  const query = search.trim().toLocaleLowerCase();
  return figures.filter((figure) => {
    const motionKinds = new Set(figure.clips.flatMap((clip) =>
      clip.locomotionKind === undefined ? [] : [clip.locomotionKind]
    ));
    const hasAction = figure.clips.some((clip) => clip.role === "action");
    const matchesGroup = groupFilter === "all" || figure.metadata.groups.includes(groupFilter);
    const matchesBodyPlan = bodyPlanFilter === "all" || figure.metadata.bodyPlans.includes(bodyPlanFilter);
    const matchesHabitat = habitatFilter === "all" || figure.metadata.habitats.includes(habitatFilter);
    const matchesDisposition = dispositionFilter === "all"
      || figure.metadata.disposition === dispositionFilter;
    const matchesMotion = motionFilter === "all"
      || (motionFilter === "action"
        ? hasAction
        : motionFilter === "other"
          ? motionKinds.size === 0
          : motionKinds.has(motionFilter));
    const matchesPromotion = promotionFilter === "all"
      || (promotionFilter === "runtime"
        ? figure.runtimePromotion !== undefined
        : figure.runtimePromotion === undefined);
    if (
      !matchesGroup
      || !matchesBodyPlan
      || !matchesHabitat
      || !matchesDisposition
      || !matchesMotion
      || !matchesPromotion
    ) {
      return false;
    }
    if (!query) {
      return true;
    }
    const haystack = [
      figure.label,
      figure.name,
      ...figure.clips.map((clip) => clip.name),
      ...figure.clips.map((clip) => clip.label),
      ...figure.clips.map((clip) => clip.role),
      ...figure.clips.flatMap((clip) => clip.nextClip === undefined ? [] : [clip.nextClip]),
      ...motionKinds,
      ...figure.metadata.groups,
      ...figure.metadata.bodyPlans,
      ...figure.metadata.habitats,
      ...figure.alphaModes,
      figure.metadata.disposition,
      figure.metadata.scale,
      ...(figure.metadata.themes ?? []),
      ...(figure.runtimePromotion
        ? ["runtime", "promoted", figure.runtimePromotion.figureId, figure.runtimePromotion.jsonPath]
        : ["asset lab only"]),
    ].join(" ").toLocaleLowerCase();
    return haystack.includes(query);
  });
}

function formatTag(value: string): string {
  return value.replaceAll("-", " ").replace(/^./u, (letter) => letter.toUpperCase());
}

function motionLabel(figure: AnimalCatalogFigure): string {
  const motion = figure.clips.find((clip) => clip.locomotionKind)?.locomotionKind;
  if (!motion) {
    return "custom motion";
  }
  return motion.replaceAll("-", " ");
}
