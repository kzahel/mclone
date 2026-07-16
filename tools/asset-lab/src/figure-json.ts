import { assertValidFigure, type FigureAsset } from "./dsl";

export interface FigureJsonDocument {
  asset: FigureAsset;
  json: string;
}

export function serializeFigureAsset(asset: FigureAsset): string {
  assertValidFigure(asset);
  return `${JSON.stringify(asset, null, 2)}\n`;
}

export function parseFigureAssetJson(json: string, sourceLabel: string): FigureAsset {
  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`Could not parse figure JSON '${sourceLabel}': ${detail}`);
  }

  return figureAssetFromUnknown(value, sourceLabel);
}

export function roundTripFigureAsset(asset: FigureAsset, sourceLabel: string): FigureJsonDocument {
  const json = serializeFigureAsset(asset);
  return {
    asset: parseFigureAssetJson(json, `${sourceLabel} generated JSON`),
    json,
  };
}

export function figureAssetFromUnknown(value: unknown, sourceLabel: string): FigureAsset {
  if (!isFigureAssetShape(value)) {
    throw new Error(
      `Expected '${sourceLabel}' to contain a schema-v1 FigureAsset with materials, textures, parts, and clips`,
    );
  }

  try {
    assertValidFigure(value);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`Figure asset '${sourceLabel}' failed validation:\n${detail}`);
  }
  return value;
}

function isFigureAssetShape(value: unknown): value is FigureAsset {
  if (!isRecord(value)) {
    return false;
  }
  return value.schemaVersion === 1
    && typeof value.name === "string"
    && isRecord(value.materials)
    && isRecord(value.textures)
    && Array.isArray(value.parts)
    && isRecord(value.clips);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
