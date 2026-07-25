import fs from "node:fs/promises";
import path from "node:path";
import type {
  TextureCandidateEntry,
  TextureImageRef,
  TextureIndexEntry,
  TextureVanillaUsage,
  VanillaCoverageAuthoredTexture,
  VanillaCoverageEntry,
  VanillaCoverageIndex,
  VanillaCoverageStatus,
} from "./index-model";

export interface BuildVanillaCoverageIndexOptions {
  referenceRoot: string | null;
  textures: TextureIndexEntry[];
  candidates: TextureCandidateEntry[];
  vanillaUsageByTexture: Map<string, TextureVanillaUsage>;
}

const BLOCK_TEXTURE_ROOT = path.join("assets", "minecraft", "textures", "block");

export async function buildVanillaCoverageIndex(options: BuildVanillaCoverageIndexOptions): Promise<VanillaCoverageIndex> {
  if (!options.referenceRoot) {
    return emptyCoverage(null);
  }

  const blockTextureRoot = path.join(options.referenceRoot, BLOCK_TEXTURE_ROOT);
  const files = (await listFiles(blockTextureRoot)).filter((file) => file.endsWith(".png")).sort();
  const authoredByVanillaTexture = authoredTextureMap(options.textures);
  const candidateCounts = candidateCountsByTexture(options.candidates);
  const entries = await Promise.all(
    files.map((file) => coverageEntry(file, blockTextureRoot, authoredByVanillaTexture, candidateCounts, options.vanillaUsageByTexture)),
  );

  return {
    referenceRoot: options.referenceRoot,
    summary: {
      vanillaTextureCount: entries.length,
      coveredTextureCount: entries.filter((entry) => entry.authoredTextures.length > 0).length,
      missingTextureCount: entries.filter((entry) => entry.status === "missing").length,
      placeholderTextureCount: entries.filter((entry) => entry.status === "placeholder").length,
      candidateTextureCount: entries.filter((entry) => entry.status === "candidate").length,
      frozenTextureCount: entries.filter((entry) => entry.status === "frozen").length,
    },
    entries,
  };
}

async function coverageEntry(
  file: string,
  blockTextureRoot: string,
  authoredByVanillaTexture: Map<string, TextureIndexEntry[]>,
  candidateCounts: Map<string, number>,
  vanillaUsageByTexture: Map<string, TextureVanillaUsage>,
): Promise<VanillaCoverageEntry> {
  const name = normalizePath(path.relative(blockTextureRoot, file)).replace(/\.png$/u, "");
  const texture = `minecraft:block/${name}`;
  const authoredTextures = authoredByVanillaTexture.get(texture) ?? [];
  const candidateCount = authoredTextures.reduce((count, authored) => count + (candidateCounts.get(authored.name) ?? 0), 0);
  const vanillaUsage = vanillaUsageByTexture.get(texture);

  return {
    texture,
    name,
    displayName: displayNameFrom(name),
    materialFamily: materialFamilyFrom(name, authoredTextures, vanillaUsage),
    status: coverageStatus(authoredTextures, candidateCount),
    candidateCount,
    blockCount: vanillaUsage?.blockCount ?? 0,
    useCount: vanillaUsage?.useCount ?? 0,
    previewHint: vanillaUsage?.previewHint ?? "unknown",
    geometryKinds: vanillaUsage?.geometryKinds ?? [],
    renderLayers: vanillaUsage?.renderLayers ?? [],
    tintRoles: vanillaUsage?.tintRoles ?? [],
    minecraftReference: await imageRef("Minecraft reference", file, "./scripts/extract-assets.sh"),
    authoredTextures: authoredTextures.map(authoredCoverageEntry),
  };
}

function authoredTextureMap(textures: TextureIndexEntry[]): Map<string, TextureIndexEntry[]> {
  const byVanillaTexture = new Map<string, TextureIndexEntry[]>();
  for (const texture of textures) {
    const vanillaTexture = texture.vanillaUsage?.texture;
    if (!vanillaTexture) {
      continue;
    }
    const entries = byVanillaTexture.get(vanillaTexture) ?? [];
    entries.push(texture);
    byVanillaTexture.set(vanillaTexture, entries);
  }
  for (const entries of byVanillaTexture.values()) {
    entries.sort((left, right) => left.name.localeCompare(right.name));
  }
  return byVanillaTexture;
}

