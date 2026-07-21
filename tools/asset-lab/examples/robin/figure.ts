import { figure } from "../../src/dsl";

// A box-only European robin with a compact olive-brown body, warm orange face
// and breast, fine wing bars, and a quick two-beat flutter.
export default figure("robin", ({
  mat,
  asciiTexture,
  part,
  box,
  wingFlap,
  followThrough,
}) => {
  mat("brown", "#665d49");
  mat("brown_light", "#847861");
  mat("brown_dark", "#403c32");
  mat("olive", "#77745a");
  mat("orange", "#ca5932");
  mat("orange_light", "#e07745");
  mat("cream", "#c9bea0");
  mat("beak", "#272824");
  mat("foot", "#67564d");

  asciiTexture("face", {
    palette: {
      ".": "#665d49",
      "o": "#ca5932",
      "l": "#e07745",
      "e": "#171816",
    },
    pixels: [
      "........",
      ".ee..ee.",
      ".ee..ee.",
      "oolllloo",
      "oooooooo",
      "oooooooo",
    ],
  });
  asciiTexture("breast_fleck", {
    palette: { ".": "#ca5932", "l": "#e07745", "c": "#c9bea0" },
    pixels: [
      "llllllll",
      "l......l",
      "..l..l..",
      ".l....l.",
      "..c..c..",
      "cccccccc",
    ],
  });
  asciiTexture("wing_feathers", {
    palette: {
      ".": "#665d49",
      "l": "#847861",
      "d": "#403c32",
      "c": "#c9bea0",
    },
    pixels: [
      "llllllllll",
      "l........l",
      "..cc..cc..",
      ".d..dd..d.",
      "dd......dd",
      "dddddddddd",
    ],
  });
  asciiTexture("tail_feathers", {
    palette: { ".": "#665d49", "l": "#847861", "d": "#403c32" },
    pixels: ["llllllll", "l......l", "..l..l..", "........", "dddddddd"],
  });

  part("body", box({
    at: [0, 0.7, 0.04],
    size: [0.56, 0.62, 0.74],
    material: "olive",
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.04, -0.43],
    size: [0.46, 0.48, 0.17],
    material: "orange",
    faces: { north: { texture: "breast_fleck" } },
  }));
  part("throat", box({
    parent: "body",
    at: [0, 0.25, -0.34],
    size: [0.42, 0.3, 0.28],
    material: "orange_light",
  }));
  part("head", box({
    parent: "throat",
    at: [0, 0.22, -0.18],
    size: [0.46, 0.44, 0.42],
    material: "brown",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, -0.18, 0.1], axis: [1, 0, 0] },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.03, -0.31],
    rot: [4, 0, 0],
    size: [0.17, 0.1, 0.21],
    material: "beak",
  }));
  part("beak_tip", box({
    parent: "beak",
    at: [0, -0.02, -0.15],
    rot: [5, 0, 0],
    size: [0.115, 0.07, 0.09],
    material: "brown_dark",
  }));

  for (const [side, x] of [["l", -0.47], ["r", 0.47]] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [x, 0.08, 0.03],
      size: [0.7, 0.07, 0.6],
      material: "brown_light",
      faces: {
        up: { texture: "wing_feathers" },
        down: { texture: "wing_feathers" },
      },
      joint: {
        pivot: [side === "l" ? 0.34 : -0.34, 0, -0.08],
        axis: [0, 0, 1],
      },
    }));
    part(`wing_tip_${side}`, box({
      parent: `wing_${side}`,
      at: [side === "l" ? -0.5 : 0.5, -0.01, 0.05],
      rot: [0, side === "l" ? 8 : -8, 0],
      size: [0.36, 0.055, 0.46],
      material: "brown_dark",
      faces: {
        up: { texture: "wing_feathers" },
        down: { texture: "wing_feathers" },
      },
    }));
  }

  for (const [side, x, yaw] of [["l", -0.11, 4], ["r", 0.11, -4]] as const) {
    part(`tail_${side}`, box({
      parent: "body",
      at: [x, 0, 0.62],
      rot: [8, yaw, 0],
      size: [0.18, 0.065, 0.58],
      material: "brown",
      faces: {
        up: { texture: "tail_feathers" },
        down: { texture: "tail_feathers" },
      },
      joint: { pivot: [0, 0, -0.28], axis: [1, 0, 0] },
    }));
  }

  for (const [side, x] of [["l", -0.15], ["r", 0.15]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.37, -0.02],
      rot: [-42, 0, 0],
      size: [0.085, 0.22, 0.085],
      material: "foot",
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.1, 0.07],
      size: [0.16, 0.055, 0.21],
      material: "brown_dark",
    }));
  }

  wingFlap("flutter", {
    fps: 20,
    duration: 0.78,
    cycleDistance: 1.1,
    loop: true,
    samples: 19,
    body: "body",
    bodyBob: 0.046,
    degrees: 38,
    frequency: 2,
    leftWing: "wing_l",
    rightWing: "wing_r",
    tracks: [
      followThrough("head", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.5, lag: 0.09 }),
      followThrough("tail_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.65, lag: 0.15 }),
      followThrough("tail_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.65, lag: 0.15 }),
    ],
  });
});
