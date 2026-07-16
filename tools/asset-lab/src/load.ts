import fs from "node:fs/promises";
import { pathToFileURL } from "node:url";
import path from "node:path";
import type { FigureAsset } from "./dsl";
import {
  figureAssetFromUnknown,
  parseFigureAssetJson,
  roundTripFigureAsset,
  serializeFigureAsset,
  type FigureJsonDocument,
} from "./figure-json";

export async function loadFigureAsset(inputPath: string): Promise<FigureAsset> {
  return (await loadFigureJsonDocument(inputPath)).asset;
}

export async function loadFigureJsonDocument(inputPath: string): Promise<FigureJsonDocument> {
  const absolutePath = path.resolve(inputPath);
  if (path.extname(absolutePath).toLowerCase() === ".json") {
    const asset = parseFigureAssetJson(await fs.readFile(absolutePath, "utf8"), inputPath);
    return { asset, json: serializeFigureAsset(asset) };
  }

  const moduleUrl = pathToFileURL(absolutePath).href;
  const module = (await import(moduleUrl)) as { default?: unknown; asset?: unknown };
  const authoredAsset = figureAssetFromUnknown(module.default ?? module.asset, inputPath);
  return roundTripFigureAsset(authoredAsset, inputPath);
}
