import fs from "node:fs/promises";
import path from "node:path";
import { assetLabRoot } from "./vite-figure-path";

export async function discoverCanonicalFigureSources(root = assetLabRoot): Promise<string[]> {
  const examplesDirectory = path.join(root, "examples");
  const entries = await fs.readdir(examplesDirectory, { withFileTypes: true });
  const sources: string[] = [];
  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    const sourcePath = path.join(examplesDirectory, entry.name, "figure.ts");
    try {
      await fs.access(sourcePath);
      sources.push(sourcePath);
    } catch {
      // A neighboring notes or fixture directory is not a catalogue entry.
    }
  }
  return sources.sort((left, right) => left.localeCompare(right));
}
