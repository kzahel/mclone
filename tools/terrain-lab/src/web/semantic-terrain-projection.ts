import type { SemanticTerrainVerticalScale } from "../state";

export const SEMANTIC_TERRAIN_VERTICAL_DATUM = 64;

export function semanticTerrainVerticalExaggeration(
  scale: SemanticTerrainVerticalScale,
): number {
  switch (scale) {
    case "1x":
      return 1;
    case "8x":
      return 8;
    case "24x":
      return 24;
  }
}

export function semanticTerrainVerticalOffset(
  height: number,
  pitch: number,
  panelScale: number,
  blocksAcross: number,
  exaggeration: number,
): number {
  return (height - SEMANTIC_TERRAIN_VERTICAL_DATUM)
    / (Math.max(blocksAcross, 1) * 0.5)
    * panelScale
    * exaggeration
    * Math.cos(pitch);
}
