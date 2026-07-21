import { createHash } from "node:crypto";
import { assertValidAuthoredStructure } from "./dsl";
import {
  MATERIAL_ROLES,
  type AuthoredStructureAsset,
  type CanonicalPaletteEntry,
  type CanonicalStructureBlock,
  type Int3,
  type MaterialRole,
  type PaletteState,
  type SocketFacing,
  type StructureAsset,
  type StructureCategory,
  type StructureComponent,
  type StructureFamily,
  type StructureMarker,
  type StructureProvenance,
  type StructureSocket,
  type StructureTheme,
} from "./model";

export const STRUCTURE_COMPILER_ID = "mclone-structure-dsl-v2";

export interface StructureJsonDocument {
  asset: StructureAsset;
  json: string;
}

export function serializeStructureAsset(asset: StructureAsset): string {
  assertValidStructureAsset(asset);
  const blocksPlaceholder = "__mclone_structure_blocks_placeholder__";
  const withPlaceholder = { ...asset, blocks: blocksPlaceholder };
  const blocks = asset.blocks.length === 0
    ? "[]"
    : `[\n${asset.blocks.map((entry) => `    ${JSON.stringify(entry)}`).join(",\n")}\n  ]`;
  const json = JSON.stringify(withPlaceholder, null, 2).replace(
    `"blocks": "${blocksPlaceholder}"`,
    `"blocks": ${blocks}`,
  );
  return `${json}\n`;
}

export function parseStructureAssetJson(json: string, sourceLabel: string): StructureAsset {
  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`Could not parse structure JSON '${sourceLabel}': ${detail}`);
  }
  return structureAssetFromUnknown(value, sourceLabel);
}

export function authoredStructureFromUnknown(
  value: unknown,
  sourceLabel: string,
): AuthoredStructureAsset {
  if (!isRecord(value) || value.schemaVersion !== 1) {
    throw new Error(`Expected '${sourceLabel}' to export a schema-v1 structure`);
  }
  const asset: AuthoredStructureAsset = {
    blocks: parseBlocks(value.blocks, sourceLabel),
    category: parseCategory(value.category, sourceLabel),
    components: parseComponents(value.components, sourceLabel),
    ...(value.defaultTheme === undefined
      ? {}
      : { defaultTheme: parseString(value.defaultTheme, sourceLabel, "defaultTheme") }),
    description: parseString(value.description, sourceLabel, "description"),
    ...(value.family === undefined ? {} : { family: parseFamily(value.family, sourceLabel) }),
    id: parseString(value.id, sourceLabel, "id"),
    label: parseString(value.label, sourceLabel, "label"),
    markers: parseMarkers(value.markers, sourceLabel),
    palette: parsePalette(value.palette, sourceLabel),
    schemaVersion: 1,
    size: parseInt3(value.size, sourceLabel, "size"),
    sockets: parseSockets(value.sockets, sourceLabel),
    tags: parseStringArray(value.tags, sourceLabel, "tags"),
    themes: parseThemes(value.themes, sourceLabel),
  };
  try {
    assertValidAuthoredStructure(asset);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`Structure '${sourceLabel}' failed validation: ${detail}`);
  }
  return asset;
}

export function structureAssetFromUnknown(value: unknown, sourceLabel: string): StructureAsset {
  const authored = authoredStructureFromUnknown(value, sourceLabel);
  if (!isRecord(value) || !isRecord(value.provenance)) {
    throw new Error(`Structure '${sourceLabel}' is missing generated provenance`);
  }
  const provenance: StructureProvenance = {
    compilerId: parseString(value.provenance.compilerId, sourceLabel, "provenance.compilerId"),
    sourcePath: parseSafeRelativePath(
      value.provenance.sourcePath,
      sourceLabel,
      "provenance.sourcePath",
    ),
    sourceSha256: parseSha256(
      value.provenance.sourceSha256,
      sourceLabel,
      "provenance.sourceSha256",
    ),
    semanticSha256: parseSha256(
      value.provenance.semanticSha256,
      sourceLabel,
      "provenance.semanticSha256",
    ),
  };
  const asset = { ...authored, provenance };
  assertValidStructureAsset(asset);
  return asset;
}

export function attachStructureProvenance(
  authored: AuthoredStructureAsset,
  sourcePath: string,
  sourceBytes: Uint8Array,
): StructureAsset {
  const baseProvenance = {
    compilerId: STRUCTURE_COMPILER_ID,
    sourcePath,
    sourceSha256: sha256(sourceBytes),
  };
  const semanticSha256 = semanticHash(authored, baseProvenance);
  const asset: StructureAsset = {
    ...authored,
    provenance: { ...baseProvenance, semanticSha256 },
  };
  assertValidStructureAsset(asset);
  return asset;
}

