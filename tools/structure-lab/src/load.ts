import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import ts from "typescript";
import type { StructureAsset } from "./model";
import { repositoryRelative, structureLabRoot } from "./paths";
import {
  attachStructureProvenance,
  authoredStructureFromUnknown,
  parseStructureAssetJson,
  serializeStructureAsset,
  type StructureJsonDocument,
} from "./structure-json";

export async function loadStructureAsset(inputPath: string): Promise<StructureAsset> {
  return (await loadStructureJsonDocument(inputPath)).asset;
}

export async function loadStructureJsonDocument(inputPath: string): Promise<StructureJsonDocument> {
  const absolutePath = path.resolve(inputPath);
  if (path.extname(absolutePath).toLowerCase() === ".json") {
    const asset = parseStructureAssetJson(await fs.readFile(absolutePath, "utf8"), inputPath);
    return { asset, json: serializeStructureAsset(asset) };
  }

  const sourceBytes = await readLocalSourceGraph(absolutePath);
  const moduleUrl = pathToFileURL(absolutePath).href;
  const module = (await import(moduleUrl)) as { default?: unknown; structure?: unknown };
  const authored = authoredStructureFromUnknown(module.default ?? module.structure, inputPath);
  const generated = attachStructureProvenance(
    authored,
    repositoryRelative(absolutePath),
    sourceBytes,
  );
  const json = serializeStructureAsset(generated);
  const asset = parseStructureAssetJson(json, `${inputPath} generated JSON`);
  return { asset, json };
}

async function readLocalSourceGraph(entryPath: string): Promise<Uint8Array> {
  const sources = new Map<string, Uint8Array>();

  async function visit(sourcePath: string): Promise<void> {
    const absolutePath = path.resolve(sourcePath);
    const relativePath = repositoryRelative(absolutePath);
    if (sources.has(relativePath)) return;
    assertInsideStructureLab(absolutePath);
    const bytes = await fs.readFile(absolutePath);
    sources.set(relativePath, bytes);
    const imports = ts.preProcessFile(bytes.toString()).importedFiles
      .map((entry) => entry.fileName)
      .filter((specifier) => specifier.startsWith("."));
    for (const specifier of imports) {
      await visit(await resolveLocalTypeScriptImport(absolutePath, specifier));
    }
  }

  await visit(entryPath);
  const chunks: Uint8Array[] = [];
  for (const [relativePath, bytes] of [...sources].sort(([left], [right]) => left.localeCompare(right))) {
    chunks.push(Buffer.from(`${relativePath}\0${bytes.byteLength}\0`, "utf8"), bytes);
  }
  return Buffer.concat(chunks);
}

async function resolveLocalTypeScriptImport(importer: string, specifier: string): Promise<string> {
  const unresolved = path.resolve(path.dirname(importer), specifier);
  for (const candidate of [
    unresolved,
    `${unresolved}.ts`,
    `${unresolved}.tsx`,
    path.join(unresolved, "index.ts"),
    path.join(unresolved, "index.tsx"),
  ]) {
    try {
      if ((await fs.stat(candidate)).isFile()) return candidate;
    } catch {
      // Continue through the bounded TypeScript resolution candidates.
    }
  }
  throw new Error(`Could not resolve local import '${specifier}' from '${repositoryRelative(importer)}'`);
}

function assertInsideStructureLab(candidate: string): void {
  const relative = path.relative(structureLabRoot, candidate);
  if (relative === "" || relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`Canonical source graph leaves tools/structure-lab at '${candidate}'`);
  }
}
