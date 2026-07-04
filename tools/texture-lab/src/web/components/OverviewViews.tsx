import type { JSX } from "react";
import type { BlockIndexEntry, TextureCandidateEntry, TextureImageRef, TextureIndexEntry } from "../../core/index-model";
import { primaryCandidateImage } from "../candidate-images";
import type { PreviewMode } from "../store/textureLabStore";
import { imageRefUrl, tintedImageRefUrl } from "../store/textureLabStore";

type VisibleBlockEntry = { block: BlockIndexEntry; faces: BlockIndexEntry["faces"] };

export function PreviewModeTabs({
  mode,
  onChange,
}: {
  mode: PreviewMode;
  onChange: (mode: PreviewMode) => void;
}): JSX.Element {
  const modes: { mode: PreviewMode; label: string }[] = [
    { mode: "auto", label: "Auto" },
    { mode: "detail", label: "Detail" },
    { mode: "atlas", label: "Atlas" },
    { mode: "blocks", label: "Blocks" },
  ];
  return (
    <div className="viewTabs" aria-label="Preview mode">
      {modes.map((entry) => (
        <button
          key={entry.mode}
          className={entry.mode === mode ? "viewTab selected" : "viewTab"}
          type="button"
          aria-pressed={entry.mode === mode}
          onClick={() => onChange(entry.mode)}
        >
          {entry.label}
        </button>
      ))}
    </div>
  );
}

export function TextureAtlas({
  textures,
  candidates,
  previewSelectionsByTexture,
  selectedTextureName,
  onSelectTexture,
}: {
  textures: TextureIndexEntry[];
  candidates: TextureCandidateEntry[];
  previewSelectionsByTexture: Record<string, string>;
  selectedTextureName: string | null;
  onSelectTexture: (name: string) => void;
}): JSX.Element {
  const groups = groupTexturesByMaterial(textures);
  const candidateCounts = candidateCountByTexture(candidates);
  const candidateById = candidateMapById(candidates);

  return (
    <div className="overviewStack">
      <div className="sectionHeader overviewHeader">
        <div>
          <h2>Texture Atlas</h2>
          <p>{textures.length} filtered textures</p>
        </div>
      </div>
      {groups.length ? (
        groups.map((group) => (
          <section key={group.material} className="atlasGroup" aria-label={`${group.material} texture atlas`}>
            <div className="subsectionHeader">
              <div>
                <h3>{humanizeName(group.material)}</h3>
                <p>{group.textures.length} textures</p>
              </div>
            </div>
            <div className="atlasGrid">
              {group.textures.map((texture) => (
                <AtlasTextureCard
                  key={texture.name}
                  texture={texture}
                  candidateCount={candidateCounts.get(texture.name) ?? 0}
                  previewCandidate={candidateById.get(previewSelectionsByTexture[texture.name] ?? "") ?? null}
                  selected={texture.name === selectedTextureName}
                  onSelect={onSelectTexture}
                />
              ))}
            </div>
          </section>
        ))
      ) : (
        <div className="candidateEmpty">No textures match the current filters.</div>
      )}
    </div>
  );
}

