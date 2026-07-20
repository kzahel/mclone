import { figure } from "../../src/dsl";

// A box-only male lion. A lighter feline barrel is framed by a block mane at
// the chest and head; the long two-piece tail ends in a dark cuboid tuft.
export default figure("lion", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#c88d3d");
  mat("coat_light", "#e4bd72");
  mat("coat_shadow", "#9a632e");
  mat("mane", "#5b321d");
  mat("mane_dark", "#2e1a12");
  mat("paw", "#6b3d20");

  asciiTexture("face", {
    palette: {
      ".": "#c88d3d",
      "m": "#5b321d",
      "d": "#2e1a12",
      "e": "#d7b44f",
      "c": "#e4bd72",
    },
    pixels: [
      "mmmmmmmm",
      "md....dm",
      "m.e..e.m",
      "m......m",
      "..cccc..",
      ".cccccc.",
      "..cccc..",
      "........",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#e4bd72", "s": "#9a632e", "n": "#16100c", "w": "#f0d99a" },
    pixels: [
      "..wwww..",
      ".wwwwww.",
      ".s....s.",
      "..nnnn..",
      "...nn...",
      "........",
    ],
  });
  asciiTexture("paw_face", {
    palette: { "p": "#6b3d20", "c": "#140f0b" },
    pixels: [
      "pppppp",
      "pcpccp",
      "pppppp",
    ],
  });

  part("body", box({
    at: [0, 1.03, 0],
    size: [1.02, 0.6, 1.58],
    material: "coat",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.32, -0.02],
    size: [0.74, 0.12, 1.04],
    material: "coat_light",
  }));
  part("chest", box({
    parent: "body",
    at: [0, 0.03, -0.66],
    size: [1.08, 0.6, 0.46],
    material: "mane",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.23, -0.86],
    rot: [-12, 0, 0],
    size: [0.68, 0.52, 0.44],
    material: "mane",
    joint: { pivot: [0, -0.25, 0.16], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.14, -0.39],
    rot: [8, 0, 0],
    size: [0.64, 0.5, 0.52],
    material: "coat",
    faces: { north: { texture: "face" } },
  }));
  part("mane_back", box({
    parent: "head",
    at: [0, 0.02, 0.2],
    size: [0.82, 0.66, 0.24],
    material: "mane",
  }));
  part("mane_top", box({
    parent: "head",
    at: [0, 0.34, -0.01],
    size: [0.76, 0.18, 0.46],
    material: "mane_dark",
  }));
  part("mane_l", box({
    parent: "head",
    at: [-0.42, 0.01, 0],
    size: [0.2, 0.54, 0.44],
    material: "mane",
  }));
  part("mane_r", box({
    parent: "head",
    at: [0.42, 0.01, 0],
    size: [0.2, 0.54, 0.44],
    material: "mane",
  }));
  part("mane_beard", box({
    parent: "head",
    at: [0, -0.34, 0],
    size: [0.52, 0.2, 0.4],
    material: "mane_dark",
    joint: { pivot: [0, 0.09, 0], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.11, -0.41],
    size: [0.48, 0.24, 0.3],
    material: "coat_light",
    faces: { north: { texture: "snout_face" } },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.29, 0.28, -0.03],
    size: [0.18, 0.18, 0.12],
    material: "mane_dark",
    joint: { pivot: [0, -0.08, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.29, 0.28, -0.03],
    size: [0.18, 0.18, 0.12],
    material: "mane_dark",
    joint: { pivot: [0, -0.08, 0], axis: [1, 0, 0] },
  }));

  for (const [suffix, x, z, rear] of [
    ["fl", -0.34, -0.48, false],
    ["fr", 0.34, -0.48, false],
    ["bl", -0.36, 0.48, true],
    ["br", 0.36, 0.48, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.52, z],
      size: [rear ? 0.23 : 0.2, rear ? 0.58 : 0.56, rear ? 0.24 : 0.21],
      material: rear ? "coat" : "coat_shadow",
      joint: { pivot: [0, rear ? 0.29 : 0.28, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.35 : -0.34, -0.06],
      size: [rear ? 0.3 : 0.28, 0.11, rear ? 0.36 : 0.34],
      material: "paw",
      faces: { north: { texture: "paw_face" } },
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.35, 0.8],
    rot: [46, 0, 0],
    size: [0.11, 0.78, 0.11],
    material: "coat",
    joint: { pivot: [0, -0.39, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", box({
    parent: "tail",
    at: [0, 0.5, 0.08],
    rot: [12, 0, 0],
    size: [0.25, 0.3, 0.24],
    material: "mane_dark",
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1.05,
    cycleDistance: 0.96,
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
    bodyBob: 0.011,
    bodyBobCenter: 0.013,
    head: "head",
    headSwingDegrees: 2.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.66,
    swingDegrees: 18,
    tail: "tail",
    tailSwingDegrees: 9,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.5, lag: 0.14 }),
      followThrough("mane_beard", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.18 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.45, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.45, lag: 0.12 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.7, lag: 0.2 }),
    ],
  });
});
