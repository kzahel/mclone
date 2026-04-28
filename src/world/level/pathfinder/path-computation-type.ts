export const PathComputationType = {
  LAND: "land",
  WATER: "water",
  AIR: "air",
} as const;

export type PathComputationType = typeof PathComputationType[keyof typeof PathComputationType];
