import fs from "node:fs/promises";
import type { Dirent } from "node:fs";
import path from "node:path";
import { assetLabRoot } from "./vite-figure-path";

export async function discoverCanonicalFigureSources(root = assetLabRoot): Promise<string[]> {
  return discoverSourcesIn(path.join(root, "examples"));
}

export async function discoverCanonicalPropSources(root = assetLabRoot): Promise<string[]> {
  return discoverSourcesIn(path.join(root, "props"), true);
}

async function discoverSourcesIn(directory: string, allowMissing = false): Promise<string[]> {
  let entries: Dirent[];
  try {
    entries = await fs.readdir(directory, { withFileTypes: true });
  } catch (error) {
    if (allowMissing && isNodeError(error) && error.code === "ENOENT") {
      return [];
    }
    throw error;
  }
  const sources: string[] = [];
  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    const sourcePath = path.join(directory, entry.name, "figure.ts");
    try {
      await fs.access(sourcePath);
      sources.push(sourcePath);
    } catch {
      // A neighboring notes or fixture directory is not a catalogue entry.
    }
  }
  return sources.sort((left, right) => left.localeCompare(right));
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
  return error instanceof Error && "code" in error;
}
