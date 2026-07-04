import { useEffect } from "react";
import type { JSX, ReactNode } from "react";
import { useShallow } from "zustand/react/shallow";
import type { TextureCandidateEntry, TextureImageRef, TextureIndexEntry } from "../core/index-model";
import {
  filteredTextures,
  materialOptions,
  selectError,
  selectedTexture,
  selectIndex,
  selectLoadStatus,
  selectMaterialFilter,
  selectSearch,
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
  const selectedTextureName = useTextureLabStore(selectSelectedTextureName);
  const search = useTextureLabStore(selectSearch);
  const materialFilter = useTextureLabStore(selectMaterialFilter);
  const statusFilter = useTextureLabStore(selectStatusFilter);
  const materials = useTextureLabStore(useShallow(materialOptions));
  const statuses = useTextureLabStore(useShallow(statusOptions));
  const loadIndex = useTextureLabStore((state) => state.loadIndex);
  const reindex = useTextureLabStore((state) => state.reindex);
  const selectTexture = useTextureLabStore((state) => state.selectTexture);
  const setSearch = useTextureLabStore((state) => state.setSearch);
  const setMaterialFilter = useTextureLabStore((state) => state.setMaterialFilter);
  const setStatusFilter = useTextureLabStore((state) => state.setStatusFilter);
  const activeCandidates =
    index && activeTexture
      ? index.candidates.filter((candidate) => candidate.textureName === activeTexture.name)
      : [];

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
          {activeTexture ? <TexturePreview texture={activeTexture} candidates={activeCandidates} /> : <EmptyState loadStatus={loadStatus} />}
        </section>

        <aside className="inspector">
          {activeTexture ? <TextureInspector texture={activeTexture} /> : <div className="panelNote">No texture selected.</div>}
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

function TexturePreview({ texture, candidates }: { texture: TextureIndexEntry; candidates: TextureCandidateEntry[] }): JSX.Element {
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
      <CandidateSection texture={texture} candidates={candidates} />
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
}: {
  texture: TextureIndexEntry;
  candidates: TextureCandidateEntry[];
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
            <CandidateCard key={candidate.id} candidate={candidate} />
          ))}
        </div>
      ) : (
        <div className="candidateEmpty">No local generated candidates are present for this texture.</div>
      )}
    </section>
  );
}

function CandidateCard({ candidate }: { candidate: TextureCandidateEntry }): JSX.Element {
  const image = primaryCandidateImage(candidate);
  const url = image ? imageRefUrl(image) : null;
  const sourceLabel = candidate.archived ? "archive" : candidate.source;
  return (
    <article className="candidateCard">
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
    </article>
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

function TextureInspector({ texture }: { texture: TextureIndexEntry }): JSX.Element {
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
    </div>
  );
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
