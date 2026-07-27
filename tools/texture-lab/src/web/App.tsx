import { useEffect } from "react";
import type { JSX, ReactNode } from "react";
import { useShallow } from "zustand/react/shallow";
import type { BlockIndexEntry, TextureCandidateEntry, TextureImageRef, TextureIndexEntry } from "../core/index-model";
import { primaryCandidateImage } from "./candidate-images";
import { BlockBundleAtlas, MinecraftCoverageAtlas, PreviewModeTabs, TextureAtlas } from "./components/OverviewViews";
import {
  activeTextureCandidates,
  filteredTextures,
  materialOptions,
  selectError,
  selectCurationStatus,
  selectIndex,
  selectLoadStatus,
  selectLifecycleFilter,
  selectMaterialFilter,
  selectPreviewMode,
  selectPreviewSelectionsByTexture,
  selectSearch,
  selectedCandidate as selectedCandidateSelector,
  selectedTexture,
  selectSelectedCandidateId,
  selectSelectedTextureName,
  selectThemeMode,
  LIFECYCLE_FILTER_OPTIONS,
} from "./store/selectors";
import type { LifecycleFilter, ThemeMode } from "./store/textureLabStore";
import { imageRefUrl, useTextureLabStore } from "./store/textureLabStore";

type TextureLifecyclePromotionHandler = (
  textureName: string,
  candidateId: string | null,
  state: "provisional" | "curated",
) => Promise<void>;

const HOSTED_READ_ONLY = import.meta.env.PROD;

