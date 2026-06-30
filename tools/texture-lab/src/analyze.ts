import { analyzeTexture, formatComparison, type TextureFeatures } from "./analysis";
import { loadTexturePack } from "./load";
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
  candidate: TextureFeatures;
  reference?: TextureFeatures | undefined;
}

const results: AnalyzedTexture[] = [];
for (const texture of textures) {
  const reference = await loadReferenceTexture(texture.exportPath, {
    include: args.includeReference,
    referenceRoot: args.referenceRoot,
  });
  results.push({
    name: texture.name,
    candidate: analyzeTexture(texture),
    reference: reference ? analyzeTexture(reference) : undefined,
  });
}

if (args.json) {
  console.log(JSON.stringify(results, null, 2));
} else {
  for (const [index, result] of results.entries()) {
    if (index > 0) {
      console.log("");
    }
    console.log(formatComparison(result.name, result.candidate, result.reference));
  }
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
