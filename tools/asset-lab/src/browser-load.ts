import type { FigureAsset } from "./dsl";
import {
  figureAssetFromUnknown,
  parseFigureAssetJson,
  roundTripFigureAsset,
} from "./figure-json";

export async function loadBrowserFigure(figurePath: string): Promise<FigureAsset> {
  if (isJsonPath(figurePath)) {
    const response = await fetch(figurePath);
    if (!response.ok) {
      throw new Error(`Could not load figure JSON '${figurePath}': HTTP ${response.status}`);
    }
    return parseFigureAssetJson(await response.text(), figurePath);
  }

  const module = (await import(/* @vite-ignore */ figurePath)) as { default?: unknown; asset?: unknown };
  const authoredAsset = figureAssetFromUnknown(module.default ?? module.asset, figurePath);
  return roundTripFigureAsset(authoredAsset, figurePath).asset;
}

function isJsonPath(figurePath: string): boolean {
  return /\.json(?:[?#]|$)/i.test(figurePath);
}
