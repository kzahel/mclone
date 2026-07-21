import {
  MATERIAL_ROLES,
  type AuthoredStructureAsset,
  type BlockPaletteState,
  type CanonicalStructureBlock,
  type Int3,
  type MaterialRole,
  type PaletteState,
  type RolePaletteState,
  type SocketFacing,
  type StructureCategory,
  type StructureComponent,
  type StructureFamily,
  type StructureMarker,
  type StructureSocket,
  type StructureTheme,
} from "./model";

export type { Int3, MaterialRole, SocketFacing } from "./model";

export interface StructureSpec {
  category: StructureCategory;
  components?: readonly StructureComponent[];
  defaultTheme?: string;
  description: string;
  family?: StructureFamily;
  id: string;
  label: string;
  palette: Readonly<Record<string, PaletteState>>;
  size: Int3;
  tags?: readonly string[];
  themes?: readonly StructureTheme[];
}

export interface StructureBuilder {
  component: (id: string, build: () => void) => void;
  fillBox: (min: Int3, maxExclusive: Int3, palette: string) => void;
  lineX: (minX: number, maxXExclusive: number, y: number, z: number, palette: string) => void;
  lineY: (x: number, minY: number, maxYExclusive: number, z: number, palette: string) => void;
  lineZ: (x: number, y: number, minZ: number, maxZExclusive: number, palette: string) => void;
  marker: (kind: string, pos: Int3) => void;
  set: (pos: Int3, palette: string) => void;
  socket: (id: string, kind: string, pos: Int3, facing: SocketFacing) => void;
}

export function role(roleName: MaterialRole): RolePaletteState {
  return { kind: "role", role: roleName };
}

export function block(state: string): BlockPaletteState {
  return { kind: "block", state };
}

export function structure(
  spec: StructureSpec,
  build: (builder: StructureBuilder) => void,
): AuthoredStructureAsset {
  validateStructureSpec(spec);
  const paletteEntries = Object.entries(spec.palette)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, state]) => ({ key, state }));
  const paletteIndex = new Map(paletteEntries.map((entry, index) => [entry.key, index] as const));
  const componentIds = new Set((spec.components ?? []).map((component) => component.id));
  const blocks = new Map<string, CanonicalStructureBlock>();
  const markers: StructureMarker[] = [];
  const sockets: StructureSocket[] = [];
  const socketIds = new Set<string>();
  const activeComponents: string[] = [];

  const validatePosition = (pos: Int3, label: string): void => {
    assertInt3(pos, label);
    if (pos.some((axis, index) => axis < 0 || axis >= spec.size[index]!)) {
      throw new Error(`${label} ${formatPos(pos)} is outside structure size ${formatPos(spec.size)}`);
    }
  };

  const resolvePalette = (key: string): number => {
    const index = paletteIndex.get(key);
    if (index === undefined) {
      throw new Error(`Structure '${spec.id}' references unknown palette key '${key}'`);
    }
    return index;
  };

  const builder: StructureBuilder = {
    component(id, componentBuild) {
      if (!componentIds.has(id)) {
        throw new Error(`Structure '${spec.id}' references undeclared component '${id}'`);
      }
      if (activeComponents.includes(id)) {
        throw new Error(`Structure '${spec.id}' nests component '${id}' inside itself`);
      }
      activeComponents.push(id);
      try {
        componentBuild();
      } finally {
        activeComponents.pop();
      }
    },

    fillBox(min, maxExclusive, palette) {
      assertInt3(min, "fillBox min");
      assertInt3(maxExclusive, "fillBox maxExclusive");
      if (min.some((axis, index) => axis >= maxExclusive[index]!)) {
        throw new Error(
          `Structure '${spec.id}' box ${formatPos(min)}..${formatPos(maxExclusive)} must have positive extent`,
        );
      }
      validatePosition(min, "fillBox min");
      validatePosition(
        [maxExclusive[0] - 1, maxExclusive[1] - 1, maxExclusive[2] - 1],
        "fillBox maxExclusive",
      );
      for (let y = min[1]; y < maxExclusive[1]; y += 1) {
        for (let z = min[2]; z < maxExclusive[2]; z += 1) {
          for (let x = min[0]; x < maxExclusive[0]; x += 1) {
            builder.set([x, y, z], palette);
          }
        }
      }
    },

    lineX(minX, maxXExclusive, y, z, palette) {
      builder.fillBox([minX, y, z], [maxXExclusive, y + 1, z + 1], palette);
    },

    lineY(x, minY, maxYExclusive, z, palette) {
      builder.fillBox([x, minY, z], [x + 1, maxYExclusive, z + 1], palette);
    },

    lineZ(x, y, minZ, maxZExclusive, palette) {
      builder.fillBox([x, y, minZ], [x + 1, y + 1, maxZExclusive], palette);
    },

    marker(kind, pos) {
      validateMarkerKind(kind);
      validatePosition(pos, `marker '${kind}'`);
      markers.push({ kind, pos: [...pos] as Int3 });
    },

    set(pos, palette) {
      validatePosition(pos, "block position");
      blocks.set(positionKey(pos), {
        components: [...activeComponents].sort(),
        palette: resolvePalette(palette),
        pos: [...pos] as Int3,
      });
    },

    socket(id, kind, pos, facing) {
      assertSafeId(id, "socket id");
      validateMarkerKind(kind);
      validatePosition(pos, `socket '${id}'`);
      if (socketIds.has(id)) {
        throw new Error(`Structure '${spec.id}' contains duplicate socket '${id}'`);
      }
      socketIds.add(id);
      sockets.push({ facing, id, kind, pos: [...pos] as Int3 });
    },
  };

  build(builder);
  const asset: AuthoredStructureAsset = {
    blocks: [...blocks.values()].sort(compareBlocks),
    category: spec.category,
    components: [...(spec.components ?? [])]
      .map((component) => ({ ...component }))
      .sort((left, right) => left.id.localeCompare(right.id)),
    ...(spec.defaultTheme === undefined ? {} : { defaultTheme: spec.defaultTheme }),
    description: spec.description.trim(),
    ...(spec.family === undefined ? {} : { family: { ...spec.family } }),
    id: spec.id,
    label: spec.label.trim(),
    markers: markers.sort(compareMarkers),
    palette: paletteEntries,
    schemaVersion: 1,
    size: [...spec.size] as Int3,
    sockets: sockets.sort((left, right) => left.id.localeCompare(right.id)),
    tags: [...(spec.tags ?? [])].sort(),
    themes: [...(spec.themes ?? [])]
      .map((theme) => ({
        ...theme,
        materials: Object.fromEntries(
          Object.entries(theme.materials).sort(([left], [right]) => left.localeCompare(right)),
        ),
      }))
      .sort((left, right) => left.id.localeCompare(right.id)),
  };
  assertValidAuthoredStructure(asset);
  return asset;
}

