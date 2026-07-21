export interface StructureCatalogDocument {
  schemaVersion: 1;
  catalogSha256: string;
  structures: StructureCatalogEntry[];
  summary: {
    structures: number;
    families: number;
    blocks: number;
    runtimePromoted: number;
  };
}

export interface StructureCatalogEntry {
  schemaVersion: 1;
  compilerId: string;
  structureId: string;
  label: string;
  description: string;
  category: string;
  tags: string[];
  family?: { id: string; member: string; label: string };
  size: [number, number, number];
  authoredOperationCount: number;
  placedBlockCount: number;
  materialBill: Array<{ paletteKey: string; count: number }>;
  components: Array<{ id: string; label: string; optional: boolean; blockCount: number }>;
  markers: Array<{ kind: string; pos: [number, number, number] }>;
  sockets: Array<{ id: string; kind: string; pos: [number, number, number]; facing: string }>;
  defaultTheme: string;
  geometry: {
    vertexCount: number;
    indexCount: number;
    faceCount: number;
    boundsMin: [number, number, number];
    boundsMax: [number, number, number];
    solidFaceCount: number;
    cutoutFaceCount: number;
    translucentFaceCount: number;
    componentGroups: Array<{ id: string; faceCount: number }>;
    verticalLayers: Array<{ y: number; faceCount: number }>;
  };
  artifacts: {
    meshPath: string;
    meshSha256: string;
    atlasPath: string;
    atlasSha256: string;
    atlasWidth: number;
    atlasHeight: number;
    atlasSpriteCount: number;
  };
  source: {
    compilerId: string;
    path: string;
    sha256: string;
    semanticSha256: string;
  };
  lighting: { id: string; description: string };
  assetPacks: Array<{
    id: string;
    origin: "first_party" | "generated";
    contentFingerprint?: string;
    sha256: string;
  }>;
  assetProvenance: {
    firstParty: number;
    generated: number;
    minecraftReference: number;
    unknown: number;
    missing: number;
  };
  meshPath: string;
  atlasPath: string;
  receiptPath: string;
  thumbnailPath: string;
  runtimeStatus: "parity-canary" | "promoted" | "lab-only";
}

export function parseStructureCatalog(value: unknown, label: string): StructureCatalogDocument {
  const catalog = object(value, label);
  if (catalog.schemaVersion !== 1 || !Array.isArray(catalog.structures)) {
    throw new Error(`Invalid Structure Lab catalog '${label}'`);
  }
  const structures = catalog.structures.map((entry, index) =>
    parseStructureCatalogEntry(entry, `${label} structures[${index}]`)
  );
  const summary = object(catalog.summary, `${label} summary`);
  return {
    schemaVersion: 1,
    catalogSha256: string(catalog.catalogSha256, `${label} catalogSha256`),
    structures,
    summary: {
      structures: integer(summary.structures, `${label} summary.structures`),
      families: integer(summary.families, `${label} summary.families`),
      blocks: integer(summary.blocks, `${label} summary.blocks`),
      runtimePromoted: integer(summary.runtimePromoted, `${label} summary.runtimePromoted`),
    },
  };
}

export function parseStructureCatalogEntry(value: unknown, label: string): StructureCatalogEntry {
  const entry = object(value, label) as unknown as StructureCatalogEntry;
  if (
    entry.schemaVersion !== 1
    || typeof entry.structureId !== "string"
    || typeof entry.label !== "string"
    || typeof entry.description !== "string"
    || !isInt3(entry.size)
    || !Array.isArray(entry.components)
    || !Array.isArray(entry.materialBill)
    || !Array.isArray(entry.markers)
    || !Array.isArray(entry.sockets)
    || typeof entry.artifacts?.meshSha256 !== "string"
    || typeof entry.artifacts?.atlasSha256 !== "string"
    || typeof entry.geometry?.vertexCount !== "number"
  ) {
    throw new Error(`Invalid Structure Lab entry '${label}'`);
  }
  if (entry.assetProvenance.minecraftReference !== 0 || entry.assetProvenance.unknown !== 0) {
    throw new Error(`Structure '${entry.structureId}' is not safe for the public catalog`);
  }
  return entry;
}

function object(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`Expected object at '${label}'`);
  }
  return value as Record<string, unknown>;
}

function string(value: unknown, label: string): string {
  if (typeof value !== "string" || value === "") {
    throw new Error(`Expected nonempty string at '${label}'`);
  }
  return value;
}

function integer(value: unknown, label: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) {
    throw new Error(`Expected nonnegative integer at '${label}'`);
  }
  return value as number;
}

function isInt3(value: unknown): value is [number, number, number] {
  return Array.isArray(value)
    && value.length === 3
    && value.every((axis) => Number.isSafeInteger(axis));
}