export function assertValidStructureAsset(asset: StructureAsset): void {
  assertValidAuthoredStructure(asset);
  if (asset.provenance.compilerId.trim() === "") {
    throw new Error(`Structure '${asset.id}' has an empty compiler id`);
  }
  if (!isSafeRelativePath(asset.provenance.sourcePath)) {
    throw new Error(`Structure '${asset.id}' has unsafe source path '${asset.provenance.sourcePath}'`);
  }
  if (!isSha256(asset.provenance.sourceSha256) || !isSha256(asset.provenance.semanticSha256)) {
    throw new Error(`Structure '${asset.id}' has invalid provenance hashes`);
  }
  const expected = semanticHash(asset, {
    compilerId: asset.provenance.compilerId,
    sourcePath: asset.provenance.sourcePath,
    sourceSha256: asset.provenance.sourceSha256,
  });
  if (asset.provenance.semanticSha256 !== expected) {
    throw new Error(
      `Structure '${asset.id}' semantic hash is stale: expected ${expected}, got ${asset.provenance.semanticSha256}`,
    );
  }
}

function semanticHash(
  authored: AuthoredStructureAsset,
  provenance: Omit<StructureProvenance, "semanticSha256">,
): string {
  const { provenance: _ignored, ...withoutProvenance } = authored as StructureAsset;
  return sha256(`${JSON.stringify({ ...withoutProvenance, provenance }, null, 2)}\n`);
}

function parsePalette(value: unknown, sourceLabel: string): CanonicalPaletteEntry[] {
  if (!Array.isArray(value)) {
    throw new Error(`Structure '${sourceLabel}' palette must be an array`);
  }
  return value.map((entry, index) => {
    if (!isRecord(entry)) {
      throw new Error(`Structure '${sourceLabel}' palette entry ${index} must be an object`);
    }
    return {
      key: parseString(entry.key, sourceLabel, `palette[${index}].key`),
      state: parsePaletteState(entry.state, sourceLabel, index),
    };
  });
}

function parsePaletteState(value: unknown, sourceLabel: string, index: number): PaletteState {
  if (!isRecord(value)) {
    throw new Error(`Structure '${sourceLabel}' palette state ${index} must be an object`);
  }
  if (value.kind === "role") {
    const roleName = parseString(value.role, sourceLabel, `palette[${index}].state.role`);
    if (!(MATERIAL_ROLES as readonly string[]).includes(roleName)) {
      throw new Error(`Structure '${sourceLabel}' palette state ${index} has unknown role '${roleName}'`);
    }
    return { kind: "role", role: roleName as MaterialRole };
  }
  if (value.kind === "block") {
    return {
      kind: "block",
      state: parseString(value.state, sourceLabel, `palette[${index}].state.state`),
    };
  }
  throw new Error(`Structure '${sourceLabel}' palette state ${index} has invalid kind`);
}

function parseBlocks(value: unknown, sourceLabel: string): CanonicalStructureBlock[] {
  if (!Array.isArray(value)) {
    throw new Error(`Structure '${sourceLabel}' blocks must be an array`);
  }
  return value.map((entry, index) => {
    if (!isRecord(entry) || !Number.isSafeInteger(entry.palette)) {
      throw new Error(`Structure '${sourceLabel}' block ${index} is invalid`);
    }
    return {
      components: parseStringArray(entry.components, sourceLabel, `blocks[${index}].components`),
      palette: entry.palette as number,
      pos: parseInt3(entry.pos, sourceLabel, `blocks[${index}].pos`),
    };
  });
}

function parseComponents(value: unknown, sourceLabel: string): StructureComponent[] {
  if (!Array.isArray(value)) {
    throw new Error(`Structure '${sourceLabel}' components must be an array`);
  }
  return value.map((entry, index) => {
    if (!isRecord(entry) || typeof entry.optional !== "boolean") {
      throw new Error(`Structure '${sourceLabel}' component ${index} is invalid`);
    }
    return {
      id: parseString(entry.id, sourceLabel, `components[${index}].id`),
      label: parseString(entry.label, sourceLabel, `components[${index}].label`),
      optional: entry.optional,
    };
  });
}

function parseFamily(value: unknown, sourceLabel: string): StructureFamily {
  if (!isRecord(value)) {
    throw new Error(`Structure '${sourceLabel}' family must be an object`);
  }
  return {
    id: parseString(value.id, sourceLabel, "family.id"),
    member: parseString(value.member, sourceLabel, "family.member"),
    label: parseString(value.label, sourceLabel, "family.label"),
  };
}

