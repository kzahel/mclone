const MAX_TERRAIN_CANVAS_DEVICE_PIXEL_RATIO = 2;
const MAX_TERRAIN_CANVAS_DIMENSION = 2_048;

export interface TerrainCanvasBackingSize {
  width: number;
  height: number;
}

export function terrainCanvasBackingSize(
  cssWidth: number,
  cssHeight: number,
  devicePixelRatio: number,
): TerrainCanvasBackingSize {
  const width = positiveFiniteOrOne(cssWidth);
  const height = positiveFiniteOrOne(cssHeight);
  const requestedScale = Math.min(
    positiveFiniteOrOne(devicePixelRatio),
    MAX_TERRAIN_CANVAS_DEVICE_PIXEL_RATIO,
  );
  const scale = Math.min(
    requestedScale,
    MAX_TERRAIN_CANVAS_DIMENSION / width,
    MAX_TERRAIN_CANVAS_DIMENSION / height,
  );
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
  };
}

function positiveFiniteOrOne(value: number): number {
  return Number.isFinite(value) && value > 0 ? value : 1;
}
