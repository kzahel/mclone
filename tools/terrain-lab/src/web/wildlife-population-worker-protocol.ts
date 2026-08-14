export type WildlifeSpecies = "rabbit" | "deer" | "mallard" | "bee";

export interface WildlifePopulationCellReceipt {
  cellX: number;
  cellZ: number;
  centerX: number;
  centerZ: number;
  surfaceY: number;
  biome: string;
  landform: string;
  land: number;
  productivity: number;
  openness: number;
  forestCover: number;
  wetland: number;
  water: number;
  rabbitWeight: number;
  deerWeight: number;
  mallardWeight: number;
  beeWeight: number;
  desiredDensity: number;
  occupancyRoll: number;
  speciesRoll: number;
  selectedX: number;
  selectedZ: number;
  species: WildlifeSpecies | null;
  groupSize: number;
  ownerChunkX: number | null;
  ownerChunkZ: number | null;
}

export interface WildlifePopulationSummary {
  schema: string;
  seed: string;
  revision: number;
  cellBlocks: number;
  minCellX: number;
  minCellZ: number;
  widthCells: number;
  depthCells: number;
  occupiedCells: number;
  animalCount: number;
  speciesCounts: [number, number, number, number];
  checksum: string;
  cells: WildlifePopulationCellReceipt[];
}

export interface WildlifePopulationWorkerBuild {
  type: "build";
  epoch: number;
  seed: string;
  centerX: number;
  centerZ: number;
  blocksAcross: number;
  aspectRatio: number;
}

export type WildlifePopulationWorkerRequest = WildlifePopulationWorkerBuild;

export interface WildlifePopulationWorkerSummary {
  type: "summary";
  epoch: number;
  buildMs: number;
  summary: WildlifePopulationSummary;
}

export interface WildlifePopulationWorkerError {
  type: "error";
  epoch: number;
  message: string;
}

export type WildlifePopulationWorkerResponse =
  | WildlifePopulationWorkerSummary
  | WildlifePopulationWorkerError;
