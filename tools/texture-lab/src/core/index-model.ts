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
