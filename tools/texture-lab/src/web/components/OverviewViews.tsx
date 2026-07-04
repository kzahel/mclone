import type { JSX } from "react";
import type { BlockIndexEntry, TextureCandidateEntry, TextureImageRef, TextureIndexEntry } from "../../core/index-model";
import type { PreviewMode } from "../store/textureLabStore";
import { imageRefUrl } from "../store/textureLabStore";

export function PreviewModeTabs({
  mode,
  onChange,
}: {
  mode: PreviewMode;
  onChange: (mode: PreviewMode) => void;
}): JSX.Element {
  const modes: { mode: PreviewMode; label: string }[] = [
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
  selectedTextureName,
  onSelectTexture,
}: {
  textures: TextureIndexEntry[];
  candidates: TextureCandidateEntry[];
  selectedTextureName: string | null;
  onSelectTexture: (name: string) => void;
}): JSX.Element {
  const groups = groupTexturesByMaterial(textures);
  const candidateCounts = candidateCountByTexture(candidates);

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
  selectedTextureName,
  onSelectTexture,
}: {
  blocks: BlockIndexEntry[];
  textures: TextureIndexEntry[];
  selectedTextureName: string | null;
  onSelectTexture: (name: string) => void;
}): JSX.Element {
  const textureByName = new Map(textures.map((texture) => [texture.name, texture]));
  const visibleBlocks = blocks
    .map((block) => ({
      block,
      faces: block.faces.filter((face) => textureByName.has(face.textureName)),
    }))
    .filter((entry) => entry.faces.length > 0);

  return (
    <div className="overviewStack">
      <div className="sectionHeader overviewHeader">
        <div>
          <h2>Block Bundles</h2>
          <p>{visibleBlocks.length} blocks with filtered textures</p>
        </div>
      </div>
      {visibleBlocks.length ? (
        <div className="blockBundleGrid">
          {visibleBlocks.map(({ block, faces }) => (
            <section key={block.name} className="blockBundleCard" aria-label={`${block.name} block bundle`}>
              <div className="blockBundleHeader">
                <strong>{humanizeName(block.name)}</strong>
                <span>{faces.length} faces</span>
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
                        <SplitTextureCompare texture={texture} compact />
                      </button>
                    ) : null;
                  })}
                </div>
              </div>
            </section>
          ))}
        </div>
      ) : (
        <div className="candidateEmpty">No block bundles match the current filters.</div>
      )}
    </div>
  );
}

function AtlasTextureCard({
  texture,
  candidateCount,
  selected,
  onSelect,
}: {
  texture: TextureIndexEntry;
  candidateCount: number;
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
        <span>{texture.status}</span>
      </div>
      <SplitTextureCompare texture={texture} />
      <div className="atlasCardMeta">
        <span>{texture.size}px</span>
        <span>{texture.tiling}</span>
        {texture.tintRole ? <span>{texture.tintRole}</span> : null}
        {candidateCount > 0 ? <span>{candidateCount} candidates</span> : null}
      </div>
    </button>
  );
}

function BlockSheetPreview({ blockName, image }: { blockName: string; image: TextureImageRef }): JSX.Element {
  const url = imageRefUrl(image);
  return (
    <div className="blockSheetPreview">
      {url ? <img src={url} alt={`${blockName} block sheet`} /> : <span>Missing sheet</span>}
    </div>
  );
}

function SplitTextureCompare({ texture, compact = false }: { texture: TextureIndexEntry; compact?: boolean }): JSX.Element {
  return (
    <div className={compact ? "splitCompare compact" : "splitCompare"}>
      <CompareImage label="Ours" image={texture.images.currentExport} alt={`${texture.name} current export`} />
      <CompareImage label="Minecraft" image={texture.images.minecraftReference} alt={`${texture.name} Minecraft reference`} />
    </div>
  );
}

function CompareImage({ label, image, alt }: { label: string; image: TextureImageRef; alt: string }): JSX.Element {
  const url = imageRefUrl(image);
  return (
    <div className="comparePanel">
      <span className="compareLabel">{label}</span>
      <div className="compareFrame">
        {url ? <img src={url} alt={alt} /> : <span className="compareMissing">Missing</span>}
      </div>
    </div>
  );
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

function humanizeName(name: string): string {
  return name
    .split(/[-_]/)
    .map((part) => (part.length > 0 ? `${part[0]!.toUpperCase()}${part.slice(1)}` : part))
    .join(" ");
}
