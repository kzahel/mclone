import { block, role, structure } from "../../src/dsl";

export default structure({
  id: "farmstead-kitchen-garden-v1",
  label: "Kitchen Garden",
  description: "A compact fenced kitchen garden with a working gate, a central irrigation run, and mixed wheat and carrot beds.",
  category: "infrastructure",
  size: [13, 2, 13],
  family: {
    id: "farmstead-garden",
    member: "kitchen-mixed-crops",
    label: "Farmstead Garden",
  },
  tags: ["crops", "farmstead", "garden", "starter-settlement"],
  components: [
    { id: "approach", label: "Gate approach and crossing", optional: false },
    { id: "beds", label: "Hydrated mixed crop beds", optional: false },
    { id: "boundary", label: "Connected oak enclosure", optional: false },
    { id: "irrigation", label: "Central irrigation run", optional: false },
  ],
  palette: {
    carrotGrowing: block("minecraft:carrots[age=3]"),
    cropPrimary: role("cropPrimary"),
    cropSecondary: role("cropSecondary"),
    fence: role("fence"),
    gate: role("gate"),
    grass: block("minecraft:grass_block"),
    path: role("floor"),
    soil: role("soil"),
    water: block("minecraft:water[level=0]"),
    wheatGrowing: block("minecraft:wheat[age=4]"),
  },
  defaultTheme: "oak-wheat-and-carrots-v1",
  themes: [{
    id: "oak-wheat-and-carrots-v1",
    label: "Oak, wheat, and carrots",
    materials: {
      cropPrimary: "minecraft:wheat[age=7]",
      cropSecondary: "minecraft:carrots[age=7]",
      fence: "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=false,west=false]",
      floor: "minecraft:oak_planks",
      gate: "minecraft:oak_fence_gate[facing=south,in_wall=false,open=false,powered=false]",
      soil: "minecraft:farmland[moisture=7]",
    },
  }],
}, ({ component, fillBox, lineX, lineZ, marker, set, socket }) => {
  component("boundary", () => {
    fillBox([0, 0, 0], [13, 1, 13], "grass");
    lineX(1, 12, 1, 2, "fence");
    lineX(1, 12, 1, 10, "fence");
    lineZ(1, 1, 3, 10, "fence");
    lineZ(11, 1, 3, 10, "fence");
    set([6, 1, 10], "gate");
  });

  component("beds", () => {
    for (const [minX, maxX, minZ, maxZ, mature, growing] of [
      [2, 6, 3, 6, "cropPrimary", "wheatGrowing"],
      [7, 11, 3, 6, "cropSecondary", "carrotGrowing"],
      [2, 6, 7, 10, "cropSecondary", "carrotGrowing"],
      [7, 11, 7, 10, "cropPrimary", "wheatGrowing"],
    ] as const) {
      fillBox([minX, 0, minZ], [maxX, 1, maxZ], "soil");
      for (let z = minZ; z < maxZ; z += 1) {
        for (let x = minX; x < maxX; x += 1) {
          set([x, 1, z], z === minZ + 1 ? growing : mature);
        }
      }
    }
  });

  component("irrigation", () => {
    lineX(2, 6, 0, 6, "water");
    lineX(7, 11, 0, 6, "water");
    marker("water:irrigation", [3, 0, 6]);
  });

  component("approach", () => {
    lineZ(6, 0, 6, 13, "path");
    marker("entrance:south", [6, 1, 12]);
    marker("harvest:carrots", [8, 1, 7]);
    socket("south-path", "attachment:path", [6, 0, 12], "south");
  });
});
