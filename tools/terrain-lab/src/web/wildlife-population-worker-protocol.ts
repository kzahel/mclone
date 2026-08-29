import type { TerrainLabProfile } from "../state";

export type WildlifeSpecies =
  | "rabbit"
  | "deer"
  | "mallard"
  | "bee"
  | "squirrel"
  | "cow"
  | "chicken";

export interface WildlifePopulationCellReceipt {
  cellX: number;
  cellZ: number;
  centerX: number;
  centerZ: number;
  surfaceY: number;
  habitat: string;
  evidenceSource: string;
  evidenceStatus: "complete" | "partial" | "unavailable";
  availableEvidence: string[];
  supported: boolean;
  land: number;
  productivity: number;
  openness: number;
  lowCover: number;
  forestCover: number;
  forestEdge: number;
  wetland: number;
  water: number;
  inlandWater: number;
  shore: number;
  bank: number;
  flowering: number;
  seedsAndSoftMast: number;
  matureTrees: number;
  rabbitWeight: number;
  deerWeight: number;
  mallardWeight: number;
  beeWeight: number;
  squirrelWeight: number;
  cowWeight: number;
  chickenWeight: number;
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
  profile: TerrainLabProfile;
  adapter: string;
  adapterStatus: "available" | "requires-published-blocks";
  revision: number;
  cellBlocks: number;
  minCellX: number;
  minCellZ: number;
  widthCells: number;
  depthCells: number;
  occupiedCells: number;
  animalCount: number;
  speciesCounts: [number, number, number, number, number, number, number];
  checksum: string;
  cells: WildlifePopulationCellReceipt[];
}

export interface WildlifePopulationWorkerBuild {
  type: "build";
  epoch: number;
  seed: string;
  profile: TerrainLabProfile;
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
