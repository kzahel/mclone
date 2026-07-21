export type Int3 = readonly [number, number, number];

export const MATERIAL_ROLES = [
  "foundation",
  "wall",
  "timberY",
  "timberX",
  "timberZ",
  "roof",
  "roofNorth",
  "roofEast",
  "roofSouth",
  "roofWest",
  "roofSlabBottom",
  "roofSlabTop",
  "glazing",
  "trim",
  "floor",
  "accent",
] as const;

export type MaterialRole = typeof MATERIAL_ROLES[number];
export type StructureCategory = "building" | "outbuilding" | "infrastructure" | "decoration";
export type SocketFacing = "north" | "east" | "south" | "west" | "up" | "down";

export interface RolePaletteState {
  kind: "role";
  role: MaterialRole;
}

export interface BlockPaletteState {
  kind: "block";
  state: string;
}

export type PaletteState = RolePaletteState | BlockPaletteState;

export interface StructureComponent {
  id: string;
  label: string;
  optional: boolean;
}

export interface StructureFamily {
  id: string;
  member: string;
  label: string;
}

export interface StructureTheme {
  id: string;
  label: string;
  materials: Partial<Record<MaterialRole, string>>;
}

export interface CanonicalPaletteEntry {
  key: string;
  state: PaletteState;
}

export interface CanonicalStructureBlock {
  components: string[];
  palette: number;
  pos: Int3;
}

export interface StructureMarker {
  kind: string;
  pos: Int3;
}

export interface StructureSocket {
  facing: SocketFacing;
  id: string;
  kind: string;
  pos: Int3;
}

export interface StructureProvenance {
  compilerId: string;
  semanticSha256: string;
  sourcePath: string;
  sourceSha256: string;
}

export interface AuthoredStructureAsset {
  blocks: CanonicalStructureBlock[];
  category: StructureCategory;
  components: StructureComponent[];
  defaultTheme?: string;
  description: string;
  family?: StructureFamily;
  id: string;
  label: string;
  markers: StructureMarker[];
  palette: CanonicalPaletteEntry[];
  schemaVersion: 1;
  size: Int3;
  sockets: StructureSocket[];
  tags: string[];
  themes: StructureTheme[];
}

export interface StructureAsset extends AuthoredStructureAsset {
  provenance: StructureProvenance;
}
