import { figure } from "../../src/dsl";

// A box-only blue dasher dragonfly with huge compound eyes, a four-stage
// striped abdomen, four independently rooted cell-pattern wings, and six legs.
export default figure("dragonfly", ({
  mat,
  asciiTexture,
  part,
  box,
  wingFlap,
  swing,
  followThrough,
}) => {
  mat("blue", "#357d99");
  mat("blue_light", "#5e9eb1");
  mat("blue_dark", "#24536b");
  mat("black", "#25282a");
  mat("brown", "#51473e");
  mat("eye", "#426b5d");
  mat("eye_light", "#88a95e");
  mat("wing", "#b7d8dc");

  asciiTexture("face", {
    palette: {
      ".": "#25282a",
      "e": "#426b5d",
      "g": "#88a95e",
      "b": "#357d99",
    },
    pixels: [
      "eeeeeeee",
      "eggeegge",
      "egg..gge",
      "eeeeeeee",
      "..bbbb..",
      "........",
    ],
  });
  asciiTexture("thorax_marks", {
    palette: { ".": "#357d99", "l": "#5e9eb1", "d": "#24536b", "b": "#25282a" },
    pixels: [
      "llllllll",
      "l......l",
      ".dd..dd.",
      "..d..d..",
      ".bb..bb.",
      "bbbbbbbb",
    ],
  });
  asciiTexture("abdomen_bands", {
    palette: { ".": "#357d99", "l": "#5e9eb1", "d": "#24536b", "b": "#25282a" },
    pixels: [
      "llllllllll",
      "..........",
      "dddddddddd",
      "..........",
      "bbbbbbbbbb",
      "..........",
    ],
  });
  asciiTexture("wing_cells", {
    palette: { ".": "#b7d8dc", "e": "#6e979f", "h": "#dcebed", "d": "#425b61" },
    pixels: [
      "eeeeeeeeeeee",
      "ehh.ehh.ehhe",
      "e..e..e.e..e",
      "ehh.ehh.ehhe",
      "e..e..e.e..e",
      "eeeeeeeeeeed",
    ],
  });

  part("thorax", box({
    at: [0, 0.72, 0],
    size: [0.34, 0.38, 0.38],
    material: "blue",
    faces: {
      up: { texture: "thorax_marks" },
      east: { texture: "thorax_marks" },
      west: { texture: "thorax_marks" },
    },
  }));
  part("head", box({
    parent: "thorax",
    at: [0, 0.01, -0.36],
    size: [0.44, 0.34, 0.34],
    material: "black",
    faces: { north: { texture: "face" } },
  }));
  for (const [side, x, roll] of [["l", -0.11, 18], ["r", 0.11, -18]] as const) {
    part(`antenna_${side}`, box({
      parent: "head",
      at: [x, 0.19, -0.08],
      rot: [-12, 0, roll],
      size: [0.025, 0.2, 0.025],
      material: "black",
    }));
  }

  part("abdomen_1", box({
    parent: "thorax",
    at: [0, -0.02, 0.42],
    size: [0.22, 0.22, 0.52],
    material: "blue_light",
    faces: {
      up: { texture: "abdomen_bands" },
      east: { texture: "abdomen_bands" },
      west: { texture: "abdomen_bands" },
    },
    joint: { pivot: [0, 0, -0.25], axis: [1, 0, 0] },
  }));
  part("abdomen_2", box({
    parent: "abdomen_1",
    at: [0, -0.01, 0.4],
    size: [0.16, 0.14, 0.34],
    material: "blue",
    faces: {
      up: { texture: "abdomen_bands" },
      east: { texture: "abdomen_bands" },
      west: { texture: "abdomen_bands" },
    },
    joint: { pivot: [0, 0, -0.16], axis: [1, 0, 0] },
  }));
  part("abdomen_3", box({
    parent: "abdomen_2",
    at: [0, -0.015, 0.29],
    size: [0.12, 0.1, 0.28],
    material: "blue_dark",
    faces: {
      up: { texture: "abdomen_bands" },
      east: { texture: "abdomen_bands" },
      west: { texture: "abdomen_bands" },
    },
    joint: { pivot: [0, 0, -0.13], axis: [1, 0, 0] },
  }));
  part("abdomen_tip", box({
    parent: "abdomen_3",
    at: [0, -0.01, 0.2],
    size: [0.09, 0.075, 0.14],
    material: "black",
  }));

  for (const [side, x] of [["l", -0.52], ["r", 0.52]] as const) {
    part(`forewing_${side}`, box({
      parent: "thorax",
      at: [x, 0.23, -0.05],
      rot: [0, side === "l" ? -8 : 8, 0],
      size: [0.82, 0.035, 0.28],
      material: "wing",
      faces: {
        up: { texture: "wing_cells" },
        down: { texture: "wing_cells" },
      },
      joint: {
        pivot: [side === "l" ? 0.4 : -0.4, 0, -0.03],
        axis: [0, 0, 1],
      },
    }));
    part(`hindwing_${side}`, box({
      parent: "thorax",
      at: [x, 0.17, 0.27],
      rot: [0, side === "l" ? 10 : -10, 0],
      size: [0.74, 0.03, 0.26],
      material: "wing",
      faces: {
        up: { texture: "wing_cells" },
        down: { texture: "wing_cells" },
      },
      joint: {
        pivot: [side === "l" ? 0.36 : -0.36, 0, 0.03],
        axis: [0, 0, 1],
      },
    }));
  }

  for (const [row, z, yaw] of [
    ["front", -0.18, 14],
    ["mid", 0, 0],
    ["rear", 0.18, -14],
  ] as const) {
    for (const [side, x, roll] of [["l", -0.28, 18], ["r", 0.28, -18]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "thorax",
        at: [x, -0.23, z],
        rot: [0, side === "l" ? -yaw : yaw, roll],
        size: [0.3, 0.035, 0.035],
        material: "brown",
      }));
    }
  }

  wingFlap("dart", {
    fps: 24,
    duration: 0.72,
    cycleDistance: 1.3,
    loop: true,
    samples: 21,
    body: "thorax",
    bodyBob: 0.025,
    degrees: 23,
    frequency: 2,
    leftWing: "forewing_l",
    rightWing: "forewing_r",
    tracks: [
      swing("hindwing_l", { axis: "z", degrees: 20, frequency: 2, phase: 0.16 }),
      swing("hindwing_r", { axis: "z", degrees: -20, frequency: 2, phase: 0.16 }),
      followThrough("abdomen_1", { source: "thorax", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.08 }),
      followThrough("abdomen_2", { source: "thorax", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.55, lag: 0.13 }),
      followThrough("abdomen_3", { source: "thorax", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.65, lag: 0.18 }),
    ],
  });
});
