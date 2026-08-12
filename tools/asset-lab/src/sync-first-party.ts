import fs from "node:fs/promises";
import path from "node:path";
import { assertCanonicalFigure } from "./dsl";
import { FIRST_PARTY_SEMANTIC_ASSETS } from "./first-party-figures";
import { loadFigureJsonDocument } from "./load";
import { repositoryRoot } from "./vite-figure-path";

const write = parseArgs(process.argv.slice(2));
await assertNoUnownedOutputs();

const stale: string[] = [];
for (const semanticAsset of FIRST_PARTY_SEMANTIC_ASSETS) {
  const document = await loadFigureJsonDocument(semanticAsset.sourcePath);
  assertCanonicalFigure(document.asset);
  if (document.asset.name !== semanticAsset.name) {
    throw new Error(
      `First-party source '${relative(semanticAsset.sourcePath)}' exports '${document.asset.name}', expected '${semanticAsset.name}'`,
    );
  }

  const current = await readIfPresent(semanticAsset.outputPath);
  if (current === document.json) {
    console.log(`Current ${relative(semanticAsset.outputPath)}`);
    continue;
  }

  if (!write) {
    stale.push(relative(semanticAsset.outputPath));
    continue;
  }

  await fs.mkdir(path.dirname(semanticAsset.outputPath), { recursive: true });
  await fs.writeFile(semanticAsset.outputPath, document.json, "utf8");
  console.log(`Wrote ${relative(semanticAsset.outputPath)} from ${relative(semanticAsset.sourcePath)}`);
}

if (stale.length > 0) {
  throw new Error(
    `Generated first-party figure JSON is stale or missing:\n${stale.map((entry) => `- ${entry}`).join("\n")}\n`
      + "Run 'pnpm asset-lab:figures:write' and review the generated diff.",
  );
}

function parseArgs(argv: string[]): boolean {
  if (argv.length === 0 || (argv.length === 1 && argv[0] === "--check")) {
    return false;
  }
  if (argv.length === 1 && argv[0] === "--write") {
    return true;
  }
  throw new Error("Usage: tsx src/sync-first-party.ts [--check|--write]");
}

async function assertNoUnownedOutputs(): Promise<void> {
  const outputDir = path.join(repositoryRoot, "assets", "mclone", "figures");
  const owned = new Set(FIRST_PARTY_SEMANTIC_ASSETS.map((asset) => path.basename(asset.outputPath)));
  const entries = await fs.readdir(outputDir, { withFileTypes: true });
  const unowned = entries
    .filter((entry) => entry.isFile() && entry.name.endsWith(".figure.json") && !owned.has(entry.name))
    .map((entry) => relative(path.join(outputDir, entry.name)));
  if (unowned.length > 0) {
    throw new Error(
      `Promoted semantic JSON has no declared TypeScript source:\n${unowned.map((entry) => `- ${entry}`).join("\n")}`,
    );
  }
}

async function readIfPresent(filePath: string): Promise<string | undefined> {
  try {
    return await fs.readFile(filePath, "utf8");
  } catch (error) {
    if (isNodeError(error) && error.code === "ENOENT") {
      return undefined;
    }
    throw error;
  }
}

function relative(filePath: string): string {
  return path.relative(repositoryRoot, filePath).replaceAll(path.sep, "/");
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
  return error instanceof Error && "code" in error;
}