export function BlockBundleAtlas({
  blocks,
  textures,
  candidates,
  previewSelectionsByTexture,
  selectedTextureName,
  onSelectTexture,
  title = "Block Bundles",
  summary,
  emptyMessage = "No block bundles match the current filters.",
}: {
  blocks: BlockIndexEntry[];
  textures: TextureIndexEntry[];
  candidates: TextureCandidateEntry[];
  previewSelectionsByTexture: Record<string, string>;
  selectedTextureName: string | null;
  onSelectTexture: (name: string) => void;
  title?: string;
  summary?: string;
  emptyMessage?: string;
}): JSX.Element {
  const textureByName = new Map(textures.map((texture) => [texture.name, texture]));
  const candidateById = candidateMapById(candidates);
  const visibleBlocks = blocks
    .map((block) => ({
      block,
      faces: block.faces.filter((face) => textureByName.has(face.textureName)),
    }))
    .filter((entry) => entry.faces.length > 0);
  const visibleGroups = groupBlocksByPreviewSource(visibleBlocks);

  return (
    <div className="overviewStack">
      <div className="sectionHeader overviewHeader">
        <div>
          <h2>{title}</h2>
          <p>{summary ?? `${visibleBlocks.length} blocks with filtered textures`}</p>
        </div>
      </div>
      {visibleGroups.length ? (
        <div className="blockBundleGroups">
          {visibleGroups.map((group) => (
            <section
              key={group.previewSource}
              className="blockBundleSourceGroup"
              aria-label={blockPreviewSourceTitle(group.previewSource)}
            >
              <div className="subsectionHeader blockSourceHeader">
                <div>
                  <h3>{blockPreviewSourceTitle(group.previewSource)}</h3>
                  <p>{group.entries.length} blocks</p>
                </div>
              </div>
              <div className="blockBundleGrid">
                {group.entries.map(({ block, faces }) => (
                  <section key={block.name} className="blockBundleCard" aria-label={`${block.name} block bundle`}>
                    <div className="blockBundleHeader">
                      <strong>{humanizeName(block.name)}</strong>
                      <span>
                        {block.kind} / {faces.length} faces / {blockPreviewSourceLabel(block.previewSource)}
                      </span>
                    </div>
                    <div className="blockBundleBody">
                      <BlockSheetPreview blockName={block.name} image={block.sheet} />
                      <div className="blockFaceGrid">
                        {faces.map((face) => {
                          const texture = textureByName.get(face.textureName);
                          return texture ? (
                            <button
                              key={`${block.name}:${face.face}:${face.textureName}`}
                              className={texture.name === selectedTextureName ? "blockFaceTile selected" : "blockFaceTile"}
                              type="button"
                              aria-pressed={texture.name === selectedTextureName}
                              aria-label={`${block.name} ${face.face} uses ${texture.displayName}`}
                              onClick={() => onSelectTexture(texture.name)}
                            >
                              <div className="blockFaceTitle">
                                <strong>{face.face}</strong>
                                <span>{texture.name}</span>
                              </div>
                              <SplitTextureCompare
                                texture={texture}
                                previewCandidate={candidateById.get(previewSelectionsByTexture[texture.name] ?? "") ?? null}
                                compact
                              />
                            </button>
                          ) : null;
                        })}
                      </div>
                    </div>
                  </section>
                ))}
              </div>
            </section>
          ))}
        </div>
      ) : (
        <div className="candidateEmpty">{emptyMessage}</div>
      )}
    </div>
  );
}

function AtlasTextureCard({
  texture,
  candidateCount,
  previewCandidate,
  selected,
  onSelect,
}: {
  texture: TextureIndexEntry;
  candidateCount: number;
  previewCandidate: TextureCandidateEntry | null;
  selected: boolean;
  onSelect: (name: string) => void;
}): JSX.Element {
  return (
    <button
      className={selected ? "atlasCard selected" : "atlasCard"}
      type="button"
      aria-pressed={selected}
      aria-label={`${texture.displayName} atlas comparison`}
      onClick={() => onSelect(texture.name)}
    >
      <div className="atlasCardHeader">
        <strong>{texture.displayName}</strong>
        <span className={texture.artSource.kind === "procedural-placeholder" ? "placeholderHeaderBadge" : undefined}>
          {texture.artSource.kind === "procedural-placeholder" ? "placeholder" : texture.status}
        </span>
      </div>
      <SplitTextureCompare texture={texture} previewCandidate={previewCandidate} />
      <div className="atlasCardMeta">
        <span className={artSourceMetaClass(texture)} title={texture.artSource.description}>
          {texture.artSource.label}
        </span>
        <span>{texture.size}px</span>
        <span>{texture.tiling}</span>
        {texture.tintRole ? <span>{texture.tintRole}</span> : null}
        {texture.frozen ? <span>{texture.frozen.codename ? `frozen ${texture.frozen.codename}` : "frozen"}</span> : null}
        {candidateCount > 0 ? <span>{candidateCount} candidates</span> : null}
        {previewCandidate ? <span>preview {previewCandidate.codename}</span> : null}
      </div>
    </button>
  );
}

function artSourceMetaClass(texture: TextureIndexEntry): string {
  return texture.artSource.kind === "procedural-placeholder" ? "artSourceMeta placeholderMeta" : "artSourceMeta";
}

function groupBlocksByPreviewSource(entries: VisibleBlockEntry[]): {
  previewSource: BlockIndexEntry["previewSource"];
  entries: VisibleBlockEntry[];
}[] {
  return (["authored", "vanilla-derived"] as const)
    .map((previewSource) => ({
      previewSource,
      entries: entries.filter((entry) => entry.block.previewSource === previewSource),
    }))
    .filter((group) => group.entries.length > 0);
}

