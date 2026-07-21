import { block, role, structure } from "../../src/dsl";

export default structure({
  id: "farmstead-rosehip-chicken-coop-v1",
  label: "Rosehip Chicken Coop",
  description: "A raised red-and-oak henhouse with sunny windows, hay nests, a little ramp, and an open timber run.",
  category: "outbuilding",
  size: [16, 11, 12],
  family: {
    id: "farmstead-coop",
    member: "rosehip-raised-run",
    label: "Farmstead Coop",
  },
  tags: ["chicken-coop", "farmstead", "livestock", "starter-settlement"],
  components: [
    { id: "house", label: "Raised red henhouse", optional: false },
    { id: "frame", label: "Oak frame and ramp", optional: false },
    { id: "windows", label: "Sunny windows", optional: false },
    { id: "roof", label: "Steep spruce roof", optional: false },
    { id: "nesting", label: "Hay nests and roost", optional: true },
    { id: "run", label: "Open timber chicken run", optional: true },
    { id: "details", label: "Lantern and rose pot", optional: true },
  ],
  palette: {
    accent: role("accent"),
    air: block("minecraft:air"),
    floor: role("floor"),
    foundation: role("foundation"),
    glazing: role("glazing"),
    poppy: block("minecraft:poppy"),
    roofEast: role("roofEast"),
    roofSlabBottom: role("roofSlabBottom"),
    roofWest: role("roofWest"),
    timberX: role("timberX"),
    timberY: role("timberY"),
    timberZ: role("timberZ"),
    trim: role("trim"),
    wall: role("wall"),
    wallTorchSouth: block("minecraft:wall_torch[facing=south]"),
  },
  defaultTheme: "rosehip-red-and-oak-v1",
  themes: [{
    id: "rosehip-red-and-oak-v1",
    label: "Rosehip red and oak",
    materials: {
      foundation: "minecraft:stone_bricks",
      wall: "minecraft:red_terracotta",
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
      trim: "minecraft:white_terracotta",
      floor: "minecraft:oak_planks",
      accent: "minecraft:hay_block",
    },
  }],
}, ({ component, fillBox, lineX, lineY, lineZ, marker, set, socket }) => {
  component("house", () => {
    for (const [x, z] of [[1, 2], [7, 2], [1, 8], [7, 8]] as const) {
      lineY(x, 0, 2, z, "foundation");
    }
    fillBox([1, 2, 2], [8, 3, 9], "floor");
    fillBox([1, 3, 2], [8, 7, 9], "wall");
    fillBox([2, 4, 3], [7, 7, 8], "air");
    fillBox([4, 3, 8], [6, 6, 9], "air");
  });

  component("frame", () => {
    for (const [x, z] of [[1, 2], [7, 2], [1, 8], [7, 8]] as const) {
      lineY(x, 3, 7, z, "timberY");
    }
    lineX(1, 8, 6, 2, "timberX");
    lineX(1, 8, 6, 8, "timberX");
    lineZ(1, 6, 2, 9, "timberZ");
    lineZ(7, 6, 2, 9, "timberZ");
    fillBox([4, 1, 9], [6, 2, 10], "floor");
    fillBox([4, 0, 10], [6, 1, 11], "floor");
  });

  component("windows", () => {
    fillBox([1, 4, 4], [2, 6, 7], "glazing");
    fillBox([7, 4, 4], [8, 6, 7], "glazing");
    fillBox([2, 4, 8], [4, 6, 9], "glazing");
    lineZ(1, 6, 4, 7, "timberZ");
    lineZ(7, 6, 4, 7, "timberZ");
    lineX(2, 4, 6, 8, "trim");
  });

  component("roof", () => {
    for (let layer = 0; layer <= 3; layer += 1) {
      const y = 7 + layer;
      lineZ(layer, y, 1, 10, "roofEast");
      lineZ(8 - layer, y, 1, 10, "roofWest");
      if (layer > 0) {
        fillBox([layer + 1, y - 1, 2], [8 - layer, y, 3], "wall");
        fillBox([layer + 1, y - 1, 8], [8 - layer, y, 9], "wall");
      }
    }
    lineZ(4, 10, 1, 10, "roofSlabBottom");
    lineY(4, 6, 10, 2, "timberY");
    lineY(4, 6, 10, 8, "timberY");
  });

  component("nesting", () => {
    fillBox([2, 3, 3], [4, 4, 5], "accent");
    fillBox([5, 3, 3], [7, 4, 5], "accent");
    lineZ(5, 4, 3, 8, "timberZ");
  });

  component("run", () => {
    for (const [x, z] of [
      [9, 2], [12, 2], [15, 2],
      [9, 10], [12, 10], [15, 10],
      [15, 6],
    ] as const) {
      lineY(x, 0, 4, z, "timberY");
    }
    for (const y of [1, 3]) {
      lineX(8, 16, y, 2, "timberX");
      lineX(8, 16, y, 10, "timberX");
      lineZ(15, y, 2, 11, "timberZ");
    }
    fillBox([12, 0, 5], [14, 1, 7], "accent");
  });

  component("details", () => {
    set([6, 4, 8], "wallTorchSouth");
    set([2, 2, 9], "floor");
    set([2, 3, 9], "poppy");
  });

  marker("entrance:south", [5, 1, 11]);
  marker("animal:chickens", [11, 1, 6]);
  marker("attachment:east-yard", [15, 1, 6]);
  socket("east-yard", "attachment:yard", [15, 1, 6], "east");
});