export function assertValidAuthoredStructure(asset: AuthoredStructureAsset): void {
  validateStructureSpec({
    category: asset.category,
    components: asset.components,
    ...(asset.defaultTheme === undefined ? {} : { defaultTheme: asset.defaultTheme }),
    description: asset.description,
    ...(asset.family === undefined ? {} : { family: asset.family }),
    id: asset.id,
    label: asset.label,
    palette: Object.fromEntries(asset.palette.map((entry) => [entry.key, entry.state])),
    size: asset.size,
    tags: asset.tags,
    themes: asset.themes,
  });
  const componentIds = new Set(asset.components.map((component) => component.id));
  const positions = new Set<string>();
  for (const blockEntry of asset.blocks) {
    assertInt3(blockEntry.pos, "block position");
    if (blockEntry.pos.some((axis, index) => axis < 0 || axis >= asset.size[index]!)) {
      throw new Error(`Structure '${asset.id}' contains an out-of-bounds block ${formatPos(blockEntry.pos)}`);
    }
    if (!Number.isInteger(blockEntry.palette) || blockEntry.palette < 0 || blockEntry.palette >= asset.palette.length) {
      throw new Error(`Structure '${asset.id}' contains invalid palette index ${blockEntry.palette}`);
    }
    const key = positionKey(blockEntry.pos);
    if (positions.has(key)) {
      throw new Error(`Structure '${asset.id}' contains duplicate block position ${formatPos(blockEntry.pos)}`);
    }
    positions.add(key);
    for (const component of blockEntry.components) {
      if (!componentIds.has(component)) {
        throw new Error(`Structure '${asset.id}' block references unknown component '${component}'`);
      }
    }
  }
  for (const markerEntry of asset.markers) {
    validateMarkerKind(markerEntry.kind);
    assertPositionInSize(markerEntry.pos, asset.size, `marker '${markerEntry.kind}'`);
  }
  const socketIds = new Set<string>();
  for (const socketEntry of asset.sockets) {
    assertSafeId(socketEntry.id, "socket id");
    validateMarkerKind(socketEntry.kind);
    assertPositionInSize(socketEntry.pos, asset.size, `socket '${socketEntry.id}'`);
    if (socketIds.has(socketEntry.id)) {
      throw new Error(`Structure '${asset.id}' contains duplicate socket '${socketEntry.id}'`);
    }
    socketIds.add(socketEntry.id);
  }
}

