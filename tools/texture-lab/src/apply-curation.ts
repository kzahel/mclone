import { discoverTextureCandidates } from "./core/candidate-index";
import { applyTextureCuration } from "./core/curation";
import { loadTexturePack } from "./load";
import { textureLabOutputRoot } from "./output-root";

interface ApplyCurationArgs {
  input: string;
  outDir: string;
}

const args = parseArgs(process.argv.slice(2));
const pack = await loadTexturePack(args.input);
const candidates = await discoverTextureCandidates(args.outDir, { textureNames: Object.keys(pack.textures) });
const result = await applyTextureCuration({ outputRoot: args.outDir, pack, candidates: candidates.candidates });

console.log(`Applied ${result.applied.length} curated texture selection(s).`);
console.log(`Manifest: ${result.manifestPath}`);
for (const applied of result.applied) {
  console.log(`${applied.textureName}: ${applied.codename} -> ${applied.exportPath}`);
}

function parseArgs(argv: string[]): ApplyCurationArgs {
  const input = argv[0];
  if (!input || input.startsWith("-")) {
    throw new Error("Usage: tsx src/apply-curation.ts <texture.ts> [--out <dir>]");
  }

  let outDir = textureLabOutputRoot();
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
    throw new Error(`Unknown argument '${arg}'`);
  }

  return { input, outDir };
}