function blockPreviewSourceLabel(source: BlockIndexEntry["previewSource"]): string {
  return source === "vanilla-derived" ? "vanilla-derived preview" : "authored block";
}

function blockPreviewSourceTitle(source: BlockIndexEntry["previewSource"]): string {
  return source === "vanilla-derived" ? "Vanilla-Derived Review Blocks" : "Authored Pack Blocks";
}

function BlockSheetPreview({ blockName, image }: { blockName: string; image: TextureImageRef }): JSX.Element {
  const url = imageRefUrl(image);
  return (
    <div className="blockSheetPreview">
      {url ? <img src={url} alt={`${blockName} block sheet`} /> : <span>Missing sheet</span>}
    </div>
  );
}

function SplitTextureCompare({
  texture,
  previewCandidate,
  compact = false,
}: {
  texture: TextureIndexEntry;
  previewCandidate: TextureCandidateEntry | null;
  compact?: boolean;
}): JSX.Element {
  const previewImage = previewCandidate ? primaryCandidateImage(previewCandidate) : null;
  const oursImage = previewImage ?? texture.images.currentExport;
  if (texture.tint) {
    return (
      <div className={compact ? "splitCompare tinted compact" : "splitCompare tinted"}>
        <CompareImage label="Ours raw" image={oursImage} alt={oursAlt(texture, previewCandidate, "raw")} />
        <CompareImage
          label="Ours tinted"
          image={oursImage}
          tint={texture.tint.normal}
          alt={oursAlt(texture, previewCandidate, "tinted")}
        />
        <CompareImage label="Minecraft raw" image={texture.images.minecraftReference} alt={`${texture.name} raw Minecraft reference`} />
        <CompareImage
          label="Minecraft tinted"
          image={texture.images.minecraftReference}
          tint={texture.tint.normal}
          alt={`${texture.name} tinted Minecraft reference`}
        />
      </div>
    );
  }
  return (
    <div className={compact ? "splitCompare compact" : "splitCompare"}>
      <CompareImage
        label={previewCandidate ? `Ours · ${previewCandidate.codename}` : "Ours"}
        image={oursImage}
        alt={previewCandidate ? `${previewCandidate.codename} preview candidate` : `${texture.name} current export`}
      />
      <CompareImage label="Minecraft" image={texture.images.minecraftReference} alt={`${texture.name} Minecraft reference`} />
    </div>
  );
}

function CompareImage({
  label,
  image,
  alt,
  tint,
}: {
  label: string;
  image: TextureImageRef;
  alt: string;
  tint?: string;
}): JSX.Element {
  const url = tint ? tintedImageRefUrl(image, tint) : imageRefUrl(image);
  return (
    <div className="comparePanel">
      <span className="compareLabel">{label}</span>
      <div className="compareFrame">
        {url ? <img src={url} alt={alt} /> : <span className="compareMissing">Missing</span>}
      </div>
    </div>
  );
}

function oursAlt(texture: TextureIndexEntry, previewCandidate: TextureCandidateEntry | null, variant: "raw" | "tinted"): string {
  return previewCandidate
    ? `${previewCandidate.codename} ${variant} preview candidate`
    : `${texture.name} ${variant} current export`;
}

function groupTexturesByMaterial(textures: TextureIndexEntry[]): { material: string; textures: TextureIndexEntry[] }[] {
  const groups = new Map<string, TextureIndexEntry[]>();
  for (const texture of textures) {
    const group = groups.get(texture.materialFamily) ?? [];
    group.push(texture);
    groups.set(texture.materialFamily, group);
  }
  return [...groups.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([material, groupTextures]) => ({
      material,
      textures: groupTextures.sort((left, right) => left.name.localeCompare(right.name)),
    }));
}

function candidateCountByTexture(candidates: TextureCandidateEntry[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const candidate of candidates) {
    if (!candidate.textureName) {
      continue;
    }
    counts.set(candidate.textureName, (counts.get(candidate.textureName) ?? 0) + 1);
  }
  return counts;
}

function candidateMapById(candidates: TextureCandidateEntry[]): Map<string, TextureCandidateEntry> {
  return new Map(candidates.map((candidate) => [candidate.id, candidate]));
}

function humanizeName(name: string): string {
  return name
    .split(/[-_]/)
    .map((part) => (part.length > 0 ? `${part[0]!.toUpperCase()}${part.slice(1)}` : part))
    .join(" ");
}
