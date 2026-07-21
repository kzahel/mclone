import { block, role, structure, type StructureBuilder } from "../dsl";
import type { AuthoredStructureAsset } from "../model";

export type BarnLength = "short" | "standard" | "long";

const lengthFacts = {
  short: { frontZ: 11, adjective: "Compact" },
  standard: { frontZ: 15, adjective: "Working" },
  long: { frontZ: 19, adjective: "Long Bay" },
} as const;

export function barnCoreVariant(length: BarnLength): AuthoredStructureAsset {
  const { frontZ, adjective } = lengthFacts[length];
  const id = length === "standard"
    ? "farmstead-barn-core-a-v2"
    : `farmstead-barn-core-${length}-v1`;

  return structure({
    id,
    label: `${adjective} Red Barn`,
    description: "A timber-bayed red barn with a broad working entrance, gambrel roof, loft window, and visible hay stores.",
    category: "building",
    size: [21, 14, frontZ + 3],
    family: {
      id: "farmstead-barn",
      member: `${length}-core`,
      label: "Farmstead Barn",
    },
    tags: ["barn", "farmstead", "starter-settlement"],
    components: [
      { id: "shell", label: "Stone floor and red shell", optional: false },
      { id: "timber-frame", label: "Spruce structural bays", optional: false },
      { id: "windows", label: "Framed working windows", optional: false },
      { id: "roof", label: "Four-stage gambrel roof", optional: false },
      { id: "hay", label: "Interior hay stores", optional: true },
      { id: "lighting", label: "Entrance lanterns", optional: true },
    ],
    palette: barnPalette(),
    defaultTheme: "red-spruce-working-barn-v2",
    themes: [barnTheme()],
  }, ({ component, fillBox, lineX, lineY, lineZ, marker, set, socket }) => {
    component("shell", () => {
      fillBox([1, 0, 2], [20, 1, frontZ + 1], "foundation");
      fillBox([1, 1, 2], [20, 8, frontZ + 1], "wall");
      fillBox([2, 1, 3], [19, 8, frontZ], "air");
      fillBox([2, 1, 3], [19, 2, frontZ], "floor");
    });

    component("timber-frame", () => {
      for (const [x, z] of [[1, 2], [19, 2], [1, frontZ], [19, frontZ]] as const) {
        lineY(x, 1, 8, z, "timberY");
      }
      for (const x of [6, 14]) {
        lineY(x, 1, 8, 2, "timberY");
        lineY(x, 1, 8, frontZ, "timberY");
      }
      for (let z = 7; z < frontZ; z += 4) {
        lineY(1, 1, 8, z, "timberY");
        lineY(19, 1, 8, z, "timberY");
      }
      for (const y of [1, 5, 7]) {
        lineX(1, 20, y, 2, "timberX");
        lineX(1, 20, y, frontZ, "timberX");
        lineZ(1, y, 2, frontZ + 1, "timberZ");
        lineZ(19, y, 2, frontZ + 1, "timberZ");
      }
    });

    component("windows", () => {
      fillBox([8, 2, frontZ], [13, 7, frontZ + 1], "air");
      lineY(7, 1, 8, frontZ, "trim");
      lineY(13, 1, 8, frontZ, "trim");
      lineX(7, 14, 7, frontZ, "trim");
      framedWindowZ({ fillBox, lineX, lineY }, 3, 5, frontZ);
      framedWindowZ({ fillBox, lineX, lineY }, 16, 18, frontZ);
      framedWindowX({ fillBox, lineY, lineZ }, 1, 5, 7);
      whiteFrameWindowZ({ lineX, lineY }, 3, 5, frontZ);
      whiteFrameWindowZ({ lineX, lineY }, 16, 18, frontZ);
    });

    component("roof", () => {
      for (const z of [2, frontZ]) {
        fillBox([1, 8, z], [20, 9, z + 1], "wall");
        fillBox([4, 9, z], [17, 10, z + 1], "wall");
        fillBox([7, 10, z], [14, 11, z + 1], "wall");
        fillBox([10, 11, z], [11, 12, z + 1], "wall");
      }
      fillBox([9, 8, frontZ], [12, 10, frontZ + 1], "glazing");
      lineY(8, 8, 11, frontZ, "trim");
      lineY(12, 8, 11, frontZ, "trim");
      lineX(8, 13, 10, frontZ, "trim");

      for (const [x, y, palette] of [
        [2, 8, "roofEast"],
        [5, 9, "roofEast"],
        [8, 10, "roofEast"],
        [9, 11, "roofEast"],
        [18, 8, "roofWest"],
        [15, 9, "roofWest"],
        [12, 10, "roofWest"],
        [11, 11, "roofWest"],
      ] as const) {
        lineZ(x, y, 1, frontZ + 2, palette);
      }
      for (const [minX, maxX, y] of [
        [0, 2, 8],
        [19, 21, 8],
        [3, 5, 9],
        [16, 18, 9],
        [6, 8, 10],
        [13, 15, 10],
      ] as const) {
        fillBox([minX, y, 1], [maxX, y + 1, frontZ + 2], "roofSlabBottom");
      }
      lineZ(10, 12, 1, frontZ + 2, "roofSlabBottom");
    });

    component("hay", () => {
      fillBox([3, 2, 5], [6, 4, 8], "accent");
      fillBox([15, 2, 7], [18, 3, 11], "accent");
      lineY(7, 2, 8, 8, "timberY");
      lineY(13, 2, 8, 8, "timberY");
    });

    component("lighting", () => {
      set([8, 4, frontZ], "wallTorchSouth");
      set([13, 4, frontZ], "wallTorchSouth");
    });

    const leanToPos = [20, 2, 8] as const;
    marker("entrance:south", [10, 2, frontZ + 2]);
    marker("attachment:east-lean-to", leanToPos);
    marker("loft:front", [10, 8, frontZ]);
    socket("east-lean-to", "attachment:lean-to", leanToPos, "east");
  });
}

