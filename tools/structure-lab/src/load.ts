import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import type { StructureAsset } from "./model";
import { repositoryRelative } from "./paths";
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

  const sourceBytes = await fs.readFile(absolutePath);
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
