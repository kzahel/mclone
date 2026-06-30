import fs from "node:fs/promises";
import path from "node:path";
import {
  analyzeTexture,
  compareCandidates,
  featureGaps,
  formatCandidateComparison,
  formatComparison,
  type TextureFeatures,
} from "./analysis";
import { areaDownsample } from "./image";
import { loadTexturePack } from "./load";
import { decodePng, type RgbaImage } from "./png";
import { loadReferenceTexture } from "./reference";
import { renderAllTextures } from "./render";

type AnalyzeArgs = PackArgs | CompareArgs;

interface CommonArgs {
  includeReference: boolean;
  referenceRoot: string | undefined;
  json: boolean;
}

interface PackArgs extends CommonArgs {
  mode: "pack";
  input: string;
  textures: string[];
}

interface CompareArgs extends CommonArgs {
  mode: "compare";
  a: string;
  b: string;
  referenceName: string | undefined;
  referencePng: string | undefined;
}

const args = parseArgs(process.argv.slice(2));
if (args.mode === "compare") {
  await runCompare(args);
} else {
  await runPack(args);
}

async function runPack(args: PackArgs): Promise<void> {
  const pack = await loadTexturePack(args.input);
  const textures = renderAllTextures(pack).filter(
    (texture) => args.textures.length === 0 || args.textures.includes(texture.name),
  );

  if (textures.length === 0) {
    throw new Error(`No textures matched ${JSON.stringify(args.textures)} in pack '${pack.name}'`);
  }

  const results: AnalyzedTexture[] = [];
  for (const texture of textures) {
    const reference = await loadReferenceTexture(texture.exportPath, {
      include: args.includeReference,
      referenceRoot: args.referenceRoot,
    });
    // Bring both tiles onto a shared grid (the smaller of the two, usually the
    // 16x16 vanilla size) before measuring, so scale-sensitive features compare
    // apples-to-apples instead of rewarding the candidate for having more pixels.
    const matched = matchResolution([texture, reference]);
    results.push({
      name: texture.name,
      candidateNative: `${texture.width}x${texture.height}`,
      referenceNative: reference ? `${reference.width}x${reference.height}` : undefined,
      comparedAt: `${matched.width}x${matched.height}`,
      candidate: analyzeTexture(matched.images[0]!),
      reference: matched.images[1] ? analyzeTexture(matched.images[1]) : undefined,
    });
  }

  if (args.json) {
    const enriched = results.map((result) => ({
      ...result,
      gaps: result.reference ? featureGaps(result.candidate, result.reference) : [],
    }));
    console.log(JSON.stringify(enriched, null, 2));
    return;
  }
  for (const [index, result] of results.entries()) {
    if (index > 0) {
      console.log("");
    }
    console.log(
      formatComparison(result.name, result.candidate, result.reference, {
        candidateNative: result.candidateNative,
        referenceNative: result.referenceNative,
        comparedAt: result.comparedAt,
      }),
    );
  }
}

async function runCompare(args: CompareArgs): Promise<void> {
  const a = await loadImage(args.a);
  const b = await loadImage(args.b);
  const reference = await resolveCompareReference(args);

  // Same apples-to-apples rule as pack mode: drop every tile to the smallest
  // shared grid before measuring, so two candidates (and vanilla) are judged at
  // one resolution regardless of whether they were authored at 16x16 or 32x32.
  const matched = matchResolution([a, b, reference]);
  const featuresA = analyzeTexture(matched.images[0]!);
  const featuresB = analyzeTexture(matched.images[1]!);
  const featuresRef = matched.images[2] ? analyzeTexture(matched.images[2]) : undefined;
  const comparison = compareCandidates(featuresA, featuresB, featuresRef);

  if (args.json) {
    console.log(
      JSON.stringify(
        {
          a: args.a,
          b: args.b,
          referenceName: args.referenceName ?? (reference ? basenameNoExt(args.a) : undefined),
          comparedAt: `${matched.width}x${matched.height}`,
          comparison,
        },
        null,
        2,
      ),
    );
    return;
  }
  console.log(
    formatCandidateComparison(args.a, args.b, comparison, {
      referenceName: args.referenceName ?? (reference ? basenameNoExt(args.a) : undefined),
      referenceNative: reference ? `${reference.width}x${reference.height}` : undefined,
      comparedAt: `${matched.width}x${matched.height}`,
    }),
  );
}

