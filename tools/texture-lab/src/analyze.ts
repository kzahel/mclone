import { analyzeTexture, featureGaps, formatComparison, type TextureFeatures } from "./analysis";
import { areaDownsample } from "./image";
import { loadTexturePack } from "./load";
import type { RgbaImage } from "./png";
import { loadReferenceTexture } from "./reference";
import { renderAllTextures } from "./render";

interface AnalyzeArgs {
  input: string;
  textures: string[];
  includeReference: boolean;
  referenceRoot: string | undefined;
  json: boolean;
}

const args = parseArgs(process.argv.slice(2));
const pack = await loadTexturePack(args.input);
const textures = renderAllTextures(pack).filter(
  (texture) => args.textures.length === 0 || args.textures.includes(texture.name),
);

if (textures.length === 0) {
  throw new Error(`No textures matched ${JSON.stringify(args.textures)} in pack '${pack.name}'`);
}

interface AnalyzedTexture {
  name: string;
  candidateNative: string;
  referenceNative?: string | undefined;
  comparedAt: string;
  candidate: TextureFeatures;
  reference?: TextureFeatures | undefined;
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
  const matched = matchResolution(texture, reference);
  results.push({
    name: texture.name,
    candidateNative: `${texture.width}x${texture.height}`,
    referenceNative: reference ? `${reference.width}x${reference.height}` : undefined,
    comparedAt: `${matched.width}x${matched.height}`,
    candidate: analyzeTexture(matched.candidate),
    reference: matched.reference ? analyzeTexture(matched.reference) : undefined,
  });
}

if (args.json) {
  const enriched = results.map((result) => ({
    ...result,
    gaps: result.reference ? featureGaps(result.candidate, result.reference) : [],
  }));
  console.log(JSON.stringify(enriched, null, 2));
} else {
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

interface MatchedResolution {
  candidate: RgbaImage;
  reference: RgbaImage | undefined;
  width: number;
  height: number;
}

function matchResolution(candidate: RgbaImage, reference: RgbaImage | undefined): MatchedResolution {
  if (!reference) {
    return { candidate, reference: undefined, width: candidate.width, height: candidate.height };
  }
  const width = Math.min(candidate.width, reference.width);
  const height = Math.min(candidate.height, reference.height);
  return {
    candidate: areaDownsample(candidate, width, height),
    reference: areaDownsample(reference, width, height),
    width,
    height,
  };
}

function parseArgs(argv: string[]): AnalyzeArgs {
  const input = argv[0];
  if (!input || input.startsWith("-")) {
    throw new Error(
      "Usage: tsx src/analyze.ts <texture.ts> [--texture <name>]... [--reference-root <dir>] [--no-reference] [--json]",
    );
  }
  const textures: string[] = [];
  let includeReference = true;
  let referenceRoot: string | undefined;
  let json = false;
  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--") {
      continue;
    } else if (arg === "--texture") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--texture requires a name");
      }
      textures.push(next);
      index += 1;
    } else if (arg === "--reference-root") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--reference-root requires a directory");
      }
      referenceRoot = next;
      index += 1;
    } else if (arg === "--no-reference") {
      includeReference = false;
    } else if (arg === "--json") {
      json = true;
    } else {
      throw new Error(`Unknown argument '${arg}'`);
    }
  }
  return { input, textures, includeReference, referenceRoot, json };
}
