import { figure } from "../../src/dsl";

// A heavy box-only brown bear based on the vanilla polar bear's two-mass
// torso, broad short legs, projecting muzzle, tiny ears, and oversized paws.
export default figure("bear", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#6f4328");
  mat("fur_mid", "#7c5030");
  mat("fur_dark", "#3f2819");
  mat("fur_shadow", "#2c1c13");
  mat("muzzle", "#a97a52");

  asciiTexture("face", {
    palette: { ".": "#6f4328", "d": "#3f2819", "e": "#120d09", "m": "#a97a52" },
    pixels: [
      "dddddddd",
      "dd....dd",
      ".e....e.",
      "..mmmm..",
      "..mmmm..",
      "...mm...",
      "........",
      "........",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#a97a52", "d": "#7c5030", "n": "#17100c" },
    pixels: [
      "..dddd..",
      ".dddddd.",
      "..nnnn..",
      "...nn...",
      "........",
      "........",
    ],
  });
  asciiTexture("claw_face", {
    palette: { "p": "#2c1c13", "c": "#15100c" },
    pixels: [
      "pppppppp",
      "pcpccpcp",
      "cccccccc",
    ],
  });

  part("body", box({
    at: [0, 1.12, 0.1],
    size: [1.08, 0.82, 1.52],
    material: "fur",
  }));
  part("shoulder_hump", box({
    parent: "body",
    at: [0, 0.42, -0.43],
    rot: [-4, 0, 0],
    size: [1, 0.34, 0.68],
    material: "fur_dark",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.43, 0.02],
    size: [0.9, 0.12, 1.14],
    material: "fur_shadow",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.18, -0.83],
    rot: [-10, 0, 0],
    size: [0.66, 0.48, 0.46],
    material: "fur_mid",
    joint: { pivot: [0, -0.24, 0.16], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.12, -0.42],
    rot: [8, 0, 0],
    size: [0.74, 0.56, 0.56],
    material: "fur",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.11, -0.4],
    size: [0.46, 0.24, 0.28],
    material: "muzzle",
    faces: { north: { texture: "snout_face" } },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.29, 0.32, -0.01],
    size: [0.19, 0.19, 0.12],
    material: "fur_dark",
    joint: { pivot: [0, -0.08, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.29, 0.32, -0.01],
    size: [0.19, 0.19, 0.12],
    material: "fur_dark",
    joint: { pivot: [0, -0.08, 0], axis: [1, 0, 0] },
  }));

  for (const [suffix, x, z, rear] of [
    ["fl", -0.36, -0.48, false],
    ["fr", 0.36, -0.48, false],
    ["bl", -0.38, 0.5, true],
    ["br", 0.38, 0.5, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.61, z],
      size: [rear ? 0.3 : 0.27, rear ? 0.52 : 0.5, rear ? 0.34 : 0.3],
      material: "fur_dark",
      joint: { pivot: [0, rear ? 0.26 : 0.25, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.32 : -0.31, -0.075],
      size: [rear ? 0.36 : 0.34, 0.13, rear ? 0.44 : 0.4],
      material: "fur_shadow",
      faces: { north: { texture: "claw_face" } },
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.08, 0.82],
    rot: [38, 0, 0],
    size: [0.16, 0.2, 0.16],
    material: "fur_dark",
    joint: { pivot: [0, 0.09, 0], axis: [0, 0, 1] },
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1.35,
    cycleDistance: 0.76,
    gait: "walk",
    loop: true,
    samples: 13,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.009,
    bodyBobCenter: 0.01,
    head: "head",
    headSwingDegrees: 1.8,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.72,
    swingDegrees: 13,
    tail: "tail",
    tailSwingDegrees: 3,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.16 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.5, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.5, lag: 0.12 }),
    ],
  });
});