function validateStructureSpec(spec: StructureSpec): void {
  assertSafeId(spec.id, "structure id");
  assertNonempty(spec.label, "structure label");
  assertNonempty(spec.description, "structure description");
  assertInt3(spec.size, "structure size");
  if (spec.size.some((axis) => axis <= 0 || axis > 512)) {
    throw new Error(`Structure '${spec.id}' size ${formatPos(spec.size)} must be within 1..512`);
  }
  const paletteKeys = Object.keys(spec.palette);
  if (paletteKeys.length === 0) {
    throw new Error(`Structure '${spec.id}' must declare a palette`);
  }
  for (const [key, state] of Object.entries(spec.palette)) {
    assertPaletteKey(key);
    if (state.kind === "role") {
      if (!(MATERIAL_ROLES as readonly string[]).includes(state.role)) {
        throw new Error(`Structure '${spec.id}' palette '${key}' has unknown role '${state.role}'`);
      }
    } else if (!isCanonicalBlockState(state.state)) {
      throw new Error(`Structure '${spec.id}' palette '${key}' has invalid block state '${state.state}'`);
    }
  }
  assertUniqueIds(spec.components ?? [], "component");
  assertUniqueIds(spec.themes ?? [], "theme");
  for (const component of spec.components ?? []) {
    assertNonempty(component.label, `component '${component.id}' label`);
  }
  for (const theme of spec.themes ?? []) {
    assertNonempty(theme.label, `theme '${theme.id}' label`);
    for (const [roleName, state] of Object.entries(theme.materials)) {
      if (!(MATERIAL_ROLES as readonly string[]).includes(roleName) || !isCanonicalBlockState(state)) {
        throw new Error(`Theme '${theme.id}' has invalid material '${roleName}=${state}'`);
      }
    }
  }
  if (spec.defaultTheme !== undefined && !(spec.themes ?? []).some((theme) => theme.id === spec.defaultTheme)) {
    throw new Error(`Structure '${spec.id}' default theme '${spec.defaultTheme}' is not declared`);
  }
  if (spec.family) {
    assertSafeId(spec.family.id, "family id");
    assertSafeId(spec.family.member, "family member");
    assertNonempty(spec.family.label, "family label");
  }
  const tags = new Set<string>();
  for (const tag of spec.tags ?? []) {
    assertSafeId(tag, "tag");
    if (tags.has(tag)) {
      throw new Error(`Structure '${spec.id}' contains duplicate tag '${tag}'`);
    }
    tags.add(tag);
  }
}

function assertUniqueIds(entries: readonly { id: string }[], label: string): void {
  const ids = new Set<string>();
  for (const entry of entries) {
    assertSafeId(entry.id, `${label} id`);
    if (ids.has(entry.id)) {
      throw new Error(`Duplicate ${label} '${entry.id}'`);
    }
    ids.add(entry.id);
  }
}

function assertPositionInSize(pos: Int3, size: Int3, label: string): void {
  assertInt3(pos, label);
  if (pos.some((axis, index) => axis < 0 || axis >= size[index]!)) {
    throw new Error(`${label} ${formatPos(pos)} is outside structure size ${formatPos(size)}`);
  }
}

function assertInt3(value: Int3, label: string): void {
  if (value.length !== 3 || value.some((axis) => !Number.isSafeInteger(axis))) {
    throw new Error(`${label} must contain three safe integers`);
  }
}

function assertSafeId(value: string, label: string): void {
  if (!/^[a-z0-9][a-z0-9_-]*$/u.test(value)) {
    throw new Error(`${label} '${value}' must use lowercase letters, digits, underscores, or hyphens`);
  }
}

function assertPaletteKey(value: string): void {
  if (!/^[a-z][A-Za-z0-9_-]*$/u.test(value)) {
    throw new Error(
      `palette key '${value}' must start lowercase and use letters, digits, underscores, or hyphens`,
    );
  }
}

function assertNonempty(value: string, label: string): void {
  if (value.trim() === "") {
    throw new Error(`${label} must not be empty`);
  }
}

function validateMarkerKind(kind: string): void {
  if (!/^[a-z0-9][a-z0-9_:-]*$/u.test(kind)) {
    throw new Error(`Marker kind '${kind}' is invalid`);
  }
}

function isCanonicalBlockState(value: string): boolean {
  return /^[a-z0-9_.-]+:[a-z0-9_./-]+(?:\[[a-z0-9_]+=[a-z0-9_.-]+(?:,[a-z0-9_]+=[a-z0-9_.-]+)*\])?$/u.test(value);
}

function compareBlocks(left: CanonicalStructureBlock, right: CanonicalStructureBlock): number {
  return comparePositions(left.pos, right.pos);
}

function compareMarkers(left: StructureMarker, right: StructureMarker): number {
  return comparePositions(left.pos, right.pos) || left.kind.localeCompare(right.kind);
}

function comparePositions(left: Int3, right: Int3): number {
  return left[0] - right[0] || left[1] - right[1] || left[2] - right[2];
}

function positionKey(pos: Int3): string {
  return `${pos[0]},${pos[1]},${pos[2]}`;
}

function formatPos(pos: Int3): string {
  return `[${pos.join(", ")}]`;
}
