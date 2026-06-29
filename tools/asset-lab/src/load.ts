import { pathToFileURL } from "node:url";
import path from "node:path";
import { assertValidFigure, type FigureAsset } from "./dsl";

export async function loadFigureAsset(inputPath: string): Promise<FigureAsset> {
  const absolutePath = path.resolve(inputPath);
  const moduleUrl = pathToFileURL(absolutePath).href;
  const module = (await import(moduleUrl)) as { default?: unknown; asset?: unknown };
  const asset = module.default ?? module.asset;

  if (!isFigureAsset(asset)) {
    throw new Error(`Expected '${inputPath}' to export a FigureAsset as default`);
  }

  assertValidFigure(asset);
  return asset;
}

function isFigureAsset(value: unknown): value is FigureAsset {
  if (!value || typeof value !== "object") {
    return false;
  }
  const asset = value as Partial<FigureAsset>;
  return asset.schemaVersion === 1 && typeof asset.name === "string" && Array.isArray(asset.parts);
}