function parseMarkers(value: unknown, sourceLabel: string): StructureMarker[] {
  if (!Array.isArray(value)) {
    throw new Error(`Structure '${sourceLabel}' markers must be an array`);
  }
  return value.map((entry, index) => {
    if (!isRecord(entry)) {
      throw new Error(`Structure '${sourceLabel}' marker ${index} is invalid`);
    }
    return {
      kind: parseString(entry.kind, sourceLabel, `markers[${index}].kind`),
      pos: parseInt3(entry.pos, sourceLabel, `markers[${index}].pos`),
    };
  });
}

function parseSockets(value: unknown, sourceLabel: string): StructureSocket[] {
  if (!Array.isArray(value)) {
    throw new Error(`Structure '${sourceLabel}' sockets must be an array`);
  }
  return value.map((entry, index) => {
    if (!isRecord(entry) || !isSocketFacing(entry.facing)) {
      throw new Error(`Structure '${sourceLabel}' socket ${index} is invalid`);
    }
    return {
      facing: entry.facing,
      id: parseString(entry.id, sourceLabel, `sockets[${index}].id`),
      kind: parseString(entry.kind, sourceLabel, `sockets[${index}].kind`),
      pos: parseInt3(entry.pos, sourceLabel, `sockets[${index}].pos`),
    };
  });
}

function parseThemes(value: unknown, sourceLabel: string): StructureTheme[] {
  if (!Array.isArray(value)) {
    throw new Error(`Structure '${sourceLabel}' themes must be an array`);
  }
  return value.map((entry, index) => {
    if (!isRecord(entry) || !isRecord(entry.materials)) {
      throw new Error(`Structure '${sourceLabel}' theme ${index} is invalid`);
    }
    const materials: Partial<Record<MaterialRole, string>> = {};
    for (const [roleName, state] of Object.entries(entry.materials)) {
      if (!(MATERIAL_ROLES as readonly string[]).includes(roleName)) {
        throw new Error(`Structure '${sourceLabel}' theme ${index} has unknown role '${roleName}'`);
      }
      materials[roleName as MaterialRole] = parseString(
        state,
        sourceLabel,
        `themes[${index}].materials.${roleName}`,
      );
    }
    return {
      id: parseString(entry.id, sourceLabel, `themes[${index}].id`),
      label: parseString(entry.label, sourceLabel, `themes[${index}].label`),
      materials,
    };
  });
}

function parseCategory(value: unknown, sourceLabel: string): StructureCategory {
  if (value === "building" || value === "outbuilding" || value === "infrastructure" || value === "decoration") {
    return value;
  }
  throw new Error(`Structure '${sourceLabel}' has invalid category '${String(value)}'`);
}

function parseInt3(value: unknown, sourceLabel: string, field: string): Int3 {
  if (!Array.isArray(value) || value.length !== 3 || !value.every(Number.isSafeInteger)) {
    throw new Error(`Structure '${sourceLabel}' ${field} must contain three safe integers`);
  }
  return [value[0] as number, value[1] as number, value[2] as number];
}

function parseStringArray(value: unknown, sourceLabel: string, field: string): string[] {
  if (!Array.isArray(value) || !value.every((entry) => typeof entry === "string")) {
    throw new Error(`Structure '${sourceLabel}' ${field} must be a string array`);
  }
  return [...value] as string[];
}

function parseString(value: unknown, sourceLabel: string, field: string): string {
  if (typeof value !== "string") {
    throw new Error(`Structure '${sourceLabel}' ${field} must be a string`);
  }
  return value;
}

function parseSha256(value: unknown, sourceLabel: string, field: string): string {
  const parsed = parseString(value, sourceLabel, field);
  if (!isSha256(parsed)) {
    throw new Error(`Structure '${sourceLabel}' ${field} must be a SHA-256 digest`);
  }
  return parsed;
}

function parseSafeRelativePath(value: unknown, sourceLabel: string, field: string): string {
  const parsed = parseString(value, sourceLabel, field);
  if (!isSafeRelativePath(parsed)) {
    throw new Error(`Structure '${sourceLabel}' ${field} must be a safe repository-relative path`);
  }
  return parsed;
}

function isSocketFacing(value: unknown): value is SocketFacing {
  return value === "north" || value === "east" || value === "south" || value === "west"
    || value === "up" || value === "down";
}

function isSafeRelativePath(value: string): boolean {
  return value !== "" && !value.startsWith("/") && !value.includes("\\")
    && value.split("/").every((part) => part !== "" && part !== "." && part !== "..");
}

function isSha256(value: string): boolean {
  return /^[a-f0-9]{64}$/u.test(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function sha256(value: string | Uint8Array): string {
  return createHash("sha256").update(value).digest("hex");
}