export function barnLeanToVariant(length: BarnLength): AuthoredStructureAsset {
  const sizeZ = lengthFacts[length].frontZ - 2;
  const id = length === "standard"
    ? "farmstead-barn-lean-to-a-v2"
    : `farmstead-barn-lean-to-${length}-v1`;
  return structure({
    id,
    label: `${lengthFacts[length].adjective} Barn Lean-to`,
    description: "An open-sided hay and livestock shelter that sockets onto the east wall of a matching farmstead barn.",
    category: "outbuilding",
    size: [7, 8, sizeZ],
    family: {
      id: "farmstead-barn",
      member: `${length}-lean-to`,
      label: "Farmstead Barn",
    },
    tags: ["barn", "farmstead", "lean-to", "starter-settlement"],
    components: [
      { id: "floor", label: "Stone and timber floor", optional: false },
      { id: "frame", label: "Open spruce frame", optional: false },
      { id: "roof", label: "Sloping spruce roof", optional: false },
      { id: "hay", label: "Hay stores", optional: true },
    ],
    palette: barnPalette(),
    defaultTheme: "red-spruce-working-barn-v2",
    themes: [barnTheme()],
  }, ({ component, fillBox, lineY, lineZ, marker, socket }) => {
    component("floor", () => {
      fillBox([0, 0, 0], [7, 1, sizeZ], "foundation");
      fillBox([0, 1, 0], [7, 2, sizeZ], "floor");
    });
    component("frame", () => {
      const postZs: number[] = [];
      for (let z = 0; z < sizeZ; z += 6) postZs.push(z);
      if (postZs.at(-1) !== sizeZ - 1) postZs.push(sizeZ - 1);
      for (const z of postZs) {
        lineY(0, 2, 7, z, "timberY");
        lineY(6, 2, 5, z, "timberY");
      }
      lineZ(6, 4, 0, sizeZ, "timberZ");
    });
    component("roof", () => {
      lineZ(0, 6, 0, sizeZ, "roofSlabBottom");
      lineZ(1, 5, 0, sizeZ, "roofWest");
      fillBox([2, 5, 0], [4, 6, sizeZ], "roofSlabBottom");
      lineZ(4, 4, 0, sizeZ, "roofWest");
      fillBox([5, 4, 0], [7, 5, sizeZ], "roofSlabBottom");
    });
    component("hay", () => {
      fillBox([4, 2, 2], [7, 4, 5], "accent");
      fillBox([2, 2, sizeZ - 5], [6, 3, sizeZ - 2], "accent");
    });
    const barnPos = [0, 2, Math.floor(sizeZ / 2)] as const;
    marker("attachment:west-barn", barnPos);
    marker("yard:south", [6, 2, sizeZ - 1]);
    socket("west-barn", "attachment:barn", barnPos, "west");
  });
}

