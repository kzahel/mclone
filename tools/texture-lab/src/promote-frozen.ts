import { discoverTextureCandidates } from "./core/candidate-index";
import { buildTextureCurationState } from "./core/curation";
import { promoteFrozenTexture } from "./core/frozen-curation";
import type { TextureCandidateEntry } from "./core/index-model";
import type { TextureFrozenMetadata } from "./dsl";
import { loadTexturePack } from "./load";
import { textureLabOutputRoot } from "./output-root";

interface PromoteFrozenArgs {
  input: string;
  outDir: string;
  textureName: string;
  candidateRef: string | undefined;
  imagePath: string | undefined;
  asset: string | undefined;
}

const args = parseArgs(process.argv.slice(2));
const pack = await loadTexturePack(args.input);
const candidate = args.imagePath
  ? null
  : await selectCandidateFromArgs({
      outDir: args.outDir,
      textureName: args.textureName,
      candidateRef: args.candidateRef,
      textureNames: Object.keys(pack.textures),
    });
const imagePath = args.imagePath ?? candidate?.images.projected.path;
if (!imagePath) {
  throw new Error("No image to promote. Pass --image, --candidate, or select a pack candidate in the texture lab UI first.");
}

const promoteOptions: Parameters<typeof promoteFrozenTexture>[0] = {
  pack,
  packInputPath: args.input,
  textureName: args.textureName,
  imagePath,
};
if (args.asset) {
  promoteOptions.asset = args.asset;
}
if (candidate) {
  promoteOptions.metadata = metadataFromCandidate(candidate);
}

const result = await promoteFrozenTexture(promoteOptions);

console.log(`Promoted frozen texture ${result.textureName}: ${result.entry.asset}`);
console.log(`Wrote ${result.assetPath}`);
console.log(`Updated ${result.manifestPath}`);

async function selectCandidateFromArgs(options: {
  outDir: string;
  textureName: string;
  candidateRef: string | undefined;
  textureNames: string[];
}): Promise<TextureCandidateEntry> {
  const discovery = await discoverTextureCandidates(options.outDir, { textureNames: options.textureNames });
  if (options.candidateRef) {
    const matches = discovery.candidates.filter(
      (candidate) =>
        candidate.textureName === options.textureName &&
        (candidate.id === options.candidateRef ||
          candidate.candidateId === options.candidateRef ||
          candidate.codename === options.candidateRef),
    );
    if (matches.length === 1) {
      return matches[0]!;
    }
    if (matches.length > 1) {
      throw new Error(`Candidate '${options.candidateRef}' is ambiguous for ${options.textureName}`);
    }
    throw new Error(`Candidate '${options.candidateRef}' not found for ${options.textureName}`);
  }

  const curation = await buildTextureCurationState(options.outDir, discovery.candidates);
  const selection = curation.selections.find((entry) => entry.textureName === options.textureName);
  if (!selection) {
    throw new Error(`Texture '${options.textureName}' has no selected pack candidate`);
  }
  const candidate = discovery.candidates.find((entry) => entry.id === selection.candidateId);
  if (!candidate) {
    throw new Error(`Selected candidate '${selection.candidateId}' is no longer indexed`);
  }
  return candidate;
}

function metadataFromCandidate(candidate: TextureCandidateEntry): TextureFrozenMetadata {
  const metadata: TextureFrozenMetadata = {
    codename: candidate.codename,
    candidateId: candidate.candidateId,
  };
  if (candidate.promptPreset) metadata.promptPreset = candidate.promptPreset;
  if (candidate.prompt) metadata.prompt = candidate.prompt;
  if (candidate.negativePrompt) metadata.negativePrompt = candidate.negativePrompt;
  if (candidate.modelId) metadata.modelId = candidate.modelId;
  if (candidate.scheduler) metadata.scheduler = candidate.scheduler;
  if (candidate.steps !== null) metadata.steps = candidate.steps;
  if (candidate.seed !== null) metadata.seed = candidate.seed;
  if (candidate.strength !== null) metadata.strength = candidate.strength;
  if (candidate.resolution !== null) metadata.resolution = candidate.resolution;
  if (candidate.archivePath) {
    metadata.sourceContext = { archiveManifest: candidate.archivePath };
  }
  return metadata;
}

function parseArgs(argv: string[]): PromoteFrozenArgs {
  const input = argv[0];
  if (!input || input.startsWith("-")) {
    throw new Error(
      "Usage: tsx src/promote-frozen.ts <texture.ts> --texture <name> [--candidate <id|codename>] [--image <png>] [--asset <relative/png>] [--out <dir>]",
    );
  }

  let outDir = textureLabOutputRoot();
  let textureName: string | undefined;
  let candidateRef: string | undefined;
  let imagePath: string | undefined;
  let asset: string | undefined;
  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--") {
      continue;
    }
    if (arg === "--out") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--out requires a directory");
      }
      outDir = next;
      index += 1;
      continue;
    }
    if (arg === "--texture") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--texture requires a texture name");
      }
      textureName = next;
      index += 1;
      continue;
    }
    if (arg === "--candidate") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--candidate requires an id or codename");
      }
      candidateRef = next;
      index += 1;
      continue;
    }
    if (arg === "--image") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--image requires a PNG path");
      }
      imagePath = next;
      index += 1;
      continue;
    }
    if (arg === "--asset") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--asset requires a relative PNG path");
      }
      asset = next;
      index += 1;
      continue;
    }
    throw new Error(`Unknown argument '${arg}'`);
  }

  if (!textureName) {
    throw new Error("--texture is required");
  }
  if (candidateRef && imagePath) {
    throw new Error("--candidate and --image are mutually exclusive");
  }

  return { input, outDir, textureName, candidateRef, imagePath, asset };
}