interface AnalyzedTexture {
  name: string;
  candidateNative: string;
  referenceNative?: string | undefined;
  comparedAt: string;
  candidate: TextureFeatures;
  reference?: TextureFeatures | undefined;
}

interface MatchedResolution {
  images: (RgbaImage | undefined)[];
  width: number;
  height: number;
}

// Downsample every present image to the smallest width/height among them, so all
// features are computed on one shared grid. Absent (undefined) inputs stay
// undefined and keep their slot, so callers can index by position.
function matchResolution(images: (RgbaImage | undefined)[]): MatchedResolution {
  const present = images.filter((image): image is RgbaImage => Boolean(image));
  if (present.length === 0) {
    return { images, width: 0, height: 0 };
  }
  const width = Math.min(...present.map((image) => image.width));
  const height = Math.min(...present.map((image) => image.height));
  return {
    images: images.map((image) => (image ? areaDownsample(image, width, height) : undefined)),
    width,
    height,
  };
}

async function loadImage(file: string): Promise<RgbaImage> {
  try {
    return decodePng(await fs.readFile(file));
  } catch (error) {
    throw new Error(`Failed to load image '${file}': ${(error as Error).message}`);
  }
}

// Resolve the vanilla counterpart to rank the two candidates against. Explicit
// flags win; otherwise infer the block name from candidate A's filename (e.g.
// stone.png -> vanilla block/stone.png). Returns undefined when nothing matches,
// in which case compare mode falls back to a plain a-vs-b diff.
async function resolveCompareReference(args: CompareArgs): Promise<RgbaImage | undefined> {
  if (!args.includeReference) {
    return undefined;
  }
  if (args.referencePng) {
    return loadImage(args.referencePng);
  }
  const name = args.referenceName ?? basenameNoExt(args.a);
  const exportPath = `assets/mclone/textures/block/${name.replace(/^block\//, "").replace(/\.png$/i, "")}.png`;
  return loadReferenceTexture(exportPath, { referenceRoot: args.referenceRoot });
}

function basenameNoExt(file: string): string {
  return path.basename(file).replace(/\.png$/i, "");
}

function parseArgs(argv: string[]): AnalyzeArgs {
  const usage =
    "Usage:\n" +
    "  analyze <texture.ts> [--texture <name>]... [--reference-root <dir>] [--no-reference] [--json]\n" +
    "  analyze --compare <a.png> <b.png> [--reference-name <block> | --reference-png <path>] [--reference-root <dir>] [--no-reference] [--json]";

  let includeReference = true;
  let referenceRoot: string | undefined;
  let json = false;
  let referenceName: string | undefined;
  let referencePng: string | undefined;
  const compare: string[] = [];
  const positional: string[] = [];
  const textures: string[] = [];
  let isCompare = false;

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]!;
    if (arg === "--") {
      continue;
    } else if (arg === "--compare") {
      isCompare = true;
      const a = argv[index + 1];
      const b = argv[index + 2];
      if (!a || !b || a.startsWith("-") || b.startsWith("-")) {
        throw new Error("--compare requires two image paths");
      }
      compare.push(a, b);
      index += 2;
    } else if (arg === "--texture") {
      textures.push(requireValue(argv, index, "--texture"));
      index += 1;
    } else if (arg === "--reference-root") {
      referenceRoot = requireValue(argv, index, "--reference-root");
      index += 1;
    } else if (arg === "--reference-name") {
      referenceName = requireValue(argv, index, "--reference-name");
      index += 1;
    } else if (arg === "--reference-png") {
      referencePng = requireValue(argv, index, "--reference-png");
      index += 1;
    } else if (arg === "--no-reference") {
      includeReference = false;
    } else if (arg === "--json") {
      json = true;
    } else if (arg.startsWith("-")) {
      throw new Error(`Unknown argument '${arg}'\n${usage}`);
    } else {
      positional.push(arg);
    }
  }

  if (isCompare) {
    return {
      mode: "compare",
      a: compare[0]!,
      b: compare[1]!,
      referenceName,
      referencePng,
      includeReference,
      referenceRoot,
      json,
    };
  }
  const input = positional[0];
  if (!input) {
    throw new Error(usage);
  }
  return { mode: "pack", input, textures, includeReference, referenceRoot, json };
}

function requireValue(argv: string[], index: number, flag: string): string {
  const next = argv[index + 1];
  if (!next) {
    throw new Error(`${flag} requires a value`);
  }
  return next;
}
