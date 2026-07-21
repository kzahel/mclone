import { figure } from "../../src/dsl";

// A box-only toco toucan with a compact black body, white throat, blue eye
// ring, enormous three-stage orange bill, layered wings, and short dark tail.
export default figure("toucan", ({
  mat,
  asciiTexture,
  part,
  box,
  wingFlap,
  followThrough,
}) => {
  mat("black", "#202120");
  mat("black_light", "#343735");
  mat("black_gloss", "#4a504c");
  mat("white", "#f0ead9");
  mat("blue", "#4c91a0");
  mat("orange", "#e79425");
  mat("yellow", "#f1bd3f");
  mat("red", "#c94c32");
  mat("beak_tip", "#2c2926");
  mat("foot", "#6e8390");

  asciiTexture("face", {
    palette: { ".": "#202120", "b": "#202120", "w": "#f0ead9", "u": "#4c91a0", "e": "#171817" },
    pixels: [
      "bbbbbbbb",
      "bu....ub",
      "bue..eub",
      "buu..uub",
      "bwwwwwwb",
      "wwwwwwww",
    ],
  });
  asciiTexture("bill_side", {
    palette: { "o": "#e79425", "y": "#f1bd3f", "r": "#c94c32", "b": "#2c2926" },
    pixels: [
      "yyyyyyyyyyyyyy",
      "yoooooooooooob",
      "yooorrrrooooob",
      "ooorrrrrroooob",
      "ooooorooooooob",
      "rrrrrrrrrrrbbb",
    ],
  });
  asciiTexture("wing_feathers", {
    palette: { ".": "#202120", "l": "#4a504c", "d": "#111312" },
    pixels: [
      "llllllllll",
      "l........l",
      "..ll..ll..",
      ".l..ll..l.",
      "ddd....ddd",
      "dddddddddd",
    ],
  });
  asciiTexture("tail_feathers", {
    palette: { ".": "#202120", "l": "#4a504c", "r": "#c94c32" },
    pixels: [
      "llllllll",
      "l......l",
      "..l..l..",
      "........",
      "rrrrrrrr",
      "........",
    ],
  });

  part("body", box({
    at: [0, 0.82, 0.08],
    size: [0.62, 0.72, 0.78],
    material: "black",
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.03, -0.44],
    size: [0.48, 0.56, 0.16],
    material: "white",
  }));
  part("throat", box({
    parent: "body",
    at: [0, 0.36, -0.3],
    size: [0.48, 0.34, 0.32],
    material: "white",
  }));
  part("head", box({
    parent: "throat",
    at: [0, 0.24, -0.15],
    size: [0.56, 0.48, 0.46],
    material: "black_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, -0.2, 0.13], axis: [1, 0, 0] },
  }));
  part("bill_base", box({
    parent: "head",
    at: [0, 0.02, -0.43],
    size: [0.46, 0.38, 0.46],
    material: "yellow",
    faces: {
      east: { texture: "bill_side" },
      west: { texture: "bill_side" },
    },
  }));
  part("bill_mid", box({
    parent: "bill_base",
    at: [0, -0.03, -0.36],
    size: [0.4, 0.32, 0.34],
    material: "orange",
    faces: {
      east: { texture: "bill_side" },
      west: { texture: "bill_side" },
    },
  }));
  part("bill_tip", box({
    parent: "bill_mid",
    at: [0, -0.05, -0.25],
    rot: [6, 0, 0],
    size: [0.3, 0.24, 0.22],
    material: "beak_tip",
  }));

  for (const [side, x] of [["l", -0.52], ["r", 0.52]] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [x, 0.08, 0.03],
      size: [0.82, 0.08, 0.7],
      material: "black_light",
      faces: {
        up: { texture: "wing_feathers" },
        down: { texture: "wing_feathers" },
      },
      joint: { pivot: [side === "l" ? 0.4 : -0.4, 0, -0.1], axis: [0, 0, 1] },
    }));
    part(`wing_tip_${side}`, box({
      parent: `wing_${side}`,
      at: [side === "l" ? -0.58 : 0.58, -0.01, 0.05],
      rot: [0, side === "l" ? 7 : -7, 0],
      size: [0.44, 0.065, 0.56],
      material: "black",
      faces: {
        up: { texture: "wing_feathers" },
        down: { texture: "wing_feathers" },
      },
    }));
  }
  part("tail", box({
    parent: "body",
    at: [0, 0.02, 0.65],
    rot: [6, 0, 0],
    size: [0.52, 0.09, 0.58],
    material: "black",
    faces: {
      up: { texture: "tail_feathers" },
      down: { texture: "tail_feathers" },
    },
    joint: { pivot: [0, 0, -0.27], axis: [1, 0, 0] },
  }));
  for (const [side, x] of [["l", -0.17], ["r", 0.17]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.43, -0.02],
      rot: [-35, 0, 0],
      size: [0.11, 0.26, 0.12],
      material: "foot",
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.13, 0.08],
      size: [0.22, 0.08, 0.28],
      material: "beak_tip",
    }));
  }

  wingFlap("fly", {
    fps: 18,
    duration: 0.94,
    cycleDistance: 1.3,
    loop: true,
    samples: 17,
    body: "body",
    bodyBob: 0.045,
    degrees: 35,
    leftWing: "wing_l",
    rightWing: "wing_r",
    tracks: [
      followThrough("head", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.11 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.17 }),
    ],
  });
});
