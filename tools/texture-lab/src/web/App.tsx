import { useEffect } from "react";
import type { JSX, ReactNode } from "react";
import { useShallow } from "zustand/react/shallow";
import type { TextureCandidateEntry, TextureImageRef, TextureIndexEntry } from "../core/index-model";
import {
  activeTextureCandidates,
  filteredTextures,
  materialOptions,
  selectError,
  selectIndex,
  selectLoadStatus,
  selectMaterialFilter,
  selectSearch,
  selectedCandidate as selectedCandidateSelector,
  selectedTexture,
  selectSelectedCandidateId,
  selectSelectedTextureName,
  selectStatusFilter,
  statusOptions,
} from "./store/selectors";
import { imageRefUrl, imageUrl, useTextureLabStore } from "./store/textureLabStore";

export function App(): JSX.Element {
  const index = useTextureLabStore(selectIndex);
  const loadStatus = useTextureLabStore(selectLoadStatus);
  const error = useTextureLabStore(selectError);
  const textures = useTextureLabStore(useShallow(filteredTextures));
  const activeTexture = useTextureLabStore(selectedTexture);
  const activeCandidates = useTextureLabStore(useShallow(activeTextureCandidates));
  const selectedCandidate = useTextureLabStore(selectedCandidateSelector);
  const selectedTextureName = useTextureLabStore(selectSelectedTextureName);
  const selectedCandidateId = useTextureLabStore(selectSelectedCandidateId);
  const search = useTextureLabStore(selectSearch);
  const materialFilter = useTextureLabStore(selectMaterialFilter);
  const statusFilter = useTextureLabStore(selectStatusFilter);
  const materials = useTextureLabStore(useShallow(materialOptions));
  const statuses = useTextureLabStore(useShallow(statusOptions));
  const loadIndex = useTextureLabStore((state) => state.loadIndex);
  const reindex = useTextureLabStore((state) => state.reindex);
  const selectTexture = useTextureLabStore((state) => state.selectTexture);
  const selectCandidate = useTextureLabStore((state) => state.selectCandidate);
  const setSearch = useTextureLabStore((state) => state.setSearch);
  const setMaterialFilter = useTextureLabStore((state) => state.setMaterialFilter);
  const setStatusFilter = useTextureLabStore((state) => state.setStatusFilter);

  useEffect(() => {
    void loadIndex();
  }, [loadIndex]);

  return (
    <div className="appShell">
      <header className="topBar">
        <div>
          <div className="eyebrow">Texture Lab</div>
          <h1>{index?.pack.name ?? "mclone-default"}</h1>
        </div>
        <div className="summaryStrip" aria-label="Pack summary">
          <SummaryItem label="textures" value={index?.summary.authoredTextures ?? 0} />
          <SummaryItem label="exports" value={index?.summary.currentExportsPresent ?? 0} />
          <SummaryItem label="sheets" value={index?.summary.sheetsPresent ?? 0} />
          <SummaryItem label="runtime" value={index?.summary.runtimeExportsPresent ?? 0} />
          <SummaryItem label="candidates" value={index?.summary.associatedCandidateCount ?? 0} />
        </div>
        <button className="toolbarButton" type="button" onClick={() => void reindex()} disabled={loadStatus === "loading"}>
          {loadStatus === "loading" ? "Indexing" : "Reindex"}
        </button>
      </header>

      {error ? <div className="errorBanner">{error}</div> : null}
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
              Status
              <select value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}>
                <option value="all">All</option>
                {statuses.map((status) => (
                  <option key={status} value={status}>
                    {status}
                  </option>
                ))}
              </select>
            </label>
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
                  {texture.materialFamily} / {texture.size}px / {texture.source}
                </span>
              </button>
            ))}
          </div>
        </aside>

        <section className="previewPane">
          {activeTexture ? (
            <TexturePreview
              texture={activeTexture}
              candidates={activeCandidates}
              selectedCandidateId={selectedCandidateId}
              onSelectCandidate={selectCandidate}
            />
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
  onSelectCandidate,
}: {
  texture: TextureIndexEntry;
  candidates: TextureCandidateEntry[];
  selectedCandidateId: string | null;
  onSelectCandidate: (id: string) => void;
}): JSX.Element {
  return (
    <>
      <div className="sectionHeader">
        <div>
          <h2>{texture.displayName}</h2>
          <p>
            {texture.exportPath} / {texture.status}
          </p>
        </div>
        <div className="chipGroup">
          <span className="chip">{texture.materialFamily}</span>
          <span className="chip">{texture.tiling}</span>
          <span className="chip">{texture.rotation}</span>
          {texture.tintRole ? <span className="chip tintChip">{texture.tintRole}</span> : null}
        </div>
      </div>
      <div className="imageGrid">
        <ImageCard texture={texture} imageKind="currentExport" refInfo={texture.images.currentExport} />
        <ImageCard texture={texture} imageKind="runtimeExport" refInfo={texture.images.runtimeExport} />
        <ImageCard texture={texture} imageKind="sheet" refInfo={texture.images.sheet} wide />
      </div>
      <CandidateSection
        texture={texture}
        candidates={candidates}
        selectedCandidateId={selectedCandidateId}
        onSelectCandidate={onSelectCandidate}
      />
    </>
  );
}