function barnPalette() {
  return {
    accent: role("accent"),
    air: block("minecraft:air"),
    floor: role("floor"),
    foundation: role("foundation"),
    glazing: role("glazing"),
    roofEast: role("roofEast"),
    roofSlabBottom: role("roofSlabBottom"),
    roofWest: role("roofWest"),
    timberX: role("timberX"),
    timberY: role("timberY"),
    timberZ: role("timberZ"),
    trim: role("trim"),
    wall: role("wall"),
    wallTorchSouth: block("minecraft:wall_torch[facing=south]"),
  } as const;
}

function barnTheme() {
  return {
    id: "red-spruce-working-barn-v2",
    label: "Red spruce working barn",
    materials: {
      foundation: "minecraft:stone_bricks",
      wall: "minecraft:red_terracotta",
      timberY: "minecraft:spruce_log[axis=y]",
      timberX: "minecraft:spruce_log[axis=x]",
      timberZ: "minecraft:spruce_log[axis=z]",
      roof: "minecraft:spruce_planks",
      roofNorth: "minecraft:spruce_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]",
      roofEast: "minecraft:spruce_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]",
      roofSouth: "minecraft:spruce_stairs[facing=south,half=bottom,shape=straight,waterlogged=false]",
      roofWest: "minecraft:spruce_stairs[facing=west,half=bottom,shape=straight,waterlogged=false]",
      roofSlabBottom: "minecraft:spruce_slab[type=bottom,waterlogged=false]",
      roofSlabTop: "minecraft:spruce_slab[type=top,waterlogged=false]",
      glazing: "minecraft:glass",
      trim: "minecraft:white_terracotta",
      floor: "minecraft:oak_planks",
      accent: "minecraft:hay_block",
    },
  } as const;
}

function framedWindowZ(
  builder: Pick<StructureBuilder, "fillBox" | "lineX" | "lineY">,
  minX: number,
  maxXExclusive: number,
  z: number,
): void {
  builder.fillBox([minX, 2, z], [maxXExclusive, 4, z + 1], "glazing");
  builder.lineY(minX - 1, 1, 5, z, "timberY");
  builder.lineY(maxXExclusive, 1, 5, z, "timberY");
  builder.lineX(minX - 1, maxXExclusive + 1, 4, z, "timberX");
}

function framedWindowX(
  builder: Pick<StructureBuilder, "fillBox" | "lineY" | "lineZ">,
  x: number,
  minZ: number,
  maxZExclusive: number,
): void {
  builder.fillBox([x, 2, minZ], [x + 1, 4, maxZExclusive], "glazing");
  builder.lineY(x, 1, 5, minZ - 1, "timberY");
  builder.lineY(x, 1, 5, maxZExclusive, "timberY");
  builder.lineZ(x, 4, minZ - 1, maxZExclusive + 1, "timberZ");
}

function whiteFrameWindowZ(
  builder: Pick<StructureBuilder, "lineX" | "lineY">,
  minX: number,
  maxXExclusive: number,
  z: number,
): void {
  builder.lineY(minX - 1, 1, 5, z, "trim");
  builder.lineY(maxXExclusive, 1, 5, z, "trim");
  builder.lineX(minX - 1, maxXExclusive + 1, 4, z, "trim");
}
