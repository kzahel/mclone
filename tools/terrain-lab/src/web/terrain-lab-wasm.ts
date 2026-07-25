import initTerrainLab from "../../generated/pkg/mclone_terrain_lab";

let terrainLabInitialization:
  | ReturnType<typeof initTerrainLab>
  | undefined;

export function initializeTerrainLab(): ReturnType<typeof initTerrainLab> {
  terrainLabInitialization ??= initTerrainLab();
  return terrainLabInitialization;
}