function ImageCard({
  texture,
  imageKind,
  refInfo,
  wide = false,
}: {
  texture: TextureIndexEntry;
  imageKind: keyof TextureIndexEntry["images"];
  refInfo: TextureImageRef;
  wide?: boolean;
}): JSX.Element {
  const url = imageUrl(texture, imageKind);
  return (
    <div className={wide ? "imageCard wide" : "imageCard"}>
      <div className="imageCardHeader">
        <strong>{refInfo.label}</strong>
        <span>{refInfo.exists ? "found" : "missing"}</span>
      </div>
      {url ? (
        <div className="imageFrame">
          <img src={url} alt={`${texture.name} ${refInfo.label}`} />
        </div>
      ) : (
        <div className="missingImage">
          <span>Missing</span>
          {refInfo.missingCommand ? <code>{refInfo.missingCommand}</code> : <span>No runtime-compatible path</span>}
        </div>
      )}
      {refInfo.path ? <code className="pathLine">{refInfo.path}</code> : null}
    </div>
  );
}

function CandidateSection({
  texture,
  candidates,
  selectedCandidateId,
  onSelectCandidate,
}: {
  texture: TextureIndexEntry;
  candidates: TextureCandidateEntry[];
  selectedCandidateId: string | null;
  onSelectCandidate: (id: string) => void;
}): JSX.Element {
  const sortedCandidates = [...candidates].sort(compareCandidateDisplay);
  return (
    <section className="candidateSection">
      <div className="subsectionHeader">
        <div>
          <h3>Generated Candidates</h3>
          <p>{sortedCandidates.length ? `${sortedCandidates.length} linked to ${texture.name}` : "No local candidates linked yet."}</p>
        </div>
      </div>
      {sortedCandidates.length ? (
        <div className="candidateGrid">
          {sortedCandidates.map((candidate) => (
            <CandidateCard
              key={candidate.id}
              candidate={candidate}
              selected={candidate.id === selectedCandidateId}
              onSelect={onSelectCandidate}
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
  onSelect,
}: {
  candidate: TextureCandidateEntry;
  selected: boolean;
  onSelect: (id: string) => void;
}): JSX.Element {
  const image = primaryCandidateImage(candidate);
  const url = image ? imageRefUrl(image) : null;
  const sourceLabel = candidate.archived ? "archive" : candidate.source;
  return (
    <button
      className={selected ? "candidateCard selected" : "candidateCard"}
      type="button"
      aria-pressed={selected}
      aria-label={`${candidate.codename} ${sourceLabel} candidate`}
      onClick={() => onSelect(candidate.id)}
    >
      <div className="candidateImageFrame">
        {url && image ? <img src={url} alt={`${candidate.codename} ${image.label}`} /> : <span>No image</span>}
      </div>
      <div className="candidateBody">
        <div className="candidateTitleRow">
          <code>{candidate.codename}</code>
          <span className="sourcePill">{sourceLabel}</span>
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

function primaryCandidateImage(candidate: TextureCandidateEntry): TextureImageRef | null {
  return (
    [
      candidate.images.projected,
      candidate.images.raw,
      candidate.images.rawTile,
      candidate.images.reviewSheet,
      candidate.images.contactSheet,
    ].find((image) => image.exists) ?? null
  );
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
        <Field label="size" value={`${texture.size}x${texture.size}`} />
        <Field label="source" value={texture.source} />
        <Field label="palette" value={texture.palette} />
        <Field label="base" value={texture.base} />
        <Field label="tint" value={texture.tintRole ?? "none"} />
      </InspectorSection>

      <InspectorSection title="Catalog">
        <Field label="status" value={texture.status} />
        <Field label="material" value={texture.materialFamily} />
        <Field label="tiling" value={texture.tiling} />
        <Field label="rotation" value={texture.rotation} />
        <Field label="authoring" value={texture.authoringRoles.length ? texture.authoringRoles.join(", ") : "none"} />
      </InspectorSection>

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
