import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { FIRST_PARTY_STRUCTURES } from "./first-party-structures";
import { repositoryRoot } from "./paths";

const execFileAsync = promisify(execFile);
const outputRoot = path.join(repositoryRoot, "assets", "mclone", "structure-previews");
const packRoot = path.join(
  repositoryRoot,
  "generated-assets",
  "first-party-stage",
  "first-party-packs",
);

async function main(): Promise<void> {
  const mode = process.argv[2];
  if (mode !== "--check" && mode !== "--write") {
    throw new Error("Usage: sync-previews.ts --check|--write");
  }
  await assertPackRoot();
  const temporaryRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mclone-structure-previews-"));
  try {
    const structurePaths = FIRST_PARTY_STRUCTURES.map((entry) => entry.outputPath);
    await execFileAsync(
      "cargo",
      [
        "run",
        "--quiet",
        "--manifest-path",
        "native/Cargo.toml",
        "-p",
        "mclone-structure-compiler",
        "--",
        "--asset-pack-root",
        packRoot,
        "--out-root",
        temporaryRoot,
        ...structurePaths,
      ],
      { cwd: repositoryRoot, maxBuffer: 16 * 1024 * 1024 },
    );
    const expected = await listFiles(temporaryRoot);
    const current = await listFiles(outputRoot);
    const stale: string[] = [];
    for (const relativePath of expected) {
      const expectedBytes = await fs.readFile(path.join(temporaryRoot, relativePath));
      const currentBytes = await fs.readFile(path.join(outputRoot, relativePath)).catch(() => null);
      if (currentBytes === null || !expectedBytes.equals(currentBytes)) {
        stale.push(relativePath);
      }
    }
    const orphaned = current.filter((relativePath) => !expected.includes(relativePath));
    if (mode === "--check") {
      if (stale.length > 0 || orphaned.length > 0) {
        throw new Error([
          stale.length > 0 ? `Stale/missing previews:\n${stale.map(indent).join("\n")}` : "",
          orphaned.length > 0 ? `Orphaned previews:\n${orphaned.map(indent).join("\n")}` : "",
          "Run `pnpm structure-lab:previews:write` after changing structures or first-party visuals.",
        ].filter(Boolean).join("\n\n"));
      }
      console.log(`Current ${expected.length} checked Structure Lab preview artifacts`);
      return;
    }
    for (const relativePath of expected) {
      const destination = path.join(outputRoot, relativePath);
      await fs.mkdir(path.dirname(destination), { recursive: true });
      await fs.copyFile(path.join(temporaryRoot, relativePath), destination);
    }
    for (const relativePath of orphaned) {
      await fs.rm(path.join(outputRoot, relativePath));
    }
    console.log(`Wrote ${expected.length} checked Structure Lab preview artifacts`);
  } finally {
    await fs.rm(temporaryRoot, { recursive: true, force: true });
  }
}

async function assertPackRoot(): Promise<void> {
  for (const file of ["mclone-authored.pbp", "mclone-generated-fallback.pbp"]) {
    await fs.access(path.join(packRoot, file)).catch(() => {
      throw new Error(
        `Missing first-party asset pack '${file}'. Run \`pnpm assets:pack:first-party\` first.`,
      );
    });
  }
}

async function listFiles(root: string): Promise<string[]> {
  const result: string[] = [];
  async function visit(relativeDirectory: string): Promise<void> {
    const directory = path.join(root, relativeDirectory);
    const entries = await fs.readdir(directory, { withFileTypes: true }).catch((error: unknown) => {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") {
        return [];
      }
      throw error;
    });
    for (const entry of entries) {
      const relativePath = path.posix.join(relativeDirectory, entry.name);
      if (entry.isDirectory()) {
        await visit(relativePath);
      } else if (entry.isFile()) {
        result.push(relativePath);
      }
    }
  }
  await visit("");
  return result.sort();
}

function indent(value: string): string {
  return `  ${value}`;
}

await main();
