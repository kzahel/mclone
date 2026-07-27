export const SEMANTIC_TERRAIN_VERTICAL_DATUM = 64;
export const SEMANTIC_TERRAIN_VERTICAL_SPAN = 192;
export const SEMANTIC_TERRAIN_VERTICAL_DISPLAY_SCALE = 1.45;

export function semanticTerrainVerticalOffset(
  height: number,
  pitch: number,
  panelScale: number,
): number {
  return (height - SEMANTIC_TERRAIN_VERTICAL_DATUM)
    / SEMANTIC_TERRAIN_VERTICAL_SPAN
    * panelScale
    * SEMANTIC_TERRAIN_VERTICAL_DISPLAY_SCALE
    * Math.cos(pitch);
}
