import fs from "node:fs/promises";
import path from "node:path";
import { FIRST_PARTY_STRUCTURES } from "./first-party-structures";
import { loadStructureJsonDocument } from "./load";
import { repositoryRelative, repositoryRoot } from "./paths";

const write = parseArgs(process.argv.slice(2));
validateMappings();
await assertNoUnownedOutputs();

const stale: string[] = [];
for (const structure of FIRST_PARTY_STRUCTURES) {
  const document = await loadStructureJsonDocument(structure.sourcePath);
  if (document.asset.id !== structure.runtimeStructureId) {
    throw new Error(
      `First-party source '${repositoryRelative(structure.sourcePath)}' exports '${document.asset.id}', expected '${structure.runtimeStructureId}'`,
    );
  }
  const current = await readIfPresent(structure.outputPath);
  if (current === document.json) {
    console.log(`Current ${repositoryRelative(structure.outputPath)}`);
    continue;
  }
  if (!write) {
    stale.push(repositoryRelative(structure.outputPath));
    continue;
  }
  await fs.mkdir(path.dirname(structure.outputPath), { recursive: true });
  await fs.writeFile(structure.outputPath, document.json, "utf8");
  console.log(
    `Wrote ${repositoryRelative(structure.outputPath)} from ${repositoryRelative(structure.sourcePath)}`,
  );
}

if (stale.length > 0) {
  throw new Error(
    `Generated first-party structure JSON is stale or missing:\n${stale.map((entry) => `- ${entry}`).join("\n")}\n`
      + "Run 'pnpm structure-lab:structures:write' and review the source and generated diffs.",
  );
}

function validateMappings(): void {
  const sources = new Set<string>();
  const outputs = new Set<string>();
  const ids = new Set<string>();
  for (const structure of FIRST_PARTY_STRUCTURES) {
    for (const [set, value, label] of [
      [sources, path.resolve(structure.sourcePath), "source"],
      [outputs, path.resolve(structure.outputPath), "output"],
      [ids, structure.runtimeStructureId, "runtime id"],
    ] as const) {
      if (set.has(value)) {
        throw new Error(`Duplicate first-party structure ${label} '${value}'`);
      }
      set.add(value);
    }
  }
}

async function assertNoUnownedOutputs(): Promise<void> {
  const outputDir = path.join(repositoryRoot, "assets", "mclone", "structures");
  const owned = new Set(FIRST_PARTY_STRUCTURES.map((structure) => path.basename(structure.outputPath)));
  let entries: import("node:fs").Dirent[];
  try {
    entries = await fs.readdir(outputDir, { withFileTypes: true });
  } catch (error) {
    if (isNodeError(error) && error.code === "ENOENT") {
      return;
    }
    throw error;
  }
  const unowned = entries
    .filter((entry) => entry.isFile() && entry.name.endsWith(".structure.json") && !owned.has(entry.name))
    .map((entry) => repositoryRelative(path.join(outputDir, entry.name)));
  if (unowned.length > 0) {
    throw new Error(
      `Promoted structure JSON has no declared TypeScript source:\n${unowned.map((entry) => `- ${entry}`).join("\n")}`,
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

function parseArgs(argv: string[]): boolean {
  if (argv.length === 0 || (argv.length === 1 && argv[0] === "--check")) {
    return false;
  }
  if (argv.length === 1 && argv[0] === "--write") {
    return true;
  }
  throw new Error("Usage: tsx src/sync-first-party.ts [--check|--write]");
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
  return error instanceof Error && "code" in error;
}
