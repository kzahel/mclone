import { discoverTextureCandidates } from "./core/candidate-index";
import { createTextureFreezeRequest } from "./core/freeze-request";
import { loadTexturePack } from "./load";
import { textureLabOutputRoot } from "./output-root";

interface FreezeRequestArgs {
  input: string;
  outDir: string;
  textureName: string;
}

const args = parseArgs(process.argv.slice(2));
const pack = await loadTexturePack(args.input);
const candidates = await discoverTextureCandidates(args.outDir, { textureNames: Object.keys(pack.textures) });
const request = await createTextureFreezeRequest({
  outputRoot: args.outDir,
  pack,
  packInputPath: args.input,
  candidates: candidates.candidates,
  textureName: args.textureName,
});

console.log(`Wrote freeze request for ${request.texture.name}: ${request.path}`);
console.log(`Candidate: ${request.candidate.codename} (${request.candidate.freezeReadiness})`);

function parseArgs(argv: string[]): FreezeRequestArgs {
  const input = argv[0];
  if (!input || input.startsWith("-")) {
    throw new Error("Usage: tsx src/freeze-request.ts <texture.ts> --texture <name> [--out <dir>]");
  }

  let outDir = textureLabOutputRoot();
  let textureName: string | undefined;
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
    throw new Error(`Unknown argument '${arg}'`);
  }

  if (!textureName) {
    throw new Error("--texture is required");
  }

  return { input, outDir, textureName };
}
