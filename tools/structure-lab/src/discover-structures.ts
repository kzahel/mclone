import fs from "node:fs/promises";
import path from "node:path";
import { structureLabRoot } from "./paths";

export async function discoverCanonicalStructureSources(root = structureLabRoot): Promise<string[]> {
  const examplesDirectory = path.join(root, "examples");
  const entries = await fs.readdir(examplesDirectory, { withFileTypes: true });
  const sources: string[] = [];
  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    const sourcePath = path.join(examplesDirectory, entry.name, "structure.ts");
    try {
      await fs.access(sourcePath);
      sources.push(sourcePath);
    } catch {
      // Notes and fixture directories are not canonical catalogue entries.
    }
  }
  return sources.sort((left, right) => left.localeCompare(right));
}
