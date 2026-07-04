import type {
  TintSourceNeutralitySpec,
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
  curation: TextureCurationState;
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
  proceduralPlaceholderCount: number;
  tintableTextures: number;
  currentExportsPresent: number;
  sheetsPresent: number;
  runtimeExportsPresent: number;
  candidateCount: number;
  associatedCandidateCount: number;
  archivedCandidateCount: number;
  curatedSelectionCount: number;
  frozenTextureCount: number;
}

export interface TextureIndexEntry {
  name: string;
  displayName: string;
  source: TextureSourceCategory;
  size: number;
  palette: string;
  base: string;
  tintRole: string | null;
  tint: TextureTintRef | null;
  materialFamily: string;
  exportPath: string;
  runtimeCompatPath: string | null;
  status: TextureCatalogStatus;
  artSource: TextureArtSource;
  tiling: TextureCatalogTiling;
  rotation: TextureCatalogRotation;
  tags: string[];
  notes: string[];
  frozen: TextureFrozenRef | null;
  authoringRoles: string[];
  blockUsages: TextureBlockUsage[];
  images: {
    currentExport: TextureImageRef;
    runtimeExport: TextureImageRef;
    minecraftReference: TextureImageRef;
    sheet: TextureImageRef;
  };
}

export type TextureArtSourceKind = "frozen" | "authored-structure" | "authored-baseline" | "procedural-placeholder";

export interface TextureArtSource {
  kind: TextureArtSourceKind;
  label: string;
  description: string;
}

export interface TextureBlockUsage {
  blockName: string;
  role: TextureRole;
  face: string;
}

export interface TextureTintRef {
  role: string;
  normal: string;
  alternates: string[];
  sourceNeutrality: TintSourceNeutralitySpec | null;
}

export interface TextureImageRef {
  label: string;
  path: string | null;
  exists: boolean;
  missingCommand: string | null;
}

export interface TextureFrozenRef {
  asset: string;
  path: string;
  sha256: string;
  codename: string | null;
  candidateId: string | null;
  sourceContextGitCommit: string | null;
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
  promotable: boolean;
  images: {
    raw: TextureImageRef;
    rawTile: TextureImageRef;
    projected: TextureImageRef;
    contactSheet: TextureImageRef;
    reviewSheet: TextureImageRef;
  };
}

export interface TextureCurationState {
  schemaVersion: 1;
  manifestPath: string;
  selectedCount: number;
  selections: TextureCurationSelectionEntry[];
  staleSelections: TextureCurationStaleSelectionEntry[];
}

export interface TextureCurationSelectionEntry {
  textureName: string;
  candidateId: string;
  codename: string;
  source: TextureCandidateSource;
  selectedAt: string;
  image: TextureImageRef;
}

export interface TextureCurationStaleSelectionEntry {
  textureName: string;
  candidateId: string;
  selectedAt: string;
  reason: string;
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
