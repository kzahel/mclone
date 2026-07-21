import { figure } from "../../src/dsl";

// A box-only adult polar bear with a long low body, sloping shoulders, narrow
// neck and head, tiny ears, and oversized snowshoe paws.
export default figure("polar_bear", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#e8e6d8");
  mat("fur_light", "#f4f2e7");
  mat("fur_mid", "#d6d8d0");
  mat("fur_shadow", "#b7c0c0");
  mat("skin", "#252927");
  mat("claw", "#4b514e");

  asciiTexture("face", {
    palette: { ".": "#e8e6d8", "s": "#b7c0c0", "e": "#171a19" },
    pixels: [
      "ss....ss",
      "s......s",
      "..e..e..",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#f4f2e7", "s": "#d6d8d0", "n": "#252927" },
    pixels: [
      "..ssss..",
      ".ssssss.",
      "..nnnn..",
      ".nnnnnn.",
      "...nn...",
      "........",
    ],
  });
  asciiTexture("claw_face", {
    palette: { ".": "#b7c0c0", "c": "#4b514e" },
    pixels: [
      "........",
      ".c.c.c..",
      "cccccccc",
    ],
  });

  part("body", box({
    at: [0, 1.09, 0.1],
    size: [1.02, 0.76, 1.74],
    material: "fur",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.02, 0.56],
    size: [1.08, 0.8, 0.72],
    material: "fur_light",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.24, -0.58],
    rot: [-3, 0, 0],
    size: [0.96, 0.54, 0.58],
    material: "fur_mid",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.4, 0],
    size: [0.84, 0.12, 1.28],
    material: "fur_shadow",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.15, -0.94],
    rot: [-6, 0, 0],
    size: [0.62, 0.5, 0.62],
    material: "fur_mid",
    joint: { pivot: [0, -0.23, 0.2], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.08, -0.48],
    rot: [4, 0, 0],
    size: [0.7, 0.52, 0.62],
    material: "fur",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.11, -0.45],
    size: [0.42, 0.23, 0.32],
    material: "fur_light",
    faces: { north: { texture: "snout_face" } },
  }));
  for (const [side, x] of [["l", -0.25], ["r", 0.25]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.29, 0],
      size: [0.14, 0.15, 0.1],
      material: "fur_shadow",
      joint: { pivot: [0, -0.06, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.34, -0.55, false],
    ["fr", 0.34, -0.55, false],
    ["bl", -0.37, 0.56, true],
    ["br", 0.37, 0.56, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.59, z],
      size: [rear ? 0.3 : 0.27, rear ? 0.56 : 0.52, rear ? 0.35 : 0.31],
      material: rear ? "fur_mid" : "fur_shadow",
      joint: { pivot: [0, rear ? 0.28 : 0.26, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.34 : -0.32, -0.09],
      size: [rear ? 0.4 : 0.38, 0.14, rear ? 0.52 : 0.48],
      material: "fur_shadow",
      faces: { north: { texture: "claw_face" } },
    }));
  }

  part("tail", box({
    parent: "rump",
    at: [0, 0.06, 0.42],
    rot: [34, 0, 0],
    size: [0.15, 0.19, 0.15],
    material: "fur_mid",
    joint: { pivot: [0, 0.08, 0], axis: [0, 0, 1] },
  }));

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.42,
    cycleDistance: 0.72,
    gait: "walk",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.009,
    bodyBobCenter: 0.011,
    head: "head",
    headSwingDegrees: 1.7,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.74,
    swingDegrees: 12,
    tail: "tail",
    tailSwingDegrees: 3,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.4, lag: 0.16 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.12 }),
    ],
  });
});