function authoredCoverageEntry(texture: TextureIndexEntry): VanillaCoverageAuthoredTexture {
  return {
    textureName: texture.name,
    displayName: texture.displayName,
    artSourceLabel: texture.artSource.label,
    artSourceKind: texture.artSource.kind,
    status: texture.status,
    source: texture.source,
    frozen: Boolean(texture.frozen),
    currentExport: texture.images.currentExport,
  };
}

function coverageStatus(textures: TextureIndexEntry[], candidateCount: number): VanillaCoverageStatus {
  if (textures.length === 0) {
    return "missing";
  }
  if (textures.some((texture) => texture.frozen)) {
    return "frozen";
  }
  if (candidateCount > 0) {
    return "candidate";
  }
  if (textures.some((texture) => texture.status === "accepted")) {
    return "accepted";
  }
  if (textures.some((texture) => texture.status === "reviewed")) {
    return "reviewed";
  }
  if (textures.every((texture) => texture.artSource.kind === "procedural-placeholder")) {
    return "placeholder";
  }
  return "draft";
}

function candidateCountsByTexture(candidates: TextureCandidateEntry[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const candidate of candidates) {
    if (!candidate.textureName) {
      continue;
    }
    counts.set(candidate.textureName, (counts.get(candidate.textureName) ?? 0) + 1);
  }
  return counts;
}

function materialFamilyFrom(name: string, authoredTextures: TextureIndexEntry[], vanillaUsage: TextureVanillaUsage | undefined): string {
  const authoredFamily = authoredTextures[0]?.materialFamily;
  if (authoredFamily) {
    return authoredFamily;
  }
  if (vanillaUsage?.tintRoles.includes("grass") || name.includes("grass")) {
    return "grass";
  }
  if (vanillaUsage?.tintRoles.includes("foliage") || name.includes("leaves") || name.includes("sapling")) {
    return "foliage";
  }
  if (name.includes("dirt") || name.includes("podzol") || name.includes("mycelium")) {
    return "dirt";
  }
  if (name.includes("ore")) {
    return "ore";
  }
  if (name.includes("redstone")) {
    return "redstone";
  }
  if (name.includes("stone") || name.includes("slate") || name.includes("tuff")) {
    return "stone";
  }
  if (name.includes("log") || name.includes("planks") || name.includes("wood")) {
    return "wood";
  }
  if (name.includes("terracotta")) {
    return "terracotta";
  }
  return "misc";
}

async function imageRef(label: string, imagePath: string | null, missingCommand: string | null): Promise<TextureImageRef> {
  return {
    label,
    path: imagePath,
    exists: imagePath ? await fileExists(imagePath) : false,
    missingCommand,
  };
}

async function fileExists(filePath: string): Promise<boolean> {
  try {
    const stat = await fs.stat(filePath);
    return stat.isFile();
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return false;
    }
    throw error;
  }
}

async function listFiles(root: string): Promise<string[]> {
  const entries = await fs.readdir(root, { withFileTypes: true });
  const files = await Promise.all(
    entries.map(async (entry) => {
      const child = path.join(root, entry.name);
      if (entry.isDirectory()) {
        return listFiles(child);
      }
      return entry.isFile() ? [child] : [];
    }),
  );
  return files.flat();
}

function displayNameFrom(name: string): string {
  return name
    .split(/[_/]/u)
    .map((part) => (part.length > 0 ? `${part[0]!.toUpperCase()}${part.slice(1)}` : part))
    .join(" ");
}

function normalizePath(filePath: string): string {
  return filePath.split(path.sep).join("/");
}

function emptyCoverage(referenceRoot: string | null): VanillaCoverageIndex {
  return {
    referenceRoot,
    summary: {
      vanillaTextureCount: 0,
      coveredTextureCount: 0,
      missingTextureCount: 0,
      placeholderTextureCount: 0,
      candidateTextureCount: 0,
      frozenTextureCount: 0,
    },
    entries: [],
  };
}
