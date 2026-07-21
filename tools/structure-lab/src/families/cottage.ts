import { block, role, structure } from "../dsl";
import type { AuthoredStructureAsset } from "../model";

export type CottageDepth = "snug" | "standard" | "deep";
export type CottageEntry = "stoop" | "canopy-porch";

interface CottageVariant {
  depth: CottageDepth;
  entry: CottageEntry;
}

const depthFacts = {
  snug: { frontZ: 10, adjective: "Cozy" },
  standard: { frontZ: 12, adjective: "Warm" },
  deep: { frontZ: 14, adjective: "Long Hearth" },
} as const;

export function cottageVariant({ depth, entry }: CottageVariant): AuthoredStructureAsset {
  const { frontZ, adjective } = depthFacts[depth];
  const sizeZ = frontZ + (entry === "stoop" ? 3 : 5);
  const isStandard = depth === "standard" && entry === "canopy-porch";
  const id = isStandard
    ? "farmstead-cottage-a-v2"
    : `farmstead-cottage-${depth}-${entry}-v1`;
  const entryLabel = entry === "stoop" ? "simple stoop" : "offset canopy porch";

  return structure({
    id,
    label: `${adjective} Oak Cottage`,
    description: `A steep-gabled plaster cottage with a ${entryLabel}, flower box, and handmade chimney.`,
    category: "building",
    size: [15, 15, sizeZ],
    family: {
      id: "farmstead-cottage",
      member: `${depth}-${entry}`,
      label: "Farmstead Cottage",
    },
    tags: ["cottage", "farmstead", "starter-settlement"],
    components: [
      { id: "shell", label: "Foundation and plaster shell", optional: false },
      { id: "timber-frame", label: "Oak timber frame", optional: false },
      { id: "windows", label: "Windows and flower box", optional: false },
      { id: "roof", label: "Steep spruce roof", optional: false },
      {
        id: "porch",
        label: entry === "stoop" ? "Entry stoop" : "Canopy porch",
        optional: true,
      },
      { id: "chimney", label: "Brick chimney", optional: true },
      { id: "weathering", label: "Foundation weathering", optional: true },
    ],
    palette: cottagePalette(),
    defaultTheme: "warm-oak-and-plaster-v2",
    themes: [cottageTheme()],
  }, ({ component, fillBox, lineX, lineY, lineZ, marker, set, socket }) => {
    component("shell", () => {
      fillBox([1, 0, 2], [14, 1, frontZ + 1], "foundation");
      fillBox([1, 1, 2], [14, 7, frontZ + 1], "wall");
      fillBox([2, 2, 3], [13, 6, frontZ], "air");
      fillBox([2, 1, 3], [13, 2, frontZ], "floor");
    });

    component("timber-frame", () => {
      for (const [x, z] of [[1, 2], [13, 2], [1, frontZ], [13, frontZ]] as const) {
        lineY(x, 1, 7, z, "timberY");
      }
      lineX(1, 14, 6, 2, "timberX");
      lineX(1, 14, 6, frontZ, "timberX");
      lineZ(1, 6, 2, frontZ + 1, "timberZ");
      lineZ(13, 6, 2, frontZ + 1, "timberZ");
    });

    component("windows", () => {
      fillBox([9, 2, frontZ], [10, 5, frontZ + 1], "air");
      fillBox([3, 2, frontZ], [6, 4, frontZ + 1], "glazing");
      fillBox([11, 3, frontZ], [13, 5, frontZ + 1], "glazing");
      lineX(3, 6, 4, frontZ, "timberX");
      lineX(11, 13, 5, frontZ, "timberX");
      fillBox([3, 1, frontZ + 1], [6, 2, frontZ + 2], "floor");
      set([3, 2, frontZ + 1], "poppy");
      set([5, 2, frontZ + 1], "cornflower");

      const westWindowZ = frontZ / 2;
      fillBox([1, 2, westWindowZ], [2, 4, westWindowZ + 3], "glazing");
      lineZ(1, 4, westWindowZ, westWindowZ + 3, "timberZ");
      fillBox([13, 3, 4], [14, 5, 6], "glazing");
      lineZ(13, 5, 4, 6, "timberZ");
      if (depth === "deep") {
        fillBox([13, 2, 9], [14, 4, 12], "glazing");
        lineZ(13, 4, 9, 12, "timberZ");
      }
    });

    component("roof", () => {
      for (let layer = 0; layer <= 6; layer += 1) {
        const y = 6 + layer;
        const leftRoofX = layer;
        const rightRoofX = 14 - layer;
        if (layer > 0) {
          fillBox([leftRoofX + 1, y, 2], [rightRoofX, y + 1, 3], "wall");
          fillBox([leftRoofX + 1, y, frontZ], [rightRoofX, y + 1, frontZ + 1], "wall");
        }
        lineZ(leftRoofX, y, 1, frontZ + 2, "roofEast");
        lineZ(rightRoofX, y, 1, frontZ + 2, "roofWest");
      }
      lineZ(7, 13, 1, frontZ + 2, "roofSlabBottom");
      lineY(7, 6, 13, 2, "timberY");
      lineY(7, 6, 13, frontZ, "timberY");
      fillBox([6, 8, frontZ], [7, 10, frontZ + 1], "glazing");
      fillBox([8, 8, frontZ], [9, 10, frontZ + 1], "glazing");
    });

    component("porch", () => {
      if (entry === "stoop") {
        fillBox([8, 1, frontZ + 1], [11, 2, frontZ + 2], "roofSlabBottom");
        fillBox([8, 5, frontZ], [11, 6, frontZ + 2], "roofSlabBottom");
        set([8, 3, frontZ + 1], "wallTorchSouth");
      } else {
        fillBox([8, 0, frontZ + 1], [12, 1, frontZ + 3], "foundation");
        fillBox([8, 1, frontZ + 1], [12, 2, frontZ + 3], "floor");
        fillBox([8, 1, frontZ + 3], [12, 2, frontZ + 4], "roofSlabBottom");
        lineY(8, 2, 5, frontZ + 1, "timberY");
        lineY(11, 2, 5, frontZ + 1, "timberY");
        fillBox([7, 5, frontZ], [13, 6, frontZ + 2], "roofSlabBottom");
        set([7, 3, frontZ + 1], "wallTorchSouth");
      }
    });

    component("chimney", () => {
      fillBox([10, 5, 5], [11, 14, 7], "accent");
      set([9, 14, 5], "accent");
      set([11, 14, 6], "accent");
    });

    component("weathering", () => {
      for (const pos of [
        [2, 0, frontZ],
        [5, 0, frontZ],
        [12, 0, frontZ],
        [1, 0, 5],
        [13, 0, 9],
      ] as const) {
        set(pos, "mossyCobblestone");
      }
    });

    const westYard = [1, 2, frontZ / 2 + 1] as const;
    marker("entrance:south", [9, 2, sizeZ - 1]);
    marker("attachment:west-yard", westYard);
    socket("west-yard", "attachment:yard", westYard, "west");
  });
}

