import { figure } from "../../src/dsl";

// A box-only European hedgehog with a low cream body, stepped dark spine coat,
// tiny legs, small ears, and a pointed face carried by two cuboid muzzle stages.
export default figure("hedgehog", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#a9825a");
  mat("fur_light", "#c9ae82");
  mat("spine", "#4a3b2c");
  mat("spine_light", "#786044");
  mat("spine_dark", "#2b251e");
  mat("cream", "#ddcba8");
  mat("black", "#1d1b18");
  mat("paw", "#65503d");

  asciiTexture("face", {
    palette: { ".": "#a9825a", "e": "#171411", "l": "#c9ae82", "c": "#ddcba8" },
    pixels: [
      "ll....ll",
      ".e....e.",
      "..c..c..",
      "..cccc..",
      "...cc...",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#ddcba8", "n": "#1d1b18" },
    pixels: [
      "........",
      "..nnnn..",
      ".nnnnnn.",
      "...nn...",
      "........",
    ],
  });
  asciiTexture("spine_side", {
    palette: { ".": "#4a3b2c", "l": "#786044", "d": "#2b251e", "c": "#ddcba8" },
    pixels: [
      "dldldldldldl",
      "ldldldldldld",
      "dldldldldldl",
      "ldldldldldld",
      "ddlllllllldd",
      "cccccccccccc",
    ],
  });
  asciiTexture("spine_top", {
    palette: { ".": "#4a3b2c", "l": "#786044", "d": "#2b251e" },
    pixels: [
      "dldldldldl",
      "ldldldldld",
      "dldldldldl",
      "ldldldldld",
      "dldldldldl",
    ],
  });

  part("body", box({
    at: [0, 0.47, 0.05],
    size: [0.7, 0.4, 1.0],
    material: "fur",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.23, -0.02],
    size: [0.54, 0.1, 0.7],
    material: "cream",
  }));
  part("spine_back", box({
    parent: "body",
    at: [0, 0.15, 0.1],
    size: [0.72, 0.36, 0.7],
    material: "spine",
    faces: {
      east: { texture: "spine_side" },
      west: { texture: "spine_side" },
      up: { texture: "spine_top" },
    },
  }));
  part("spine_front", box({
    parent: "body",
    at: [0, 0.08, -0.25],
    size: [0.64, 0.28, 0.32],
    material: "spine_light",
    faces: {
      east: { texture: "spine_side" },
      west: { texture: "spine_side" },
    },
  }));
  part("spine_rear", box({
    parent: "body",
    at: [0, 0.1, 0.4],
    size: [0.64, 0.3, 0.28],
    material: "spine_dark",
    faces: {
      east: { texture: "spine_side" },
      west: { texture: "spine_side" },
    },
  }));
  part("spine_crown", box({
    parent: "body",
    at: [0, 0.32, 0.08],
    size: [0.55, 0.18, 0.5],
    material: "spine_dark",
    faces: { up: { texture: "spine_top" } },
  }));
  part("head", box({
    parent: "body",
    at: [0, -0.02, -0.64],
    size: [0.44, 0.36, 0.42],
    material: "fur_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.18], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.11, -0.31],
    size: [0.28, 0.2, 0.22],
    material: "cream",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.17], ["r", 0.17]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.21, 0.03],
      size: [0.12, 0.14, 0.09],
      material: "spine_light",
      joint: { pivot: [0, -0.05, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.23, -0.28],
    ["fr", 0.23, -0.28],
    ["bl", -0.23, 0.3],
    ["br", 0.23, 0.3],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.32, z],
      size: [0.11, 0.24, 0.12],
      material: "fur",
      joint: { pivot: [0, 0.12, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.15, -0.035],
      size: [0.15, 0.08, 0.19],
      material: "paw",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.03, 0.57],
    rot: [32, 0, 0],
    size: [0.1, 0.16, 0.1],
    material: "spine_dark",
    joint: { pivot: [0, 0.07, 0], axis: [0, 0, 1] },
  }));

  quadrupedWalk("scurry", {
    fps: 20,
    duration: 0.7,
    cycleDistance: 0.48,
    gait: "trot",
    loop: true,
    samples: 17,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.56,
    swingDegrees: 20,
    tail: "tail",
    tailSwingDegrees: 5,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.6, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.6, lag: 0.1 }),
    ],
  });
});
