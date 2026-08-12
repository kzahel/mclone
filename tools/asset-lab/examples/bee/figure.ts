import { figure } from "../../src/dsl";

// A box-only worker bee with a striped abdomen, fuzzy thorax, large dark eyes,
// paired pale wings, six tucked legs, antennae, and a rapid hovering flap.
export default figure("bee", ({
  mat,
  asciiTexture,
  defaultClip,
  metadata,
  part,
  box,
  wingFlap,
  followThrough,
  swing,
}) => {
  metadata({
    bodyPlans: ["winged"],
    disposition: "passive",
    groups: ["animal"],
    habitats: ["air", "land"],
    scale: "tiny",
    themes: ["bee", "flower", "pollinator", "temperate"],
  });

  mat("yellow", "#d9a72e");
  mat("yellow_light", "#eccb58");
  mat("brown", "#4a3525");
  mat("brown_light", "#69503a");
  mat("black", "#211d1a");
  mat("eye", "#293b43");
  mat("wing", "#b9dbe0");
  mat("wing_edge", "#789ca5");

  asciiTexture("face", {
    palette: {
      ".": "#4a3525",
      "e": "#293b43",
      "g": "#6c9299",
      "m": "#211d1a",
    },
    pixels: [
      "eeeeeeee",
      "eggeegge",
      "eggeegge",
      "eeeeeeee",
      "...mm...",
      "........",
    ],
  });
  asciiTexture("abdomen_bands", {
    palette: {
      ".": "#d9a72e",
      "l": "#eccb58",
      "b": "#4a3525",
      "k": "#211d1a",
    },
    pixels: [
      "llllllllll",
      "..........",
      "bbbbbbbbbb",
      "bbbbbbbbbb",
      "..........",
      "kkkkkkkkkk",
      "kkkkkkkkkk",
    ],
  });
  asciiTexture("thorax_fuzz", {
    palette: { ".": "#4a3525", "l": "#69503a", "y": "#d9a72e" },
    pixels: [
      "ll....ll",
      ".llllll.",
      "ll.yy.ll",
      ".l....l.",
      "ll....ll",
      ".llllll.",
    ],
  });
  asciiTexture("wing_cells", {
    palette: {
      ".": "#b9dbe0",
      "e": "#789ca5",
      "h": "#dbeaec",
    },
    pixels: [
      "eeeeeeeeee",
      "ehh.ehh.he",
      "e..e..e..e",
      "ehh.ehh.he",
      "e..e..e..e",
      "eeeeeeeeee",
    ],
  });

  part("abdomen", box({
    at: [0, 0.68, 0.24],
    size: [0.44, 0.4, 0.78],
    material: "yellow",
    faces: {
      up: { texture: "abdomen_bands" },
      east: { texture: "abdomen_bands" },
      west: { texture: "abdomen_bands" },
    },
  }));
  part("abdomen_tip", box({
    parent: "abdomen",
    at: [0, -0.02, 0.47],
    size: [0.32, 0.3, 0.24],
    material: "brown",
    joint: { pivot: [0, 0, -0.11], axis: [1, 0, 0] },
  }));
  part("stinger", box({
    parent: "abdomen_tip",
    at: [0, -0.02, 0.18],
    size: [0.1, 0.09, 0.14],
    material: "black",
  }));
  part("thorax", box({
    parent: "abdomen",
    at: [0, 0.03, -0.55],
    size: [0.5, 0.5, 0.48],
    material: "brown_light",
    faces: {
      east: { texture: "thorax_fuzz" },
      west: { texture: "thorax_fuzz" },
    },
  }));
  part("head", box({
    parent: "thorax",
    at: [0, 0.01, -0.42],
    size: [0.44, 0.42, 0.4],
    material: "brown",
    faces: { north: { texture: "face" } },
  }));

  for (const [side, x, roll] of [["l", -0.14, 24], ["r", 0.14, -24]] as const) {
    part(`antenna_${side}`, box({
      parent: "head",
      at: [x, 0.25, -0.09],
      rot: [-18, 0, roll],
      size: [0.035, 0.34, 0.035],
      material: "black",
    }));
  }

  for (const [side, x] of [["l", -0.43], ["r", 0.43]] as const) {
    part(`forewing_${side}`, box({
      parent: "thorax",
      at: [x, 0.17, -0.03],
      rot: [0, side === "l" ? -12 : 12, 0],
      size: [0.64, 0.035, 0.5],
      material: "wing",
      faces: {
        up: { texture: "wing_cells" },
        down: { texture: "wing_cells" },
      },
      joint: {
        pivot: [side === "l" ? 0.31 : -0.31, 0, -0.08],
        axis: [0, 0, 1],
      },
    }));
    part(`hindwing_${side}`, box({
      parent: `forewing_${side}`,
      at: [side === "l" ? 0.03 : -0.03, -0.005, 0.34],
      rot: [0, side === "l" ? 8 : -8, 0],
      size: [0.48, 0.03, 0.34],
      material: "wing",
      faces: {
        up: { texture: "wing_cells" },
        down: { texture: "wing_cells" },
      },
    }));
  }

  for (const [row, z, yaw] of [
    ["front", -0.2, 12],
    ["mid", 0, 0],
    ["rear", 0.2, -12],
  ] as const) {
    for (const [side, x, roll] of [["l", -0.31, 20], ["r", 0.31, -20]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "thorax",
        at: [x, -0.27, z],
        rot: [0, side === "l" ? yaw : -yaw, roll],
        size: [0.34, 0.045, 0.045],
        material: "black",
      }));
    }
  }

  wingFlap("hover", {
    label: "Colony hover",
    role: "idle",
    fps: 24,
    duration: 0.8,
    loop: true,
    samples: 21,
    body: "abdomen",
    bodyBob: 0.04,
    degrees: 28,
    frequency: 3,
    leftWing: "forewing_l",
    rightWing: "forewing_r",
    tracks: [
      followThrough("abdomen_tip", { source: "abdomen", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.12 }),
    ],
  });
  wingFlap("fly", {
    label: "Forage flight",
    role: "locomotion",
    fps: 30,
    duration: 0.58,
    cycleDistance: 0.68,
    loop: true,
    samples: 19,
    body: "abdomen",
    bodyBob: 0.025,
    degrees: 38,
    frequency: 4,
    leftWing: "forewing_l",
    rightWing: "forewing_r",
    tracks: [
      swing("abdomen", { axis: "x", center: -8, degrees: 2.5, frequency: 1 }),
      followThrough("abdomen_tip", { source: "abdomen", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.5, lag: 0.1 }),
      swing("antenna_l", { axis: "x", center: -8, degrees: 6, frequency: 1, phase: 0.12 }),
      swing("antenna_r", { axis: "x", center: -8, degrees: 6, frequency: 1, phase: 0.12 }),
    ],
  });
  wingFlap("forage", {
    label: "Flower forage",
    role: "action",
    fps: 24,
    duration: 1.08,
    loop: true,
    samples: 27,
    body: "abdomen",
    bodyBob: 0.018,
    bodyBobCenter: -0.04,
    degrees: 18,
    frequency: 2,
    leftWing: "forewing_l",
    rightWing: "forewing_r",
    tracks: [
      swing("abdomen", { axis: "x", center: 23, degrees: 5, frequency: 1 }),
      swing("head", { axis: "x", center: 16, degrees: 8, frequency: 1, phase: 0.12 }),
      swing("antenna_l", { axis: "x", center: 12, degrees: 12, frequency: 1, phase: 0.25 }),
      swing("antenna_r", { axis: "x", center: 12, degrees: 12, frequency: 1, phase: 0.25 }),
      followThrough("abdomen_tip", { source: "abdomen", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 5, overshoot: 0.35, lag: 0.1 }),
    ],
  });
  defaultClip("hover");
});