function cottagePalette() {
  return {
    accent: role("accent"),
    air: block("minecraft:air"),
    cornflower: block("minecraft:cornflower"),
    floor: role("floor"),
    foundation: role("foundation"),
    glazing: role("glazing"),
    mossyCobblestone: block("minecraft:mossy_cobblestone"),
    poppy: block("minecraft:poppy"),
    roofEast: role("roofEast"),
    roofSlabBottom: role("roofSlabBottom"),
    roofWest: role("roofWest"),
    timberX: role("timberX"),
    timberY: role("timberY"),
    timberZ: role("timberZ"),
    wall: role("wall"),
    wallTorchSouth: block("minecraft:wall_torch[facing=south]"),
  } as const;
}

function cottageTheme() {
  return {
    id: "warm-oak-and-plaster-v2",
    label: "Warm oak and plaster",
    materials: {
      foundation: "minecraft:cobblestone",
      wall: "minecraft:white_terracotta",
      timberY: "minecraft:oak_log[axis=y]",
      timberX: "minecraft:oak_log[axis=x]",
      timberZ: "minecraft:oak_log[axis=z]",
      roof: "minecraft:spruce_planks",
      roofNorth: "minecraft:spruce_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]",
      roofEast: "minecraft:spruce_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]",
      roofSouth: "minecraft:spruce_stairs[facing=south,half=bottom,shape=straight,waterlogged=false]",
      roofWest: "minecraft:spruce_stairs[facing=west,half=bottom,shape=straight,waterlogged=false]",
      roofSlabBottom: "minecraft:spruce_slab[type=bottom,waterlogged=false]",
      roofSlabTop: "minecraft:spruce_slab[type=top,waterlogged=false]",
      glazing: "minecraft:glass",
      trim: "minecraft:oak_planks",
      floor: "minecraft:oak_planks",
      accent: "minecraft:bricks",
    },
  } as const;
}
