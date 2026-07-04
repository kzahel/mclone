import type {
  TextureCatalogRotation,
  TextureCatalogStatus,
  TextureCatalogTiling,
  TextureSourceCategory,
} from "../dsl";

export const TEXTURE_LAB_INDEX_SCHEMA_VERSION = 1;

export type TextureRole = "all" | "top" | "bottom" | "side" | "overlay" | "particle" | "face" | "unknown";

export interface TextureLabIndex {
  schemaVersion: typeof TEXTURE_LAB_INDEX_SCHEMA_VERSION;
  generatedAt: string;
  outputRoot: string;
  pack: TexturePackSummary;
  summary: TextureLabIndexSummary;
  textures: TextureIndexEntry[];
  candidates: TextureCandidateEntry[];
  blocks: BlockIndexEntry[];
  warnings: string[];
}

export interface TexturePackSummary {
  name: string;
  inputPath: string;
  defaultSize: number;
  textureCount: number;
  blockCount: number;
}

export interface TextureLabIndexSummary {
  authoredTextures: number;
  tintableTextures: number;
  currentExportsPresent: number;
  sheetsPresent: number;
  runtimeExportsPresent: number;
  candidateCount: number;
  associatedCandidateCount: number;
  archivedCandidateCount: number;
}

export interface TextureIndexEntry {
  name: string;
  displayName: string;
  source: TextureSourceCategory;
  size: number;
  palette: string;
  base: string;
  tintRole: string | null;
  materialFamily: string;
  exportPath: string;
  runtimeCompatPath: string | null;
  status: TextureCatalogStatus;
  tiling: TextureCatalogTiling;
  rotation: TextureCatalogRotation;
  tags: string[];
  notes: string[];
  authoringRoles: string[];
  blockUsages: TextureBlockUsage[];
  images: {
    currentExport: TextureImageRef;
    runtimeExport: TextureImageRef;
    minecraftReference: TextureImageRef;
    sheet: TextureImageRef;
  };
}

export interface TextureBlockUsage {
  blockName: string;
  role: TextureRole;
  face: string;
}

export interface TextureImageRef {
  label: string;
  path: string | null;
  exists: boolean;
  missingCommand: string | null;
}

export type TextureCandidateSource = "diffusion" | "projection" | "archive";

export interface TextureCandidateEntry {
  id: string;
  candidateId: string;
  codename: string;
  source: TextureCandidateSource;
  textureName: string | null;
  artifactRoot: string;
  manifestPath: string | null;
  projectionReportPath: string | null;
  archivePath: string | null;
  archived: boolean;
  promptPreset: string | null;
  prompt: string | null;
  negativePrompt: string | null;
  modelId: string | null;
  scheduler: string | null;
  steps: number | null;
  seed: number | null;
  strength: number | null;
  resolution: number | null;
  resolutions: number[];
  paletteColors: string[];
  status: string | null;
  score: number | null;
  reasons: string[];
  images: {
    raw: TextureImageRef;
    rawTile: TextureImageRef;
    projected: TextureImageRef;
    contactSheet: TextureImageRef;
    reviewSheet: TextureImageRef;
  };
}

export interface BlockIndexEntry {
  name: string;
  kind: "cube";
  faces: BlockFaceIndexEntry[];
  sheet: TextureImageRef;
}

export interface BlockFaceIndexEntry {
  face: string;
  role: TextureRole;
  textureName: string;
}