export function App(): JSX.Element {
  const index = useTextureLabStore(selectIndex);
  const loadStatus = useTextureLabStore(selectLoadStatus);
  const error = useTextureLabStore(selectError);
  const curationStatus = useTextureLabStore(selectCurationStatus);
  const textures = useTextureLabStore(useShallow(filteredTextures));
  const activeTexture = useTextureLabStore(selectedTexture);
  const activeCandidates = useTextureLabStore(useShallow(activeTextureCandidates));
  const selectedCandidate = useTextureLabStore(selectedCandidateSelector);
  const selectedTextureName = useTextureLabStore(selectSelectedTextureName);
  const selectedCandidateId = useTextureLabStore(selectSelectedCandidateId);
  const previewSelectionsByTexture = useTextureLabStore(selectPreviewSelectionsByTexture);
  const visibleSelectionsByTexture = previewSelectionsByTexture;
  const themeMode = useTextureLabStore(selectThemeMode);
  const previewMode = useTextureLabStore(selectPreviewMode);
  const search = useTextureLabStore(selectSearch);
  const materialFilter = useTextureLabStore(selectMaterialFilter);
  const lifecycleFilter = useTextureLabStore(selectLifecycleFilter);
  const materials = useTextureLabStore(useShallow(materialOptions));
  const loadIndex = useTextureLabStore((state) => state.loadIndex);
  const reindex = useTextureLabStore((state) => state.reindex);
  const selectTexture = useTextureLabStore((state) => state.selectTexture);
  const selectCandidate = useTextureLabStore((state) => state.selectCandidate);
  const setPreviewCandidate = useTextureLabStore((state) => state.setPreviewCandidate);
  const clearPreviewCandidate = useTextureLabStore((state) => state.clearPreviewCandidate);
  const promoteLifecycle = useTextureLabStore((state) => state.promoteLifecycle);
  const returnToCandidate = useTextureLabStore((state) => state.returnToCandidate);
  const setPreviewMode = useTextureLabStore((state) => state.setPreviewMode);
  const syncSystemTheme = useTextureLabStore((state) => state.syncSystemTheme);
  const toggleTheme = useTextureLabStore((state) => state.toggleTheme);
  const setSearch = useTextureLabStore((state) => state.setSearch);
  const setMaterialFilter = useTextureLabStore((state) => state.setMaterialFilter);
  const setLifecycleFilter = useTextureLabStore((state) => state.setLifecycleFilter);

  useEffect(() => {
    void loadIndex();
  }, [loadIndex]);

  useEffect(() => {
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const onSystemThemeChange = (event: MediaQueryListEvent): void => {
      syncSystemTheme(themeModeFromSystem(event.matches));
    };
    syncSystemTheme(themeModeFromSystem(mediaQuery.matches));
    mediaQuery.addEventListener("change", onSystemThemeChange);
    return () => mediaQuery.removeEventListener("change", onSystemThemeChange);
  }, [syncSystemTheme]);

  return (
    <div className="appShell" data-theme={themeMode}>
      <header className="topBar">
        <div>
          <div className="eyebrow">Texture Lab</div>
          <h1>{index?.pack.name ?? "mclone-default"}</h1>
        </div>
        <div className="summaryStrip" aria-label="Pack summary">
          <SummaryItem label="candidate" value={index?.summary.candidateLifecycleCount ?? 0} />
          <SummaryItem label="provisional" value={index?.summary.provisionalLifecycleCount ?? 0} />
          <SummaryItem label="curated" value={index?.summary.curatedLifecycleCount ?? 0} />
          <SummaryItem label="legacy LOD" value={index?.summary.legacyDerivedTextureCount ?? 0} />
        </div>
        <div className="toolbarActions">
          <a className="toolbarButton toolbarLink" href="/">Home</a>
          {HOSTED_READ_ONLY ? <span className="hostedBadge">Hosted · read-only</span> : null}
          <button
            className="toolbarButton"
            type="button"
            aria-label={`Switch to ${oppositeThemeMode(themeMode)} mode`}
            onClick={toggleTheme}
          >
            {themeMode === "dark" ? "Light" : "Dark"}
          </button>
          {!HOSTED_READ_ONLY ? (
            <button className="toolbarButton" type="button" onClick={() => void reindex()} disabled={loadStatus === "loading"}>
              {loadStatus === "loading" ? "Indexing" : "Reindex"}
            </button>
          ) : null}
        </div>
      </header>

      {error ? <div className="errorBanner">{error}</div> : null}
      {curationStatus ? <div className="statusBanner">{curationStatus}</div> : null}
      {index?.warnings.length ? (
        <div className="warningBanner">
          {index.warnings.map((warning) => (
            <span key={warning}>{warning}</span>
          ))}
        </div>
      ) : null}

      <main className="workbench">
        <aside className="sidebar">
          <div className="sidebarControls">
            <label>
              Search
              <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="texture, block, tag" />
            </label>
            <label>
              Material
              <select value={materialFilter} onChange={(event) => setMaterialFilter(event.target.value)}>
                <option value="all">All</option>
                {materials.map((material) => (
                  <option key={material} value={material}>
                    {material}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Lifecycle
              <select
                aria-label="Lifecycle filter"
                value={lifecycleFilter}
                onChange={(event) => setLifecycleFilter(event.target.value as LifecycleFilter)}
              >
                {LIFECYCLE_FILTER_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <div className="filterCount" aria-live="polite">
              {textures.length} texture{textures.length === 1 ? "" : "s"}
            </div>
          </div>
          <div className="textureList" aria-label="Textures">
            {textures.map((texture) => (
              <button
                key={texture.name}
                className={texture.name === selectedTextureName ? "textureRow selected" : "textureRow"}
                type="button"
                onClick={() => selectTexture(texture.name)}
              >
                <span className="textureName">{texture.name}</span>
                <span className="textureMeta">
                  {texture.lifecycle.state} / {texture.materialFamily} / {texture.size}px
                </span>
              </button>
            ))}
          </div>
        </aside>

        <section className="previewPane">
          {index ? (
            <>
              <div className="previewModeBar">
                <PreviewModeTabs mode={previewMode} onChange={setPreviewMode} />
              </div>
              {previewMode === "atlas" ? (
                <TextureAtlas
                  textures={textures}
                  candidates={index.candidates}
                  previewSelectionsByTexture={visibleSelectionsByTexture}
                  selectedTextureName={selectedTextureName}
                  onSelectTexture={selectTexture}
                />
              ) : previewMode === "mc" ? (
                <MinecraftCoverageAtlas
                  coverage={index.vanillaCoverage}
                  search={search}
                  selectedTextureName={selectedTextureName}
                  onSelectTexture={selectTexture}
                />
              ) : previewMode === "blocks" ? (
                <BlockBundleAtlas
                  blocks={index.blocks}
                  textures={textures}
                  candidates={index.candidates}
                  previewSelectionsByTexture={visibleSelectionsByTexture}
                  selectedTextureName={selectedTextureName}
                  onSelectTexture={selectTexture}
                />
              ) : previewMode === "auto" && activeTexture ? (
                <AutoTexturePreview
                  texture={activeTexture}
                  blocks={index.blocks}
                  candidates={activeCandidates}
                  selectedCandidateId={selectedCandidateId}
                  previewCandidateId={previewSelectionsByTexture[activeTexture.name] ?? null}
                  previewSelectionsByTexture={visibleSelectionsByTexture}
                  onSelectTexture={selectTexture}
                  onSelectCandidate={selectCandidate}
                  onUsePreview={setPreviewCandidate}
                  onClearPreview={clearPreviewCandidate}
                  onPromote={promoteLifecycle}
                  onReturnToCandidate={returnToCandidate}
                />
              ) : activeTexture ? (
                <TexturePreview
                  texture={activeTexture}
                  candidates={activeCandidates}
                  selectedCandidateId={selectedCandidateId}
                  previewCandidateId={previewSelectionsByTexture[activeTexture.name] ?? null}
                  onSelectCandidate={selectCandidate}
                  onUsePreview={setPreviewCandidate}
                  onClearPreview={clearPreviewCandidate}
                  onPromote={promoteLifecycle}
                  onReturnToCandidate={returnToCandidate}
                />
              ) : (
                <EmptyState loadStatus={loadStatus} />
              )}
            </>
          ) : (
            <EmptyState loadStatus={loadStatus} />
          )}
        </section>

        <aside className="inspector">
          {activeTexture ? (
            <TextureInspector texture={activeTexture} candidate={selectedCandidate} />
          ) : (
            <div className="panelNote">No texture selected.</div>
          )}
        </aside>
      </main>
    </div>
  );
}

function themeModeFromSystem(prefersDark: boolean): ThemeMode {
  return prefersDark ? "dark" : "light";
}

function oppositeThemeMode(themeMode: ThemeMode): ThemeMode {
  return themeMode === "dark" ? "light" : "dark";
}

function SummaryItem({ label, value }: { label: string; value: number }): JSX.Element {
  return (
    <div className="summaryItem">
      <strong>{value}</strong>
      <span>{label}</span>
    </div>
  );
}

function TexturePreview({
  texture,
  candidates,
  selectedCandidateId,
  previewCandidateId,
  onSelectCandidate,
  onUsePreview,
  onClearPreview,
  onPromote,
  onReturnToCandidate,
}: {
  texture: TextureIndexEntry;
  candidates: TextureCandidateEntry[];
  selectedCandidateId: string | null;
  previewCandidateId: string | null;
  onSelectCandidate: (id: string) => void;
  onUsePreview: (textureName: string, candidateId: string) => void;
  onClearPreview: (textureName: string) => void;
  onPromote: TextureLifecyclePromotionHandler;
  onReturnToCandidate: (textureName: string) => Promise<void>;
}): JSX.Element {
  return (
    <>
      <TexturePreviewHeader texture={texture} />
      <TextureLifecycleGrid
        texture={texture}
        candidate={candidates.find((entry) => entry.id === selectedCandidateId) ?? null}
      />
      <CandidateSection
        texture={texture}
        candidates={candidates}
        selectedCandidateId={selectedCandidateId}
        previewCandidateId={previewCandidateId}
        onSelectCandidate={onSelectCandidate}
        onUsePreview={onUsePreview}
        onClearPreview={onClearPreview}
        onPromote={onPromote}
        onReturnToCandidate={onReturnToCandidate}
      />
    </>
  );
}

function AutoTexturePreview({
  texture,
  blocks,
  candidates,
  selectedCandidateId,
  previewCandidateId,
  previewSelectionsByTexture,
  onSelectTexture,
  onSelectCandidate,
  onUsePreview,
  onClearPreview,
  onPromote,
  onReturnToCandidate,
}: {
  texture: TextureIndexEntry;
  blocks: BlockIndexEntry[];
  candidates: TextureCandidateEntry[];
  selectedCandidateId: string | null;
  previewCandidateId: string | null;
  previewSelectionsByTexture: Record<string, string>;
  onSelectTexture: (name: string) => void;
  onSelectCandidate: (id: string) => void;
  onUsePreview: (textureName: string, candidateId: string) => void;
  onClearPreview: (textureName: string) => void;
  onPromote: TextureLifecyclePromotionHandler;
  onReturnToCandidate: (textureName: string) => Promise<void>;
}): JSX.Element {
  const focusedBlocks = blocks.filter((block) => block.faces.some((face) => face.textureName === texture.name));
  return (
    <>
      <TexturePreviewHeader texture={texture} subtitle={autoPreviewSubtitle(texture)} />
      {focusedBlocks.length ? (
        <div className="autoPreviewBlock">
          <BlockBundleAtlas
            blocks={focusedBlocks}
            textures={[texture]}
            candidates={candidates}
            previewSelectionsByTexture={previewSelectionsByTexture}
            selectedTextureName={texture.name}
            onSelectTexture={onSelectTexture}
            title="Rendered Uses"
            summary={autoRenderedUsesSummary(texture, focusedBlocks)}
            emptyMessage="No rendered uses found for this texture."
          />
        </div>
      ) : null}
      <TextureLifecycleGrid
        texture={texture}
        candidate={candidates.find((entry) => entry.id === selectedCandidateId) ?? null}
      />
      <CandidateSection
        texture={texture}
        candidates={candidates}
        selectedCandidateId={selectedCandidateId}
        previewCandidateId={previewCandidateId}
        onSelectCandidate={onSelectCandidate}
        onUsePreview={onUsePreview}
        onClearPreview={onClearPreview}
        onPromote={onPromote}
        onReturnToCandidate={onReturnToCandidate}
      />
    </>
  );
}

function TexturePreviewHeader({
  texture,
  subtitle = lifecycleSubtitle(texture),
}: {
  texture: TextureIndexEntry;
  subtitle?: string;
}): JSX.Element {
  return (
    <div className="sectionHeader">
      <div>
        <h2>{texture.displayName}</h2>
        <p>{subtitle}</p>
      </div>
      <div className="chipGroup">
        <span className={`chip lifecycleChip ${texture.lifecycle.state}`} title={texture.lifecycle.note}>
          {lifecycleLabel(texture.lifecycle.state)}
        </span>
        <span className="chip">{texture.materialFamily}</span>
        {texture.vanillaUsage && texture.vanillaUsage.previewHint !== "unknown" ? (
          <span className="chip">{texture.vanillaUsage.previewHint}</span>
        ) : null}
        <span className="chip">{texture.tiling}</span>
        <span className="chip">{texture.rotation}</span>
        {texture.tintRole ? <span className="chip tintChip">{texture.tintRole}</span> : null}
      </div>
    </div>
  );
}

function TextureLifecycleGrid({
  texture,
  candidate,
}: {
  texture: TextureIndexEntry;
  candidate: TextureCandidateEntry | null;
}): JSX.Element {
  const candidateImage = candidate ? primaryCandidateImage(candidate) : null;
  const candidateRef =
    candidateImage ??
    (texture.lifecycle.state === "candidate"
      ? { ...texture.images.currentExport, label: "Candidate · not runtime eligible" }
      : { label: "Candidate", path: null, exists: false, missingCommand: null });
  return (
    <div className="imageGrid lifecycleGrid" aria-label="Texture lifecycle comparison">
      <LifecycleImageCard texture={texture} refInfo={candidateRef} stage="Candidate" />
      <LifecycleImageCard texture={texture} refInfo={texture.images.provisional} stage="Provisional" />
      <LifecycleImageCard texture={texture} refInfo={texture.images.curated} stage="Curated" />
      <LifecycleImageCard
        texture={texture}
        refInfo={texture.images.minecraftReference}
        stage="Minecraft Reference"
        readOnly
      />
    </div>
  );
}

function LifecycleImageCard({
  texture,
  refInfo,
  stage,
  readOnly = false,
}: {
  texture: TextureIndexEntry;
  refInfo: TextureImageRef;
  stage: string;
  readOnly?: boolean;
}): JSX.Element {
  const url = imageRefUrl(refInfo);
  return (
    <div className="imageCard lifecycleCard">
      <div className="imageCardHeader">
        <strong>{stage}</strong>
        <span>
          {readOnly
            ? refInfo.exists ? "read-only" : "not hosted"
            : refInfo.exists ? "available" : "empty"}
        </span>
      </div>
      {url ? (
        <div className="imageFrame">
          <img src={url} alt={`${texture.name} ${stage}`} />
        </div>
      ) : (
        <div className="missingImage">
          <span>{readOnly ? "Reference unavailable" : "No promoted texture"}</span>
          {refInfo.missingCommand ? <code>{refInfo.missingCommand}</code> : null}
        </div>
      )}
    </div>
  );
}

function autoPreviewSubtitle(texture: TextureIndexEntry): string {
  const hint = texture.vanillaUsage?.previewHint;
  if (!hint || hint === "unknown") {
    return lifecycleSubtitle(texture);
  }
  return `${hint} preview / ${lifecycleSubtitle(texture)}`;
}

function lifecycleSubtitle(texture: TextureIndexEntry): string {
  const bindings = texture.lifecycle.runtimeMaterials.length
    ? texture.lifecycle.runtimeMaterials.join(", ")
    : "no runtime binding";
  return `${lifecycleLabel(texture.lifecycle.state)} / ${bindings}`;
}

function lifecycleLabel(state: TextureIndexEntry["lifecycle"]["state"]): string {
  if (state === "legacy-derived") {
    return "Legacy derived LOD";
  }
  return `${state[0]!.toUpperCase()}${state.slice(1)}`;
}

function autoRenderedUsesSummary(texture: TextureIndexEntry, blocks: BlockIndexEntry[]): string {
  const hint = texture.vanillaUsage?.previewHint;
  const faces = blocks.reduce((sum, block) => sum + block.faces.filter((face) => face.textureName === texture.name).length, 0);
  const blockWord = blocks.length === 1 ? "block" : "blocks";
  const faceWord = faces === 1 ? "face" : "faces";
  return `${hint && hint !== "unknown" ? `${hint} / ` : ""}${blocks.length} ${blockWord} / ${faces} ${faceWord}`;
}

function CandidateSection({
  texture,
  candidates,
  selectedCandidateId,
  previewCandidateId,
  onSelectCandidate,
  onUsePreview,
  onClearPreview,
  onPromote,
  onReturnToCandidate,
}: {
  texture: TextureIndexEntry;
  candidates: TextureCandidateEntry[];
  selectedCandidateId: string | null;
  previewCandidateId: string | null;
  onSelectCandidate: (id: string) => void;
  onUsePreview: (textureName: string, candidateId: string) => void;
  onClearPreview: (textureName: string) => void;
  onPromote: TextureLifecyclePromotionHandler;
  onReturnToCandidate: (textureName: string) => Promise<void>;
}): JSX.Element {
  const sortedCandidates = [...candidates].sort(compareCandidateDisplay);
  const selectedCandidate = sortedCandidates.find((candidate) => candidate.id === selectedCandidateId) ?? null;
  const selectedCandidateIdForPromotion =
    selectedCandidate?.promotable && selectedCandidate.images.projected.exists
      ? selectedCandidate.id
      : null;
  const canPromote = texture.lifecycle.promotable;
  return (
    <section className="candidateSection">
      <div className="subsectionHeader">
        <div>
          <h3>Generated Candidates</h3>
          <p>
            {sortedCandidates.length ? `${sortedCandidates.length} linked to ${texture.name}` : "No local candidates linked yet."}
            {` / ${texture.lifecycle.note}`}
          </p>
        </div>
        <div className="candidateActions">
          {!HOSTED_READ_ONLY ? (
            <>
              <button
                className="inlineButton"
                type="button"
                disabled={!canPromote}
                title={canPromote ? "Promote the selected candidate or current recipe render" : texture.lifecycle.note}
                onClick={() =>
                  void onPromote(texture.name, selectedCandidateIdForPromotion, "provisional")
                }
              >
                Use as Provisional
              </button>
              <button
                className="inlineButton primaryAction"
                type="button"
                disabled={!canPromote}
                title={canPromote ? "Accept the selected candidate or current recipe render" : texture.lifecycle.note}
                onClick={() => void onPromote(texture.name, selectedCandidateIdForPromotion, "curated")}
              >
                Accept as Curated
              </button>
              {texture.lifecycle.state === "provisional" || texture.lifecycle.state === "curated" ? (
                <button className="inlineButton" type="button" onClick={() => void onReturnToCandidate(texture.name)}>
                  Return to Candidate
                </button>
              ) : null}
            </>
          ) : null}
          {previewCandidateId ? (
            <button className="inlineButton" type="button" onClick={() => onClearPreview(texture.name)}>
              Clear Preview
            </button>
          ) : null}
        </div>
      </div>
      {sortedCandidates.length ? (
        <div className="candidateGrid">
          {sortedCandidates.map((candidate) => (
            <CandidateCard
              key={candidate.id}
              candidate={candidate}
              selected={candidate.id === selectedCandidateId}
              previewed={candidate.id === previewCandidateId}
              active={false}
              onSelect={onSelectCandidate}
              onUsePreview={onUsePreview}
            />
          ))}
        </div>
      ) : (
        <div className="candidateEmpty">No local generated candidates are present for this texture.</div>
      )}
    </section>
  );
}

function CandidateCard({
  candidate,
  selected,
  previewed,
  active,
  onSelect,
  onUsePreview,
}: {
  candidate: TextureCandidateEntry;
  selected: boolean;
  previewed: boolean;
  active: boolean;
  onSelect: (id: string) => void;
  onUsePreview: (textureName: string, candidateId: string) => void;
}): JSX.Element {
  const image = primaryCandidateImage(candidate);
  const url = image ? imageRefUrl(image) : null;
  const sourceLabel = candidate.archived ? "archive" : candidate.source;
  return (
    <button
      className={candidateCardClassName(selected, previewed, active)}
      type="button"
      aria-pressed={selected}
      aria-label={`${candidate.codename} ${sourceLabel} candidate`}
      onClick={() => {
        onSelect(candidate.id);
        if (candidate.textureName) {
          onUsePreview(candidate.textureName, candidate.id);
        }
      }}
    >
      <div className="candidateImageFrame">
        {url && image ? <img src={url} alt={`${candidate.codename} ${image.label}`} /> : <span>No image</span>}
      </div>
      <div className="candidateBody">
        <div className="candidateTitleRow">
          <code>{candidate.codename}</code>
          <span className="candidatePills">
            {previewed ? <span className="previewPill">Previewing</span> : null}
            {active ? <span className="activePill">Pack</span> : null}
            <span className="sourcePill">{sourceLabel}</span>
          </span>
        </div>
        <div className="candidateMeta">
          <span>{candidate.status ?? "untriaged"}</span>
          {candidate.score !== null ? <span>score {formatScore(candidate.score)}</span> : null}
          {candidate.resolution !== null ? <span>{candidate.resolution}px</span> : null}
        </div>
        <div className="candidateFacts">
          <span>{candidate.seed !== null ? `seed ${candidate.seed}` : "seed unknown"}</span>
          <span>{candidate.strength !== null ? `strength ${candidate.strength.toFixed(2)}` : "strength unknown"}</span>
        </div>
        <div className="candidatePrompt">
          <strong>{candidate.promptPreset ?? "custom prompt"}</strong>
          {candidate.modelId ? <span>{candidate.modelId}</span> : null}
        </div>
        {candidate.reasons.length ? <p className="candidateReasons">{candidate.reasons.slice(0, 2).join("; ")}</p> : null}
      </div>
    </button>
  );
}

function candidateCardClassName(selected: boolean, previewed: boolean, active: boolean): string {
  return ["candidateCard", selected ? "selected" : "", previewed ? "previewed" : "", active ? "active" : ""].filter(Boolean).join(" ");
}

function compareCandidateDisplay(left: TextureCandidateEntry, right: TextureCandidateEntry): number {
  return (
    sourceOrder(left.source) - sourceOrder(right.source) ||
    left.codename.localeCompare(right.codename) ||
    left.id.localeCompare(right.id)
  );
}

function sourceOrder(source: TextureCandidateEntry["source"]): number {
  if (source === "archive") {
    return 0;
  }
  if (source === "projection") {
    return 1;
  }
  return 2;
}

function formatScore(score: number): string {
  return Number.isInteger(score) ? `${score}` : score.toFixed(2);
}

function TextureInspector({
  texture,
  candidate,
}: {
  texture: TextureIndexEntry;
  candidate: TextureCandidateEntry | null;
}): JSX.Element {
  return (
    <div className="inspectorStack">
      <InspectorSection title="Texture">
        <Field label="name" value={texture.name} />
        <Field label="lifecycle" value={lifecycleLabel(texture.lifecycle.state)} />
        <Field label="runtime material" value={joinOrNone(texture.lifecycle.runtimeMaterials)} />
        <Field label="lifecycle note" value={texture.lifecycle.note} />
        <Field label="size" value={`${texture.size}x${texture.size}`} />
        <Field label="source" value={texture.source} />
        <Field label="palette" value={texture.palette} />
        <Field label="base" value={texture.base} />
        <Field label="tint" value={texture.tintRole ?? "none"} />
        <Field label="source policy" value={sourcePolicyLabel(texture)} />
      </InspectorSection>

      <InspectorSection title="Catalog">
        <Field label="material" value={texture.materialFamily} />
        <Field label="tiling" value={texture.tiling} />
        <Field label="rotation" value={texture.rotation} />
        <Field label="authoring" value={texture.authoringRoles.length ? texture.authoringRoles.join(", ") : "none"} />
      </InspectorSection>

      <VanillaUsageInspector texture={texture} />

      <InspectorSection title="Block Usage">
        {texture.blockUsages.length ? (
          <div className="usageList">
            {texture.blockUsages.map((usage) => (
              <span key={`${usage.blockName}:${usage.face}`}>
                {usage.blockName} / {usage.face}
              </span>
            ))}
          </div>
        ) : (
          <div className="panelNote">No block usage in the authored pack.</div>
        )}
      </InspectorSection>

      <InspectorSection title="Paths">
        <Field label="export" value={texture.exportPath} />
        <Field label="runtime" value={texture.runtimeCompatPath ?? "none"} />
      </InspectorSection>

      <CandidateInspector candidate={candidate} />
    </div>
  );
}

function VanillaUsageInspector({ texture }: { texture: TextureIndexEntry }): JSX.Element {
  const usage = texture.vanillaUsage;
  return (
    <InspectorSection title="Vanilla Usage">
      {usage ? (
        <>
          <Field label="texture" value={usage.texture} />
          <Field label="preview" value={usage.previewHint} />
          <Field label="geometry" value={joinOrNone(usage.geometryKinds)} />
          <Field label="families" value={joinOrNone(usage.modelFamilies.slice(0, 5))} />
          <Field label="layers" value={joinOrNone(usage.renderLayers)} />
          <Field label="slots" value={joinOrNone(usage.textureSlots)} />
          <Field label="tint" value={joinOrNone(usage.tintRoles.length ? usage.tintRoles : usage.tintIndexes.map(String))} />
          <Field label="uses" value={`${usage.useCount} faces/particles across ${usage.blockCount} blocks`} />
          {usage.authoringNotes.length ? (
            <div className="reasonList">
              {usage.authoringNotes.map((note) => (
                <span key={note}>{note}</span>
              ))}
            </div>
          ) : null}
          {usage.uses.length ? (
            <div className="usageList">
              {usage.uses.slice(0, 12).map((entry) => (
                <span key={`${entry.block}:${entry.model}:${entry.selector}:${entry.face ?? entry.role}:${entry.textureSlot ?? ""}`}>
                  {entry.block} / {entry.textureSlot ?? entry.face ?? entry.role} / {entry.geometryKind}
                </span>
              ))}
              {usage.uses.length > 12 ? <span>{usage.uses.length - 12} more vanilla uses</span> : null}
            </div>
          ) : (
            <div className="panelNote">No vanilla blockstate/model use found for this texture.</div>
          )}
        </>
      ) : (
        <div className="panelNote">No single vanilla block texture counterpart.</div>
      )}
    </InspectorSection>
  );
}

function sourcePolicyLabel(texture: TextureIndexEntry): string {
  const neutrality = texture.tint?.sourceNeutrality;
  if (!neutrality) {
    return "none";
  }
  const parts = [`neutral mean <= ${neutrality.maxMeanSaturation}`];
  if (neutrality.maxPixelSaturation !== undefined) {
    parts.push(`pixel <= ${neutrality.maxPixelSaturation}`);
  }
  return parts.join(", ");
}

function joinOrNone(values: (string | number)[]): string {
  return values.length ? values.join(", ") : "none";
}

function CandidateInspector({ candidate }: { candidate: TextureCandidateEntry | null }): JSX.Element {
  if (!candidate) {
    return (
      <InspectorSection title="Candidate">
        <div className="panelNote">No generated candidate selected.</div>
      </InspectorSection>
    );
  }

  return (
    <>
      <InspectorSection title="Candidate">
        <Field label="codename" value={candidate.codename} />
        <Field label="source" value={candidate.archived ? "archive" : candidate.source} />
        <Field label="status" value={candidate.status ?? "untriaged"} />
        <Field label="score" value={candidate.score !== null ? formatScore(candidate.score) : "none"} />
        <Field label="seed" value={candidate.seed !== null ? `${candidate.seed}` : "unknown"} />
        <Field label="strength" value={candidate.strength !== null ? candidate.strength.toFixed(2) : "unknown"} />
        <Field label="size" value={candidateResolutionText(candidate)} />
        <Field label="archived" value={candidate.archived ? "yes" : "no"} />
      </InspectorSection>

      <InspectorSection title="Prompt">
        <Field label="preset" value={candidate.promptPreset ?? "none"} />
        <Field label="model" value={candidate.modelId ?? "unknown"} />
        <Field label="scheduler" value={candidate.scheduler ?? "unknown"} />
        <Field label="steps" value={candidate.steps !== null ? `${candidate.steps}` : "unknown"} />
        <Field label="prompt" value={candidate.prompt ?? "none"} />
        <Field label="negative" value={candidate.negativePrompt ?? "none"} />
      </InspectorSection>

      <InspectorSection title="Projection">
        {candidate.paletteColors.length ? <PaletteSwatches colors={candidate.paletteColors} /> : <div className="panelNote">No palette colors recorded.</div>}
        {candidate.reasons.length ? (
          <div className="reasonList">
            {candidate.reasons.map((reason) => (
              <span key={reason}>{reason}</span>
            ))}
          </div>
        ) : null}
      </InspectorSection>

      <InspectorSection title="Artifacts">
        <Field label="manifest" value={candidate.manifestPath ?? "none"} />
        <Field label="report" value={candidate.projectionReportPath ?? "none"} />
        <Field label="archive" value={candidate.archivePath ?? "none"} />
        {candidateImageFields(candidate).map(({ label, path }) => (
          <Field key={label} label={label} value={path ?? "none"} />
        ))}
      </InspectorSection>
    </>
  );
}

function PaletteSwatches({ colors }: { colors: string[] }): JSX.Element {
  return (
    <div className="paletteSwatches" aria-label="Projection palette">
      {colors.map((color, index) => (
        <span key={`${color}:${index}`} title={color} style={{ backgroundColor: color }} />
      ))}
    </div>
  );
}

function candidateResolutionText(candidate: TextureCandidateEntry): string {
  if (candidate.resolutions.length) {
    return candidate.resolutions.map((resolution) => `${resolution}px`).join(", ");
  }
  return candidate.resolution !== null ? `${candidate.resolution}px` : "unknown";
}

function candidateImageFields(candidate: TextureCandidateEntry): { label: string; path: string | null }[] {
  return [
    { label: "raw", path: candidate.images.raw.path },
    { label: "tile", path: candidate.images.rawTile.path },
    { label: "project", path: candidate.images.projected.path },
    { label: "review", path: candidate.images.reviewSheet.path },
    { label: "contact", path: candidate.images.contactSheet.path },
  ];
}

function InspectorSection({ title, children }: { title: string; children: ReactNode }): JSX.Element {
  return (
    <section className="inspectorSection">
      <h3>{title}</h3>
      {children}
    </section>
  );
}

function Field({ label, value }: { label: string; value: string }): JSX.Element {
  return (
    <div className="fieldRow">
      <span>{label}</span>
      <code>{value}</code>
    </div>
  );
}

function EmptyState({ loadStatus }: { loadStatus: string }): JSX.Element {
  return (
    <div className="emptyState">
      <strong>{loadStatus === "loading" ? "Building texture index" : "No texture selected"}</strong>
      <span>{loadStatus === "loading" ? "Reading the authored pack and local outputs." : "Pick a texture from the left list."}</span>
    </div>
  );
}
