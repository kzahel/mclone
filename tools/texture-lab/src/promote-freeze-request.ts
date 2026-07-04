import { promoteFrozenTextureFromFreezeRequest } from "./core/frozen-curation";
import { loadTexturePack } from "./load";

interface PromoteFreezeRequestArgs {
  input: string;
  requestPath: string;
  asset: string | undefined;
}

const args = parseArgs(process.argv.slice(2));
const pack = await loadTexturePack(args.input);
const promoteOptions: Parameters<typeof promoteFrozenTextureFromFreezeRequest>[0] = {
  pack,
  packInputPath: args.input,
  requestPath: args.requestPath,
};
if (args.asset) {
  promoteOptions.asset = args.asset;
}

const result = await promoteFrozenTextureFromFreezeRequest(promoteOptions);

console.log(`Promoted freeze request for ${result.textureName}: ${result.entry.asset}`);
console.log(`Wrote ${result.assetPath}`);
console.log(`Updated ${result.manifestPath}`);

function parseArgs(argv: string[]): PromoteFreezeRequestArgs {
  const input = argv[0];
  const requestPath = argv[1];
  if (!input || input.startsWith("-") || !requestPath || requestPath.startsWith("-")) {
    throw new Error(
      "Usage: tsx src/promote-freeze-request.ts <texture.ts> <freeze-request.json> [--asset <relative/png>]",
    );
  }

  let asset: string | undefined;
  for (let index = 2; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--") {
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

  return { input, requestPath, asset };
}
