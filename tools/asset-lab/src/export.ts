import fs from "node:fs/promises";
import path from "node:path";
import { loadFigureJsonDocument } from "./load";

interface ExportArgs {
  input: string;
  outDir: string | undefined;
}

const args = parseArgs(process.argv.slice(2));
const document = await loadFigureJsonDocument(args.input);
const asset = document.asset;
const outDir = args.outDir ?? path.join("/tmp", "mclone-asset-lab", asset.name);
await fs.mkdir(outDir, { recursive: true });

const jsonPath = path.join(outDir, "figure.json");
await fs.writeFile(jsonPath, document.json, "utf8");
console.log(`Wrote ${jsonPath}`);

function parseArgs(argv: string[]): ExportArgs {
  const input = argv[0];
  if (!input || input.startsWith("-")) {
    throw new Error("Usage: tsx src/export.ts <figure.ts|figure.json> [--out <dir>]");
  }

  let outDir: string | undefined;
  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--out") {
      outDir = argv[index + 1];
      index += 1;
    } else {
      throw new Error(`Unknown argument '${arg}'`);
    }
  }

  return { input, outDir };
}
