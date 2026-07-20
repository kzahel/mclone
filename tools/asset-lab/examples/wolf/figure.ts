import { figure } from "../../src/dsl";

// A tall, long-backed wolf based on the vanilla two-mass torso. The shoulder
// block, narrow legs, upright ears, and straight heavy tail separate it from
// the broad domestic dog and low fox.
export default figure("wolf", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#6d7479");
  mat("coat_dark", "#2f3438");
  mat("coat_mid", "#535b60");
  mat("coat_light", "#c7c2b6");
  mat("paw", "#242729");

  asciiTexture("face", {
    palette: {
      ".": "#6d7479",
      "d": "#2f3438",
      "e": "#d7b15a",
      "p": "#c7c2b6",
    },
    pixels: [
      "dd....dd",
      "de....ed",
      ".e....e.",
      "..pppp..",
      "..pppp..",
      "...pp...",
      "........",
      "........",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#c7c2b6", "d": "#535b60", "n": "#111315" },
    pixels: [
      "..dddd..",
      ".dddddd.",
      "...nn...",
      "..nnnn..",
      "........",
      "........",
    ],
  });
  asciiTexture("wolf_side", {
    palette: { ".": "#535b60", "d": "#2f3438", "l": "#c7c2b6" },
    pixels: [
      "dddddddddddd",
      "ddd......ddd",
      "............",
      "............",
      "..llllllll..",
      ".llllllllll.",
    ],
  });

  part("body", box({
    at: [0, 1.02, 0.12],
    size: [0.72, 0.5, 1.32],
    material: "coat_mid",
    faces: {
      east: { texture: "wolf_side" },
      west: { texture: "wolf_side" },
    },
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.08, -0.62],
    size: [0.84, 0.62, 0.58],
    material: "coat",
  }));
  part("head", box({
    parent: "shoulders",
    at: [0, 0.19, -0.52],
    rot: [7, 0, 0],
    size: [0.52, 0.46, 0.48],
    material: "coat",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.21], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.09, -0.38],
    size: [0.32, 0.21, 0.34],
    material: "coat_light",
    faces: { north: { texture: "snout_face" } },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.19, 0.31, 0],
    rot: [-7, 0, -10],
    size: [0.15, 0.34, 0.11],
    material: "coat_dark",
    joint: { pivot: [0, -0.15, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.19, 0.31, 0],
    rot: [-7, 0, 10],
    size: [0.15, 0.34, 0.11],
    material: "coat_dark",
    joint: { pivot: [0, -0.15, 0], axis: [1, 0, 0] },
  }));

  for (const [suffix, x, z, dark] of [
    ["fl", -0.25, -0.43, false],
    ["fr", 0.25, -0.43, false],
    ["bl", -0.25, 0.47, true],
    ["br", 0.25, 0.47, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.48, z],
      size: [0.14, 0.54, 0.15],
      material: dark ? "coat_dark" : "coat",
      joint: { pivot: [0, 0.27, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.32, -0.045],
      size: [0.19, 0.1, 0.25],
      material: "paw",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.18, 0.75],
    rot: [37, 0, 0],
    size: [0.19, 0.66, 0.19],
    material: "coat_dark",
    joint: { pivot: [0, 0.33, 0], axis: [0, 0, 1] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, -0.44, 0.08],
    rot: [-9, 0, 0],
    size: [0.21, 0.28, 0.21],
    material: "coat_mid",
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 0.86,
    cycleDistance: 0.94,
    gait: "trot",
    loop: true,
    samples: 13,
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
    stanceRatio: 0.58,
    swingDegrees: 23,
    tail: "tail",
    tailSwingDegrees: 13,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.1 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.7, lag: 0.18 }),
    ],
  });
});
