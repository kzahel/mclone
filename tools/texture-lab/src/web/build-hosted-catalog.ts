import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import type {
  TextureImageRef,
  TextureIndexEntry,
  TextureLabIndex,
} from "../core/index-model";
import { buildTextureLabIndex } from "../core/texture-index";
import { parseHexColor, tintTexture } from "../image";
import { repoRoot, textureLabOutputRoot } from "../output-root";
import { decodePng, encodePng } from "../png";

const projectRoot = repoRoot();
const textureLabRoot = path.join(projectRoot, "tools", "texture-lab");
const outputRoot = textureLabOutputRoot();
const distributionRoot = path.join(textureLabRoot, "dist", "web");
const mediaRoot = path.join(distributionRoot, "media");
const catalogRoot = path.join(distributionRoot, "catalog");
const packInput = path.join(textureLabRoot, "packs", "mclone-default", "texture.ts");
const safeSourceRoots = [
  outputRoot,
  path.join(textureLabRoot, "assets", "frozen"),
];
const stagedBySource = new Map<string, string>();

await fs.mkdir(mediaRoot, { recursive: true });
await fs.mkdir(catalogRoot, { recursive: true });

const index = await buildTextureLabIndex({
  inputPath: packInput,
  outputRoot,
});
const hosted = await buildHostedIndex(index);
validateHostedIndex(hosted);
await fs.writeFile(
  path.join(catalogRoot, "index.json"),
  `${JSON.stringify(hosted)}\n`,
);

console.log(
  `Texture Lab hosted catalog: ${hosted.textures.length} textures, `
    + `${stagedBySource.size} first-party images`,
);

async function buildHostedIndex(index: TextureLabIndex): Promise<TextureLabIndex> {
  const textures = await Promise.all(index.textures.map(hostTexture));
  const candidates = index.candidates.map((candidate) => ({
      ...candidate,
      artifactRoot: "",
      manifestPath: null,
      projectionReportPath: null,
      archivePath: null,
      images: {
        raw: unavailableImage(candidate.images.raw),
        rawTile: unavailableImage(candidate.images.rawTile),
        projected: unavailableImage(candidate.images.projected),
        contactSheet: unavailableImage(candidate.images.contactSheet),
        reviewSheet: unavailableImage(candidate.images.reviewSheet),
      },
    }));
  const curationSelections = await Promise.all(
    index.curation.selections.map(async (selection) => ({
      ...selection,
      image: await hostImage(selection.image),
    })),
  );
  const vanillaEntries = await Promise.all(
    index.vanillaCoverage.entries.map(async (entry) => ({
      ...entry,
      minecraftReference: unavailableImage(entry.minecraftReference),
      authoredTextures: await Promise.all(
        entry.authoredTextures.map(async (authored) => ({
          ...authored,
          currentExport: await hostImage(authored.currentExport),
        })),
      ),
    })),
  );

  return {
    ...index,
    outputRoot: "",
    pack: {
      ...index.pack,
      inputPath: "packs/mclone-default/texture.ts",
    },
    curation: {
      ...index.curation,
      manifestPath: "",
      selections: curationSelections,
    },
    textures,
    candidates,
    blocks: index.blocks.map((block) => ({
      ...block,
      sheet: unavailableImage(block.sheet),
    })),
    vanillaCoverage: {
      ...index.vanillaCoverage,
      referenceRoot: null,
      entries: vanillaEntries,
    },
    warnings: [],
  };
}

async function hostTexture(texture: TextureIndexEntry): Promise<TextureIndexEntry> {
  const tints = texture.tint
    ? [texture.tint.normal, ...texture.tint.alternates]
    : [];
  return {
    ...texture,
    frozen: texture.frozen
      ? {
          ...texture.frozen,
          path: path.basename(texture.frozen.path),
        }
      : null,
    images: {
      currentExport: await hostImage(texture.images.currentExport, tints),
      runtimeExport: await hostImage(texture.images.runtimeExport, tints),
      minecraftReference: unavailableImage(texture.images.minecraftReference),
      provisional: await hostImage(texture.images.provisional, tints),
      curated: await hostImage(texture.images.curated, tints),
      sheet: unavailableImage(texture.images.sheet),
    },
  };
}

async function hostImage(
  ref: TextureImageRef,
  tints: readonly string[] = [],
): Promise<TextureImageRef> {
  if (!ref.exists || !ref.path || !isSafeFirstPartySource(ref.path)) {
    return unavailableImage(ref);
  }

  const sourcePath = path.resolve(ref.path);
  const publicUrl = await stageSourceImage(sourcePath);
  const tintUrls: Record<string, string> = {};
  if (path.extname(sourcePath).toLowerCase() === ".png") {
    for (const tint of tints) {
      tintUrls[tint] = await stageTintedImage(sourcePath, tint);
    }
  }

  return {
    label: ref.label,
    path: null,
    exists: true,
    missingCommand: null,
    publicUrl,
    ...(Object.keys(tintUrls).length ? { tintUrls } : {}),
  };
}

function unavailableImage(ref: TextureImageRef): TextureImageRef {
  return {
    label: ref.label,
    path: null,
    exists: false,
    missingCommand: null,
  };
}

function isSafeFirstPartySource(filePath: string): boolean {
  const resolved = path.resolve(filePath);
  return safeSourceRoots.some((root) => {
    const relative = path.relative(path.resolve(root), resolved);
    return relative !== "" && !relative.startsWith("..") && !path.isAbsolute(relative);
  });
}

async function stageSourceImage(sourcePath: string): Promise<string> {
  const existing = stagedBySource.get(sourcePath);
  if (existing) {
    return existing;
  }
  const data = await fs.readFile(sourcePath);
  const extension = safeExtension(sourcePath);
  const name = `${createHash("sha256").update(data).digest("hex").slice(0, 16)}${extension}`;
  const publicUrl = `/textures/media/${name}`;
  await fs.copyFile(sourcePath, path.join(mediaRoot, name));
  stagedBySource.set(sourcePath, publicUrl);
  return publicUrl;
}

async function stageTintedImage(sourcePath: string, tint: string): Promise<string> {
  const cacheKey = `${sourcePath}\0${tint}`;
  const existing = stagedBySource.get(cacheKey);
  if (existing) {
    return existing;
  }
  const source = decodePng(await fs.readFile(sourcePath));
  const data = encodePng(tintTexture(source, parseHexColor(tint)));
  const name = `${createHash("sha256").update(data).digest("hex").slice(0, 16)}.png`;
  const publicUrl = `/textures/media/${name}`;
  await fs.writeFile(path.join(mediaRoot, name), data);
  stagedBySource.set(cacheKey, publicUrl);
  return publicUrl;
}

function safeExtension(filePath: string): string {
  const extension = path.extname(filePath).toLowerCase();
  return [".png", ".jpg", ".jpeg", ".webp"].includes(extension)
    ? extension
    : ".bin";
}

function validateHostedIndex(index: TextureLabIndex): void {
  const serialized = JSON.stringify(index);
  if (serialized.includes(projectRoot) || serialized.includes(outputRoot)) {
    throw new Error("Texture Lab hosted catalog contains a local absolute path");
  }
  for (const texture of index.textures) {
    if (texture.images.minecraftReference.exists
        || texture.images.minecraftReference.publicUrl) {
      throw new Error(
        `Texture Lab hosted catalog exposed Minecraft reference ${texture.name}`,
      );
    }
  }
}
