import { figure } from "../../src/dsl";

// A box-only giant panda derived from the reviewed heavy bear vocabulary.
// Black shoulder mass, limbs, ears, and eye patches carry the identity while
// the large white head and rump keep the familiar high-contrast silhouette.
export default figure("panda", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("white", "#eee9dc");
  mat("white_shadow", "#d7d0c2");
  mat("black", "#17191a");
  mat("black_soft", "#252829");
  mat("muzzle", "#e7dfd2");

  asciiTexture("face", {
    palette: { ".": "#eee9dc", "p": "#17191a", "e": "#73613e" },
    pixels: [
      "........",
      ".pp..pp.",
      "ppe..epp",
      ".pp..pp.",
      "........",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#e7dfd2", "n": "#101112", "m": "#574c44" },
    pixels: [
      "........",
      "..nnnn..",
      "...nn...",
      "..mmmm..",
      "...mm...",
      "........",
    ],
  });
  asciiTexture("paw_face", {
    palette: { "p": "#252829", "c": "#101112" },
    pixels: [
      "pppppppp",
      "pcpccpcp",
      "cccccccc",
    ],
  });

  part("body", box({
    at: [0, 1.11, 0.12],
    size: [1.08, 0.84, 1.5],
    material: "white",
  }));
  part("shoulder_band", box({
    parent: "body",
    at: [0, 0.12, -0.48],
    rot: [-3, 0, 0],
    size: [1.1, 0.68, 0.55],
    material: "black",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.44, 0.18],
    size: [0.88, 0.12, 0.9],
    material: "white_shadow",
  }));
  part("neck", box({
    parent: "shoulder_band",
    at: [0, 0.14, -0.47],
    rot: [-8, 0, 0],
    size: [0.68, 0.46, 0.42],
    material: "black",
    joint: { pivot: [0, -0.21, 0.14], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.13, -0.43],
    rot: [7, 0, 0],
    size: [0.76, 0.64, 0.58],
    material: "white",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.13, -0.42],
    size: [0.48, 0.26, 0.28],
    material: "muzzle",
    faces: { north: { texture: "snout_face" } },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.32, 0.36, 0],
    size: [0.22, 0.22, 0.14],
    material: "black",
    joint: { pivot: [0, -0.09, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.32, 0.36, 0],
    size: [0.22, 0.22, 0.14],
    material: "black",
    joint: { pivot: [0, -0.09, 0], axis: [1, 0, 0] },
  }));

  for (const [suffix, x, z, rear] of [
    ["fl", -0.36, -0.48, false],
    ["fr", 0.36, -0.48, false],
    ["bl", -0.38, 0.5, true],
    ["br", 0.38, 0.5, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.62, z],
      size: [rear ? 0.31 : 0.28, rear ? 0.54 : 0.51, rear ? 0.35 : 0.31],
      material: "black",
      joint: { pivot: [0, rear ? 0.27 : 0.255, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.33 : -0.315, -0.075],
      size: [rear ? 0.37 : 0.35, 0.13, rear ? 0.45 : 0.41],
      material: "black_soft",
      faces: { north: { texture: "paw_face" } },
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.08, 0.82],
    rot: [38, 0, 0],
    size: [0.18, 0.21, 0.18],
    material: "white_shadow",
    joint: { pivot: [0, 0.095, 0], axis: [0, 0, 1] },
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1.38,
    cycleDistance: 0.74,
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
