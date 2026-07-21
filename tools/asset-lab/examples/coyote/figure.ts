import { figure } from "../../src/dsl";

// A box-only desert coyote with a narrow frame, oversized ears, pale throat,
// long legs, pointed muzzle, and a low black-tipped brush tail.
export default figure("coyote", ({
  asciiTexture,
  box,
  defaultClip,
  followThrough,
  mat,
  part,
  quadrupedWalk,
  swing,
}) => {
  mat("sand", "#aa8256");
  mat("sand_light", "#cfb184");
  mat("gray", "#746d61");
  mat("cream", "#e0cfaa");
  mat("brown", "#51453a");
  mat("black", "#252522");

  asciiTexture("saddle", {
    palette: { ".": "#aa8256", "g": "#746d61", "d": "#51453a", "c": "#cfb184" },
    pixels: [
      "dddddddddddd",
      "dggggggggggd",
      "ggg......ggg",
      "g..........g",
      "..cccccccc..",
      ".cccccccccc.",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#aa8256", "c": "#e0cfaa", "e": "#d2b65f", "b": "#252522" },
    pixels: [
      "bb......bb",
      "be......eb",
      "..e....e..",
      "..cccccc..",
      ".cccccccc.",
      "....bb....",
    ],
  });
  asciiTexture("muzzle", {
    palette: { ".": "#e0cfaa", "d": "#51453a", "n": "#252522" },
    pixels: [
      "..dddd..",
      ".dddddd.",
      "...nn...",
      "..nnnn..",
      "........",
    ],
  });

  part("body", box({
    at: [0, 0.78, 0.08],
    size: [0.62, 0.46, 1.18],
    material: "sand",
    faces: {
      east: { texture: "saddle" },
      west: { texture: "saddle" },
    },
  }));
  part("chest", box({
    parent: "body",
    at: [0, 0.05, -0.48],
    size: [0.68, 0.55, 0.46],
    material: "gray",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.02, 0.45],
    size: [0.66, 0.48, 0.42],
    material: "sand",
  }));
  part("throat", box({
    parent: "chest",
    at: [0, -0.13, -0.3],
    rot: [-8, 0, 0],
    size: [0.38, 0.38, 0.34],
    material: "cream",
    joint: { pivot: [0, 0, 0.14], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "throat",
    at: [0, 0.13, -0.32],
    rot: [4, 0, 0],
    size: [0.52, 0.43, 0.46],
    material: "sand",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.08, -0.36],
    size: [0.28, 0.2, 0.34],
    material: "cream",
    faces: { north: { texture: "muzzle" } },
  }));
  part("nose", box({
    parent: "muzzle",
    at: [0, 0.01, -0.2],
    size: [0.2, 0.14, 0.09],
    material: "black",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.18, 0.34, 0.03],
      rot: [-5, 0, sign * -7],
      size: [0.17, 0.42, 0.14],
      material: "brown",
      joint: { pivot: [0, -0.19, 0], axis: [1, 0, 0] },
    }));
    part(`ear_inner_${side}`, box({
      parent: `ear_${side}`,
      at: [0, 0.03, -0.075],
      size: [0.09, 0.28, 0.035],
      material: "sand_light",
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.22, -0.39, false],
    ["fr", 0.22, -0.39, false],
    ["bl", -0.22, 0.4, true],
    ["br", 0.22, 0.4, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.4, z],
      size: [0.13, rear ? 0.5 : 0.48, 0.14],
      material: rear ? "gray" : "sand_light",
      joint: { pivot: [0, rear ? 0.24 : 0.23, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.31 : -0.3, -0.06],
      size: [0.18, 0.1, 0.25],
      material: "brown",
    }));
  }

  part("tail_1", box({
    parent: "rump",
    at: [0, 0.02, 0.42],
    rot: [12, 0, 0],
    size: [0.27, 0.27, 0.58],
    material: "gray",
    joint: { pivot: [0, 0, -0.26], axis: [0, 1, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, -0.05, 0.48],
    rot: [12, 0, 0],
    size: [0.24, 0.24, 0.48],
    material: "sand_light",
    joint: { pivot: [0, 0, -0.21], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_2",
    at: [0, -0.05, 0.35],
    rot: [9, 0, 0],
    size: [0.19, 0.19, 0.3],
    material: "black",
  }));

  quadrupedWalk("trot", {
    label: "Desert trot",
    role: "locomotion",
    fps: 18,
    duration: 0.82,
    cycleDistance: 0.92,
    gait: "trot",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.014,
    bodyBobCenter: 0.014,
    head: "throat",
    headSwingDegrees: 3,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.57,
    swingDegrees: 24,
    tail: "tail_1",
    tailSwingDegrees: 8,
    tracks: [
      swing("tail_2", { axis: "y", degrees: 10, phase: 0.12 }),
      swing("tail_tip", { axis: "y", degrees: 13, phase: 0.22 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.6, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.6, lag: 0.1 }),
    ],
  });
  defaultClip("trot");
});
